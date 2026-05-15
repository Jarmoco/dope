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
    hyper::{Method, Request, Response, StatusCode, header},
    tokio_tungstenite::tungstenite::Message,
    HttpHandler, RequestOrResponse, WebSocketContext, WebSocketHandler,
};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tracing::*;

use crate::{charset, inject, logging, modify};

/* --- Types ----------------------------------------------------------------- */

struct PendingRequest {
    url: String,
    req_id: String,
}

#[derive(Clone)]
pub struct TrafficHandler {
    pending_urls: Arc<Mutex<HashMap<SocketAddr, VecDeque<PendingRequest>>>>,
}

impl TrafficHandler {
    pub fn new() -> Self {
        TrafficHandler {
            pending_urls: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/* --- Helpers --------------------------------------------------------------- */

fn is_websocket_upgrade(req: &Request<Body>) -> bool {
    req.headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

fn extract_domain(full_url: &str) -> String {
    full_url
        .strip_prefix("https://")
        .or_else(|| full_url.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(full_url)
        .split(':')
        .next()
        .unwrap_or("")
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

        let req_id = uuid::Uuid::new_v4().to_string();
        logging::log_request(&req_id, req.method(), req.uri(), req.headers(), &host);

        if req.method() == Method::CONNECT {
            return req.into();
        }

        self.pending_urls
            .lock()
            .unwrap()
            .entry(ctx.client_addr)
            .or_default()
            .push_back(PendingRequest { url: full_uri, req_id });

        if is_websocket_upgrade(&req) {
            req.headers_mut().remove("sec-websocket-extensions");
            req.headers_mut().remove("sec-websocket-protocol");
        }

        let cfg = dope_core::load_config();
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
        let (req_id, full_url) = self
            .pending_urls
            .lock()
            .unwrap()
            .get_mut(&ctx.client_addr)
            .and_then(|queue| queue.pop_front())
            .map_or(("-".to_string(), String::new()), |p| (p.req_id, p.url));

        let domain = extract_domain(&full_url);

        let is_html = res
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.to_str().unwrap_or("").contains("text/html"));

        if !is_html {
            logging::log_response(&req_id, res.status(), res.headers(), "");
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

        let content_type = parts
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());
        let mut html = charset::decode_body(&decoded_bytes, content_type);

        /* --- Domain matching and modification ------------------------------ */

        info!("Response for URL: {} → domain: {}", full_url, domain);

        if !domain.is_empty() {
            let cfg = dope_core::load_config();

            if cfg.server.pause.unwrap_or(false) {
                info!("Paused — skipping injection and manipulation for {}", domain);
            } else {
                if let Some(response_modifier) = cfg.get_response_modifiers(&domain) {
                    modify::apply_response_modifiers(&mut parts.headers, response_modifier);
                }

                inject::inject_scripts(&mut html, &domain, &cfg);
            }
        }

        /* --- Finalize ------------------------------------------------------ */

        parts.headers.remove(header::CONTENT_ENCODING);
        parts.headers.remove(header::CONTENT_LENGTH);

        if let Some(ct) = parts.headers.get_mut(header::CONTENT_TYPE) {
            if let Ok(val) = ct.to_str() {
                let base = val.split(';').next().unwrap_or(val).trim();
                *ct = header::HeaderValue::from_str(&format!("{}; charset=utf-8", base))
                    .expect("Content-Type base is valid ASCII");
            }
        }

        let response = Response::from_parts(parts, Body::from(html.clone()));

        logging::log_response(&req_id, response.status(), response.headers(), &html);

        response
    }

    /* --- Error Handler ----------------------------------------------------- */

    async fn handle_error(
        &mut self,
        ctx: &HttpContext,
        err: hudsucker::hyper_util::client::legacy::Error,
    ) -> Response<Body> {
        error!("Proxy error: {}", err);

        let req_id = self
            .pending_urls
            .lock()
            .unwrap()
            .get_mut(&ctx.client_addr)
            .and_then(|queue| queue.pop_front())
            .map_or_else(|| uuid::Uuid::new_v4().to_string(), |p| p.req_id);

        logging::log_proxy_error(&req_id, ctx, &err);

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
