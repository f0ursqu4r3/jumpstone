mod config;
mod messaging;
#[cfg(feature = "metrics")]
mod metrics;
mod session;

#[cfg(feature = "metrics")]
use anyhow::Context;
use anyhow::Result;
use axum::{
    body::HttpBody,
    extract::{MatchedPath, State},
    routing::{get, post},
    Json, Router,
};
#[cfg(feature = "metrics")]
use axum::{
    http::{header::CONTENT_TYPE, StatusCode},
    response::IntoResponse,
};
use clap::Parser;
use serde::Serialize;
#[cfg(feature = "metrics")]
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
#[cfg(test)]
use tokio::sync::Notify;
use tokio::{net::TcpListener, signal};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{error, info, Level};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::{fmt, EnvFilter};

#[cfg(test)]
use once_cell::sync::Lazy;
#[cfg(test)]
use std::sync::Mutex;

use openguild_storage::{connect, StoragePool};
use session::{InMemorySessionStore, PostgresSessionRepository, SessionContext, SessionSigner};

use crate::config::{CliOverrides, LogFormat, ServerConfig};
#[cfg(feature = "metrics")]
use crate::metrics::MetricsContext;

#[derive(Clone)]
struct StorageState {
    status: StorageStatus,
    pool: Option<StoragePool>,
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

