use axum::Router;

use crate::app::AppState;

/// Wraps the Axum router with application-wide middleware.
///
/// This is the single place where global middleware is layered on top of the
/// router. It currently passes the router through unchanged; `state` is kept in
/// the signature as the seam for state-aware middleware added later.
pub fn apply_axum_middleware(_state: AppState, router: Router) -> Router {
    router
}
