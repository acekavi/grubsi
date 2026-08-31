use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

/// Split database handles.
///
/// The write pool holds exactly one connection, so all writers serialize
/// and `SQLITE_BUSY` cannot occur. At restaurant scale this costs nothing.
/// Because that connection is a global lock, no CPU-bound or network work
/// may happen inside a write transaction.
#[derive(Clone)]
pub struct Db {
    pub read: SqlitePool,
    pub write: SqlitePool,
}

const READ_POOL_SIZE: u32 = 8;

impl Db {
    pub async fn open(path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let write = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;

        // Migrations run on the write pool before readers are handed out,
        // so no reader can observe a half-migrated schema.
        sqlx::migrate!("../../migrations").run(&write).await?;

        let read = SqlitePoolOptions::new()
            .max_connections(READ_POOL_SIZE)
            .connect_with(options)
            .await?;

        Ok(Self { read, write })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("grubsi.db")).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn the_connection_pragmas_are_set_on_both_pools() {
        let (_dir, db) = temp_db().await;

        for (name, pool) in [("write", &db.write), ("read", &db.read)] {
            let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(journal, "wal", "{name} pool journal_mode");

            let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(fk, 1, "{name} pool foreign_keys");

            // NORMAL (1). Under WAL this is the durable-enough setting:
            // a crash cannot corrupt the database, only lose the tail.
            let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(synchronous, 1, "{name} pool synchronous");

            // The single-writer pool should make SQLITE_BUSY impossible;
            // this is the belt to that pair of braces.
            let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(busy_timeout, 5_000, "{name} pool busy_timeout");
        }
    }

    #[tokio::test]
    async fn the_write_pool_holds_exactly_one_connection() {
        // The spec's defence against SQLITE_BUSY. If this ever changes,
        // concurrent writers become possible and the guarantee is gone.
        let (_dir, db) = temp_db().await;
        assert_eq!(db.write.options().get_max_connections(), 1);
        assert!(db.read.options().get_max_connections() > 1);
    }

    #[tokio::test]
    async fn migrations_create_the_audit_log() {
        let (_dir, db) = temp_db().await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs")
            .fetch_one(&db.read)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
