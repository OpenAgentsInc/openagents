use serde::{Deserialize, Serialize};

/// Agent event types for streaming output
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentEvent {
    Text(String),
    Tool {
        name: String,
        #[serde(default)]
        params: serde_json::Value,
        output: Option<String>,
        done: bool,
        #[serde(default)]
        is_error: bool,
    },
    ToolProgress {
        name: String,
        elapsed: f32,
    },
}

/// Token usage data for tracking costs
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UsageData {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    #[serde(default)]
    pub total_cost_usd: f64,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
    pub num_turns: Option<u64>,
    pub context_window: Option<u64>,
    pub model: Option<String>,
}

/// Token event for streaming
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentToken {
    Text(String),
    Chunk(String),
    Usage(UsageData),
    Done(String),
    SessionId(String),
    Error(String),
    ToolUse {
        name: String,
        params: serde_json::Value,
    },
    ToolDone {
        name: String,
        output: String,
        is_error: bool,
    },
    Progress {
        name: String,
        elapsed_secs: f64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum AgentModel {
    #[default]
    Sonnet,
    Opus,
}

impl AgentModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentModel::Sonnet => "sonnet",
            AgentModel::Opus => "opus",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub text: String,
    #[allow(dead_code)]
    pub timestamp: f32,
    pub status: LogStatus,
    /// Which UI section this line belongs to for collapsible grouping.
    pub section: Option<StartupSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LogStatus {
    Pending,
    Success,
    Error,
    Info,
    Thinking,
}

/// Logical groupings of startup phases for collapsible UI sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StartupSection {
    Auth,
    Preflight,
    Tools,
    Pylon,
    Compute,
    Agent,
}

impl StartupSection {
    /// Generate a summary text for this section based on its detail lines.
    pub fn summary_text(&self, details: &[LogLine]) -> String {
        match self {
            StartupSection::Auth => {
                let has_auth = details.iter().any(|l| l.text.contains("Auth ready"));
                if has_auth {
                    "Auth ready".to_string()
                } else {
                    "Checking auth".to_string()
                }
            }
            StartupSection::Preflight => {
                let directive_count = details
                    .iter()
                    .find(|l| l.text.contains("directives"))
                    .and_then(|l| l.text.split_whitespace().find(|s| s.parse::<u32>().is_ok()))
                    .unwrap_or("0");
                format!("Preflight complete ({} directives)", directive_count)
            }
            StartupSection::Tools => {
                let ok_count = details.iter().filter(|l| l.text.contains("[OK]")).count();
                format!("{} tools ready", ok_count)
            }
            StartupSection::Pylon => {
                let running = details
                    .iter()
                    .any(|l| l.text.contains("running") || l.text.contains("started"));
                if running {
                    "Pylon ready".to_string()
                } else {
                    "Pylon not running".to_string()
                }
            }
            StartupSection::Compute => {
                let backends = details.iter().filter(|l| l.text.contains("[OK]")).count();
                format!("Compute: {} local backend(s)", backends)
            }
            StartupSection::Agent => "Agent session".to_string(),
        }
    }

    pub fn summary_status(&self, details: &[LogLine]) -> LogStatus {
        if details.iter().any(|l| l.status == LogStatus::Error) {
            LogStatus::Error
        } else if details
            .iter()
            .all(|l| l.status == LogStatus::Success || l.status == LogStatus::Info)
        {
            LogStatus::Success
        } else if details.iter().any(|l| l.status == LogStatus::Pending) {
            LogStatus::Pending
        } else {
            LogStatus::Info
        }
    }
}