    fn connected_with_pool(pool: StoragePool) -> Self {
        Self {
            status: StorageStatus::Connected,
            pool: Some(pool),
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
    fn pool(&self) -> Option<StoragePool> {
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
    server_name: Option<String>,
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
    #[arg(long)]
    session_signing_key: Option<String>,
}

impl CliOptions {
    fn into_overrides(self) -> CliOverrides {
        CliOverrides {
            bind_addr: self.bind_addr,
            host: self.host,
            server_name: self.server_name,
            port: self.port,
            log_format: self.log_format,
            metrics_enabled: self.metrics_enabled,
            metrics_bind_addr: self.metrics_bind_addr,
            database_url: self.database_url,
            session_signing_key: self.session_signing_key,
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

    let session_signer = SessionSigner::from_config(&config.session)?;
    if config.session.signing_key.is_none() {
        info!(
            verifying_key = %session_signer.verifying_key_base64(),
            "no session signing key supplied; generated ephemeral key"
        );
    }
    let authenticator = Arc::new(InMemorySessionStore::new());
    let repository: Arc<dyn session::SessionRepository> = match storage.pool() {
        Some(pool) => Arc::new(PostgresSessionRepository::new(pool)),
        None => authenticator.clone(),
    };
    let session_context = Arc::new(SessionContext::new(
        session_signer,
        authenticator.clone(),
        repository,
    ));
    #[cfg(feature = "metrics")]
    let metrics_ctx = if config.metrics.enabled {
        Some(MetricsContext::init()?)
    } else {
        None
    };

    #[cfg(feature = "metrics")]
    let messaging_service = Arc::new(messaging::init_messaging_service(
        &config,
        storage.pool(),
        metrics_ctx.clone(),
    ));

    #[cfg(not(feature = "metrics"))]
    let messaging_service = Arc::new(messaging::init_messaging_service(&config, storage.pool()));

    #[cfg(feature = "metrics")]
    let state = AppState::new(config.clone(), storage.clone(), messaging_service.clone())
        .with_session(session_context.clone())
        .with_metrics(metrics_ctx.clone());

    #[cfg(not(feature = "metrics"))]
    let state = AppState::new(config.clone(), storage, messaging_service.clone())
        .with_session(session_context.clone());

    #[cfg(feature = "metrics")]
    let metrics_state = state.clone();

    let app = build_app(state);

    #[cfg(feature = "metrics")]
    {
        if config.metrics.enabled {
            if let Some(bind_addr) = &config.metrics.bind_addr {
                let metrics_addr: SocketAddr = bind_addr
                    .parse()
                    .context("failed to parse metrics bind addr")?;
                let state = metrics_state;
                tokio::spawn(async move {
                    if let Err(err) = serve_metrics(metrics_addr, state).await {
                        error!(?err, "metrics server terminated unexpectedly");
                    }
                });
            }
        }
    }

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
    #[cfg_attr(not(feature = "metrics"), allow(dead_code))]
    config: Arc<ServerConfig>,
    storage: StorageState,
    messaging: Arc<messaging::MessagingService>,
    session: Option<Arc<SessionContext>>,
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<MetricsContext>>,
}

impl AppState {
    fn new(
        config: Arc<ServerConfig>,
        storage: StorageState,
        messaging: Arc<messaging::MessagingService>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            config,
            storage,
            messaging,
            session: None,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    #[cfg(test)]
    fn with_start_time(
        config: Arc<ServerConfig>,
        storage: StorageState,
        messaging: Arc<messaging::MessagingService>,
        started_at: Instant,
    ) -> Self {
        Self {
            started_at,
            config,
            storage,
            messaging,
            session: None,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    fn with_session(mut self, session: Arc<SessionContext>) -> Self {
        self.session = Some(session);
        self
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

    fn session(&self) -> Arc<SessionContext> {
        self.session
            .as_ref()
            .cloned()
            .expect("session context not configured")
    }

    fn messaging(&self) -> Option<Arc<messaging::MessagingService>> {
        Some(self.messaging.clone())
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

    #[cfg(feature = "metrics")]
    fn metrics_context(&self) -> Option<Arc<MetricsContext>> {
        self.metrics.clone()
    }
}

async fn health(matched_path: MatchedPath, State(state): State<AppState>) -> &'static str {
    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), axum::http::StatusCode::OK.as_u16());
    #[cfg(not(feature = "metrics"))]
    {
        let _ = state;
        let _ = matched_path;
    }
    "ok"
}

async fn readiness(
    matched_path: MatchedPath,
    State(state): State<AppState>,
) -> Json<ReadinessResponse> {
    let components = vec![state.database_component()];
    let status = state.storage.readiness_status();

    #[cfg(feature = "metrics")]
    state.record_db_ready(state.storage.is_ready());

    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), axum::http::StatusCode::OK.as_u16());
    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

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

async fn version(
    matched_path: MatchedPath,
    State(state): State<AppState>,
) -> Json<VersionResponse> {
    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), axum::http::StatusCode::OK.as_u16());
    #[cfg(not(feature = "metrics"))]
    {
        let _ = state;
        let _ = matched_path;
    }

    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}

fn build_app(state: AppState) -> Router {
    #[cfg(feature = "metrics")]
    let metrics_enabled = state.metrics_enabled();
    #[cfg(feature = "metrics")]
    let expose_metrics_here = metrics_enabled && state.config.metrics.bind_addr.is_none();
    #[cfg(feature = "metrics")]
    let metrics_ctx = state.metrics_context();

    #[cfg_attr(not(feature = "metrics"), allow(unused_mut))]
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/version", get(version))
        .route("/sessions/login", post(session::login))
        .route(
            "/guilds",
            get(messaging::list_guilds).post(messaging::create_guild),
        )
        .route(
            "/guilds/:guild_id/channels",
            get(messaging::list_channels).post(messaging::create_channel),
        )
        .route(
            "/channels/:channel_id/messages",
            post(messaging::post_message),
        )
        .route("/channels/:channel_id/ws", get(messaging::channel_socket));

    #[cfg(feature = "metrics")]
    {
        if expose_metrics_here {
            router = router.route("/metrics", get(metrics_handler));
        }
    }

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(HttpSpanMaker::default())
        .on_response(HttpOnResponse::new());

    let builder = ServiceBuilder::new()
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(trace_layer)
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    #[cfg(feature = "metrics")]
    let builder = builder.layer(MetricsRecorderLayer::new(metrics_ctx.clone()));

    let instrumentation_layers = builder.into_inner();

    let router = router.layer(instrumentation_layers);

    router.with_state(state)
}

#[derive(Clone, Default)]
struct HttpSpanMaker;

impl<B> tower_http::trace::MakeSpan<B> for HttpSpanMaker
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
{
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let method = request.method().clone();
        let uri_path = request.uri().path().to_string();
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map(|matched| matched.as_str().to_string())
            .unwrap_or_else(|| uri_path.clone());
        let request_id = request
            .extensions()
            .get::<RequestId>()
            .and_then(|rid| rid.header_value().to_str().ok())
            .unwrap_or("unknown");

        tracing::info_span!(
            "http.request",
            method = %method,
            route = %route,
            request_id = %request_id,
            status_code = tracing::field::Empty,
            latency_ms = tracing::field::Empty
        )
    }
}

#[derive(Clone, Default)]
struct HttpOnResponse;

impl HttpOnResponse {
    fn new() -> Self {
        Self
    }
}

#[cfg(feature = "metrics")]
#[derive(Clone)]
struct MetricsRecorderLayer {
    metrics: Option<Arc<MetricsContext>>,
}

#[cfg(feature = "metrics")]
impl MetricsRecorderLayer {
    fn new(metrics: Option<Arc<MetricsContext>>) -> Self {
        Self { metrics }
    }
}

#[cfg(feature = "metrics")]
impl<S> tower::Layer<S> for MetricsRecorderLayer {
    type Service = MetricsRecorderService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsRecorderService {
            inner,
            metrics: self.metrics.clone(),
        }
    }
}

#[cfg(feature = "metrics")]
#[derive(Clone)]
struct MetricsRecorderService<S> {
    inner: S,
    metrics: Option<Arc<MetricsContext>>,
}

#[cfg(feature = "metrics")]
impl<S, B> tower::Service<axum::http::Request<B>> for MetricsRecorderService<S>
where
    S: tower::Service<axum::http::Request<B>>,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: axum::http::Request<B>) -> Self::Future {
        let metrics = self.metrics.clone();
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map(|matched| matched.as_str().to_string())
            .unwrap_or_else(|| request.uri().path().to_string());
        let start = Instant::now();

        let future = self.inner.call(request);

        Box::pin(async move {
            let response = future.await?;
            if let Some(metrics) = metrics {
                metrics.observe_http_latency(&route, response.status().as_u16(), start.elapsed());
            }
            Ok(response)
        })
    }
}

