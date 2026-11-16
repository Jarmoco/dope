use std::collections::HashMap;

use axum::Form;
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use dope_core;

use crate::html;

async fn load_config() -> dope_core::Config {
    tokio::task::spawn_blocking(dope_core::load_config)
        .await
        .unwrap_or_else(|_| dope_core::Config::default())
}

async fn save_config(config: &dope_core::Config) -> bool {
    let cfg = config.clone();
    tokio::task::spawn_blocking(move || dope_core::save_config(&cfg))
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some()
}

fn ok_html_with_toast(body: String, msg: &str) -> Response {
    let json = format!(r##"{{"showToast":"{}"}}"##, msg);
    let trigger = axum::http::HeaderName::from_static("hx-trigger");
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (trigger, json.as_str()),
        ],
        Html(body),
    )
        .into_response()
}

/* --- Server Config --------------------------------------------------------- */

pub async fn update_server_port(Form(body): Form<HashMap<String, String>>) -> impl IntoResponse {
    let mut config = load_config().await;
    if let Some(port_str) = body.get("value").or_else(|| body.get("cfg-port")) {
        if let Ok(port) = port_str.parse::<u16>() {
            config.server.port = port;
        }
    }
    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Port updated")
}

pub async fn update_server_pause() -> impl IntoResponse {
    let mut config = load_config().await;
    let paused = !config.server.pause.unwrap_or(false);
    config.server.pause = Some(paused);
    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Pause toggled")
}

pub async fn update_server_intercept(
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mode = match body.get("value").map(String::as_str) {
        Some("all") => "all",
        Some("none") => "none",
        _ => "configured",
    };
    config.server.intercept = Some(mode.to_string());
    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Intercept mode updated")
}

pub async fn update_server_passthrough(
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mut hosts: Vec<String> = body
        .get("value")
        .map(|value| {
            value
                .lines()
                .flat_map(|line| line.split(','))
                .map(|host| host.trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect()
        })
        .unwrap_or_default();

    hosts.sort();
    hosts.dedup();
    config.server.passthrough = if hosts.is_empty() { None } else { Some(hosts) };

    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Passthrough hosts updated")
}

/* --- Log Config ------------------------------------------------------------ */

pub async fn update_log_enabled() -> impl IntoResponse {
    let mut config = load_config().await;
    let enabled = !config.log_enabled();
    config
        .log
        .get_or_insert(dope_core::LogConfig {
            enabled: None,
            max_size: None,
        })
        .enabled = Some(enabled);
    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Logging toggled")
}

pub async fn update_log_max_size(Form(body): Form<HashMap<String, String>>) -> impl IntoResponse {
    let mut config = load_config().await;
    if let Some(size_str) = body.get("value") {
        if let Ok(size) = size_str.parse::<u64>() {
            config
                .log
                .get_or_insert(dope_core::LogConfig {
                    enabled: None,
                    max_size: None,
                })
                .max_size = Some(size);
        }
    }
    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Max log size updated")
}

/* --- Domain CRUD ----------------------------------------------------------- */

pub async fn add_domain(Form(body): Form<HashMap<String, String>>) -> impl IntoResponse {
    let mut config = load_config().await;
    let domain = body
        .get("domain")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match domain {
        Some(domain) => {
            let exists = config
                .scripts
                .as_ref()
                .map_or(false, |r| r.iter().any(|r| r.domain == domain));
            if !exists {
                config
                    .scripts
                    .get_or_insert_with(Vec::new)
                    .push(dope_core::ScriptRule {
                        domain: domain.clone(),
                        ..Default::default()
                    });
            }
            save_config(&config).await;
            ok_html_with_toast(
                html::config_page(&config),
                &format!("Domain '{}' added", domain),
            )
        }
        None => ok_html_with_toast(html::config_page(&config), "Invalid domain name"),
    }
}

pub async fn remove_domain(Path(domain): Path<String>) -> impl IntoResponse {
    let mut config = load_config().await;
    if let Some(ref mut rules) = config.scripts {
        rules.retain(|r| r.domain != domain);
    }
    if let Some(ref mut rules) = config.modify_response {
        rules.retain(|r| r.domain != domain);
    }
    if let Some(ref mut rules) = config.modify_request {
        rules.retain(|r| r.domain != domain);
    }
    save_config(&config).await;
    ok_html_with_toast(
        html::config_page(&config),
        &format!("Domain '{}' removed", domain),
    )
}

/* --- Domain Scripts -------------------------------------------------------- */

pub async fn add_domain_script(Path(domain): Path<String>) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .scripts
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    match idx {
        Some(idx) => {
            if let Some(rule) = config.scripts.as_mut().and_then(|rules| rules.get_mut(idx)) {
                rule.scripts.push(String::new());
            }
        }
        None => {
            config
                .scripts
                .get_or_insert_with(Vec::new)
                .push(dope_core::ScriptRule {
                    domain: domain.clone(),
                    scripts: vec![String::new()],
                });
        }
    }

    save_config(&config).await;
    ok_html_with_toast(html::domain_card(&config, &domain, true), "Script added")
}

