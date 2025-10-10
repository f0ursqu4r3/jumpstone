use anyhow::Result;
use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::{env, net::SocketAddr, time::Instant};
use tokio::{net::TcpListener, signal};
use tracing::{error, info, Level};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let state = AppState::default();
    let app = build_app(state);

    let addr: SocketAddr = bind_addr()?;
    let listener = TcpListener::bind(addr).await?;
    info!("listening on {addr}");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    started_at: Instant,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl AppState {
    fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn readiness(State(state): State<AppState>) -> Json<ReadinessResponse> {
    let components = vec![ComponentStatus {
        name: "database",
        status: "pending",
        details: Some("storage layer not yet wired"),
    }];

    Json(ReadinessResponse {
        status: "degraded",
        uptime_seconds: state.uptime_seconds(),
        components,
    })
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

fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/version", get(version))
        .with_state(state)
}

#[derive(Serialize)]
struct ReadinessResponse {
    status: &'static str,
    uptime_seconds: u64,
    components: Vec<ComponentStatus>,
}

#[derive(Serialize)]
struct ComponentStatus {
    name: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serial_test::serial;
    use std::net::{IpAddr, Ipv4Addr};
    use std::str;
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn health_route_returns_ok() {
        let app = build_app(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let message = str::from_utf8(&body).unwrap();
        assert_eq!(message, "ok");
    }

    #[tokio::test]
    async fn version_route_reports_package_version() {
        let app = build_app(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["version"].as_str().unwrap(),
            env!("CARGO_PKG_VERSION")
        );
    }

    #[tokio::test]
    async fn readiness_route_reports_degraded_until_dependencies_exist() {
        let app_state = AppState::default();
        let app = build_app(app_state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["status"], "degraded");
        let uptime = payload["uptime_seconds"].as_u64().unwrap();
        assert!(uptime <= 1);

        let components = payload["components"].as_array().unwrap();
        assert_eq!(components.len(), 1);
        let component = &components[0];
        assert_eq!(component["name"], "database");
        assert_eq!(component["status"], "pending");
        assert_eq!(component["details"], "storage layer not yet wired");
    }

    #[test]
    #[serial]
    fn bind_addr_prefers_bind_addr_var() {
        env::set_var("BIND_ADDR", "127.0.0.1:4000");
        env::remove_var("HOST");
        env::remove_var("PORT");

        let addr = bind_addr().unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(addr.port(), 4000);

        env::remove_var("BIND_ADDR");
    }

    #[test]
    #[serial]
    fn bind_addr_composes_from_host_and_port() {
        env::remove_var("BIND_ADDR");
        env::set_var("HOST", "192.168.1.10");
        env::set_var("PORT", "9000");

        let addr = bind_addr().unwrap();
        assert_eq!(addr.to_string(), "192.168.1.10:9000");

        env::remove_var("HOST");
        env::remove_var("PORT");
    }

    #[test]
    #[serial]
    fn bind_addr_falls_back_to_defaults_on_invalid_values() {
        env::remove_var("BIND_ADDR");
        env::remove_var("HOST");
        env::set_var("PORT", "not_a_number");

        let addr = bind_addr().unwrap();
        assert_eq!(addr.to_string(), "0.0.0.0:8080");

        env::remove_var("PORT");
    }

    #[test]
    fn app_state_reports_uptime_in_seconds() {
        let state = AppState::default();
        assert_eq!(state.uptime_seconds(), 0);
    }
}
