/* -----------------------------------------------------------------------------
 * handler.rs
 * Intercepts HTTP request/response pairs, decompresses HTML bodies, and
 * dispatches to script injectors and response/request modifiers.
 * -------------------------------------------------------------------------- */

use http_body_util::BodyDataStream;
use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use hudsucker::{
    Body, HttpContext,
    futures::TryStreamExt,
    hyper::{Request, Response, StatusCode, header},
    tokio_tungstenite::tungstenite::Message,
    HttpHandler, RequestOrResponse, WebSocketContext, WebSocketHandler,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tracing::*;

use crate::{config, inject, logging, modify};
/* --- Types ----------------------------------------------------------------- */

#[derive(Clone)]
pub struct TrafficHandler {
    pub pending_urls: Arc<Mutex<HashMap<SocketAddr, String>>>,
}

impl TrafficHandler {
    pub fn new() -> Self {
        TrafficHandler {
            pending_urls: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/* --- Helpers --------------------------------------------------------------- */

fn extract_domain(full_url: &str) -> String {
    full_url
        .strip_prefix("https://")
        .or_else(|| full_url.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(full_url)
        .to_string()
}

/* --- Request Handler ------------------------------------------------------- */

impl HttpHandler for TrafficHandler {
    async fn handle_request(
        &mut self,
        ctx: &HttpContext,
        mut req: Request<Body>,
    ) -> RequestOrResponse {
        let uri_str = req.uri().to_string();
        let host = req
            .uri()
            .host()
            .or_else(|| {
                req.headers()
                    .get(header::HOST)
                    .and_then(|v| v.to_str().ok())
            })
            .unwrap_or("unknown")
            .to_string();

        let full_uri = if req.uri().scheme().is_some() {
            uri_str
        } else {
            format!("https://{}{}", host, uri_str)
        };

        logging::log_request(req.method(), req.uri(), req.headers(), &host);

        self.pending_urls
            .lock()
            .unwrap()
            .insert(ctx.client_addr, full_uri);

        let cfg = config::load_config();
        if let Some(modifier_config) = cfg.get_request_modifiers(&host) {
            modify::apply_request_modifiers(req.headers_mut(), modifier_config);
        }

        req.into()
    }

    /* --- Response Handler -------------------------------------------------- */
    async fn handle_response(
        &mut self,
        ctx: &HttpContext,
        res: Response<Body>,
    ) -> Response<Body> {
        let is_html = res
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.to_str().unwrap_or("").contains("text/html"));

        if !is_html {
            logging::log_response(res.status(), res.headers(), "");
            return res;
        }

        let (mut parts, body) = res.into_parts();

        let stream = BodyDataStream::new(body).map_err(std::io::Error::other);
        let mut reader = StreamReader::new(stream);

        let mut decoded_bytes = Vec::new();

        let encoding = parts
            .headers
            .get(header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        match encoding {
            "gzip" => {
                let mut decoder = GzipDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Gzip decompression failed: {}", e);
                }
            }
            "br" => {
                let mut decoder = BrotliDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Brotli decompression failed: {}", e);
                }
            }
            "deflate" => {
                let mut decoder = ZlibDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Zlib decompression failed: {}", e);
                }
            }
            "zstd" => {
                let mut decoder = ZstdDecoder::new(reader);
                if let Err(e) = decoder.read_to_end(&mut decoded_bytes).await {
                    error!("Zstd decompression failed: {}", e);
                }
            }
            _ => {
                if !encoding.is_empty() {
                    warn!("Unsupported encoding: {}", encoding);
                }
                if let Err(e) = reader.read_to_end(&mut decoded_bytes).await {
                    error!("Body read failed: {}", e);
                }
            }
        }

        let mut html = String::from_utf8_lossy(&decoded_bytes).into_owned();

        /* --- Domain matching and modification ------------------------------ */

        let full_url = self
            .pending_urls
            .lock()
            .unwrap()
            .remove(&ctx.client_addr)
            .unwrap_or_default();
        let domain = extract_domain(&full_url);

        if !domain.is_empty() {
            let cfg = config::load_config();

            if let Some(response_modifier) = cfg.get_response_modifiers(&domain) {
                modify::apply_response_modifiers(&mut parts.headers, response_modifier);
            }

            inject::inject_scripts(&mut html, &domain, &cfg);
        }

        /* --- Finalize ------------------------------------------------------ */

        parts.headers.remove(header::CONTENT_ENCODING);
        parts.headers.remove(header::CONTENT_LENGTH);

        let response = Response::from_parts(parts, Body::from(html.clone()));

        logging::log_response(response.status(), response.headers(), &html);

        response
    }

    /* --- Error Handler ----------------------------------------------------- */

    async fn handle_error(
        &mut self,
        ctx: &HttpContext,
        err: hudsucker::hyper_util::client::legacy::Error,
    ) -> Response<Body> {
        error!("Proxy error: {}", err);
        logging::log_proxy_error(ctx, &err);
        Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::empty())
            .expect("Failed to build response")
    }
}

/* --- WebSocket Handler ----------------------------------------------------- */

impl WebSocketHandler for TrafficHandler {
    async fn handle_message(
        &mut self,
        _ctx: &WebSocketContext,
        msg: Message,
    ) -> Option<Message> {
        Some(msg)
    }
}
