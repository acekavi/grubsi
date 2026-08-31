use std::net::SocketAddr;

use grubsi_server::infra::db::Db;
use grubsi_server::{AppState, build_router};
use tempfile::TempDir;

// `state` and `get_json` are unused by this task's test but are kept for
// later tasks (Task 10 extends this harness with the fake printer).
#[allow(dead_code)]
pub struct TestApp {
    pub addr: SocketAddr,
    pub state: AppState,
    _dir: TempDir,
}

impl TestApp {
    /// Bind the real router to an ephemeral loopback port, backed by a
    /// throwaway database file. Not `:memory:` — WAL behaves differently
    /// there, and WAL is what production runs.
    pub async fn spawn() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
        let state = AppState::new(db);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router(state.clone());

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        Self {
            addr,
            state,
            _dir: dir,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    pub async fn post(&self, path: &str) -> reqwest::Response {
        reqwest::Client::new()
            .post(self.url(path))
            .send()
            .await
            .unwrap()
    }

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
