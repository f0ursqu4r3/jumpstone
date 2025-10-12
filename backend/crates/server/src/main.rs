mod config;

use anyhow::Result;
use axum::{extract::State, routing::get, Json, Router};
#[cfg(feature = "metrics")]
use axum::{
    http::{header::CONTENT_TYPE, StatusCode},
    response::IntoResponse,
};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc, time::Instant};
#[cfg(test)]
use tokio::sync::Notify;
use tokio::{net::TcpListener, signal};
use tracing::{error, info, Level};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::{fmt, EnvFilter};

#[cfg(test)]
use once_cell::sync::Lazy;
#[cfg(test)]
use std::sync::Mutex;

use crate::config::{LogFormat, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(ServerConfig::load()?);
    run(config).await
}

async fn run(config: Arc<ServerConfig>) -> Result<()> {
    init_tracing(&config);

    let state = AppState::new(config.clone());
    let app = build_app(state);

    let addr: SocketAddr = config.listener_addr()?;
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
    #[allow(dead_code)]
    config: Arc<ServerConfig>,
}

impl AppState {
    fn new(config: Arc<ServerConfig>) -> Self {
        Self {
            started_at: Instant::now(),
            config,
        }
    }

    #[cfg(test)]
    fn with_start_time(config: Arc<ServerConfig>, started_at: Instant) -> Self {
        Self { started_at, config }
    }

    fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    #[cfg(feature = "metrics")]
    fn metrics_enabled(&self) -> bool {
        self.config.metrics.enabled
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

fn init_tracing(config: &ServerConfig) {
    // Respect RUST_LOG if set, otherwise default to info for our crates.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,openguild_server=info,openguild=info"));

    let json = matches!(config.log_format(), LogFormat::Json);
    let subscriber = build_subscriber(json, env_filter);

    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("failed to install tracing subscriber: {err}");
    }
}

async fn shutdown_signal() {
    #[cfg(test)]
    {
        let notify_opt = TEST_SHUTDOWN_NOTIFY.lock().unwrap().clone();
        if let Some(notify) = notify_opt {
            tokio::select! {
                res = signal::ctrl_c() => {
                    if let Err(e) = res {
                        error!(?e, "failed to install Ctrl+C handler");
                    }
                }
                _ = notify.notified() => {}
            }
            info!("shutdown signal received");
            *TEST_SHUTDOWN_NOTIFY.lock().unwrap() = None;
            return;
        }
    }

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
    #[cfg(feature = "metrics")]
    let metrics_enabled = state.metrics_enabled();

    #[cfg_attr(not(feature = "metrics"), allow(unused_mut))]
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/version", get(version));

    #[cfg(feature = "metrics")]
    {
        if metrics_enabled {
            router = router.route("/metrics", get(metrics_handler));
        }
    }

    router.with_state(state)
}

fn build_subscriber_with_writer<W>(
    json: bool,
    env_filter: EnvFilter,
    writer: W,
) -> Box<dyn tracing::Subscriber + Send + Sync>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let builder = fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_level(true)
        .with_max_level(Level::INFO)
        .with_writer(writer);

    if json {
        Box::new(builder.json().finish())
    } else {
        Box::new(builder.compact().finish())
    }
}

fn build_subscriber(
    json: bool,
    env_filter: EnvFilter,
) -> Box<dyn tracing::Subscriber + Send + Sync> {
    let builder = fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_level(true)
        .with_max_level(Level::INFO);

    if json {
        Box::new(builder.json().finish())
    } else {
        Box::new(builder.compact().finish())
    }
}

#[cfg(test)]
static TEST_SHUTDOWN_NOTIFY: Lazy<Mutex<Option<Arc<Notify>>>> = Lazy::new(|| Mutex::new(None));

