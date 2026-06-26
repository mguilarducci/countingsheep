//! Application error type and its HTTP/JSON rendering.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::error;

/// The application's error type. Add a variant to extend the contract.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    /// One or more validation failures; each string becomes its own `detail`.
    #[error("validation failed")]
    Validation(Vec<String>),
    /// Wrong or missing request media type.
    #[error("{0}")]
    UnsupportedMediaType(String),
    #[error("not found")]
    NotFound,
    /// Internal failures. The cause is logged; the client sees a generic 500.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, details) = match self {
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, vec![message]),
            AppError::Validation(details) => (StatusCode::BAD_REQUEST, details),
            AppError::UnsupportedMediaType(message) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, vec![message])
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, vec!["not found".to_string()]),
            AppError::Internal(error) => {
                error!(%error, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    vec!["internal server error".to_string()],
                )
            }
        };

        let errors: Vec<_> = details
            .into_iter()
            .map(|d| json!({ "detail": d }))
            .collect();
        (status, Json(json!({ "errors": errors }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn not_found_renders_404_json() {
        let response = AppError::NotFound.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [{ "detail": "not found" }] })
        );
    }

    #[tokio::test]
    async fn bad_request_uses_provided_detail() {
        let response = AppError::BadRequest("nope".into()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [{ "detail": "nope" }] })
        );
    }

    #[tokio::test]
    async fn internal_hides_cause_and_returns_500() {
        let error = AppError::Internal(anyhow::anyhow!("db dsn: secret://do-not-leak"));
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // Client sees a generic message...
        assert_eq!(
            json,
            serde_json::json!({ "errors": [{ "detail": "internal server error" }] })
        );
        // ...and the underlying cause must never reach the response body.
        let text = String::from_utf8_lossy(&body);
        assert!(!text.contains("secret"));
    }

    #[tokio::test]
    async fn validation_renders_400_with_all_details() {
        let response =
            AppError::Validation(vec!["id is required".into(), "type is required".into()])
                .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [
                { "detail": "id is required" },
                { "detail": "type is required" }
            ] })
        );
    }

    #[tokio::test]
    async fn unsupported_media_type_renders_415() {
        let response = AppError::UnsupportedMediaType("nope".into()).into_response();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [{ "detail": "nope" }] })
        );
    }
}
