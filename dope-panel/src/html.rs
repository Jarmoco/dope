use std::collections::HashMap;

use dope_core::{Config, LogEntry, RequestModifier, ResponseModifier, ScriptRule};
use include_dir::{include_dir, Dir};

static TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");

fn template(name: &str) -> &'static str {
    TEMPLATES.get_file(name).unwrap().contents_utf8().unwrap()
}

fn render(t: &str, params: &[(&str, &str)]) -> String {
    let mut s = t.to_string();
    for (k, v) in params {
        s = s.replace(&format!("{{{}}}", k), v);
    }
    s
}

/* --- Page Shell ------------------------------------------------------------ */

pub fn page_shell(content: &str) -> String {
    render(template("page-shell.html"), &[("content", content)])
}

/* --- Dashboard ------------------------------------------------------------- */

pub fn dashboard_page(total: usize, hosts: usize, errors: usize) -> String {
    let stats = stats_cards(total, hosts, errors);
    render(template("dashboard-page.html"), &[("stats", &stats)])
}

pub fn stats_cards(total: usize, hosts: usize, errors: usize) -> String {
    render(
        template("dashboard-stats.html"),
        &[
            ("total", &total.to_string()),
            ("hosts", &hosts.to_string()),
            ("errors", &errors.to_string()),
        ],
    )
}

/* --- Logs ------------------------------------------------------------------ */

pub fn logs_page() -> String {
    template("logs-page.html").to_string()
}

pub fn log_rows(entries: &[LogEntry], search: Option<&str>) -> String {
    let filtered = match search {
        Some(q) if !q.is_empty() => {
            let q = q.to_lowercase();
            let matching_ids: std::collections::HashSet<String> = entries
                .iter()
                .filter(|e| {
                    let s = serde_json::to_string(e).unwrap_or_default();
                    s.to_lowercase().contains(&q)
                })
                .map(|e| e.req_id().to_string())
                .collect();
            entries
                .iter()
                .filter(|e| matching_ids.contains(&e.req_id().to_string()))
                .cloned()
                .collect::<Vec<_>>()
        }
        _ => entries.to_vec(),
    };

    if filtered.is_empty() {
        return render(template("empty-state.html"), &[("message", "No matching entries.")]);
    }

    let groups = group_log_entries(&filtered);

    if groups.is_empty() {
        return render(template("empty-state.html"), &[("message", "No matching entries.")]);
    }

    let rows: String = groups
        .iter()
        .map(|g| combined_row(g, true))
        .collect::<Vec<_>>()
        .join("");

    render(template("log-table.html"), &[("rows", &rows)])
}

/* --- Log Helpers ----------------------------------------------------------- */

struct LogGroup {
    request: Option<LogEntry>,
    response: Option<LogEntry>,
    error: Option<LogEntry>,
    ts: u64,
}

fn group_log_entries(entries: &[LogEntry]) -> Vec<LogGroup> {
    let mut map: HashMap<String, LogGroup> = HashMap::new();
    for e in entries {
        let id = e.req_id().to_string();
        let ts = e.ts();
        let g = map.entry(id).or_insert(LogGroup {
            request: None,
            response: None,
            error: None,
            ts: u64::MAX,
        });
        match e {
            LogEntry::Request { .. } => g.request = Some(e.clone()),
            LogEntry::Response { .. } => g.response = Some(e.clone()),
            LogEntry::Error { .. } => g.error = Some(e.clone()),
        }
        if ts < g.ts {
            g.ts = ts;
        }
    }
    let mut result: Vec<LogGroup> = map.into_values().collect();
    result.sort_by_key(|g| std::cmp::Reverse(g.ts));
    result
}

