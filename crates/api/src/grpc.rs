use std::sync::Arc;

use bb_core::{CoreServices, Error};
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle};
use tonic::transport::Server;

use crate::{ApiConfig, error::ApiError};

mod error;
pub mod system;

pub(crate) mod system_proto {
    tonic::include_proto!("bookboss.system");
}

pub(crate) struct GrpcSubsystem {
    config: ApiConfig,
    core_services: Arc<CoreServices>,
}

impl GrpcSubsystem {
    pub(crate) fn new(config: &ApiConfig, core_services: Arc<CoreServices>) -> Self {
        Self {
            config: config.to_owned(),
            core_services,
        }
    }
}

impl IntoSubsystem<Error> for GrpcSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        let addr = format!("{}:{}", self.config.grpc_listen_ip, self.config.grpc_listen_port)
            .parse()
            .map_err(|_| Error::from(ApiError::AddressParse("0.0.0.0:3001".into())))?;

        let system_service = system::GrpcSystemService::new();

        tracing::info!("listening on {}", addr);
        tokio::select! {
            _ = subsys.on_shutdown_requested() => {
                tracing::info!("GrpcSubsystem shutting down...");
            }
            _ = Server::builder()
                .add_service(system_proto::system_service_server::SystemServiceServer::new(system_service))
                .serve(addr) => {
                subsys.request_shutdown();
            }
        }

        Ok(())
    }
}
