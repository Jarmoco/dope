/* -----------------------------------------------------------------------------
 * dope-core/src/lib.rs
 * Shared types, config I/O, and log I/O for dope proxy and dope-panel.
 * -------------------------------------------------------------------------- */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::*;

/* --- Log Types ------------------------------------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEntry {
    #[serde(rename = "request")]
    Request {
        req_id: String,
        ts: u64,
        method: String,
        uri: String,
        host: String,
        user_agent: String,
        accept: String,
    },
    #[serde(rename = "response")]
    Response {
        req_id: String,
        ts: u64,
        status: u16,
        content_type: String,
        body_preview: String,
    },
    #[serde(rename = "error")]
    Error {
        req_id: String,
        ts: u64,
        client_addr: String,
        error: String,
    },
}

/* --- Log Helpers ----------------------------------------------------------- */

impl LogEntry {
    pub fn ts(&self) -> u64 {
        match self {
            LogEntry::Request { ts, .. }
            | LogEntry::Response { ts, .. }
            | LogEntry::Error { ts, .. } => *ts,
        }
    }

    pub fn req_id(&self) -> &str {
        match self {
            LogEntry::Request { req_id, .. }
            | LogEntry::Response { req_id, .. }
            | LogEntry::Error { req_id, .. } => req_id,
        }
    }
}

pub fn read_log_entries(since: u64, limit: usize) -> Vec<LogEntry> {
    let path = trace_path();
    if !path.exists() {
        return vec![];
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let entries: Vec<LogEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(|e: &LogEntry| e.ts() >= since)
        .collect();

    let start = entries.len().saturating_sub(limit);
    entries[start..].to_vec()
}

pub fn log_entry_count() -> usize {
    let path = trace_path();
    if !path.exists() {
        return 0;
    }
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .count()
}

/* --- Config Types ---------------------------------------------------------- */

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub scripts: Option<Vec<ScriptRule>>,
    pub modify_response: Option<Vec<ResponseModifier>>,
    pub modify_request: Option<Vec<RequestModifier>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub pause: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ScriptRule {
    pub domain: String,
    pub scripts: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ResponseModifier {
    pub domain: String,
    pub csp: Option<String>,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<HashMap<String, String>>,
    pub inject_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct RequestModifier {
    pub domain: String,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<HashMap<String, String>>,
}

/* --- Config Lookups -------------------------------------------------------- */

impl Config {
    pub fn get_scripts_for_domain(&self, domain: &str) -> Vec<String> {
        let mut result = Vec::new();
        if let Some(scripts) = &self.scripts {
            for rule in scripts {
                if rule.domain == domain || rule.domain == "*" {
                    result.extend(rule.scripts.clone());
                }
            }
        }
        result
    }

    pub fn get_response_modifiers(&self, domain: &str) -> Option<&ResponseModifier> {
        let modifiers = self.modify_response.as_ref()?;
        modifiers.iter().find(|m| m.domain == domain)
            .or_else(|| modifiers.iter().find(|m| m.domain == "*"))
    }

    pub fn get_request_modifiers(&self, domain: &str) -> Option<&RequestModifier> {
        let modifiers = self.modify_request.as_ref()?;
        modifiers.iter().find(|m| m.domain == domain)
            .or_else(|| modifiers.iter().find(|m| m.domain == "*"))
    }
}

/* --- Paths ----------------------------------------------------------------- */

pub fn config_path() -> PathBuf {
    PathBuf::from("config.toml")
}

pub fn trace_path() -> PathBuf {
    PathBuf::from("logs/dope-traces.jsonl")
}

pub fn list_available_scripts() -> Vec<String> {
    let path = std::path::Path::new("scripts");
    if !path.is_dir() {
        return vec![];
    }
    let mut scripts: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stripped) = name.strip_suffix(".user.js") {
                scripts.push(stripped.to_string());
            }
        }
    }
    scripts.sort();
    scripts
}

/* --- Config I/O ------------------------------------------------------------ */

pub fn create_default_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = r#"# -----------------------------------------------------------------------------
# dope configuration
# -----------------------------------------------------------------------------
#
# Copy this file and adapt it to your needs.
# All fields shown below are optional — omit what you don't need.
#
# [[scripts]]        — userscripts to inject into matching domains
# [[modify_response]] — modify response headers (CSP, add/remove)
# [[modify_request]]  — modify outgoing request headers
#
# Scripts live in ./scripts/{name}.user.js and use the standard
# Greasemonkey / ViolentMonkey format.
# -----------------------------------------------------------------------------


