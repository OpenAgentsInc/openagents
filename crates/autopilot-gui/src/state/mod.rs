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
        }
    }
}
