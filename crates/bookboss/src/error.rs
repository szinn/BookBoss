use config::ConfigError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Any(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error(transparent)]
    ConfigError(#[from] ConfigError),
}
