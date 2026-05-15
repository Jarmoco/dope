/* -----------------------------------------------------------------------------
 * main.rs
 * MITM proxy entry point — loads the CA certificate, reads config, and starts
 * the proxy on the configured port.
 * -------------------------------------------------------------------------- */

use hudsucker::{
    certificate_authority::RcgenAuthority,
    rcgen::{Issuer, KeyPair},
    rustls::crypto::aws_lc_rs,
    Proxy,
};
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::*;
use tracing_appender;
use tracing_subscriber::{prelude::*, EnvFilter};

mod charset;
mod handler;
mod inject;
mod logging;
mod modify;
mod scripts;

/* --- Help ------------------------------------------------------------------ */

fn print_help() {
    println!("dope — MITM proxy with userscript injection");
    println!();
    println!("USAGE:");
    println!("  dope [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("  -h, --help          Print this help message");
    println!("  -pp                 Pretty-print logs (no timestamps, no targets)");
    println!("  --scripts <path>    Userscript directory           [default: scripts]");
    println!("  --logs <path>       Log output directory           [default: logs]");
    println!("  --ca <path>         CA certificate directory       [default: ca]");
    println!("  --config <path>     Configuration file path        [default: config.toml]");
    println!();
    println!("ENVIRONMENT:");
    println!("  RUST_LOG            Log level (trace, debug, info, warn, error)");
    println!("                      Default: info");
}

/* --- Config Summary -------------------------------------------------------- */

fn print_config_summary(cfg: &dope_core::Config) {
    info!("----------");
    info!("Target domains:");

    if let Some(scripts) = &cfg.scripts {
        for rule in scripts {
            let list = rule
                .scripts
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ");
            info!("- {} [{}]", rule.domain, list);
        }
    }

    if let Some(modifiers) = &cfg.modify_response {
        for m in modifiers {
            info!("  modify_response on {}:", m.domain);
            if let Some(ref csp) = m.csp {
                info!("    csp: {}", csp);
            }
            if let Some(ref remove) = m.remove_headers {
                info!("    remove_headers: [{}]", remove.join(", "));
            }
            if let Some(ref add) = m.add_headers {
                for (k, v) in add {
                    info!("    add_header: {} = {}", k, v);
                }
            }
            if let Some(ref pos) = m.inject_at {
                info!("    inject_at: {}", pos);
            }
        }
    }

    if let Some(modifiers) = &cfg.modify_request {
        for m in modifiers {
            info!("  modify_request on {}:", m.domain);
            if let Some(ref remove) = m.remove_headers {
                info!("    remove_headers: [{}]", remove.join(", "));
            }
            if let Some(ref add) = m.add_headers {
                for (k, v) in add {
                    info!("    add_header: {} = {}", k, v);
                }
            }
        }
    }

    info!("----------");
}

/* --- Shutdown -------------------------------------------------------------- */

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}

/* --- Main ------------------------------------------------------------------ */

#[tokio::main]
async fn main() {
    /* --- CLI argument parsing ---------------------------------------------- */

    let mut pretty = false;
    let mut scripts_dir = PathBuf::from("scripts");
    let mut logs_dir = PathBuf::from("logs");
    let mut ca_dir = PathBuf::from("ca");
    let mut config_path = PathBuf::from("config.toml");

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-pp" => pretty = true,
            "--scripts" if i + 1 < args.len() => {
                i += 1;
                scripts_dir = PathBuf::from(&args[i]);
            }
            "--logs" if i + 1 < args.len() => {
                i += 1;
                logs_dir = PathBuf::from(&args[i]);
            }
            "--ca" if i + 1 < args.len() => {
                i += 1;
                ca_dir = PathBuf::from(&args[i]);
            }
            "--config" if i + 1 < args.len() => {
                i += 1;
                config_path = PathBuf::from(&args[i]);
            }
            _ => {
                eprintln!("Unknown option: {} (use --help for usage)", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    dope_core::init_dirs(dope_core::Dirs {
        config: config_path,
        scripts: scripts_dir,
        logs: logs_dir.clone(),
    });

    /* --- Tracing ----------------------------------------------------------- */

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_appender = tracing_appender::rolling::never(&logs_dir, "dope.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer);

    if std::io::stdout().is_terminal() {
        if pretty {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .pretty();

            subscriber.with(stdout_layer).init();
        } else {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true);

            subscriber.with(stdout_layer).init();
        }
    } else {
        subscriber.init();
    }

    /* --- CA certificate ---------------------------------------------------- */

    let ca_key_path = ca_dir.join("ca.key");
    let ca_cert_path = ca_dir.join("ca.cer");

    let key_pem = match std::fs::read_to_string(&ca_key_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read {}: {}", ca_key_path.display(), e);
            error!("Generate with: openssl req -x509 -newkey rsa:4096 -keyout {} -out {} -days 365 -nodes",
                   ca_key_path.display(), ca_cert_path.display());
            error!("Then add {} to your browser's trusted certificates.", ca_cert_path.display());
            return;
        }
    };

    let ca_cert_pem = match std::fs::read_to_string(&ca_cert_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read {}: {}", ca_cert_path.display(), e);
            error!("Generate with: openssl req -x509 -newkey rsa:4096 -keyout {} -out {} -days 365 -nodes",
                   ca_key_path.display(), ca_cert_path.display());
            error!("Then add {} to your browser's trusted certificates.", ca_cert_path.display());
            return;
        }
    };

    let key_pair = match KeyPair::from_pem(&key_pem) {
        Ok(kp) => kp,
        Err(e) => {
            error!("Failed to parse private key: {}", e);
            return;
        }
    };

    let issuer = match Issuer::from_ca_cert_pem(&ca_cert_pem, key_pair) {
        Ok(issuer) => issuer,
        Err(e) => {
            error!("Failed to parse CA certificate: {}", e);
            return;
        }
    };

    let ca = RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider());

    /* --- Start proxy ------------------------------------------------------- */

    let traffic_handler = handler::TrafficHandler::new();

    let cfg = dope_core::load_config();
    let port = cfg.server.port;

    print_config_summary(&cfg);

    let proxy = Proxy::builder()
        .with_addr(SocketAddr::from(([127, 0, 0, 1], port)))
        .with_ca(ca)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(traffic_handler.clone())
        .with_websocket_handler(traffic_handler.clone())
        .with_graceful_shutdown(shutdown_signal())
        .build();

    match proxy {
        Ok(p) => {
            info!("Proxy running on http://127.0.0.1:{}", port);
            if let Err(e) = p.start().await {
                error!("Proxy error: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to create proxy: {}", e);
        }
    }
}
