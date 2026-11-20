use nxm_db::{open, open_from, Backend, DbConfig, DbConfigSource, DbError, Result};
use std::sync::atomic::{AtomicUsize, Ordering};

// --- Fake source implementing the DbConfigSource port ---

struct FakeSource {
    config: Option<DbConfig>,
    error: Option<DbError>,
    calls: AtomicUsize,
}

impl FakeSource {
    fn new(config: Result<DbConfig>) -> Self {
        match config {
            Ok(c) => Self {
                config: Some(c),
                error: None,
                calls: AtomicUsize::new(0),
            },
            Err(e) => Self {
                config: None,
                error: Some(e),
                calls: AtomicUsize::new(0),
            },
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl DbConfigSource for FakeSource {
    fn db_config(&self) -> Result<DbConfig> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match (&self.config, &self.error) {
            (Some(c), None) => Ok(c.clone()),
            (None, Some(e)) => Err(e.clone()),
            _ => unreachable!("fake source holds either config or error"),
        }
    }
}

fn tmp_db_path() -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir()
        .join(format!(
            "nxm_db-config-source-{}-{nonce}/data.db",
            std::process::id()
        ))
        .to_string_lossy()
        .into_owned()
}

// --- Test 1: trait object resolves to a known DbConfig ---

#[test]
fn config_source_trait_object_resolves() {
    let expected = DbConfig {
        backend: Backend::Sqlite,
        url: tmp_db_path(),
        tenant: "tenant-a".to_string(),
    };
    let source = FakeSource::new(Ok(expected.clone()));

    let config = source.db_config().expect("source should resolve");

    assert_eq!(config, expected);
    assert_eq!(source.calls(), 1);
}

// --- Test 2: open_from calls the source and passes the config to open ---

#[test]
fn open_from_uses_source_config() {
    let config = DbConfig {
        backend: Backend::Sqlite,
        url: tmp_db_path(),
        tenant: "tenant-b".to_string(),
    };
    let source = FakeSource::new(Ok(config.clone()));

    let from_source = open_from(&source);
    let direct = open(&config);

    assert!(
        from_source.is_ok(),
        "open_from should open the source's backend: {:?}",
        from_source.err()
    );
    assert!(
        direct.is_ok(),
        "open should succeed with the same config: {:?}",
        direct.err()
    );
    assert_eq!(source.calls(), 1, "source should be queried exactly once");
}

// --- Test 3: open_from propagates source error without calling open ---

#[test]
fn open_from_propagates_source_error() {
    let source_error = DbError::Config("source rejected config".to_string());
    let source = FakeSource::new(Err(source_error.clone()));

    let result = open_from(&source);

    assert!(
        result.is_err(),
        "open_from should propagate the source error"
    );
    match result {
        Err(DbError::Config(msg)) => assert_eq!(msg, "source rejected config"),
        Err(other) => panic!("expected DbError::Config, got {:?}", other),
        Ok(_) => panic!("expected an error"),
    }
    // db_config was called once; open was never reached because the source
    // error propagated before open could be invoked.
    assert_eq!(source.calls(), 1);
}
