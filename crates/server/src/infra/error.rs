use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// An error on its way to a client.
///
/// The user-facing `message` is mandatory: it is a constructor argument, not
/// an option. Internal detail is carried separately and is logged, never
/// serialized — so a database string cannot reach a steward's tablet.
#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    code: &'static str,
    message: String,
    internal: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            internal: None,
        }
    }

    /// An unexpected failure. The caller supplies detail for the log; the
    /// client gets a generic message.
    pub fn internal(internal: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "Something went wrong on the server. Please try again.".to_owned(),
            internal: Some(internal.into()),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn with_internal(mut self, detail: impl Into<String>) -> Self {
        self.internal = Some(detail.into());
        self
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let Some(detail) = &self.internal {
            tracing::error!(code = self.code, status = %self.status, detail, "request failed");
        } else if self.status.is_server_error() {
            tracing::error!(code = self.code, status = %self.status, "request failed");
        } else {
            tracing::debug!(code = self.code, status = %self.status, "request rejected");
        }

        let body = ErrorBody {
            code: self.code,
            message: &self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::internal(format!("sqlx: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn internal_detail_never_reaches_the_response() {
        let err = AppError::internal("SQLITE_CONSTRAINT_UNIQUE on tables.name");

        let response = err.into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["code"], "internal");
        assert!(!body["message"].as_str().unwrap().is_empty());
        // The whole point of the type: the detail is not in the payload.
        let rendered = body.to_string();
        assert!(!rendered.contains("SQLITE_CONSTRAINT_UNIQUE"));
        assert!(!rendered.contains("tables.name"));
    }

    #[tokio::test]
    async fn user_facing_errors_keep_their_message() {
        let err = AppError::new(
            axum::http::StatusCode::CONFLICT,
            "table_name_taken",
            "A table with this name already exists.",
        );

        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["code"], "table_name_taken");
        assert_eq!(body["message"], "A table with this name already exists.");
    }
}
