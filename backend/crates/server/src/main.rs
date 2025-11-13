mod config;
mod federation;
mod messaging;
#[cfg(feature = "metrics")]
mod metrics;
mod mls;
mod session;
mod users;

const REQUEST_ID_HEADER: &str = "x-request-id";
const CONTENT_SECURITY_POLICY: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";
const REFERRER_POLICY: &str = "no-referrer";
const X_CONTENT_TYPE_OPTIONS: &str = "nosniff";
const X_FRAME_OPTIONS: &str = "DENY";
const FEDERATION_ORIGIN_HEADER: &str = "x-openguild-origin";

#[cfg(feature = "metrics")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use axum::{
    body::HttpBody,
    extract::{MatchedPath, Path, Query, State},
    http::{header::HeaderName, HeaderMap, HeaderValue},
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
    task::{Context as TaskContext, Poll},
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

use openguild_storage::{
    connect, CreateUserError, MessagingRepository, MlsKeyPackageStore, StoragePool, UserRepository,
};
use session::{
    DatabaseSessionAuthenticator, InMemorySessionStore, PostgresSessionRepository, SessionContext,
    SessionSigner,
};

#[cfg(feature = "metrics")]
use crate::metrics::MetricsContext;
use crate::{
    config::{CliOverrides, LogFormat, ServerConfig},
    messaging::MessagingError,
    mls::{handshake_test_vectors, list_key_packages, rotate_key_package, MlsKeyStore},
};
use uuid::Uuid;

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

    #[allow(dead_code)]
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
    #[arg(long)]
    messaging_max_messages_per_user_per_window: Option<usize>,
    #[arg(long)]
    messaging_max_messages_per_ip_per_window: Option<usize>,
    #[arg(long)]
    messaging_rate_limit_window_secs: Option<u64>,
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
            max_messages_per_user_per_window: self.messaging_max_messages_per_user_per_window,
            max_messages_per_ip_per_window: self.messaging_max_messages_per_ip_per_window,
            rate_limit_window_secs: self.messaging_rate_limit_window_secs,
        }
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Seed a user account into the configured database.
    SeedUser(SeedUserCommand),
    /// Assign a server-wide role to a user.
    AssignServerRole(ServerRoleCommand),
    /// Revoke a server-wide role from a user.
    RevokeServerRole(ServerRoleCommand),
    /// Assign a guild-scoped role to a user.
    AssignGuildRole(GuildRoleCommand),
    /// Assign a channel-scoped role to a user.
    AssignChannelRole(ChannelRoleCommand),
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

#[derive(Args, Debug)]
struct ServerRoleCommand {
    /// Username to modify.
    #[arg(long)]
    username: String,
    /// Role label to assign or revoke (e.g., owner, admin, moderator).
    #[arg(long)]
    role: String,
}

#[derive(Args, Debug)]
struct GuildRoleCommand {
    /// Username to modify.
    #[arg(long)]
    username: String,
    /// Target guild identifier.
    #[arg(long)]
    guild_id: Uuid,
    /// Role label to assign (e.g., owner, admin, moderator, member).
    #[arg(long)]
    role: String,
}

#[derive(Args, Debug)]
struct ChannelRoleCommand {
    /// Username to modify.
    #[arg(long)]
    username: String,
    /// Target channel identifier.
    #[arg(long)]
    channel_id: Uuid,
    /// Role label to assign.
    #[arg(long)]
    role: String,
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
        CliCommand::AssignServerRole(cmd) => assign_server_role(config, cmd).await,
        CliCommand::RevokeServerRole(cmd) => revoke_server_role(config, cmd).await,
        CliCommand::AssignGuildRole(cmd) => assign_guild_role(config, cmd).await,
        CliCommand::AssignChannelRole(cmd) => assign_channel_role(config, cmd).await,
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

async fn assign_server_role(config: &ServerConfig, cmd: ServerRoleCommand) -> Result<()> {
    let database_url = config
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("database_url must be configured to assign server roles"))?;

