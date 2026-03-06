use std::sync::Arc;

use bb_core::{
    Error, RepositoryError,
    import::ImportJobId,
    jobs::{Enqueueable, JobHandler},
    pipeline::PipelineService,
    repository::{RepositoryService, read_only_transaction},
};
use serde::{Deserialize, Serialize};

/// Payload for a `process_import` job. Contains only the import job id; the
/// handler fetches the full `ImportJob` record before calling the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessImportPayload {
    pub import_job_id: ImportJobId,
}

impl Enqueueable for ProcessImportPayload {
    const JOB_TYPE: &'static str = "process_import";
    const DEFAULT_PRIORITY: i16 = 1;
}

/// Handles `process_import` jobs by fetching the `ImportJob` and running it
/// through the acquisition pipeline.
///
/// `PipelineService::process_job` is responsible for all status transitions
/// and DB writes — the handler does not write the updated job itself.
pub struct ProcessImportHandler {
    repository_service: Arc<RepositoryService>,
    pipeline: Arc<dyn PipelineService>,
}

impl ProcessImportHandler {
    pub fn new(repository_service: Arc<RepositoryService>, pipeline: Arc<dyn PipelineService>) -> Self {
        Self { repository_service, pipeline }
    }
}

impl JobHandler for ProcessImportHandler {
    const JOB_TYPE: &'static str = "process_import";
    type Payload = ProcessImportPayload;

    async fn handle(&self, p: ProcessImportPayload) -> Result<(), Error> {
        let import_job_id = p.import_job_id;
        let repo = self.repository_service.clone();
        let repository = self.repository_service.repository().clone();

        let job = read_only_transaction(&*repository, |tx| {
            let repo = repo.clone();
            Box::pin(async move { repo.import_job_repository().find_by_id(tx, import_job_id).await })
        })
        .await?;

        let job = job.ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        self.pipeline.process_job(job).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serde_roundtrip() {
        let payload = ProcessImportPayload { import_job_id: 42 };
        let json = serde_json::to_value(&payload).unwrap();
        let back: ProcessImportPayload = serde_json::from_value(json).unwrap();
        assert_eq!(back.import_job_id, 42);
    }

    #[test]
    fn payload_job_type_and_priority() {
        assert_eq!(ProcessImportPayload::JOB_TYPE, "process_import");
        assert_eq!(ProcessImportPayload::DEFAULT_PRIORITY, 1);
    }
}
