//! Cross-cutting observability concerns layered over the service.
//!
//! Today this is error tracking (capturing panics and 5xx to PostHog). The
//! `tracing` subscriber lives in [`crate::util::tracing`].

pub mod error_tracking;
