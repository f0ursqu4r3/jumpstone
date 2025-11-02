use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use axum::{
    extract::State,
    http::{
        header::{AUTHORIZATION, USER_AGENT},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use openguild_crypto::{
    signing_key_from_base64, verifying_key_from_base64, Signature, SigningKeyRing,
};
use openguild_storage::{
    CredentialError, DeviceMetadata, NewRefreshSession, PersistedSession, RefreshSessionRecord,
    RefreshSessionStore, SessionPersistence, StoragePool, UserRepository,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{config::SessionConfig, AppState};

const SESSION_TTL_HOURS: i64 = 12;
const REFRESH_TTL_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub struct DeviceContext {
    pub device_id: String,
    pub device_name: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

impl DeviceContext {
    fn new(
        device_id: impl Into<String>,
        device_name: Option<impl Into<String>>,
        ip_address: Option<impl Into<String>>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            device_name: device_name.map(Into::into),
            user_agent: None,
            ip_address: ip_address.map(Into::into).and_then(|ip| {
                if ip.trim().is_empty() {
                    None
                } else {
                    Some(ip)
                }
            }),
        }
    }

    fn set_user_agent<S: Into<String>>(&mut self, user_agent: Option<S>) {
        self.user_agent =
            user_agent
                .map(Into::into)
                .and_then(|ua| if ua.trim().is_empty() { None } else { Some(ua) });
    }

    fn set_ip_address<S: Into<String>>(&mut self, ip_address: Option<S>) {
        if let Some(value) = ip_address {
            let trimmed = value.into();
            if trimmed.trim().is_empty() {
                self.ip_address = None;
            } else {
                self.ip_address = Some(trimmed);
            }
        } else {
            self.ip_address = None;
        }
    }

    fn to_metadata(&self) -> DeviceMetadata {
        DeviceMetadata::new(
            self.device_id.clone(),
            self.device_name.clone(),
            self.user_agent.clone(),
            self.ip_address.clone(),
        )
    }

    fn from_storage(record: &RefreshSessionRecord) -> Self {
        Self {
            device_id: record.device_id.clone(),
            device_name: record.device_name.clone(),
            user_agent: record.user_agent.clone(),
            ip_address: record.ip_address.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub refresh_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub created_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub last_used_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub revoked_at: Option<DateTime<Utc>>,
    pub device: DeviceContext,
}

#[derive(Debug, Clone)]
pub struct AccessTokenClaims {
    #[allow(dead_code)]
    pub session_id: Uuid,
    pub user_id: Uuid,
    #[allow(dead_code)]
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct SessionContext {
    signer: SessionSigner,
    authenticator: Arc<dyn SessionAuthenticator>,
    repository: Arc<dyn SessionRepository>,
    ttl: Duration,
    refresh_ttl: Duration,
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
            refresh_ttl: Duration::days(REFRESH_TTL_DAYS),
        }
    }

    pub async fn login(&self, attempt: LoginAttempt) -> Result<Option<LoginResponse>> {
        let user = match self.authenticator.authenticate(&attempt).await? {
            Some(user) => user,
            None => return Ok(None),
        };

        let record = self.build_record(user.user_id);
        let access_token = self.signer.sign(&record)?;
        self.repository.persist_session(&record).await?;

        let refresh_record =
            self.build_refresh_record(user.user_id, record.session_id, &attempt.device);
        let stored_refresh = self
            .repository
            .upsert_refresh_session(&refresh_record)
            .await?;
        let refresh_token = encode_refresh_token(stored_refresh.refresh_id);

        Ok(Some(LoginResponse {
            access_token,
            access_expires_at: record.expires_at,
            refresh_token,
            refresh_expires_at: stored_refresh.expires_at,
        }))
    }

    pub async fn refresh(&self, token: &str) -> Result<Option<LoginResponse>> {
        let refresh_id = match decode_refresh_token(token) {
            Ok(id) => id,
            Err(_) => return Ok(None),
        };

        let stored = match self.repository.find_refresh_session(refresh_id).await? {
            Some(record) => record,
            None => return Ok(None),
        };

        if stored.revoked_at.is_some() || stored.expires_at <= Utc::now() {
            return Ok(None);
        }

        self.repository
            .touch_refresh_session(refresh_id, Utc::now())
            .await?;

        let session_record = self.build_record(stored.user_id);
        let access_token = self.signer.sign(&session_record)?;
        self.repository.persist_session(&session_record).await?;

        let refresh_device = stored.device.clone();
        let new_refresh_record =
            self.build_refresh_record(stored.user_id, session_record.session_id, &refresh_device);
        let stored_refresh = self
            .repository
            .upsert_refresh_session(&new_refresh_record)
            .await?;
        let refresh_token = encode_refresh_token(stored_refresh.refresh_id);

        Ok(Some(LoginResponse {
            access_token,
            access_expires_at: session_record.expires_at,
            refresh_token,
            refresh_expires_at: stored_refresh.expires_at,
        }))
    }

    pub fn verify_access_token(&self, token: &str) -> Result<Option<AccessTokenClaims>> {
        match self.signer.verify(token) {
            Ok(claims) => {
                if claims.expires_at <= Utc::now() {
                    Ok(None)
                } else {
                    Ok(Some(claims))
                }
            }
            Err(err) => {
                tracing::debug!(?err, "access token verification failed");
                Ok(None)
            }
        }
    }

    pub async fn revoke(&self, token: &str) -> Result<bool> {
        let refresh_id = match decode_refresh_token(token) {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        if self
            .repository
            .find_refresh_session(refresh_id)
            .await?
            .is_none()
        {
            return Ok(false);
        }

        self.repository
            .revoke_refresh_session(refresh_id, Utc::now())
            .await?;
        Ok(true)
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

    fn build_refresh_record(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        device: &DeviceContext,
    ) -> RefreshTokenRecord {
        let created_at = Utc::now();
        let expires_at = created_at + self.refresh_ttl;
        RefreshTokenRecord {
            refresh_id: Uuid::new_v4(),
            user_id,
            session_id,
            created_at,
            last_used_at: created_at,
            expires_at,
            revoked_at: None,
            device: device.clone(),
        }
    }
}

fn encode_refresh_token(refresh_id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(refresh_id.as_bytes())
}

fn decode_refresh_token(token: &str) -> Result<Uuid> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token.trim().as_bytes())
        .map_err(|_| anyhow!("invalid refresh token encoding"))?;
    if bytes.len() != 16 {
        return Err(anyhow!("invalid refresh token length"));
    }
    let mut arr = [0u8; 16];
    arr.copy_from_slice(&bytes);
    Ok(Uuid::from_bytes(arr))
}

pub fn authenticate_bearer(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AccessTokenClaims, StatusCode> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::trim);

    let value = match header {
        Some(value) if !value.is_empty() => value,
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    match state.session().verify_access_token(token) {
        Ok(Some(claims)) => Ok(claims),
        Ok(None) => Err(StatusCode::UNAUTHORIZED),
        Err(err) => {
            tracing::error!(?err, "failed to verify access token");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Clone)]
pub struct SessionSigner {
    key_ring: SigningKeyRing,
}

impl SessionSigner {
    pub fn from_config(config: &SessionConfig) -> Result<Self> {
        let key_ring = match config.active_signing_key.as_deref() {
            Some(raw) => {
                let signing_key = signing_key_from_base64(raw).with_context(|| {
                    "failed to decode session signing key from config (url-safe base64 expected)"
                })?;
                let mut fallbacks = Vec::new();
                for key in &config.fallback_verifying_keys {
                    let verifier = verifying_key_from_base64(key).with_context(|| {
                        "failed to decode fallback verifying key (url-safe base64 expected)"
                    })?;
                    fallbacks.push(verifier);
                }
                SigningKeyRing::new(signing_key, fallbacks)
            }
            None => SigningKeyRing::default(),
        };
        Ok(Self { key_ring })
    }

    pub fn verifying_key_base64(&self) -> String {
        let verifying = self.key_ring.active_verifying_key();
        URL_SAFE_NO_PAD.encode(verifying.as_bytes())
    }

    pub fn sign(&self, record: &SessionRecord) -> Result<String> {
        let payload = serde_json::to_vec(&SessionClaims::from(record))?;
        let signature = self.key_ring.sign(&payload);

        let token = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(&payload),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        );
        Ok(token)
    }

    pub fn verify(&self, token: &str) -> Result<AccessTokenClaims> {
        let mut components = token.split('.');
        let payload_b64 = components
            .next()
            .ok_or_else(|| anyhow!("access token missing payload"))?;
        let signature_b64 = components
            .next()
            .ok_or_else(|| anyhow!("access token missing signature"))?;
        if components.next().is_some() {
            return Err(anyhow!("access token contains unexpected components"));
        }

        let payload = URL_SAFE_NO_PAD
            .decode(payload_b64.as_bytes())
            .map_err(|_| anyhow!("failed to decode access token payload"))?;
        let signature_bytes = URL_SAFE_NO_PAD
            .decode(signature_b64.as_bytes())
            .map_err(|_| anyhow!("failed to decode access token signature"))?;
        if signature_bytes.len() != 64 {
            return Err(anyhow!("invalid access token signature length"));
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&signature_bytes);
        let signature = Signature::from_bytes(&sig_arr);

        self.key_ring.verify(&payload, &signature)?;

        let claims: SessionClaimsOwned = serde_json::from_slice(&payload)?;
        Ok(AccessTokenClaims {
            session_id: claims.session_id,
            user_id: claims.user_id,
            issued_at: claims.issued_at,
            expires_at: claims.expires_at,
        })
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

#[derive(Debug, Deserialize)]
struct SessionClaimsOwned {
    session_id: Uuid,
    user_id: Uuid,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LoginAttempt {
    pub identifier: String,
    pub secret: String,
    pub device: DeviceContext,
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
    async fn upsert_refresh_session(
        &self,
        record: &RefreshTokenRecord,
    ) -> Result<RefreshTokenRecord>;
    async fn touch_refresh_session(&self, refresh_id: Uuid, used_at: DateTime<Utc>) -> Result<()>;
    async fn revoke_refresh_session(
        &self,
        refresh_id: Uuid,
        revoked_at: DateTime<Utc>,
    ) -> Result<()>;
    async fn find_refresh_session(&self, refresh_id: Uuid) -> Result<Option<RefreshTokenRecord>>;
}

pub struct InMemorySessionStore {
    accounts: RwLock<HashMap<String, AccountRecord>>,
    sessions: RwLock<HashMap<Uuid, SessionRecord>>,
    refresh_tokens: RwLock<HashMap<Uuid, RefreshTokenRecord>>,
    refresh_index: RwLock<HashMap<(Uuid, String), Uuid>>,
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            refresh_tokens: RwLock::new(HashMap::new()),
            refresh_index: RwLock::new(HashMap::new()),
        }
    }
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

    async fn upsert_refresh_session(
        &self,
        record: &RefreshTokenRecord,
    ) -> Result<RefreshTokenRecord> {
        let mut tokens = self.refresh_tokens.write().await;
        let mut index = self.refresh_index.write().await;
        let key = (record.user_id, record.device.device_id.clone());
        if let Some(previous) = index.insert(key, record.refresh_id) {
            tokens.remove(&previous);
        }
        tokens.insert(record.refresh_id, record.clone());
        Ok(record.clone())
    }

    async fn touch_refresh_session(&self, refresh_id: Uuid, used_at: DateTime<Utc>) -> Result<()> {
        let mut tokens = self.refresh_tokens.write().await;
        if let Some(entry) = tokens.get_mut(&refresh_id) {
            entry.last_used_at = used_at;
        }
        Ok(())
    }

    async fn revoke_refresh_session(
        &self,
        refresh_id: Uuid,
        revoked_at: DateTime<Utc>,
    ) -> Result<()> {
        let mut tokens = self.refresh_tokens.write().await;
        if let Some(entry) = tokens.get_mut(&refresh_id) {
            entry.revoked_at = Some(revoked_at);
        }
        Ok(())
    }

    async fn find_refresh_session(&self, refresh_id: Uuid) -> Result<Option<RefreshTokenRecord>> {
        let tokens = self.refresh_tokens.read().await;
        Ok(tokens.get(&refresh_id).cloned())
    }
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
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

    #[cfg(test)]
    pub async fn refresh_count(&self) -> usize {
        self.refresh_tokens.read().await.len()
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
    refresh: RefreshSessionStore,
}

impl PostgresSessionRepository {
    pub fn new(pool: StoragePool) -> Self {
        let refresh_store = RefreshSessionStore::new(pool.clone());
        Self {
            persistence: SessionPersistence::new(pool),
            refresh: refresh_store,
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

    async fn upsert_refresh_session(
        &self,
        record: &RefreshTokenRecord,
    ) -> Result<RefreshTokenRecord> {
        let metadata = record.device.to_metadata();
        let new_session = NewRefreshSession {
            refresh_id: record.refresh_id,
            user_id: record.user_id,
            session_id: record.session_id,
            issued_at: record.created_at,
            expires_at: record.expires_at,
            metadata,
        };
        let stored = self.refresh.upsert(&new_session).await?;
        Ok(refresh_from_storage(stored))
    }

    async fn touch_refresh_session(&self, refresh_id: Uuid, used_at: DateTime<Utc>) -> Result<()> {
        self.refresh.record_use(refresh_id, used_at).await
    }

    async fn revoke_refresh_session(
        &self,
        refresh_id: Uuid,
        revoked_at: DateTime<Utc>,
    ) -> Result<()> {
        self.refresh.revoke(refresh_id, revoked_at).await
    }

    async fn find_refresh_session(&self, refresh_id: Uuid) -> Result<Option<RefreshTokenRecord>> {
        let record = self.refresh.find(refresh_id).await?;
        Ok(record.map(refresh_from_storage))
    }
}

fn refresh_from_storage(record: RefreshSessionRecord) -> RefreshTokenRecord {
    RefreshTokenRecord {
        refresh_id: record.refresh_id,
        user_id: record.user_id,
        session_id: record.session_id,
        created_at: record.created_at,
        last_used_at: record.last_used_at,
        expires_at: record.expires_at,
        revoked_at: record.revoked_at,
        device: DeviceContext::from_storage(&record),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeviceRequest {
    pub device_id: String,
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub ip_address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub secret: String,
    #[serde(default)]
    pub device: Option<DeviceRequest>,
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

        let device = match self.device {
            Some(device) => {
                let device_id = device.device_id.trim().to_string();
                if device_id.is_empty() {
                    errors.push(FieldError::new("device.device_id", "must be provided"));
                }
                let device_name = device.device_name.and_then(|name| {
                    let trimmed = name.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                });
                let ip_address = device.ip_address.and_then(|ip| {
                    let trimmed = ip.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                });

                DeviceContext::new(device_id, device_name, ip_address)
            }
            None => {
                errors.push(FieldError::new("device", "must be provided"));
                DeviceContext::new(
                    String::new(),
                    Option::<String>::None,
                    Option::<String>::None,
                )
            }
        };

        if errors.is_empty() {
            Ok(LoginAttempt {
                identifier,
                secret,
                device,
            })
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

impl RefreshTokenRequest {
    fn validate(self) -> Result<String, Vec<FieldError>> {
        let token = self.refresh_token.trim().to_string();
        if token.is_empty() {
            Err(vec![FieldError::new("refresh_token", "must be provided")])
        } else {
            Ok(token)
        }
    }
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

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Response {
    let mut attempt = match payload.validate() {
        Ok(attempt) => attempt,
        Err(errors) => {
            let status = StatusCode::BAD_REQUEST;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.login", status.as_u16());
            return (status, Json(ErrorBody::validation(errors))).into_response();
        }
    };

    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    attempt.device.set_user_agent(user_agent);
    if let Some(ip) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        attempt.device.set_ip_address(Some(ip.to_string()));
    }

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

pub async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Response {
    let token = match payload.validate() {
        Ok(token) => token,
        Err(details) => {
            let status = StatusCode::BAD_REQUEST;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.refresh", status.as_u16());
            return (status, Json(ErrorBody::validation(details))).into_response();
        }
    };

    let session = state.session();
    match session.refresh(&token).await {
        Ok(Some(response)) => {
            let status = StatusCode::OK;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.refresh", status.as_u16());
            (status, Json(response)).into_response()
        }
        Ok(None) => {
            let status = StatusCode::UNAUTHORIZED;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.refresh", status.as_u16());
            (status, Json(ErrorBody::invalid_refresh_token())).into_response()
        }
        Err(err) => {
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.refresh", status.as_u16());
            tracing::error!(?err, "failed to refresh session");
            (status, Json(ErrorBody::server_error())).into_response()
        }
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Response {
    let token = match payload.validate() {
        Ok(token) => token,
        Err(details) => {
            let status = StatusCode::BAD_REQUEST;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.revoke", status.as_u16());
            return (status, Json(ErrorBody::validation(details))).into_response();
        }
    };

    let session = state.session();
    match session.revoke(&token).await {
        Ok(_) => {
            let status = StatusCode::NO_CONTENT;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.revoke", status.as_u16());
            status.into_response()
        }
        Err(err) => {
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request("sessions.revoke", status.as_u16());
            tracing::error!(?err, "failed to revoke refresh token");
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

    fn invalid_refresh_token() -> Self {
        Self {
            error: "invalid_refresh_token",
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
    use chrono::Utc;

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

    #[tokio::test]
    async fn login_emits_refresh_tokens() {
        let (harness, _) = session_context_with_user("bob@example.org", "letmein").await;
        let mut attempt = LoginAttempt {
            identifier: "bob@example.org".to_string(),
            secret: "letmein".to_string(),
            device: DeviceContext::new("test-device", Some("Unit Test"), None::<String>),
        };
        attempt.device.set_user_agent(Some("session-tests"));

        let response = harness
            .context
            .login(attempt)
            .await
            .expect("login succeeds")
            .expect("response present");

        assert!(!response.access_token.is_empty());
        assert!(!response.refresh_token.is_empty());
        assert!(response.refresh_expires_at > Utc::now());
        assert_eq!(harness.store.session_count().await, 1);
        assert_eq!(harness.store.refresh_count().await, 1);
    }

    #[tokio::test]
    async fn refresh_rotates_tokens() {
        let (harness, _) = session_context_with_user("carol@example.org", "topsecret").await;
        let attempt = LoginAttempt {
            identifier: "carol@example.org".to_string(),
            secret: "topsecret".to_string(),
            device: DeviceContext::new("carol-device", Some("Carol's Laptop"), None::<String>),
        };

        let login = harness
            .context
            .login(attempt)
            .await
            .expect("login succeeds")
            .expect("login response");

        let refreshed = harness
            .context
            .refresh(&login.refresh_token)
            .await
            .expect("refresh succeeds")
            .expect("refresh response");

        assert_ne!(refreshed.refresh_token, login.refresh_token);
        assert!(
            harness
                .context
                .refresh(&login.refresh_token)
                .await
                .expect("refresh call")
                .is_none(),
            "old refresh token should no longer be accepted"
        );
    }

    #[tokio::test]
    async fn revoke_refresh_token_rejects_future_use() {
        let (harness, _) = session_context_with_user("dave@example.org", "p@ssword").await;
        let attempt = LoginAttempt {
            identifier: "dave@example.org".to_string(),
            secret: "p@ssword".to_string(),
            device: DeviceContext::new("dave-device", Some("Dave's Phone"), None::<String>),
        };

        let login = harness
            .context
            .login(attempt)
            .await
            .expect("login succeeds")
            .expect("login response");

        let revoked = harness
            .context
            .revoke(&login.refresh_token)
            .await
            .expect("revoke call");
        assert!(revoked, "token should have been revoked");

        assert!(
            harness
                .context
                .refresh(&login.refresh_token)
                .await
                .expect("refresh call")
                .is_none(),
            "revoked refresh token should not refresh"
        );
    }
}
