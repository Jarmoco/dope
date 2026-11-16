/* -----------------------------------------------------------------------------
 * dope-core/src/lib.rs
 * Shared types, config I/O, and log I/O for dope proxy and dope-panel.
 * -------------------------------------------------------------------------- */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
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
    pub log: Option<LogConfig>,
    pub scripts: Option<Vec<ScriptRule>>,
    pub modify_response: Option<Vec<ResponseModifier>>,
    pub modify_request: Option<Vec<RequestModifier>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub pause: Option<bool>,

    /// Which HTTPS connections to intercept: "configured" (default), "all", or "none".
    #[serde(default)]
    pub intercept: Option<String>,

    /// Hosts that are never intercepted. A bare domain also covers its subdomains.
    #[serde(default)]
    pub passthrough: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub enabled: Option<bool>,
    pub max_size: Option<u64>,
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

/* --- Defaults -------------------------------------------------------------- */

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            pause: None,
            intercept: None,
            passthrough: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            log: None,
            scripts: None,
            modify_response: None,
            modify_request: None,
        }
    }
}

/* --- Host Matching --------------------------------------------------------- */

fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Rule matching: exact host, `*.example.com` for subdomains, `*` for everything.
pub fn host_matches_rule(host: &str, pattern: &str) -> bool {
    let host = normalize_host(host);
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();

    if pattern == "*" {
        return true;
    }

    if let Some(suffix) = pattern.strip_prefix("*.") {
        return !suffix.is_empty() && host.ends_with(&format!(".{suffix}"));
    }

    !pattern.is_empty() && host == pattern
}

/// Passthrough matching: apex plus all subdomains.
pub fn host_matches_suffix(host: &str, pattern: &str) -> bool {
    let host = normalize_host(host);
    let pattern = pattern
        .trim()
        .trim_start_matches("*.")
        .trim_end_matches('.')
        .to_ascii_lowercase();

    if pattern == "*" {
        return true;
    }

    !pattern.is_empty() && (host == pattern || host.ends_with(&format!(".{pattern}")))
}

