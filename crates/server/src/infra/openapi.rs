use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "grubsi", version = "0.1.0", description = "Local-first restaurant POS"),
    paths(crate::features::health::routes::health),
    components(schemas(
        crate::features::health::routes::HealthResponse,
        crate::infra::ws::Envelope,
        crate::infra::ws::ClientFrame,
    )),
    tags((name = "system", description = "Health and diagnostics"))
)]
pub struct ApiDoc;
