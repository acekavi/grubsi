//! grubsi server library. The binary is a thin wrapper so integration
//! tests can build the same router the production binary serves.

pub mod features;
pub mod infra;
pub mod state;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

// Only the debug-only `dev_ping` needs these; a release build compiles it
// out, so the imports go with it.
#[cfg(debug_assertions)]
use axum::{extract::State, routing::post};
#[cfg(debug_assertions)]
use grubsi_core::event::DomainEvent;

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

/// Publishes one event. This exists so the M0 skeleton has something to
/// drive the socket; it is removed when real features begin emitting
/// events in M4.
///
/// Debug-only. It is unauthenticated and fans out to every connected
/// socket, so a release build must not carry it: any device on the
/// restaurant LAN could otherwise loop it to flood every client.
#[cfg(debug_assertions)]
async fn dev_ping(State(state): State<AppState>) -> StatusCode {
    state.hub.publish(DomainEvent::ping());
    StatusCode::ACCEPTED
}

pub fn build_router(state: AppState) -> Router {
    // Do not call `.with_state` on the inner router: `nest` requires both
    // routers to share a state type, and applying state early turns this
    // into a `Router<()>` that will not nest into a stateful parent.
    let api = Router::new().route("/health", get(features::health::routes::health));
    #[cfg(debug_assertions)]
    let api = api.route("/dev/ping", post(dev_ping));
    let api = api.fallback(api_not_found);

    Router::new()
        .nest("/api/v1", api)
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/ws", get(infra::ws::ws_handler))
        .fallback(infra::assets::serve_asset)
        .with_state(state)
}
