use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::infra::db::Db;
use crate::infra::ws::EventHub;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
    pub db: Db,
    pub hub: Arc<EventHub>,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self {
            started_at: Utc::now(),
            db,
            hub: Arc::new(EventHub::new()),
        }
    }
}
