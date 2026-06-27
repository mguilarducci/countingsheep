//! Server-side error tracking: capture panics and 5xx errors to PostHog as
//! `$exception` events.
//!
//! # Design
//!
//! The two places an error actually surfaces — the `CatchPanicLayer` panic
//! handler and [`crate::error::AppError`]'s `into_response` — both have
//! signatures that cannot receive application state, so capture goes through a
//! process-global reporter (mirroring how the `tracing` subscriber is already
//! global, and how `posthog-rs` itself exposes a global client).
//!
//! # Safety by default
//!
//! - Unconfigured (no key) or kill-switched (`POSTHOG_ENABLED=false`) ⇒ a
//!   `NoopSink`; the app runs normally and sends nothing.
//! - Delivery is fire-and-forget via `posthog-rs`'s background worker, so the
//!   request path never blocks on or fails because of PostHog.
//! - A sink that panics is caught (`dispatch`) so it can never unwind into
//!   request handling.
//! - Logs remain the source of truth: enabling/disabling and every failure are
//!   logged, independently of whether PostHog is reachable.

use std::any::Any;
use std::sync::OnceLock;

use serde_json::json;
use tracing::{info, warn};

use crate::config::PostHogConfig;

/// Distinct id for backend exception events. The service has no per-user
/// identity, so all exceptions are attributed to one stable id.
const SERVICE_DISTINCT_ID: &str = "countingsheep-backend";

/// A normalized exception ready to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionReport {
    /// Becomes `$exception_list[].type` — e.g. `"AppError::Internal"` or
    /// `"panic"`.
    pub kind: String,
    /// Becomes `$exception_list[].value` — the human-readable message.
    pub value: String,
    /// `mechanism.handled`: `false` for panics, `true` for caught 5xx.
    pub handled: bool,
}

/// Where reported exceptions go. This trait is the seam tests fake.
pub trait ExceptionSink: Send + Sync {
    /// Deliver one exception. Implementations must not block the caller.
    fn report(&self, report: ExceptionReport);
}

/// Drops every report. The safe default when error tracking is off.
struct NoopSink;

impl ExceptionSink for NoopSink {
    fn report(&self, _report: ExceptionReport) {}
}

/// Sends to PostHog via the library's global client (sync, fire-and-forget).
struct PosthogSink;

impl ExceptionSink for PosthogSink {
    fn report(&self, report: ExceptionReport) {
        match build_exception_event(&report) {
            // Fire-and-forget; a no-op if the global client was never set up.
            Ok(event) => posthog_rs::capture(event),
            Err(error) => warn!(%error, "failed to build $exception event; report dropped"),
        }
    }
}

static SINK: OnceLock<Box<dyn ExceptionSink>> = OnceLock::new();

/// Initialize error tracking from config. Must run inside the Tokio runtime
/// (the PostHog client construction is async). Always logs whether it ended up
/// enabled or disabled, and why.
pub async fn init(config: &PostHogConfig) {
    let sink: Box<dyn ExceptionSink> = match config.active() {
        Some(active) => {
            let result = match active.host {
                Some(host) => posthog_rs::init_global((active.api_key, host)).await,
                None => posthog_rs::init_global(active.api_key).await,
            };
            match result {
                Ok(()) => {
                    info!(
                        host = active.host.unwrap_or(posthog_rs::DEFAULT_HOST),
                        "error tracking enabled"
                    );
                    Box::new(PosthogSink)
                }
                Err(error) => {
                    warn!(%error, "PostHog initialization failed; error tracking disabled");
                    Box::new(NoopSink)
                }
            }
        }
        None => {
            match config.disabled_reason() {
                // A malformed flag is operator error: still additive (capture
                // stays off, the service runs), but loud enough to get noticed.
                Some(reason) if reason.is_misconfiguration() => {
                    warn!(reason = %reason, "error tracking disabled");
                }
                reason => {
                    let reason = reason.map(|reason| reason.to_string()).unwrap_or_default();
                    info!(%reason, "error tracking disabled");
                }
            }
            Box::new(NoopSink)
        }
    };

    if SINK.set(sink).is_err() {
        warn!("error tracking already initialized; ignoring repeat init");
    }
}

/// Flush and stop the PostHog background worker, draining buffered events.
/// Call once during graceful shutdown. A no-op when error tracking is disabled
/// (the global client was never initialized).
pub async fn shutdown() {
    posthog_rs::shutdown().await;
}

/// Report an exception through the global sink. A no-op until [`init`] has run.
pub fn report_exception(report: ExceptionReport) {
    if let Some(sink) = SINK.get() {
        dispatch(sink.as_ref(), report);
    }
}

/// Capture a panic message (handled = false).
pub fn report_panic(message: String) {
    report_exception(panic_report(message));
}

/// Hand a report to a sink, isolating a buggy sink so it can never unwind into
/// request handling.
fn dispatch(sink: &dyn ExceptionSink, report: ExceptionReport) {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    if catch_unwind(AssertUnwindSafe(|| sink.report(report))).is_err() {
        warn!("exception sink panicked while reporting; report dropped");
    }
}

