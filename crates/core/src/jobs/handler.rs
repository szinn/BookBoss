use std::{future::Future, pin::Pin};

use serde::de::DeserializeOwned;

use crate::Error;

/// Implement this trait on your handler struct. `JOB_TYPE` must match the
/// corresponding `Enqueueable::JOB_TYPE` on the payload.
pub trait JobHandler: Send + Sync + 'static {
    const JOB_TYPE: &'static str;
    type Payload: DeserializeOwned + Send;

    fn handle(&self, payload: Self::Payload) -> impl Future<Output = Result<(), Error>> + Send;
}

/// Object-safe erased version of `JobHandler` used for HashMap storage.
/// Not part of the public API — handler authors implement `JobHandler`.
pub(crate) trait ErasedJobHandler: Send + Sync {
    fn job_type(&self) -> &str;
    fn handle<'a>(&'a self, payload: serde_json::Value) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
}

impl<H: JobHandler> ErasedJobHandler for H {
    fn job_type(&self) -> &str {
        H::JOB_TYPE
    }

    fn handle<'a>(&'a self, payload: serde_json::Value) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            let typed: H::Payload = serde_json::from_value(payload).map_err(|e| Error::Infrastructure(format!("job payload deserialize failed: {e}")))?;
            JobHandler::handle(self, typed).await
        })
    }
}
