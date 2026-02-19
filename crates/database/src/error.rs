use bb_core::RepositoryError;
use sea_orm::{DbErr, RuntimeErr};

/// PostgreSQL error codes.
/// See: <https://www.postgresql.org/docs/current/errcodes-appendix.html>
mod pg_error_codes {
    /// 25006: read_only_sql_transaction
    /// Raised when a write operation is attempted on a read-only transaction.
    pub const READ_ONLY_SQL_TRANSACTION: &str = "25006";
    /// 23505: unique_violation
    /// Raised when a unique constraint is violated.
    pub const UNIQUE_VIOLATION: &str = "23505";
    /// 23503: foreign_key_violation
    /// Raised when a foreign key constraint is violated.
    pub const FOREIGN_KEY_VIOLATION: &str = "23503";
    /// 40001: serialization_failure
    /// Raised when a transaction cannot be serialized.
    pub const SERIALIZATION_FAILURE: &str = "40001";
    /// 57014: query_canceled
    /// Raised when a query is canceled.
    pub const QUERY_CANCELED: &str = "57014";
}

pub fn handle_dberr(error: DbErr) -> RepositoryError {
    if let DbErr::Query(RuntimeErr::SqlxError(sqlx_err)) | DbErr::Exec(RuntimeErr::SqlxError(sqlx_err)) = &error {
        if let Some(db_err) = sqlx_err.as_database_error() {
            if let Some(code) = db_err.code() {
                return match code.as_ref() {
                    pg_error_codes::READ_ONLY_SQL_TRANSACTION => RepositoryError::ReadOnly,
                    pg_error_codes::UNIQUE_VIOLATION => RepositoryError::Constraint(db_err.message().to_string()),
                    pg_error_codes::FOREIGN_KEY_VIOLATION => RepositoryError::Constraint(format!("Foreign key violation: {}", db_err.message())),
                    pg_error_codes::SERIALIZATION_FAILURE => RepositoryError::Conflict,
                    pg_error_codes::QUERY_CANCELED => {
                        tracing::warn!(error = %error, "Query canceled");
                        RepositoryError::QueryCanceled
                    }
                    _ => {
                        tracing::error!(error_code = %code, error = %error, "Unhandled database error code");
                        RepositoryError::Database(error.to_string())
                    }
                };
            }
        }
    }

    match error.sql_err() {
        Some(error) => match error {
            sea_orm::SqlErr::UniqueConstraintViolation(msg) => RepositoryError::Constraint(msg),
            _ => {
                tracing::error!("Got sql_err {:?}", error);
                RepositoryError::Database(error.to_string())
            }
        },
        _ => {
            tracing::error!("Got DbErr {:?}", error);
            RepositoryError::Database(error.to_string())
        }
    }
}
