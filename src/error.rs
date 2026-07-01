//! Application error type and its HTTP/JSON rendering.
//!
//! This is an internal module (not part of the crate's public API). `AppError`
//! is the request-handling layer's mechanism for turning failures into HTTP
//! responses; it is produced and consumed entirely within the crate (by
//! `router` and `ingest`). The binary only sees the `axum::Router` returned by
//! [`crate::build_handler`] and never names this type, so it stays `mod`, not
//! `pub mod`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::error;

use crate::observability::error_tracking::{self, ExceptionReport};

/// A single validation failure. `index` ties the failure to a position within
/// a batch submission; it is `None` for single-event requests, which keeps the
/// single-event response shape (`{ "detail": ... }`, no `index`) unchanged.
#[derive(Debug)]
pub(crate) struct ValidationItem {
    pub index: Option<usize>,
    pub detail: String,
}

impl ValidationItem {
    /// A failure tied to event `index` within a batch submission.
    pub(crate) fn at(index: usize, detail: String) -> Self {
        Self {
            index: Some(index),
            detail,
        }
    }
}

impl From<String> for ValidationItem {
    /// A failure with no batch position (single-event path).
    fn from(detail: String) -> Self {
        Self {
            index: None,
            detail,
        }
    }
}

/// The application's error type. Add a variant to extend the contract.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
    #[error("{0}")]
    BadRequest(String),
    /// One or more validation failures; each becomes its own `detail`, carrying
    /// an optional batch `index`.
    #[error("validation failed")]
    Validation(Vec<ValidationItem>),
    /// Wrong or missing request media type.
    #[error("{0}")]
    UnsupportedMediaType(String),
    #[error("not found")]
    NotFound,
    /// A known path reached with an unsupported HTTP method.
    #[error("method not allowed")]
    MethodNotAllowed,
    /// Local back-pressure (e.g. producer queue full). Asks client
    /// retry; not code fault, so not captured PostHog.
    // Constructed by the Kafka publish seam (plan Task 8, `record_accepted` on
    // `ProduceError::QueueFull`). Remove this `expect` there — once the variant
    // is constructed the expectation becomes unfulfilled and clippy will flag it.
    #[expect(dead_code, reason = "constructed by the Kafka publish task (plan Task 8)")]
    #[error("{0}")]
    ServiceUnavailable(String),
    /// Internal failures. The cause is logged; the client sees a generic 500.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub(crate) type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// Builds a `Validation` error from plain detail strings with no batch
    /// position — the single-event ingestion path.
    pub(crate) fn validation(details: Vec<String>) -> Self {
        AppError::Validation(details.into_iter().map(ValidationItem::from).collect())
    }
}

/// Renders a single detail string as `{ "detail": ... }`.
fn detail(message: String) -> serde_json::Value {
    json!({ "detail": message })
}

/// Renders a validation item, including `index` only when it carries one.
fn render_validation_item(item: ValidationItem) -> serde_json::Value {
    match item.index {
        Some(index) => json!({ "index": index, "detail": item.detail }),
        None => json!({ "detail": item.detail }),
    }
}

/// The PostHog exception report for an error, or `None` when it must not be
/// tracked. Only `Internal` (5xx) is a server-side fault worth capturing; 4xx
/// are expected client errors and tracking them would only bury real issues.
fn exception_report(error: &AppError) -> Option<ExceptionReport> {
    match error {
        AppError::Internal(cause) => Some(error_tracking::internal_report(cause)),
        AppError::BadRequest(_)
        | AppError::Validation(_)
        | AppError::UnsupportedMediaType(_)
        | AppError::ServiceUnavailable(_)
        | AppError::NotFound
        | AppError::MethodNotAllowed => None,
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Capture server-side faults (5xx) to PostHog before rendering. This is
        // additive: the `error!` log below remains the source of truth, and a
        // no-op when error tracking is disabled.
        if let Some(report) = exception_report(&self) {
            error_tracking::report_exception(report);
        }

        let (status, errors) = match self {
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, vec![detail(message)]),
            AppError::Validation(items) => (
                StatusCode::BAD_REQUEST,
                items.into_iter().map(render_validation_item).collect(),
            ),
            AppError::UnsupportedMediaType(message) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, vec![detail(message)])
            }
            AppError::ServiceUnavailable(message) => {
                (StatusCode::SERVICE_UNAVAILABLE, vec![detail(message)])
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, vec![detail("not found".to_string())]),
            AppError::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                vec![detail("method not allowed".to_string())],
            ),
            AppError::Internal(error) => {
                error!(%error, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    vec![detail("internal server error".to_string())],
                )
            }
        };

        (status, Json(json!({ "errors": errors }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn only_internal_errors_are_reported_to_posthog() {
        assert!(exception_report(&AppError::Internal(anyhow::anyhow!("boom"))).is_some());
        assert!(exception_report(&AppError::BadRequest("x".into())).is_none());
        assert!(exception_report(&AppError::Validation(vec![])).is_none());
        assert!(exception_report(&AppError::UnsupportedMediaType("x".into())).is_none());
        assert!(exception_report(&AppError::ServiceUnavailable("x".into())).is_none());
        assert!(exception_report(&AppError::NotFound).is_none());
        assert!(exception_report(&AppError::MethodNotAllowed).is_none());
    }

    #[test]
    fn internal_error_report_is_a_handled_500_with_the_cause() {
        let report = exception_report(&AppError::Internal(anyhow::anyhow!("db down")))
            .expect("internal errors are reported");
        assert_eq!(report.kind, "AppError::Internal");
        assert!(report.handled);
        assert!(report.value.contains("db down"));
    }

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
    async fn method_not_allowed_renders_405_json() {
        let response = AppError::MethodNotAllowed.into_response();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [{ "detail": "method not allowed" }] })
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
            AppError::validation(vec!["id is required".into(), "type is required".into()])
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
    async fn validation_without_index_omits_index_field() {
        // The single-event path must stay byte-for-byte as before: no `index` key.
        let response = AppError::validation(vec!["id is required".into()]).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let first = &json["errors"][0];
        assert_eq!(first, &serde_json::json!({ "detail": "id is required" }));
        assert!(
            first.get("index").is_none(),
            "index must be absent, got {first}"
        );
    }

    #[tokio::test]
    async fn validation_with_index_renders_index_and_detail() {
        let response = AppError::Validation(vec![ValidationItem::at(2, "id is required".into())])
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [
                { "index": 2, "detail": "id is required" }
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

    #[tokio::test]
    async fn service_unavailable_renders_503_json() {
        let response = AppError::ServiceUnavailable("kafka queue full".into()).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "errors": [{ "detail": "kafka queue full" }] })
        );
    }
}
