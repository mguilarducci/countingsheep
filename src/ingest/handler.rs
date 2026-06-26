//! HTTP handler for `POST /api/v1/sheeps` and the accept seam.

use axum::body::Bytes;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use tracing::info;

use crate::error::{AppError, AppResult};
use crate::ingest::sheep::{Sheep, validate};

const CLOUDEVENTS_JSON: &str = "application/cloudevents+json";

/// Accept a single CloudEvents-shaped sheep. `202` on success.
pub(crate) async fn create_sheep(headers: HeaderMap, body: Bytes) -> AppResult<StatusCode> {
    let raw_content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    // Ignore parameters such as "; charset=utf-8".
    let media_type = raw_content_type.split(';').next().unwrap_or("").trim();
    if media_type != CLOUDEVENTS_JSON {
        return Err(AppError::UnsupportedMediaType(format!(
            "Content-Type must be {CLOUDEVENTS_JSON}"
        )));
    }

    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("body must be valid JSON".to_string()))?;

    let sheep = validate(value).map_err(AppError::Validation)?;
    record_accepted(&sheep);
    Ok(StatusCode::ACCEPTED)
}

/// The seam: where an accepted sheep goes. Today, a structured tracing event.
/// Durable storage / a broker would replace the body here later.
fn record_accepted(sheep: &Sheep) {
    info!(
        id = %sheep.id,
        source = %sheep.source,
        event_type = %sheep.r#type,
        "sheep accepted"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    fn sample() -> Sheep {
        Sheep {
            id: "a-1".into(),
            source: "/svc".into(),
            r#type: "usage.created".into(),
            specversion: "1.0".into(),
            subject: None,
            time: None,
            data: None,
            datacontenttype: None,
            dataschema: None,
        }
    }

    #[traced_test]
    #[test]
    fn record_accepted_emits_observable_event() {
        record_accepted(&sample());
        assert!(logs_contain("sheep accepted"));
        assert!(logs_contain("a-1")); // the sheep's id reaches the log
    }
}
