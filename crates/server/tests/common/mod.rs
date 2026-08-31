use std::net::SocketAddr;

use grubsi_escpos::transport::fake::{FakeMode, FakePrinter};
use grubsi_server::infra::db::Db;
use grubsi_server::{AppState, build_router};
use tempfile::TempDir;

pub struct TestApp {
    pub addr: SocketAddr,
    // Unused by ws.rs, which doesn't touch the printer — used by harness.rs.
    // Each integration-test binary compiles this module separately, so an
    // item unused in one binary still needs its own allow.
    #[allow(dead_code)]
    pub printer: FakePrinter,
    _dir: TempDir,
}

impl TestApp {
    /// Bind the real router to an ephemeral loopback port, backed by a
    /// throwaway database file. Not `:memory:` — WAL behaves differently
    /// there, and WAL is what production runs.
    pub async fn spawn() -> Self {
        Self::spawn_with_printer(FakeMode::Ok).await
    }

    /// Like `spawn`, but with a fake printer listening in `mode`, reachable
    /// at `printer.addr()`. M4's print queue tests build on this.
    pub async fn spawn_with_printer(mode: FakeMode) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
        // Built and handed to the router, but not stored: no test reaches
        // into the state, and a field nothing reads is not a fixture.
        let state = AppState::new(db);
        let printer = FakePrinter::start(mode).await;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router(state);

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        Self {
            addr,
            printer,
            _dir: dir,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    // Unused by harness.rs — used by ws.rs.
    #[allow(dead_code)]
    pub async fn post(&self, path: &str) -> reqwest::Response {
        reqwest::Client::new()
            .post(self.url(path))
            .send()
            .await
            .unwrap()
    }

    // Unused by ws.rs — used by harness.rs.
    #[allow(dead_code)]
    pub async fn get_json(&self, path: &str) -> serde_json::Value {
        reqwest::get(self.url(path))
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}
