use tonic::{Request, Response, Status};

use crate::grpc::{
    error::map_core_error,
    system_proto::{StatusRequest, StatusResponse, system_service_server::SystemService},
};

/// gRPC SystemService implementation
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

    pub(crate) async fn status(request: StatusRequest) -> Result<StatusResponse, Error> {
        Ok(StatusResponse {
            answer: format!("{}: Answered", request.question),
        })
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
        let request = StatusRequest { question: "Hello".into() };

        let result = handler::status(request).await.unwrap();

        assert_eq!(result.answer, "Hello: Answered");
    }

    #[tokio::test]
    async fn test_handler_status_empty_question() {
        let request = StatusRequest { question: String::new() };

        let result = handler::status(request).await.unwrap();

        assert_eq!(result.answer, ": Answered");
    }

    #[tokio::test]
    async fn test_handler_status_long_question() {
        let long_question = "a".repeat(1000);
        let request = StatusRequest {
            question: long_question.clone(),
        };

        let result = handler::status(request).await.unwrap();

        assert_eq!(result.answer, format!("{}: Answered", long_question));
    }

    // ===================
    // Tests: GrpcSystemService trait implementation
    // ===================
    #[tokio::test]
    async fn test_grpc_service_status() {
        let service = GrpcSystemService::new();

        let request = Request::new(StatusRequest {
            question: "Test Question".into(),
        });

        let response = service.status(request).await.unwrap();
        let status_response = response.into_inner();

        assert_eq!(status_response.answer, "Test Question: Answered");
    }

    #[tokio::test]
    async fn test_grpc_service_status_with_special_characters() {
        let service = GrpcSystemService::new();

        let request = Request::new(StatusRequest {
            question: "What's the status? 🚀".into(),
        });

        let response = service.status(request).await.unwrap();
        let status_response = response.into_inner();

        assert_eq!(status_response.answer, "What's the status? 🚀: Answered");
    }
}

pub mod api {
    use bb_core::Error;

    use crate::{
        ApiError,
        grpc::system_proto::{StatusRequest, StatusResponse, system_service_client::SystemServiceClient},
    };

    #[tracing::instrument(level = "trace")]
    pub async fn status(question: String) -> Result<String, Error> {
        let mut client = SystemServiceClient::connect("http://localhost:3001")
            .await
            .map_err(|e| Error::from(ApiError::GrpcClient(e.to_string())))?;

        let request = tonic::Request::new(StatusRequest { question });
        let response: StatusResponse = client
            .status(request)
            .await
            .map_err(|e| Error::from(ApiError::GrpcClient(e.to_string())))?
            .into_inner();

        Ok(response.answer)
    }
}
