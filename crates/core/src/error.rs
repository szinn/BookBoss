/// Categorizes errors for response mapping in adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Resource not found.
    NotFound,
    /// Resource conflict, e.g., optimistic locking failure.
    Conflict,
    /// Invalid input or constraint violation.
    InvalidInput,
    /// Malformed request data.
    BadRequest,
    /// Internal or infrastructure error.
    Internal,
    /// Service temporarily unavailable (transient — DB or storage unreachable).
    ServiceUnavailable,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Invalid ID: {0}")]
    InvalidId(u64),

    #[error("Invalid page size: {0}")]
    InvalidPageSize(u64),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid transaction type")]
    InvalidTransactionType,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Infrastructure error: {0}")]
    Infrastructure(String),

    /// Storage system unreachable (NFS gone, library path missing). Transient.
    #[error("Storage unavailable: {0}")]
    StorageUnavailable(String),

    #[error("Frontend error: {0}")]
    FrontendError(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error(transparent)]
    RepositoryError(#[from] RepositoryError),

    #[cfg(any(test, feature = "test-support"))]
    #[error("Mock not configured: {0}")]
    MockNotConfigured(&'static str),
}

impl Error {
    /// Returns the error kind for response mapping in adapters.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::InvalidId(_) | Self::InvalidPageSize(_) | Self::InvalidToken(_) => ErrorKind::BadRequest,
            Self::Validation(_) => ErrorKind::InvalidInput,
            Self::InvalidTransactionType | Self::Infrastructure(_) | Self::CryptoError(_) => ErrorKind::Internal,
            Self::StorageUnavailable(_) => ErrorKind::ServiceUnavailable,
            Self::RepositoryError(e) => e.kind(),
            Self::FrontendError(_) => ErrorKind::Internal,
            #[cfg(any(test, feature = "test-support"))]
            Self::MockNotConfigured(_) => ErrorKind::Internal,
        }
    }

    /// Returns `true` for errors caused by transient infrastructure failures
    /// (DB connectivity loss, NFS unavailable). The server can recover without
    /// a restart; subsystems should retry rather than propagating these.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::RepositoryError(RepositoryError::Connection(_) | RepositoryError::Busy(_)) | Self::StorageUnavailable(_)
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RepositoryError {
    #[error("Constraint Error - {0}")]
    Constraint(String),

    #[error("Conflict Error")]
    Conflict,

    #[error("Not found")]
    NotFound,

    #[error("Read-only Transaction")]
    ReadOnly,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Query canceled")]
    QueryCanceled,

    /// Database connection lost (DNS failure, network partition, pool
    /// exhausted). Transient — callers should retry.
    #[error("Connection Error: {0}")]
    Connection(String),

    /// Database temporarily locked by another writer (e.g. SQLite
    /// SQLITE_BUSY/SQLITE_LOCKED). Transient — callers should retry.
    #[error("Database busy: {0}")]
    Busy(String),
}

impl RepositoryError {
    /// Returns the error kind for response mapping in adapters.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::NotFound => ErrorKind::NotFound,
            Self::Conflict => ErrorKind::Conflict,
            Self::Constraint(_) => ErrorKind::InvalidInput,
            Self::ReadOnly | Self::Database(_) | Self::QueryCanceled => ErrorKind::Internal,
            Self::Connection(_) | Self::Busy(_) => ErrorKind::ServiceUnavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_transient_connection_error() {
        let e = Error::RepositoryError(RepositoryError::Connection("timeout".into()));
        assert!(e.is_transient());
    }

    #[test]
    fn is_transient_storage_unavailable() {
        let e = Error::StorageUnavailable("NFS gone".into());
        assert!(e.is_transient());
    }

    #[test]
    fn is_transient_returns_false_for_other_errors() {
        assert!(!Error::Infrastructure("bad".into()).is_transient());
        assert!(!Error::RepositoryError(RepositoryError::NotFound).is_transient());
        assert!(!Error::RepositoryError(RepositoryError::Constraint("dup".into())).is_transient());
        assert!(!Error::RepositoryError(RepositoryError::Database("boom".into())).is_transient());
    }

    #[test]
    fn connection_error_kind_is_service_unavailable() {
        let e = RepositoryError::Connection("x".into());
        assert_eq!(e.kind(), ErrorKind::ServiceUnavailable);
    }

    #[test]
    fn storage_unavailable_kind_is_service_unavailable() {
        let e = Error::StorageUnavailable("x".into());
        assert_eq!(e.kind(), ErrorKind::ServiceUnavailable);
    }

    #[test]
    fn is_transient_busy_error() {
        let e = Error::RepositoryError(RepositoryError::Busy("database is locked".into()));
        assert!(e.is_transient());
    }

    #[test]
    fn busy_kind_is_service_unavailable() {
        let e = RepositoryError::Busy("database is locked".into());
        assert_eq!(e.kind(), ErrorKind::ServiceUnavailable);
    }
}
