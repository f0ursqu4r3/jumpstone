mod config;
#[cfg(feature = "metrics")]
mod metrics;

use anyhow::Result;
use axum::{extract::State, routing::get, Json, Router};
#[cfg(feature = "metrics")]
use axum::{
    http::{header::CONTENT_TYPE, StatusCode},
    response::IntoResponse,
};
use clap::Parser;
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

use openguild_storage::connect;

use crate::config::{CliOverrides, LogFormat, ServerConfig};
#[cfg(feature = "metrics")]
use crate::metrics::MetricsContext;

#[derive(Clone)]
struct StorageState {
    status: StorageStatus,
    pool: Option<Arc<openguild_storage::PgPool>>,
}

#[derive(Clone)]
enum StorageStatus {
    Unconfigured,
    Connected,
    Error(String),
}

impl StorageState {
    fn unconfigured() -> Self {
        Self {
            status: StorageStatus::Unconfigured,
            pool: None,
        }
    }

    fn connected() -> Self {
        Self {
            status: StorageStatus::Connected,
            pool: None,
        }
    }

    fn connected_with_pool(pool: openguild_storage::PgPool) -> Self {
        Self {
            status: StorageStatus::Connected,
            pool: Some(Arc::new(pool)),
        }
    }

    fn error(message: String) -> Self {
        Self {
            status: StorageStatus::Error(message),
            pool: None,
        }
    }

    fn component(&self) -> ComponentStatus {
        match &self.status {
            StorageStatus::Unconfigured => ComponentStatus {
                name: "database",
                status: "pending",
                details: Some("database_url not configured".to_string()),
            },
            StorageStatus::Connected => ComponentStatus {
                name: "database",
                status: "configured",
                details: Some("connection established".to_string()),
            },
            StorageStatus::Error(message) => ComponentStatus {
                name: "database",
                status: "error",
                details: Some(message.clone()),
            },
        }
    }

    fn readiness_status(&self) -> &'static str {
        match self.status {
            StorageStatus::Connected => "ready",
            StorageStatus::Unconfigured | StorageStatus::Error(_) => "degraded",
        }
    }

    #[cfg(feature = "metrics")]
    fn is_ready(&self) -> bool {
        matches!(self.status, StorageStatus::Connected)
    }

    #[allow(dead_code)]
    fn pool(&self) -> Option<Arc<openguild_storage::PgPool>> {
        self.pool.clone()
    }
}

#[derive(Parser, Debug, Default)]
#[command(
    name = "openguild-server",
    version,
    about = "OpenGuild homeserver gateway"
)]
struct CliOptions {
    #[arg(long)]
    bind_addr: Option<String>,
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    log_format: Option<LogFormat>,
    #[arg(long)]
    metrics_enabled: Option<bool>,
    #[arg(long)]
    metrics_bind_addr: Option<String>,
    #[arg(long)]
    database_url: Option<String>,
}

