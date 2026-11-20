//! Datasource configuration — the single knob a product turns to pick a backend.
//!
//! Loaded from `~/.nxmconfig/<name>_config.{json,toml}` by `nxm_config`: the
//! `db` section (JSON key / TOML table) is extracted with `DbConfig::from_mcp`.
//! The point: switching local <-> remote is a config change, never a code
//! change — this is the 3-modes promise made real.

use crate::{DbError, Result};

use nxm_config::ConfigFormat;
use serde::{Deserialize, Serialize};

/// Which datasource backend to use.
///
/// The **public** edition is sqlite-only: SQLite is the only supported backend.
///
/// # Examples
///
/// ```
/// use nxm_db::Backend;
///
/// assert_eq!(Backend::Sqlite as u8, 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// Local file database. The only backend in the public edition.
    Sqlite,
}

impl From<nxm_config::DatabaseKind> for Backend {
    fn from(kind: nxm_config::DatabaseKind) -> Self {
        match kind {
            nxm_config::DatabaseKind::Sqlite => Backend::Sqlite,
            nxm_config::DatabaseKind::Postgres => Backend::Sqlite, // will be handled by feature flags
            nxm_config::DatabaseKind::Mongo => Backend::Sqlite,
        }
    }
}

/// The full datasource configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DbConfig {
    /// Which backend to open. Only `sqlite` is valid in the public edition.
    pub backend: Backend,

    /// Connection string / path, interpreted per backend:
    /// - sqlite:   a file path (e.g. `~/.nxm/auth/auth.db`)
    pub url: String,

    /// Tenant identifier. Present from day one so moving mode 2 -> 3
    /// (multi-tenant / privacy) is configuration, not a rewrite.
    /// Defaults to `"default"` for single-tenant / local use.
    #[serde(default = "default_tenant")]
    pub tenant: String,
}

fn default_tenant() -> String {
    "default".to_string()
}

/// A source of `DbConfig` — describes what the crate needs to obtain a
/// `DbConfig`. Products implement this for their edition of config;
/// `nxm_db` only defines the port.
///
/// The tenant is already inside `DbConfig`; multi-tenant is a config change,
/// not a separate parameter.
///
/// # Examples
///
/// ```
/// use nxm_db::{DbConfig, DbConfigSource, Result};
///
/// struct FixedSource;
///
/// impl DbConfigSource for FixedSource {
///     fn db_config(&self) -> Result<DbConfig> {
///         Ok(DbConfig::default())
///     }
/// }
///
/// let config = FixedSource.db_config().expect("config should resolve");
/// assert_eq!(config.tenant, "default");
/// ```
pub trait DbConfigSource {
    /// Resolves a `DbConfig`.
    ///
    /// # Errors
    /// Returns a [`DbError`] if the configuration cannot be resolved.
    fn db_config(&self) -> Result<DbConfig>;
}

impl Default for DbConfig {
    /// The zero-config local default: a SQLite file, single tenant.
    fn default() -> Self {
        Self {
            backend: Backend::Sqlite,
            url: "~/.nxm/shared-db/data.db".to_string(),
            tenant: default_tenant(),
        }
    }
}

impl DbConfig {
    /// Deserializes a `[db]` section from a generic `toml::Value`.
    ///
    /// The helper depends on no config loader: the caller passes the already
    /// extracted `[db]` value. [`DbConfig::from_mcp`] is the loader-based path.
    ///
    /// # Errors
    /// Returns [`DbError::Config`] if the value cannot be deserialized
    /// (e.g. unknown backend, missing mandatory field).
    ///
    /// # Examples
    ///
    /// ```
    /// use nxm_db::{Backend, DbConfig, DbError, Result};
    /// use toml::from_str;
    ///
    /// let value: toml::Value = from_str(
    ///     r#"
    ///     backend = "sqlite"
    ///     url = "~/.nxm/shared-db/data.db"
    ///     tenant = "acme"
    ///     "#,
    /// )
    /// .expect("toml snippet is valid");
    /// let config = DbConfig::from_toml(value).expect("valid section");
    /// assert_eq!(config.backend, Backend::Sqlite);
    /// assert_eq!(config.url, "~/.nxm/shared-db/data.db");
    /// assert_eq!(config.tenant, "acme");
    /// ```
    pub fn from_toml(value: toml::Value) -> Result<Self> {
        value
            .try_into()
            .map_err(|e| DbError::Config(format!("failed to parse db config section: {e}")))
    }

    /// Estrae la sezione `db` dalla config di un server MCP già caricata.
    ///
    /// Il file `~/.nxmconfig/<nome>_config.{json,toml}` viene caricato da
    /// [`nxm_config`]; questo metodo ne legge il contenuto grezzo
    /// (`cfg.raw`) nel formato dichiarato (`cfg.format`), estrae la sezione
    /// (chiave) `db` e la deserializza in un `DbConfig`.
    ///
    /// # Errors
    /// Returns [`DbError::Config`] if the raw content is malformed, the `db`
    /// section is missing, or the section cannot be deserialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use nxm_db::DbConfig;
    /// use nxm_config::{ConfigFormat, McpConfig};
    /// use std::path::PathBuf;
    ///
    /// let cfg = McpConfig {
    ///     server_name: "shared".to_string(),
    ///     path: PathBuf::from("~/.nxmconfig/shared_config.toml"),
    ///     format: ConfigFormat::Toml,
    ///     raw: "[db]\nbackend = \"sqlite\"\nurl = \"~/.nxm/shared-db/data.db\"\n".to_string(),
    /// };
    /// let config = DbConfig::from_mcp(&cfg).expect("valid db section");
    /// assert_eq!(config.tenant, "default");
    /// ```
    pub fn from_mcp(cfg: &nxm_config::McpConfig) -> Result<Self> {
        match cfg.format {
            ConfigFormat::Toml => {
                let value: toml::Value = toml::from_str(&cfg.raw)
                    .map_err(|e| DbError::Config(format!("failed to parse toml config: {e}")))?;
                let section = value.get("db").ok_or_else(|| {
                    DbError::Config("missing `db` section in toml config".to_string())
                })?;
                Self::from_toml(section.clone())
            }
            ConfigFormat::Json => {
                let value: serde_json::Value = serde_json::from_str(&cfg.raw)
                    .map_err(|e| DbError::Config(format!("failed to parse json config: {e}")))?;
                let section = value.get("db").ok_or_else(|| {
                    DbError::Config("missing `db` key in json config".to_string())
                })?;
                serde_json::from_value(section.clone())
                    .map_err(|e| DbError::Config(format!("failed to parse db config section: {e}")))
            }
        }
    }
}
