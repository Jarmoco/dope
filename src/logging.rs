use hudsucker::hyper::{HeaderMap, Method, Uri, header};
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