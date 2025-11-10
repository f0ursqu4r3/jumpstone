use axum::{
    extract::{MatchedPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use openguild_storage::{
    ChannelMembershipSummary, CreateUserError, GuildMembershipSummary, UserRecord, UserRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, warn};
use uuid::Uuid;

use crate::{session, AppState};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug)]
struct ValidatedRegister {
    username: String,
    password: String,
}

impl RegisterRequest {
    fn validate(self) -> Result<ValidatedRegister, Vec<FieldError>> {
        let mut errors = Vec::new();

        let username = self.username.trim().to_string();
        if username.is_empty() {
            errors.push(FieldError::new("username", "must be provided"));
        }

        let password = self.password.trim().to_string();
        if password.is_empty() {
            errors.push(FieldError::new("password", "must be provided"));
        } else if password.len() < 8 {
            errors.push(FieldError::new(
                "password",
                "must be at least 8 characters long",
            ));
        }

        if errors.is_empty() {
            Ok(ValidatedRegister { username, password })
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    user_id: Uuid,
    username: String,
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

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Response {
    let request = match payload.validate() {
        Ok(valid) => valid,
        Err(details) => {
            let status = StatusCode::BAD_REQUEST;
            #[cfg(feature = "metrics")]
            state.record_http_request("users.register", status.as_u16());
            return (status, Json(ErrorBody::validation(details))).into_response();
        }
    };

    let pool = match state.storage_pool() {
        Some(pool) => pool,
        None => {
            let status = StatusCode::SERVICE_UNAVAILABLE;
            #[cfg(feature = "metrics")]
            state.record_http_request("users.register", status.as_u16());
            return (status, Json(ErrorBody::simple("database_unavailable"))).into_response();
        }
    };

    match UserRepository::create_user(pool.pool(), &request.username, &request.password).await {
        Ok(user_id) => {
            let status = StatusCode::CREATED;
            #[cfg(feature = "metrics")]
            state.record_http_request("users.register", status.as_u16());
            (
                status,
                Json(RegisterResponse {
                    user_id,
                    username: request.username,
                }),
            )
                .into_response()
        }
        Err(CreateUserError::UsernameTaken) => {
            let status = StatusCode::CONFLICT;
            #[cfg(feature = "metrics")]
            state.record_http_request("users.register", status.as_u16());
            (status, Json(ErrorBody::simple("username_taken"))).into_response()
        }
        Err(CreateUserError::Other(err)) => {
            error!(?err, "failed to create user");
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            #[cfg(feature = "metrics")]
            state.record_http_request("users.register", status.as_u16());
            (status, Json(ErrorBody::simple("server_error"))).into_response()
        }
    }
}

#[derive(Debug, Serialize)]
struct GuildMembershipResponse {
    guild_id: Uuid,
    name: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    joined_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct ChannelMembershipResponse {
    channel_id: Uuid,
    guild_id: Uuid,
    name: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    joined_at: Option<DateTime<Utc>>,
}

impl From<GuildMembershipSummary> for GuildMembershipResponse {
    fn from(summary: GuildMembershipSummary) -> Self {
        Self {
            guild_id: summary.guild_id,
            name: summary.guild_name,
            role: summary.role,
            joined_at: Some(summary.joined_at),
        }
    }
}

impl From<ChannelMembershipSummary> for ChannelMembershipResponse {
    fn from(summary: ChannelMembershipSummary) -> Self {
        Self {
            channel_id: summary.channel_id,
            guild_id: summary.guild_id,
            name: summary.channel_name,
            role: summary.role,
            joined_at: Some(summary.joined_at),
        }
    }
}

#[derive(Debug, Serialize)]
struct UserProfileResponse {
    user_id: Uuid,
    username: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_guild_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guilds: Vec<GuildMembershipResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    channels: Vec<ChannelMembershipResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    devices: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
}

impl UserProfileResponse {
    fn fallback(user_id: Uuid, server_name: String) -> Self {
        let username = user_id.to_string();
        Self {
            user_id,
            username: username.clone(),
            display_name: username,
            email: None,
            avatar_url: None,
            server_name: Some(server_name),
            default_guild_id: None,
            timezone: None,
            locale: None,
            created_at: None,
            updated_at: None,
            roles: Vec::new(),
            guilds: Vec::new(),
            channels: Vec::new(),
            devices: Vec::new(),
            metadata: None,
        }
    }

    fn apply_user_record(&mut self, record: &UserRecord) {
        self.user_id = record.user_id;
        self.username = record.username.clone();
        self.display_name = record.username.clone();
        self.created_at = Some(record.created_at);
        self.updated_at = Some(record.updated_at);
    }
}

pub async fn me(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let claims = match session::authenticate_bearer(&state, &headers) {
        Ok(claims) => claims,
        Err(status) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), status.as_u16());
            return status.into_response();
        }
    };

    let mut response = UserProfileResponse::fallback(claims.user_id, state.server_name());

    if let Some(pool) = state.storage_pool() {
        match UserRepository::find_user_by_id(pool.pool(), claims.user_id).await {
            Ok(Some(record)) => response.apply_user_record(&record),
            Ok(None) => {
                warn!(
                    user_id = %claims.user_id,
                    "user record not found while building profile response"
                );
            }
            Err(err) => {
                error!(
                    ?err,
                    user_id = %claims.user_id,
                    "failed to fetch user profile record"
                );
            }
        }

        match UserRepository::list_roles(pool.pool(), claims.user_id).await {
            Ok(roles) => response.roles = roles,
            Err(err) => warn!(?err, user_id = %claims.user_id, "failed to load user roles"),
        }
    }

    if let Some(messaging) = state.messaging() {
        match messaging.guild_memberships_for_user(claims.user_id).await {
            Ok(memberships) => {
                response.guilds = memberships.into_iter().map(Into::into).collect();
            }
            Err(err) => warn!(?err, user_id = %claims.user_id, "failed to load guild memberships"),
        }

        match messaging.channel_memberships_for_user(claims.user_id).await {
            Ok(memberships) => {
                response.channels = memberships.into_iter().map(Into::into).collect();
            }
            Err(err) => warn!(
                ?err,
                user_id = %claims.user_id,
                "failed to load channel memberships"
            ),
        }
    }

    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    (StatusCode::OK, Json(response)).into_response()
}

impl<'a> ErrorBody<'a> {
    fn validation(details: Vec<FieldError>) -> Self {
        Self {
            error: "validation_error",
            details: Some(details),
        }
    }

    fn simple(error: &'a str) -> Self {
        Self {
            error,
            details: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rejects_short_passwords() {
        let request = RegisterRequest {
            username: "testuser".into(),
            password: "short".into(),
        };
        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn validation_accepts_valid_payload() {
        let request = RegisterRequest {
            username: "  testuser  ".into(),
            password: "supersecret".into(),
        };
        let record = request.validate().expect("valid input");
        assert_eq!(record.username, "testuser");
        assert_eq!(record.password, "supersecret");
    }
}
