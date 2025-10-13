use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use openguild_storage::{CreateUserError, UserRepository};
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use crate::AppState;

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
