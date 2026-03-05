use chrono::{DateTime, Utc};

pub type JobId = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub job_type: String,
    pub payload: serde_json::Value,
    pub status: JobStatus,
    pub priority: i16,
    pub attempt: i16,
    pub max_attempts: i16,
    pub version: i32,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
