//! Runtime entry point and opaque backend handle.
//!
//! This module defines the contract between a product and the SQLite
//! datasource backend. Concrete I/O belongs to the backend module.

use std::any::Any;
use std::str::FromStr;
use std::time::Instant;

use tracing::{debug, info};

use sqlx;

use crate::backend::sqlite::SqlitePool;
use crate::migrate::sqlite::block_on_async;
use crate::migrate::{run_migrations, MigrationBackend};
use crate::{Backend, DbConfig, DbConfigSource, DbError, MigrationSet, Result};

/// Opaque handle to the datasource backend.
///
/// The handle keeps the public runtime surface stable while backend-specific
/// pool and migration implementations are added incrementally.
///
/// # Examples
///
/// ```
/// use nxm_db::{open, DbConfig};
///
/// let _handle = open(&DbConfig::default());
/// ```
pub struct Db {
    pub(crate) backend: Backend,
    pub(crate) pool: Option<Box<dyn Any + Send + Sync>>,
}

impl Db {
    /// Returns the SQLite connection pool.
    ///
    /// Only available when the backend is SQLite and the pool is available.
    ///
    /// # Errors
    /// Returns `DbError::BackendNotImplemented` if the pool is not available.
    pub fn sqlite_pool(&self) -> Result<SqlitePool> {
        let started = Instant::now();
        debug!(target: "nxm_db::sqlite", "ENTER sqlite_pool");

        let pool = self
            .pool
            .as_ref()
            .and_then(|p| p.downcast_ref::<SqlitePool>())
            .cloned()
            .ok_or_else(|| {
                DbError::BackendNotImplemented("sqlite pool not available".to_string())
            })?;

        debug!(
            target: "nxm_db::sqlite",
            elapsed_ms = started.elapsed().as_millis(),
            "EXIT sqlite_pool"
        );
        Ok(pool)
    }

    /// Runs the migrations in `set` for this backend.
    ///
    /// For SQLite the runner is wired: the migration set is applied through
    /// [`run_migrations`], creating the version-tracking table on first use
    /// and applying each pending migration in its own transaction.
    ///
    /// # Errors
    /// Returns `DbError::Migration` if a migration fails (that migration's
    /// transaction is rolled back).
    ///
    /// # Examples
    ///
    /// ```
    /// use nxm_db::{open, DbConfig, MigrationSet};
    ///
    /// let db = open(&DbConfig::default()).expect("open should succeed");
    /// let set = MigrationSet::new(vec![]).expect("empty set is valid");
    /// let _migration = db.migrate(&set);
    /// ```
    pub fn migrate(&self, set: &MigrationSet) -> Result<()> {
        let started = Instant::now();
        debug!(
            target: "nxm_db::runtime",
            backend = ?self.backend,
            migration_count = set.migrations().len(),
            "ENTER migrate"
        );

        let pool = self.sqlite_pool()?;
        let backend = crate::migrate::sqlite::SqliteMigrationBackend::new(pool);
        let initial = backend.current_version()?;
        let run_result = run_migrations(&backend, set);
        let final_version = match backend.current_version() {
            Ok(v) => v,
            Err(_) => initial,
        };
        let applied = final_version.saturating_sub(initial);
        match &run_result {
            Ok(()) => {
                debug!(
                    target: "nxm_db::runtime",
                    backend = ?self.backend,
                    applied,
                    final_version,
                    elapsed_ms = started.elapsed().as_millis(),
                    "EXIT migrate"
                );
            }
            Err(error) => {
                debug!(
                    target: "nxm_db::runtime",
                    backend = ?self.backend,
                    applied,
                    final_version,
                    error = %error,
                    elapsed_ms = started.elapsed().as_millis(),
                    "EXIT migrate"
                );
            }
        }
        run_result
    }
}

/// Opens the backend selected by `config`.
///
/// This entry point performs compile-time feature dispatch only. It does not
/// open files or network connections.
///
/// # Examples
///
/// ```
/// use nxm_db::{open, DbConfig};
///
/// let _result = open(&DbConfig::default());
/// ```
pub fn open(config: &DbConfig) -> Result<Db> {
    let started = Instant::now();
    debug!(target: "nxm_db::runtime", backend = ?config.backend, "ENTER open");

    let result = match config.backend {
        Backend::Sqlite => block_on_async(crate::backend::sqlite::open_sqlite_impl(config)),
    };

    match &result {
        Ok(_) => info!(
            target: "nxm_db::runtime",
            backend = ?config.backend,
            elapsed_ms = started.elapsed().as_millis(),
            "EXIT open"
        ),
        Err(error) => debug!(
            target: "nxm_db::runtime",
            backend = ?config.backend,
            error = %error,
            elapsed_ms = started.elapsed().as_millis(),
            "EXIT open"
        ),
    }
    result
}

/// Opens the backend selected by the config resolved from `source`.
///
/// Calls [`DbConfigSource::db_config`] on `source` and passes the result to
/// [`open`]. Errors from the source are propagated without calling `open`.
///
/// # Examples
///
/// ```
/// use nxm_db::{open_from, DbConfig, DbConfigSource, Result};
///
/// struct FixedSource;
///
/// impl DbConfigSource for FixedSource {
///     fn db_config(&self) -> Result<DbConfig> {
///         Ok(DbConfig::default())
///     }
/// }
///
/// let result = open_from(&FixedSource);
/// ```
pub fn open_from<S: DbConfigSource>(source: &S) -> Result<Db> {
    let started = Instant::now();
    debug!(target: "nxm_db::runtime", "ENTER open_from");

    let config = source.db_config()?;
    let result = open(&config);

    match &result {
        Ok(_) => debug!(
            target: "nxm_db::runtime",
            backend = ?config.backend,
            elapsed_ms = started.elapsed().as_millis(),
            "EXIT open_from"
        ),
        Err(error) => debug!(
            target: "nxm_db::runtime",
            backend = ?config.backend,
            error = %error,
            elapsed_ms = started.elapsed().as_millis(),
            "EXIT open_from"
        ),
    }
    result
}

/// Creates a SQLite connection pool directly from a URL.
///
/// # Errors
/// Returns `DbError::Config` if the URL is invalid or path cannot be resolved.
/// Returns `DbError::Connection` if the database cannot be opened or pool creation fails.
///
/// # Examples
///
/// ```
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use nxm_db::sqlite_pool;
///
/// let pool = sqlite_pool("sqlite:./data.db").await?;
/// # Ok(()) }
/// ```
pub async fn sqlite_pool(url: &str) -> Result<SqlitePool> {
    let expanded_path =
        expand_tilde(url).map_err(|e| DbError::Config(format!("failed to expand path: {e}")))?;

    if let Some(parent) = std::path::Path::new(&expanded_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            DbError::Connection(format!("failed to create directory {parent:?}: {e}"))
        })?;
    }

    let options =
        sqlx::sqlite::SqliteConnectOptions::from_str(&expanded_path)?.create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .map_err(|e| DbError::Connection(format!("failed to create pool: {e}")))?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| DbError::Connection(format!("pool test failed: {e}")))?;

    Ok(pool)
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
