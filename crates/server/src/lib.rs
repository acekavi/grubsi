//! grubsi server library. The binary is a thin wrapper so integration
//! tests can build the same router the production binary serves.

pub mod features;
pub mod infra;
pub mod state;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

pub use state::AppState;

use infra::error::AppError;
use infra::openapi::ApiDoc;

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Unknown `/api` paths must answer in JSON — clients parse it, and an
/// HTML fallback would be a confusing parse error rather than a 404.
async fn api_not_found() -> AppError {
    AppError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "That endpoint does not exist.",
    )
}

pub fn build_router(state: AppState) -> Router {
    // Do not call `.with_state` on the inner router: `nest` requires both
    // routers to share a state type, and applying state early turns this
    // into a `Router<()>` that will not nest into a stateful parent.
    let api = Router::new()
        .route("/health", get(features::health::routes::health))
        .fallback(api_not_found);

    Router::new()
        .nest("/api/v1", api)
        .route("/api-docs/openapi.json", get(openapi_json))
        .with_state(state)
}
