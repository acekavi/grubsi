use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SinkError {
    #[error("could not reach the printer: {0}")]
    Connect(String),
    #[error("the printer accepted the connection but not the job: {0}")]
    Write(String),
    #[error("the printer stopped responding")]
    Timeout,
}

/// Where a rendered ticket goes.
///
/// Three implementations: TCP for production, a file sink for local
/// development, and a fake for tests. The print queue drives all of them
/// identically.
#[async_trait]
pub trait TicketSink: Send + Sync {
    async fn send(&self, bytes: &[u8]) -> Result<(), SinkError>;
}
