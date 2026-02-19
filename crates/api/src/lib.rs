use std::sync::Arc;

use bb_core::{CoreServices, Error};
use serde::Deserialize;
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle};

use crate::grpc::GrpcSubsystem;

mod error;
pub mod grpc;

pub use error::ApiError;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    /// IP address where the GRPC server should listen.
    /// e.g. 0.0.0.0
    /// Environment variable: BOOKBOSS__API__GRPC_LISTEN_IP
    pub grpc_listen_ip: String,

    /// Port the GRPC server should listen on.
    /// e.g. 8081
    /// Environment variable: BOOKBOSS__API__GRPC_LISTEN_PORT
    pub grpc_listen_port: u16,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            grpc_listen_ip: "0.0.0.0".to_string(),
            grpc_listen_port: 8081,
        }
    }
}

pub struct ApiSubsystem {
    config: ApiConfig,
    core_services: Arc<CoreServices>,
}

impl IntoSubsystem<Error> for ApiSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        tracing::info!("ApiSubsystem starting...");
        let grpc_subsystem = GrpcSubsystem::new(&self.config, self.core_services.clone());

        subsys.start(SubsystemBuilder::new("Grpc", grpc_subsystem.into_subsystem()));

        tracing::info!("ApiSubsystem started");

        subsys.on_shutdown_requested().await;
        tracing::info!("ApiSubsystem shutting down");

        Ok(())
    }
}

pub fn create_api_subsystem(config: &ApiConfig, core_services: Arc<CoreServices>) -> ApiSubsystem {
    ApiSubsystem {
        config: config.to_owned(),
        core_services,
    }
}
