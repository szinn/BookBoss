//! Resilience infrastructure for subsystems.
//!
//! [`ResilienceWrapper`] wraps any [`CheckedSubsystem`]: it runs `check()` with
//! exponential backoff until the subsystem is ready to start, then starts it.
//! If the subsystem exits with a transient error, the wrapper loops back to
//! the check phase and restarts. Permanent errors emit a system message and
//! exit cleanly — the tokio-graceful-shutdown `Toplevel` never sees an `Err`.

use std::{sync::Arc, time::Duration};

use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle};

use crate::{
    Error,
    message::{MessageSeverity, NewSystemMessage, SystemMessageId, SystemMessageService},
};

const BACKOFF_INITIAL: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_mins(5);

/// Result of a subsystem precondition check.
pub enum CheckResult {
    /// Precondition met — proceed to start the subsystem.
    Ok,
    /// Precondition not yet met due to a transient condition (DB unreachable,
    /// NFS mount gone). The wrapper will retry with exponential backoff.
    Transient(String),
    /// Precondition permanently unmet (misconfiguration, wrong credentials).
    /// The wrapper emits a system message and stops — no further retries.
    Permanent(String),
}

/// A subsystem that declares a startup precondition check.
///
/// `Clone` is required because [`ResilienceWrapper`] re-clones the inner
/// subsystem before each `run()` invocation to support restart. All current
/// subsystems hold only `Arc<>` fields and `Duration`, so `#[derive(Clone)]`
/// is sufficient.
#[async_trait::async_trait]
pub trait CheckedSubsystem: Clone + Send + Sync + 'static {
    /// Check whether this subsystem's preconditions are satisfied.
    ///
    /// Called before each `run()` invocation (including after a transient
    /// restart). Should be fast and non-destructive.
    async fn check(&self) -> CheckResult;

    /// Run the subsystem. Only called after `check()` returns `Ok`.
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error>;
}

/// Wraps a [`CheckedSubsystem`] with precondition checking and automatic
/// restart on transient failures.
///
/// Never returns `Err` to tokio-graceful-shutdown — all failures are handled
/// internally via system messages and clean exits.
pub struct ResilienceWrapper<S: CheckedSubsystem> {
    name: &'static str,
    inner: S,
    system_message_service: Arc<dyn SystemMessageService>,
}

impl<S: CheckedSubsystem> ResilienceWrapper<S> {
    #[must_use]
    pub fn new(name: &'static str, inner: S, system_message_service: Arc<dyn SystemMessageService>) -> Self {
        Self {
            name,
            inner,
            system_message_service,
        }
    }

    async fn emit_message(&self, id_slot: &mut Option<SystemMessageId>, severity: MessageSeverity, message: String) {
        if id_slot.is_some() {
            return; // Already have an active message — don't duplicate.
        }
        let msg = NewSystemMessage {
            source_task: format!("resilience.{}", self.name),
            severity,
            message,
        };
        match self.system_message_service.add_message(msg).await {
            Ok(m) => *id_slot = Some(m.id),
            Err(e) => tracing::warn!(subsystem = self.name, "failed to emit resilience message: {e}"),
        }
    }

    async fn clear_message(&self, id_slot: &mut Option<SystemMessageId>) {
        if let Some(id) = id_slot.take()
            && let Err(e) = self.system_message_service.delete_message(id).await
        {
            tracing::warn!(subsystem = self.name, "failed to clear resilience message: {e}");
        }
    }
}

impl<S: CheckedSubsystem> IntoSubsystem<Error> for ResilienceWrapper<S> {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        let mut message_id: Option<SystemMessageId> = None;

