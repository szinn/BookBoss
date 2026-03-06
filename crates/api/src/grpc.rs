use std::sync::Arc;

use bb_core::{CoreServices, Error};
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle};
use tonic::transport::Server;

use crate::{ApiConfig, error::ApiError};

mod error;
pub mod system;

pub(crate) mod system_proto {
    tonic::include_proto!("bookboss.system");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("system_descriptor");
}

pub(crate) struct GrpcSubsystem {
    config: ApiConfig,
    // core_services: Arc<CoreServices>,
}

impl GrpcSubsystem {
    pub(crate) fn new(config: &ApiConfig, _core_services: Arc<CoreServices>) -> Self {
        Self {
            config: config.to_owned(),
            // core_services,
        }
    }
}

impl IntoSubsystem<Error> for GrpcSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        let host_addr = format!("{}:{}", self.config.grpc_listen_ip, self.config.grpc_listen_port);
        let addr = host_addr.parse().map_err(|_| Error::from(ApiError::AddressParse(host_addr)))?;

        let service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(system_proto::FILE_DESCRIPTOR_SET)
            .build_v1()
            .unwrap();
        let system_service = system::GrpcSystemService::new();

        tracing::info!("listening on {}", addr);
        tokio::select! {
            _ = subsys.on_shutdown_requested() => {
                tracing::info!("GrpcSubsystem shutting down...");
            }
            _ = Server::builder()
                .add_service(service)
                .add_service(system_proto::system_service_server::SystemServiceServer::new(system_service))
                .serve(addr) => {
                subsys.request_shutdown();
            }
        }

        Ok(())
    }
}