fn combined_row(g: &LogGroup, show_expanded: bool) -> String {
    let ts = format_ts(g.ts);
    let req = &g.request;
    let resp = &g.response;
    let err = &g.error;

    let method = req
        .as_ref()
        .map(|r| match r {
            LogEntry::Request { method, .. } => method.as_str(),
            _ => "???",
        })
        .unwrap_or_else(|| if err.is_some() { "ERR" } else { "???" });

    let host_val = req
        .as_ref()
        .map(|r| match r {
            LogEntry::Request { host, .. } => host.as_str(),
            _ => "-",
        })
        .unwrap_or_else(|| {
            err.as_ref()
                .map(|e| match e {
                    LogEntry::Error { client_addr, .. } => client_addr.as_str(),
                    _ => "-",
                })
                .unwrap_or("-")
        });

    let status = resp.as_ref().map(|r| match r {
        LogEntry::Response { status, .. } => *status,
        _ => 0,
    });
    let has_err = err.is_some();
    let status_display = match status {
        Some(s) => s.to_string(),
        None => {
            if has_err {
                "ERR".to_string()
            } else {
                "-".to_string()
            }
        }
    };

    let content_type = resp
        .as_ref()
        .and_then(|r| match r {
            LogEntry::Response { content_type, .. } => Some(content_type.as_str()),
            _ => None,
        })
        .unwrap_or("");

    let duration = match (&req, &resp) {
        (
            Some(LogEntry::Request { ts: req_ts, .. }),
            Some(LogEntry::Response { ts: resp_ts, .. }),
        ) => {
            let d = resp_ts.saturating_sub(*req_ts).max(1);
            format!("{}ms", d)
        }
        _ => "-".to_string(),
    };

    let badge_class = match status {
        Some(s) if s < 300 => "2xx",
        Some(s) if s < 400 => "3xx",
        Some(s) if s < 500 => "4xx",
        Some(_) => "5xx",
        None => {
            if has_err {
                "error"
            } else {
                "request"
            }
        }
    };

    let mut resp_details = String::new();
    if !content_type.is_empty() {
        resp_details.push_str(&format!(
            r##"<span class="meta">{}</span> "##,
            html_esc(content_type)
        ));
    }
    if duration != "-" {
        resp_details.push_str(&format!(r##"<span class="duration">{}</span> "##, duration));
    }

    let details_expanded = if show_expanded {
        let mut d = String::new();
        if let Some(LogEntry::Request {
            method,
            uri,
            host,
            user_agent,
            accept,
            ..
        }) = req
        {
            d.push_str(&format!(
                r##"<div class="detail-section"><h4>Request</h4><pre>{}</pre></div>"##,
                html_esc(
                    &serde_json::to_string_pretty(&serde_json::json!({
                        "method": method,
                        "uri": uri,
                        "host": host,
                        "user_agent": user_agent,
                        "accept": accept
                    }))
                    .unwrap_or_default()
                )
            ));
        }
        if let Some(LogEntry::Response {
            status,
            content_type,
            body_preview,
            ..
        }) = resp
        {
            d.push_str(&format!(
                r##"<div class="detail-section"><h4>Response</h4><pre>{}</pre></div>"##,
                html_esc(
                    &serde_json::to_string_pretty(&serde_json::json!({
                        "status": status,
                        "content_type": content_type,
                        "body_preview": body_preview
                    }))
                    .unwrap_or_default()
                )
            ));
        }
        if let Some(LogEntry::Error {
            client_addr,
            error,
            ..
        }) = err
        {
            d.push_str(&format!(
                r##"<div class="detail-section"><h4>Error</h4><pre>{}</pre></div>"##,
                html_esc(
                    &serde_json::to_string_pretty(&serde_json::json!({
                        "client_addr": client_addr,
                        "error": error
                    }))
                    .unwrap_or_default()
                )
            ));
        }
        d
    } else {
        String::new()
    };

    render(
        template("combined-row.html"),
        &[
            ("ts", &ts),
            ("badge", badge_class),
            ("method", &html_esc(method)),
            ("host", &html_esc(host_val)),
            ("status", &html_esc(&status_display)),
            ("resp_details", &resp_details),
            ("details_display", "none"),
            ("details", &details_expanded),
        ],
    )
}

fn format_ts(ts: u64) -> String {
    let secs = ts / 1000;
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn urlencode(s: &str) -> String {
    let mut result = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

/* --- Config Page ----------------------------------------------------------- */

pub fn config_page(config: &Config) -> String {
    render(
        template("config-page.html"),
        &[
            ("port", &config.server.port.to_string()),
            (
                "paused",
                if config.server.pause.unwrap_or(false) {
                    " checked"
                } else {
                    ""
                },
            ),
            ("domain_cards", &domain_list(config)),
        ],
    )
}

fn all_domains(config: &Config) -> Vec<String> {
    let mut domains: Vec<String> = Vec::new();
    if let Some(ref rules) = config.scripts {
        for rule in rules {
            if !domains.contains(&rule.domain) {
                domains.push(rule.domain.clone());
            }
        }
    }
    if let Some(ref rules) = config.modify_response {
        for rule in rules {
            if !domains.contains(&rule.domain) {
                domains.push(rule.domain.clone());
            }
        }
    }
    if let Some(ref rules) = config.modify_request {
        for rule in rules {
            if !domains.contains(&rule.domain) {
                domains.push(rule.domain.clone());
            }
        }
    }
    domains.sort();
    domains
}

pub fn domain_list(config: &Config) -> String {
    let domains = all_domains(config);
    if domains.is_empty() {
        return render(template("empty-state.html"), &[("message", "No domain rules configured.")]);
    }
    domains
        .iter()
        .map(|d| domain_card(config, d, false))
        .collect::<Vec<_>>()
        .join("")
}

pub fn domain_card(config: &Config, domain: &str, expanded: bool) -> String {
    let script = config
        .scripts
        .as_ref()
        .and_then(|rules| rules.iter().find(|r| r.domain == domain));
    let response = config
        .modify_response
        .as_ref()
        .and_then(|rules| rules.iter().find(|r| r.domain == domain));
    let request = config
        .modify_request
        .as_ref()
        .and_then(|rules| rules.iter().find(|r| r.domain == domain));

    let body_display = if expanded { "block" } else { "none" };
    let toggle_icon = if expanded { "\u{25BC}" } else { "\u{25B6}" };

    let summary = build_domain_summary(script, response, request);

    let scripts_val = script.map(|s| s.scripts.join(", ")).unwrap_or_default();
    let csp_opts = csp_options(response.and_then(|r| r.csp.as_deref()));
    let inject_opts = inject_options(response.and_then(|r| r.inject_at.as_deref()));

    let (response_remove_headers, response_headers) = match response {
        Some(r) => (
            build_remove_headers_list("response", domain, r.remove_headers.as_deref()),
            build_headers_list("response", domain, r.add_headers.as_ref()),
        ),
        None => (String::new(), String::new()),
    };

    let (request_remove_headers, request_headers) = match request {
        Some(r) => (
            build_remove_headers_list("request", domain, r.remove_headers.as_deref()),
            build_headers_list("request", domain, r.add_headers.as_ref()),
        ),
        None => (String::new(), String::new()),
    };

    render(
        template("domain-card.html"),
        &[
            ("domain_url", &urlencode(domain)),
            ("domain", &html_esc(domain)),
            ("toggle_icon", toggle_icon),
            ("body_display", body_display),
            ("summary", &summary),
            ("scripts_val", &html_esc(&scripts_val)),
            ("csp_options", &csp_opts),
            ("inject_options", &inject_opts),
            ("response_remove_headers", &response_remove_headers),
            ("response_headers", &response_headers),
            ("request_remove_headers", &request_remove_headers),
            ("request_headers", &request_headers),
        ],
    )
}

fn build_domain_summary(
    script: Option<&ScriptRule>,
    response: Option<&ResponseModifier>,
    request: Option<&RequestModifier>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = script {
        if !s.scripts.is_empty() {
            parts.push(format!("Scripts: {}", s.scripts.len()));
        }
    }
    if let Some(r) = response {
        let mut sub: Vec<String> = Vec::new();
        if r.csp.is_some() {
            sub.push("CSP".to_string());
        }
        if let Some(ref hs) = r.remove_headers {
            if !hs.is_empty() {
                sub.push(format!("-{} hdrs", hs.len()));
            }
        }
        if let Some(ref hs) = r.add_headers {
            if !hs.is_empty() {
                sub.push(format!("+{} hdrs", hs.len()));
            }
        }
        if r.inject_at.is_some() {
            sub.push("inject".to_string());
        }
        if !sub.is_empty() {
            parts.push(format!("Response: {}", sub.join(", ")));
        }
    }
    if let Some(r) = request {
        let mut sub: Vec<String> = Vec::new();
        if let Some(ref hs) = r.remove_headers {
            if !hs.is_empty() {
                sub.push(format!("-{} hdrs", hs.len()));
            }
        }
        if let Some(ref hs) = r.add_headers {
            if !hs.is_empty() {
                sub.push(format!("+{} hdrs", hs.len()));
            }
        }
        if !sub.is_empty() {
            parts.push(format!("Request: {}", sub.join(", ")));
        }
    }
    if parts.is_empty() {
        "No rules configured".to_string()
    } else {
        parts.join(" | ")
    }
}

/* --- Config Shared Helpers ------------------------------------------------- */

fn csp_options(current: Option<&str>) -> String {
    render(
        template("csp-options.html"),
        &[
            (
                "csp_none",
                if current.is_none() || current == Some("") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "csp_remove_nonce",
                if current == Some("remove_nonce") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "csp_remove_all",
                if current == Some("remove_all") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "csp_relax_connect_src",
                if current == Some("relax_connect_src") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "csp_keep",
                if current == Some("keep") {
                    " selected"
                } else {
                    ""
                },
            ),
        ],
    )
}

fn inject_options(current: Option<&str>) -> String {
    render(
        template("inject-options.html"),
        &[
            (
                "inject_default",
                if current.is_none() || current == Some("") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "inject_head_end",
                if current == Some("head_end") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "inject_body_end",
                if current == Some("body_end") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "inject_html_end",
                if current == Some("html_end") {
                    " selected"
                } else {
                    ""
                },
            ),
            (
                "inject_append",
                if current == Some("append") {
                    " selected"
                } else {
                    ""
                },
            ),
        ],
    )
}

fn build_remove_headers_list(ty: &str, domain: &str, headers: Option<&[String]>) -> String {
    match headers {
        Some(hs) => hs
            .iter()
            .enumerate()
            .map(|(hi, h)| remove_header_row(ty, domain, hi, h))
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

fn build_headers_list(
    ty: &str,
    domain: &str,
    headers: Option<&HashMap<String, String>>,
) -> String {
    match headers {
        Some(hs) => hs
            .iter()
            .enumerate()
            .map(|(hi, (k, v))| header_pair_row(ty, domain, hi, k, v))
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

fn header_pair_row(ty: &str, domain: &str, _hi: usize, key: &str, val: &str) -> String {
    let ty_lower = ty.to_lowercase();
    render(
        template("header-pair.html"),
        &[
            ("domain_url", &urlencode(domain)),
            ("ty_lower", &ty_lower),
            ("key", &html_esc(key)),
            ("key_url", &urlencode(key)),
            ("val", &html_esc(val)),
        ],
    )
}

fn remove_header_row(ty: &str, domain: &str, hi: usize, val: &str) -> String {
    let ty_lower = ty.to_lowercase();
    render(
        template("remove-header.html"),
        &[
            ("domain_url", &urlencode(domain)),
            ("ty_lower", &ty_lower),
            ("hi", &hi.to_string()),
            ("val", &html_esc(val)),
        ],
    )
}
