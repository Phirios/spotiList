use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("track not found: {0}")]
    NotFound(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Upstream(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Auth(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
