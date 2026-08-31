use futures::FutureExt;
use grubsi_server::infra::db::Db;
use grubsi_server::infra::error::AppError;
use grubsi_server::infra::write::{AuditRecord, write_tx};

async fn temp_db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
    // A throwaway table to mutate, so the test does not depend on a
    // feature table that M0 has not built yet.
    sqlx::query("CREATE TABLE probe (id INTEGER NOT NULL PRIMARY KEY, note TEXT NOT NULL) STRICT")
        .execute(&db.write)
        .await
        .unwrap();
    (dir, db)
}

async fn audit_count(db: &Db) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_logs")
        .fetch_one(&db.read)
        .await
        .unwrap()
}

async fn probe_count(db: &Db) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM probe")
        .fetch_one(&db.read)
        .await
        .unwrap()
}

#[tokio::test]
async fn commits_the_mutation_and_its_audit_record_together() {
    let (_dir, db) = temp_db().await;

    let written = write_tx(
        &db,
        AuditRecord::new("probe.create", "probe").after(serde_json::json!({"note": "hello"})),
        |conn| {
            async move {
                sqlx::query("INSERT INTO probe (id, note) VALUES (1, 'hello')")
                    .execute(&mut *conn)
                    .await?;
                Ok((42_i32, Vec::new()))
            }
            .boxed()
        },
    )
    .await
    .unwrap();

    assert_eq!(written.value, 42);
    assert_eq!(probe_count(&db).await, 1);
    assert_eq!(audit_count(&db).await, 1);

    let action: String = sqlx::query_scalar("SELECT action FROM audit_logs")
        .fetch_one(&db.read)
        .await
        .unwrap();
    assert_eq!(action, "probe.create");
}

#[tokio::test]
async fn a_failing_mutation_leaves_no_audit_record() {
    // Audit and mutation share one transaction. A partial write that
    // recorded the audit but not the change would be worse than neither.
    let (_dir, db) = temp_db().await;

    let result = write_tx(&db, AuditRecord::new("probe.create", "probe"), |conn| {
        async move {
            sqlx::query("INSERT INTO probe (id, note) VALUES (1, 'hello')")
                .execute(&mut *conn)
                .await?;
            Err::<((), Vec<grubsi_core::event::DomainEvent>), AppError>(AppError::internal(
                "deliberate failure",
            ))
        }
        .boxed()
    })
    .await;

    assert!(result.is_err());
    assert_eq!(probe_count(&db).await, 0);
    assert_eq!(audit_count(&db).await, 0);
}

#[tokio::test]
async fn events_are_returned_for_the_caller_to_publish_after_commit() {
    // write_tx must not publish. Returning events is what keeps the
    // after-commit rule structurally hard to violate.
    let (_dir, db) = temp_db().await;

    let written = write_tx(&db, AuditRecord::new("probe.create", "probe"), |conn| {
        async move {
            sqlx::query("INSERT INTO probe (id, note) VALUES (7, 'x')")
                .execute(&mut *conn)
                .await?;
            Ok(((), vec![grubsi_core::event::DomainEvent::ping()]))
        }
        .boxed()
    })
    .await
    .unwrap();

    assert_eq!(written.events.len(), 1);
}
