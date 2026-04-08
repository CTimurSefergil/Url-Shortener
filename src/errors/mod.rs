use actix_web::{HttpResponse, ResponseError, http::StatusCode};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("gone")]
    Gone,

    #[error("too many requests")]
    TooManyRequests,

    #[error("service unavailable")]
    ServiceUnavailable,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Gone => StatusCode::GONE,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let body = serde_json::json!({
            "error": self.to_string(),
            "status": self.status_code().as_u16(),
        });
        HttpResponse::build(self.status_code()).json(body)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        Self::Internal("database error".to_string())
    }
}

impl From<scylla::errors::NewSessionError> for AppError {
    fn from(e: scylla::errors::NewSessionError) -> Self {
        tracing::error!(error = %e, "cassandra session error");
        Self::Internal("cassandra error".to_string())
    }
}
