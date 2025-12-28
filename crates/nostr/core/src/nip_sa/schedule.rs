//! Agent Schedule Event (kind:39202)
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
//!   "kind": 39202,
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
pub const KIND_AGENT_SCHEDULE: u16 = 39202;

/// Errors that can occur during NIP-SA schedule operations
#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("invalid heartbeat: {0}")]
    InvalidHeartbeat(String),

    #[error("invalid business time: {0}")]
    InvalidBusinessTime(String),

    #[error("invalid business hours: {0}")]
    InvalidBusinessHours(String),

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

/// Day of week for business hours
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub fn to_tag_value(&self) -> &'static str {
        match self {
            Weekday::Mon => "mon",
            Weekday::Tue => "tue",
            Weekday::Wed => "wed",
            Weekday::Thu => "thu",
            Weekday::Fri => "fri",
            Weekday::Sat => "sat",
            Weekday::Sun => "sun",
        }
    }
}

/// Time-of-day for business hours (24h clock)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessTime {
    pub hour: u8,
    pub minute: u8,
}

impl BusinessTime {
    pub fn new(hour: u8, minute: u8) -> Result<Self, ScheduleError> {
        if hour > 23 || minute > 59 {
            return Err(ScheduleError::InvalidBusinessTime(format!(
                "invalid time {:02}:{:02}",
                hour, minute
            )));
        }
        Ok(Self { hour, minute })
    }

    pub fn total_minutes(&self) -> u16 {
        self.hour as u16 * 60 + self.minute as u16
    }

    pub fn validate(&self) -> Result<(), ScheduleError> {
        if self.hour > 23 || self.minute > 59 {
            return Err(ScheduleError::InvalidBusinessTime(format!(
                "invalid time {:02}:{:02}",
                self.hour, self.minute
            )));
        }
        Ok(())
    }
}

impl std::fmt::Display for BusinessTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

/// Business hours configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessHours {
    pub days: Vec<Weekday>,
    pub start: BusinessTime,
    pub end: BusinessTime,
}

impl BusinessHours {
    pub fn new(
        days: Vec<Weekday>,
        start: BusinessTime,
        end: BusinessTime,
    ) -> Result<Self, ScheduleError> {
        let hours = Self { days, start, end };
        hours.validate()?;
        Ok(hours)
    }

    pub fn validate(&self) -> Result<(), ScheduleError> {
        if self.days.is_empty() {
            return Err(ScheduleError::InvalidBusinessHours(
                "business hours require at least one day".to_string(),
            ));
        }
        self.start.validate()?;
        self.end.validate()?;
        if self.start.total_minutes() >= self.end.total_minutes() {
            return Err(ScheduleError::InvalidBusinessHours(
                "start time must be before end time".to_string(),
            ));
        }
        Ok(())
    }

    pub fn allows(&self, day: Weekday, time: BusinessTime) -> bool {
        if !self.days.contains(&day) {
            return false;
        }
        let minutes = time.total_minutes();
        minutes >= self.start.total_minutes() && minutes < self.end.total_minutes()
    }

    pub fn to_tag_value(&self) -> String {
        let days = self
            .days
            .iter()
            .map(Weekday::to_tag_value)
            .collect::<Vec<_>>()
            .join(",");
        format!("{} {}-{}", days, self.start, self.end)
    }
}

/// Agent schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSchedule {
    /// Heartbeat interval in seconds (None = no heartbeat)
    pub heartbeat_seconds: Option<u64>,
    /// Event triggers
    pub triggers: Vec<TriggerType>,
    /// Whether schedule is active
    pub active: bool,
    /// Optional business hours restriction
    pub business_hours: Option<BusinessHours>,
}

