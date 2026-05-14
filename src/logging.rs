/* -----------------------------------------------------------------------------
 * logging.rs
 * Structured JSON Lines logging for requests, responses, and proxy errors.
 * Each log entry is a single JSON object written to logs/dope-traces.jsonl.
 * Entries are correlated via a req_id (UUID v4) generated per request.
 * -------------------------------------------------------------------------- */

use hudsucker::hyper::{HeaderMap, Method, StatusCode, Uri, header};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::*;

fn get_trace_path() -> PathBuf {
    PathBuf::from("logs/dope-traces.jsonl")
}

fn ensure_log_directory() -> Result<(), std::io::Error> {
    let path = get_trace_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn append_entry(value: &serde_json::Value) {
    let path = get_trace_path();
    if let Err(e) = ensure_log_directory() {
        error!("Failed to create log directory: {}", e);
        return;
    }
    if let Err(e) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| {
            let line = serde_json::to_string(value).unwrap_or_default();
            writeln!(file, "{}", line)
        })
    {
        error!("Failed to write log file: {}", e);
    }
}

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

    let entry = serde_json::json!({
        "type": "request",
        "req_id": req_id,
        "ts": now_millis(),
        "method": method.to_string(),
        "uri": uri.to_string(),
        "host": host,
        "user_agent": ua,
        "accept": accept,
    });

    append_entry(&entry);
}

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
        let start = &body[..40];
        let end = &body[body.len() - 40..];
        format!("{} ... {}", start, end)
    } else {
        body.to_string()
    };
    let body_preview = body_preview.replace('\n', "\\n").replace('\r', "\\r");

    let entry = serde_json::json!({
        "type": "response",
        "req_id": req_id,
        "ts": now_millis(),
        "status": status.as_u16(),
        "content_type": content_type,
        "body_preview": body_preview,
    });

    append_entry(&entry);
}

pub fn log_proxy_error(
    req_id: &str,
    ctx: &hudsucker::HttpContext,
    err: &hudsucker::hyper_util::client::legacy::Error,
) {
    let entry = serde_json::json!({
        "type": "error",
        "req_id": req_id,
        "ts": now_millis(),
        "client_addr": ctx.client_addr.to_string(),
        "error": err.to_string(),
    });

    append_entry(&entry);
}
