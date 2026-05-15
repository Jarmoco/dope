/* -----------------------------------------------------------------------------
 * dope-panel/src/main.rs
 * Web admin panel for dope proxy — serves an HTMX-powered HTML UI.
 * -------------------------------------------------------------------------- */

use std::collections::HashSet;
use std::net::SocketAddr;

use axum::{
    extract::Query,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use dope_core::{Config, LogEntry};
use serde::Deserialize;
use tracing::*;

mod config_routes;
mod html;

/* --- Main ------------------------------------------------------------------ */

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let app = Router::new()
        // Static
        .route("/static/style.css", get(serve_style))
        .route("/static/htmx.min.js", get(serve_htmx))
        // JSON API (keep existing)
        .route("/api/config", get(get_config).put(update_config))
        .route("/api/logs", get(get_logs))
        // Page routes
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/logs", get(serve_logs))
        .route("/config", get(serve_config))
        // HTML data routes
        .route("/api/html/dashboard-stats", get(serve_dashboard_stats))
        .route("/api/html/logs", get(serve_log_table))
        // Server config
        .route(
            "/api/html/config/server/port",
            put(config_routes::update_server_port),
        )
        .route(
            "/api/html/config/server/pause",
            put(config_routes::update_server_pause),
        )
        // Domain CRUD
        .route(
            "/api/html/config/domain",
            post(config_routes::add_domain),
        )
        .route(
            "/api/html/config/domain/:domain",
            delete(config_routes::remove_domain),
        )
        // Domain scripts
        .route(
            "/api/html/config/domain/:domain/scripts",
            put(config_routes::update_domain_scripts),
        )
        // Domain response modifiers
        .route(
            "/api/html/config/domain/:domain/response",
            put(config_routes::update_domain_response),
        )
        .route(
            "/api/html/config/domain/:domain/response/headers",
            post(config_routes::add_domain_response_header),
        )
        .route(
            "/api/html/config/domain/:domain/response/headers/:key",
            delete(config_routes::remove_domain_response_header),
        )
        .route(
            "/api/html/config/domain/:domain/response/headers/key",
            put(config_routes::update_domain_response_header_key),
        )
        .route(
            "/api/html/config/domain/:domain/response/headers/val",
            put(config_routes::update_domain_response_header_val),
        )
        .route(
            "/api/html/config/domain/:domain/response/remove-headers",
            post(config_routes::add_domain_response_remove_header),
        )
        .route(
            "/api/html/config/domain/:domain/response/remove-headers/:hi",
            delete(config_routes::remove_domain_response_remove_header),
        )
        .route(
            "/api/html/config/domain/:domain/response/remove-headers/:hi",
            put(config_routes::update_domain_response_remove_header),
        )
        // Domain request modifiers
        .route(
            "/api/html/config/domain/:domain/request/headers",
            post(config_routes::add_domain_request_header),
        )
        .route(
            "/api/html/config/domain/:domain/request/headers/:key",
            delete(config_routes::remove_domain_request_header),
        )
        .route(
            "/api/html/config/domain/:domain/request/headers/key",
            put(config_routes::update_domain_request_header_key),
        )
        .route(
            "/api/html/config/domain/:domain/request/headers/val",
            put(config_routes::update_domain_request_header_val),
        )
        .route(
            "/api/html/config/domain/:domain/request/remove-headers",
            post(config_routes::add_domain_request_remove_header),
        )
        .route(
            "/api/html/config/domain/:domain/request/remove-headers/:hi",
            delete(config_routes::remove_domain_request_remove_header),
        )
        .route(
            "/api/html/config/domain/:domain/request/remove-headers/:hi",
            put(config_routes::update_domain_request_remove_header),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 9090));
    info!("dope-panel listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port 9090");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

/* --- Helpers --------------------------------------------------------------- */

fn page_or_fragment(headers: &HeaderMap, content: String) -> Response {
    if headers.contains_key("hx-request") {
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content).into_response()
    } else {
        let full = html::page_shell(&content);
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], full).into_response()
    }
}

async fn load_log_entries(since: u64, limit: usize) -> Vec<LogEntry> {
    tokio::task::spawn_blocking(move || dope_core::read_log_entries(since, limit))
        .await
        .unwrap_or_default()
}

