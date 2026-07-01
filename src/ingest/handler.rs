//! HTTP handler for `POST /api/v1/sheeps` and the accept seam.

use axum::body::Bytes;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;
use time::OffsetDateTime;
use tracing::info;

use crate::app::AppState;
use crate::error::{AppError, AppResult, ValidationItem};
use crate::ingest::sheep::validate;
use crate::ingest::stamp::{AcceptedSheep, stamp};

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
    // The one place the clock is read for a single event: stamp `occurred_at`
    // (defaulting a missing time to now) and `received_at` at the edge.
    let accepted = stamp(sheep, OffsetDateTime::now_utc());
    record_accepted(&accepted);
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

    // Read the clock once for the whole batch, then stamp every event with the
    // same `received_at` at the ingestion edge.
    let now = OffsetDateTime::now_utc();
    for sheep in accepted {
        record_accepted(&stamp(sheep, now));
    }
    Ok(StatusCode::ACCEPTED)
}

/// Parse a request body as JSON, mapping parse failures to a stable 400.
fn parse_json(body: &Bytes) -> AppResult<Value> {
    serde_json::from_slice(body)
        .map_err(|_| AppError::BadRequest("body must be valid JSON".to_string()))
}

/// The seam: where an accepted sheep goes. Today, a structured tracing event
/// carrying both timestamps. Durable storage / a broker would replace the body
/// here later.
fn record_accepted(accepted: &AcceptedSheep) {
    let sheep = &accepted.sheep;
    info!(
        id = %sheep.id,
        source = %sheep.source,
        event_type = %sheep.r#type,
        occurred_at = %accepted.occurred_at,
        received_at = %accepted.received_at,
        "sheep accepted"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::sheep::Sheep;
    use crate::ingest::stamp::stamp;
    use std::sync::{Arc, Mutex};
    use time::macros::datetime;
    use tracing::subscriber::with_default;
    use tracing_subscriber::fmt::MakeWriter;

    fn sample_sheep() -> Sheep {
        Sheep {
            id: "a-1".into(),
            source: "/svc".into(),
            r#type: "usage.created".into(),
            specversion: "1.0".into(),
            subject: "customer-1".into(),
            time: Some(datetime!(2026-06-20 08:30:00 UTC)),
            data: None,
            datacontenttype: None,
            dataschema: None,
        }
    }

    /// Appends every emitted log line into a shared buffer. Capturing through a
    /// test-local subscriber (installed only for the current thread via
    /// `with_default`) keeps this test deterministic: it never races the global
    /// subscriber that other tests install, unlike `tracing-test`'s shared
    /// capture.
    #[derive(Clone, Default)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn record_accepted_logs_both_stamps() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(CaptureWriter(Arc::clone(&buffer)))
            .finish();

        with_default(subscriber, || {
            let accepted = stamp(sample_sheep(), datetime!(2026-06-26 10:00:00 UTC));
            record_accepted(&accepted);
        });

        let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("sheep accepted"));
        assert!(logs.contains("a-1")); // the sheep's id reaches the log
        assert!(logs.contains("occurred_at")); // when it happened
        assert!(logs.contains("received_at")); // when we received it
    }
}
