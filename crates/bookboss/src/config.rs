use std::path::PathBuf;

use bb_api::ApiConfig;
use bb_database::DatabaseConfig;
use bb_frontend::FrontendConfig;
use bb_metadata::MetadataConfig;
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub struct ImportConfig {
    pub watch_directory: PathBuf,
    #[serde(default = "ImportConfig::default_scan_interval")]
    pub scan_interval_secs: u64,
    #[serde(default = "ImportConfig::default_worker_poll_interval")]
    pub worker_poll_interval_secs: u64,
}

impl ImportConfig {
    fn default_scan_interval() -> u64 {
        60
    }

    fn default_worker_poll_interval() -> u64 {
        5
    }
}

#[derive(Debug, Deserialize)]
pub struct LibraryConfig {
    pub library_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub frontend: FrontendConfig,
    pub import: ImportConfig,
    pub library: LibraryConfig,
    #[serde(default)]
    pub metadata: MetadataConfig,
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
