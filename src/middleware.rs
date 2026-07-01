use std::any::Any;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::error;

use crate::app::AppState;
use crate::observability::error_tracking;

/// Maximum time a single request may take before it is aborted.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Wraps the Axum router with application-wide, app-agnostic middleware.
///
/// Crate-internal: reached only through [`crate::build_handler`], the single
/// public entry point, so it stays off the crate's public API.
///
/// App-specific layers (auth, sessions, rate-limiting, metrics, CORS) belong
/// here too — add them to the `ServiceBuilder` below.
///
/// NOTE: trailing-slash normalization (`NormalizePathLayer`) is intentionally
/// not included here; it must wrap the router at the make-service level to run
/// before routing. Add it in `build_handler` when needed.
pub(crate) fn apply_axum_middleware(_state: AppState, router: Router) -> Router {
    router.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CatchPanicLayer::custom(handle_panic))
            // Use `with_status_code` because `TimeoutLayer::new` is deprecated since tower-http 0.6.7.
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                REQUEST_TIMEOUT,
            ))
            .layer(CompressionLayer::new()),
    )
}

/// Turn a caught panic into the same `500 "Service panicked"` response the
/// default `CatchPanicLayer` produced, while preserving its `error!` log and
/// additionally reporting the panic to PostHog as an unhandled exception.
fn handle_panic(payload: Box<dyn Any + Send + 'static>) -> Response {
    let message = error_tracking::panic_message(payload.as_ref());
    // Logs remain the source of truth — keep the default layer's error log...
    error!("Service panicked: {message}");
    // ...then capture it (a no-op when error tracking is disabled).
    error_tracking::report_panic(message);

    (StatusCode::INTERNAL_SERVER_ERROR, "Service panicked").into_response()
}

#[cfg(test)]
mod tests {
    use super::{apply_axum_middleware, handle_panic};
    use crate::app::{App, AppState};
    use crate::build_handler;
    use crate::config::{PostHogConfig, Server};
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use std::any::Any;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        let config = Server {
            ip: [127, 0, 0, 1].into(),
            port: 0,
            max_batch_events: 1000,
            posthog: PostHogConfig::default(),
            kafka: crate::config::KafkaConfig::default(),
        };
        AppState(Arc::new(App::builder().config(Arc::new(config)).build()))
    }

    #[tokio::test]
    async fn panic_handler_renders_the_default_500_response() {
        let payload: Box<dyn Any + Send + 'static> = Box::new("kaboom");
        let response = handle_panic(payload);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"Service panicked");
    }

    #[tokio::test]
    async fn panicking_route_is_caught_and_returns_500() {
        async fn boom() {
            panic!("boom in handler");
        }

        let state = test_state();
        let router = Router::new()
            .route("/boom", get(boom))
            .with_state(state.clone());
        let handler = apply_axum_middleware(state, router);

        let request = Request::builder().uri("/boom").body(Body::empty()).unwrap();
        let response = handler.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn health_passes_through_middleware() {
        let config = Server {
            ip: [127, 0, 0, 1].into(),
            port: 0,
            max_batch_events: 1000,
            posthog: crate::config::PostHogConfig::default(),
            kafka: crate::config::KafkaConfig::default(),
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