impl AgentSchedule {
    /// Create a new schedule with no heartbeat or triggers
    pub fn new() -> Self {
        Self {
            heartbeat_seconds: None,
            triggers: Vec::new(),
            active: true,
            business_hours: None,
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

    /// Set schedule active state
    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Pause schedule
    pub fn pause(&mut self) {
        self.active = false;
    }

    /// Resume schedule
    pub fn resume(&mut self) {
        self.active = true;
    }

    /// Set business hours
    pub fn with_business_hours(mut self, hours: BusinessHours) -> Self {
        self.business_hours = Some(hours);
        self
    }

    /// Check if schedule is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Check whether a given time is allowed by schedule
    pub fn allows_time(&self, day: Weekday, time: BusinessTime) -> bool {
        if !self.active {
            return false;
        }
        match &self.business_hours {
            Some(hours) => hours.allows(day, time),
            None => true,
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), "schedule".to_string()]];
        tags.push(vec![
            "active".to_string(),
            self.active.to_string(),
        ]);

        // Add heartbeat tag if present
        if let Some(seconds) = self.heartbeat_seconds {
            tags.push(vec!["heartbeat".to_string(), seconds.to_string()]);
        }

        // Add trigger tags
        for trigger in &self.triggers {
            tags.push(vec!["trigger".to_string(), trigger.to_tag_value()]);
        }

        if let Some(hours) = &self.business_hours {
            tags.push(vec![
                "business_hours".to_string(),
                hours.to_tag_value(),
            ]);
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
        if let Some(hours) = &self.business_hours {
            hours.validate()?;
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
    fn test_schedule_pause_resume() {
        let mut schedule = AgentSchedule::new();
        assert!(schedule.is_active());
        schedule.pause();
        assert!(!schedule.is_active());
        schedule.resume();
        assert!(schedule.is_active());
    }

    #[test]
    fn test_business_hours_allows_time() {
        let hours = BusinessHours::new(
            vec![Weekday::Mon, Weekday::Tue],
            BusinessTime::new(9, 0).unwrap(),
            BusinessTime::new(17, 0).unwrap(),
        )
        .unwrap();

        assert!(hours.allows(Weekday::Mon, BusinessTime::new(10, 0).unwrap()));
        assert!(!hours.allows(Weekday::Sun, BusinessTime::new(10, 0).unwrap()));
        assert!(!hours.allows(Weekday::Mon, BusinessTime::new(18, 0).unwrap()));
    }

    #[test]
    fn test_schedule_with_business_hours_tags() {
        let hours = BusinessHours::new(
            vec![Weekday::Mon, Weekday::Tue],
            BusinessTime::new(9, 0).unwrap(),
            BusinessTime::new(17, 0).unwrap(),
        )
        .unwrap();

        let schedule = AgentSchedule::new().with_business_hours(hours);
        let tags = schedule.build_tags();

        assert!(tags.iter().any(|t| {
            t[0] == "business_hours" && t[1] == "mon,tue 09:00-17:00"
        }));
    }

    #[test]
    fn test_schedule_allows_time_respects_pause() {
        let hours = BusinessHours::new(
            vec![Weekday::Mon],
            BusinessTime::new(9, 0).unwrap(),
            BusinessTime::new(17, 0).unwrap(),
        )
        .unwrap();

        let mut schedule = AgentSchedule::new().with_business_hours(hours);
        assert!(schedule.allows_time(Weekday::Mon, BusinessTime::new(10, 0).unwrap()));

        schedule.pause();
        assert!(!schedule.allows_time(Weekday::Mon, BusinessTime::new(10, 0).unwrap()));
    }

    #[test]
    fn test_business_hours_validation() {
        assert!(BusinessTime::new(24, 0).is_err());

        let invalid = BusinessHours::new(
            vec![],
            BusinessTime::new(9, 0).unwrap(),
            BusinessTime::new(17, 0).unwrap(),
        );
        assert!(invalid.is_err());

        let invalid_range = BusinessHours::new(
            vec![Weekday::Mon],
            BusinessTime::new(17, 0).unwrap(),
            BusinessTime::new(9, 0).unwrap(),
        );
        assert!(invalid_range.is_err());
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
        assert!(tags.iter().any(|t| t[0] == "active" && t[1] == "true"));
        assert!(tags.iter().any(|t| t[0] == "heartbeat" && t[1] == "900"));
        assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "mention"));
        assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "dm"));
    }

    #[test]
    fn test_schedule_tags_no_heartbeat() {
        let schedule = AgentSchedule::new()
            .add_trigger(TriggerType::Mention)
            .add_trigger(TriggerType::Zap);

        let tags = schedule.build_tags();

        assert_eq!(tags[0], vec!["d", "schedule"]);
        assert!(tags.iter().any(|t| t[0] == "active" && t[1] == "true"));
        assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "mention"));
        assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "zap"));
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
