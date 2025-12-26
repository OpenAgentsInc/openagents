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

#[cfg(test)]
mod tests {
    use super::*;

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
}