impl CliOptions {
    fn into_overrides(self) -> CliOverrides {
        CliOverrides {
            bind_addr: self.bind_addr,
            host: self.host,
            port: self.port,
            log_format: self.log_format,
            metrics_enabled: self.metrics_enabled,
            metrics_bind_addr: self.metrics_bind_addr,
            database_url: self.database_url,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = CliOptions::parse();
    let overrides = cli.into_overrides();
    let mut config = ServerConfig::load()?;
    config.apply_overrides(&overrides)?;
    let config = Arc::new(config);
    run(config).await
}

async fn run(config: Arc<ServerConfig>) -> Result<()> {
    init_tracing(&config);

    let storage = match config.database_url.as_deref() {
        Some(url) => match connect(url).await {
            Ok(pool) => {
                info!("database connection established");
                StorageState::connected_with_pool(pool)
            }
            Err(err) => {
                error!(?err, "failed to establish database connection");
                StorageState::error(err.to_string())
            }
        },
        None => StorageState::unconfigured(),
    };

    #[cfg(feature = "metrics")]
    let state = {
        let metrics_ctx = if config.metrics.enabled {
            Some(MetricsContext::init()?)
        } else {
            None
        };
        AppState::new(config.clone(), storage).with_metrics(metrics_ctx)
    };

    #[cfg(not(feature = "metrics"))]
    let state = AppState::new(config.clone(), storage);
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
    storage: StorageState,
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<MetricsContext>>,
}

impl AppState {
    fn new(config: Arc<ServerConfig>, storage: StorageState) -> Self {
        Self {
            started_at: Instant::now(),
            config,
            storage,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    #[cfg(test)]
    fn with_start_time(
        config: Arc<ServerConfig>,
        storage: StorageState,
        started_at: Instant,
    ) -> Self {
        Self {
            started_at,
            config,
            storage,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    #[cfg(feature = "metrics")]
    fn with_metrics(mut self, metrics: Option<Arc<MetricsContext>>) -> Self {
        self.metrics = metrics;
        self
    }

    fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    #[cfg(feature = "metrics")]
    fn metrics_enabled(&self) -> bool {
        self.config.metrics.enabled
    }

    #[cfg(feature = "metrics")]
    fn metrics(&self) -> Option<Arc<MetricsContext>> {
        self.metrics.clone()
    }

    #[cfg(feature = "metrics")]
    fn record_http_request(&self, route: &str, status: u16) {
        if let Some(metrics) = &self.metrics {
            let status_str = status.to_string();
            metrics
                .http_requests_total
                .with_label_values(&[route, status_str.as_str()])
                .inc();
        }
    }

    fn database_component(&self) -> ComponentStatus {
        self.storage.component()
    }

    #[cfg(feature = "metrics")]
    fn record_db_ready(&self, ready: bool) {
        if let Some(metrics) = &self.metrics {
            metrics.set_db_ready(ready);
        }
    }
}

async fn health(State(state): State<AppState>) -> &'static str {
    #[cfg(feature = "metrics")]
    state.record_http_request("health", axum::http::StatusCode::OK.as_u16());
    #[cfg(not(feature = "metrics"))]
    let _ = state;
    "ok"
}

async fn readiness(State(state): State<AppState>) -> Json<ReadinessResponse> {
    let components = vec![state.database_component()];
    let status = state.storage.readiness_status();

    #[cfg(feature = "metrics")]
    state.record_db_ready(state.storage.is_ready());

    #[cfg(feature = "metrics")]
    state.record_http_request("ready", axum::http::StatusCode::OK.as_u16());

    Json(ReadinessResponse {
        status,
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

async fn version(State(state): State<AppState>) -> Json<VersionResponse> {
    #[cfg(feature = "metrics")]
    state.record_http_request("version", axum::http::StatusCode::OK.as_u16());
    #[cfg(not(feature = "metrics"))]
    let _ = state;

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
    details: Option<String>,
}

#[cfg(feature = "metrics")]
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    if !state.metrics_enabled() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let Some(metrics) = state.metrics() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    match metrics.encode() {
        Ok(body) => (
            StatusCode::OK,
            [(CONTENT_TYPE, "text/plain; version=0.0.4")],
            body,
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, "failed to encode metrics");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "metrics")]
    use crate::metrics::MetricsContext;
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

    fn storage_unconfigured() -> StorageState {
        StorageState::unconfigured()
    }

    fn storage_connected() -> StorageState {
        StorageState::connected()
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
        let app = build_app(AppState::new(test_config(), storage_unconfigured()));
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
        let app = build_app(AppState::new(test_config(), storage_unconfigured()));
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
        let app_state = AppState::new(test_config(), storage_unconfigured());
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
        assert_eq!(component["details"], "database_url not configured");
    }

    #[tokio::test]
    async fn readiness_reports_elapsed_uptime() {
        let past = Instant::now() - Duration::from_secs(2);
        let app_state = AppState::with_start_time(test_config(), storage_unconfigured(), past);
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

    #[tokio::test]
    async fn readiness_reports_configured_when_database_url_present() {
        let mut config = ServerConfig::default();
        config.database_url = Some("postgres://app:secret@localhost/app".into());
        let app_state = AppState::new(Arc::new(config), storage_connected());
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
        assert_eq!(payload["status"], "ready");
        let component = &payload["components"].as_array().unwrap()[0];
        assert_eq!(component["status"], "configured");
        assert_eq!(component["details"], "connection established");
    }

    #[test]
    fn app_state_reports_uptime_in_seconds() {
        let state = AppState::new(test_config(), storage_unconfigured());
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

    #[test]
    fn cli_overrides_convert_and_apply() {
        let cli = CliOptions::parse_from([
            "openguild-server",
            "--bind-addr",
            "127.0.0.1:5000",
            "--host",
            "127.0.0.1",
            "--port",
            "5000",
            "--log-format",
            "json",
            "--metrics-enabled",
            "true",
            "--metrics-bind-addr",
            "127.0.0.1:9100",
            "--database-url",
            "postgres://app:secret@localhost/app",
        ]);

        let overrides = cli.into_overrides();
        let mut config = ServerConfig::default();
        config.apply_overrides(&overrides).expect("overrides apply");

        assert_eq!(config.bind_addr.as_deref(), Some("127.0.0.1:5000"));
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 5000);
        assert_eq!(config.log_format, LogFormat::Json);
        assert!(config.metrics.enabled);
        assert_eq!(config.metrics.bind_addr.as_deref(), Some("127.0.0.1:9100"));
        assert_eq!(
            config.database_url.as_deref(),
            Some("postgres://app:secret@localhost/app")
        );
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_exposed_when_enabled() {
        let metrics_ctx = MetricsContext::init().expect("metrics init");
        let state = AppState::new(metrics_enabled_config(), storage_unconfigured())
            .with_metrics(Some(metrics_ctx));

        // Issue a readiness check to drive counters and DB gauge.
        let ready_app = build_app(state.clone());
        ready_app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let metrics_app = build_app(state);
        let response = metrics_app
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
        assert!(text.contains("openguild_http_requests_total"));
        assert!(text.contains("openguild_db_ready"));
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_absent_when_disabled() {
        let state = AppState::new(test_config(), storage_unconfigured());
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
