use serde::Serialize;

use crate::{Error, jobs::model::Job, repository::Transaction};

#[async_trait::async_trait]
pub trait JobRepository: Send + Sync {
    async fn enqueue_raw(&self, transaction: &dyn Transaction, job_type: &str, payload: serde_json::Value, priority: i16) -> Result<Job, Error>;

    /// Claim the next pending job. The version-based optimistic locking loop
    /// lives in the adapter; this returns a claimed job or `None` if the queue
    /// is empty.
    async fn claim_next(&self, transaction: &dyn Transaction) -> Result<Option<Job>, Error>;

    async fn complete(&self, transaction: &dyn Transaction, job: Job) -> Result<Job, Error>;

    /// Mark a job as failed. If `attempt < max_attempts`, reschedules with
    /// exponential backoff (`30s * 2^attempt`) and resets status to pending.
    /// Otherwise sets status to failed and preserves the error message.
    async fn fail(&self, transaction: &dyn Transaction, job: Job, error: String) -> Result<Job, Error>;

    /// Reset any jobs left in `running` state back to `pending`. Called on
    /// startup to recover from a previous crash. Returns the number of jobs
    /// reset.
    async fn reset_running_to_pending(&self, transaction: &dyn Transaction) -> Result<u64, Error>;
}

/// Marker trait for typed job payloads. Implement this on your payload struct
/// alongside `JobHandler` in your handler crate.
pub trait Enqueueable: Serialize {
    const JOB_TYPE: &'static str;
    const DEFAULT_PRIORITY: i16 = 0;
}

/// Extension methods on any `JobRepository` for typed enqueueing.
///
/// Blanket-implemented for all `JobRepository` impls — no manual work per job
/// type.
#[async_trait::async_trait]
pub trait JobRepositoryExt: JobRepository {
    async fn enqueue<P: Enqueueable + Send + Sync>(&self, transaction: &dyn Transaction, payload: &P) -> Result<Job, Error> {
        let value = serde_json::to_value(payload).map_err(|e| Error::Infrastructure(format!("failed to serialize job payload: {e}")))?;
        self.enqueue_raw(transaction, P::JOB_TYPE, value, P::DEFAULT_PRIORITY).await
    }
}

impl<R: JobRepository + ?Sized> JobRepositoryExt for R {}
