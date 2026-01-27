//! Tick Events (kinds:39210, 39211)
//!
//! Tick events track agent execution cycles. Each "tick" represents one run of
//! the agent - processing inputs, updating state, taking actions.
//!
//! ## Tick Request (kind:39210)
//!
//! Published by the runner at the start of a tick to signal execution.
//!
//! Tags:
//! - `["runner", "<runner-pubkey>"]` - Runner identity
//! - `["trigger", "heartbeat|mention|dm|zap"]` - What triggered this tick
//!
//! ## Tick Result (kind:39211)
//!
//! Published by the runner at the end of a tick with outcome metrics.
//!
//! Tags:
//! - `["request", "<tick-request-event-id>"]` - Links to request event
//! - `["runner", "<runner-pubkey>"]` - Runner identity
//! - `["status", "success|failure|timeout"]` - Tick outcome
//! - `["duration_ms", "1234"]` - Execution time
//! - `["actions", "3"]` - Number of actions taken
//!
//! Content: JSON metrics
//!
//! ```json
//! {
//!   "tokens_in": 1000,
//!   "tokens_out": 500,
//!   "cost_usd": 0.05,
//!   "goals_updated": 2,
//!   "actions": [
//!     {"type": "post", "id": "event-id-1"},
//!     {"type": "dm", "recipient": "npub..."}
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Kind for tick request event
pub const KIND_TICK_REQUEST: u16 = 39210;

/// Kind for tick result event
pub const KIND_TICK_RESULT: u16 = 39211;

/// Errors that can occur during NIP-SA tick operations
#[derive(Debug, Error)]
pub enum TickError {
    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("invalid duration: {0}")]
    InvalidDuration(String),
}

/// What triggered a tick
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickTrigger {
    /// Regular heartbeat
    Heartbeat,
    /// Mentioned in a note
    Mention,
    /// Received a DM
    Dm,
    /// Received a zap
    Zap,
    /// Manual trigger
    Manual,
}

impl TickTrigger {
    pub fn from_tag_value(value: &str) -> Option<Self> {
        match value {
            "heartbeat" => Some(TickTrigger::Heartbeat),
            "mention" => Some(TickTrigger::Mention),
            "dm" => Some(TickTrigger::Dm),
            "zap" => Some(TickTrigger::Zap),
            "manual" => Some(TickTrigger::Manual),
            _ => None,
        }
    }
}

/// Tick outcome status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickStatus {
    /// Tick completed successfully
    Success,
    /// Tick failed with error
    Failure,
    /// Tick exceeded time limit
    Timeout,
}

impl TickStatus {
    pub fn from_tag_value(value: &str) -> Option<Self> {
        match value {
            "success" => Some(TickStatus::Success),
            "failure" => Some(TickStatus::Failure),
            "timeout" => Some(TickStatus::Timeout),
            _ => None,
        }
    }
}

/// Action taken during a tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickAction {
    /// Action type (post, dm, zap, etc.)
    #[serde(rename = "type")]
    pub action_type: String,
    /// Event ID if action resulted in an event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Additional action metadata
    #[serde(flatten)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

/// Tick request
#[derive(Debug, Clone)]
pub struct TickRequest {
    /// Runner pubkey (hex)
    pub runner: String,
    /// What triggered this tick
    pub trigger: TickTrigger,
}

impl TickRequest {
    /// Create a new tick request
    pub fn new(runner: impl Into<String>, trigger: TickTrigger) -> Self {
        Self {
            runner: runner.into(),
            trigger,
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        vec![
            vec!["runner".to_string(), self.runner.clone()],
            vec!["trigger".to_string(), self.trigger_to_string()],
        ]
    }

    fn trigger_to_string(&self) -> String {
        match self.trigger {
            TickTrigger::Heartbeat => "heartbeat".to_string(),
            TickTrigger::Mention => "mention".to_string(),
            TickTrigger::Dm => "dm".to_string(),
            TickTrigger::Zap => "zap".to_string(),
            TickTrigger::Manual => "manual".to_string(),
        }
    }
}

/// Tick result metrics (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickResultContent {
    /// Tokens consumed (input)
    pub tokens_in: u64,
    /// Tokens generated (output)
    pub tokens_out: u64,
    /// Cost in USD
    pub cost_usd: f64,
    /// Number of goals updated
    pub goals_updated: u32,
    /// Actions taken during this tick
    pub actions: Vec<TickAction>,
}

impl TickResultContent {
    /// Create new tick result content
    pub fn new(tokens_in: u64, tokens_out: u64, cost_usd: f64, goals_updated: u32) -> Self {
        Self {
            tokens_in,
            tokens_out,
            cost_usd,
            goals_updated,
            actions: Vec::new(),
        }
    }

