use nxm_db::{Backend, DbConfig};

#[test]
fn default_is_local_sqlite_single_tenant() {
    let c = DbConfig::default();
    assert_eq!(c.backend, Backend::Sqlite);
    assert_eq!(c.tenant, "default");
}

#[test]
fn backend_serde_is_lowercase() {
    let json = serde_json::to_string(&Backend::Sqlite).unwrap();
    assert_eq!(json, "\"sqlite\"");
}
