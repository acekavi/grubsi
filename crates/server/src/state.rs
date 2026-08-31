use chrono::{DateTime, Utc};

use crate::infra::db::Db;

/// Shared application state. The event hub lands in Task 6.
#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
    pub db: Db,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self {
            started_at: Utc::now(),
            db,
        }
    }
}
