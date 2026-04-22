/* -----------------------------------------------------------------------------
 * MITM Proxy with Userscript Injection
 * -----------------------------------------------------------------------------
 *
 * This module implements a man-in-the-middle HTTP/HTTPS proxy that intercepts
 * and modifies HTML responses to inject userscripts in the ViolentMonkey /
 * GreaseMonkey format
 *
 * Architecture:
 * - TrafficHandler: Core HTTP request/response interceptor
 * - Decompression: Handles gzip, brotli, deflate, and zstd encodings transparently
 * - Script Injection: Reads userscripts from filesystem and injects them into HTML
 * - GM Polyfill: Automatically injects greasemonkey API polyfill when needed
 * - CSP Modification: Removes restrictive CSP headers to allow script execution
 *
 * Design Choices:
 * - Stream-based decompression: Memory efficient for large responses
 * - Early returns: Keep main logic at lowest indentation level
 * - Clone domain: Needed for use across multiple await points
 */

use http_body_util::BodyDataStream;

use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use hudsucker::{
    Body, HttpContext,
    certificate_authority::RcgenAuthority,
    futures::TryStreamExt,
    hyper::{Request, Response, header},
    rcgen::{Issuer, KeyPair},
    rustls::crypto::aws_lc_rs,
    tokio_tungstenite::tungstenite::Message,
    *,
};
use std::net::SocketAddr;
use tracing::*;

mod config;
mod scripts;

#[derive(Clone)]
struct TrafficHandler {
    current_domain: Option<String>,
}

