use axum::body::Body;
use axum::http::{Request, StatusCode};
use grubsi_server::infra::db::Db;
use grubsi_server::{AppState, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn state() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
    (dir, AppState::new(db))
}

#[tokio::test]
async fn health_reports_ok() {
    let (_dir, st) = state().await;
    let app = build_router(st);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn unknown_api_routes_return_json_not_html() {
    // A steward's tablet parses JSON. An unknown /api path must not fall
    // through to the SPA and hand it an HTML document.
    let (_dir, st) = state().await;
    let app = build_router(st);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        content_type.contains("application/json"),
        "got {content_type}"
    );
}

#[tokio::test]
async fn openapi_document_lists_the_health_path() {
    let (_dir, st) = state().await;
    let app = build_router(st);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        doc["paths"]["/api/v1/health"].is_object(),
        "health path missing from OpenAPI"
    );
}
