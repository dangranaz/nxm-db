use nxm_config::{ConfigFormat, McpConfig};
use nxm_db::{Backend, DbConfig, DbError};
use std::path::PathBuf;

fn mcp(format: ConfigFormat, raw: &str) -> McpConfig {
    McpConfig {
        server_name: "test".to_string(),
        path: PathBuf::from("/tmp/test_config"),
        format,
        raw: raw.to_string(),
    }
}

#[test]
fn from_mcp_parses_json_db_section() {
    let cfg = mcp(
        ConfigFormat::Json,
        r#"{"db": {"backend": "sqlite", "url": "~/.nxm/t/t.db", "tenant": "acme"}}"#,
    );
    let db = DbConfig::from_mcp(&cfg).expect("valid json db section parses");
    assert_eq!(db.backend, Backend::Sqlite);
    assert_eq!(db.url, "~/.nxm/t/t.db");
    assert_eq!(db.tenant, "acme");
}

#[test]
fn from_mcp_parses_toml_db_section() {
    let cfg = mcp(
        ConfigFormat::Toml,
        "[db]\nbackend = \"sqlite\"\nurl = \"~/.nxm/t/t.db\"\ntenant = \"acme\"\n",
    );
    let db = DbConfig::from_mcp(&cfg).expect("valid toml db section parses");
    assert_eq!(db.backend, Backend::Sqlite);
    assert_eq!(db.url, "~/.nxm/t/t.db");
    assert_eq!(db.tenant, "acme");
}

#[test]
fn from_mcp_missing_db_section_errors_json() {
    let cfg = mcp(ConfigFormat::Json, "{}");
    assert!(matches!(DbConfig::from_mcp(&cfg), Err(DbError::Config(_))));
}

#[test]
fn from_mcp_missing_db_section_errors_toml() {
    let cfg = mcp(ConfigFormat::Toml, "other = 1\n");
    assert!(matches!(DbConfig::from_mcp(&cfg), Err(DbError::Config(_))));
}

#[test]
fn from_mcp_invalid_backend_errors() {
    let cfg = mcp(
        ConfigFormat::Json,
        r#"{"db": {"backend": "postgres", "url": "x"}}"#,
    );
    assert!(matches!(DbConfig::from_mcp(&cfg), Err(DbError::Config(_))));
}

#[test]
fn from_mcp_db_wrong_shape_errors() {
    let cfg = mcp(ConfigFormat::Json, r#"{"db": "oops"}"#);
    assert!(matches!(DbConfig::from_mcp(&cfg), Err(DbError::Config(_))));
}

#[test]
fn from_mcp_malformed_raw_errors() {
    let cfg = mcp(ConfigFormat::Json, "not json {");
    assert!(matches!(DbConfig::from_mcp(&cfg), Err(DbError::Config(_))));
}
