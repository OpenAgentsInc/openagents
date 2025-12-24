//! Agent Schedule Event (kind:38002)
//!
//! Defines when and how an agent should be triggered to run.
//!
//! ## Trigger Types
//!
//! - **Heartbeat** - Regular interval (e.g., every 15 minutes)
//! - **Event** - Triggered by Nostr events (mentions, DMs, zaps)
//! - **Condition** - Triggered when a condition is met (e.g., price threshold)
//!
//! ## Tags
//!
//! - `["d", "schedule"]` - Addressable event marker
//! - `["heartbeat", "900"]` - Heartbeat interval in seconds
//! - `["trigger", "mention"]` - Event trigger type
//! - `["trigger", "dm"]` - Event trigger type
//! - `["trigger", "zap"]` - Event trigger type
//!
//! ## Example
//!
//! ```json
//! {
//!   "kind": 38002,
//!   "pubkey": "<agent-pubkey>",
//!   "content": "",
//!   "tags": [
//!     ["d", "schedule"],
//!     ["heartbeat", "900"],
//!     ["trigger", "mention"],
//!     ["trigger", "dm"]
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for agent schedule event
pub const KIND_AGENT_SCHEDULE: u16 = 38002;

/// Errors that can occur during NIP-SA schedule operations
#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("invalid heartbeat: {0}")]
    InvalidHeartbeat(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Event trigger type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    /// Mentioned in a note
    Mention,
    /// Received a DM
    Dm,
    /// Received a zap
    Zap,
    /// Custom event kind
    Custom(u32),
}

impl TriggerType {
    /// Convert trigger type to tag value
    pub fn to_tag_value(&self) -> String {
        match self {
            TriggerType::Mention => "mention".to_string(),
            TriggerType::Dm => "dm".to_string(),
            TriggerType::Zap => "zap".to_string(),
            TriggerType::Custom(kind) => format!("custom:{}", kind),
        }
    }

    /// Parse trigger type from tag value
    pub fn from_tag_value(value: &str) -> Option<Self> {
        match value {
            "mention" => Some(TriggerType::Mention),
            "dm" => Some(TriggerType::Dm),
            "zap" => Some(TriggerType::Zap),
            s if s.starts_with("custom:") => {
                s.strip_prefix("custom:")
                    .and_then(|kind_str| kind_str.parse::<u32>().ok())
                    .map(TriggerType::Custom)
            }
            _ => None,
        }
    }
}

/// Agent schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSchedule {
    /// Heartbeat interval in seconds (None = no heartbeat)
    pub heartbeat_seconds: Option<u64>,
    /// Event triggers
    pub triggers: Vec<TriggerType>,
}

impl AgentSchedule {
    /// Create a new schedule with no heartbeat or triggers
    pub fn new() -> Self {
        Self {
            heartbeat_seconds: None,
            triggers: Vec::new(),
        }
    }

    /// Set heartbeat interval in seconds
    pub fn with_heartbeat(mut self, seconds: u64) -> Result<Self, ScheduleError> {
        if seconds == 0 {
            return Err(ScheduleError::InvalidHeartbeat(
                "heartbeat must be > 0".to_string(),
            ));
        }
        self.heartbeat_seconds = Some(seconds);
        Ok(self)
    }

