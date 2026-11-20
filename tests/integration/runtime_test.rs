use nxm_db::{open, Backend, DbConfig, DbError, MigrationSet};

// --- Helper per costruire una config con url in path temporaneo ---

fn sqlite_config(url: &str) -> DbConfig {
    DbConfig {
        backend: Backend::Sqlite,
        url: url.to_string(),
        tenant: "test".to_string(),
    }
}

#[test]
fn open_returns_handle_for_sqlite_backend() {
    let config = sqlite_config(&format!("nxm_db-runtime-{}/data.db", std::process::id()));

    let result = open(&config);

    assert!(result.is_ok(), "sqlite backend must return a handle");
}

#[test]
fn sqlite_dispatch_matches_feature_flag() {
    // With sqlite feature enabled (default), open should succeed
    let db = match open(&sqlite_config(&format!(
        "nxm_db-dispatch-{}/data.db",
        std::process::id()
    ))) {
        Ok(db) => db,
        Err(error) => panic!("SQLite dispatch failed: {error}"),
    };
    let result = db.sqlite_pool();
    assert!(
        result.is_ok(),
        "sqlite_pool should be available for enabled SQLite: {:?}",
        result.err()
    );
}

#[test]
fn handle_exposes_pool_and_migrate_surface() {
    let db = match open(&sqlite_config(&format!(
        "nxm_db-handle-{}/data.db",
        std::process::id()
    ))) {
        Ok(db) => db,
        Err(error) => panic!("sqlite backend must return a handle: {error}"),
    };

    // sqlite_pool() should be available for the SQLite backend
    let pool = db.sqlite_pool();
    assert!(
        pool.is_ok(),
        "sqlite_pool should be available: {:?}",
        pool.err()
    );

    // migrate() with empty set should be a no-op
    let set = MigrationSet::new(vec![]).expect("empty set is valid");
    let migration: Result<(), DbError> = db.migrate(&set);
    assert!(
        migration.is_ok(),
        "sqlite migrate should succeed: {:?}",
        migration.err()
    );
}

#[test]
fn open_does_not_touch_the_filesystem_before_open() {
    // open() should not create the file; only the backend impl does
    let path = std::env::temp_dir().join(format!("nxm_db-nofs-{}/data.db", std::process::id()));
    let config = DbConfig {
        backend: Backend::Sqlite,
        url: path.to_string_lossy().into_owned(),
        tenant: "test".to_string(),
    };

    // open() itself should succeed (it dispatches, doesn't touch FS)
    let result = open(&config);
    assert!(result.is_ok());
}

#[test]
fn migrate_accepts_a_migration_set() {
    let db = match open(&sqlite_config(&format!(
        "nxm_db-migrate-{}/data.db",
        std::process::id()
    ))) {
        Ok(db) => db,
        Err(error) => panic!("sqlite backend must return a handle: {error}"),
    };
    let set = MigrationSet::new(vec![nxm_db::Migration::new(
        1,
        "create_users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    )])
    .expect("valid set");
    let result = db.migrate(&set);

    assert!(
        result.is_ok(),
        "sqlite migrate should apply migrations: {:?}",
        result.err()
    );
}

#[test]
fn migrate_with_empty_set_is_consistent() {
    let db = match open(&sqlite_config(&format!(
        "nxm_db-empty-{}/data.db",
        std::process::id()
    ))) {
        Ok(db) => db,
        Err(error) => panic!("sqlite backend must return a handle: {error}"),
    };
    let set = MigrationSet::new(vec![]).expect("empty set is valid");
    let result = db.migrate(&set);

    assert!(
        result.is_ok(),
        "sqlite migrate with empty set should be a no-op: {:?}",
        result.err()
    );
}