/* --- Static ---------------------------------------------------------------- */

async fn serve_style() -> impl IntoResponse {
    (
        [("content-type", "text/css")],
        include_str!("../static/style.css"),
    )
}

async fn serve_htmx() -> impl IntoResponse {
    (
        [("content-type", "application/javascript")],
        include_str!("../static/htmx.min.js"),
    )
}

/* --- Page Routes ----------------------------------------------------------- */

async fn count_log_entries() -> usize {
    tokio::task::spawn_blocking(dope_core::log_entry_count)
        .await
        .unwrap_or(0)
}

async fn serve_dashboard(headers: HeaderMap) -> Response {
    let total = count_log_entries().await;
    let entries = load_log_entries(0, 5000).await;
    let hosts = entries
        .iter()
        .filter_map(|e| match e {
            LogEntry::Request { host, .. } => Some(host.as_str()),
            _ => None,
        })
        .collect::<HashSet<_>>()
        .len();
    let errors = entries
        .iter()
        .filter(|e| {
            matches!(e, LogEntry::Error { .. })
                || matches!(e, LogEntry::Response { status, .. } if *status >= 500)
        })
        .count();
    let content = html::dashboard_page(total, hosts, errors);
    page_or_fragment(&headers, content)
}

async fn serve_logs(headers: HeaderMap) -> Response {
    let content = html::logs_page();
    page_or_fragment(&headers, content)
}

async fn serve_config(headers: HeaderMap) -> Response {
    let config = tokio::task::spawn_blocking(dope_core::load_config)
        .await
        .unwrap_or_else(|_| Config {
            server: dope_core::ServerConfig {
                port: 8080,
                pause: None,
            },
            scripts: None,
            modify_response: None,
            modify_request: None,
        });
    let content = html::config_page(&config);
    page_or_fragment(&headers, content)
}

/* --- HTML Data Routes ------------------------------------------------------ */

async fn serve_dashboard_stats() -> impl IntoResponse {
    let total = count_log_entries().await;
    let entries = load_log_entries(0, 5000).await;
    let hosts = entries
        .iter()
        .filter_map(|e| match e {
            LogEntry::Request { host, .. } => Some(host.as_str()),
            _ => None,
        })
        .collect::<HashSet<_>>()
        .len();
    let errors = entries
        .iter()
        .filter(|e| {
            matches!(e, LogEntry::Error { .. })
                || matches!(e, LogEntry::Response { status, .. } if *status >= 500)
        })
        .count();
    Html(html::stats_cards(total, hosts, errors))
}

/* --- JSON API (unchanged) -------------------------------------------------- */

#[derive(Deserialize)]
struct LogQuery {
    since: Option<u64>,
    limit: Option<usize>,
}

async fn get_config() -> Json<Config> {
    let cfg = tokio::task::spawn_blocking(dope_core::load_config)
        .await
        .unwrap_or_else(|_| Config {
            server: dope_core::ServerConfig {
                port: 8080,
                pause: None,
            },
            scripts: None,
            modify_response: None,
            modify_request: None,
        });
    Json(cfg)
}

async fn update_config(Json(config): Json<Config>) -> StatusCode {
    let result = tokio::task::spawn_blocking(move || dope_core::save_config(&config)).await;
    match result {
        Ok(Ok(())) => StatusCode::OK,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn get_logs(Query(query): Query<LogQuery>) -> Json<Vec<LogEntry>> {
    let since = query.since.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let entries = load_log_entries(since, limit).await;
    Json(entries)
}

/* --- Log Table (HTML fragment) --------------------------------------------- */

#[derive(Deserialize)]
struct LogTableQuery {
    since: Option<u64>,
    limit: Option<usize>,
    search: Option<String>,
}

async fn serve_log_table(Query(query): Query<LogTableQuery>) -> impl IntoResponse {
    let since = query.since.unwrap_or(0);
    let limit = query.limit.unwrap_or(500);
    let entries = load_log_entries(since, limit).await;
    Html(html::log_rows(&entries, query.search.as_deref()))
}
