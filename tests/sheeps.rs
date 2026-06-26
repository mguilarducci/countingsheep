use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use countingsheep_test_utils::TestApp;
use serde_json::{Value, json};

fn valid_sheep() -> Value {
    json!({ "id": "a-1", "source": "/svc", "type": "usage.created", "specversion": "1.0" })
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
async fn reports_multiple_errors_at_once() {
    let app = TestApp::init();
    let response = app
        .post_cloudevent("/api/v1/sheeps", json!({ "specversion": "1.0" }))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json: Value = response.json();
    assert_eq!(json["errors"].as_array().unwrap().len(), 3);
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
