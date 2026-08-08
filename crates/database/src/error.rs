use bb_core::RepositoryError;
use sea_orm::{DbErr, RuntimeErr};

/// `PostgreSQL` error codes.
/// See: <https://www.postgresql.org/docs/current/errcodes-appendix.html>
mod pg_error_codes {
    /// 25006: `read_only_sql_transaction`
    /// Raised when a write operation is attempted on a read-only transaction.
    pub const READ_ONLY_SQL_TRANSACTION: &str = "25006";
    /// 23505: `unique_violation`
    /// Raised when a unique constraint is violated.
    pub const UNIQUE_VIOLATION: &str = "23505";
    /// 23503: `foreign_key_violation`
    /// Raised when a foreign key constraint is violated.
    pub const FOREIGN_KEY_VIOLATION: &str = "23503";
    /// 40001: `serialization_failure`
    /// Raised when a transaction cannot be serialized.
    pub const SERIALIZATION_FAILURE: &str = "40001";
    /// 57014: `query_canceled`
    /// Raised when a query is canceled.
    pub const QUERY_CANCELED: &str = "57014";
}

/// SQLite primary and extended result codes for lock contention.
/// `sqlx-sqlite` reports the *extended* result code via `code()`.
/// See: <https://www.sqlite.org/rescode.html>
mod sqlite_error_codes {
    /// 5: `SQLITE_BUSY` — database file is locked by another connection.
    pub const BUSY: &str = "5";
    /// 261: `SQLITE_BUSY_RECOVERY` — busy while another connection recovers a
    /// WAL/journal.
    pub const BUSY_RECOVERY: &str = "261";
    /// 517: `SQLITE_BUSY_SNAPSHOT` — busy due to a stale WAL snapshot.
    pub const BUSY_SNAPSHOT: &str = "517";
    /// 773: `SQLITE_BUSY_TIMEOUT` — busy_timeout expired while waiting for a
    /// lock.
    pub const BUSY_TIMEOUT: &str = "773";
    /// 6: `SQLITE_LOCKED` — a table in the database is locked.
    pub const LOCKED: &str = "6";
    /// 262: `SQLITE_LOCKED_SHAREDCACHE` — locked due to shared cache
    /// contention.
    pub const LOCKED_SHAREDCACHE: &str = "262";
}

