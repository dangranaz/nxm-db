//! SQLite backend implementation.
//!
//! Provides a concrete implementation of the runtime contract for SQLite
//! using `sqlx` connection pooling.

use std::path::Path;
use std::str::FromStr;
use std::time::Instant;

use tracing::{debug, info};

use crate::{Backend, DbConfig, DbError, Result};

/// Inner pool type for SQLite.
/// Uses `sqlx::SqlitePool` for connection pooling.
pub type SqlitePool = sqlx::SqlitePool;

/// Opens a SQLite database with connection pooling.
///
/// # Errors
/// Returns `DbError::Config` if the URL is invalid or path cannot be resolved.
/// Returns `DbError::Connection` if the database cannot be opened or pool creation fails.
pub async fn open_sqlite_impl(config: &DbConfig) -> Result<crate::runtime::Db> {
    let started = Instant::now();
    debug!(target: "nxm_db::sqlite", url = %config.url, "ENTER open_sqlite");

    // Expand tilde in path
    let expanded_path = expand_tilde(&config.url)
        .map_err(|e| DbError::Config(format!("failed to expand path: {e}")))?;

    // Ensure parent directory exists
    if let Some(parent) = Path::new(&expanded_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            DbError::Connection(format!("failed to create directory {parent:?}: {e}"))
        })?;
    }

    // Create pool
    let options =
        sqlx::sqlite::SqliteConnectOptions::from_str(&expanded_path)?.create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .map_err(|e| DbError::Connection(format!("failed to create pool: {e}")))?;

    // Test the pool
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| DbError::Connection(format!("pool test failed: {e}")))?;

    info!(
        target: "nxm_db::sqlite",
        path = %expanded_path,
        elapsed_ms = started.elapsed().as_millis(),
        "EXIT open_sqlite"
    );

    Ok(crate::runtime::Db {
        backend: Backend::Sqlite,
        pool: Some(Box::new(pool)),
    })
}

/// Expands `~` and `~user` in paths to the home directory.
fn expand_tilde(path: &str) -> Result<String> {
    if !path.starts_with('~') {
        return Ok(path.to_string());
    }

    let path = path.trim_start_matches('~');
    let path = path.trim_start_matches('/');

    let home = dirs::home_dir()
        .ok_or_else(|| DbError::Config("could not determine home directory".to_string()))?;

    let expanded = home.join(path);
    Ok(expanded.to_string_lossy().into_owned())
}
