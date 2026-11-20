//! Migration runner contract — backend-agnostic migration application.
//!
//! This module defines the pure-logic contract for applying migrations:
//! - `Migration` model with version, name, and SQL text
//! - `MigrationSet` registry with validation (duplicates, monotonicity)
//! - `MigrationBackend` trait that backends must implement
//! - `run_migrations` function that applies missing migrations in order
//!
//! Concrete backend implementations (SQLite) provide their own
//! `MigrationBackend` implementation. This module contains no I/O — it only
//! orchestrates the migration logic.

use std::time::Instant;

use tracing::{debug, info};

use crate::{DbError, Result};

/// A single migration with version, name, and SQL text.
///
/// Versions are monotonic integers (1, 2, 3...). The `up` field contains
/// the SQL text to execute for this migration.
///
/// # Examples
///
/// ```
/// use nxm_db::migrate::Migration;
///
/// let m = Migration::new(1, "create_users", "CREATE TABLE users (id INTEGER PRIMARY KEY);");
/// assert_eq!(m.version, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// Monotonic version number (1, 2, 3...)
    pub version: u32,
    /// Human-readable name for this migration
    pub name: String,
    /// SQL text to execute for the "up" migration
    pub up: String,
}

impl Migration {
    /// Creates a new migration.
    ///
    /// # Examples
    ///
    /// ```
    /// use nxm_db::migrate::Migration;
    ///
    /// let m = Migration::new(1, "create_users", "CREATE TABLE users (id INTEGER PRIMARY KEY);");
    /// assert_eq!(m.version, 1);
    /// assert_eq!(m.name, "create_users");
    /// ```
    pub fn new(version: u32, name: impl Into<String>, up: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            up: up.into(),
        }
    }
}

/// A validated, ordered set of migrations.
///
/// `MigrationSet` guarantees:
/// - No duplicate versions
/// - Versions are strictly increasing (monotonic)
/// - Migrations must be **declared** in version order; a set given out of
///   order is rejected (it is not silently reordered).
#[derive(Debug, Clone)]
pub struct MigrationSet {
    migrations: Vec<Migration>,
}

impl MigrationSet {
    /// Creates a new migration set from a list of migrations.
    ///
    /// Validates that versions are unique and strictly increasing in the
    /// provided order. Returns `DbError::Config` if validation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use nxm_db::migrate::{Migration, MigrationSet};
    ///
    /// let migrations = vec![
    ///     Migration::new(1, "first", "CREATE TABLE a;"),
    ///     Migration::new(2, "second", "CREATE TABLE b;"),
    /// ];
    /// let set = MigrationSet::new(migrations).expect("valid");
    /// assert_eq!(set.migrations().len(), 2);
    /// ```
    pub fn new(migrations: Vec<Migration>) -> Result<Self> {
        if migrations.is_empty() {
            return Ok(Self { migrations: vec![] });
        }

        // Validate the set in the order provided: callers must declare
        // migrations in strictly increasing version order. We validate rather
        // than silently reorder so a mis-declared set is a loud error, not a
        // surprise at apply time. A duplicate is reported distinctly from a
        // plain out-of-order pair for a clearer diagnostic.
        for i in 1..migrations.len() {
            let (prev, cur) = (migrations[i - 1].version, migrations[i].version);
            if cur == prev {
                return Err(DbError::Config(format!(
                    "duplicate migration version: {cur}"
                )));
            }
            if cur < prev {
                return Err(DbError::Config(format!(
                    "migration versions must be strictly increasing: {cur} after {prev}"
                )));
            }
        }

        Ok(Self { migrations })
    }

    /// Returns the migrations in version order.
    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }

    /// Returns the highest version in the set, or 0 if empty.
    pub fn max_version(&self) -> u32 {
        self.migrations.last().map(|m| m.version).unwrap_or(0)
    }
}

/// Trait that a database backend must implement to support migrations.
///
/// This trait is backend-agnostic and contains no SQLite-specific types.
/// Backends implement this trait to provide:
/// - Reading the currently applied migration version
/// - Applying a migration's SQL and recording its version
///
/// # Examples
///
/// ```
/// use nxm_db::{migrate::MigrationBackend, Result};
///
/// struct MyBackend;
///
/// impl MigrationBackend for MyBackend {
///     fn current_version(&self) -> Result<u32> { Ok(0) }
///     fn apply_migration(&self, version: u32, sql: &str) -> Result<()> { Ok(()) }
/// }
/// ```
pub trait MigrationBackend: Send + Sync {
    /// Returns the current applied migration version.
    ///
    /// Returns 0 if no migrations have been applied yet.
    fn current_version(&self) -> Result<u32>;

    /// Applies a migration and records its version.
    ///
    /// The backend should execute the SQL and persist the version
    /// atomically (e.g., in a transaction).
    fn apply_migration(&self, version: u32, sql: &str) -> Result<()>;
}

pub mod sqlite;

/// Runs all pending migrations for the given backend.
///
/// Applies migrations in version order, skipping any that are already
/// applied (version <= current_version). Returns an error if any
/// migration fails to apply.
///
/// # Examples
///
/// ```
/// use nxm_db::migrate::{Migration, MigrationSet, run_migrations};
///
/// let migrations = vec![
///     Migration::new(1, "first", "CREATE TABLE a;"),
/// ];
/// let set = MigrationSet::new(migrations).unwrap();
/// // run_migrations(&backend, &set);
/// ```
pub fn run_migrations(backend: &dyn MigrationBackend, set: &MigrationSet) -> Result<()> {
    let started = Instant::now();
    debug!(target: "nxm_db::migrate", "ENTER run_migrations");

    let current = backend.current_version()?;
    debug!(target: "nxm_db::migrate", current_version = current, "current version");

    for migration in set.migrations() {
        if migration.version <= current {
            debug!(
                target: "nxm_db::migrate",
                version = migration.version,
                "skipping already applied migration"
            );
            continue;
        }

        debug!(
            target: "nxm_db::migrate",
            version = migration.version,
            name = %migration.name,
            "applying migration"
        );
        backend.apply_migration(migration.version, &migration.up)?;
    }

    info!(
        target: "nxm_db::migrate",
        elapsed_ms = started.elapsed().as_millis(),
        "EXIT run_migrations"
    );
    Ok(())
}
