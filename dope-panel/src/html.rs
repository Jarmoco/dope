/* -----------------------------------------------------------------------------
 * dope-panel/src/html.rs
 * Generates all HTML fragments for the admin panel via format!().
 * -------------------------------------------------------------------------- */

use std::collections::HashMap;

use dope_core::{Config, LogEntry, RequestModifier, ResponseModifier, ScriptRule};

/* --- Page Shell ------------------------------------------------------------ */

pub fn page_shell(content: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>dope panel</title>
<link rel="stylesheet" href="/static/style.css">
<script src="/static/htmx.min.js"></script>
</head>
<body>
<nav>
  <h1>dope panel</h1>
  <a href="/dashboard" class="nav-link" hx-get="/dashboard" hx-target="#app" hx-push-url="true">Dashboard</a>
  <a href="/logs" class="nav-link" hx-get="/logs" hx-target="#app" hx-push-url="true">Logs</a>
  <a href="/config" class="nav-link" hx-get="/config" hx-target="#app" hx-push-url="true">Config</a>
</nav>
<div id="app">{}</div>
<div id="toast" class="toast"></div>
<script>
  htmx.config.defaultSwap = "innerHTML";
  document.body.addEventListener('htmx:after:swap', function() {{
    document.querySelectorAll('.nav-link').forEach(function(a) {{
      a.classList.toggle('active', a.pathname === location.pathname);
    }});
  }});
  document.body.addEventListener('showToast', function(evt) {{
    var el = document.getElementById('toast');
    el.textContent = evt.detail.value;
    el.classList.add('show');
    setTimeout(function() {{ el.classList.remove('show'); }}, 2500);
  }});
  function toggleRow(tr) {{
    var next = tr.nextElementSibling;
    if (next && next.style.display === 'none') next.style.display = 'table-row';
    else if (next) next.style.display = 'none';
  }}
</script>
</body>
</html>"##,
        content
    )
}

/* --- Dashboard ------------------------------------------------------------- */

pub fn dashboard_page(total: usize, hosts: usize, errors: usize) -> String {
    format!(
        r##"<h2>Dashboard</h2>
<div class="stats" id="stats" hx-get="/api/html/dashboard-stats" hx-trigger="every 3s" hx-target="#stats" hx-swap="innerHTML">{}</div>
<h2>Recent Activity</h2>
<div id="activity" hx-get="/api/html/activity" hx-trigger="load, every 3s" hx-target="#activity" hx-swap="innerHTML"></div>"##,
        stats_cards(total, hosts, errors)
    )
}

pub fn stats_cards(total: usize, hosts: usize, errors: usize) -> String {
    format!(
        r##"<div class="stat-card"><div class="value">{total}</div><div class="label">Total Entries</div></div>
<div class="stat-card"><div class="value">{hosts}</div><div class="label">Unique Hosts</div></div>
<div class="stat-card"><div class="value">{errors}</div><div class="label">Errors</div></div>"##,
        total = total,
        hosts = hosts,
        errors = errors
    )
}

pub fn activity_table(entries: &[LogEntry]) -> String {
    if entries.is_empty() {
        return r##"<div class="empty">No entries yet.</div>"##.to_string();
    }
    let rows = activity_rows(entries);
    format!(
        r##"<table>
<thead><tr><th>Time</th><th>Method</th><th>Host</th><th>Status</th><th>Response</th></tr></thead>
<tbody>{}</tbody>
</table>"##,
        rows
    )
}

fn activity_rows(entries: &[LogEntry]) -> String {
    let groups = group_log_entries(entries);
    let top: Vec<_> = groups.iter().take(20).collect();
    top.iter()
        .map(|g| combined_row(g, false))
        .collect::<Vec<_>>()
        .join("")
}

/* --- Logs ------------------------------------------------------------------ */