        loop {
            // ── Phase 1: precondition check with exponential backoff ────────
            let mut delay = BACKOFF_INITIAL;
            loop {
                tokio::select! {
                    () = subsys.on_shutdown_requested() => {
                        self.clear_message(&mut message_id).await;
                        return Ok(());
                    }
                    result = self.inner.check() => match result {
                        CheckResult::Ok => break,
                        CheckResult::Transient(msg) => {
                            tracing::warn!(subsystem = self.name, "check failed (transient): {msg}, retrying in {delay:?}");
                            self.emit_message(
                                &mut message_id,
                                MessageSeverity::Warning,
                                format!("{} temporarily unavailable: {msg}", self.name),
                            ).await;
                            tokio::time::sleep(delay).await;
                            delay = (delay * 2).min(BACKOFF_MAX);
                        }
                        CheckResult::Permanent(msg) => {
                            tracing::error!(subsystem = self.name, "check failed (permanent): {msg}");
                            self.emit_message(
                                &mut message_id,
                                MessageSeverity::Error,
                                format!("{} failed to start (permanent): {msg}", self.name),
                            ).await;
                            return Ok(()); // Clean exit — no server shutdown.
                        }
                    }
                }
            }

            // ── Phase 2: clear degraded message, run the inner subsystem ────
            self.clear_message(&mut message_id).await;
            tracing::info!(subsystem = self.name, "check passed, starting subsystem");

            match self.inner.clone().run(subsys).await {
                Ok(()) => {
                    tracing::info!(subsystem = self.name, "subsystem exited cleanly");
                    return Ok(());
                }
                Err(e) if e.is_transient() => {
                    tracing::warn!(subsystem = self.name, "exited with transient error: {e}, restarting");
                    // Loop back to check phase.
                }
                Err(e) => {
                    tracing::error!(subsystem = self.name, "exited with permanent error: {e}");
                    self.emit_message(&mut message_id, MessageSeverity::Error, format!("{} failed permanently: {e}", self.name))
                        .await;
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::message::{SystemMessage, service::MockSystemMessageService};

    fn make_mock_message(id: u64) -> SystemMessage {
        SystemMessage {
            id,
            source_task: "test".into(),
            severity: MessageSeverity::Warning,
            message: "test".into(),
            created_at: chrono::Utc::now(),
        }
    }

    fn message_service_that_records(
        added: &Arc<Mutex<Vec<NewSystemMessage>>>,
        deleted: &Arc<Mutex<Vec<SystemMessageId>>>,
    ) -> Arc<dyn crate::message::SystemMessageService> {
        let mut mock = MockSystemMessageService::new();
        let added_clone = added.clone();
        mock.expect_add_message().returning(move |msg| {
            let id = 1u64;
            added_clone.lock().unwrap().push(msg);
            Box::pin(async move { Ok(make_mock_message(id)) })
        });
        let deleted_clone = deleted.clone();
        mock.expect_delete_message().returning(move |id| {
            deleted_clone.lock().unwrap().push(id);
            Box::pin(async { Ok(()) })
        });
        Arc::new(mock)
    }

    // A CheckedSubsystem that fails `fail_checks` times, then succeeds.
    // run() returns Ok immediately (simulates clean shutdown).
    #[derive(Clone)]
    struct SucceedAfterN {
        checks_remaining: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl CheckedSubsystem for SucceedAfterN {
        async fn check(&self) -> CheckResult {
            let mut n = self.checks_remaining.lock().unwrap();
            if *n > 0 {
                *n -= 1;
                CheckResult::Transient("not ready".into())
            } else {
                CheckResult::Ok
            }
        }

        async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
            subsys.on_shutdown_requested().await;
            Ok(())
        }
    }

    #[tokio::test]
    async fn check_loop_retries_transient_then_succeeds() {
        let inner = SucceedAfterN {
            checks_remaining: Arc::new(Mutex::new(2)),
        };
        assert!(matches!(inner.check().await, CheckResult::Transient(_)));
        assert!(matches!(inner.check().await, CheckResult::Transient(_)));
        assert!(matches!(inner.check().await, CheckResult::Ok));
    }

    #[tokio::test]
    async fn emit_message_on_first_failure_then_clear_on_recovery() {
        let added = Arc::new(Mutex::new(vec![]));
        let deleted = Arc::new(Mutex::new(vec![]));
        let svc = message_service_that_records(&added, &deleted);

        let inner = SucceedAfterN {
            checks_remaining: Arc::new(Mutex::new(0)),
        };
        let wrapper = ResilienceWrapper::new("Test", inner, svc);

        let mut id_slot: Option<SystemMessageId> = None;

        // Emit a warning message.
        wrapper.emit_message(&mut id_slot, MessageSeverity::Warning, "unavailable".into()).await;
        assert!(id_slot.is_some());
        assert_eq!(added.lock().unwrap().len(), 1);

        // Second emit call is a no-op (id_slot already set).
        wrapper.emit_message(&mut id_slot, MessageSeverity::Warning, "still unavailable".into()).await;
        assert_eq!(added.lock().unwrap().len(), 1);

        // Clear.
        wrapper.clear_message(&mut id_slot).await;
        assert!(id_slot.is_none());
        assert_eq!(deleted.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn is_transient_used_for_restart_decision() {
        use crate::{Error, RepositoryError};
        assert!(Error::RepositoryError(RepositoryError::Connection("x".into())).is_transient());
        assert!(!Error::Infrastructure("x".into()).is_transient());
    }
}
