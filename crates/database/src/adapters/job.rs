// M3.16 will implement this fully. Stub exists to satisfy
// RepositoryServiceBuilder.

use bb_core::{
    Error,
    jobs::{Job, JobRepository},
    repository::Transaction,
};

pub struct JobRepositoryAdapter;

impl JobRepositoryAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl JobRepository for JobRepositoryAdapter {
    async fn enqueue_raw(&self, _transaction: &dyn Transaction, _job_type: &str, _payload: serde_json::Value, _priority: i16) -> Result<Job, Error> {
        unimplemented!("JobRepositoryAdapter: M3.16")
    }

    async fn claim_next(&self, _transaction: &dyn Transaction) -> Result<Option<Job>, Error> {
        unimplemented!("JobRepositoryAdapter: M3.16")
    }

    async fn complete(&self, _transaction: &dyn Transaction, _job: Job) -> Result<Job, Error> {
        unimplemented!("JobRepositoryAdapter: M3.16")
    }

    async fn fail(&self, _transaction: &dyn Transaction, _job: Job, _error: String) -> Result<Job, Error> {
        unimplemented!("JobRepositoryAdapter: M3.16")
    }

    async fn reset_running_to_pending(&self, _transaction: &dyn Transaction) -> Result<u64, Error> {
        unimplemented!("JobRepositoryAdapter: M3.16")
    }
}
