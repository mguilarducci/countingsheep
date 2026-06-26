use axum::Router;
use axum::routing::{get, post};

use crate::app::AppState;
use crate::error::AppError;
use crate::ingest::handler::create_sheep;

/// Builds the application's Axum router with all routes and the shared state.
pub fn build_axum_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/sheeps", post(create_sheep))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

/// Render unknown routes through our consistent error shape.
async fn not_found() -> AppError {
    AppError::NotFound
}

/// Render wrong-method requests on a known path through the same error shape,
/// so a 405 carries the `{ "errors": [...] }` envelope like every other path.
async fn method_not_allowed() -> AppError {
    AppError::MethodNotAllowed
}
