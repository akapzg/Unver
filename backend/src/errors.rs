use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("HTTP client error")]
    Http(#[from] reqwest::Error),

    #[error("Authentication failed")]
    Unauthorized,

    #[error("Too many requests")]
    RateLimited,

    #[error("Forbidden")]
    Forbidden,

    #[error("Not found")]
    NotFound,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Setup required")]
    SetupRequired,

    #[error("Internal error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => {
                tracing::error!("DB error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            AppError::Http(e) => {
                tracing::error!("HTTP client error: {e}");
                (StatusCode::BAD_GATEWAY, "External service request failed")
            }
            AppError::Unauthorized    => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::RateLimited     => (StatusCode::TOO_MANY_REQUESTS, "Too many requests"),
            AppError::Forbidden       => (StatusCode::FORBIDDEN, "Forbidden"),
            AppError::NotFound        => (StatusCode::NOT_FOUND, "Not found"),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),  // SAFETY: only used below
            AppError::SetupRequired   => (StatusCode::FORBIDDEN, "Initial setup required"),
            AppError::Internal(e)     => {
                tracing::error!("Internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        // For BadRequest, use the dynamic message
        let msg = if let AppError::BadRequest(m) = &self {
            m.as_str().to_string()
        } else {
            message.to_string()
        };

        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