    /// Add an action
    pub fn add_action(mut self, action: TickAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Add multiple actions
    pub fn with_actions(mut self, actions: Vec<TickAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, TickError> {
        serde_json::to_string(self).map_err(|e| TickError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, TickError> {
        serde_json::from_str(json).map_err(|e| TickError::Deserialization(e.to_string()))
    }
}

impl TickAction {
    /// Create a new tick action
    pub fn new(action_type: impl Into<String>) -> Self {
        Self {
            action_type: action_type.into(),
            id: None,
            metadata: serde_json::Map::new(),
        }
    }

    /// Set event ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Add metadata field
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Tick result
#[derive(Debug, Clone)]
pub struct TickResult {
    /// Request event ID
    pub request_id: String,
    /// Runner pubkey (hex)
    pub runner: String,
    /// Tick status
    pub status: TickStatus,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of actions taken
    pub action_count: u32,
    /// Optional trajectory hash for verification
    pub trajectory_hash: Option<String>,
    /// Result content (metrics)
    pub content: TickResultContent,
}

impl TickResult {
    /// Create a new tick result
    pub fn new(
        request_id: impl Into<String>,
        runner: impl Into<String>,
        status: TickStatus,
        duration_ms: u64,
        content: TickResultContent,
    ) -> Self {
        let action_count = content.actions.len() as u32;
        Self {
            request_id: request_id.into(),
            runner: runner.into(),
            status,
            duration_ms,
            action_count,
            trajectory_hash: None,
            content,
        }
    }

    /// Attach trajectory hash for verification
    pub fn with_trajectory_hash(mut self, hash: impl Into<String>) -> Self {
        self.trajectory_hash = Some(hash.into());
        self
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["request".to_string(), self.request_id.clone()],
            vec!["runner".to_string(), self.runner.clone()],
            vec!["status".to_string(), self.status_to_string()],
            vec!["duration_ms".to_string(), self.duration_ms.to_string()],
            vec!["actions".to_string(), self.action_count.to_string()],
        ];

        if let Some(hash) = &self.trajectory_hash {
            tags.push(vec!["trajectory_hash".to_string(), hash.clone()]);
        }

        tags
    }

    fn status_to_string(&self) -> String {
        match self.status {
            TickStatus::Success => "success".to_string(),
            TickStatus::Failure => "failure".to_string(),
            TickStatus::Timeout => "timeout".to_string(),
        }
    }

    /// Validate the tick result
    pub fn validate(&self) -> Result<(), TickError> {
        if self.duration_ms == 0 {
            return Err(TickError::InvalidDuration(
                "duration must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Combined view of tick request and result for history inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickHistoryEntry {
    pub request_id: String,
    pub runner: String,
    pub trigger: Option<TickTrigger>,
    pub request_created_at: Option<u64>,
    pub result_id: Option<String>,
    pub result_created_at: Option<u64>,
    pub status: Option<TickStatus>,
    pub duration_ms: Option<u64>,
    pub action_count: Option<u32>,
    pub trajectory_hash: Option<String>,
}

impl TickHistoryEntry {
    fn new(request_id: impl Into<String>, runner: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            runner: runner.into(),
            trigger: None,
            request_created_at: None,
            result_id: None,
            result_created_at: None,
            status: None,
            duration_ms: None,
            action_count: None,
            trajectory_hash: None,
        }
    }

    fn sort_timestamp(&self) -> u64 {
        self.request_created_at
            .or(self.result_created_at)
            .unwrap_or(0)
    }
}

/// Build tick history entries from a mixed list of tick request/result events.
pub fn build_tick_history(events: &[crate::Event]) -> Vec<TickHistoryEntry> {
    let mut entries: HashMap<String, TickHistoryEntry> = HashMap::new();

    for event in events {
        match event.kind {
            KIND_TICK_REQUEST => {
                let runner = get_tag_value(&event.tags, "runner")
                    .unwrap_or(&event.pubkey)
                    .to_string();
                let trigger =
                    get_tag_value(&event.tags, "trigger").and_then(TickTrigger::from_tag_value);
                let entry = entries
                    .entry(event.id.clone())
                    .or_insert_with(|| TickHistoryEntry::new(&event.id, &runner));

                entry.runner = runner;
                entry.trigger = trigger;
                entry.request_created_at = Some(event.created_at);
            }
            KIND_TICK_RESULT => {
                let request_id = match get_tag_value(&event.tags, "request") {
                    Some(value) => value.to_string(),
                    None => continue,
                };
                let runner = get_tag_value(&event.tags, "runner")
                    .unwrap_or(&event.pubkey)
                    .to_string();
                let status =
                    get_tag_value(&event.tags, "status").and_then(TickStatus::from_tag_value);
                let duration_ms = get_tag_value(&event.tags, "duration_ms")
                    .and_then(|value| value.parse::<u64>().ok());
                let action_count = get_tag_value(&event.tags, "actions")
                    .and_then(|value| value.parse::<u32>().ok());
                let trajectory_hash =
                    get_tag_value(&event.tags, "trajectory_hash").map(|value| value.to_string());

                let entry = entries
                    .entry(request_id.clone())
                    .or_insert_with(|| TickHistoryEntry::new(&request_id, &runner));

                entry.runner = runner;
                entry.result_id = Some(event.id.clone());
                entry.result_created_at = Some(event.created_at);
                entry.status = status;
                entry.duration_ms = duration_ms;
                entry.action_count = action_count;
                entry.trajectory_hash = trajectory_hash;
            }
            _ => {}
        }
    }

    let mut history: Vec<TickHistoryEntry> = entries.into_values().collect();
    history.sort_by_key(|entry| std::cmp::Reverse(entry.sort_timestamp()));
    history
}

fn get_tag_value<'a>(tags: &'a [Vec<String>], key: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| {
        if tag.first().map(|value| value.as_str()) == Some(key) {
            tag.get(1).map(|value| value.as_str())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_event(
        id: &str,
        pubkey: &str,
        created_at: u64,
        kind: u16,
        tags: Vec<Vec<String>>,
    ) -> crate::Event {
        crate::Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at,
            kind,
            tags,
            content: String::new(),
            sig: String::new(),
        }
    }

    #[test]
    fn test_tick_request_creation() {
        let request = TickRequest::new("runner_pubkey", TickTrigger::Heartbeat);
        assert_eq!(request.runner, "runner_pubkey");
        assert_eq!(request.trigger, TickTrigger::Heartbeat);
    }

    #[test]
    fn test_tick_request_tags() {
        let request = TickRequest::new("runner_pubkey", TickTrigger::Mention);
        let tags = request.build_tags();

        assert_eq!(tags[0], vec!["runner", "runner_pubkey"]);
        assert_eq!(tags[1], vec!["trigger", "mention"]);
    }

    #[test]
    fn test_tick_result_content_creation() {
        let content = TickResultContent::new(1000, 500, 0.05, 2);
        assert_eq!(content.tokens_in, 1000);
        assert_eq!(content.tokens_out, 500);
        assert_eq!(content.cost_usd, 0.05);
        assert_eq!(content.goals_updated, 2);
        assert_eq!(content.actions.len(), 0);
    }

    #[test]
    fn test_tick_result_content_with_actions() {
        let action1 = TickAction::new("post").with_id("event-1");
        let action2 = TickAction::new("dm").with_metadata(
            "recipient",
            serde_json::Value::String("npub...".to_string()),
        );

        let content = TickResultContent::new(1000, 500, 0.05, 2)
            .add_action(action1)
            .add_action(action2);

        assert_eq!(content.actions.len(), 2);
        assert_eq!(content.actions[0].action_type, "post");
        assert_eq!(content.actions[0].id, Some("event-1".to_string()));
        assert_eq!(content.actions[1].action_type, "dm");
    }

    #[test]
    fn test_tick_result_content_serialization() {
        let content = TickResultContent::new(1000, 500, 0.05, 2)
            .add_action(TickAction::new("post").with_id("event-1"));

        let json = content.to_json().unwrap();
        let parsed = TickResultContent::from_json(&json).unwrap();

        assert_eq!(parsed.tokens_in, 1000);
        assert_eq!(parsed.tokens_out, 500);
        assert_eq!(parsed.goals_updated, 2);
        assert_eq!(parsed.actions.len(), 1);
    }

    #[test]
    fn test_tick_result_creation() {
        let content = TickResultContent::new(1000, 500, 0.05, 2);
        let result = TickResult::new(
            "request-id",
            "runner_pubkey",
            TickStatus::Success,
            1234,
            content,
        );

        assert_eq!(result.request_id, "request-id");
        assert_eq!(result.runner, "runner_pubkey");
        assert_eq!(result.status, TickStatus::Success);
        assert_eq!(result.duration_ms, 1234);
        assert_eq!(result.action_count, 0);
        assert!(result.trajectory_hash.is_none());
    }

    #[test]
    fn test_tick_result_tags() {
        let content = TickResultContent::new(1000, 500, 0.05, 2)
            .add_action(TickAction::new("post"))
            .add_action(TickAction::new("dm"))
            .add_action(TickAction::new("zap"));

        let result = TickResult::new(
            "request-id",
            "runner_pubkey",
            TickStatus::Success,
            1234,
            content,
        )
        .with_trajectory_hash("hash123");

        let tags = result.build_tags();

        assert_eq!(tags[0], vec!["request", "request-id"]);
        assert_eq!(tags[1], vec!["runner", "runner_pubkey"]);
        assert_eq!(tags[2], vec!["status", "success"]);
        assert_eq!(tags[3], vec!["duration_ms", "1234"]);
        assert_eq!(tags[4], vec!["actions", "3"]);
        assert_eq!(tags[5], vec!["trajectory_hash", "hash123"]);
    }

    #[test]
    fn test_tick_result_validation() {
        let content = TickResultContent::new(1000, 500, 0.05, 2);
        let valid = TickResult::new(
            "request-id",
            "runner_pubkey",
            TickStatus::Success,
            1234,
            content,
        );
        assert!(valid.validate().is_ok());

        let content = TickResultContent::new(1000, 500, 0.05, 2);
        let invalid = TickResult::new(
            "request-id",
            "runner_pubkey",
            TickStatus::Success,
            0,
            content,
        );
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_tick_trigger_serialization() {
        let heartbeat = TickTrigger::Heartbeat;
        let json = serde_json::to_string(&heartbeat).unwrap();
        assert_eq!(json, "\"heartbeat\"");

        let mention = TickTrigger::Mention;
        let json = serde_json::to_string(&mention).unwrap();
        assert_eq!(json, "\"mention\"");

        let dm = TickTrigger::Dm;
        let json = serde_json::to_string(&dm).unwrap();
        assert_eq!(json, "\"dm\"");

        let zap = TickTrigger::Zap;
        let json = serde_json::to_string(&zap).unwrap();
        assert_eq!(json, "\"zap\"");

        let manual = TickTrigger::Manual;
        let json = serde_json::to_string(&manual).unwrap();
        assert_eq!(json, "\"manual\"");
    }

    #[test]
    fn test_tick_status_serialization() {
        let success = TickStatus::Success;
        let json = serde_json::to_string(&success).unwrap();
        assert_eq!(json, "\"success\"");

        let failure = TickStatus::Failure;
        let json = serde_json::to_string(&failure).unwrap();
        assert_eq!(json, "\"failure\"");

        let timeout = TickStatus::Timeout;
        let json = serde_json::to_string(&timeout).unwrap();
        assert_eq!(json, "\"timeout\"");
    }

    #[test]
    fn test_build_tick_history() {
        let request_event = mock_event(
            "req-1",
            "runner_pubkey",
            100,
            KIND_TICK_REQUEST,
            vec![
                vec!["runner".to_string(), "runner_pubkey".to_string()],
                vec!["trigger".to_string(), "heartbeat".to_string()],
            ],
        );
        let result_event = mock_event(
            "res-1",
            "runner_pubkey",
            120,
            KIND_TICK_RESULT,
            vec![
                vec!["request".to_string(), "req-1".to_string()],
                vec!["runner".to_string(), "runner_pubkey".to_string()],
                vec!["status".to_string(), "success".to_string()],
                vec!["duration_ms".to_string(), "500".to_string()],
                vec!["actions".to_string(), "2".to_string()],
                vec!["trajectory_hash".to_string(), "hash123".to_string()],
            ],
        );

        let history = build_tick_history(&[result_event, request_event]);
        assert_eq!(history.len(), 1);

        let entry = &history[0];
        assert_eq!(entry.request_id, "req-1");
        assert_eq!(entry.runner, "runner_pubkey");
        assert_eq!(entry.trigger, Some(TickTrigger::Heartbeat));
        assert_eq!(entry.request_created_at, Some(100));
        assert_eq!(entry.result_id.as_deref(), Some("res-1"));
        assert_eq!(entry.result_created_at, Some(120));
        assert_eq!(entry.status, Some(TickStatus::Success));
        assert_eq!(entry.duration_ms, Some(500));
        assert_eq!(entry.action_count, Some(2));
        assert_eq!(entry.trajectory_hash.as_deref(), Some("hash123"));
    }

    #[test]
    fn test_build_tick_history_orders_by_latest() {
        let first = mock_event(
            "req-1",
            "runner_pubkey",
            100,
            KIND_TICK_REQUEST,
            vec![vec!["trigger".to_string(), "heartbeat".to_string()]],
        );
        let second = mock_event(
            "req-2",
            "runner_pubkey",
            200,
            KIND_TICK_REQUEST,
            vec![vec!["trigger".to_string(), "mention".to_string()]],
        );

        let history = build_tick_history(&[first, second]);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].request_id, "req-2");
        assert_eq!(history[1].request_id, "req-1");
    }
}