#[allow(clippy::needless_pass_by_value, reason = "Required for map_err")]
pub fn handle_dberr(error: DbErr) -> RepositoryError {
    // Connectivity errors: network/DNS failure, pool exhaustion, closed pool.
    // Checked before sql_err() because these need special transient handling.
    if let DbErr::Conn(RuntimeErr::SqlxError(ref sqlx_err)) = error
        && matches!(**sqlx_err, sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed)
    {
        return RepositoryError::Connection(error.to_string());
    }
    // Pool acquire failure (e.g. pool exhausted before timeout).
    if let DbErr::ConnectionAcquire(_) = &error {
        return RepositoryError::Connection(error.to_string());
    }

    // Check sql_err first — it is database-agnostic and handles common constraint
    // violations uniformly across Postgres, MySQL, and SQLite.
    if let Some(sql_err) = error.sql_err() {
        return match sql_err {
            sea_orm::SqlErr::UniqueConstraintViolation(msg) => RepositoryError::Constraint(msg),
            sea_orm::SqlErr::ForeignKeyConstraintViolation(msg) => RepositoryError::Constraint(format!("Foreign key violation: {msg}")),
            _ => {
                tracing::error!(error = ?error, "Unhandled sql_err");
                RepositoryError::Database(error.to_string())
            }
        };
    }

    // Fall back to database-specific error codes for errors not covered by sql_err
    // (read-only transactions, serialization failures, query cancellation, etc.).
    if let DbErr::Query(RuntimeErr::SqlxError(sqlx_err)) | DbErr::Exec(RuntimeErr::SqlxError(sqlx_err)) = &error
        && let Some(db_err) = sqlx_err.as_database_error()
        && let Some(code) = db_err.code()
    {
        return match code.as_ref() {
            pg_error_codes::READ_ONLY_SQL_TRANSACTION => RepositoryError::ReadOnly,
            pg_error_codes::UNIQUE_VIOLATION => RepositoryError::Constraint(db_err.message().to_string()),
            pg_error_codes::FOREIGN_KEY_VIOLATION => RepositoryError::Constraint(format!("Foreign key violation: {}", db_err.message())),
            pg_error_codes::SERIALIZATION_FAILURE => RepositoryError::Conflict,
            pg_error_codes::QUERY_CANCELED => {
                tracing::warn!(error = %error, "Query canceled");
                RepositoryError::QueryCanceled
            }
            sqlite_error_codes::BUSY
            | sqlite_error_codes::BUSY_RECOVERY
            | sqlite_error_codes::BUSY_SNAPSHOT
            | sqlite_error_codes::BUSY_TIMEOUT
            | sqlite_error_codes::LOCKED
            | sqlite_error_codes::LOCKED_SHAREDCACHE => {
                tracing::warn!(error = %error, "Database busy (transient lock contention)");
                RepositoryError::Busy(db_err.message().to_string())
            }
            _ => {
                tracing::error!(error_code = %code, error = %error, "Unhandled database error code");
                RepositoryError::Database(error.to_string())
            }
        };
    }

    tracing::error!(error = ?error, "Unhandled database error");
    RepositoryError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bb_core::Error;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, Statement};
    use sqlx::{
        Connection, Executor,
        sqlite::{SqliteConnectOptions, SqliteConnection, SqliteJournalMode},
    };

    use super::*;

    /// Reproduces the real SQLITE_BUSY condition from issue #214: one
    /// connection holds an uncommitted write transaction while a second
    /// connection attempts a concurrent write. Verifies `handle_dberr`
    /// classifies the resulting error as `RepositoryError::Busy` and that
    /// it is treated as transient, so the resilience layer retries instead
    /// of permanently killing the subsystem.
    #[tokio::test]
    async fn sqlite_busy_error_maps_to_transient_repository_error() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let db_path = dir.path().join("bb-busy-test.sqlite");

        let setup_opts = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let mut setup_conn = SqliteConnection::connect_with(&setup_opts).await.expect("open setup connection");
        setup_conn
            .execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)")
            .await
            .expect("create table");
        drop(setup_conn);

        // Holds the write lock via an uncommitted BEGIN IMMEDIATE transaction.
        let locker_opts = SqliteConnectOptions::new().filename(&db_path).journal_mode(SqliteJournalMode::Wal);
        let mut locker = SqliteConnection::connect_with(&locker_opts).await.expect("open locker connection");
        locker.execute("BEGIN IMMEDIATE").await.expect("begin immediate");
        locker.execute("INSERT INTO t (v) VALUES (1)").await.expect("locker insert");

        // Contending connection with a short busy_timeout so the test stays fast.
        let url = format!("sqlite://{}", db_path.display());
        let mut opt = ConnectOptions::new(&url);
        opt.max_connections(1).min_connections(1);
        opt.map_sqlx_sqlite_opts(|o| o.busy_timeout(Duration::from_millis(200)));
        let db = Database::connect(opt).await.expect("open contending connection");

        let result = db
            .execute_raw(Statement::from_string(db.get_database_backend(), "INSERT INTO t (v) VALUES (2)"))
            .await;

        let db_err = result.expect_err("expected SQLITE_BUSY error from contended write");
        let repo_err = handle_dberr(db_err);

        assert!(matches!(repo_err, RepositoryError::Busy(_)), "expected Busy variant, got {repo_err:?}");
        assert!(Error::from(repo_err).is_transient(), "SQLite busy errors must be treated as transient");

        drop(locker);
    }
}
