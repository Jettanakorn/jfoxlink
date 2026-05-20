use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventLevel {
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub id: Uuid,
    pub case_id: Option<Uuid>,
    pub skill_id: Option<Uuid>,
    pub stage: String,
    pub level: EventLevel,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

impl SystemEvent {
    pub fn new(
        case_id: Option<Uuid>,
        stage: impl Into<String>,
        level: EventLevel,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            case_id,
            skill_id: None,
            stage: stage.into(),
            level,
            message: message.into(),
            metadata: None,
            timestamp: Utc::now(),
        }
    }

    pub fn info(case_id: Option<Uuid>, stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(case_id, stage, EventLevel::Info, message)
    }

    pub fn warn(case_id: Option<Uuid>, stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(case_id, stage, EventLevel::Warn, message)
    }

    pub fn error(case_id: Option<Uuid>, stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(case_id, stage, EventLevel::Error, message)
    }
}

pub type EventBus = tokio::sync::broadcast::Sender<SystemEvent>;

pub fn create_event_bus(capacity: usize) -> EventBus {
    tokio::sync::broadcast::channel(capacity).0
}
