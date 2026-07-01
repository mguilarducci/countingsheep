use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use countingsheep_test_utils::TestApp;
use serde_json::{Value, json};

fn valid_sheep() -> Value {
    json!({ "id": "a-1", "source": "/svc", "type": "usage.created",
            "specversion": "1.0", "subject": "customer-1" })
}

#[tokio::test]
async fn accepts_valid_sheep() {
    let app = TestApp::init();
    let response = app.post_cloudevent("/api/v1/sheeps", valid_sheep()).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(response.text().is_empty());
}

#[tokio::test]
async fn rejects_missing_required_field() {
    let app = TestApp::init();
    let mut body = valid_sheep();
    body.as_object_mut().unwrap().remove("id");

    let response = app.post_cloudevent("/api/v1/sheeps", body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "id is required");
}

#[tokio::test]
async fn rejects_empty_required_field() {
    let app = TestApp::init();
    let mut body = valid_sheep();
    body["id"] = json!("");

    let response = app.post_cloudevent("/api/v1/sheeps", body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "id must not be empty");
}

#[tokio::test]
async fn rejects_empty_optional_attribute() {
    // CloudEvents v1.0.2: an optional attribute that is present MUST still
    // satisfy its value constraint — `subject` MUST be a non-empty string.
    // Prove the validate() branch propagates through the full HTTP wire path
    // into our consistent {errors:[{detail}]} shape.
    let app = TestApp::init();
    let mut body = valid_sheep();
    body["subject"] = json!("");

    let response = app.post_cloudevent("/api/v1/sheeps", body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "subject must not be empty");
}