    let username = cmd.username.trim();
    if username.is_empty() {
        anyhow::bail!("username must be provided");
    }

    let role = validate_role(&cmd.role)?;

    let pool = connect(database_url).await?;
    let Some(user_id) = UserRepository::find_user_id_by_username(pool.pool(), username).await?
    else {
        anyhow::bail!("user '{}' not found", username);
    };

    UserRepository::upsert_role(pool.pool(), user_id, &role).await?;
    println!(
        "Assigned server role '{}' to user '{}' (id: {})",
        role, username, user_id
    );
    Ok(())
}

async fn revoke_server_role(config: &ServerConfig, cmd: ServerRoleCommand) -> Result<()> {
    let database_url = config
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("database_url must be configured to revoke server roles"))?;

    let username = cmd.username.trim();
    if username.is_empty() {
        anyhow::bail!("username must be provided");
    }

    let role = validate_role(&cmd.role)?;

    let pool = connect(database_url).await?;
    let Some(user_id) = UserRepository::find_user_id_by_username(pool.pool(), username).await?
    else {
        anyhow::bail!("user '{}' not found", username);
    };

    let removed = UserRepository::revoke_role(pool.pool(), user_id, &role).await?;
    if removed {
        println!(
            "Removed server role '{}' from user '{}' (id: {})",
            role, username, user_id
        );
    } else {
        println!(
            "User '{}' (id: {}) did not have server role '{}'",
            username, user_id, role
        );
    }
    Ok(())
}

async fn assign_guild_role(config: &ServerConfig, cmd: GuildRoleCommand) -> Result<()> {
    let database_url = config
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("database_url must be configured to assign guild roles"))?;

    let username = cmd.username.trim();
    if username.is_empty() {
        anyhow::bail!("username must be provided");
    }

    let role = validate_role(&cmd.role)?;
    let pool = connect(database_url).await?;
    let Some(user_id) = UserRepository::find_user_id_by_username(pool.pool(), username).await?
    else {
        anyhow::bail!("user '{}' not found", username);
    };

    let repo = MessagingRepository::new(pool.clone());
    repo.upsert_guild_membership(cmd.guild_id, user_id, &role)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;
    println!(
        "Assigned guild role '{}' to user '{}' in guild {}",
        role, username, cmd.guild_id
    );
    Ok(())
}

async fn assign_channel_role(config: &ServerConfig, cmd: ChannelRoleCommand) -> Result<()> {
    let database_url = config
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("database_url must be configured to assign channel roles"))?;

    let username = cmd.username.trim();
    if username.is_empty() {
        anyhow::bail!("username must be provided");
    }

    let role = validate_role(&cmd.role)?;
    let pool = connect(database_url).await?;
    let Some(user_id) = UserRepository::find_user_id_by_username(pool.pool(), username).await?
    else {
        anyhow::bail!("user '{}' not found", username);
    };

    let repo = MessagingRepository::new(pool.clone());
    repo.upsert_channel_membership(cmd.channel_id, user_id, &role)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;
    println!(
        "Assigned channel role '{}' to user '{}' in channel {}",
        role, username, cmd.channel_id
    );
    Ok(())
}

fn validate_role(raw: &str) -> Result<String> {
    let role = raw.trim().to_lowercase();
    if role.is_empty() {
        anyhow::bail!("role must be provided");
    }

    const ALLOWED_ROLES: &[&str] = &[
        "owner",
        "admin",
        "moderator",
        "maintainer",
        "member",
        "contributor",
        "viewer",
        "guest",
    ];

    if !ALLOWED_ROLES.contains(&role.as_str()) {
        anyhow::bail!(
            "unsupported role '{}'; expected one of {:?}",
            role,
            ALLOWED_ROLES
        );
    };

    Ok(role)
}

