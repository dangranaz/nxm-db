use nxm_db::backend::sqlite::SqlitePool;
use nxm_db::{open, Backend, DbConfig, DbError, Migration, MigrationSet, Result};
use tempfile::tempdir;
use tokio::runtime::Runtime;

fn sqlite_config(url: &str) -> DbConfig {
    DbConfig {
        backend: Backend::Sqlite,
        url: url.to_string(),
        tenant: "test".to_string(),
    }
}

#[test]
fn tracking_table_created_on_first_run() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("tracking_test.db");
    let config = sqlite_config(&db_path.to_string_lossy());

    let db = open(&config).expect("open should succeed");
    let set = MigrationSet::new(vec![Migration::new(
        1,
        "users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    )])
    .expect("valid set");

    db.migrate(&set).expect("migrate should succeed");

    let pool: Result<SqlitePool> = db.sqlite_pool();
    let pool = pool.expect("pool should be available");
    let rt = Runtime::new().unwrap();
    let count: i64 = rt
        .block_on(async {
            sqlx::query_scalar("SELECT COUNT(*) FROM schema_version WHERE id = 1")
                .fetch_one(&pool)
                .await
        })
        .expect("query");
    assert_eq!(count, 1, "tracking table should exist");
}

#[test]
fn second_run_is_idempotent() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("idempotent_test.db");
    let config = sqlite_config(&db_path.to_string_lossy());

    let db = open(&config).expect("open should succeed");
    let set = MigrationSet::new(vec![Migration::new(
        1,
        "users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    )])
    .expect("valid set");

    db.migrate(&set).expect("first migrate should succeed");
    let result = db.migrate(&set);

    assert!(result.is_ok(), "second migrate should succeed (idempotent)");
}

#[test]
fn db_migrate_runs_the_runner_for_sqlite() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("db_migrate_test.db");
    let config = sqlite_config(&db_path.to_string_lossy());

    let db = open(&config).expect("open should succeed");
    let set = MigrationSet::new(vec![Migration::new(
        1,
        "users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    )])
    .expect("valid set");

    assert!(db.migrate(&set).is_ok(), "migrate should apply migrations");

    let pool: Result<SqlitePool> = db.sqlite_pool();
    let pool = pool.expect("pool should be available");
    let rt = Runtime::new().unwrap();
    let count: i64 = rt
        .block_on(async {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='users'",
            )
            .fetch_one(&pool)
            .await
        })
        .expect("query");
    assert_eq!(count, 1, "migration table should exist");
}

#[test]
fn failing_migration_rolls_back_and_errors() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("rollback_test.db");
    let config = sqlite_config(&db_path.to_string_lossy());

    let db = open(&config).expect("open should succeed");
    let set = MigrationSet::new(vec![
        Migration::new(1, "bad", "CREATE TABLE will_rollback (id INTEGER);"),
        Migration::new(2, "also_bad", "INVALID SQL HERE;;;"),
    ])
    .expect("valid set");

    let result = db.migrate(&set);

    assert!(result.is_err(), "failing migration must return an error");
    match result {
        Err(DbError::Migration(_)) => {}
        Err(other) => panic!("expected DbError::Migration, got {:?}", other),
        Ok(_) => panic!("expected an error"),
    }

    // First migration SHOULD have been applied (committed before the second failed).
    // Only the failing migration's transaction is rolled back.
    let pool: Result<SqlitePool> = db.sqlite_pool();
    let pool = pool.expect("pool should be available");
    let rt = Runtime::new().unwrap();
    let count: i64 = rt
        .block_on(async {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='will_rollback'",
            )
            .fetch_one(&pool)
            .await
        })
        .expect("query");
    assert_eq!(
        count, 1,
        "first migration table should still exist (committed before failure)"
    );
}

#[test]
fn first_run_applies_all() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("first_run_test.db");
    let config = sqlite_config(&db_path.to_string_lossy());

    let db = open(&config).expect("open should succeed");
    let set = MigrationSet::new(vec![
        Migration::new(1, "a", "CREATE TABLE a (id INTEGER);"),
        Migration::new(2, "b", "CREATE TABLE b (id INTEGER);"),
    ])
    .expect("valid set");

    assert!(db.migrate(&set).is_ok(), "first run should apply all");

    let pool: Result<SqlitePool> = db.sqlite_pool();
    let pool = pool.expect("pool should be available");
    let rt = Runtime::new().unwrap();
    let count_a: i64 = rt
        .block_on(async {
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='a'")
                .fetch_one(&pool)
                .await
        })
        .expect("query");
    let count_b: i64 = rt
        .block_on(async {
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='b'")
                .fetch_one(&pool)
                .await
        })
        .expect("query");
    assert_eq!(count_a, 1, "table a should exist");
    assert_eq!(count_b, 1, "table b should exist");
}

#[test]
fn applies_only_pending() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("pending_test.db");
    let config = sqlite_config(&db_path.to_string_lossy());

    let db = open(&config).expect("open should succeed");
    let set1 = MigrationSet::new(vec![Migration::new(1, "a", "CREATE TABLE a (id INTEGER);")])
        .expect("valid set");
    db.migrate(&set1).expect("first migrate should succeed");

    let set2 = MigrationSet::new(vec![
        Migration::new(1, "a", "CREATE TABLE a (id INTEGER);"),
        Migration::new(2, "b", "CREATE TABLE b (id INTEGER);"),
    ])
    .expect("valid set");
    db.migrate(&set2)
        .expect("second migrate should apply only pending");

    let pool: Result<SqlitePool> = db.sqlite_pool();
    let pool = pool.expect("pool should be available");
    let rt = Runtime::new().unwrap();
    let count_b: i64 = rt
        .block_on(async {
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='b'")
                .fetch_one(&pool)
                .await
        })
        .expect("query");
    assert_eq!(count_b, 1, "table b should exist after applying pending");
}
