use std::path::PathBuf;

use autopilot::metrics::{SessionMetrics, SummaryStats};
use autopilot::parallel::AgentInfo;

pub struct AppState {
    pub sessions: Vec<SessionMetrics>,
    pub summary: SummaryStats,
    pub agents: Vec<AgentInfo>,
    pub log_lines: Vec<String>,
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            summary: SummaryStats::default(),
            agents: Vec::new(),
            log_lines: Vec::new(),
            log_path: None,
            log_session_id: None,
            status_message: None,
        }
    }
}
