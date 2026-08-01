use serde::Serialize;

pub const EVENT_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug)]
pub enum EventName {
    RuntimeProgress,
    RuntimeActivity,
    RuntimeCompleted,
    BatchProgress,
    BatchItemCompleted,
    BatchCompleted,
    TaskFailed,
    TaskCancelled,
}

impl EventName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeProgress => "runtime://progress",
            Self::RuntimeActivity => "runtime://activity",
            Self::RuntimeCompleted => "runtime://completed",
            Self::BatchProgress => "batch://progress",
            Self::BatchItemCompleted => "batch://item-completed",
            Self::BatchCompleted => "batch://completed",
            Self::TaskFailed => "task://failed",
            Self::TaskCancelled => "task://cancelled",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendEvent<T> {
    pub schema_version: u8,
    pub task_id: String,
    pub sequence: u64,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: T,
}
