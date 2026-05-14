/* -----------------------------------------------------------------------------
 * config.rs
 * Configuration types and TOML loading for domains, scripts, and modifiers.
 * -------------------------------------------------------------------------- */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::*;

/* --- Types ----------------------------------------------------------------- */

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub scripts: Option<Vec<ScriptRule>>,
    pub modify_response: Option<Vec<ResponseModifier>>,
    pub modify_request: Option<Vec<RequestModifier>>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct ScriptRule {
    pub domain: String,
    pub scripts: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ResponseModifier {
    pub domain: String,
    pub csp: Option<String>,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<HashMap<String, String>>,
    pub inject_at: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct RequestModifier {
    pub domain: String,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<HashMap<String, String>>,
}

/* --- Lookups --------------------------------------------------------------- */

impl Config {
    pub fn get_scripts_for_domain(&self, domain: &str) -> Vec<String> {
        let mut result = Vec::new();
        if let Some(scripts) = &self.scripts {
            for rule in scripts {
                if rule.domain == domain {
                    result.extend(rule.scripts.clone());
                }
            }
        }
        result
    }

    pub fn get_response_modifiers(&self, domain: &str) -> Option<&ResponseModifier> {
        self.modify_response
            .as_ref()?
            .iter()
            .find(|m| m.domain == domain)
    }

    pub fn get_request_modifiers(&self, domain: &str) -> Option<&RequestModifier> {
        self.modify_request
            .as_ref()?
            .iter()
            .find(|m| m.domain == domain)
    }
}

/* --- Default Config -------------------------------------------------------- */

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
# -----------------------------------------------------------------------------

[[scripts]]
domain = "github.com"
scripts = ["my-github-script"]

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
#   domain          — domain to match (required)
#   csp             — Content-Security-Policy handling (optional)
#                     "remove_nonce"       — remove CSP only if it uses nonces
#                     "remove_all"         — always remove CSP entirely
#                     "relax_connect_src"  — replace connect-src 'self' with '*'
#                     "keep"              — leave CSP untouched
#   remove_headers  — list of response headers to strip (optional)
#   add_headers     — map of headers to inject into the response (optional)
#   inject_at       — where to inject <script> tags in the HTML (optional)
#                     "body_end" (default) — before </body>, fallback to </html>
#                     "html_end"           — before </html>, fallback to append
#                     "append"             — at the very end of the document
# -----------------------------------------------------------------------------

[[modify_response]]
domain = "github.com"
csp = "remove_nonce"
remove_headers = ["x-frame-options", "strict-transport-security"]
add_headers = { "x-dope" = "injected" }
inject_at = "body_end"

[[modify_response]]
domain = "www.google.com"
csp = "remove_nonce"


# -----------------------------------------------------------------------------
# REQUEST MODIFIERS
# -----------------------------------------------------------------------------
# Each entry modifies outgoing HTTP requests for a matching domain.
# Only the first matching [[modify_request]] entry is applied per domain.
#
# Fields:
#   domain          — domain to match (required)
#   remove_headers  — list of request headers to strip (optional)
#   add_headers     — map of headers to inject into the request (optional)
# -----------------------------------------------------------------------------

[[modify_request]]
domain = "github.com"
add_headers = { "x-forwarded-proto" = "https" }
"#;

    std::fs::write("config.toml", config)?;

    Ok(())
}

/* --- Load ------------------------------------------------------------------ */

pub fn load_config() -> Config {
    if !std::path::Path::new("config.toml").exists()
        && let Err(e) = create_default_config()
    {
        error!("Failed to create default config: {}", e);
        return Config {
            server: ServerConfig { port: 8080 },
            scripts: None,
            modify_response: None,
            modify_request: None,
        };
    }

    let content = match std::fs::read_to_string("config.toml") {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read config.toml: {}", e);
            return Config {
                server: ServerConfig { port: 8080 },
                scripts: None,
                modify_response: None,
                modify_request: None,
            };
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse config.toml: {}", e);
            Config {
                server: ServerConfig { port: 8080 },
                scripts: None,
                modify_response: None,
                modify_request: None,
            }
        }
    }
}
