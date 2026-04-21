/* -----------------------------------------------------------------------------
 * Configuration Management
 * -----------------------------------------------------------------------------
 *
 * This module handles runtime configuration for userscript injection rules.
 * Configuration is stored in TOML format and includes server ports and
 * website-to-script mappings.
 *
 * Architecture:
 * - Config: Root structure combining server and userscript settings
 * - ServerConfig: Network port configuration for different proxy services
 * - UserscriptRule: Maps a website domain to a list of userscript files
 *
 * Design Choices:
 * - TOML format: Human-readable and easy to edit manually
 * - Default creation: Auto-generates config on first run to simplify setup
 * - Vector rules: Maintains insertion order for predictable script injection
 * - Clone-based retrieval: API returns owned values for simplicity
 */

use serde::{Deserialize, Serialize};
use tracing::*;

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct UserscriptRule {
    pub website: String,
    pub scripts: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct UserscriptsConfig {
    pub rule: Vec<UserscriptRule>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub userscripts: UserscriptsConfig,
}

impl Config {
    pub fn get_websites(&self) -> Vec<String> {
        self.userscripts
            .rule
            .iter()
            .map(|rule| rule.website.clone())
            .collect()
    }

    pub fn get_scripts_for_website(&self, website: &str) -> Option<Vec<String>> {
        for rule in &self.userscripts.rule {
            if rule.website == website {
                return Some(rule.scripts.clone());
            }
        }
        None
    }
}

/* -----------------------------------------------------------------------------
 * Default Configuration Creation
 * -------------------------------------------------------------------------- */
/* Why: Auto-generate config on first run to reduce setup friction */
pub fn create_default_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config {
        server: ServerConfig {
            port: 8080,
        },
        userscripts: UserscriptsConfig {
            rule: vec![
                UserscriptRule {
                    website: "www.google.com".to_string(),
                    scripts: vec!["example".to_string()],
                },
                UserscriptRule {
                    website: "www.youtube.com".to_string(),
                    scripts: vec!["example".to_string()],
                },
            ],
        },
    };

    let toml = toml::to_string(&config)?;
    std::fs::write("config.toml", toml)?;

    Ok(())
}

/* -----------------------------------------------------------------------------
 * Configuration Loading
 * -------------------------------------------------------------------------- */
/* Why: Create default config if missing, then parse and return */
pub fn load_config() -> Config {
    if !std::path::Path::new("config.toml").exists()
        && let Err(e) = create_default_config()
    {
        error!("Failed to create default config: {}", e);
        // Return a minimal config to allow the application to continue
        return Config {
            server: ServerConfig {
                port: 8080,
            },
            userscripts: UserscriptsConfig { rule: vec![] },
        };
    }

    let content = match std::fs::read_to_string("config.toml") {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read config.toml: {}", e);
            // Return a minimal config to allow the application to continue
            return Config {
                server: ServerConfig {
                    port: 8080,
                },
                userscripts: UserscriptsConfig { rule: vec![] },
            };
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse config.toml: {}", e);
            // Return a minimal config to allow the application to continue
            Config {
                server: ServerConfig {
                    port: 8080,
                },
                userscripts: UserscriptsConfig { rule: vec![] },
            }
        }
    }
}
