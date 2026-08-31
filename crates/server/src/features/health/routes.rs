use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Always "ok" when the process is serving requests.
    pub status: &'static str,
    pub version: &'static str,
    /// Seconds since the process started.
    pub uptime_seconds: i64,
}

/// Liveness probe.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses((status = 200, description = "Server is serving requests", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime = (chrono::Utc::now() - state.started_at).num_seconds();
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
    })
}