pub async fn update_domain_script(
    Path((domain, idx)): Path<(String, usize)>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let rule_idx = config
        .scripts
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(rule_idx) = rule_idx {
        if let Some(rule) = config
            .scripts
            .as_mut()
            .and_then(|rules| rules.get_mut(rule_idx))
        {
            if idx < rule.scripts.len() {
                if let Some(name) = body.get("name") {
                    rule.scripts[idx] = name.clone();
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(html::domain_card(&config, &domain, true), "Script updated")
}

pub async fn remove_domain_script(Path((domain, idx)): Path<(String, usize)>) -> impl IntoResponse {
    let mut config = load_config().await;
    let rule_idx = config
        .scripts
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(rule_idx) = rule_idx {
        if let Some(rule) = config
            .scripts
            .as_mut()
            .and_then(|rules| rules.get_mut(rule_idx))
        {
            if idx < rule.scripts.len() {
                rule.scripts.remove(idx);
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(html::domain_card(&config, &domain, true), "Script removed")
}

/* --- Domain Response Modifiers --------------------------------------------- */

pub async fn update_domain_response(
    Path(domain): Path<String>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;

    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    match idx {
        Some(idx) => {
            if let Some(rule) = config
                .modify_response
                .as_mut()
                .and_then(|rules| rules.get_mut(idx))
            {
                for (field, value) in &body {
                    match field.as_str() {
                        "csp" => {
                            rule.csp = if value.is_empty() {
                                None
                            } else {
                                Some(value.clone())
                            }
                        }
                        "inject_at" => {
                            rule.inject_at = if value.is_empty() {
                                None
                            } else {
                                Some(value.clone())
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None => {
            let mut csp = None;
            let mut inject_at = None;
            for (field, value) in &body {
                match field.as_str() {
                    "csp" => {
                        csp = if value.is_empty() {
                            None
                        } else {
                            Some(value.clone())
                        }
                    }
                    "inject_at" => {
                        inject_at = if value.is_empty() {
                            None
                        } else {
                            Some(value.clone())
                        }
                    }
                    _ => {}
                }
            }
            config
                .modify_response
                .get_or_insert_with(Vec::new)
                .push(dope_core::ResponseModifier {
                    domain: domain.clone(),
                    csp,
                    inject_at,
                    ..Default::default()
                });
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Response modifier updated",
    )
}

/* --- Domain Response Headers ----------------------------------------------- */

pub async fn add_domain_response_header(Path(domain): Path<String>) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    match idx {
        Some(idx) => {
            if let Some(rule) = config
                .modify_response
                .as_mut()
                .and_then(|rules| rules.get_mut(idx))
            {
                let headers = rule.add_headers.get_or_insert_with(HashMap::new);
                let n = headers.len();
                headers.insert(format!("x-new-header-{n}"), String::new());
            }
        }
        None => {
            let mut headers = HashMap::new();
            headers.insert("x-new-header-0".to_string(), String::new());
            config
                .modify_response
                .get_or_insert_with(Vec::new)
                .push(dope_core::ResponseModifier {
                    domain: domain.clone(),
                    add_headers: Some(headers),
                    ..Default::default()
                });
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Response header added",
    )
}

pub async fn remove_domain_response_header(
    Path((domain, key)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_response
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.add_headers {
                headers.remove(&key);
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Response header removed",
    )
}

pub async fn update_domain_response_header_key(
    Path(domain): Path<String>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_response
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.add_headers {
                for (old_key, new_key) in &body {
                    if let Some(val) = headers.remove(old_key) {
                        headers.insert(new_key.clone(), val);
                    }
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Response header key updated",
    )
}

pub async fn update_domain_response_header_val(
    Path(domain): Path<String>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_response
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.add_headers {
                for (key, new_val) in &body {
                    if headers.contains_key(key) {
                        headers.insert(key.clone(), new_val.clone());
                    }
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Response header value updated",
    )
}

/* --- Domain Response Remove-Headers ---------------------------------------- */

pub async fn add_domain_response_remove_header(Path(domain): Path<String>) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    match idx {
        Some(idx) => {
            if let Some(rule) = config
                .modify_response
                .as_mut()
                .and_then(|rules| rules.get_mut(idx))
            {
                rule.remove_headers
                    .get_or_insert_with(Vec::new)
                    .push(String::new());
            }
        }
        None => {
            config
                .modify_response
                .get_or_insert_with(Vec::new)
                .push(dope_core::ResponseModifier {
                    domain: domain.clone(),
                    remove_headers: Some(vec![String::new()]),
                    ..Default::default()
                });
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Remove-header added",
    )
}

pub async fn remove_domain_response_remove_header(
    Path((domain, hi)): Path<(String, usize)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_response
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.remove_headers {
                if hi < headers.len() {
                    headers.remove(hi);
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Remove-header removed",
    )
}

pub async fn update_domain_response_remove_header(
    Path((domain, hi)): Path<(String, usize)>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_response
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.remove_headers {
                if hi < headers.len() {
                    for (_, val) in &body {
                        headers[hi] = val.clone();
                    }
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Remove-header updated",
    )
}

/* --- Domain Request Headers ------------------------------------------------ */

pub async fn add_domain_request_header(Path(domain): Path<String>) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    match idx {
        Some(idx) => {
            if let Some(rule) = config
                .modify_request
                .as_mut()
                .and_then(|rules| rules.get_mut(idx))
            {
                let headers = rule.add_headers.get_or_insert_with(HashMap::new);
                let n = headers.len();
                headers.insert(format!("x-new-header-{n}"), String::new());
            }
        }
        None => {
            let mut headers = HashMap::new();
            headers.insert("x-new-header-0".to_string(), String::new());
            config
                .modify_request
                .get_or_insert_with(Vec::new)
                .push(dope_core::RequestModifier {
                    domain: domain.clone(),
                    add_headers: Some(headers),
                    ..Default::default()
                });
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Request header added",
    )
}

pub async fn remove_domain_request_header(
    Path((domain, key)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_request
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.add_headers {
                headers.remove(&key);
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Request header removed",
    )
}

pub async fn update_domain_request_header_key(
    Path(domain): Path<String>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_request
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.add_headers {
                for (old_key, new_key) in &body {
                    if let Some(val) = headers.remove(old_key) {
                        headers.insert(new_key.clone(), val);
                    }
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Request header key updated",
    )
}

pub async fn update_domain_request_header_val(
    Path(domain): Path<String>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_request
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.add_headers {
                for (key, new_val) in &body {
                    if headers.contains_key(key) {
                        headers.insert(key.clone(), new_val.clone());
                    }
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Request header value updated",
    )
}

/* --- Domain Request Remove-Headers ----------------------------------------- */

pub async fn add_domain_request_remove_header(Path(domain): Path<String>) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    match idx {
        Some(idx) => {
            if let Some(rule) = config
                .modify_request
                .as_mut()
                .and_then(|rules| rules.get_mut(idx))
            {
                rule.remove_headers
                    .get_or_insert_with(Vec::new)
                    .push(String::new());
            }
        }
        None => {
            config
                .modify_request
                .get_or_insert_with(Vec::new)
                .push(dope_core::RequestModifier {
                    domain: domain.clone(),
                    remove_headers: Some(vec![String::new()]),
                    ..Default::default()
                });
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Remove-header added",
    )
}

pub async fn remove_domain_request_remove_header(
    Path((domain, hi)): Path<(String, usize)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_request
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.remove_headers {
                if hi < headers.len() {
                    headers.remove(hi);
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Remove-header removed",
    )
}

pub async fn update_domain_request_remove_header(
    Path((domain, hi)): Path<(String, usize)>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let idx = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().position(|r| r.domain == domain));

    if let Some(idx) = idx {
        if let Some(rule) = config
            .modify_request
            .as_mut()
            .and_then(|rules| rules.get_mut(idx))
        {
            if let Some(ref mut headers) = rule.remove_headers {
                if hi < headers.len() {
                    for (_, val) in &body {
                        headers[hi] = val.clone();
                    }
                }
            }
        }
    }

    save_config(&config).await;
    ok_html_with_toast(
        html::domain_card(&config, &domain, true),
        "Remove-header updated",
    )
}

/* --- Tests ----------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dirs() -> dope_core::Dirs {
        let root = std::env::temp_dir().join(format!(
            "dope-panel-config-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::create_dir_all(root.join("logs")).unwrap();

        dope_core::Dirs {
            config: root.join("config.toml"),
            scripts: root.join("scripts"),
            logs: root.join("logs"),
        }
    }

    #[tokio::test]
    async fn panel_can_fix_legacy_config() {
        let dirs = test_dirs();
        std::fs::write(&dirs.config, "[server]\nport = 8080\n").unwrap();
        dope_core::init_dirs(dirs);

        // Legacy config without `intercept` behaves as "configured".
        assert_eq!(dope_core::load_config().intercept_mode(), "configured");

        let mut body = HashMap::new();
        body.insert("value".to_string(), "none".to_string());
        let _ = update_server_intercept(axum::Form(body)).await;

        let config = dope_core::load_config();
        assert_eq!(config.server.intercept.as_deref(), Some("none"));
        assert_eq!(config.intercept_mode(), "none");

        let mut body = HashMap::new();
        body.insert(
            "value".to_string(),
            "www.amazon.it, amazon.it\nmedia-amazon.com".to_string(),
        );
        let _ = update_server_passthrough(axum::Form(body)).await;

        let config = dope_core::load_config();
        assert_eq!(
            config.server.passthrough,
            Some(vec![
                "amazon.it".to_string(),
                "media-amazon.com".to_string(),
                "www.amazon.it".to_string(),
            ])
        );
        assert!(!config.should_intercept("amazon.it"));
    }
}
