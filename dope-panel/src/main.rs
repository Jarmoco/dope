/* -----------------------------------------------------------------------------
 * dope-panel/src/main.rs
 * Web admin panel for dope proxy — serves a Vanilla JS SPA with JSON API.
 * -------------------------------------------------------------------------- */

use axum::{
    extract::Query,
    http::StatusCode,
    response::Html,
    routing::{get},
    Json, Router,
};
use serde::Deserialize;
use std::net::SocketAddr;
use tracing::*;

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
        .route("/", get(serve_index))
        .route("/api/config", get(get_config).put(update_config))
        .route("/api/logs", get(get_logs));

    let addr = SocketAddr::from(([127, 0, 0, 1], 9090));
    info!("dope-panel listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port 9090");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

/* --- Index ----------------------------------------------------------------- */

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/* --- Config Handlers ------------------------------------------------------- */

async fn get_config() -> Json<dope_core::Config> {
    let cfg = tokio::task::spawn_blocking(dope_core::load_config)
        .await
        .unwrap_or_else(|_| default_config());

    Json(cfg)
}

async fn update_config(
    Json(config): Json<dope_core::Config>,
) -> StatusCode {
    let result = tokio::task::spawn_blocking(move || dope_core::save_config(&config))
        .await;

    match result {
        Ok(Ok(())) => StatusCode::OK,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn default_config() -> dope_core::Config {
    dope_core::Config {
        server: dope_core::ServerConfig {
            port: 8080,
            pause: None,
        },
        scripts: None,
        modify_response: None,
        modify_request: None,
    }
}

/* --- Log Handlers ---------------------------------------------------------- */

#[derive(Deserialize)]
struct LogQuery {
    since: Option<u64>,
    limit: Option<usize>,
}

async fn get_logs(Query(query): Query<LogQuery>) -> Json<Vec<dope_core::LogEntry>> {
    let since = query.since.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);

    let entries = tokio::task::spawn_blocking(move || dope_core::read_log_entries(since, limit))
        .await
        .unwrap_or_default();

    Json(entries)
}
