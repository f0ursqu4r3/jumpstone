use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use openguild_crypto::{generate_signing_key, sign_message, verifying_key_from, SigningKey};
use openguild_storage::{
    CredentialError, PersistedSession, SessionPersistence, StoragePool, UserRepository,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{config::SessionConfig, AppState};

const SESSION_TTL_HOURS: i64 = 12;

#[derive(Clone)]
pub struct SessionContext {
    signer: SessionSigner,
    authenticator: Arc<dyn SessionAuthenticator>,
    repository: Arc<dyn SessionRepository>,
    ttl: Duration,
}

impl SessionContext {
    pub fn new(
        signer: SessionSigner,
        authenticator: Arc<dyn SessionAuthenticator>,
        repository: Arc<dyn SessionRepository>,
    ) -> Self {
        Self {
            signer,
            authenticator,
            repository,
            ttl: Duration::hours(SESSION_TTL_HOURS),
        }
    }

    pub async fn login(&self, attempt: LoginAttempt) -> Result<Option<LoginResponse>> {
        let user = match self.authenticator.authenticate(&attempt).await? {
            Some(user) => user,
            None => return Ok(None),
        };

        let record = self.build_record(user.user_id);
        let token = self.signer.sign(&record)?;
        self.repository.persist_session(&record).await?;

        Ok(Some(LoginResponse {
            token,
            expires_at: record.expires_at,
        }))
    }

    fn build_record(&self, user_id: Uuid) -> SessionRecord {
        let issued_at = Utc::now();
        let expires_at = issued_at + self.ttl;
        SessionRecord {
            session_id: Uuid::new_v4(),
            user_id,
            issued_at,
            expires_at,
        }
    }
}

#[derive(Clone)]
pub struct SessionSigner {
    signing_key: SigningKey,
}

impl SessionSigner {
    pub fn from_config(config: &SessionConfig) -> Result<Self> {
        match config.signing_key.as_deref() {
            Some(raw) => {
                let decoded = URL_SAFE_NO_PAD.decode(raw.trim()).with_context(|| {
                    "failed to decode session signing key from base64 (URL-safe)"
                })?;
                let bytes: [u8; 32] = decoded
                    .try_into()
                    .map_err(|_| anyhow!("session signing key must be 32 bytes"))?;
                let signing_key = SigningKey::from_bytes(&bytes);
                Ok(Self { signing_key })
            }
            None => Ok(Self {
                signing_key: generate_signing_key(),
            }),
        }
    }

    pub fn verifying_key_base64(&self) -> String {
        let verifying = verifying_key_from(&self.signing_key);
        URL_SAFE_NO_PAD.encode(verifying.as_bytes())
    }

    pub fn sign(&self, record: &SessionRecord) -> Result<String> {
        let payload = serde_json::to_vec(&SessionClaims::from(record))?;
        let signature = sign_message(&self.signing_key, &payload);

        let token = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(&payload),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        );
        Ok(token)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionRecord {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SessionClaims<'a> {
    session_id: &'a Uuid,
    user_id: &'a Uuid,
    issued_at: &'a DateTime<Utc>,
    expires_at: &'a DateTime<Utc>,
}

impl<'a> From<&'a SessionRecord> for SessionClaims<'a> {
    fn from(value: &'a SessionRecord) -> Self {
        Self {
            session_id: &value.session_id,
            user_id: &value.user_id,
            issued_at: &value.issued_at,
            expires_at: &value.expires_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoginAttempt {
    pub identifier: String,
    pub secret: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

#[async_trait]
pub trait SessionAuthenticator: Send + Sync {
    async fn authenticate(&self, attempt: &LoginAttempt) -> Result<Option<AuthenticatedUser>>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn persist_session(&self, record: &SessionRecord) -> Result<()>;
}

#[derive(Default)]
pub struct InMemorySessionStore {
    accounts: RwLock<HashMap<String, AccountRecord>>,
    sessions: RwLock<HashMap<Uuid, SessionRecord>>,
}

struct AccountRecord {
    user_id: Uuid,
    secret: String,
}

#[async_trait]
impl SessionAuthenticator for InMemorySessionStore {
    async fn authenticate(&self, attempt: &LoginAttempt) -> Result<Option<AuthenticatedUser>> {
        let accounts = self.accounts.read().await;
        match accounts.get(&attempt.identifier) {
            Some(record) if record.secret == attempt.secret => Ok(Some(AuthenticatedUser {
                user_id: record.user_id,
            })),
            _ => Ok(None),
        }
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionStore {
    async fn persist_session(&self, record: &SessionRecord) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(record.session_id, record.clone());
        Ok(())
    }
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_user(
        &self,
        identifier: impl Into<String>,
        secret: impl Into<String>,
        user_id: Uuid,
    ) {
        let mut accounts = self.accounts.write().await;
        accounts.insert(
            identifier.into(),
            AccountRecord {
                user_id,
                secret: secret.into(),
            },
        );
    }

    #[cfg(test)]
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

#[derive(Clone)]
pub struct DatabaseSessionAuthenticator {
    pool: StoragePool,
}

impl DatabaseSessionAuthenticator {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionAuthenticator for DatabaseSessionAuthenticator {
    async fn authenticate(&self, attempt: &LoginAttempt) -> Result<Option<AuthenticatedUser>> {
        match UserRepository::verify_credentials(
            self.pool.pool(),
            &attempt.identifier,
            &attempt.secret,
        )
        .await
        {
            Ok(user_id) => Ok(Some(AuthenticatedUser { user_id })),
            Err(err) => {
                if let Some(creds) = err.downcast_ref::<CredentialError>() {
                    match creds {
                        CredentialError::InvalidCredentials | CredentialError::UserNotFound => {
                            Ok(None)
                        }
                    }
                } else {
                    Err(err)
                }
            }
        }
    }
}

pub struct PostgresSessionRepository {
    persistence: SessionPersistence,
}

impl PostgresSessionRepository {
    pub fn new(pool: StoragePool) -> Self {
        Self {
            persistence: SessionPersistence::new(pool),
        }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn persist_session(&self, record: &SessionRecord) -> Result<()> {
        let persisted = PersistedSession {
            session_id: record.session_id,
            user_id: record.user_id,
            issued_at: record.issued_at,
            expires_at: record.expires_at,
        };
        self.persistence.store_session(&persisted).await
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub secret: String,
}

impl LoginRequest {
    fn validate(self) -> Result<LoginAttempt, Vec<FieldError>> {
        let mut errors = Vec::new();
        let identifier = self.identifier.trim().to_string();
        if identifier.is_empty() {
            errors.push(FieldError::new("identifier", "must be provided"));
        }

        let secret = self.secret.trim().to_string();
        if secret.is_empty() {
            errors.push(FieldError::new("secret", "must be provided"));
        }

        if errors.is_empty() {
            Ok(LoginAttempt { identifier, secret })
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Vec<FieldError>>,
}

#[derive(Debug, Serialize)]
struct FieldError {
    field: &'static str,
    message: &'static str,
}

impl FieldError {
    const fn new(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }
}

pub async fn login(State(state): State<AppState>, Json(payload): Json<LoginRequest>) -> Response {
    let attempt = match payload.validate() {
        Ok(attempt) => attempt,
        Err(errors) => {
            let status = StatusCode::BAD_REQUEST;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.login", status.as_u16());
            return (status, Json(ErrorBody::validation(errors))).into_response();
        }
    };

    match state.session().login(attempt).await {
        Ok(Some(response)) => {
            let status = StatusCode::OK;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.login", status.as_u16());
            (status, Json(response)).into_response()
        }
        Ok(None) => {
            let status = StatusCode::UNAUTHORIZED;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.login", status.as_u16());
            (status, Json(ErrorBody::unauthorized())).into_response()
        }
        Err(err) => {
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.login", status.as_u16());
            tracing::error!(?err, "failed to complete login attempt");
            (status, Json(ErrorBody::server_error())).into_response()
        }
    }
}

impl<'a> ErrorBody<'a> {
    fn validation(details: Vec<FieldError>) -> Self {
        Self {
            error: "validation_error",
            details: Some(details),
        }
    }

    fn unauthorized() -> Self {
        Self {
            error: "invalid_credentials",
            details: None,
        }
    }

    fn server_error() -> Self {
        Self {
            error: "server_error",
            details: None,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct SessionTestHarness {
        pub context: Arc<SessionContext>,
        pub store: Arc<InMemorySessionStore>,
    }

    impl SessionTestHarness {
        pub fn new() -> Self {
            let store = Arc::new(InMemorySessionStore::new());
            let signer = SessionSigner::from_config(&SessionConfig::default()).expect("signer");
            let context = Arc::new(SessionContext::new(signer, store.clone(), store.clone()));
            Self { context, store }
        }

        pub async fn register_user(
            &self,
            identifier: impl Into<String>,
            secret: impl Into<String>,
            user_id: Uuid,
        ) {
            self.store.register_user(identifier, secret, user_id).await;
        }
    }

    pub fn empty_session_context() -> SessionTestHarness {
        SessionTestHarness::new()
    }

    pub async fn session_context_with_user(
        identifier: &str,
        secret: &str,
    ) -> (SessionTestHarness, Uuid) {
        let harness = SessionTestHarness::new();
        let user_id = Uuid::new_v4();
        harness
            .register_user(identifier.to_string(), secret.to_string(), user_id)
            .await;
        (harness, user_id)
    }
}
