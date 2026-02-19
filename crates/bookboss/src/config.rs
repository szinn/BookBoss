use bb_api::ApiConfig;
use bb_database::DatabaseConfig;
use bb_frontend::FrontendConfig;
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub frontend: FrontendConfig,
}

impl Config {
    pub fn load() -> Result<Config, Error> {
        let config = config::Config::builder()
            .add_source(config::Environment::with_prefix("BOOKBOSS").try_parsing(true).separator("__"))
            .build()?;

        let config: Config = config.try_deserialize()?;

        Ok(config)
    }
}
