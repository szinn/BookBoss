//! API-level error types for infrastructure concerns.

use bb_core::Error as CoreError;

/// Infrastructure errors specific to the API layer.
#[derive(Debug, thiserror::Error)]
pub enum FrontendError {
    #[error("Dioxus error: {0}")]
    DioxusError(String),
}

impl From<FrontendError> for CoreError {
    fn from(err: FrontendError) -> Self {
        CoreError::FrontendError(err.to_string())
    }
}
