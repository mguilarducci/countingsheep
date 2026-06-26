use std::sync::Arc;

pub use crate::app::App;
pub use crate::error::{AppError, AppResult};

use crate::app::AppState;
use crate::router::build_axum_router;

pub mod app;
pub mod config;
pub mod error;
mod ingest;
pub mod middleware;
mod router;
pub mod util;

/// Builds the application's HTTP handler.
///
/// This is the crate's public entry point: the binary calls this and never
/// reaches into the internal modules directly.
pub fn build_handler(app: Arc<App>) -> axum::Router {
    let state = AppState(app);

    let axum_router = build_axum_router(state.clone());
    middleware::apply_axum_middleware(state, axum_router)
}
