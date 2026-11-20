use nxm_db::{open_from, Backend, DbConfig, DbConfigSource, Result};
use nxm_logging::{init, LogConfig, LogFilter};

struct FixedSource(DbConfig);

impl DbConfigSource for FixedSource {
    fn db_config(&self) -> Result<DbConfig> {
        Ok(self.0.clone())
    }
}

#[test]
fn logging_contract_init_first_wins() {
    // 1. primo init → Ok
    init(LogConfig {
        filter: LogFilter("nxm_db=debug".into()),
        ..Default::default()
    })
    .expect("first init must succeed");

    // 2. il runtime logga con il subscriber globale installato → funziona
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let url = std::env::temp_dir()
        .join(format!(
            "nxm_db-init-{}-{nonce}/data.db",
            std::process::id()
        ))
        .to_string_lossy()
        .into_owned();
    let config = DbConfig {
        backend: Backend::Sqlite,
        url,
        tenant: "default".to_string(),
    };
    let handle = open_from(&FixedSource(config)).expect("open_from works under global subscriber");
    drop(handle);

    // 3. secondo init → Err ("first wins")
    assert!(init(LogConfig::default()).is_err());
}
