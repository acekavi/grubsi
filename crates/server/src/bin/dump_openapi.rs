//! Writes the OpenAPI document to stdout. The TypeScript client is
//! generated from this, so the API contract is a build artifact.

use grubsi_server::infra::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let doc = ApiDoc::openapi();
    println!(
        "{}",
        serde_json::to_string_pretty(&doc).expect("serialize OpenAPI")
    );
}
