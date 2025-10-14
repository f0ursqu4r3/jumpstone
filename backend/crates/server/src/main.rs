mod config;
mod messaging;
#[cfg(feature = "metrics")]
mod metrics;
mod session;
mod users;

const REQUEST_ID_HEADER: &str = "x-request-id";
const CONTENT_SECURITY_POLICY: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";
const REFERRER_POLICY: &str = "no-referrer";
const X_CONTENT_TYPE_OPTIONS: &str = "nosniff";
const X_FRAME_OPTIONS: &str = "DENY";

#[cfg(feature = "metrics")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use axum::{
    body::HttpBody,
    extract::{MatchedPath, State},
    http::{header::HeaderName, HeaderValue},
    routing::{get, post},
    Json, Router,
};
#[cfg(feature = "metrics")]
use axum::{
    http::{header::CONTENT_TYPE, StatusCode},
    response::IntoResponse,
};
use clap::{ArgAction, Args, Parser, Subcommand};
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
    propagate_header::PropagateHeaderLayer,
    request_id::{MakeRequestUuid, RequestId, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::field::{Field, Visit};
use tracing::{error, info, Event, Subscriber};
use tracing_subscriber::fmt::{
    format::Format as FmtFormat, format::Writer as FmtWriter, writer::MakeWriter, FmtContext,
    FormatEvent, FormatFields,
};
use tracing_subscriber::layer::{Context as LayerContext, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

#[cfg(test)]
use once_cell::sync::Lazy;
#[cfg(test)]
use std::sync::Mutex;

use openguild_storage::{connect, CreateUserError, StoragePool, UserRepository};
use session::{
    DatabaseSessionAuthenticator, InMemorySessionStore, PostgresSessionRepository, SessionContext,
    SessionSigner,
};

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
struct Cli {
    #[command(flatten)]
    config: ConfigArgs,
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Args, Debug, Default, Clone)]
struct ConfigArgs {
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
    #[arg(long = "session-fallback-verifying-key", action = ArgAction::Append)]
    session_fallback_verifying_key: Vec<String>,
}

impl ConfigArgs {
    fn into_overrides(self) -> CliOverrides {
        let fallback_keys = if self.session_fallback_verifying_key.is_empty() {
            None
        } else {
            Some(self.session_fallback_verifying_key)
        };
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
            session_fallback_verifying_keys: fallback_keys,
        }
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Seed a user account into the configured database.
    SeedUser(SeedUserCommand),
}

#[derive(Args, Debug)]
struct SeedUserCommand {
    /// Username for the seeded account.
    #[arg(long)]
    username: String,
    /// Plaintext password for the seeded account.
    #[arg(long)]
    password: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let overrides = cli.config.clone().into_overrides();
    let mut config = ServerConfig::load()?;
    config.apply_overrides(&overrides)?;

    if let Some(command) = cli.command {
        return run_command(&config, command).await;
    }

    let config = Arc::new(config);
    run(config).await
}

async fn run_command(config: &ServerConfig, command: CliCommand) -> Result<()> {
    match command {
        CliCommand::SeedUser(cmd) => seed_user(config, cmd).await,
    }
}

async fn seed_user(config: &ServerConfig, cmd: SeedUserCommand) -> Result<()> {
    let database_url = config
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("database_url must be configured to seed users"))?;

    let pool = connect(database_url).await?;
    match UserRepository::create_user(pool.pool(), &cmd.username, &cmd.password).await {
        Ok(user_id) => {
            println!("Seeded user '{}' with id {}", cmd.username, user_id);
            Ok(())
        }
        Err(CreateUserError::UsernameTaken) => {
            println!("User '{}' already exists; skipping", cmd.username);
            Ok(())
        }
        Err(CreateUserError::Other(err)) => Err(err),
    }
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
    match (
        config.session.active_signing_key.is_some(),
        config.session.fallback_verifying_keys.is_empty(),
    ) {
        (false, _) => {
            info!(
                verifying_key = %session_signer.verifying_key_base64(),
                "no session signing key supplied; generated ephemeral key"
            );
        }
        (true, false) => {
            info!(
                active_verifying_key = %session_signer.verifying_key_base64(),
                fallback_keys = %config.session.fallback_verifying_keys.len(),
                "session signing key configured with rotation fallbacks"
            );
        }
        (true, true) => {
            info!(
                verifying_key = %session_signer.verifying_key_base64(),
                "session signing key loaded from configuration"
            );
        }
    }
    let (authenticator, repository): (
        Arc<dyn session::SessionAuthenticator>,
        Arc<dyn session::SessionRepository>,
    ) = match storage.pool() {
        Some(pool) => (
            Arc::new(DatabaseSessionAuthenticator::new(pool.clone())),
            Arc::new(PostgresSessionRepository::new(pool)),
        ),
        None => {
            let store = Arc::new(InMemorySessionStore::new());
            let auth: Arc<dyn session::SessionAuthenticator> = store.clone();
            let repo: Arc<dyn session::SessionRepository> = store.clone();
            (auth, repo)
        }
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

    fn storage_pool(&self) -> Option<StoragePool> {
        self.storage.pool()
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

    fn record_messaging_rejection(&self, reason: &str) {
        #[cfg(feature = "metrics")]
        if let Some(metrics) = &self.metrics {
            metrics.increment_messaging_rejection(reason);
        }
        #[cfg(not(feature = "metrics"))]
        {
            let _ = (self, reason);
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
        .route("/users/register", post(users::register))
        .route("/sessions/login", post(session::login))
        .route("/sessions/refresh", post(session::refresh))
        .route("/sessions/revoke", post(session::revoke))
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

    let request_id_header = HeaderName::from_static(REQUEST_ID_HEADER);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(HttpSpanMaker::default())
        .on_response(HttpOnResponse::new());

    let builder = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static(REFERRER_POLICY),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static(X_CONTENT_TYPE_OPTIONS),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static(X_FRAME_OPTIONS),
        ))
        .layer(PropagateHeaderLayer::new(request_id_header.clone()))
        .layer(trace_layer)
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid));

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
            .map(|value| value.to_owned())
            .unwrap_or_else(|| "unknown".to_string());

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
        let request_id = response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown");

        span.record("status_code", &tracing::field::display(status));
        span.record("latency_ms", &tracing::field::display(latency_ms));

        tracing::debug!(
            parent: span,
            request_id = %request_id,
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
    W: for<'a> MakeWriter<'a> + Send + Sync + Clone + 'static,
{
    build_subscriber_inner(json, env_filter, writer)
}

fn build_subscriber(
    json: bool,
    env_filter: EnvFilter,
) -> Box<dyn tracing::Subscriber + Send + Sync> {
    build_subscriber_inner(json, env_filter, || std::io::stderr())
}

#[derive(Default)]
struct RequestIdStorageLayer;

#[derive(Clone)]
struct RequestIdExtension(String);

impl RequestIdExtension {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Default)]
struct RequestIdVisitor {
    request_id: Option<String>,
}

impl Visit for RequestIdVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "request_id" {
            self.request_id = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "request_id" && self.request_id.is_none() {
            self.request_id = Some(format!("{value:?}"));
        }
    }
}

