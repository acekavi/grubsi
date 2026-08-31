mod common;

use common::TestApp;

#[tokio::test]
async fn unknown_ui_routes_fall_back_to_the_spa() {
    // TanStack Router owns client-side routing; a deep link typed into a
    // tablet must reach index.html, not a 404.
    let app = TestApp::spawn().await;

    let response = reqwest::get(app.url("/steward/floor/3")).await.unwrap();
    assert_eq!(response.status(), 200);

    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(content_type.starts_with("text/html"), "got {content_type}");
}

#[tokio::test]
async fn api_routes_still_win_over_the_spa_fallback() {
    let app = TestApp::spawn().await;

    let response = reqwest::get(app.url("/api/v1/nope")).await.unwrap();
    assert_eq!(response.status(), 404);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        content_type.contains("application/json"),
        "got {content_type}"
    );
}
