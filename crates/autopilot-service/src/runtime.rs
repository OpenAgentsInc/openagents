use std::path::PathBuf;
use std::time::Instant;

use autopilot_core::{
    AgentModel, UsageData, LogLine, LogStatus, SessionCheckpoint, StartupPhase,
    StartupSection, StartupState, ACP_PHASE_META_KEY, ACP_TOOL_NAME_META_KEY,
    ACP_TOOL_PROGRESS_META_KEY,
};
use agent_client_protocol_schema as acp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionPhase {
    Plan,
    Execute,
    Review,
    Fix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEvent {
    Text {
        phase: SessionPhase,
        content: String,
    },
    Tool {
        phase: SessionPhase,
        name: String,
        params: String,
        done: bool,
        output: Option<String>,
        is_error: bool,
    },
    ToolProgress {
        phase: SessionPhase,
        tool_name: String,
        elapsed_secs: f64,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SdkSessionIds {
    pub plan: Option<String>,
    pub exec: Option<String>,
    pub review: Option<String>,
    pub fix: Option<String>,
}

/// A grouped section of log lines for collapsible UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSection {
    pub section: StartupSection,
    pub summary: String,
    pub summary_status: LogStatus,
    pub details: Vec<LogLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub phase: StartupPhase,
    pub model: AgentModel,
    pub lines: Vec<LogLine>,
    /// Grouped sections for collapsible UI (startup phases only).
    pub sections: Vec<LogSection>,
    pub events: Vec<SessionEvent>,
    /// Accumulated session usage stats (tokens, cost, duration).
    pub session_usage: UsageData,
    pub autopilot_session_id: String,
    pub sdk_session_ids: SdkSessionIds,
}

pub struct AutopilotRuntime {
    started_at: Instant,
    state: StartupState,
    acp_cursor: usize,
}

impl AutopilotRuntime {
    pub fn new(model: AgentModel) -> Self {
        Self {
            started_at: Instant::now(),
            state: StartupState::with_model(model),
            acp_cursor: 0,
        }
    }

    /// Create a new runtime in idle state, waiting for user input.
    pub fn new_idle(model: AgentModel) -> Self {
        Self {
            started_at: Instant::now(),
            state: StartupState::new_idle(model),
            acp_cursor: 0,
        }
    }

    pub fn tick(&mut self) {
        let elapsed = self.started_at.elapsed().as_secs_f32();
        self.state.tick(elapsed);
    }

    pub fn snapshot(&mut self) -> RuntimeSnapshot {
        let mut events = Vec::new();
        Self::append_acp_events(&self.state.acp_events, &mut self.acp_cursor, &mut events);

        // Build sections from log lines for collapsible UI
        let sections = Self::build_sections(&self.state.lines);

        RuntimeSnapshot {
            phase: self.state.phase,
            model: self.state.model,
            lines: self.state.lines.clone(),
            sections,
            events,
            session_usage: self.state.session_usage.clone(),
            autopilot_session_id: self.state.session_id.clone(),
            sdk_session_ids: SdkSessionIds {
                plan: self.state.plan_session_id.clone(),
                exec: self.state.exec_session_id.clone(),
                review: self.state.review_session_id.clone(),
                fix: self.state.fix_session_id.clone(),
            },
        }
    }

    /// Group log lines into sections for collapsible display.
    fn build_sections(lines: &[LogLine]) -> Vec<LogSection> {
        use std::collections::HashMap;

        // Collect lines by section (only non-Claude sections are collapsible)
        let mut section_lines: HashMap<StartupSection, Vec<LogLine>> = HashMap::new();
        let section_order = [
            StartupSection::Auth,
            StartupSection::Preflight,
            StartupSection::Tools,
            StartupSection::Pylon,
            StartupSection::Compute,
        ];

        for line in lines {
            if let Some(section) = line.section {
                // Only group startup sections, not Claude
                if section != StartupSection::Agent {
                    section_lines.entry(section).or_default().push(line.clone());
                }
            }
        }

        // Build sections in order
        let mut sections = Vec::new();
        for section in section_order {
            if let Some(details) = section_lines.get(&section) {
                if !details.is_empty() {
                    let summary = section.summary_text(details);
                    let summary_status = section.summary_status(details);
                    sections.push(LogSection {
                        section,
                        summary,
                        summary_status,
                        details: details.clone(),
                    });
                }
            }
        }

        sections
    }

    pub fn state(&self) -> &StartupState {
        &self.state
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.state.session_id
    }

    /// Save a checkpoint of the current runtime state.
    pub fn save_checkpoint(&self, working_dir: PathBuf) -> Result<PathBuf, std::io::Error> {
        let checkpoint = self.state.create_checkpoint(
            self.acp_cursor,
            working_dir,
        );
        checkpoint.save()
    }

    /// Create a runtime from a saved checkpoint.
    pub fn from_checkpoint(cp: SessionCheckpoint) -> Self {
        let acp_cursor = cp.acp_cursor;

        Self {
            started_at: Instant::now(),
            state: StartupState::from_checkpoint(cp),
            acp_cursor,
        }
    }

    /// Load a runtime from a checkpoint file.
    pub fn load_checkpoint(session_id: &str) -> Result<Self, std::io::Error> {
        let checkpoint = SessionCheckpoint::load(session_id)?;
        Ok(Self::from_checkpoint(checkpoint))
    }

    /// Set the model for this runtime.
    pub fn set_model(&mut self, model: AgentModel) {
        self.state.model = model;
    }

    /// Get the current model.
    pub fn model(&self) -> AgentModel {
        self.state.model
    }

    /// Request interruption of the current operation.
    pub fn interrupt(&mut self) {
        self.state.force_stopped = true;
    }

    /// Check if interruption was requested.
    pub fn is_interrupted(&self) -> bool {
        self.state.force_stopped
    }

    /// Reset runtime to fresh state with specified model.
    pub fn reset(&mut self, model: AgentModel) {
        self.started_at = Instant::now();
        self.state = StartupState::with_model(model);
        self.acp_cursor = 0;
    }

    /// Reset runtime to idle state, waiting for user input.
    pub fn reset_to_idle(&mut self, model: AgentModel) {
        self.started_at = Instant::now();
        self.state = StartupState::new_idle(model);
        self.acp_cursor = 0;
    }

    /// Start a new run with a prompt. If currently idle, begins execution.
    pub fn start_run(&mut self, prompt: String) {
        if self.state.is_idle() {
            self.state.start_with_prompt(prompt);
        } else {
            // If not idle, reset and start fresh
            let model = self.state.model;
            self.reset(model);
            self.state.user_prompt = Some(prompt);
        }
    }

    /// Check if currently running (not idle and not complete).
    pub fn is_running(&self) -> bool {
        self.state.is_running()
    }

    /// Check if in idle state.
    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    /// Get the current user prompt, if any.
    pub fn user_prompt(&self) -> Option<&str> {
        self.state.user_prompt.as_deref()
    }

    fn append_acp_events(
        source: &[acp::SessionNotification],
        cursor: &mut usize,
        out: &mut Vec<SessionEvent>,
    ) {
        if *cursor >= source.len() {
            return;
        }

        for event in &source[*cursor..] {
            if let Some(mapped) = Self::acp_notification_to_session_event(event) {
                out.push(mapped);
            }
        }

        *cursor = source.len();
    }

    fn acp_notification_to_session_event(
        notification: &acp::SessionNotification,
    ) -> Option<SessionEvent> {
        match &notification.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                let (phase, text) = Self::extract_phase_and_text(&chunk.content)?;
                Some(SessionEvent::Text { phase, content: text })
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                let (phase, text) = Self::extract_phase_and_text(&chunk.content)?;
                Some(SessionEvent::Text { phase, content: text })
            }
            acp::SessionUpdate::ToolCall(call) => {
                let phase = Self::extract_phase_from_meta(call.meta.as_ref())?;
                let params = call
                    .raw_input
                    .as_ref()
                    .map(|input| input.to_string())
                    .unwrap_or_default();
                Some(SessionEvent::Tool {
                    phase,
                    name: call.title.clone(),
                    params,
                    done: false,
                    output: None,
                    is_error: false,
                })
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                if let Some(meta) = update.meta.as_ref() {
                    if let Some(progress) = meta.get(ACP_TOOL_PROGRESS_META_KEY) {
                        if let Some(elapsed_secs) = progress.as_f64() {
                            let tool_name = meta
                                .get(ACP_TOOL_NAME_META_KEY)
                                .and_then(|name| name.as_str())
                                .unwrap_or("tool")
                                .to_string();
                            let phase = Self::extract_phase_from_meta(Some(meta))?;
                            return Some(SessionEvent::ToolProgress {
                                phase,
                                tool_name,
                                elapsed_secs,
                            });
                        }
                    }
                }

                let phase = Self::extract_phase_from_meta(update.meta.as_ref())?;
                let status = update
                    .fields
                    .status
                    .unwrap_or(acp::ToolCallStatus::InProgress);
                let done = matches!(
                    status,
                    acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed
                );
                let is_error = matches!(status, acp::ToolCallStatus::Failed);
                let output = update
                    .fields
                    .raw_output
                    .as_ref()
                    .and_then(Self::format_tool_output);
                let name = update
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.get(ACP_TOOL_NAME_META_KEY))
                    .and_then(|value| value.as_str())
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| update.tool_call_id.to_string());
                let params = update
                    .fields
                    .raw_input
                    .as_ref()
                    .map(|input| input.to_string())
                    .unwrap_or_default();
                Some(SessionEvent::Tool {
                    phase,
                    name,
                    params,
                    done,
                    output,
                    is_error,
                })
            }
            _ => None,
        }
    }

    fn extract_phase_and_text(content: &acp::ContentBlock) -> Option<(SessionPhase, String)> {
        let phase = match content {
            acp::ContentBlock::Text(text) => Self::extract_phase_from_meta(text.meta.as_ref())?,
            _ => return None,
        };
        let text = match content {
            acp::ContentBlock::Text(text) => text.text.clone(),
            _ => return None,
        };
        Some((phase, text))
    }

    fn extract_phase_from_meta(meta: Option<&acp::Meta>) -> Option<SessionPhase> {
        let phase = meta?
            .get(ACP_PHASE_META_KEY)
            .and_then(|value| value.as_str())?;
        match phase {
            "plan" => Some(SessionPhase::Plan),
            "exec" => Some(SessionPhase::Execute),
            "review" => Some(SessionPhase::Review),
            "fix" => Some(SessionPhase::Fix),
            _ => None,
        }
    }

    fn format_tool_output(value: &serde_json::Value) -> Option<String> {
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            return Some(content.to_string());
        }
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            return Some(format!("Error: {}", error));
        }
        let stdout = value.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
        let stderr = value.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
        if !stdout.is_empty() || !stderr.is_empty() {
            if !stdout.is_empty() && !stderr.is_empty() {
                return Some(format!("{}\n\nstderr:\n{}", stdout, stderr));
            }
            if !stdout.is_empty() {
                return Some(stdout.to_string());
            }
            if !stderr.is_empty() {
                return Some(stderr.to_string());
            }
        }
        Some(value.to_string())
    }
}

impl Default for AutopilotRuntime {
    fn default() -> Self {
        Self::new(AgentModel::default())
    }
}
