//! API-level error types for infrastructure concerns.

use bb_core::Error as CoreError;

/// Infrastructure errors specific to the API layer.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("gRPC client error: {0}")]
    GrpcClient(String),

    #[error("Failed to parse address: {0}")]
    AddressParse(String),
}

impl From<ApiError> for CoreError {
    fn from(err: ApiError) -> Self {
        CoreError::Infrastructure(err.to_string())
    }
}