#[tokio::test]
async fn rejects_bad_specversion() {
    let app = TestApp::init();
    let mut body = valid_sheep();
    body["specversion"] = json!("0.3");

    let response = app.post_cloudevent("/api/v1/sheeps", body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    let detail = json["errors"][0]["detail"].as_str().unwrap();
    assert!(detail.contains("specversion"), "detail was {detail:?}");
}

#[tokio::test]
async fn rejects_bad_time() {
    let app = TestApp::init();
    let mut body = valid_sheep();
    body["time"] = json!("not-a-date");

    let response = app.post_cloudevent("/api/v1/sheeps", body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    let detail = json["errors"][0]["detail"].as_str().unwrap();
    assert!(detail.contains("RFC 3339"), "detail was {detail:?}");
}

#[tokio::test]
async fn accepts_sheep_with_offset_time() {
    // A present, offset-bearing time is parsed, normalized to UTC, and stamped
    // end-to-end without disturbing the 202 contract. Guards the normalization
    // path through the full wire, complementing the timeless `accepts_valid_sheep`.
    let app = TestApp::init();
    let mut body = valid_sheep();
    body["time"] = json!("2026-06-26T12:00:00+02:00");

    let response = app.post_cloudevent("/api/v1/sheeps", body).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(response.text().is_empty());
}

#[tokio::test]
async fn reports_multiple_errors_at_once() {
    let app = TestApp::init();
    let response = app
        .post_cloudevent("/api/v1/sheeps", json!({ "specversion": "1.0" }))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    assert_eq!(json["errors"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn rejects_invalid_json() {
    let app = TestApp::init();
    let response = app
        .post_raw(
            "/api/v1/sheeps",
            "application/cloudevents+json",
            b"{not json".to_vec(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "body must be valid JSON");
}

#[tokio::test]
async fn rejects_empty_body() {
    let app = TestApp::init();
    let response = app
        .post_raw("/api/v1/sheeps", "application/cloudevents+json", Vec::new())
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_wrong_content_type() {
    let app = TestApp::init();
    let response = app
        .post_raw(
            "/api/v1/sheeps",
            "application/json",
            serde_json::to_vec(&valid_sheep()).unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn accepts_case_insensitive_content_type() {
    // RFC 9110 §8.3.1: media type and subtype are case-insensitive, so a
    // conformant client may send a differently-cased Content-Type.
    let app = TestApp::init();
    let response = app
        .post_raw(
            "/api/v1/sheeps",
            "Application/CloudEvents+JSON; charset=utf-8",
            serde_json::to_vec(&valid_sheep()).unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn rejects_missing_content_type() {
    let app = TestApp::init();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/sheeps")
        .body(Body::from(serde_json::to_vec(&valid_sheep()).unwrap()))
        .unwrap();
    let response = app.run(request).await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn rejects_wrong_method() {
    let app = TestApp::init();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sheeps")
        .body(Body::empty())
        .unwrap();
    let response = app.run(request).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

    // A wrong method on a known path carries the same error envelope as every
    // other error path, not Axum's built-in empty 405 body.
    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "method not allowed");
}

#[tokio::test]
async fn unknown_route_returns_consistent_404() {
    let app = TestApp::init();
    let response = app.post_cloudevent("/api/v1/sheep", valid_sheep()).await; // typo: singular
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "not found");
}

#[tokio::test]
async fn duplicate_submission_is_accepted_twice() {
    let app = TestApp::init();
    // No dedup yet: the same id+source submitted twice both succeed.
    let first = app.post_cloudevent("/api/v1/sheeps", valid_sheep()).await;
    let second = app.post_cloudevent("/api/v1/sheeps", valid_sheep()).await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(second.status(), StatusCode::ACCEPTED);
}

// ---------------------------------------------------------------------------
// Batch ingestion (`application/cloudevents-batch+json`)
// ---------------------------------------------------------------------------

// --- Happy & contract ---

#[tokio::test]
async fn accepts_batch_of_valid_events() {
    let app = TestApp::init();
    let response = app
        .post_cloudevent_batch(
            "/api/v1/sheeps",
            vec![valid_sheep(), valid_sheep(), valid_sheep()],
        )
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(response.text().is_empty());
}

#[tokio::test]
async fn accepts_single_element_batch() {
    let app = TestApp::init();
    let response = app
        .post_cloudevent_batch("/api/v1/sheeps", vec![valid_sheep()])
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn duplicate_events_within_a_batch_are_both_accepted() {
    // No dedup yet: identical events in one batch both succeed, mirroring the
    // single-event contract.
    let app = TestApp::init();
    let response = app
        .post_cloudevent_batch("/api/v1/sheeps", vec![valid_sheep(), valid_sheep()])
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

// --- Adversarial shape ---

#[tokio::test]
async fn rejects_batch_body_that_is_not_an_array() {
    // A bare object under the batch content type.
    let app = TestApp::init();
    let response = app
        .post_raw(
            "/api/v1/sheeps",
            "application/cloudevents-batch+json",
            serde_json::to_vec(&valid_sheep()).unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    assert_eq!(
        json["errors"][0]["detail"],
        "batch body must be a JSON array"
    );
}

#[tokio::test]
async fn rejects_array_sent_to_single_content_type() {
    // An array sent to the SINGLE content type: the single validator sees a
    // non-object and rejects it — a clear 400, not a 500.
    let app = TestApp::init();
    let response = app
        .post_raw(
            "/api/v1/sheeps",
            "application/cloudevents+json",
            serde_json::to_vec(&json!([valid_sheep()])).unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "body must be a JSON object");
}

#[tokio::test]
async fn rejects_non_object_elements_with_their_index() {
    let app = TestApp::init();
    let response = app
        .post_cloudevent_batch(
            "/api/v1/sheeps",
            vec![valid_sheep(), json!(42), json!(null)],
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    let errors = json["errors"].as_array().unwrap();
    assert!(
        errors
            .iter()
            .any(|e| e["index"] == 1 && e["detail"] == "body must be a JSON object"),
        "expected index 1 non-object error, got {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e["index"] == 2 && e["detail"] == "body must be a JSON object"),
        "expected index 2 non-object error, got {errors:?}"
    );
}

#[tokio::test]
async fn rejects_invalid_json_under_batch_content_type() {
    let app = TestApp::init();
    let response = app
        .post_raw(
            "/api/v1/sheeps",
            "application/cloudevents-batch+json",
            b"[{not json".to_vec(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    assert_eq!(json["errors"][0]["detail"], "body must be valid JSON");
}

// --- Index & cross-products ---

#[tokio::test]
async fn scattered_invalid_events_report_correct_indices() {
    let app = TestApp::init();
    let mut bad = valid_sheep();
    bad.as_object_mut().unwrap().remove("id");

    let response = app
        .post_cloudevent_batch("/api/v1/sheeps", vec![bad.clone(), valid_sheep(), bad])
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    let errors = json["errors"].as_array().unwrap();

    // Indices 0 and 2 failed (missing id); index 1 is valid and never appears.
    assert!(
        errors
            .iter()
            .any(|e| e["index"] == 0 && e["detail"] == "id is required")
    );
    assert!(
        errors
            .iter()
            .any(|e| e["index"] == 2 && e["detail"] == "id is required")
    );
    assert!(
        errors.iter().all(|e| e["index"] != 1),
        "the valid event at index 1 must not appear, got {errors:?}"
    );
}

#[tokio::test]
async fn collects_every_error_across_events_with_indices() {
    // A 4-error event (index 0) next to a 3-error event (index 1) => 7 errors,
    // each correctly indexed: per-event all-at-once times across-batch.
    let app = TestApp::init();
    let four_errors = json!({ "specversion": "1.0" }); // missing id, source, type, subject
    let three_errors = json!({ "specversion": "1.0", "type": "usage.created" }); // missing id, source, subject

    let response = app
        .post_cloudevent_batch("/api/v1/sheeps", vec![four_errors, three_errors])
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    let errors = json["errors"].as_array().unwrap();

    assert_eq!(errors.len(), 7, "expected 7 errors, got {errors:?}");
    assert_eq!(errors.iter().filter(|e| e["index"] == 0).count(), 4);
    assert_eq!(errors.iter().filter(|e| e["index"] == 1).count(), 3);
}

// --- Boundaries & resource safety ---

#[tokio::test]
async fn rejects_empty_batch() {
    let app = TestApp::init();
    let response = app.post_cloudevent_batch("/api/v1/sheeps", vec![]).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    assert_eq!(
        json["errors"][0]["detail"],
        "batch must contain at least one event"
    );
}

#[tokio::test]
async fn accepts_batch_exactly_at_the_cap() {
    let app = TestApp::with_max_batch_events(3);
    let response = app
        .post_cloudevent_batch(
            "/api/v1/sheeps",
            vec![valid_sheep(), valid_sheep(), valid_sheep()],
        )
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn rejects_batch_one_over_the_cap() {
    let app = TestApp::with_max_batch_events(3);
    let response = app
        .post_cloudevent_batch(
            "/api/v1/sheeps",
            vec![valid_sheep(), valid_sheep(), valid_sheep(), valid_sheep()],
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    assert_eq!(
        json["errors"][0]["detail"],
        "batch has 4 events, but the maximum is 3"
    );
}

#[tokio::test]
async fn cap_is_checked_before_validation() {
    // An over-cap batch whose events are ALSO all invalid must report the cap
    // error, not validation errors — proving the cap short-circuits before we
    // spend work validating events.
    let app = TestApp::with_max_batch_events(2);
    let invalid = json!({ "specversion": "1.0" }); // missing id, source, type, subject
    let response = app
        .post_cloudevent_batch(
            "/api/v1/sheeps",
            vec![invalid.clone(), invalid.clone(), invalid],
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: Value = response.json();
    let errors = json["errors"].as_array().unwrap();
    assert_eq!(
        errors.len(),
        1,
        "expected only the cap error, got {errors:?}"
    );
    assert_eq!(
        errors[0]["detail"],
        "batch has 3 events, but the maximum is 2"
    );
}
