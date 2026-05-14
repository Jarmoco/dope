/* -----------------------------------------------------------------------------
 * modify.rs
 * Applies config-driven request and response modifiers (CSP, header add/remove).
 * -------------------------------------------------------------------------- */

use hudsucker::hyper::http::HeaderName;
use hudsucker::hyper::HeaderMap;
use tracing::*;

use crate::config::{RequestModifier, ResponseModifier};

/* --- Response Modifiers ---------------------------------------------------- */

pub fn apply_response_modifiers(headers: &mut HeaderMap, config: &ResponseModifier) {
    if let Some(csp_mode) = &config.csp {
        handle_csp(headers, csp_mode);
    }

    if let Some(remove) = &config.remove_headers {
        for name in remove {
            headers.remove(name.as_str());
        }
    }

    if let Some(add) = &config.add_headers {
        for (name, value) in add {
            let header_name: HeaderName = match name.parse() {
                Ok(n) => n,
                Err(e) => {
                    warn!("Invalid header name: {} ({})", name, e);
                    continue;
                }
            };
            match value.parse() {
                Ok(val) => {
                    headers.insert(header_name, val);
                }
                Err(e) => {
                    warn!("Invalid header value for {}: {} ({})", name, value, e);
                }
            }
        }
    }
}

/* --- Request Modifiers ----------------------------------------------------- */

pub fn apply_request_modifiers(headers: &mut HeaderMap, config: &RequestModifier) {
    if let Some(remove) = &config.remove_headers {
        for name in remove {
            headers.remove(name.as_str());
        }
    }

    if let Some(add) = &config.add_headers {
        for (name, value) in add {
            let header_name: HeaderName = match name.parse() {
                Ok(n) => n,
                Err(e) => {
                    warn!("Invalid header name: {} ({})", name, e);
                    continue;
                }
            };
            match value.parse() {
                Ok(val) => {
                    headers.insert(header_name, val);
                }
                Err(e) => {
                    warn!("Invalid header value for {}: {} ({})", name, value, e);
                }
            }
        }
    }
}

/* --- CSP Handler ----------------------------------------------------------- */

fn handle_csp(headers: &mut HeaderMap, mode: &str) {
    match mode {
        "remove_nonce" => {
            if let Some(csp) = headers.get("content-security-policy") {
                let csp_str = match csp.to_str() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                if csp_str.contains("'nonce-") {
                    headers.remove("content-security-policy");
                }
            }
        }
        "remove_all" => {
            headers.remove("content-security-policy");
        }
        "relax_connect_src" => {
            if let Some(csp) = headers.get("content-security-policy") {
                let csp_str = match csp.to_str() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let new_csp = csp_str.replace("connect-src 'self'", "connect-src *");
                match new_csp.parse() {
                    Ok(val) => {
                        headers.insert("content-security-policy", val);
                    }
                    Err(e) => {
                        warn!("Failed to update CSP header: {}", e);
                        headers.remove("content-security-policy");
                    }
                }
            }
        }
        "keep" => {}
        other => {
            warn!("Unknown CSP mode: {}", other);
        }
    }
}
