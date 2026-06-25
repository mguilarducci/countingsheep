use std::time::Duration;

use axum::Router;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::app::AppState;

/// Maximum time a single request may take before it is aborted.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Wraps the Axum router with application-wide, app-agnostic middleware.
///
/// App-specific layers (auth, sessions, rate-limiting, metrics, CORS) belong
/// here too — add them to the `ServiceBuilder` below.
///
/// NOTE: trailing-slash normalization (`NormalizePathLayer`) is intentionally
/// not included here; it must wrap the router at the make-service level to run
/// before routing. Add it in `build_handler` when needed.
pub fn apply_axum_middleware(_state: AppState, router: Router) -> Router {
    router.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CatchPanicLayer::new())
            .layer(TimeoutLayer::with_status_code(axum::http::StatusCode::REQUEST_TIMEOUT, REQUEST_TIMEOUT))
            .layer(CompressionLayer::new()),
    )
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::build_handler;
    use crate::config::Server;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_passes_through_middleware() {
        let config = Server {
            ip: [127, 0, 0, 1].into(),
            port: 0,
        };
        let app = Arc::new(App::builder().config(Arc::new(config)).build());
        let handler = build_handler(app);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let response = handler.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
