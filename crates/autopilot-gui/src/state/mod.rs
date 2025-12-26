use std::collections::HashSet;
use std::path::PathBuf;

use autopilot::daemon::supervisor::DaemonMetrics;
use autopilot::metrics::{SessionMetrics, SummaryStats};
use autopilot::parallel::AgentInfo;
use wgpui::components::atoms::{ToolStatus, ToolType};

#[derive(Clone, Debug, PartialEq)]
pub enum ChatEntry {
    User {
        text: String,
        timestamp: Option<String>,
    },
    Assistant {
        text: String,
        timestamp: Option<String>,
        streaming: bool,
    },
    System {
        text: String,
        timestamp: Option<String>,
    },
    ToolCall(ToolCallData),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCallData {
    pub id: String,
    pub name: String,
    pub tool_type: ToolType,
    pub status: ToolStatus,
    pub input: Option<String>,
    pub output: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueSummary {
    pub number: i32,
    pub title: String,
    pub priority: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParallelPlatformInfo {
    pub platform: String,
    pub max_agents: usize,
    pub memory_limit: String,
    pub cpu_limit: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEntry {
    pub label: String,
    pub timestamp: Option<String>,
}

/// APM tier classification for color-coded display
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ApmTier {
    /// 0-5 APM: Baseline performance (gray)
    Baseline,
    /// 5-15 APM: Active work (blue)
    Active,
    /// 15-30 APM: Productive performance (green)
    Productive,
    /// 30-50 APM: High performance (amber)
    HighPerformance,
    /// 50+ APM: Elite performance (gold)
    Elite,
}

impl ApmTier {
    /// Get the tier for a given APM value
    pub fn from_apm(apm: f64) -> Self {
        if apm < 5.0 {
            Self::Baseline
        } else if apm < 15.0 {
            Self::Active
        } else if apm < 30.0 {
            Self::Productive
        } else if apm < 50.0 {
            Self::HighPerformance
        } else {
            Self::Elite
        }
    }

    /// Get the display name for this tier
    pub fn name(&self) -> &'static str {
        match self {
            Self::Baseline => "Baseline",
            Self::Active => "Active",
            Self::Productive => "Productive",
            Self::HighPerformance => "High Performance",
            Self::Elite => "Elite",
        }
    }
}

pub struct AppState {
    pub sessions: Vec<SessionMetrics>,
    pub summary: SummaryStats,
    pub agents: Vec<AgentInfo>,
    pub open_issues: Vec<IssueSummary>,
    pub parallel_platform: ParallelPlatformInfo,
    pub chat_entries: Vec<ChatEntry>,
    pub chat_revision: u64,
    pub prompt_running: bool,
    pub prompt_last: Option<String>,
    pub full_auto_metrics: Option<DaemonMetrics>,
    pub log_path: Option<PathBuf>,
    pub log_session_id: Option<String>,
    pub status_message: Option<String>,
    /// Current APM value (from active session or most recent)
    pub current_apm: Option<f64>,
    /// Set of chat entry indices with expanded thinking blocks
    pub thinking_expanded: HashSet<usize>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_status(&mut self, message: Option<String>) {
        self.status_message = message;
    }

    pub fn set_chat_entries(&mut self, entries: Vec<ChatEntry>) {
        self.chat_entries = entries;
        self.chat_revision = self.chat_revision.wrapping_add(1);
    }

    /// Set thinking block expansion for a given chat entry index
    pub fn set_thinking_expanded(&mut self, index: usize, expanded: bool) {
        if expanded {
            self.thinking_expanded.insert(index);
        } else {
            self.thinking_expanded.remove(&index);
        }
        self.chat_revision = self.chat_revision.wrapping_add(1);
    }

    /// Check if thinking block is expanded for a given chat entry index
    pub fn is_thinking_expanded(&self, index: usize) -> bool {
        self.thinking_expanded.contains(&index)
    }

    pub fn active_session(&self) -> Option<&SessionMetrics> {
        if let Some(id) = self.log_session_id.as_ref() {
            if let Some(session) = self.sessions.iter().find(|session| &session.id == id) {
                return Some(session);
            }
        }
        self.sessions.first()
    }

    pub fn session_error_rate(&self) -> Option<f64> {
        let session = self.active_session()?;
        if session.tool_calls <= 0 {
            return None;
        }
        Some(session.tool_errors as f64 / session.tool_calls as f64)
    }

    pub fn session_cost_usd(&self) -> Option<f64> {
        self.active_session().map(|session| session.cost_usd)
    }

    pub fn timeline_entries(&self, limit: usize) -> Vec<TimelineEntry> {
        if limit == 0 || self.chat_entries.is_empty() {
            return Vec::new();
        }

        let start = self.chat_entries.len().saturating_sub(limit);
        self.chat_entries[start..]
            .iter()
            .map(|entry| TimelineEntry {
                label: timeline_label(entry),
                timestamp: chat_entry_timestamp(entry).cloned(),
            })
            .collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            summary: SummaryStats::default(),
            agents: Vec::new(),
            open_issues: Vec::new(),
            parallel_platform: ParallelPlatformInfo {
                platform: "unknown".to_string(),
                max_agents: 0,
                memory_limit: "-".to_string(),
                cpu_limit: "-".to_string(),
            },
            chat_entries: Vec::new(),
            chat_revision: 0,
            prompt_running: false,
            prompt_last: None,
            full_auto_metrics: None,
            log_path: None,
            log_session_id: None,
            status_message: None,
            current_apm: None,
            thinking_expanded: HashSet::new(),
        }
    }
}

fn chat_entry_timestamp(entry: &ChatEntry) -> Option<&String> {
    match entry {
        ChatEntry::User { timestamp, .. } => timestamp.as_ref(),
        ChatEntry::Assistant { timestamp, .. } => timestamp.as_ref(),
        ChatEntry::System { timestamp, .. } => timestamp.as_ref(),
        ChatEntry::ToolCall(_) => None,
    }
}

fn tool_status_label(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Pending => "pending",
        ToolStatus::Running => "running",
        ToolStatus::Success => "success",
        ToolStatus::Error => "error",
        ToolStatus::Cancelled => "cancelled",
    }
}

fn timeline_label(entry: &ChatEntry) -> String {
    match entry {
        ChatEntry::User { .. } => "User prompt".to_string(),
        ChatEntry::Assistant { streaming, .. } => {
            if *streaming {
                "Assistant (streaming)".to_string()
            } else {
                "Assistant response".to_string()
            }
        }
        ChatEntry::System { .. } => "System message".to_string(),
        ChatEntry::ToolCall(tool) => {
            format!("Tool {} ({})", tool.name, tool_status_label(tool.status))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopilot::metrics::SessionStatus;
    use chrono::Utc;

    fn make_session(id: &str, tool_calls: i32, tool_errors: i32, cost: f64) -> SessionMetrics {
        SessionMetrics {
            id: id.to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "test".to_string(),
            duration_seconds: 60.0,
            tokens_in: 0,
            tokens_out: 0,
            tokens_cached: 0,
            cost_usd: cost,
            issues_claimed: 0,
            issues_completed: 0,
            tool_calls,
            tool_errors,
            final_status: SessionStatus::Completed,
            messages: 0,
            apm: None,
            source: "autopilot".to_string(),
            issue_numbers: None,
            directive_id: None,
        }
    }

    #[test]
    fn test_apm_tier_baseline() {
        assert_eq!(ApmTier::from_apm(0.0), ApmTier::Baseline);
        assert_eq!(ApmTier::from_apm(4.9), ApmTier::Baseline);
        assert_eq!(ApmTier::Baseline.name(), "Baseline");
    }

    #[test]
    fn test_apm_tier_active() {
        assert_eq!(ApmTier::from_apm(5.0), ApmTier::Active);
        assert_eq!(ApmTier::from_apm(14.9), ApmTier::Active);
        assert_eq!(ApmTier::Active.name(), "Active");
    }

    #[test]
    fn test_apm_tier_productive() {
        assert_eq!(ApmTier::from_apm(15.0), ApmTier::Productive);
        assert_eq!(ApmTier::from_apm(29.9), ApmTier::Productive);
        assert_eq!(ApmTier::Productive.name(), "Productive");
    }

    #[test]
    fn test_apm_tier_high_performance() {
        assert_eq!(ApmTier::from_apm(30.0), ApmTier::HighPerformance);
        assert_eq!(ApmTier::from_apm(49.9), ApmTier::HighPerformance);
        assert_eq!(ApmTier::HighPerformance.name(), "High Performance");
    }

    #[test]
    fn test_apm_tier_elite() {
        assert_eq!(ApmTier::from_apm(50.0), ApmTier::Elite);
        assert_eq!(ApmTier::from_apm(100.0), ApmTier::Elite);
        assert_eq!(ApmTier::Elite.name(), "Elite");
    }

    #[test]
    fn test_app_state_default_has_no_apm() {
        let state = AppState::default();
        assert!(state.current_apm.is_none());
    }

    #[test]
    fn test_active_session_prefers_log_session_id() {
        let mut state = AppState::default();
        state.sessions = vec![make_session("first", 0, 0, 0.0), make_session("second", 3, 1, 1.5)];
        state.log_session_id = Some("second".to_string());

        let session = state.active_session().expect("active session");
        assert_eq!(session.id, "second");
    }

    #[test]
    fn test_session_error_rate_handles_zero_calls() {
        let mut state = AppState::default();
        state.sessions = vec![make_session("session", 0, 0, 0.0)];
        assert_eq!(state.session_error_rate(), None);
    }

    #[test]
    fn test_session_error_rate_calculates_ratio() {
        let mut state = AppState::default();
        state.sessions = vec![make_session("session", 10, 2, 0.0)];
        let rate = state.session_error_rate().expect("rate");
        assert!((rate - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_session_cost_usd_uses_active_session() {
        let mut state = AppState::default();
        state.sessions = vec![make_session("session", 1, 0, 4.25)];
        assert_eq!(state.session_cost_usd(), Some(4.25));
    }

    #[test]
    fn test_timeline_entries_use_recent_activity() {
        let mut state = AppState::default();
        state.chat_entries = vec![
            ChatEntry::User {
                text: "hello".to_string(),
                timestamp: Some("t1".to_string()),
            },
            ChatEntry::ToolCall(ToolCallData {
                id: "tool-1".to_string(),
                name: "Read".to_string(),
                tool_type: ToolType::Read,
                status: ToolStatus::Success,
                input: None,
                output: None,
            }),
            ChatEntry::Assistant {
                text: "done".to_string(),
                timestamp: Some("t2".to_string()),
                streaming: false,
            },
        ];

        let entries = state.timeline_entries(2);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Tool Read (success)");
        assert_eq!(entries[0].timestamp, None);
        assert_eq!(entries[1].label, "Assistant response");
        assert_eq!(entries[1].timestamp.as_deref(), Some("t2"));
    }
}