pub fn logs_page() -> String {
    r##"<h2>Logs</h2>
<div class="filters">
  <input type="text" id="filter-search" name="search" placeholder="Search by host, method, status, content type..."
         hx-get="/api/html/logs" hx-trigger="input delay:500ms" hx-target="#log-table"
         hx-swap="innerHTML" style="flex:1">
</div>
<div id="log-table" hx-get="/api/html/logs" hx-trigger="load, every 3s" hx-target="#log-table"
     hx-swap="innerHTML" hx-include="#filter-search"></div>"##.to_string()
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
        return r##"<div class="empty">No matching entries.</div>"##.to_string();
    }

    let groups = group_log_entries(&filtered);

    if groups.is_empty() {
        return r##"<div class="empty">No matching entries.</div>"##.to_string();
    }

    let rows: String = groups
        .iter()
        .map(|g| combined_row(g, true))
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<table>
<thead><tr><th>Time</th><th>Method</th><th>Host</th><th>Status</th><th>Response</th></tr></thead>
<tbody>{}</tbody>
</table>"##,
        rows
    )
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

    let method = req.as_ref().map(|r| match r {
        LogEntry::Request { method, .. } => method.as_str(),
        _ => "???",
    }).unwrap_or_else(|| if err.is_some() { "ERR" } else { "???" });

    let host_val = req.as_ref().map(|r| match r {
        LogEntry::Request { host, .. } => host.as_str(),
        _ => "-",
    }).unwrap_or_else(|| err.as_ref().map(|e| match e {
        LogEntry::Error { client_addr, .. } => client_addr.as_str(),
        _ => "-",
    }).unwrap_or("-"));

    let status = resp.as_ref().map(|r| match r {
        LogEntry::Response { status, .. } => *status,
        _ => 0,
    });
    let has_err = err.is_some();
    let status_display = match status {
        Some(s) => s.to_string(),
        None => {
            if has_err { "ERR".to_string() } else { "-".to_string() }
        }
    };

    let content_type = resp.as_ref().and_then(|r| match r {
        LogEntry::Response { content_type, .. } => Some(content_type.as_str()),
        _ => None,
    }).unwrap_or("");

    let duration = match (&req, &resp) {
        (Some(LogEntry::Request { ts: req_ts, .. }), Some(LogEntry::Response { ts: resp_ts, .. })) => {
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
            if has_err { "error" } else { "request" }
        }
    };

    let mut resp_details = String::new();
    if !content_type.is_empty() {
        resp_details.push_str(&format!(r##"<span class="meta">{}</span> "##, html_esc(content_type)));
    }
    if duration != "-" {
        resp_details.push_str(&format!(r##"<span class="duration">{}</span> "##, duration));
    }

    let details_expanded = if show_expanded {
        let mut d = String::new();
        if let Some(LogEntry::Request { method, uri, host, user_agent, accept, .. }) = req {
            d.push_str(&format!(
                r##"<div class="detail-section"><h4>Request</h4><pre>{}</pre></div>"##,
                html_esc(&serde_json::to_string_pretty(&serde_json::json!({
                    "method": method, "uri": uri, "host": host,
                    "user_agent": user_agent, "accept": accept
                })).unwrap_or_default())
            ));
        }
        if let Some(LogEntry::Response { status, content_type, body_preview, .. }) = resp {
            d.push_str(&format!(
                r##"<div class="detail-section"><h4>Response</h4><pre>{}</pre></div>"##,
                html_esc(&serde_json::to_string_pretty(&serde_json::json!({
                    "status": status, "content_type": content_type,
                    "body_preview": body_preview
                })).unwrap_or_default())
            ));
        }
        if let Some(LogEntry::Error { client_addr, error, .. }) = err {
            d.push_str(&format!(
                r##"<div class="detail-section"><h4>Error</h4><pre>{}</pre></div>"##,
                html_esc(&serde_json::to_string_pretty(&serde_json::json!({
                    "client_addr": client_addr, "error": error
                })).unwrap_or_default())
            ));
        }
        d
    } else {
        String::new()
    };

    format!(
        r##"<tr class="combined-row" onclick="toggleRow(this)" style="cursor:pointer">
  <td>{ts}</td>
  <td><span class="badge badge-{badge}">{method}</span></td>
  <td>{host}</td>
  <td><span class="badge badge-{badge}">{status_display}</span></td>
  <td>{resp_details}</td>
</tr>
<tr class="combined-details" style="display:{details_display}"><td colspan="5">{details}</td></tr>"##,
        ts = ts,
        badge = badge_class,
        method = html_esc(method),
        host = html_esc(host_val),
        status_display = html_esc(&status_display),
        resp_details = resp_details,
        details_display = if details_expanded.is_empty() { "none" } else { "none" },
        details = details_expanded,
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

/* --- Config ---------------------------------------------------------------- */

pub fn config_page(config: &Config) -> String {
    format!(
        r##"<h2>Configuration</h2>
<div class="config-section">
  <h3>Server</h3>
  <div class="config-row">
    <label>Port</label>
    <input type="number" id="cfg-port" name="value" value="{port}" min="1" max="65535"
           hx-put="/api/html/config/server/port" hx-trigger="change" hx-target="#app" hx-swap="innerHTML">
  </div>
  <div class="config-row">
    <label>Pause</label>
    <input type="checkbox" id="cfg-pause" name="cfg-pause"{paused}
           hx-put="/api/html/config/server/pause" hx-trigger="change" hx-target="#app" hx-swap="innerHTML">
  </div>
</div>
<div class="config-section">
  <h3>Script Rules</h3>
  <div id="scripts-list">{scripts}</div>
  <button class="btn btn-primary btn-sm add-row" hx-post="/api/html/config/scripts" hx-target="#scripts-list" hx-swap="innerHTML">+ Add Rule</button>
</div>
<div class="config-section">
  <h3>Response Modifiers</h3>
  <div id="response-list">{response}</div>
  <button class="btn btn-primary btn-sm add-row" hx-post="/api/html/config/response" hx-target="#response-list" hx-swap="innerHTML">+ Add Rule</button>
</div>
<div class="config-section">
  <h3>Request Modifiers</h3>
  <div id="request-list">{request}</div>
  <button class="btn btn-primary btn-sm add-row" hx-post="/api/html/config/request" hx-target="#request-list" hx-swap="innerHTML">+ Add Rule</button>
</div>"##,
        port = config.server.port,
        paused = if config.server.pause.unwrap_or(false) { " checked" } else { "" },
        scripts = scripts_section(config.scripts.as_deref().unwrap_or_default()),
        response = response_section(config.modify_response.as_deref().unwrap_or_default()),
        request = request_section(config.modify_request.as_deref().unwrap_or_default()),
    )
}

/* --- Script Rules ---------------------------------------------------------- */

pub fn scripts_section(rules: &[ScriptRule]) -> String {
    if rules.is_empty() {
        return r##"<div class="empty">No script rules configured.</div>"##.to_string();
    }
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| script_card(i, r))
        .collect::<Vec<_>>()
        .join("")
}

pub fn script_card(idx: usize, r: &ScriptRule) -> String {
    let domain_esc = html_esc(&r.domain);
    let scripts_val = html_esc(&r.scripts.join(", "));
    format!(
        r##"<div class="rule-card" id="script-card-{idx}">
  <div class="rule-header">
    <span class="domain-label">{domain}</span>
    <button class="btn btn-danger btn-sm" hx-delete="/api/html/config/scripts/{idx}" hx-target="#scripts-list" hx-swap="innerHTML">Remove</button>
  </div>
  <div class="config-row">
    <label>Domain</label>
    <input type="text" value="{domain}" name="domain"
           hx-put="/api/html/config/scripts/{idx}" hx-trigger="change" hx-target="#script-card-{idx}" hx-swap="outerHTML">
  </div>
  <div class="config-row">
    <label>Scripts</label>
    <input type="text" value="{scripts}" name="scripts" placeholder="comma-separated"
           hx-put="/api/html/config/scripts/{idx}" hx-trigger="change" hx-target="#script-card-{idx}" hx-swap="outerHTML">
  </div>
</div>"##,
        idx = idx,
        domain = domain_esc,
        scripts = scripts_val,
    )
}

/* --- Response Rules -------------------------------------------------------- */

pub fn response_section(rules: &[ResponseModifier]) -> String {
    if rules.is_empty() {
        return r##"<div class="empty">No response modifiers configured.</div>"##.to_string();
    }
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| response_card(i, r))
        .collect::<Vec<_>>()
        .join("")
}

pub fn response_card(idx: usize, r: &ResponseModifier) -> String {
    let domain_esc = html_esc(&r.domain);
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

    format!(
        r##"<div class="rule-card" id="response-card-{idx}">
  <div class="rule-header">
    <span class="domain-label">{domain}</span>
    <button class="btn btn-danger btn-sm" hx-delete="/api/html/config/response/{idx}" hx-target="#response-list" hx-swap="innerHTML">Remove</button>
  </div>
  <div class="config-row">
    <label>Domain</label>
    <input type="text" value="{domain}" name="domain"
           hx-put="/api/html/config/response/{idx}" hx-trigger="change" hx-target="#response-card-{idx}" hx-swap="outerHTML">
  </div>
  <div class="config-row">
    <label>CSP</label>
    <select name="csp" hx-put="/api/html/config/response/{idx}" hx-trigger="change" hx-target="#response-card-{idx}" hx-swap="outerHTML">
{csp}
    </select>
  </div>
  <div class="config-row"><label>Remove Headers</label></div>
  <div id="response-remove-headers-{idx}">{remove_headers}</div>
  <button class="btn btn-sm btn-secondary" hx-post="/api/html/config/response/{idx}/remove-headers" hx-target="#response-card-{idx}" hx-swap="outerHTML">+ Add Header to Remove</button>
  <div class="config-row config-row-gap">
    <label>Inject At</label>
    <select name="inject_at" hx-put="/api/html/config/response/{idx}" hx-trigger="change" hx-target="#response-card-{idx}" hx-swap="outerHTML">
{inject}
    </select>
  </div>
  <div class="config-row config-row-gap"><label>Add Headers</label></div>
  <div id="response-headers-{idx}">{headers}</div>
  <button class="btn btn-sm btn-secondary" hx-post="/api/html/config/response/{idx}/headers" hx-target="#response-card-{idx}" hx-swap="outerHTML">+ Add Header</button>
</div>"##,
        idx = idx,
        domain = domain_esc,
        csp = csp_opts,
        remove_headers = remove_headers_html,
        inject = inject_opts,
        headers = headers_html,
    )
}

/* --- Request Rules --------------------------------------------------------- */

pub fn request_section(rules: &[RequestModifier]) -> String {
    if rules.is_empty() {
        return r##"<div class="empty">No request modifiers configured.</div>"##.to_string();
    }
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| request_card(i, r))
        .collect::<Vec<_>>()
        .join("")
}

pub fn request_card(idx: usize, r: &RequestModifier) -> String {
    let domain_esc = html_esc(&r.domain);

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

    format!(
        r##"<div class="rule-card" id="request-card-{idx}">
  <div class="rule-header">
    <span class="domain-label">{domain}</span>
    <button class="btn btn-danger btn-sm" hx-delete="/api/html/config/request/{idx}" hx-target="#request-list" hx-swap="innerHTML">Remove</button>
  </div>
  <div class="config-row">
    <label>Domain</label>
    <input type="text" value="{domain}" name="domain"
           hx-put="/api/html/config/request/{idx}" hx-trigger="change" hx-target="#request-card-{idx}" hx-swap="outerHTML">
  </div>
  <div class="config-row"><label>Remove Headers</label></div>
  <div id="request-remove-headers-{idx}">{remove_headers}</div>
  <button class="btn btn-sm btn-secondary" hx-post="/api/html/config/request/{idx}/remove-headers" hx-target="#request-card-{idx}" hx-swap="outerHTML">+ Add Header to Remove</button>
  <div class="config-row config-row-gap"><label>Add Headers</label></div>
  <div id="request-headers-{idx}">{headers}</div>
  <button class="btn btn-sm btn-secondary" hx-post="/api/html/config/request/{idx}/headers" hx-target="#request-card-{idx}" hx-swap="outerHTML">+ Add Header</button>
</div>"##,
        idx = idx,
        domain = domain_esc,
        remove_headers = remove_headers_html,
        headers = headers_html,
    )
}

/* --- Shared Config Helpers ------------------------------------------------- */

fn csp_options(current: Option<&str>) -> String {
    let opts = [
        ("", "(none)"),
        ("remove_nonce", "remove_nonce"),
        ("remove_all", "remove_all"),
        ("relax_connect_src", "relax_connect_src"),
        ("keep", "keep"),
    ];
    opts.iter()
        .map(|(val, label)| {
            let selected = if current == Some(val) {
                " selected"
            } else {
                ""
            };
            format!(
                r##"        <option value="{}"{}>{}</option>"##,
                html_esc(val),
                selected,
                html_esc(label)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn inject_options(current: Option<&str>) -> String {
    let opts = [
        ("", "(default)"),
        ("head_end", "head_end"),
        ("body_end", "body_end"),
        ("html_end", "html_end"),
        ("append", "append"),
    ];
    opts.iter()
        .map(|(val, label)| {
            let selected = if current == Some(val) {
                " selected"
            } else {
                ""
            };
            format!(
                r##"        <option value="{}"{}>{}</option>"##,
                html_esc(val),
                selected,
                html_esc(label)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn header_pair_row(ty: &str, idx: usize, _hi: usize, key: &str, val: &str) -> String {
    let ty_lower = ty.to_lowercase();
    format!(
        r##"<div class="header-pair config-row">
  <input type="text" value="{key}" name="{key}" placeholder="Name"
         hx-put="/api/html/config/{ty_lower}/{idx}/headers/key" hx-trigger="change" hx-target="#{ty_lower}-card-{idx}" hx-swap="outerHTML">
  <span>=</span>
  <input type="text" value="{val}" name="{key}" placeholder="Value"
         hx-put="/api/html/config/{ty_lower}/{idx}/headers/val" hx-trigger="change" hx-target="#{ty_lower}-card-{idx}" hx-swap="outerHTML">
  <button class="btn btn-danger btn-sm" hx-delete="/api/html/config/{ty_lower}/{idx}/headers/{key_url}" hx-target="#{ty_lower}-card-{idx}" hx-swap="outerHTML">x</button>
</div>"##,
        ty_lower = ty_lower,
        idx = idx,
        key = html_esc(key),
        key_url = urlencode(key),
        val = html_esc(val),
    )
}

fn remove_header_row(ty: &str, idx: usize, hi: usize, val: &str) -> String {
    format!(
        r##"<div class="header-pair config-row">
  <input type="text" value="{val}" placeholder="Header name"
         hx-put="/api/html/config/{ty}/{idx}/remove-headers/{hi}" hx-trigger="change" hx-target="#{ty}-card-{idx}" hx-swap="outerHTML">
  <button class="btn btn-danger btn-sm" hx-delete="/api/html/config/{ty}/{idx}/remove-headers/{hi}" hx-target="#{ty}-card-{idx}" hx-swap="outerHTML">x</button>
</div>"##,
        ty = ty,
        idx = idx,
        hi = hi,
        val = html_esc(val),
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
