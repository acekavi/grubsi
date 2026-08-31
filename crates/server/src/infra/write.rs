use chrono::Utc;
use futures::future::BoxFuture;
use grubsi_core::event::DomainEvent;
use serde_json::Value;
use sqlx::SqliteConnection;
use uuid::Uuid;

use crate::infra::db::Db;
use crate::infra::error::{AppError, AppResult};

/// What happened, who did it, and what changed.
///
/// This is a required argument to `write_tx`. The spec argues that `core`'s
/// boundary works because the compiler enforces it; audit logging is the
/// most cross-cutting concern in the system and the easiest to forget, so
/// it gets the same treatment rather than a convention.
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

impl AuditRecord {
    pub fn new(action: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            user_id: None,
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id: None,
            before: None,
            after: None,
        }
    }

    pub fn by(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn entity(mut self, entity_id: Uuid) -> Self {
        self.entity_id = Some(entity_id);
        self
    }

    pub fn before(mut self, before: Value) -> Self {
        self.before = Some(before);
        self
    }

    pub fn after(mut self, after: Value) -> Self {
        self.after = Some(after);
        self
    }
}

/// The result of a write: the value produced, plus events the caller
/// publishes *after* the transaction has committed.
#[derive(Debug)]
pub struct Written<T> {
    pub value: T,
    pub events: Vec<DomainEvent>,
}

/// Run a mutation inside a single write transaction, recording an audit
/// entry in the same transaction.
///
/// Events are returned rather than published: publishing inside the
/// transaction would eventually broadcast state that gets rolled back.
pub async fn write_tx<T, F>(db: &Db, audit: AuditRecord, f: F) -> AppResult<Written<T>>
where
    F: for<'c> FnOnce(&'c mut SqliteConnection) -> BoxFuture<'c, AppResult<(T, Vec<DomainEvent>)>>,
{
    let mut tx = db.write.begin().await?;

    let (value, events) = f(&mut tx).await?;

    sqlx::query(
        "INSERT INTO audit_logs
            (id, user_id, action, entity_type, entity_id, before_json, after_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7())
    .bind(audit.user_id)
    .bind(&audit.action)
    .bind(&audit.entity_type)
    .bind(audit.entity_id)
    .bind(audit.before.as_ref().map(|v| v.to_string()))
    .bind(audit.after.as_ref().map(|v| v.to_string()))
    .bind(Utc::now().to_rfc3339())
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("audit insert: {e}")))?;

    tx.commit().await?;

    Ok(Written { value, events })
}
