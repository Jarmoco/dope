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
use tracing::*;
use tracing_appender;
use tracing_subscriber::{prelude::*, EnvFilter};

mod config;
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
    println!("  -h, --help       Print this help message");
    println!();
    println!("ENVIRONMENT:");
    println!("  RUST_LOG         Log level (trace, debug, info, warn, error)");
    println!("                   Default: info");
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
    if std::env::args().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_appender = tracing_appender::rolling::never("logs", "dope.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer);

    if std::io::stdout().is_terminal() {
        let stdout_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true);

        subscriber.with(stdout_layer).init();
    } else {
        subscriber.init();
    }

    let ca_key_path = "ca/ca.key";
    let ca_cert_path = "ca/ca.cer";

    let key_pem = match std::fs::read_to_string(ca_key_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read {}: {}", ca_key_path, e);
            error!("Generate with: openssl req -x509 -newkey rsa:4096 -keyout ca/ca.key -out ca/ca.cer -days 365 -nodes");
            error!("Then add ca/ca.cer to your browser's trusted certificates.");
            return;
        }
    };

    let ca_cert_pem = match std::fs::read_to_string(ca_cert_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read {}: {}", ca_cert_path, e);
            error!("Generate with: openssl req -x509 -newkey rsa:4096 -keyout ca/ca.key -out ca/ca.cer -days 365 -nodes");
            error!("Then add ca/ca.cer to your browser's trusted certificates.");
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

    let traffic_handler = handler::TrafficHandler::new();

    let cfg = config::load_config();
    let port = cfg.server.port;

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
