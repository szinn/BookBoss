use tonic::{Request, Response, Status};

use crate::grpc::{
    error::map_core_error,
    system_proto::{StatusRequest, StatusResponse, system_service_server::SystemService},
};

pub(crate) struct GrpcSystemService;

impl GrpcSystemService {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[tonic::async_trait]
impl SystemService for GrpcSystemService {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn status(&self, request: Request<StatusRequest>) -> Result<Response<StatusResponse>, Status> {
        let response = handler::status(request.into_inner()).await.map_err(map_core_error)?;
        Ok(Response::new(response))
    }
}

pub(crate) mod handler {
    use bb_core::Error;

    use crate::grpc::system_proto::{StatusRequest, StatusResponse};

    pub(crate) async fn status(_request: StatusRequest) -> Result<StatusResponse, Error> {
        Ok(StatusResponse { status: "Running".to_string() })
    }
}

#[cfg(test)]
mod tests {
    use tonic::Request;

    use super::{GrpcSystemService, handler};
    use crate::grpc::system_proto::{StatusRequest, system_service_server::SystemService};

    // ===================
    // Tests: handler::status
    // ===================
    #[tokio::test]
    async fn test_handler_status_success() {
        let request = StatusRequest {};

        let result = handler::status(request).await.unwrap();

        assert_eq!(result.status, "Running");
    }

    // ===================
    // Tests: GrpcSystemService trait implementation
    // ===================
    #[tokio::test]
    async fn test_grpc_service_status() {
        let service = GrpcSystemService::new();

        let request = Request::new(StatusRequest {});

        let response = service.status(request).await.unwrap();
        let status_response = response.into_inner();

        assert_eq!(status_response.status, "Running");
    }
}

pub mod api {
    use bb_core::Error;

    use crate::{
        ApiError,
        grpc::system_proto::{StatusRequest, StatusResponse, system_service_client::SystemServiceClient},
    };

    #[tracing::instrument(level = "trace")]
    pub async fn status(host: &str, port: u16) -> Result<String, Error> {
        let server = format!("http://{}:{}", host, port);
        let mut client = SystemServiceClient::connect(server)
            .await
            .map_err(|e| Error::from(ApiError::GrpcClient(e.to_string())))?;

        let request = tonic::Request::new(StatusRequest {});
        let response: StatusResponse = client
            .status(request)
            .await
            .map_err(|e| Error::from(ApiError::GrpcClient(e.to_string())))?
            .into_inner();

        Ok(response.status)
    }
}
