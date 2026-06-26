//! HTTP handler for `POST /api/v1/sheeps` and the accept seam.

use axum::body::Bytes;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;
use tracing::info;

use crate::app::AppState;
use crate::error::{AppError, AppResult, ValidationItem};
use crate::ingest::sheep::{Sheep, validate};

const CLOUDEVENTS_JSON: &str = "application/cloudevents+json";
const CLOUDEVENTS_BATCH_JSON: &str = "application/cloudevents-batch+json";

/// Accept usage events: a single CloudEvent (`application/cloudevents+json`) or
/// a batch array (`application/cloudevents-batch+json`). `202` on success.
///
/// `state` and `headers` are `FromRequestParts` extractors, so they precede the
/// body-consuming `Bytes`.
pub(crate) async fn create_sheep(
    state: AppState,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<StatusCode> {
    let raw_content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    // Ignore parameters such as "; charset=utf-8".
    let media_type = raw_content_type.split(';').next().unwrap_or("").trim();

    if media_type.eq_ignore_ascii_case(CLOUDEVENTS_JSON) {
        ingest_single(&body)
    } else if media_type.eq_ignore_ascii_case(CLOUDEVENTS_BATCH_JSON) {
        ingest_batch(&body, state.config.max_batch_events)
    } else {
        Err(AppError::UnsupportedMediaType(format!(
            "Content-Type must be {CLOUDEVENTS_JSON} or {CLOUDEVENTS_BATCH_JSON}"
        )))
    }
}

/// Validate and accept one event. Errors carry no batch index, so the
/// single-event response shape is unchanged.
fn ingest_single(body: &Bytes) -> AppResult<StatusCode> {
    let value = parse_json(body)?;
    let sheep = validate(value).map_err(AppError::validation)?;
    record_accepted(&sheep);
    Ok(StatusCode::ACCEPTED)
}

/// Validate and accept a batch array. All-or-nothing: every event is validated
/// and, only if all pass, every event is recorded — so a partial failure
/// records nothing. Each failure carries its event's index. The size cap is
/// enforced *before* validation, so an oversized batch is rejected cheaply.
fn ingest_batch(body: &Bytes, max_batch_events: usize) -> AppResult<StatusCode> {
    let Value::Array(events) = parse_json(body)? else {
        return Err(AppError::BadRequest(
            "batch body must be a JSON array".to_string(),
        ));
    };

    if events.is_empty() {
        return Err(AppError::BadRequest(
            "batch must contain at least one event".to_string(),
        ));
    }

    if events.len() > max_batch_events {
        return Err(AppError::BadRequest(format!(
            "batch has {} events, but the maximum is {max_batch_events}",
            events.len()
        )));
    }

    let mut accepted = Vec::with_capacity(events.len());
    let mut errors = Vec::new();
    for (index, event) in events.into_iter().enumerate() {
        match validate(event) {
            Ok(sheep) => accepted.push(sheep),
            Err(details) => errors.extend(
                details
                    .into_iter()
                    .map(|detail| ValidationItem::at(index, detail)),
            ),
        }
    }

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    for sheep in &accepted {
        record_accepted(sheep);
    }
    Ok(StatusCode::ACCEPTED)
}

/// Parse a request body as JSON, mapping parse failures to a stable 400.
fn parse_json(body: &Bytes) -> AppResult<Value> {
    serde_json::from_slice(body)
        .map_err(|_| AppError::BadRequest("body must be valid JSON".to_string()))
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