#[cfg(test)]
fn install_shutdown_trigger() -> Arc<Notify> {
    let notify = Arc::new(Notify::new());
    *TEST_SHUTDOWN_NOTIFY.lock().unwrap() = Some(notify.clone());
    notify
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

#[cfg(feature = "metrics")]
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    if !state.metrics_enabled() {
        return StatusCode::NOT_FOUND.into_response();
    }

    (
        StatusCode::OK,
        [(CONTENT_TYPE, "text/plain; version=0.0.4")],
        "# HELP openguild_metrics_placeholder gauge placeholder\n# TYPE openguild_metrics_placeholder gauge\nopenguild_metrics_placeholder 1\n",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serial_test::serial;
    use std::io::Write;
    use std::str;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::time::{sleep, timeout};
    use tower::ServiceExt; // for `oneshot`
    use tracing::info;
    use tracing_subscriber::fmt::writer::MakeWriter;
    use tracing_subscriber::EnvFilter;

    fn test_config() -> Arc<ServerConfig> {
        Arc::new(ServerConfig::default())
    }

    #[cfg(feature = "metrics")]
    fn metrics_enabled_config() -> Arc<ServerConfig> {
        let mut config = ServerConfig::default();
        config.metrics.enabled = true;
        Arc::new(config)
    }

    #[derive(Clone, Default)]
    struct CaptureWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl CaptureWriter {
        fn contents(&self) -> String {
            let data = self.buffer.lock().expect("lock");
            String::from_utf8_lossy(&data).to_string()
        }
    }

    struct CaptureHandle {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl<'a> MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureHandle;

        fn make_writer(&'a self) -> Self::Writer {
            CaptureHandle {
                buffer: self.buffer.clone(),
            }
        }
    }

    impl Write for CaptureHandle {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut guard = self.buffer.lock().expect("lock");
            guard.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn health_route_returns_ok() {
        let app = build_app(AppState::new(test_config()));
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
        let app = build_app(AppState::new(test_config()));
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
        let app_state = AppState::new(test_config());
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

    #[tokio::test]
    async fn readiness_reports_elapsed_uptime() {
        let past = Instant::now() - Duration::from_secs(2);
        let app_state = AppState::with_start_time(test_config(), past);
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
        let uptime = payload["uptime_seconds"].as_u64().unwrap();
        assert!(uptime >= 2);
    }

    #[test]
    fn app_state_reports_uptime_in_seconds() {
        let state = AppState::new(test_config());
        assert_eq!(state.uptime_seconds(), 0);
    }

    #[test]
    fn build_subscriber_emits_expected_formats() {
        let json_writer = CaptureWriter::default();
        let json_subscriber =
            build_subscriber_with_writer(true, EnvFilter::new("info"), json_writer.clone());
        tracing::subscriber::with_default(json_subscriber, || {
            info!(message = "json-output");
        });
        let json_output = json_writer.contents();
        assert!(json_output.contains("\"message\":\"json-output\""));

        let compact_writer = CaptureWriter::default();
        let compact_subscriber =
            build_subscriber_with_writer(false, EnvFilter::new("info"), compact_writer.clone());
        tracing::subscriber::with_default(compact_subscriber, || {
            info!("compact-output");
        });
        let compact_output = compact_writer.contents();
        assert!(compact_output.contains("compact-output"));
        assert!(!compact_output.contains("\"compact-output\""));
    }

    #[test]
    #[serial]
    fn init_tracing_tolerates_multiple_invocations() {
        let config = ServerConfig::default();
        init_tracing(&config);
        init_tracing(&config);
    }

    #[test]
    #[serial]
    fn init_tracing_honors_json_format_env() {
        let mut config = ServerConfig::default();
        config.log_format = LogFormat::Json;
        init_tracing(&config);
    }

    #[tokio::test]
    async fn server_shuts_down_when_triggered() {
        let notify = install_shutdown_trigger();
        let mut config = ServerConfig::default();
        config.bind_addr = Some("127.0.0.1:0".into());
        let config = Arc::new(config);

        let handle = tokio::spawn(run(config));

        sleep(Duration::from_millis(50)).await;
        notify.notify_one();

        let join = timeout(Duration::from_secs(2), handle)
            .await
            .expect("server did not shut down in time");
        join.expect("server task panicked")
            .expect("server returned error");
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_exposed_when_enabled() {
        let state = AppState::new(metrics_enabled_config());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = str::from_utf8(&body).unwrap();
        assert!(text.contains("openguild_metrics_placeholder"));
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_absent_when_disabled() {
        let state = AppState::new(test_config());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
