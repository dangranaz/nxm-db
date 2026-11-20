#[cfg(feature = "sqlite")]
mod sqlite_tests {
    use nxm_db::backend::sqlite::SqlitePool;
    use nxm_db::{open, Backend, DbConfig, DbError, Migration, MigrationSet};
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
    fn open_creates_db_file() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let config = sqlite_config(&db_path.to_string_lossy());

        let result = open(&config);

        assert!(result.is_ok(), "open should succeed: {:?}", result.err());
        assert!(db_path.exists(), "database file should be created");
    }

    #[test]
    fn tilde_in_url_is_expanded() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("tilde_test.db");
        let tilde_path = format!("~/{}", db_path.file_name().unwrap().to_string_lossy());
        let config = sqlite_config(&tilde_path);

        let result = open(&config);

        assert!(
            result.is_ok(),
            "open should succeed with tilde: {:?}",
            result.err()
        );
        let db = result.unwrap();
        let pool_result: Result<SqlitePool, DbError> = db.sqlite_pool();
        assert!(
            pool_result.is_ok(),
            "pool should work with tilde expansion: {:?}",
            pool_result.err()
        );
    }

    #[test]
    fn pool_hands_out_multiple_connections() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("pool_test.db");
        let config = sqlite_config(&db_path.to_string_lossy());

        let db = open(&config).expect("open should succeed");
        let pool_result: Result<SqlitePool, DbError> = db.sqlite_pool();

        // Pool should be available (not BackendNotImplemented)
        assert!(
            pool_result.is_ok(),
            "pool should be implemented for SQLite: {:?}",
            pool_result.err()
        );

        let pool = pool_result.unwrap();

        // Use the pool - execute a simple query (sqlx API)
        let rt = Runtime::new().unwrap();
        let _: i64 = rt
            .block_on(async { sqlx::query_scalar("SELECT 1").fetch_one(&pool).await })
            .expect("SELECT 1 on pool");
    }

    #[test]
    fn sqlite_pool_still_works() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("pool_still_works.db");
        let config = sqlite_config(&db_path.to_string_lossy());

        let db = open(&config).expect("open should succeed");
        let pool: Result<SqlitePool, DbError> = db.sqlite_pool();
        let pool = pool.expect("sqlite_pool should be available");

        let rt = Runtime::new().unwrap();
        let value: i64 = rt
            .block_on(async { sqlx::query_scalar("SELECT 1").fetch_one(&pool).await })
            .expect("SELECT 1");
        assert_eq!(value, 1);
    }

    #[test]
    fn unopenable_path_errors_connection() {
        // Try to open a database in a non-existent, non-writable directory
        let config = sqlite_config("/non/existent/path/that/cannot/be/created/db.sqlite");

        let result = open(&config);

        assert!(result.is_err(), "open should fail for unopenable path");
        match result {
            Err(DbError::Connection(msg)) => {
                assert!(!msg.is_empty(), "error message should not be empty");
            }
            Err(other) => panic!("expected DbError::Connection, got {:?}", other),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn migrate_runs_the_runner_for_sqlite() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("migrate_test.db");
        let config = sqlite_config(&db_path.to_string_lossy());

        let db = open(&config).expect("open should succeed");
        let set = MigrationSet::new(vec![Migration::new(
            1,
            "users",
            "CREATE TABLE users (id INTEGER PRIMARY KEY);",
        )])
        .expect("valid set");

        // migrate() is no longer a stub for SQLite: it runs the real runner.
        assert!(db.migrate(&set).is_ok(), "migrate should apply migrations");
        let pool: Result<SqlitePool, DbError> = db.sqlite_pool();
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
}
