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

    #[error("Frontend error: {0}")]
    FrontendError(String),

    #[error(transparent)]
    RepositoryError(#[from] RepositoryError),

    #[cfg(any(test, feature = "test-support"))]
    #[error("Mock not configured: {0}")]
    MockNotConfigured(&'static str),
}

impl Error {
    /// Returns the error kind for response mapping in adapters.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::InvalidId(_) | Error::InvalidPageSize(_) | Error::InvalidToken(_) => ErrorKind::BadRequest,
            Error::Validation(_) => ErrorKind::InvalidInput,
            Error::InvalidTransactionType | Error::Infrastructure(_) => ErrorKind::Internal,
            Error::RepositoryError(e) => e.kind(),
            Error::FrontendError(_) => ErrorKind::Internal,
            #[cfg(any(test, feature = "test-support"))]
            Error::MockNotConfigured(_) => ErrorKind::Internal,
        }
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
}

impl RepositoryError {
    /// Returns the error kind for response mapping in adapters.
    pub fn kind(&self) -> ErrorKind {
        match self {
            RepositoryError::NotFound => ErrorKind::NotFound,
            RepositoryError::Conflict => ErrorKind::Conflict,
            RepositoryError::Constraint(_) => ErrorKind::InvalidInput,
            RepositoryError::ReadOnly | RepositoryError::Database(_) | RepositoryError::QueryCanceled => ErrorKind::Internal,
        }
    }
}
