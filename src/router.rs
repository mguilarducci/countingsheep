use axum::Router;
use axum::routing::get;

use crate::app::AppState;

/// Builds the application's Axum router with all routes and the shared state.
pub fn build_axum_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
