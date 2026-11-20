use nxm_db::migrate::{Migration, MigrationSet};
use nxm_db::{DbError, Result};

#[test]
fn migration_set_preserves_valid_order_and_rejects_unordered() {
    let set = MigrationSet::new(vec![
        Migration::new(1, "first", "CREATE TABLE a ();"),
        Migration::new(2, "second", "CREATE TABLE b ();"),
        Migration::new(3, "third", "CREATE TABLE c ();"),
    ])
    .expect("valid monotonic set");
    assert_eq!(set.migrations().len(), 3);
    assert_eq!(set.max_version(), 3);
}

#[test]
fn non_monotonic_version_is_rejected() {
    let result = MigrationSet::new(vec![
        Migration::new(2, "second", "CREATE TABLE b ();"),
        Migration::new(1, "first", "CREATE TABLE a ();"),
    ]);
    assert!(result.is_err(), "non-monotonic versions must be rejected");
    match result {
        Err(DbError::Config(msg)) => {
            assert!(msg.contains("strictly increasing"));
        }
        Err(other) => panic!("expected DbError::Config, got {:?}", other),
        Ok(_) => panic!("expected an error"),
    }
}

#[test]
fn duplicate_version_is_rejected() {
    let result = MigrationSet::new(vec![
        Migration::new(1, "first", "CREATE TABLE a ();"),
        Migration::new(1, "dup", "CREATE TABLE b ();"),
    ]);
    assert!(result.is_err(), "duplicate versions must be rejected");
    match result {
        Err(DbError::Config(msg)) => {
            assert!(msg.contains("duplicate migration version"));
        }
        Err(other) => panic!("expected DbError::Config, got {:?}", other),
        Ok(_) => panic!("expected an error"),
    }
}

#[test]
fn runner_invokes_backend_in_order() {
    use nxm_db::migrate::MigrationBackend;

    struct FakeBackend {
        applied: std::sync::Mutex<Vec<u32>>,
    }

    impl MigrationBackend for FakeBackend {
        fn current_version(&self) -> Result<u32> {
            Ok(0)
        }

        fn apply_migration(&self, version: u32, sql: &str) -> Result<()> {
            self.applied.lock().unwrap().push(version);
            let _ = sql;
            Ok(())
        }
    }

    let backend = FakeBackend {
        applied: std::sync::Mutex::new(Vec::new()),
    };
    let set = MigrationSet::new(vec![
        Migration::new(1, "a", "SQL1"),
        Migration::new(2, "b", "SQL2"),
        Migration::new(3, "c", "SQL3"),
    ])
    .expect("valid set");

    nxm_db::migrate::run_migrations(&backend, &set).expect("run ok");

    let applied = backend.applied.lock().unwrap().clone();
    assert_eq!(applied, vec![1, 2, 3], "migrations applied in order");
}

#[test]
fn already_applied_migrations_are_skipped() {
    use nxm_db::migrate::MigrationBackend;

    struct FakeBackend {
        current: std::sync::Mutex<u32>,
        applied: std::sync::Mutex<Vec<u32>>,
    }

    impl MigrationBackend for FakeBackend {
        fn current_version(&self) -> Result<u32> {
            Ok(*self.current.lock().unwrap())
        }

        fn apply_migration(&self, version: u32, sql: &str) -> Result<()> {
            self.applied.lock().unwrap().push(version);
            let _ = sql;
            Ok(())
        }
    }

    let backend = FakeBackend {
        current: std::sync::Mutex::new(2),
        applied: std::sync::Mutex::new(Vec::new()),
    };
    let set = MigrationSet::new(vec![
        Migration::new(1, "a", "SQL1"),
        Migration::new(2, "b", "SQL2"),
        Migration::new(3, "c", "SQL3"),
    ])
    .expect("valid set");

    nxm_db::migrate::run_migrations(&backend, &set).expect("run ok");

    let applied = backend.applied.lock().unwrap().clone();
    assert_eq!(applied, vec![3], "only pending migration applied");
}

#[test]
fn empty_registry_is_noop() {
    use nxm_db::migrate::MigrationBackend;

    struct FakeBackend;

    impl MigrationBackend for FakeBackend {
        fn current_version(&self) -> Result<u32> {
            Ok(0)
        }

        fn apply_migration(&self, _version: u32, _sql: &str) -> Result<()> {
            unreachable!("no migration should be applied for empty set")
        }
    }

    let backend = FakeBackend;
    let set = MigrationSet::new(vec![]).expect("empty set is valid");

    nxm_db::migrate::run_migrations(&backend, &set).expect("run ok");
}

#[test]
fn all_applied_is_noop() {
    use nxm_db::migrate::MigrationBackend;

    struct FakeBackend {
        current: std::sync::Mutex<u32>,
        applied: std::sync::Mutex<Vec<u32>>,
    }

    impl MigrationBackend for FakeBackend {
        fn current_version(&self) -> Result<u32> {
            Ok(*self.current.lock().unwrap())
        }

        fn apply_migration(&self, version: u32, sql: &str) -> Result<()> {
            self.applied.lock().unwrap().push(version);
            let _ = sql;
            Ok(())
        }
    }

    let backend = FakeBackend {
        current: std::sync::Mutex::new(5),
        applied: std::sync::Mutex::new(Vec::new()),
    };
    let set = MigrationSet::new(vec![
        Migration::new(1, "a", "SQL1"),
        Migration::new(2, "b", "SQL2"),
        Migration::new(3, "c", "SQL3"),
    ])
    .expect("valid set");

    nxm_db::migrate::run_migrations(&backend, &set).expect("run ok");

    let applied = backend.applied.lock().unwrap().clone();
    assert!(
        applied.is_empty(),
        "all migrations already applied, none run"
    );
}