impl HttpHandler for TrafficHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let host = req.uri().host().unwrap_or("unknown").to_string();

        self.current_domain = Some(host);
        req.into()
    }

    /* -----------------------------------------------------------------------------
     * HTTP Response Processing & Script Injection
     * -------------------------------------------------------------------------- */

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        let is_html = res
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.to_str().unwrap_or("").contains("text/html"));

        if !is_html {
            return res;
        }

        let (mut parts, body) = res.into_parts();

        let stream = BodyDataStream::new(body).map_err(std::io::Error::other);
        let reader = StreamReader::new(stream);

        let mut decoded_bytes = Vec::new();

        let encoding = parts
            .headers
            .get(header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        /* Decompress before injection to get readable HTML */
        match encoding {
            "gzip" => {
                let mut decoder = GzipDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Gzip decompression failed: {}", e);
                    return Response::from_parts(parts, Body::from(String::new()));
                }
            }
            "br" => {
                let mut decoder = BrotliDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Brotli decompression failed: {}", e);
                    return Response::from_parts(parts, Body::from(String::new()));
                }
            }
            "deflate" => {
                let mut decoder = ZlibDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Zlib decompression failed: {}", e);
                    return Response::from_parts(parts, Body::from(String::new()));
                }
            }
            "zstd" => {
                let mut decoder = ZstdDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Zstd decompression failed: {}", e);
                    return Response::from_parts(parts, Body::from(String::new()));
                }
            }
            _ => {
                if !encoding.is_empty() {
                    warn!("Unsupported encoding: {}", encoding);
                }
                let mut r = reader;
                if let Err(e) = r.read_to_end(&mut decoded_bytes).await {
                    error!("Body read failed: {}", e);
                    return Response::from_parts(parts, Body::from(String::new()));
                }
            }
        }

        let mut html = String::from_utf8_lossy(&decoded_bytes).into_owned();

        /* -----------------------------------------------------------------------------
         * Script Injection Logic
         * -------------------------------------------------------------------------- */
        /* Inject scripts at the end of body to ensure DOM is ready */
        let config = config::load_config();
        let websites_with_rules = config.get_websites();

        if let Some(domain) = &self.current_domain
            && websites_with_rules.contains(domain)
            && let Some(scripts) = config.get_scripts_for_website(domain)
        {
            for script in scripts {
                info!("Injecting script: {} into {}", script, domain);

                let script_content = match scripts::read_script(&script) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("{}", e);
                        return Response::from_parts(parts, Body::from(html));
                    }
                };

                /* GM_ indicates Greasemonkey API usage requiring polyfill */
                if script_content.contains("GM_") {
                    let gm_polyfill = include_str!("../lib/gm_polyfill.js");
                    let inject = format!(
                        "<script>{}</script><script>{}</script>",
                        gm_polyfill, script_content
                    );
                    // Try </body> first, fallback to </html>, then append at end
                    if html.contains("</body>") {
                        html = html.replace("</body>", &format!("{}{}", inject, "</body>"));
                    } else if html.contains("</html>") {
                        html = html.replace("</html>", &format!("{}{}", inject, "</html>"));
                    } else {
                        warn!("Neither </body> nor </html> found, appending script at end");
                        html.push_str(&inject);
                    }

                    /* Greasemonkey APIs could need external connectivity, bypass CSP restrictions */
                    if let Some(csp) = parts.headers.get("content-security-policy") {
                        let csp_str = csp.to_str().unwrap_or("");
                        // Remove CSP if it contains nonces (e.g., Instagram) since we can't match them
                        if csp_str.contains("'nonce-") {
                            warn!("Removing CSP header with nonces to allow script injection");
                            parts.headers.remove("content-security-policy");
                        } else {
                            let new_csp = csp_str.replace("connect-src 'self'", "connect-src *");
                            match new_csp.parse() {
                                Ok(header_value) => {
                                    parts
                                        .headers
                                        .insert("content-security-policy", header_value);
                                }
                                Err(e) => {
                                    warn!("Failed to update CSP header: {}", e);
                                    // Remove the restrictive CSP rather than setting an invalid one
                                    parts.headers.remove("content-security-policy");
                                }
                            }
                        }
                    } else {
                        let inject = format!("<script>{}</script>", script_content);
                        // Try </body> first, fallback to </html>, then append at end
                        if html.contains("</body>") {
                            html = html.replace("</body>", &format!("{}{}", inject, "</body>"));
                        } else if html.contains("</html>") {
                            html = html.replace("</html>", &format!("{}{}", inject, "</html>"));
                        } else {
                            warn!("Neither </body> nor </html> found, appending script at end");
                            html.push_str(&inject);
                        }
                    }
                }
            }
        }

        /* Remove encoding/length headers since body changed from compressed to plain */
        parts.headers.remove(header::CONTENT_ENCODING);
        parts.headers.remove(header::CONTENT_LENGTH);

        Response::from_parts(parts, Body::from(html))
    }
}

/* -----------------------------------------------------------------------------
 * WebSocket Message Handling: No operations are done here.
 * -------------------------------------------------------------------------- */

impl WebSocketHandler for TrafficHandler {
    async fn handle_message(&mut self, _ctx: &WebSocketContext, msg: Message) -> Option<Message> {
        info!("WebSocket message: {:?}", msg);
        Some(msg)
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let ca_key_path = "ca/ca.key";
    let ca_cert_path = "ca/ca.cer";

    let key_pem = match std::fs::read_to_string(ca_key_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read {}: {}", ca_key_path, e);
            error!("Please generate the certificate files with:");
            error!("openssl req -x509 -newkey rsa:4096 -keyout ca/ca.key -out ca/ca.cer -days 365 -nodes");
            error!("Then add ca/ca.cer to your browser's trusted certificates.");
            return;
        }
    };

    let ca_cert_pem = match std::fs::read_to_string(ca_cert_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read {}: {}", ca_cert_path, e);
            error!("Please generate the certificate files with:");
            error!("openssl req -x509 -newkey rsa:4096 -keyout ca/ca.key -out ca/ca.cer -days 365 -nodes");
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

    /* -----------------------------------------------------------------------------
     * Proxy Configuration & Startup
     * -------------------------------------------------------------------------- */
    let traffic_handler = TrafficHandler {
        current_domain: None,
    };

    let config = config::load_config();
    let port = config.server.port;

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