async fn run(config: Arc<ServerConfig>) -> Result<()> {
    init_tracing(&config);

    let env_override_keys = ServerConfig::environment_override_keys();
    if env_override_keys.is_empty() {
        info!("no OPENGUILD_SERVER environment overrides detected");
    } else {
        info!(keys = ?env_override_keys, "detected OPENGUILD_SERVER environment overrides");
    }

    let log_bind_addr = config.bind_addr.clone();
    let log_host = config.host.clone();
    let log_server_name = config.server_name.clone();
    let log_metrics_bind_addr = config.metrics.bind_addr.clone();
    let log_format = config.log_format;
    let log_mls_ciphersuite = config.mls.ciphersuite.clone();
    let log_mls_identity_count = config.mls.identities.len();

    info!(
        bind_addr = ?log_bind_addr,
        host = %log_host,
        server_name = %log_server_name,
        port = config.port,
        log_format = ?log_format,
        metrics_enabled = config.metrics.enabled,
        metrics_bind_addr = ?log_metrics_bind_addr,
        database_url_configured = config.database_url.is_some(),
        session_active_signing_key_configured = config.session.active_signing_key.is_some(),
        session_fallback_verifying_key_count = config.session.fallback_verifying_keys.len(),
        messaging_max_messages_per_user_per_window = config.messaging.max_messages_per_user_per_window,
        messaging_max_messages_per_ip_per_window = config.messaging.max_messages_per_ip_per_window,
        messaging_rate_limit_window_secs = config.messaging.rate_limit_window_secs,
        federation_trusted_server_count = config.federation.trusted_servers.len(),
        mls_enabled = config.mls.enabled,
        mls_ciphersuite = %log_mls_ciphersuite,
        mls_identity_count = log_mls_identity_count,
        "resolved server configuration"
    );

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

    let federation_service =
        federation::FederationService::from_config(&config.federation)?.map(Arc::new);
    let mls_store = if config.mls.enabled {
        let persistence = storage
            .pool()
            .map(|pool| MlsKeyPackageStore::new(pool.cloned()));
        let store = match persistence {
            Some(store) => {
                mls::MlsKeyStore::with_persistence(
                    config.mls.ciphersuite.clone(),
                    config.mls.identities.clone(),
                    store,
                )
                .await?
            }
            None => {
                info!("MLS persistence unavailable; using in-memory key store");
                mls::MlsKeyStore::new(
                    config.mls.ciphersuite.clone(),
                    config.mls.identities.clone(),
                )
            }
        };
        Some(Arc::new(store))
    } else {
        None
    };

    #[cfg(feature = "metrics")]
    let state = AppState::new(config.clone(), storage.clone(), messaging_service.clone())
        .with_session(session_context.clone())
        .with_federation(federation_service.clone())
        .with_mls(mls_store.clone())
        .with_metrics(metrics_ctx.clone());

    #[cfg(not(feature = "metrics"))]
    let state = AppState::new(config.clone(), storage, messaging_service.clone())
        .with_session(session_context.clone())
        .with_federation(federation_service.clone())
        .with_mls(mls_store.clone());

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
    federation: Option<Arc<federation::FederationService>>,
    mls: Option<Arc<MlsKeyStore>>,
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
            federation: None,
            mls: None,
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
            federation: None,
            mls: None,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    fn with_session(mut self, session: Arc<SessionContext>) -> Self {
        self.session = Some(session);
        self
    }

    fn with_federation(mut self, federation: Option<Arc<federation::FederationService>>) -> Self {
        self.federation = federation;
        self
    }

    fn with_mls(mut self, mls: Option<Arc<MlsKeyStore>>) -> Self {
        self.mls = mls;
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

    fn federation(&self) -> Option<Arc<federation::FederationService>> {
        self.federation.clone()
    }

    fn mls(&self) -> Option<Arc<MlsKeyStore>> {
        self.mls.clone()
    }

    fn messaging(&self) -> Option<Arc<messaging::MessagingService>> {
        Some(self.messaging.clone())
    }

    fn server_name(&self) -> String {
        self.config.server_name.clone()
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

async fn federation_transactions(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    Json(payload): Json<federation::TransactionRequest>,
) -> (
    axum::http::StatusCode,
    Json<federation::TransactionResponse>,
) {
    let federation::TransactionRequest { origin, pdus } = payload;

    let (status, body) = match state.federation() {
        Some(service) => {
            let mut evaluation = service.evaluate_transaction(&origin, pdus);
            if let Some(messaging) = state.messaging() {
                let mut stored = Vec::new();
                let mut rejections = Vec::new();
                for event in evaluation.accepted_events.into_iter() {
                    match messaging.ingest_event(&event).await {
                        Ok(_) => stored.push(event),
                        Err(err) => {
                            tracing::warn!(
                                event_id = %event.event_id,
                                %origin,
                                ?err,
                                "failed to persist federated event"
                            );
                            rejections.push(federation::RejectedEvent {
                                event_id: event.event_id.clone(),
                                reason: format!("delivery failed: {err}"),
                            });
                        }
                    }
                }
                evaluation.accepted_events = stored;
                evaluation.rejected.extend(rejections);
            }
            let response = evaluation.into_response(false);
            let status = federation_status(&response);
            (status, response)
        }
        None => (
            axum::http::StatusCode::NOT_IMPLEMENTED,
            federation::TransactionResponse::disabled(origin),
        ),
    };

    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), status.as_u16());
    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    (status, Json(body))
}

fn federation_status(response: &federation::TransactionResponse) -> axum::http::StatusCode {
    if response.disabled {
        axum::http::StatusCode::NOT_IMPLEMENTED
    } else if response.rejected.is_empty() {
        axum::http::StatusCode::ACCEPTED
    } else if response.accepted.is_empty() {
        axum::http::StatusCode::BAD_REQUEST
    } else {
        axum::http::StatusCode::MULTI_STATUS
    }
}

async fn federation_events(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
    Query(query): Query<messaging::TimelineQuery>,
) -> (
    axum::http::StatusCode,
    Json<federation::FederationEventsResponse>,
) {
    let Some(service) = state.federation() else {
        let status = axum::http::StatusCode::NOT_IMPLEMENTED;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        let response = federation::FederationEventsResponse {
            origin: state.server_name(),
            channel_id,
            events: Vec::new(),
        };
        return (status, Json(response));
    };

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let Some(origin) = headers
        .get(FEDERATION_ORIGIN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
    else {
        let status = axum::http::StatusCode::UNAUTHORIZED;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        let response = federation::FederationEventsResponse {
            origin: state.server_name(),
            channel_id,
            events: Vec::new(),
        };
        return (status, Json(response));
    };

    if !service.is_trusted(&origin) {
        let status = axum::http::StatusCode::FORBIDDEN;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        let response = federation::FederationEventsResponse {
            origin: state.server_name(),
            channel_id,
            events: Vec::new(),
        };
        return (status, Json(response));
    }

    let Some(messaging) = state.messaging() else {
        let status = axum::http::StatusCode::SERVICE_UNAVAILABLE;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        let response = federation::FederationEventsResponse {
            origin: state.server_name(),
            channel_id,
            events: Vec::new(),
        };
        return (status, Json(response));
    };

    let limit = query
        .limit
        .unwrap_or(messaging::DEFAULT_TIMELINE_LIMIT)
        .clamp(1, messaging::MAX_TIMELINE_LIMIT);

    let events_result = messaging
        .recent_events(channel_id, query.since, limit)
        .await;

    let (status, events) = match events_result {
        Ok(events) => (axum::http::StatusCode::OK, events),
        Err(MessagingError::ChannelNotFound) => (axum::http::StatusCode::NOT_FOUND, Vec::new()),
        Err(err) => {
            tracing::error!(?err, channel_id = %channel_id, "failed to list federated events");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Vec::new())
        }
    };

    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), status.as_u16());

    let response = federation::FederationEventsResponse {
        origin: state.server_name(),
        channel_id,
        events: events
            .into_iter()
            .map(messaging::TimelineEvent::from)
            .collect(),
    };

    (status, Json(response))
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

    let client_v1_routes = Router::new()
        .route("/users/register", post(users::register))
        .route("/users/me", get(users::me))
        .route("/sessions/login", post(session::login))
        .route("/sessions/refresh", post(session::refresh))
        .route("/sessions/revoke", post(session::revoke))
        .route(
            "/guilds",
            get(messaging::list_guilds).post(messaging::create_guild),
        )
        .route(
            "/guilds/{guild_id}/channels",
            get(messaging::list_channels).post(messaging::create_channel),
        )
        .route(
            "/channels/{channel_id}/messages",
            post(messaging::post_message),
        )
        .route("/channels/{channel_id}/events", get(messaging::list_events))
        .route(
            "/channels/{channel_id}/read",
            post(messaging::mark_channel_read),
        )
        .route("/channels/unread", get(messaging::list_unread_states))
        .route("/channels/{channel_id}/ws", get(messaging::channel_socket))
        .route("/notifications/ws", get(messaging::notification_socket));

    #[cfg_attr(not(feature = "metrics"), allow(unused_mut))]
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/version", get(version))
        .route("/federation/transactions", post(federation_transactions))
        .route("/mls/key-packages", get(list_key_packages))
        .route("/mls/handshake-test-vectors", get(handshake_test_vectors))
        .route(
            "/mls/key-packages/{identity}/rotate",
            post(rotate_key_package),
        )
        .route(
            "/federation/channels/{channel_id}/events",
            get(federation_events),
        );

    #[cfg(feature = "metrics")]
    {
        if expose_metrics_here {
            router = router.route("/metrics", get(metrics_handler));
        }
    }

    // Keep legacy paths while exposing the same handlers under a versioned prefix.
    router = router.merge(client_v1_routes.clone());
    router = router.nest("/client/v1", client_v1_routes);

    let request_id_header = HeaderName::from_static(REQUEST_ID_HEADER);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(HttpSpanMaker)
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
    S: tower::Service<axum::http::Request<B>, Response = axum::response::Response>,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
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

        span.record("status_code", tracing::field::display(status));
        span.record("latency_ms", tracing::field::display(latency_ms));

        tracing::debug!(
            parent: span,
            request_id = %request_id,
            status = status,
            latency_ms,
            "request completed"
        );
    }
}