impl<S> Layer<S> for RequestIdStorageLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: LayerContext<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            let mut visitor = RequestIdVisitor::default();
            attrs.record(&mut visitor);
            if let Some(mut request_id) = visitor.request_id {
                if request_id.starts_with('"') && request_id.ends_with('"') && request_id.len() >= 2
                {
                    request_id = request_id.trim_matches('"').to_string();
                }
                span.extensions_mut().insert(RequestIdExtension(request_id));
            }
        }
    }
}

struct RequestIdEventFormat<E> {
    inner: E,
}

impl<E> RequestIdEventFormat<E> {
    fn new(inner: E) -> Self {
        Self { inner }
    }
}

impl<S, N, E> FormatEvent<S, N> for RequestIdEventFormat<E>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: FmtWriter<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        if let Some(span) = ctx.lookup_current() {
            if let Some(request_id) = span.extensions().get::<RequestIdExtension>() {
                write!(writer, "[request_id={}] ", request_id.as_str())?;
            }
        }

        self.inner.format_event(ctx, writer, event)
    }
}

fn build_subscriber_inner<W>(
    json: bool,
    env_filter: EnvFilter,
    make_writer: W,
) -> Box<dyn tracing::Subscriber + Send + Sync>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + Clone + 'static,
{
    if json {
        let format = FmtFormat::default()
            .with_target(true)
            .with_level(true)
            .json();

        Box::new(
            tracing_subscriber::registry()
                .with(env_filter)
                .with(RequestIdStorageLayer::default())
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .event_format(RequestIdEventFormat::new(format))
                        .with_writer(make_writer),
                ),
        )
    } else {
        let format = FmtFormat::default().with_target(true).with_level(true);

        Box::new(
            tracing_subscriber::registry()
                .with(env_filter)
                .with(RequestIdStorageLayer::default())
                .with(
                    tracing_subscriber::fmt::layer()
                        .event_format(RequestIdEventFormat::new(format))
                        .with_writer(make_writer),
                ),
        )
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
    use axum::http::{HeaderValue, Request, StatusCode};
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

    const TEST_USER_IDENTIFIER: &str = "tester@example.org";
    const TEST_USER_SECRET: &str = "test-secret";
    const TEST_DEVICE_ID: &str = "test-device";

    async fn session_with_logged_in_user() -> (session::tests::SessionTestHarness, String, Uuid) {
        let harness = session::tests::empty_session_context();
        let user_id = Uuid::new_v4();
        harness
            .register_user(TEST_USER_IDENTIFIER, TEST_USER_SECRET, user_id)
            .await;
        let attempt = session::LoginAttempt {
            identifier: TEST_USER_IDENTIFIER.to_string(),
            secret: TEST_USER_SECRET.to_string(),
            device: session::DeviceContext {
                device_id: TEST_DEVICE_ID.to_string(),
                device_name: Some("Test Device".to_string()),
                user_agent: Some("server-tests".to_string()),
                ip_address: None,
            },
        };
        let login = harness
            .context
            .login(attempt)
            .await
            .expect("login succeeds")
            .expect("login response");
        (harness, format!("Bearer {}", login.access_token), user_id)
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
        {
            let headers = response.headers();
            assert_eq!(
                headers
                    .get("content-security-policy")
                    .and_then(|value| value.to_str().ok()),
                Some(CONTENT_SECURITY_POLICY)
            );
            assert_eq!(
                headers
                    .get("referrer-policy")
                    .and_then(|value| value.to_str().ok()),
                Some(REFERRER_POLICY)
            );
            assert_eq!(
                headers
                    .get("x-content-type-options")
                    .and_then(|value| value.to_str().ok()),
                Some(X_CONTENT_TYPE_OPTIONS)
            );
            assert_eq!(
                headers
                    .get("x-frame-options")
                    .and_then(|value| value.to_str().ok()),
                Some(X_FRAME_OPTIONS)
            );
        }
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
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"identifier":"","secret":" ","device":{"device_id":"cli"}}"#,
                    ))
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

        let response = app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"identifier":"alice@example.org","secret":"supersecret","device":{"device_id":"cli","device_name":"CLI"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: session::LoginResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.access_token.is_empty());
        assert!(payload.access_expires_at > Utc::now());
        assert!(!payload.refresh_token.is_empty());
        assert!(payload.refresh_expires_at > Utc::now());
        assert_eq!(harness.store.session_count().await, 1);

        // ensure session contains expected user (decode payload portion)
        let parts: Vec<&str> = payload.access_token.split('.').collect();
        assert_eq!(parts.len(), 2);
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[0])
            .expect("decode payload");
        let claims: serde_json::Value = serde_json::from_slice(&decoded).expect("claims json");
        assert_eq!(claims["user_id"].as_str().unwrap(), user_id.to_string());

        let refresh_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/refresh")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"refresh_token":"{}"}}"#,
                        payload.refresh_token
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refresh_response.status(), StatusCode::OK);
        let body = to_bytes(refresh_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let refreshed: session::LoginResponse = serde_json::from_slice(&body).unwrap();
        assert_ne!(refreshed.refresh_token, payload.refresh_token);

        let revoke_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/revoke")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"refresh_token":"{}"}}"#,
                        refreshed.refresh_token
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(revoke_response.status(), StatusCode::NO_CONTENT);

        let reuse_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sessions/refresh")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"refresh_token":"{}"}}"#,
                        refreshed.refresh_token
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reuse_response.status(), StatusCode::UNAUTHORIZED);
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
                        r#"{"identifier":"unknown@example.org","secret":"nope","device":{"device_id":"cli"}}"#,
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
        let (session_harness, auth_header, user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);
        let authorization = auth_header.as_str();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/guilds")
                    .header("content-type", "application/json")
                    .header("authorization", authorization)
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
                    .header("authorization", authorization)
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
                    .header("authorization", authorization)
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
                    .header("authorization", authorization)
                    .body(Body::from(format!(
                        "{{\"sender\":\"{}\",\"content\":\"hello world\"}}",
                        user_id
                    )))
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
    async fn create_guild_requires_bearer_token() {
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
                    .method("POST")
                    .uri("/guilds")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Auth Required"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_guilds_requires_bearer_token() {
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
                    .method("GET")
                    .uri("/guilds")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_channel_requires_bearer_token() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Authz Guild")
            .await
            .expect("guild creation");
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/guilds/{}/channels", guild.guild_id))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"general"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn post_message_requires_bearer_token() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Authz Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/channels/{}/messages", channel.channel_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"sender":"00000000-0000-0000-0000-000000000000","content":"hello"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_channel_rejects_long_name() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Limit Test Guild")
            .await
            .expect("guild creation");
        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let long_name = "a".repeat(65);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/guilds/{}/channels", guild.guild_id))
                    .header("content-type", "application/json")
                    .header("authorization", auth_header.as_str())
                    .body(Body::from(format!("{{\"name\":\"{}\"}}", long_name)))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_message_rejects_sender_mismatch() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Mismatch Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let other_user = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/channels/{}/messages", channel.channel_id))
                    .header("content-type", "application/json")
                    .header("authorization", auth_header.as_str())
                    .body(Body::from(format!(
                        "{{\"sender\":\"{}\",\"content\":\"hello\"}}",
                        other_user
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn post_message_rejects_oversized_content() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Oversize Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let (session_harness, auth_header, user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let long_content = "a".repeat(4_001);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/channels/{}/messages", channel.channel_id))
                    .header("content-type", "application/json")
                    .header("authorization", auth_header.as_str())
                    .body(Body::from(format!(
                        "{{\"sender\":\"{}\",\"content\":\"{}\"}}",
                        user_id, long_content
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_message_hits_rate_limit() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Rate Limit Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let (session_harness, auth_header, user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);
        let authorization = auth_header.as_str();
        let uri = format!("/channels/{}/messages", channel.channel_id);

        for _ in 0..3 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(&uri)
                        .header("content-type", "application/json")
                        .header("authorization", authorization)
                        .body(Body::from(format!(
                            "{{\"sender\":\"{}\",\"content\":\"hello\"}}",
                            user_id
                        )))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&uri)
                    .header("content-type", "application/json")
                    .header("authorization", authorization)
                    .body(Body::from(format!(
                        "{{\"sender\":\"{}\",\"content\":\"rate limited\"}}",
                        user_id
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn websocket_requires_bearer_token() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Unauthorized Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("unauthorized websocket test server error");
        });

        let url = format!("ws://{}/channels/{}/ws", addr, channel.channel_id);
        match connect_async(url).await {
            Ok(_) => panic!("handshake unexpectedly succeeded without authorization"),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            }
            Err(err) => panic!("unexpected websocket error: {err:?}"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn create_channel_returns_not_found_when_guild_missing() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);
        let authorization = auth_header.as_str();

        let bogus_guild = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/guilds/{bogus_guild}/channels"))
                    .header("content-type", "application/json")
                    .header("authorization", authorization)
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
        let (session_harness, auth_header, user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);
        let authorization = auth_header.as_str();

        let channel_id = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/channels/{channel_id}/messages"))
                    .header("content-type", "application/json")
                    .header("authorization", authorization)
                    .body(Body::from(format!(
                        "{{\"sender\":\"{}\",\"content\":\"hello world\"}}",
                        user_id
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn channel_socket_returns_not_found_for_unknown_channel() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("websocket test server error");
        });

        let url = format!("ws://{}/channels/{}/ws", addr, Uuid::new_v4());
        let mut request = url.into_client_request().unwrap();
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&auth_header).expect("authorization header"),
        );
        match connect_async(request).await {
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

        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_session(session_harness.context.clone());

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
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&auth_header).expect("authorization header"),
        );

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
    async fn websocket_rejects_when_capacity_reached() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let config = test_config();
        let mut messaging = messaging::MessagingService::new_in_memory(config.server_name.clone());
        messaging.set_max_websocket_connections(1);
        let messaging = Arc::new(messaging);

        let guild = messaging.create_guild("Capacity Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .unwrap();

        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("websocket capacity test server error");
        });

        let url = format!("ws://{}/channels/{}/ws", addr, channel.channel_id);
        let mut first = url.clone().into_client_request().unwrap();
        first.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&auth_header).expect("authorization header"),
        );
        let (first_socket, _) = connect_async(first).await.expect("first connection");

        let mut second = url.into_client_request().unwrap();
        second.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&auth_header).expect("authorization header"),
        );

        match connect_async(second).await {
            Ok(_) => panic!("second websocket connection should be rate limited"),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            }
            Err(err) => panic!("unexpected websocket error: {err:?}"),
        }

        drop(first_socket);
        server.abort();
    }

    #[tokio::test]
    async fn websocket_broadcasts_events() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging.create_guild("Web Socket Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .unwrap();

        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("websocket test server error");
        });

        let url = format!("ws://{}/channels/{}/ws", addr, channel.channel_id);
        let mut request = url.into_client_request().unwrap();
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&auth_header).expect("authorization header"),
        );
        let (mut socket, _) = connect_async(request).await.unwrap();

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
        let active_signer = openguild_crypto::generate_signing_key();
        let signing_base64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(active_signer.to_bytes());
        let fallback_signer = openguild_crypto::generate_signing_key();
        let fallback_verifier = openguild_crypto::verifying_key_from(&fallback_signer);
        let fallback_base64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(fallback_verifier.to_bytes());

        let cli = Cli::parse_from(vec![
            "openguild-server".into(),
            "--bind-addr".into(),
            "127.0.0.1:5000".into(),
            "--host".into(),
            "127.0.0.1".into(),
            "--server-name".into(),
            "app.openguild.test".into(),
            "--port".into(),
            "5000".into(),
            "--log-format".into(),
            "json".into(),
            "--metrics-enabled".into(),
            "true".into(),
            "--metrics-bind-addr".into(),
            "127.0.0.1:9100".into(),
            "--database-url".into(),
            "postgres://app:secret@localhost/app".into(),
            format!("--session-signing-key={signing_base64}"),
            format!("--session-fallback-verifying-key={fallback_base64}"),
        ]);

        let overrides = cli.config.into_overrides();
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
            config.session.active_signing_key.as_deref(),
            Some(signing_base64.as_str())
        );
        assert_eq!(
            config.session.fallback_verifying_keys,
            vec![fallback_base64]
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
