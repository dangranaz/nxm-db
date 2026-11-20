//! nxm_db — datasource auto-configuration for NXM MCP servers (sqlite-only).
//!
//! Mental model: a brick like Spring Boot's starter + auto-configuration, but
//! for MCP servers. Give it a config, get back a ready backend handle
//! (SQLite) + pool + migrations + tenant. That's it.
//!
//! It is **NOT** an ORM, not a query builder, not entity abstraction. Products
//! own their schema ("schema di competenza"): they define their own repository
//! traits and backend implementations. This crate only provides the runtime
//! and configuration they sit on.
//!
//! SQLite is the only backend.

pub mod backend;
pub mod config;
pub mod error;
pub mod migrate;
pub mod runtime;

pub use config::{Backend, DbConfig, DbConfigSource};
pub use error::{DbError, Result};
pub use migrate::{run_migrations, Migration, MigrationBackend, MigrationSet};
pub use runtime::{open, open_from, sqlite_pool, Db};
