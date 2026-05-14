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
    let config = Config {
        server: ServerConfig { port: 8080 },
        scripts: Some(vec![ScriptRule {
            domain: "www.google.com".to_string(),
            scripts: vec!["example".to_string()],
        }]),
        modify_response: None,
        modify_request: None,
    };

    let toml = toml::to_string(&config)?;
    std::fs::write("config.toml", toml)?;

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
