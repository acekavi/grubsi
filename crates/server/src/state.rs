use chrono::{DateTime, Utc};

/// Shared application state. Grows through M0: the database lands in Task 4
/// and the event hub in Task 6.
#[derive(Clone)]
pub struct AppState {
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            started_at: Utc::now(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
