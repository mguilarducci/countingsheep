//! Application-wide components accessible from each request.

use std::sync::Arc;

use axum::extract::{FromRequestParts, State};
use bon::Builder;
use derive_more::Deref;

use crate::config;
use crate::ingest::producer::Producer;

/// Holds the main components of the application shared across every request.
#[derive(Debug, Builder)]
pub struct App {
    /// The server configuration.
    pub config: Arc<config::Server>,
    /// Where accepted sheep are published — the ingestion terminus.
    pub producer: Arc<dyn Producer>,
}

/// The clonable wrapper that Axum injects into handlers via `State`.
#[derive(Debug, Clone, FromRequestParts, Deref)]
#[from_request(via(State))]
pub struct AppState(pub Arc<App>);
