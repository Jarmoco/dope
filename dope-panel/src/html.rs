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

pub fn page_shell(content: &str) -> String {
    render(template("page-shell.html"), &[("content", content)])
}

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

pub fn activity_table(entries: &[LogEntry]) -> String {
    if entries.is_empty() {
        return render(template("empty-state.html"), &[("message", "No entries yet.")]);
    }
    let rows = activity_rows(entries);
    render(template("activity-table.html"), &[("rows", &rows)])
}

fn activity_rows(entries: &[LogEntry]) -> String {
    let groups = group_log_entries(entries);
    let top: Vec<_> = groups.iter().take(20).collect();
    top.iter()
        .map(|g| combined_row(g, false))
        .collect::<Vec<_>>()
        .join("")
}

pub fn logs_page() -> String {
    template("logs-page.html").to_string()
}

pub fn log_rows(entries: &[LogEntry], search: Option<&str>) -> String {
    let filtered = match search {
        Some(q) if !q.is_empty() => {
            let q = q.to_lowercase();
            entries
                .iter()
                .filter(|e| {
                    let s = serde_json::to_string(e).unwrap_or_default();
                    s.to_lowercase().contains(&q)
                })
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
            (
                "scripts",
                &scripts_section(config.scripts.as_deref().unwrap_or_default()),
            ),
            (
                "response",
                &response_section(config.modify_response.as_deref().unwrap_or_default()),
            ),
            (
                "request",
                &request_section(config.modify_request.as_deref().unwrap_or_default()),
            ),
        ],
    )
}

pub fn scripts_section(rules: &[ScriptRule]) -> String {
    if rules.is_empty() {
        return render(template("empty-state.html"), &[("message", "No script rules configured.")]);
    }
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| script_card(i, r))
        .collect::<Vec<_>>()
        .join("")
}

pub fn script_card(idx: usize, r: &ScriptRule) -> String {
    render(
        template("script-card.html"),
        &[
            ("idx", &idx.to_string()),
            ("domain", &html_esc(&r.domain)),
            ("scripts", &html_esc(&r.scripts.join(", "))),
        ],
    )
}

pub fn response_section(rules: &[ResponseModifier]) -> String {
    if rules.is_empty() {
        return render(template("empty-state.html"), &[("message", "No response modifiers configured.")]);
    }
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| response_card(i, r))
        .collect::<Vec<_>>()
        .join("")
}

pub fn response_card(idx: usize, r: &ResponseModifier) -> String {
    let csp_opts = csp_options(r.csp.as_deref());
    let inject_opts = inject_options(r.inject_at.as_deref());

    let remove_headers_html = match &r.remove_headers {
        Some(hs) => hs
            .iter()
            .enumerate()
            .map(|(hi, h)| remove_header_row("response", idx, hi, h))
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    };

    let headers_html = match &r.add_headers {
        Some(hs) => hs
            .iter()
            .enumerate()
            .map(|(hi, (k, v))| header_pair_row("Response", idx, hi, k, v))
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    };

    render(
        template("response-card.html"),
        &[
            ("idx", &idx.to_string()),
            ("domain", &html_esc(&r.domain)),
            ("csp", &csp_opts),
            ("remove_headers", &remove_headers_html),
            ("inject", &inject_opts),
            ("headers", &headers_html),
        ],
    )
}

pub fn request_section(rules: &[RequestModifier]) -> String {
    if rules.is_empty() {
        return render(template("empty-state.html"), &[("message", "No request modifiers configured.")]);
    }
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| request_card(i, r))
        .collect::<Vec<_>>()
        .join("")
}

pub fn request_card(idx: usize, r: &RequestModifier) -> String {
    let remove_headers_html = match &r.remove_headers {
        Some(hs) => hs
            .iter()
            .enumerate()
            .map(|(hi, h)| remove_header_row("request", idx, hi, h))
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    };

    let headers_html = match &r.add_headers {
        Some(hs) => hs
            .iter()
            .enumerate()
            .map(|(hi, (k, v))| header_pair_row("Request", idx, hi, k, v))
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    };

    render(
        template("request-card.html"),
        &[
            ("idx", &idx.to_string()),
            ("domain", &html_esc(&r.domain)),
            ("remove_headers", &remove_headers_html),
            ("headers", &headers_html),
        ],
    )
}

fn csp_options(current: Option<&str>) -> String {
    render(
        template("csp-options.html"),
        &[
            ("csp_none", if current.is_none() || current == Some("") { " selected" } else { "" }),
            ("csp_remove_nonce", if current == Some("remove_nonce") { " selected" } else { "" }),
            ("csp_remove_all", if current == Some("remove_all") { " selected" } else { "" }),
            ("csp_relax_connect_src", if current == Some("relax_connect_src") { " selected" } else { "" }),
            ("csp_keep", if current == Some("keep") { " selected" } else { "" }),
        ],
    )
}

fn inject_options(current: Option<&str>) -> String {
    render(
        template("inject-options.html"),
        &[
            ("inject_default", if current.is_none() || current == Some("") { " selected" } else { "" }),
            ("inject_head_end", if current == Some("head_end") { " selected" } else { "" }),
            ("inject_body_end", if current == Some("body_end") { " selected" } else { "" }),
            ("inject_html_end", if current == Some("html_end") { " selected" } else { "" }),
            ("inject_append", if current == Some("append") { " selected" } else { "" }),
        ],
    )
}

fn header_pair_row(ty: &str, idx: usize, _hi: usize, key: &str, val: &str) -> String {
    let ty_lower = ty.to_lowercase();
    render(
        template("header-pair.html"),
        &[
            ("ty", ty),
            ("ty_lower", &ty_lower),
            ("idx", &idx.to_string()),
            ("key", &html_esc(key)),
            ("key_url", &urlencode(key)),
            ("val", &html_esc(val)),
        ],
    )
}

fn remove_header_row(ty: &str, idx: usize, hi: usize, val: &str) -> String {
    render(
        template("remove-header.html"),
        &[
            ("ty", ty),
            ("idx", &idx.to_string()),
            ("hi", &hi.to_string()),
            ("val", &html_esc(val)),
        ],
    )
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
