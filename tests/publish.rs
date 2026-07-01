//! End-to-end publishing tests: verify that `record_accepted` serializes and
//! enqueues events, and that error conditions surface the right HTTP statuses.

use countingsheep_test_utils::TestApp;
use serde_json::json;

fn event() -> serde_json::Value {
    json!({
        "id": "evt-1",
        "source": "/svc",
        "type": "usage.created",
        "specversion": "1.0",
        "subject": "customer-1",
        "time": "2026-06-20T08:30:00Z",
        "data": { "tokens": 42 }
    })
}

#[tokio::test]
async fn accepted_event_is_published_with_flattened_payload() {
    let app = TestApp::init();
    let response = app.post_cloudevent("/api/v1/sheeps", event()).await;
    assert_eq!(response.status(), 202);

    let published = app.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].key, "customer-1");
    assert_eq!(published[0].specversion, "1.0");

    let payload: serde_json::Value = serde_json::from_slice(&published[0].payload).unwrap();
    assert_eq!(payload["id"], "evt-1");
    assert_eq!(payload["type"], "usage.created");
    assert_eq!(payload["source"], "/svc");
    assert_eq!(payload["subject"], "customer-1");
    assert_eq!(payload["time"], 1781944200_i64); // 2026-06-20T08:30:00Z
    assert_eq!(payload["data"], json!({ "tokens": 42 }));
}

#[tokio::test]
async fn rejected_batch_publishes_nothing() {
    let app = TestApp::init();
    // Second event is invalid (missing subject) → all-or-nothing, nothing published.
    let good = event();
    let bad = json!({
        "id": "evt-2",
        "source": "/svc",
        "type": "usage.created",
        "specversion": "1.0"
    });
    let response = app
        .post_cloudevent_batch("/api/v1/sheeps", vec![good, bad])
        .await;
    assert_eq!(response.status(), 400);
    assert!(app.published().is_empty());
}

#[tokio::test]
async fn queue_full_maps_to_503() {
    let app = TestApp::with_failing_producer();
    let response = app.post_cloudevent("/api/v1/sheeps", event()).await;
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn backend_error_maps_to_500() {
    let app = TestApp::with_backend_failing_producer();
    let response = app.post_cloudevent("/api/v1/sheeps", event()).await;
    assert_eq!(response.status(), 500);
    assert!(app.published().is_empty());
}