fn response_modifier_effective(m: &ResponseModifier) -> bool {
    let csp = m.csp.as_deref().is_some_and(|c| {
        let c = c.trim();
        !c.is_empty() && !c.eq_ignore_ascii_case("keep")
    });
    let remove = m
        .remove_headers
        .as_ref()
        .is_some_and(|hs| hs.iter().any(|h| !h.trim().is_empty()));
    let add = m.add_headers.as_ref().is_some_and(|hm| {
        hm.iter()
            .any(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
    });

    csp || remove || add
}

fn request_modifier_effective(m: &RequestModifier) -> bool {
    let remove = m
        .remove_headers
        .as_ref()
        .is_some_and(|hs| hs.iter().any(|h| !h.trim().is_empty()));
    let add = m.add_headers.as_ref().is_some_and(|hm| {
        hm.iter()
            .any(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
    });

    remove || add
}

/* --- Config Lookups -------------------------------------------------------- */

impl Config {
    pub fn log_enabled(&self) -> bool {
        self.log.as_ref().and_then(|l| l.enabled).unwrap_or(true)
    }

    pub fn log_max_size(&self) -> u64 {
        self.log
            .as_ref()
            .and_then(|l| l.max_size)
            .unwrap_or(10 * 1024 * 1024)
    }

    /// Interception policy: "configured" (default), "all", or "none".
    pub fn intercept_mode(&self) -> &str {
        match self.server.intercept.as_deref() {
            Some("all") => "all",
            Some("none") => "none",
            _ => "configured",
        }
    }

    pub fn is_passthrough(&self, host: &str) -> bool {
        self.server
            .passthrough
            .as_deref()
            .is_some_and(|list| list.iter().any(|p| host_matches_suffix(host, p)))
    }

    /// Whether a host has at least one rule that actually does something.
    pub fn has_rules_for(&self, host: &str) -> bool {
        self.has_scripts_for(host)
            || self.modify_response.as_deref().is_some_and(|rules| {
                rules
                    .iter()
                    .any(|m| host_matches_rule(host, &m.domain) && response_modifier_effective(m))
            })
            || self.modify_request.as_deref().is_some_and(|rules| {
                rules
                    .iter()
                    .any(|m| host_matches_rule(host, &m.domain) && request_modifier_effective(m))
            })
    }

    /// Whether a CONNECT to `host` should be MITM'd.
    pub fn should_intercept(&self, host: &str) -> bool {
        if self.is_passthrough(host) {
            return false;
        }

        match self.intercept_mode() {
            "all" => true,
            "none" => false,
            _ => self.has_rules_for(host),
        }
    }

    pub fn has_scripts_for(&self, domain: &str) -> bool {
        self.scripts.as_deref().is_some_and(|rules| {
            rules.iter().any(|r| {
                host_matches_rule(domain, &r.domain)
                    && r.scripts.iter().any(|s| !s.trim().is_empty())
            })
        })
    }

    pub fn get_scripts_for_domain(&self, domain: &str) -> Vec<String> {
        let mut result = Vec::new();
        if let Some(scripts) = &self.scripts {
            for rule in scripts {
                if host_matches_rule(domain, &rule.domain) {
                    result.extend(
                        rule.scripts
                            .iter()
                            .filter(|s| !s.trim().is_empty())
                            .cloned(),
                    );
                }
            }
        }
        result
    }

    pub fn get_response_modifiers(&self, domain: &str) -> Option<&ResponseModifier> {
        self.modify_response
            .as_deref()?
            .iter()
            .find(|m| host_matches_rule(domain, &m.domain) && response_modifier_effective(m))
    }

    /// Injection position for a domain, independent of header modifiers.
    pub fn get_inject_at(&self, domain: &str) -> Option<&str> {
        self.modify_response
            .as_deref()?
            .iter()
            .find(|m| {
                host_matches_rule(domain, &m.domain)
                    && m.inject_at.as_deref().is_some_and(|p| !p.trim().is_empty())
            })
            .and_then(|m| m.inject_at.as_deref())
    }

    pub fn get_request_modifiers(&self, domain: &str) -> Option<&RequestModifier> {
        self.modify_request
            .as_deref()?
            .iter()
            .find(|m| host_matches_rule(domain, &m.domain) && request_modifier_effective(m))
    }
}

/* --- Directory Configuration ----------------------------------------------- */

#[derive(Clone)]
pub struct Dirs {
    pub config: PathBuf,
    pub scripts: PathBuf,
    pub logs: PathBuf,
}

impl Default for Dirs {
    fn default() -> Self {
        Self {
            config: PathBuf::from("config.toml"),
            scripts: PathBuf::from("scripts"),
            logs: PathBuf::from("logs"),
        }
    }
}

static DIRS: OnceLock<Dirs> = OnceLock::new();

pub fn init_dirs(dirs: Dirs) {
    DIRS.set(dirs).ok();
}

pub fn dirs() -> Dirs {
    DIRS.get().cloned().unwrap_or_default()
}

/* --- Paths ----------------------------------------------------------------- */

pub fn config_path() -> PathBuf {
    dirs().config
}

pub fn trace_path() -> PathBuf {
    dirs().logs.join("dope-traces.jsonl")
}

pub fn list_available_scripts() -> Vec<String> {
    let path = dirs().scripts;
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
# pause = true        # skip injection and header changes (default: false)
#
# Which HTTPS connections dope should terminate:
#   "configured" (default) — only hosts with at least one active rule below
#   "all"                  — every HTTPS host except passthrough (old behavior)
#   "none"                 — tunnel everything, ignore all rules
intercept = "configured"
#
# Hosts that are never intercepted. A bare domain also covers its subdomains.
# passthrough = ["amazon.it"]

# [log]
# enabled = true      # set to false to disable request/response logging
# max_size = 10485760 # rotate log when it exceeds this many bytes (10 MB)


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
#   domain = "github.com"        — matches only github.com
#   domain = "*.github.com"      — matches subdomains (not github.com itself)
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
#   domain          — domain to match (required; "*.example.com" and "*" work)
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
#   domain          — domain to match (required; "*.example.com" and "*" work)
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
    if !path.exists()
        && let Err(e) = create_default_config()
    {
        error!("Failed to create default config: {}", e);
        return Config::default();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read {}: {}", path.display(), e);
            return Config::default();
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse {}: {}", path.display(), e);
            Config::default()
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
    let cfg = load_config();
    if !cfg.log_enabled() {
        return;
    }

    if let Err(e) = ensure_log_directory() {
        error!("Failed to create log directory: {}", e);
        return;
    }

    let path = trace_path();
    // ponytail: check file size every write — simple enough, add periodic check if perf matters
    let max_size = cfg.log_max_size();
    if path.exists() {
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > max_size {
                if let Err(e) = std::fs::write(&path, "") {
                    error!("Failed to truncate log file: {}", e);
                }
            }
        }
    }

    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| {
            let line = serde_json::to_string(value).unwrap_or_default();
            use std::io::Write;
            writeln!(file, "{}", line)
        })
    {
        error!("Failed to write log file: {}", e);
    }
}

/* --- Tests ----------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_script(domain: &str, scripts: &[&str]) -> Config {
        let mut config = Config::default();
        config.scripts = Some(vec![ScriptRule {
            domain: domain.to_string(),
            scripts: scripts.iter().map(|s| s.to_string()).collect(),
        }]);
        config
    }

    #[test]
    fn rule_matching_is_exact_by_default() {
        assert!(host_matches_rule("example.com", "example.com"));
        assert!(!host_matches_rule("www.example.com", "example.com"));
        assert!(host_matches_rule("EXAMPLE.com", "example.COM"));
    }

    #[test]
    fn rule_matching_supports_subdomain_wildcards() {
        assert!(host_matches_rule("www.example.com", "*.example.com"));
        assert!(host_matches_rule("a.b.example.com", "*.example.com"));
        assert!(!host_matches_rule("example.com", "*.example.com"));
        assert!(!host_matches_rule("notexample.com", "*.example.com"));
    }

    #[test]
    fn rule_matching_supports_global_wildcard() {
        assert!(host_matches_rule("example.com", "*"));
        assert!(host_matches_rule("anything.else", "*"));
    }

    #[test]
    fn passthrough_matching_covers_apex_and_subdomains() {
        assert!(host_matches_suffix("amazon.it", "amazon.it"));
        assert!(host_matches_suffix("www.amazon.it", "amazon.it"));
        assert!(host_matches_suffix("fls-eu.amazon.it", "amazon.it"));
        assert!(host_matches_suffix("www.amazon.it", "*.amazon.it"));
        assert!(!host_matches_suffix("notamazon.it", "amazon.it"));
    }

    #[test]
    fn configured_mode_only_intercepts_hosts_with_effective_rules() {
        let config = config_with_script("www.example.com", &["example"]);

        assert!(config.should_intercept("www.example.com"));
        assert!(!config.should_intercept("example.com"));
        assert!(!config.should_intercept("amazon.it"));
    }

    #[test]
    fn empty_placeholder_rules_do_not_count_as_configured() {
        let mut config = Config::default();
        config.scripts = Some(vec![ScriptRule {
            domain: "example.com".to_string(),
            scripts: vec![String::new()],
        }]);
        config.modify_response = Some(vec![ResponseModifier {
            domain: "example.com".to_string(),
            remove_headers: Some(vec![String::new()]),
            ..Default::default()
        }]);

        assert!(!config.has_rules_for("example.com"));
        assert!(!config.should_intercept("example.com"));
    }

    #[test]
    fn all_mode_intercepts_unconfigured_hosts_unless_passthrough() {
        let mut config = Config::default();
        config.server.intercept = Some("all".to_string());

        assert!(config.should_intercept("amazon.it"));

        config.server.passthrough = Some(vec!["amazon.it".to_string()]);
        assert!(!config.should_intercept("www.amazon.it"));
    }

    #[test]
    fn none_mode_never_intercepts() {
        let mut config = config_with_script("www.example.com", &["example"]);
        config.server.intercept = Some("none".to_string());

        assert!(!config.should_intercept("www.example.com"));
    }

    #[test]
    fn scripts_merge_and_skip_empty_entries() {
        let mut config = config_with_script("*.example.com", &["a", ""]);
        config.scripts.as_mut().unwrap().push(ScriptRule {
            domain: "*".to_string(),
            scripts: vec!["b".to_string()],
        });

        let scripts = config.get_scripts_for_domain("www.example.com");
        assert_eq!(scripts, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn modifier_lookup_skips_empty_rules() {
        let mut config = Config::default();
        config.modify_response = Some(vec![
            ResponseModifier {
                domain: "example.com".to_string(),
                ..Default::default()
            },
            ResponseModifier {
                domain: "example.com".to_string(),
                csp: Some("remove_all".to_string()),
                ..Default::default()
            },
        ]);

        let found = config.get_response_modifiers("example.com").unwrap();
        assert_eq!(found.csp.as_deref(), Some("remove_all"));
    }
}