[server]
port = 8080
# pause = true        # stop all injection and manipulation (default: false)


# -----------------------------------------------------------------------------
# SCRIPTS
# -----------------------------------------------------------------------------
# Each entry injects one or more userscripts into every HTML page served by
# the matching domain. Multiple [[scripts]] entries for the same domain are
# merged — all listed scripts will run.
#
# If a script uses GM_* APIs (GM_addStyle, GM_xmlhttpRequest, etc.), the
# GM polyfill is automatically injected alongside it.
#
# Example:
#   domain = "github.com"        — matches github.com and all subpages
#   domain = "*"                 — matches all domains
# -----------------------------------------------------------------------------

[[scripts]]
domain = "www.google.com"
scripts = ["example"]

# -----------------------------------------------------------------------------
# RESPONSE MODIFIERS
# -----------------------------------------------------------------------------
# Each entry modifies HTTP responses for a matching domain.
# Only the first matching [[modify_response]] entry is applied per domain.
#
# Fields:
#   domain          — domain to match (required, use "*" to match all domains)
#   csp             — Content-Security-Policy handling (optional)
#                     "remove_nonce"       — remove CSP only if it uses nonces
#                     "remove_all"         — always remove CSP entirely
#                     "relax_connect_src"  — replace connect-src 'self' with '*'
#                     "keep"              — leave CSP untouched
#   remove_headers  — list of response headers to strip (optional)
#   add_headers     — map of headers to inject into the response (optional)
#   inject_at       — where to inject <script> tags in the HTML (optional)
#                     "head_end"           — before </head>, fallback to <body>
#                     "body_end" (default) — before </body>, fallback to </html>
#                     "html_end"           — before </html>, fallback to append
#                     "append"             — at the very end of the document
# -----------------------------------------------------------------------------

[[modify_response]]
domain = "www.google.com"
csp = "remove_nonce"
remove_headers = ["x-frame-options", "strict-transport-security"]
add_headers = { "x-dope" = "injected" }
inject_at = "body_end"

# -----------------------------------------------------------------------------
# REQUEST MODIFIERS
# -----------------------------------------------------------------------------
# Each entry modifies outgoing HTTP requests for a matching domain.
# Only the first matching [[modify_request]] entry is applied per domain.
#
# Fields:
#   domain          — domain to match (required, use "*" to match all domains)
#   remove_headers  — list of request headers to strip (optional)
#   add_headers     — map of headers to inject into the request (optional)
# -----------------------------------------------------------------------------

[[modify_request]]
domain = "www.google.com"
add_headers = { "x-forwarded-proto" = "https" }
"#;

    std::fs::write(config_path(), config)?;
    Ok(())
}

pub fn load_config() -> Config {
    let path = config_path();
    if !path.exists() && let Err(e) = create_default_config() {
        error!("Failed to create default config: {}", e);
        return Config {
            server: ServerConfig { port: 8080, pause: None },
            scripts: None,
            modify_response: None,
            modify_request: None,
        };
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read {}: {}", path.display(), e);
            return Config {
                server: ServerConfig { port: 8080, pause: None },
                scripts: None,
                modify_response: None,
                modify_request: None,
            };
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse {}: {}", path.display(), e);
            Config {
                server: ServerConfig { port: 8080, pause: None },
                scripts: None,
                modify_response: None,
                modify_request: None,
            }
        }
    }
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(config_path(), toml_str)?;
    Ok(())
}

/* --- Log I/O --------------------------------------------------------------- */

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn ensure_log_directory() -> Result<(), std::io::Error> {
    let path = trace_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn append_entry(value: &LogEntry) {
    if let Err(e) = ensure_log_directory() {
        error!("Failed to create log directory: {}", e);
        return;
    }
    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(trace_path())
        .and_then(|mut file| {
            let line = serde_json::to_string(value).unwrap_or_default();
            use std::io::Write;
            writeln!(file, "{}", line)
        })
    {
        error!("Failed to write log file: {}", e);
    }
}
