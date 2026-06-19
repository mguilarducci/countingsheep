use std::sync::Arc;

pub use crate::app::App;

use crate::app::AppState;
use crate::router::build_axum_router;

pub mod app;
pub mod config;
pub mod middleware;
mod router;

/// Builds the application's HTTP handler.
///
/// This is the crate's public entry point: the binary calls this and never
/// reaches into the internal modules directly.
pub fn build_handler(app: Arc<App>) -> axum::Router {
    let state = AppState(app);

    let axum_router = build_axum_router(state.clone());
    middleware::apply_axum_middleware(state, axum_router)
}

/// Used for setting different values depending on whether the app is being run in production,
/// in development, or for testing.
///
/// The app's `config.env` value is set in *src/bin/server.rs* to `Production` if the environment
/// variable `HEROKU` is set and `Development` otherwise. `config.env` is set to `Test`
/// unconditionally in *src/test/all.rs*.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Env {
    Development,
    Test,
    Production,
}
