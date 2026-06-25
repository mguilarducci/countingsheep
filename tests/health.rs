use axum::http::StatusCode;
use countingsheep_test_utils::TestApp;

#[tokio::test]
async fn health_returns_ok() {
    let app = TestApp::init();

    let response = app.get("/health").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text(), "ok");
}
