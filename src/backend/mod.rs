//! Backend implementations for different database engines.
//!
//! The public edition is sqlite-only.

#[cfg(feature = "sqlite")]
pub mod sqlite;