impl<B> tower_http::trace::OnResponse<B> for HttpOnResponse
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
{
    fn on_response(
        self,
        response: &axum::http::Response<B>,
        latency: Duration,
        span: &tracing::Span,
    ) {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        let status = response.status().as_u16();

        span.record("status_code", &tracing::field::display(status));
        span.record("latency_ms", &tracing::field::display(latency_ms));

        tracing::debug!(
            parent: span,
            status = status,
            latency_ms,
            "request completed"
        );
    }
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

#[cfg(feature = "metrics")]
fn build_metrics_router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

#[cfg(feature = "metrics")]
async fn serve_metrics(bind_addr: SocketAddr, state: AppState) -> Result<()> {
    let router = build_metrics_router(state);
    let listener = TcpListener::bind(bind_addr).await?;
    let addr = listener.local_addr()?;
    info!("metrics listening on {addr}");
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "metrics")]
    use crate::metrics::MetricsContext;
    use crate::{messaging, session};
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use base64::Engine;
    use chrono::Utc;
    use futures::StreamExt;
    use serde_json::Value;
    use serial_test::serial;
    use std::io::Write;
    use std::str;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::time::{sleep, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
    use tower::ServiceExt; // for `oneshot`
    use tracing::info;
    use tracing_subscriber::fmt::writer::MakeWriter;
    use tracing_subscriber::EnvFilter;
    use uuid::Uuid;

    fn test_config() -> Arc<ServerConfig> {
        Arc::new(ServerConfig::default())
    }

    fn storage_unconfigured() -> StorageState {
        StorageState::unconfigured()
    }

    fn storage_connected() -> StorageState {
        StorageState::connected()
    }

    fn default_session_context() -> Arc<SessionContext> {
        session::tests::empty_session_context().context.clone()
    }

    fn app_state_with_default_session(
        config: Arc<ServerConfig>,
        storage: StorageState,
    ) -> AppState {
        let session = session::tests::empty_session_context().context.clone();
        let origin = config.server_name.clone();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(origin));
        AppState::new(config, storage, messaging).with_session(session)
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
        let state = app_state_with_default_session(test_config(), storage_unconfigured());
        let app = build_app(state);
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
    async fn request_id_propagates_into_traces_for_http() {
        use tower::ServiceExt;
        use tower_http::trace::MakeSpan;
        let state = app_state_with_default_session(test_config(), storage_unconfigured());
        let app = build_app(state);

        let request_id = "test-observability".to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("x-request-id", request_id.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok()),
            Some(request_id.as_str())
        );

        use tracing::field::Visit;
        use tracing_subscriber::{
            layer::{Context as LayerContext, SubscriberExt},
            registry::LookupSpan,
            Layer,
        };

        #[derive(Default, Clone)]
        struct RequestIdCapture {
            ids: Arc<Mutex<Vec<String>>>,
        }

        impl<S> Layer<S> for RequestIdCapture
        where
            S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
        {
            fn on_new_span(
                &self,
                attrs: &tracing::span::Attributes<'_>,
                _id: &tracing::span::Id,
                _ctx: LayerContext<'_, S>,
            ) {
                if attrs.metadata().name() != "http.request" {
                    return;
                }
                let mut visitor = RequestIdVisitor::default();
                attrs.record(&mut visitor);
                if let Some(request_id) = visitor.request_id {
                    self.ids.lock().expect("lock").push(request_id);
                }
            }
        }

        #[derive(Default)]
        struct RequestIdVisitor {
            request_id: Option<String>,
        }

        impl Visit for RequestIdVisitor {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "request_id" {
                    self.request_id = Some(value.to_string());
                }
            }

            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "request_id" && self.request_id.is_none() {
                    let rendered = format!("{value:?}");
                    self.request_id = Some(rendered.trim_matches('"').to_string());
                }
            }
        }

        use axum::http::HeaderValue;
        use tower_http::request_id::RequestId;

        let capture = RequestIdCapture::default();
        let subscriber = tracing_subscriber::registry().with(capture.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut span_maker = HttpSpanMaker::default();
        let header_value = HeaderValue::from_str(request_id.as_str()).unwrap();
        let mut span_request = Request::builder()
            .uri("/health")
            .header("x-request-id", header_value.clone())
            .body(Body::empty())
            .unwrap();
        span_request
            .extensions_mut()
            .insert(RequestId::new(header_value));
        let span = span_maker.make_span(&span_request);
        drop(span);

        let captured = capture.ids.lock().expect("lock");
        assert!(
            captured.iter().any(|value| value == request_id.as_str()),
            "span did not capture request id"
        );
    }

    #[tokio::test]
    async fn version_route_reports_package_version() {
        let state = app_state_with_default_session(test_config(), storage_unconfigured());
        let app = build_app(state);
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
        let app_state = app_state_with_default_session(test_config(), storage_unconfigured());
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
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let app_state = AppState::with_start_time(config, storage_unconfigured(), messaging, past)
            .with_session(default_session_context());
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
        let config = Arc::new(config);
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let app_state = AppState::new(config, storage_connected(), messaging)
            .with_session(default_session_context());
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
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
        assert_eq!(state.uptime_seconds(), 0);
    }

    #[tokio::test]
    async fn login_route_rejects_blank_inputs() {
        let state = app_state_with_default_session(test_config(), storage_unconfigured());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/login")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"identifier":"","secret":" "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["error"], "validation_error");
        let details = payload["details"].as_array().unwrap();
        assert_eq!(details.len(), 2);
    }

    #[tokio::test]
    async fn login_route_returns_token_on_success() {
        let (harness, user_id) =
            session::tests::session_context_with_user("alice@example.org", "supersecret").await;
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(harness.context.clone());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"identifier":"alice@example.org","secret":"supersecret"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: session::LoginResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.token.is_empty());
        assert!(payload.expires_at > Utc::now());
        assert_eq!(harness.store.session_count().await, 1);

        // ensure session contains expected user (decode payload portion)
        let parts: Vec<&str> = payload.token.split('.').collect();
        assert_eq!(parts.len(), 2);
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[0])
            .expect("decode payload");
        let claims: serde_json::Value = serde_json::from_slice(&decoded).expect("claims json");
        assert_eq!(claims["user_id"].as_str().unwrap(), user_id.to_string());
    }

    #[tokio::test]
    async fn login_route_returns_unauthorized_on_invalid_credentials() {
        let harness = session::tests::empty_session_context();
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(harness.context.clone());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"identifier":"unknown@example.org","secret":"nope"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["error"], "invalid_credentials");
    }

    #[tokio::test]
    async fn guild_channel_crud_endpoints() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/guilds")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Test Guild"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let guild: Value = serde_json::from_slice(&body).unwrap();
        let guild_id = guild["guild_id"].as_str().unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/guilds")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let guilds: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(guilds.as_array().unwrap().len(), 1);

        let create_channel_uri = format!("/guilds/{guild_id}/channels");
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&create_channel_uri)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"general"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let channel: Value = serde_json::from_slice(&body).unwrap();
        let channel_id = channel["channel_id"].as_str().unwrap();

        let message_uri = format!("/channels/{channel_id}/messages");
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&message_uri)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"sender":"@user:example.org","content":"hello world"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["sequence"], 1);
    }

    #[tokio::test]
    async fn create_channel_returns_not_found_when_guild_missing() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let bogus_guild = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/guilds/{bogus_guild}/channels"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"general"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn post_message_returns_not_found_for_unknown_channel() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let channel_id = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/channels/{channel_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"sender":"@user:example.org","content":"hello world"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn channel_socket_returns_not_found_for_unknown_channel() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("websocket test server error");
        });

        let url = format!("ws://{}/channels/{}/ws", addr, Uuid::new_v4());
        match connect_async(url).await {
            Ok(_) => panic!("handshake unexpectedly succeeded"),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::NOT_FOUND);
            }
            Err(err) => panic!("unexpected websocket error: {err:?}"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn websocket_connection_limit_logs_request_id() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let config = test_config();
        let mut messaging = messaging::MessagingService::new_in_memory(config.server_name.clone());
        messaging.set_max_websocket_connections(0);
        let messaging = Arc::new(messaging);

        let guild = messaging.create_guild("Limit Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .unwrap();

        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_session(default_session_context());

        let writer = CaptureWriter::default();
        let subscriber =
            build_subscriber_with_writer(true, EnvFilter::new("debug"), writer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("websocket limit test server error");
        });

        let request_id = "ws-test-id";
        let url = format!("ws://{}/channels/{}/ws", addr, channel.channel_id);
        let mut request = url.into_client_request().unwrap();
        request
            .headers_mut()
            .insert("x-request-id", request_id.parse().unwrap());

        match connect_async(request).await {
            Ok(_) => panic!("handshake unexpectedly succeeded"),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            }
            Err(err) => panic!("unexpected websocket error: {err:?}"),
        }

        server.abort();

        let logs = writer.contents();
        assert!(
            logs.contains(&format!("\"request_id\":\"{request_id}\"")),
            "logs missing request id: {logs}"
        );
    }

    #[tokio::test]
    async fn websocket_broadcasts_events() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging.create_guild("Web Socket Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .unwrap();

        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_session(default_session_context());
        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("websocket test server error");
        });

        let url = format!("ws://{}/channels/{}/ws", addr, channel.channel_id);
        let (mut socket, _) = connect_async(url).await.unwrap();

        messaging
            .append_message(channel.channel_id, "@user:example.org", "hi there")
            .await
            .unwrap();

        let msg = timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("message expected")
            .expect("stream item");

        let text = match msg {
            Ok(WsMessage::Text(text)) => text,
            Ok(other) => panic!("unexpected websocket message {other:?}"),
            Err(err) => panic!("websocket stream error: {err:?}"),
        };
        let payload: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(payload["sequence"].as_i64().unwrap(), 1);
        assert_eq!(
            payload["event"]["content"]["content"].as_str().unwrap(),
            "hi there"
        );

        server.abort();
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
            "--server-name",
            "app.openguild.test",
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
            "--session-signing-key",
            "dGVzdC1zZXNzaW9uLWtleQ",
        ]);

        let overrides = cli.into_overrides();
        let mut config = ServerConfig::default();
        config.apply_overrides(&overrides).expect("overrides apply");

        assert_eq!(config.bind_addr.as_deref(), Some("127.0.0.1:5000"));
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.server_name, "app.openguild.test");
        assert_eq!(config.port, 5000);
        assert_eq!(config.log_format, LogFormat::Json);
        assert!(config.metrics.enabled);
        assert_eq!(config.metrics.bind_addr.as_deref(), Some("127.0.0.1:9100"));
        assert_eq!(
            config.database_url.as_deref(),
            Some("postgres://app:secret@localhost/app")
        );
        assert_eq!(
            config.session.signing_key.as_deref(),
            Some("dGVzdC1zZXNzaW9uLWtleQ")
        );
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_exposed_when_enabled() {
        let metrics_ctx = MetricsContext::init().expect("metrics init");
        let config = metrics_enabled_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context())
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
        assert!(text.contains("openguild_http_request_duration_seconds_bucket"));
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_absent_when_disabled() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
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

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_route_served_on_dedicated_listener_when_configured() {
        let metrics_ctx = MetricsContext::init().expect("metrics init");
        let mut config = ServerConfig::default();
        config.metrics.enabled = true;
        config.metrics.bind_addr = Some("127.0.0.1:0".into());
        let config = Arc::new(config);
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context())
            .with_metrics(Some(metrics_ctx.clone()));

        let main_app = build_app(state.clone());
        let response = main_app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let metrics_app = build_metrics_router(state);
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
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn messaging_metrics_reflect_event_activity() {
        use tower::ServiceExt;

        let metrics_ctx = MetricsContext::init().expect("metrics init");
        let config = metrics_enabled_config();
        let storage = storage_unconfigured();
        let messaging = Arc::new(messaging::init_messaging_service(
            &config,
            storage.pool(),
            Some(metrics_ctx.clone()),
        ));
        let state = AppState::new(config.clone(), storage, messaging.clone())
            .with_session(default_session_context())
            .with_metrics(Some(metrics_ctx));
        let mut app = build_app(state);

        let guild = messaging.create_guild("Metrics Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .unwrap();
        messaging
            .append_message(channel.channel_id, "@user:example.org", "hi there")
            .await
            .unwrap();

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

        let channel_id = channel.channel_id.to_string();
        assert!(text.contains("openguild_messaging_events_total{outcome=\"dropped\"} 1"));
        assert!(text.contains(&format!(
            "openguild_websocket_queue_depth{{channel_id=\"{channel_id}\"}} 0"
        )));
    }
}
