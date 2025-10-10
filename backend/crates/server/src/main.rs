use anyhow::Result;
use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::{env, net::SocketAddr};
use tokio::{net::TcpListener, signal};
use tracing::{error, info, Level};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let state = AppState::default();
    let app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .with_state(state);

    let addr: SocketAddr = bind_addr()?;
    let listener = TcpListener::bind(addr).await?;
    info!("listening on {addr}");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[derive(Clone, Default)]
struct AppState;

async fn health() -> &'static str {
    "ok"
}

fn init_tracing() {
    // Respect RUST_LOG if set, otherwise default to info for our crates.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,openguild_server=info,openguild=info"));

    let json = env::var("LOG_FORMAT").ok().as_deref() == Some("json");

    let builder = fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_level(true)
        .with_max_level(Level::INFO);

    if json {
        builder.json().init();
    } else {
        builder.compact().init();
    }
}

fn bind_addr() -> Result<SocketAddr> {
    // Prefer BIND_ADDR if set, otherwise compose from HOST/PORT with defaults.
    if let Ok(addr) = env::var("BIND_ADDR") {
        return Ok(addr.parse()?);
    }

    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);
    Ok(format!("{host}:{port}").parse()?)
}

async fn shutdown_signal() {
    // Ctrl+C
    if let Err(e) = signal::ctrl_c().await {
        error!(?e, "failed to install Ctrl+C handler");
    }
    info!("shutdown signal received");
}

#[derive(Serialize)]
struct VersionResponse {
    version: &'static str,
}

async fn version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}
