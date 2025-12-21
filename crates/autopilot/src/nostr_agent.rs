//! Nostr-Integrated Agent
//!
//! This module provides a wrapper around autopilot execution that publishes
//! NIP-SA tick events (kind:38010, 38011) to track autonomous agent runs.

use nostr::{
    TickAction, TickRequest, TickResult, TickResultContent, TickStatus, TickTrigger,
    KIND_TICK_REQUEST, KIND_TICK_RESULT,
};
use std::time::SystemTime;

/// Nostr-integrated autonomous agent
pub struct NostrAgent {
    /// Agent runner identifier (pubkey hex)
    runner: String,
}

impl NostrAgent {
    /// Create a new Nostr agent
    pub fn new(runner: impl Into<String>) -> Self {
        Self {
            runner: runner.into(),
        }
    }

    /// Create a tick request for run start
    pub fn create_tick_request(&self, trigger: TickTrigger) -> TickRequest {
        TickRequest::new(&self.runner, trigger)
    }

    /// Create a tick result for run end
    pub fn create_tick_result(
        &self,
        request_id: impl Into<String>,
        status: TickStatus,
        duration_ms: u64,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        goals_updated: u32,
        actions: Vec<TickAction>,
    ) -> TickResult {
        let content = TickResultContent::new(tokens_in, tokens_out, cost_usd, goals_updated)
            .with_actions(actions);

        TickResult::new(request_id, &self.runner, status, duration_ms, content)
    }

    /// Get tick request event kind
    pub fn tick_request_kind() -> u16 {
        KIND_TICK_REQUEST
    }

    /// Get tick result event kind
    pub fn tick_result_kind() -> u16 {
        KIND_TICK_RESULT
    }
}

/// Helper to create a tick trigger from a string
pub fn trigger_from_string(s: &str) -> TickTrigger {
    match s.to_lowercase().as_str() {
        "heartbeat" => TickTrigger::Heartbeat,
        "manual" => TickTrigger::Manual,
        "mention" => TickTrigger::Mention,
        "dm" => TickTrigger::Dm,
        "zap" => TickTrigger::Zap,
        _ => TickTrigger::Manual,
    }
}

/// Helper to get current Unix timestamp
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Tick execution context
pub struct TickContext {
    /// When the tick started (Unix timestamp seconds)
    pub started_at: u64,
    /// When the tick started (milliseconds for duration calculation)
    started_at_ms: u128,
    /// What triggered this tick
    pub trigger: TickTrigger,
    /// Runner identifier
    pub runner: String,
}

impl TickContext {
    /// Create a new tick context
    pub fn new(trigger: TickTrigger, runner: impl Into<String>) -> Self {
        let now = SystemTime::now();
        let started_at = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started_at_ms = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        Self {
            started_at,
            started_at_ms,
            trigger,
            runner: runner.into(),
        }
    }

    /// Calculate tick duration in seconds
    pub fn duration_seconds(&self) -> u64 {
        current_timestamp().saturating_sub(self.started_at)
    }

    /// Calculate tick duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        (now_ms - self.started_at_ms) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nostr_agent_creation() {
        let agent = NostrAgent::new("test-runner-pubkey");
        assert_eq!(agent.runner, "test-runner-pubkey");
    }

    #[test]
    fn test_create_tick_request() {
        let agent = NostrAgent::new("test-runner");
        let request = agent.create_tick_request(TickTrigger::Manual);

        assert_eq!(request.runner, "test-runner");
        assert_eq!(request.trigger, TickTrigger::Manual);

        let tags = request.build_tags();
        assert!(tags.iter().any(|t| t[0] == "runner" && t[1] == "test-runner"));
        assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "manual"));
    }

    #[test]
    fn test_create_tick_result() {
        let agent = NostrAgent::new("test-runner");
        let actions = vec![
            TickAction::new("issue_complete").with_metadata("number", serde_json::json!(123)),
            TickAction::new("issue_complete").with_metadata("number", serde_json::json!(456)),
        ];

        let result = agent.create_tick_result(
            "request-event-id",
            TickStatus::Success,
            5000,
            1000,
            500,
            0.05,
            2,
            actions,
        );

        assert_eq!(result.request_id, "request-event-id");
        assert_eq!(result.runner, "test-runner");
        assert_eq!(result.status, TickStatus::Success);
        assert_eq!(result.duration_ms, 5000);
        assert_eq!(result.content.tokens_in, 1000);
        assert_eq!(result.content.tokens_out, 500);
        assert!((result.content.cost_usd - 0.05).abs() < 0.001);
        assert_eq!(result.content.goals_updated, 2);
        assert_eq!(result.content.actions.len(), 2);
        assert_eq!(result.action_count, 2);

        let tags = result.build_tags();
        assert!(tags
            .iter()
            .any(|t| t[0] == "request" && t[1] == "request-event-id"));
        assert!(tags.iter().any(|t| t[0] == "status" && t[1] == "success"));
        assert!(tags.iter().any(|t| t[0] == "actions" && t[1] == "2"));
    }

    #[test]
    fn test_event_kinds() {
        assert_eq!(NostrAgent::tick_request_kind(), 38010);
        assert_eq!(NostrAgent::tick_result_kind(), 38011);
    }

    #[test]
    fn test_trigger_from_string() {
        assert_eq!(trigger_from_string("heartbeat"), TickTrigger::Heartbeat);
        assert_eq!(trigger_from_string("manual"), TickTrigger::Manual);
        assert_eq!(trigger_from_string("Manual"), TickTrigger::Manual);
        assert_eq!(trigger_from_string("mention"), TickTrigger::Mention);
        assert_eq!(trigger_from_string("dm"), TickTrigger::Dm);
        assert_eq!(trigger_from_string("zap"), TickTrigger::Zap);
        assert_eq!(trigger_from_string("unknown"), TickTrigger::Manual);
    }

    #[test]
    fn test_tick_context() {
        let context = TickContext::new(TickTrigger::Manual, "test");
        assert!(context.started_at > 0);
        assert_eq!(context.trigger, TickTrigger::Manual);
        assert_eq!(context.runner, "test");

        // Duration should be very small (just created)
        let duration = context.duration_seconds();
        assert!(duration < 2); // Should complete in under 2 seconds

        let duration_ms = context.duration_ms();
        assert!(duration_ms < 2000); // Should be under 2000ms
    }

    #[test]
    fn test_current_timestamp() {
        let ts1 = current_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = current_timestamp();
        assert!(ts2 >= ts1);
    }
}
