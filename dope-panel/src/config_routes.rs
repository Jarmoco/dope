/* -----------------------------------------------------------------------------
 * dope-panel/src/config_routes.rs
 * HTMX route handlers for config CRUD — each returns HTML fragments.
 * -------------------------------------------------------------------------- */

use std::collections::HashMap;

use axum::extract::Path;
use axum::Form;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use dope_core;

use crate::html;

/* --- Helpers --------------------------------------------------------------- */

async fn load_config() -> dope_core::Config {
    tokio::task::spawn_blocking(dope_core::load_config)
        .await
        .unwrap_or_else(|_| dope_core::Config {
            server: dope_core::ServerConfig { port: 8080, pause: None },
            scripts: None,
            modify_response: None,
            modify_request: None,
        })
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
        [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (trigger, json.as_str())],
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

pub async fn update_server_pause(body: String) -> impl IntoResponse {
    let mut config = load_config().await;
    config.server.pause = if body.contains("cfg-pause") { Some(true) } else { None };
    save_config(&config).await;
    ok_html_with_toast(html::config_page(&config), "Pause toggled")
}

/* --- Script Rules ---------------------------------------------------------- */

pub async fn add_script_rule() -> impl IntoResponse {
    let mut config = load_config().await;
    config.scripts.get_or_insert_with(Vec::new).push(dope_core::ScriptRule::default());
    save_config(&config).await;
    ok_html_with_toast(html::scripts_section(config.scripts.as_deref().unwrap_or_default()), "Script rule added")
}

pub async fn remove_script_rule(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    if let Some(rules) = &mut config.scripts {
        if idx < rules.len() {
            rules.remove(idx);
        }
    }
    save_config(&config).await;
    ok_html_with_toast(html::scripts_section(config.scripts.as_deref().unwrap_or_default()), "Script rule removed")
}

pub async fn update_script_rule(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.scripts.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            for (field, value) in &body {
                match field.as_str() {
                    "domain" => rule.domain = value.clone(),
                    "scripts" => {
                        rule.scripts = value
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    _ => {}
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.scripts {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::script_card(idx, rule), "Script rule updated");
        }
    }
    ok_html_with_toast(html::scripts_section(config.scripts.as_deref().unwrap_or_default()), "Script rule updated")
}

/* --- Response Rules -------------------------------------------------------- */

pub async fn add_response_rule() -> impl IntoResponse {
    let mut config = load_config().await;
    config.modify_response.get_or_insert_with(Vec::new).push(dope_core::ResponseModifier::default());
    save_config(&config).await;
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response rule added")
}

pub async fn remove_response_rule(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    if let Some(rules) = &mut config.modify_response {
        if idx < rules.len() { rules.remove(idx); }
    }
    save_config(&config).await;
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response rule removed")
}

pub async fn update_response_rule(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            for (field, value) in &body {
                match field.as_str() {
                    "domain" => rule.domain = value.clone(),
                    "csp" => rule.csp = if value.is_empty() { None } else { Some(value.clone()) },
                    "inject_at" => rule.inject_at = if value.is_empty() { None } else { Some(value.clone()) },
                    _ => {}
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Response rule updated");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response rule updated")
}

/* --- Response Headers ------------------------------------------------------ */

pub async fn add_response_header(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            let headers = rule.add_headers.get_or_insert_with(HashMap::new);
            let n = headers.len();
            headers.insert(format!("key{}", n), "value".to_string());
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Response header added");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response header added")
}

pub async fn remove_response_header(
    Path((idx, key)): Path<(usize, String)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.add_headers {
                headers.remove(&key);
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Response header removed");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response header removed")
}

pub async fn update_response_header_key(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.add_headers {
                for (old_key, new_key) in &body {
                    if let Some(val) = headers.remove(old_key) {
                        headers.insert(new_key.clone(), val);
                    }
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Response header key updated");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response header key updated")
}

pub async fn update_response_header_val(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.add_headers {
                for (key, new_val) in &body {
                    if headers.contains_key(key) {
                        headers.insert(key.clone(), new_val.clone());
                    }
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Response header value updated");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Response header value updated")
}

/* --- Response Remove-Headers ----------------------------------------------- */

pub async fn add_response_remove_header(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            rule.remove_headers.get_or_insert_with(Vec::new).push(String::new());
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Remove-header added");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Remove-header added")
}

pub async fn remove_response_remove_header(
    Path((idx, hi)): Path<(usize, usize)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.remove_headers {
                if hi < headers.len() { headers.remove(hi); }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Remove-header removed");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Remove-header removed")
}

pub async fn update_response_remove_header(
    Path((idx, hi)): Path<(usize, usize)>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_response.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.remove_headers {
                if hi < headers.len() {
                    for (_, val) in &body { headers[hi] = val.clone(); }
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_response {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::response_card(idx, rule), "Remove-header updated");
        }
    }
    ok_html_with_toast(html::response_section(config.modify_response.as_deref().unwrap_or_default()), "Remove-header updated")
}

/* --- Request Rules --------------------------------------------------------- */

pub async fn add_request_rule() -> impl IntoResponse {
    let mut config = load_config().await;
    config.modify_request.get_or_insert_with(Vec::new).push(dope_core::RequestModifier::default());
    save_config(&config).await;
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request rule added")
}

pub async fn remove_request_rule(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    if let Some(rules) = &mut config.modify_request {
        if idx < rules.len() { rules.remove(idx); }
    }
    save_config(&config).await;
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request rule removed")
}

pub async fn update_request_rule(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            for (field, value) in &body {
                if field == "domain" { rule.domain = value.clone(); }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Request rule updated");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request rule updated")
}

/* --- Request Headers ------------------------------------------------------- */

pub async fn add_request_header(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            let headers = rule.add_headers.get_or_insert_with(HashMap::new);
            let n = headers.len();
            headers.insert(format!("key{}", n), "value".to_string());
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Request header added");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request header added")
}

pub async fn remove_request_header(
    Path((idx, key)): Path<(usize, String)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.add_headers {
                headers.remove(&key);
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Request header removed");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request header removed")
}

pub async fn update_request_header_key(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.add_headers {
                for (old_key, new_key) in &body {
                    if let Some(val) = headers.remove(old_key) {
                        headers.insert(new_key.clone(), val);
                    }
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Request header key updated");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request header key updated")
}

pub async fn update_request_header_val(
    Path(idx): Path<usize>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.add_headers {
                for (key, new_val) in &body {
                    if headers.contains_key(key) {
                        headers.insert(key.clone(), new_val.clone());
                    }
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Request header value updated");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Request header value updated")
}

/* --- Request Remove-Headers ------------------------------------------------ */

pub async fn add_request_remove_header(Path(idx): Path<usize>) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            rule.remove_headers.get_or_insert_with(Vec::new).push(String::new());
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Remove-header added");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Remove-header added")
}

pub async fn remove_request_remove_header(
    Path((idx, hi)): Path<(usize, usize)>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.remove_headers {
                if hi < headers.len() { headers.remove(hi); }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Remove-header removed");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Remove-header removed")
}

pub async fn update_request_remove_header(
    Path((idx, hi)): Path<(usize, usize)>,
    Form(body): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut config = load_config().await;
    let mutated = config.modify_request.as_mut().and_then(|rules| {
        rules.get_mut(idx).map(|rule| {
            if let Some(headers) = &mut rule.remove_headers {
                if hi < headers.len() {
                    for (_, val) in &body { headers[hi] = val.clone(); }
                }
            }
        })
    }).is_some();
    if mutated { save_config(&config).await; }
    if let Some(rules) = &config.modify_request {
        if let Some(rule) = rules.get(idx) {
            return ok_html_with_toast(html::request_card(idx, rule), "Remove-header updated");
        }
    }
    ok_html_with_toast(html::request_section(config.modify_request.as_deref().unwrap_or_default()), "Remove-header updated")
}
