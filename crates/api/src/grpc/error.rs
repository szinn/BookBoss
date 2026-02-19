//! gRPC error mapping from core errors to tonic Status codes.

use bb_core::{Error as CoreError, ErrorKind};
use tonic::Status;

/// Maps a core error to the appropriate tonic Status code.
pub fn map_core_error(error: CoreError) -> Status {
    let message = error.to_string();
    match error.kind() {
        ErrorKind::NotFound => Status::not_found(message),
        ErrorKind::Conflict => Status::already_exists(message),
        ErrorKind::InvalidInput => Status::invalid_argument(message),
        ErrorKind::BadRequest => Status::invalid_argument(message),
        ErrorKind::Internal => Status::internal(message),
    }
}

#[cfg(test)]
mod tests {
    use bb_core::{Error, RepositoryError};
    use tonic::Code;

    use super::map_core_error;

    #[test]
    fn test_not_found_maps_to_not_found() {
        let error = Error::RepositoryError(RepositoryError::NotFound);

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::NotFound);
        assert!(status.message().contains("Not found"));
    }

    #[test]
    fn test_conflict_maps_to_already_exists() {
        let error = Error::RepositoryError(RepositoryError::Conflict);

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::AlreadyExists);
    }

    #[test]
    fn test_constraint_maps_to_invalid_argument() {
        let error = Error::RepositoryError(RepositoryError::Constraint("duplicate email".into()));

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::InvalidArgument);
        assert!(status.message().contains("duplicate email"));
    }

    #[test]
    fn test_invalid_id_maps_to_invalid_argument() {
        let error = Error::InvalidId(0);

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::InvalidArgument);
    }

    #[test]
    fn test_invalid_page_size_maps_to_invalid_argument() {
        let error = Error::InvalidPageSize(0);

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::InvalidArgument);
    }

    #[test]
    fn test_infrastructure_error_maps_to_internal() {
        let error = Error::Infrastructure("connection refused".into());

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::Internal);
        assert!(status.message().contains("connection refused"));
    }

    #[test]
    fn test_repository_database_error_maps_to_internal() {
        let error = Error::RepositoryError(RepositoryError::Database("database error".into()));

        let status = map_core_error(error);

        assert_eq!(status.code(), Code::Internal);
    }
}
