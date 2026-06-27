//! In-process test harness for the countingsheep HTTP app.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;
use countingsheep::app::App;
use countingsheep::build_handler;
use countingsheep::config::Server;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

/// Default batch cap for the harness, mirroring the production default so
/// `init()` exercises realistic behavior.
const DEFAULT_TEST_MAX_BATCH_EVENTS: usize = 1000;

/// A booted application, ready to accept requests in-process (no socket).
pub struct TestApp {
    router: Router,
}

impl TestApp {
    /// Boots the app with the default batch cap (matching production).
    pub fn init() -> Self {
        Self::with_max_batch_events(DEFAULT_TEST_MAX_BATCH_EVENTS)
    }

    /// Boots the app with a custom batch cap, so oversize-batch tests can trip
    /// the limit with a handful of events instead of building a thousand.
    pub fn with_max_batch_events(max_batch_events: usize) -> Self {
        countingsheep::util::tracing::init_for_test();

        let config = Server {
            ip: [127, 0, 0, 1].into(),
            port: 0,
            max_batch_events,
            posthog: countingsheep::config::PostHogConfig::default(),
        };
        let app = Arc::new(App::builder().config(Arc::new(config)).build());
        let router = build_handler(app);

        Self { router }
    }

    /// Sends a pre-built request through the full middleware stack.
    pub async fn run(&self, request: Request<Body>) -> TestResponse {
        // The real server is served with ConnectInfo<SocketAddr>; oneshot
        // bypasses the socket, so inject a mock address for any layer that
        // extracts ConnectInfo.
        let mock_addr = SocketAddr::from(([127, 0, 0, 1], 52381));
        let router = self.router.clone().layer(MockConnectInfo(mock_addr));

        let response = router.oneshot(request).await.unwrap();
        TestResponse::collect(response).await
    }

    /// Sends a GET request.
    pub async fn get(&self, path: &str) -> TestResponse {
        let request = Request::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        self.run(request).await
    }

    /// Sends a POST request with a JSON body.
    pub async fn post_json(&self, path: &str, json: serde_json::Value) -> TestResponse {
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&json).unwrap()))
            .unwrap();
        self.run(request).await
    }

    /// Sends a POST with an explicit Content-Type and raw body bytes.
    pub async fn post_raw(&self, path: &str, content_type: &str, body: Vec<u8>) -> TestResponse {
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap();
        self.run(request).await
    }

    /// Sends a POST as `application/cloudevents+json`.
    pub async fn post_cloudevent(&self, path: &str, json: serde_json::Value) -> TestResponse {
        self.post_raw(
            path,
            "application/cloudevents+json",
            serde_json::to_vec(&json).unwrap(),
        )
        .await
    }

    /// Sends a POST as `application/cloudevents-batch+json` from an event array.
    pub async fn post_cloudevent_batch(
        &self,
        path: &str,
        events: Vec<serde_json::Value>,
    ) -> TestResponse {
        self.post_raw(
            path,
            "application/cloudevents-batch+json",
            serde_json::to_vec(&events).unwrap(),
        )
        .await
    }
}

/// A fully-read HTTP response, with ergonomic accessors.
pub struct TestResponse {
    status: StatusCode,
    body: Bytes,
}

impl TestResponse {
    async fn collect(response: Response) -> Self {
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        Self { status, body }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body).unwrap()
    }
}
