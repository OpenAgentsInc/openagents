//! Domain events for event-sourced architecture.
//!
//! All state changes in the system are captured as append-only domain events.
//! This enables full audit trails, time-travel debugging, and event replay.

use crate::ids::*;
use crate::message::Message;
use crate::run::{CostSummary, RunStatus, StepStatus};
use crate::tool::{ToolResult, ToolUse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Domain events capturing all state changes.
///
/// Events are append-only and immutable. They represent facts
/// that have occurred in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    // ==================
    // Thread/Chat Events
    // ==================
    /// A new thread was created.
    ThreadCreated {
        thread_id: ThreadId,
        project_id: Option<ProjectId>,
        timestamp: DateTime<Utc>,
    },

    /// A message was added to a thread.
    MessageAdded {
        thread_id: ThreadId,
        message: Message,
    },

    /// Streaming content was appended to a message.
    MessageStreaming {
        thread_id: ThreadId,
        message_id: MessageId,
        delta: String,
        timestamp: DateTime<Utc>,
    },

    /// A streaming message was completed.
    MessageComplete {
        thread_id: ThreadId,
        message_id: MessageId,
        timestamp: DateTime<Utc>,
    },

    // ==================
    // Tool Use Events
    // ==================
    /// A tool use was started.
    ToolUseStarted {
        thread_id: ThreadId,
        message_id: MessageId,
        tool_use: ToolUse,
    },

    /// A tool use completed.
    ToolUseComplete {
        thread_id: ThreadId,
        message_id: MessageId,
        tool_use_id: ToolUseId,
        result: ToolResult,
    },

    // ==================
    // Run Events
    // ==================
    /// A new run was started.
    RunStarted {
        run_id: RunId,
        workflow_id: WorkflowId,
        timestamp: DateTime<Utc>,
    },

    /// A step within a run was updated.
    RunStepUpdated {
        run_id: RunId,
        step_id: StepId,
        status: StepStatus,
        log_delta: String,
    },

    /// An artifact was added to a run.
    RunArtifactAdded {
        run_id: RunId,
        step_id: StepId,
        artifact_id: ArtifactId,
        artifact_type: String,
        path: String,
    },

    /// A run finished.
    RunFinished {
        run_id: RunId,
        status: RunStatus,
        cost: CostSummary,
        timestamp: DateTime<Utc>,
    },

    // ==================
    // Project Events
    // ==================
    /// A project was created.
    ProjectCreated {
        project_id: ProjectId,
        name: String,
        timestamp: DateTime<Utc>,
    },

    /// A project was updated.
    ProjectUpdated {
        project_id: ProjectId,
        changes: ProjectChanges,
        timestamp: DateTime<Utc>,
    },
}

/// Changes to a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectChanges {
    /// New name (if changed).
    pub name: Option<String>,
    /// New description (if changed).
    pub description: Option<String>,
}

impl DomainEvent {
    /// Get the timestamp of the event.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            DomainEvent::ThreadCreated { timestamp, .. } => *timestamp,
            DomainEvent::MessageAdded { message, .. } => message.created_at,
            DomainEvent::MessageStreaming { timestamp, .. } => *timestamp,
            DomainEvent::MessageComplete { timestamp, .. } => *timestamp,
            DomainEvent::ToolUseStarted { tool_use, .. } => tool_use.started_at,
            DomainEvent::ToolUseComplete { .. } => Utc::now(), // Result has its own timestamp
            DomainEvent::RunStarted { timestamp, .. } => *timestamp,
            DomainEvent::RunStepUpdated { .. } => Utc::now(),
            DomainEvent::RunArtifactAdded { .. } => Utc::now(),
            DomainEvent::RunFinished { timestamp, .. } => *timestamp,
            DomainEvent::ProjectCreated { timestamp, .. } => *timestamp,
            DomainEvent::ProjectUpdated { timestamp, .. } => *timestamp,
        }
    }

    /// Get the thread ID if this event is thread-related.
    pub fn thread_id(&self) -> Option<ThreadId> {
        match self {
            DomainEvent::ThreadCreated { thread_id, .. } => Some(*thread_id),
            DomainEvent::MessageAdded { thread_id, .. } => Some(*thread_id),
            DomainEvent::MessageStreaming { thread_id, .. } => Some(*thread_id),
            DomainEvent::MessageComplete { thread_id, .. } => Some(*thread_id),
            DomainEvent::ToolUseStarted { thread_id, .. } => Some(*thread_id),
            DomainEvent::ToolUseComplete { thread_id, .. } => Some(*thread_id),
            _ => None,
        }
    }

    /// Get the run ID if this event is run-related.
    pub fn run_id(&self) -> Option<RunId> {
        match self {
            DomainEvent::RunStarted { run_id, .. } => Some(*run_id),
            DomainEvent::RunStepUpdated { run_id, .. } => Some(*run_id),
            DomainEvent::RunArtifactAdded { run_id, .. } => Some(*run_id),
            DomainEvent::RunFinished { run_id, .. } => Some(*run_id),
            _ => None,
        }
    }

    /// Get the project ID if this event is project-related.
    pub fn project_id(&self) -> Option<ProjectId> {
        match self {
            DomainEvent::ThreadCreated { project_id, .. } => *project_id,
            DomainEvent::ProjectCreated { project_id, .. } => Some(*project_id),
            DomainEvent::ProjectUpdated { project_id, .. } => Some(*project_id),
            _ => None,
        }
    }
}

/// Event envelope with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique sequence number for this event.
    pub sequence: u64,
    /// The domain event.
    pub event: DomainEvent,
    /// When this event was persisted.
    pub persisted_at: DateTime<Utc>,
}

impl EventEnvelope {
    /// Create a new event envelope.
    pub fn new(sequence: u64, event: DomainEvent) -> Self {
        Self {
            sequence,
            event,
            persisted_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Role;

    #[test]
    fn test_event_serialization() {
        let thread_id = ThreadId::new();
        let message = Message::new(Role::User, "Hello!");

        let event = DomainEvent::MessageAdded {
            thread_id,
            message: message.clone(),
        };

        // Should serialize to JSON
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("message_added"));

        // Should deserialize back
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        if let DomainEvent::MessageAdded { message: m, .. } = deserialized {
            assert_eq!(m.content, "Hello!");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_event_envelope() {
        let event = DomainEvent::ThreadCreated {
            thread_id: ThreadId::new(),
            project_id: None,
            timestamp: Utc::now(),
        };

        let envelope = EventEnvelope::new(1, event);
        assert_eq!(envelope.sequence, 1);
    }
}
