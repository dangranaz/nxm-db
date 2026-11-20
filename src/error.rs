//! Unified error type for the datasource layer.
//!
//! Kept small and transport-agnostic: products map these into their own error
//! surfaces. No backend-specific leakage in the public shape.

use thiserror::Error;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, DbError>;

/// Errors raised while configuring or opening a datasource.
#[derive(Debug, Error, Clone)]
pub enum DbError {
    /// The configuration was missing or malformed.
    #[error("invalid db config: {0}")]
    Config(String),

    /// The requested backend was not compiled in (feature flag off).
    #[error("backend '{0}' is not enabled (missing cargo feature)")]
    BackendNotEnabled(String),

    /// The backend is enabled but its implementation is not available yet.
    #[error("backend '{0}' is enabled but not implemented")]
    BackendNotImplemented(String),

    /// Opening the connection / pool failed.
    #[error("connection failed: {0}")]
    Connection(String),

    /// A migration step failed.
    #[error("migration failed: {0}")]
    Migration(String),

    /// SQLx error (stored as string since sqlx::Error is not Clone).
    #[error("sqlx error: {0}")]
    Sqlx(String),
}

impl From<sqlx::Error> for DbError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error.to_string())
    }
}

#[cfg(feature = "mongodb")]
impl From<mongodb::error::Error> for DbError {
    fn from(error: mongodb::error::Error) -> Self {
        Self::Connection(error.to_string())
    }
}
