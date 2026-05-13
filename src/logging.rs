use hudsucker::hyper::{HeaderMap, Method, StatusCode, Uri, header};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::*;

pub struct RequestLog {
    pub timestamp: u64,
    pub method: String,
    pub uri: String,
    pub host: String,
    pub user_agent: Option<String>,
    pub accept: Option<String>,
}

pub struct ResponseLog {
    pub timestamp: u64,
    pub status: u16,
    pub content_type: Option<String>,
    pub body_preview: String,
}

impl RequestLog {
    pub fn new(method: &Method, uri: &Uri, headers: &HeaderMap, host: &str) -> Self {
        let ua = headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let accept = headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            method: method.to_string(),
            uri: uri.to_string(),
            host: host.to_string(),
            user_agent: ua,
            accept,
        }
    }
}

fn get_log_path() -> PathBuf {
    PathBuf::from("logs/dope-requests.txt")
}

fn ensure_log_directory() -> Result<(), std::io::Error> {
    let log_path = get_log_path();
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn log_request(method: &Method, uri: &Uri, headers: &HeaderMap, host: &str) {
    let log = RequestLog::new(method, uri, headers, host);

    // Create plain text log entry: timestamp method uri host user-agent accept
    let user_agent = log.user_agent.as_deref().unwrap_or("-");
    let accept = log.accept.as_deref().unwrap_or("-");
    let entry = format!("{} {} {} {} {} {}\n", 
        log.timestamp, 
        log.method, 
        log.uri, 
        log.host,
        user_agent,
        accept
    );

    let log_path = get_log_path();

    // Ensure log directory exists
    if let Err(e) = ensure_log_directory() {
        error!("Failed to create log directory: {}", e);
        return;
    }

    // Append to log file
    if let Err(e) = fs::OpenOptions::new().create(true).append(true).open(&log_path).and_then(|mut file| {
        use std::io::Write;
        file.write_all(entry.as_bytes())
    }) {
        error!("Failed to write log file: {}", e);
    }
}

fn get_response_log_path() -> PathBuf {
    PathBuf::from("logs/dope-responses.txt")
}

impl ResponseLog {
    pub fn new(status: StatusCode, headers: &HeaderMap, body: &str) -> Self {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        let body_preview = if body.len() > 100 {
            let start = &body[..40];
            let end = &body[body.len() - 40..];
            format!("{} ... {}", start, end)
        } else {
            body.to_string()
        };

        let body_preview = body_preview.replace('\n', "\\n").replace('\r', "\\r");

        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            status: status.as_u16(),
            content_type,
            body_preview,
        }
    }
}

pub fn log_response(status: StatusCode, headers: &HeaderMap, body: &str) {
    let log = ResponseLog::new(status, headers, body);

    let content_type = log.content_type.as_deref().unwrap_or("-");
    let entry = format!("{} {} {} {}\n",
        log.timestamp,
        log.status,
        content_type,
        log.body_preview
    );

    let log_path = get_response_log_path();

    if let Err(e) = ensure_log_directory_for(&log_path) {
        error!("Failed to create log directory: {}", e);
        return;
    }

    if let Err(e) = fs::OpenOptions::new().create(true).append(true).open(&log_path).and_then(|mut file| {
        use std::io::Write;
        file.write_all(entry.as_bytes())
    }) {
        error!("Failed to write response log file: {}", e);
    }
}

fn ensure_log_directory_for(path: &PathBuf) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn log_proxy_error(ctx: &hudsucker::HttpContext, err: &hudsucker::hyper_util::client::legacy::Error) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let entry = format!("{} {} ERROR {}\n",
        timestamp,
        ctx.client_addr,
        err
    );

    let log_path = get_response_log_path();

    if let Err(e) = ensure_log_directory_for(&log_path) {
        error!("Failed to create log directory: {}", e);
        return;
    }

    if let Err(e) = fs::OpenOptions::new().create(true).append(true).open(&log_path).and_then(|mut file| {
        use std::io::Write;
        file.write_all(entry.as_bytes())
    }) {
        error!("Failed to write error log file: {}", e);
    }
}