    /// Add a trigger
    pub fn add_trigger(mut self, trigger: TriggerType) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Add multiple triggers
    pub fn with_triggers(mut self, triggers: Vec<TriggerType>) -> Self {
        self.triggers = triggers;
        self
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), "schedule".to_string()]];

        // Add heartbeat tag if present
        if let Some(seconds) = self.heartbeat_seconds {
            tags.push(vec!["heartbeat".to_string(), seconds.to_string()]);
        }

        // Add trigger tags
        for trigger in &self.triggers {
            tags.push(vec!["trigger".to_string(), trigger.to_tag_value()]);
        }

        tags
    }

    /// Validate the schedule
    pub fn validate(&self) -> Result<(), ScheduleError> {
        if let Some(seconds) = self.heartbeat_seconds {
            if seconds == 0 {
                return Err(ScheduleError::InvalidHeartbeat(
                    "heartbeat must be > 0".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl Default for AgentSchedule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_creation() {
        let schedule = AgentSchedule::new();
        assert_eq!(schedule.heartbeat_seconds, None);
        assert_eq!(schedule.triggers.len(), 0);
    }

    #[test]
    fn test_schedule_with_heartbeat() {
        let schedule = AgentSchedule::new().with_heartbeat(900).unwrap();
        assert_eq!(schedule.heartbeat_seconds, Some(900));
    }

    #[test]
    fn test_schedule_invalid_heartbeat() {
        let result = AgentSchedule::new().with_heartbeat(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_schedule_with_triggers() {
        let schedule = AgentSchedule::new()
            .add_trigger(TriggerType::Mention)
            .add_trigger(TriggerType::Dm)
            .add_trigger(TriggerType::Zap);

        assert_eq!(schedule.triggers.len(), 3);
        assert_eq!(schedule.triggers[0], TriggerType::Mention);
        assert_eq!(schedule.triggers[1], TriggerType::Dm);
        assert_eq!(schedule.triggers[2], TriggerType::Zap);
    }

    #[test]
    fn test_schedule_with_triggers_vec() {
        let triggers = vec![
            TriggerType::Mention,
            TriggerType::Dm,
            TriggerType::Custom(1234),
        ];
        let schedule = AgentSchedule::new().with_triggers(triggers);

        assert_eq!(schedule.triggers.len(), 3);
        assert_eq!(schedule.triggers[2], TriggerType::Custom(1234));
    }

    #[test]
    fn test_schedule_tags() {
        let schedule = AgentSchedule::new()
            .with_heartbeat(900)
            .unwrap()
            .add_trigger(TriggerType::Mention)
            .add_trigger(TriggerType::Dm);

        let tags = schedule.build_tags();

        assert_eq!(tags[0], vec!["d", "schedule"]);
        assert_eq!(tags[1], vec!["heartbeat", "900"]);
        assert_eq!(tags[2], vec!["trigger", "mention"]);
        assert_eq!(tags[3], vec!["trigger", "dm"]);
    }

    #[test]
    fn test_schedule_tags_no_heartbeat() {
        let schedule = AgentSchedule::new()
            .add_trigger(TriggerType::Mention)
            .add_trigger(TriggerType::Zap);

        let tags = schedule.build_tags();

        assert_eq!(tags[0], vec!["d", "schedule"]);
        assert_eq!(tags[1], vec!["trigger", "mention"]);
        assert_eq!(tags[2], vec!["trigger", "zap"]);
    }

    #[test]
    fn test_schedule_validation() {
        let valid = AgentSchedule::new().with_heartbeat(900).unwrap();
        assert!(valid.validate().is_ok());

        let no_heartbeat = AgentSchedule::new().add_trigger(TriggerType::Mention);
        assert!(no_heartbeat.validate().is_ok());
    }

    #[test]
    fn test_trigger_type_tag_conversion() {
        assert_eq!(TriggerType::Mention.to_tag_value(), "mention");
        assert_eq!(TriggerType::Dm.to_tag_value(), "dm");
        assert_eq!(TriggerType::Zap.to_tag_value(), "zap");
        assert_eq!(TriggerType::Custom(1234).to_tag_value(), "custom:1234");

        assert_eq!(
            TriggerType::from_tag_value("mention"),
            Some(TriggerType::Mention)
        );
        assert_eq!(TriggerType::from_tag_value("dm"), Some(TriggerType::Dm));
        assert_eq!(TriggerType::from_tag_value("zap"), Some(TriggerType::Zap));
        assert_eq!(
            TriggerType::from_tag_value("custom:1234"),
            Some(TriggerType::Custom(1234))
        );
        assert_eq!(TriggerType::from_tag_value("invalid"), None);
    }

    #[test]
    fn test_trigger_type_serialization() {
        let mention = TriggerType::Mention;
        let json = serde_json::to_string(&mention).unwrap();
        assert_eq!(json, "\"mention\"");

        let dm = TriggerType::Dm;
        let json = serde_json::to_string(&dm).unwrap();
        assert_eq!(json, "\"dm\"");

        let zap = TriggerType::Zap;
        let json = serde_json::to_string(&zap).unwrap();
        assert_eq!(json, "\"zap\"");
    }
}