#[cfg(test)]
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
    build_subscriber_inner(json, env_filter, std::io::stderr)
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
                .with(RequestIdStorageLayer)
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
                .with(RequestIdStorageLayer)
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
    use crate::{federation, messaging, mls, session};
    use axum::body::{to_bytes, Body};
    use axum::http::{HeaderValue, Request, StatusCode};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use chrono::Utc;
    use futures::StreamExt;
    use openguild_core::{messaging::MessageAuthorSnapshot, EventBuilder};
    use openguild_crypto::{generate_signing_key, verifying_key_from, SigningKey};
    use serde_json::{json, Value};
    use serial_test::serial;
    use std::convert::TryInto;
    use std::io::ErrorKind;
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

    async fn bind_test_listener() -> Option<TcpListener> {
        match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => Some(listener),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping websocket test due to permission error: {err}");
                None
            }
            Err(err) => panic!("failed to bind test listener: {err}"),
        }
    }

    fn author_snapshot(id: impl Into<String>) -> MessageAuthorSnapshot {
        let value = id.into();
        MessageAuthorSnapshot {
            id: value.clone(),
            username: value,
            display_name: None,
        }
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
    async fn client_v1_login_route_rejects_blank_inputs() {
        let state = app_state_with_default_session(test_config(), storage_unconfigured());
        let app = build_app(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/client/v1/sessions/login")
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
    async fn user_profile_includes_memberships() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let (session_harness, auth_header, user_id) = session_with_logged_in_user().await;
        let guild = messaging
            .create_guild("Profile Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        messaging
            .upsert_guild_membership(guild.guild_id, user_id, "admin")
            .await
            .expect("guild membership");
        messaging
            .upsert_channel_membership(channel.channel_id, user_id, "moderator")
            .await
            .expect("channel membership");

        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);
        let authorization = auth_header.as_str();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/users/me")
                    .header("authorization", authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        let guilds = payload["guilds"].as_array().expect("guilds array");
        assert_eq!(guilds.len(), 1);
        assert_eq!(guilds[0]["role"], "admin");
        assert_eq!(guilds[0]["name"], "Profile Guild");

        let channels = payload["channels"].as_array().expect("channels array");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["role"], "moderator");
        assert_eq!(channels[0]["name"], "general");
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
    async fn post_message_hits_ip_rate_limit() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("IP Rate Limit Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");

        let harness = session::tests::empty_session_context();
        let session_context = harness.context.clone();
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_context.clone());
        let app = build_app(state);

        let ip_address = "198.51.100.42";
        let limit = messaging::TEST_MAX_MESSAGES_PER_IP_PER_WINDOW;
        let total_requests = limit + 1;

        let mut tokens = Vec::new();
        for idx in 0..total_requests {
            let identifier = format!("ip-user-{idx}@example.org");
            let secret = "letmein".to_string();
            let user_id = Uuid::new_v4();
            harness
                .register_user(identifier.clone(), secret.clone(), user_id)
                .await;
            let attempt = session::LoginAttempt {
                identifier,
                secret,
                device: session::DeviceContext {
                    device_id: format!("device-{idx}"),
                    device_name: None,
                    user_agent: Some("ip-rate-test".into()),
                    ip_address: None,
                },
            };
            let login = harness
                .context
                .login(attempt)
                .await
                .expect("login attempt")
                .expect("login response");
            tokens.push((format!("Bearer {}", login.access_token), user_id));
        }

        let uri = format!("/channels/{}/messages", channel.channel_id);
        for idx in 0..limit {
            let (auth, user_id) = &tokens[idx];
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(&uri)
                        .header("content-type", "application/json")
                        .header("authorization", auth.as_str())
                        .header("x-forwarded-for", ip_address)
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

        let (auth, user_id) = &tokens[limit];
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&uri)
                    .header("content-type", "application/json")
                    .header("authorization", auth.as_str())
                    .header("x-forwarded-for", ip_address)
                    .body(Body::from(format!(
                        "{{\"sender\":\"{}\",\"content\":\"limit reached\"}}",
                        user_id
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn list_events_returns_recent_messages() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let guild = messaging
            .create_guild("Timeline Guild")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let (session_harness, auth_header, user_id) = session_with_logged_in_user().await;
        let author = author_snapshot(user_id.to_string());
        messaging
            .append_message(channel.channel_id, &author, "first")
            .await
            .expect("first message stored");
        messaging
            .append_message(channel.channel_id, &author, "second")
            .await
            .expect("second message stored");

        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone());
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/channels/{}/events?limit=1", channel.channel_id))
                    .header("authorization", auth_header.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let events: Vec<messaging::TimelineEvent> =
            serde_json::from_slice(&body).expect("timeline parses");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 2);
        assert_eq!(
            events[0].event["content"]["content"]
                .as_str()
                .expect("content string"),
            "second"
        );
    }

    #[tokio::test]
    async fn list_key_packages_requires_auth() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let mls_store = Arc::new(mls::MlsKeyStore::new(
            "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519",
            vec!["alice".into()],
        ));

        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_mls(Some(mls_store.clone()));
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/mls/key-packages")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone())
            .with_mls(Some(mls_store));
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/mls/key-packages")
                    .header("authorization", auth_header.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let packages: Vec<mls::PublicKeyPackage> =
            serde_json::from_slice(&body).expect("packages parse");
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].identity, "alice");
    }

    #[tokio::test]
    async fn rotate_key_package_requires_managed_identity() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let mls_store = Arc::new(mls::MlsKeyStore::new(
            "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519",
            vec!["alice".into()],
        ));

        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone())
            .with_mls(Some(mls_store));
        let app = build_app(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mls/key-packages/alice/rotate")
                    .header("authorization", auth_header.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let rotated: mls::PublicKeyPackage =
            serde_json::from_slice(&body).expect("rotation result parses");
        assert_eq!(rotated.identity, "alice");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mls/key-packages/bob/rotate")
                    .header("authorization", auth_header.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn handshake_test_vectors_require_auth() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        use openguild_crypto::{verifying_key_from_base64, Signature};

        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let mls_store = Arc::new(mls::MlsKeyStore::new(
            "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519",
            vec!["alice".into()],
        ));

        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_mls(Some(mls_store.clone()));
        let app = build_app(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/mls/handshake-test-vectors")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let (session_harness, auth_header, _user_id) = session_with_logged_in_user().await;
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(session_harness.context.clone())
            .with_mls(Some(mls_store));
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/mls/handshake-test-vectors")
                    .header("authorization", auth_header.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let vectors: Vec<mls::HandshakeTestVector> =
            serde_json::from_slice(&body).expect("vectors parse");
        assert_eq!(vectors.len(), 1);
        let vector = &vectors[0];
        assert_eq!(vector.identity, "alice");

        let verifying =
            verifying_key_from_base64(&vector.verifying_key).expect("verifying key decodes");
        let signature_bytes = URL_SAFE_NO_PAD
            .decode(vector.signature.as_bytes())
            .expect("signature decodes");
        let signature_array: [u8; 64] = signature_bytes.try_into().expect("signature length");
        let signature = Signature::from_bytes(&signature_array);
        verifying
            .verify_strict(vector.message.as_bytes(), &signature)
            .expect("signature verifies");
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

        let Some(listener) = bind_test_listener().await else {
            return;
        };
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

        let Some(listener) = bind_test_listener().await else {
            return;
        };
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

        let Some(listener) = bind_test_listener().await else {
            return;
        };
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

        let Some(listener) = bind_test_listener().await else {
            return;
        };
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

        let Some(listener) = bind_test_listener().await else {
            return;
        };
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

        let author = author_snapshot("@user:example.org");
        messaging
            .append_message(channel.channel_id, &author, "hi there")
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
        if bind_test_listener().await.is_none() {
            return;
        }
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

    fn federation_ready_state() -> (
        Arc<ServerConfig>,
        Arc<messaging::MessagingService>,
        Arc<federation::FederationService>,
        SigningKey,
    ) {
        let mut cfg = ServerConfig::default();
        let signing = generate_signing_key();
        let verifying = verifying_key_from(&signing);
        let verifying_b64 = URL_SAFE_NO_PAD.encode(verifying.to_bytes());
        cfg.federation
            .trusted_servers
            .push(config::FederatedServerConfig {
                server_name: "remote.example.org".into(),
                key_id: "1".into(),
                verifying_key: verifying_b64,
            });
        let config = Arc::new(cfg);
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let service = federation::FederationService::from_config(&config.federation)
            .expect("federation config loads")
            .expect("service enabled");
        (config, messaging, Arc::new(service), signing)
    }

    #[tokio::test]
    async fn federation_transactions_route_disabled() {
        let config = test_config();
        let messaging = Arc::new(messaging::MessagingService::new_in_memory(
            config.server_name.clone(),
        ));
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging)
            .with_session(default_session_context());
        let app = build_app(state);

        let payload = json!({
            "origin": "remote.example.org",
            "pdus": [],
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/federation/transactions")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn federation_transactions_accept_valid_events() {
        let (config, messaging, federation_service, signing) = federation_ready_state();
        let guild = messaging.create_guild("Remote Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "federation")
            .await
            .unwrap();
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging.clone())
            .with_session(default_session_context())
            .with_federation(Some(federation_service));
        let app = build_app(state);

        let mut event = EventBuilder::new(
            "remote.example.org",
            &channel.channel_id.to_string(),
            "m.room.message",
        )
        .sender("@remote:example.org")
        .content(json!({ "body": "hello federation" }))
        .build();
        event.sign_with("remote.example.org", "1", &signing);
        let expected_id = event.event_id.clone();

        let payload = json!({
            "origin": "remote.example.org",
            "pdus": [event],
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/federation/transactions")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let parsed: federation::TransactionResponse =
            serde_json::from_slice(&body).expect("response parses");
        assert_eq!(parsed.origin, "remote.example.org");
        assert_eq!(parsed.accepted, vec![expected_id]);
        assert!(parsed.rejected.is_empty());
        assert!(!parsed.disabled);

        let recent = messaging
            .recent_events(channel.channel_id, None, 10)
            .await
            .expect("recent events load");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_id, parsed.accepted[0]);
    }

    #[tokio::test]
    async fn federation_transactions_report_invalid_events() {
        let (config, messaging, federation_service, _signing) = federation_ready_state();
        let guild = messaging.create_guild("Remote Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "federation")
            .await
            .unwrap();
        let state = AppState::new(config.clone(), storage_unconfigured(), messaging)
            .with_session(default_session_context())
            .with_federation(Some(federation_service));
        let app = build_app(state);

        let event = EventBuilder::new(
            "remote.example.org",
            &channel.channel_id.to_string(),
            "m.room.message",
        )
        .sender("@remote:example.org")
        .content(json!({ "body": "unsigned" }))
        .build();

        let payload = json!({
            "origin": "remote.example.org",
            "pdus": [event],
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/federation/transactions")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let parsed: federation::TransactionResponse =
            serde_json::from_slice(&body).expect("response parses");
        assert!(parsed.accepted.is_empty());
        assert_eq!(parsed.rejected.len(), 1);
        assert!(parsed.rejected[0]
            .reason
            .to_lowercase()
            .contains("signature"));
    }

    #[tokio::test]
    async fn federation_events_require_trusted_origin() {
        let (config, messaging, federation_service, _signing) = federation_ready_state();
        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context())
            .with_federation(Some(federation_service));
        let app = build_app(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/federation/channels/{}/events", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/federation/channels/{}/events", Uuid::new_v4()))
                    .header(FEDERATION_ORIGIN_HEADER, "untrusted.example.org")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn federation_events_return_events() {
        let (config, messaging, federation_service, _signing) = federation_ready_state();
        let guild = messaging
            .create_guild("Federation Timeline")
            .await
            .expect("guild creation");
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .expect("channel creation");
        let user_id = Uuid::new_v4().to_string();
        let author = author_snapshot(user_id.clone());
        messaging
            .append_message(channel.channel_id, &author, "first")
            .await
            .expect("first message");
        messaging
            .append_message(channel.channel_id, &author, "second")
            .await
            .expect("second message");

        let state = AppState::new(config, storage_unconfigured(), messaging)
            .with_session(default_session_context())
            .with_federation(Some(federation_service));
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/federation/channels/{}/events?limit=1",
                        channel.channel_id
                    ))
                    .header(FEDERATION_ORIGIN_HEADER, "remote.example.org")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let parsed: federation::FederationEventsResponse =
            serde_json::from_slice(&body).expect("response parses");
        assert_eq!(parsed.channel_id, channel.channel_id);
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].sequence, 2);
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
        let app = build_app(state);

        let guild = messaging.create_guild("Metrics Guild").await.unwrap();
        let channel = messaging
            .create_channel(guild.guild_id, "general")
            .await
            .unwrap();
        let author = author_snapshot("@user:example.org");
        messaging
            .append_message(channel.channel_id, &author, "hi there")
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
