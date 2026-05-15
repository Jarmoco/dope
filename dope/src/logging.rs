/* -----------------------------------------------------------------------------
 * logging.rs
 * Structured JSON Lines logging for requests, responses, and proxy errors.
 * Each log entry is a single JSON object written to logs/dope-traces.jsonl.
 * Entries are correlated via a req_id (UUID v4) generated per request.
 * -------------------------------------------------------------------------- */

use hudsucker::hyper::{HeaderMap, Method, StatusCode, Uri, header};

/* --- Request Log ----------------------------------------------------------- */

pub fn log_request(
    req_id: &str,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    host: &str,
) {
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    let entry = dope_core::LogEntry::Request {
        req_id: req_id.to_string(),
        ts: dope_core::now_millis(),
        method: method.to_string(),
        uri: uri.to_string(),
        host: host.to_string(),
        user_agent: ua.to_string(),
        accept: accept.to_string(),
    };

    dope_core::append_entry(&entry);
}

/* --- Response Log ---------------------------------------------------------- */

pub fn log_response(
    req_id: &str,
    status: StatusCode,
    headers: &HeaderMap,
    body: &str,
) {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    let body_preview = if body.len() > 100 {
        let start: String = body.chars().take(40).collect();
        let end: String = body.chars().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect();
        format!("{} ... {}", start, end)
    } else {
        body.to_string()
    };
    let body_preview = body_preview.replace('\n', "\\n").replace('\r', "\\r");

    let entry = dope_core::LogEntry::Response {
        req_id: req_id.to_string(),
        ts: dope_core::now_millis(),
        status: status.as_u16(),
        content_type: content_type.to_string(),
        body_preview,
    };

    dope_core::append_entry(&entry);
}

/* --- Error Log ------------------------------------------------------------- */

pub fn log_proxy_error(
    req_id: &str,
    ctx: &hudsucker::HttpContext,
    err: &hudsucker::hyper_util::client::legacy::Error,
) {
    let entry = dope_core::LogEntry::Error {
        req_id: req_id.to_string(),
        ts: dope_core::now_millis(),
        client_addr: ctx.client_addr.to_string(),
        error: err.to_string(),
    };

    dope_core::append_entry(&entry);
}
