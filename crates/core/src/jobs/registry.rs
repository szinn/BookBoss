use std::collections::HashMap;

use crate::jobs::handler::{ErasedJobHandler, JobHandler};

pub struct JobRegistry {
    handlers: HashMap<String, Box<dyn ErasedJobHandler>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self { handlers: HashMap::new() }
    }

    pub fn register<H: JobHandler>(&mut self, handler: H) -> &mut Self {
        self.handlers.insert(H::JOB_TYPE.to_string(), Box::new(handler));
        self
    }

    pub(crate) fn get(&self, job_type: &str) -> Option<&dyn ErasedJobHandler> {
        self.handlers.get(job_type).map(|h| h.as_ref())
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}
