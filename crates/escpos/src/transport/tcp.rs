use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::sink::{SinkError, TicketSink};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Production transport: raw ESC/POS over TCP, conventionally port 9100.
pub struct TcpSink {
    addr: SocketAddr,
    timeout: Duration,
}

impl TcpSink {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl TicketSink for TcpSink {
    async fn send(&self, bytes: &[u8]) -> Result<(), SinkError> {
        let connect = tokio::time::timeout(self.timeout, TcpStream::connect(self.addr));
        let mut stream = match connect.await {
            Err(_) => return Err(SinkError::Timeout),
            Ok(Err(e)) => return Err(SinkError::Connect(e.to_string())),
            Ok(Ok(s)) => s,
        };

        let write = async {
            stream.write_all(bytes).await?;
            stream.flush().await
        };

        match tokio::time::timeout(self.timeout, write).await {
            Err(_) => Err(SinkError::Timeout),
            Ok(Err(e)) => Err(SinkError::Write(e.to_string())),
            Ok(Ok(())) => Ok(()),
        }
    }
}