/// The exception report for a 5xx server fault, carrying the full `anyhow`
/// cause chain (outermost first) as the value.
pub fn internal_report(error: &anyhow::Error) -> ExceptionReport {
    ExceptionReport {
        kind: "AppError::Internal".to_string(),
        // `{:#}` renders the full cause chain on one line, outermost first.
        value: format!("{error:#}"),
        handled: true,
    }
}

/// The exception report for a caught panic (unhandled).
fn panic_report(message: String) -> ExceptionReport {
    ExceptionReport {
        kind: "panic".to_string(),
        value: message,
        handled: false,
    }
}

/// Extract a human-readable message from a panic payload, tolerating the common
/// `&str`/`String` shapes and anything else.
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// Build the `$exception` event, mirroring `posthog-rs`'s own `ExceptionItem`
/// /`ExceptionMechanism` serialization so PostHog groups it into issues.
fn build_exception_event(report: &ExceptionReport) -> Result<posthog_rs::Event, posthog_rs::Error> {
    let mut event = posthog_rs::Event::new("$exception", SERVICE_DISTINCT_ID);
    event.insert_prop(
        "$exception_list",
        json!([{
            "type": report.kind,
            "value": report.value,
            "mechanism": { "type": "generic", "handled": report.handled, "synthetic": false },
        }]),
    )?;
    event.insert_prop("$exception_level", "error")?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingSink(Arc<Mutex<Vec<ExceptionReport>>>);

    impl ExceptionSink for RecordingSink {
        fn report(&self, report: ExceptionReport) {
            self.0.lock().unwrap().push(report);
        }
    }

    struct PanickingSink;

    impl ExceptionSink for PanickingSink {
        fn report(&self, _report: ExceptionReport) {
            panic!("sink boom");
        }
    }

    fn sample(kind: &str, value: &str, handled: bool) -> ExceptionReport {
        ExceptionReport {
            kind: kind.to_string(),
            value: value.to_string(),
            handled,
        }
    }

    #[test]
    fn panic_message_reads_str_payload() {
        let payload: Box<dyn Any + Send> = Box::new("boom");
        assert_eq!(panic_message(payload.as_ref()), "boom");
    }

    #[test]
    fn panic_message_reads_string_payload() {
        let payload: Box<dyn Any + Send> = Box::new(String::from("kaboom"));
        assert_eq!(panic_message(payload.as_ref()), "kaboom");
    }

    #[test]
    fn panic_message_falls_back_for_unknown_payload() {
        let payload: Box<dyn Any + Send> = Box::new(42_i32);
        assert_eq!(panic_message(payload.as_ref()), "unknown panic payload");
    }

    #[test]
    fn internal_report_carries_the_chain_and_is_handled() {
        let error = anyhow::anyhow!("db down").context("loading sheep");
        let report = internal_report(&error);
        assert_eq!(report.kind, "AppError::Internal");
        assert!(report.handled);
        assert!(report.value.contains("loading sheep"));
        assert!(report.value.contains("db down"));
    }

    #[test]
    fn panic_report_is_unhandled() {
        let report = panic_report("boom".to_string());
        assert_eq!(report.kind, "panic");
        assert_eq!(report.value, "boom");
        assert!(!report.handled);
    }

    #[test]
    fn build_event_mirrors_posthog_exception_schema() {
        let event = build_exception_event(&sample("AppError::Internal", "db down", true)).unwrap();
        assert_eq!(event.event_name(), "$exception");
        assert_eq!(event.distinct_id(), SERVICE_DISTINCT_ID);

        let props = event.properties();
        assert_eq!(props["$exception_level"], json!("error"));
        let item = &props["$exception_list"][0];
        assert_eq!(item["type"], json!("AppError::Internal"));
        assert_eq!(item["value"], json!("db down"));
        assert_eq!(item["mechanism"]["handled"], json!(true));
        assert_eq!(item["mechanism"]["synthetic"], json!(false));
    }

    #[test]
    fn build_event_marks_panics_unhandled() {
        let event = build_exception_event(&sample("panic", "boom", false)).unwrap();
        let props = event.properties();
        assert_eq!(
            props["$exception_list"][0]["mechanism"]["handled"],
            json!(false)
        );
    }

    #[test]
    fn dispatch_delivers_to_the_sink() {
        let recorder = RecordingSink::default();
        let log = Arc::clone(&recorder.0);

        dispatch(&recorder, sample("panic", "boom", false));

        let recorded = log.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].value, "boom");
        assert!(!recorded[0].handled);
    }

    #[test]
    fn dispatch_isolates_a_panicking_sink() {
        // A buggy sink must never be able to unwind into request handling.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        dispatch(&PanickingSink, sample("panic", "boom", false));
        std::panic::set_hook(previous);
        // Reaching this line without unwinding is the assertion.
    }
}
