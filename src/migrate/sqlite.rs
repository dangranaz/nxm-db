//! SQLite implementation of the [`MigrationBackend`] trait.
//!
//! Each migration (its SQL plus the version registration) runs inside a
//! single `sqlx` transaction. A `schema_version` table tracks the
//! current applied migration version, making `migrate` idempotent.

use std::time::Instant;

use sqlx::{AssertSqlSafe, Row, SqlitePool};
use tracing::debug;

use crate::migrate::MigrationBackend;
use crate::{DbError, Result};

/// Name of the version-tracking table.
const SCHEMA_VERSION_TABLE: &str = "schema_version";

/// SQL that creates the version-tracking table if it is absent.
///
/// The table holds a single row (`id = 1`) with the current applied version,
/// so there is never more than one tracked version at a time.
const ENSURE_TRACKING_TABLE: &str =
    "CREATE TABLE IF NOT EXISTS schema_version (id INTEGER PRIMARY KEY CHECK (id = 1), version INTEGER NOT NULL)";

/// SQLite port of [`MigrationBackend`].
///
/// Holds a pooled connection handle; every backend call borrows a
/// connection from the pool.
pub struct SqliteMigrationBackend {
    pool: SqlitePool,
}

impl SqliteMigrationBackend {
    /// Wraps an existing SQLite connection pool as a migration backend.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Ensures the version-tracking table exists (with a `version = 0` row),
    /// then returns the current applied version.
    async fn current_version_locked(&self, pool: &SqlitePool) -> Result<u32> {
        sqlx::query(ENSURE_TRACKING_TABLE)
            .execute(pool)
            .await
            .map_err(|e| DbError::Migration(format!("failed to ensure tracking table: {e}")))?;

        sqlx::query("INSERT OR IGNORE INTO schema_version (id, version) VALUES (1, 0)")
            .execute(pool)
            .await
            .map_err(|e| DbError::Migration(format!("failed to initialize tracking row: {e}")))?;

        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT version FROM {SCHEMA_VERSION_TABLE} WHERE id = 1"
        )))
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Migration(format!("failed to read current version: {e}")))?;

        row.try_get::<u32, _>(0)
            .map_err(|e| DbError::Migration(format!("failed to parse current version: {e}")))
    }
}

impl MigrationBackend for SqliteMigrationBackend {
    fn current_version(&self) -> Result<u32> {
        let started = Instant::now();
        debug!(target: "nxm_db::migrate", "ENTER current_version");

        let version = block_on_async(self.current_version_locked(&self.pool));

        match &version {
            Ok(v) => debug!(
                target: "nxm_db::migrate",
                current_version = v,
                elapsed_ms = started.elapsed().as_millis(),
                "EXIT current_version"
            ),
            Err(error) => debug!(
                target: "nxm_db::migrate",
                error = %error,
                elapsed_ms = started.elapsed().as_millis(),
                "EXIT current_version"
            ),
        }
        version
    }

    fn apply_migration(&self, version: u32, sql: &str) -> Result<()> {
        let started = Instant::now();
        debug!(
            target: "nxm_db::migrate",
            version = version,
            "ENTER apply_migration"
        );

        let result = block_on_async(async {
            // One transaction: migration SQL + version registration. On error the
            // transaction is rolled back (dropped without commit), so no partial
            // state of this migration survives and previously committed migrations
            // remain applied.
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| DbError::Migration(format!("failed to begin transaction: {e}")))?;

            sqlx::query(ENSURE_TRACKING_TABLE)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Migration(format!("failed to ensure tracking table: {e}")))?;

            // Run the migration SQL. Any error rolls back the whole transaction
            // (SQL + version registration) when `tx` is dropped without commit.
            sqlx::query(AssertSqlSafe(sql))
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Migration(format!("migration {version} failed: {e}")))?;

            // Record the applied version in the same transaction (single row, id = 1).
            sqlx::query(AssertSqlSafe(format!(
                "INSERT OR REPLACE INTO {SCHEMA_VERSION_TABLE} (id, version) VALUES (1, ?1)"
            )))
            .bind(version)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Migration(format!("failed to record version {version}: {e}")))?;

            tx.commit().await.map_err(|e| {
                DbError::Migration(format!("failed to commit migration {version}: {e}"))
            })?;

            Ok(())
        });

        match &result {
            Ok(()) => debug!(
                target: "nxm_db::migrate",
                version = version,
                elapsed_ms = started.elapsed().as_millis(),
                "EXIT apply_migration"
            ),
            Err(error) => debug!(
                target: "nxm_db::migrate",
                error = %error,
                elapsed_ms = started.elapsed().as_millis(),
                "EXIT apply_migration"
            ),
        }
        result
    }
}

/// Block on an async future from a sync context using a tokio runtime.
///
/// `nxm-db`'s public migration API is synchronous (`MigrationBackend` trait),
/// while `sqlx` is async. This helper bridges the two by running the future
/// on a tokio runtime.
pub(crate) fn block_on_async<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}
