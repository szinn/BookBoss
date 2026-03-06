use std::{sync::Arc, time::Duration};

use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle};

use crate::{
    Error,
    jobs::{JobRegistry, JobRepository},
    repository::{Repository, transaction},
};

pub struct JobWorker {
    registry: JobRegistry,
    repository: Arc<dyn Repository>,
    job_repo: Arc<dyn JobRepository>,
    poll_interval: Duration,
}

impl JobWorker {
    pub fn new(registry: JobRegistry, repository: Arc<dyn Repository>, job_repo: Arc<dyn JobRepository>, poll_interval: Duration) -> Self {
        Self {
            registry,
            repository,
            job_repo,
            poll_interval,
        }
    }
}

impl IntoSubsystem<Error> for JobWorker {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        let job_repo = self.job_repo;
        let repository = self.repository;
        let registry = self.registry;
        let poll_interval = self.poll_interval;

        // Crash recovery: reset any jobs left running from a previous crash.
        let reset = transaction(&*repository, |tx| {
            let job_repo = job_repo.clone();
            Box::pin(async move { job_repo.reset_running_to_pending(tx).await })
        })
        .await?;

        if reset > 0 {
            tracing::warn!("reset {} running jobs to pending after startup", reset);
        }

        loop {
            tokio::select! {
                _ = subsys.on_shutdown_requested() => {
                    tracing::info!("JobWorker shutting down");
                    break;
                }
                _ = async {} => {
                    // Claim the next pending job.
                    let job = {
                        let job_repo = job_repo.clone();
                        transaction(&*repository, |tx| {
                            Box::pin(async move { job_repo.claim_next(tx).await })
                        })
                        .await?
                    };

                    match job {
                        None => {
                            tokio::time::sleep(poll_interval).await;
                        }
                        Some(job) => {
                            let job_type = job.job_type.clone();
                            let payload = job.payload.clone();

                            match registry.get(&job_type) {
                                None => {
                                    tracing::warn!(job_type, "no handler registered for job type");
                                    let job_repo = job_repo.clone();
                                    transaction(&*repository, |tx| {
                                        let job = job.clone();
                                        Box::pin(async move {
                                            job_repo
                                                .fail(tx, job, format!("no handler for job type '{job_type}'"))
                                                .await
                                        })
                                    })
                                    .await?;
                                }
                                Some(handler) => {
                                    match handler.handle(payload).await {
                                        Ok(()) => {
                                            let job_repo = job_repo.clone();
                                            transaction(&*repository, |tx| {
                                                let job = job.clone();
                                                Box::pin(async move { job_repo.complete(tx, job).await })
                                            })
                                            .await?;
                                        }
                                        Err(e) => {
                                            tracing::error!(job_type, error = %e, "job handler failed");
                                            let job_repo = job_repo.clone();
                                            transaction(&*repository, |tx| {
                                                let job = job.clone();
                                                Box::pin(async move {
                                                    job_repo.fail(tx, job, e.to_string()).await
                                                })
                                            })
                                            .await?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
