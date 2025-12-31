use std::path::PathBuf;
use std::time::Instant;

use autopilot::{
    ClaudeEvent, ClaudeModel, ClaudeUsageData, LogLine, LogStatus, SessionCheckpoint, StartupPhase,
    StartupSection, StartupState,
};
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
    pub model: ClaudeModel,
    pub lines: Vec<LogLine>,
    /// Grouped sections for collapsible UI (startup phases only).
    pub sections: Vec<LogSection>,
    pub events: Vec<SessionEvent>,
    /// Accumulated session usage stats (tokens, cost, duration).
    pub session_usage: ClaudeUsageData,
    pub autopilot_session_id: String,
    pub sdk_session_ids: SdkSessionIds,
}

pub struct AutopilotRuntime {
    started_at: Instant,
    state: StartupState,
    plan_cursor: usize,
    exec_cursor: usize,
    review_cursor: usize,
    fix_cursor: usize,
}

impl AutopilotRuntime {
    pub fn new(model: ClaudeModel) -> Self {
        Self {
            started_at: Instant::now(),
            state: StartupState::with_model(model),
            plan_cursor: 0,
            exec_cursor: 0,
            review_cursor: 0,
            fix_cursor: 0,
        }
    }

    pub fn tick(&mut self) {
        let elapsed = self.started_at.elapsed().as_secs_f32();
        self.state.tick(elapsed);
    }

    pub fn snapshot(&mut self) -> RuntimeSnapshot {
        let mut events = Vec::new();
        Self::append_events(
            &self.state.claude_events,
            SessionPhase::Plan,
            &mut self.plan_cursor,
            &mut events,
        );
        Self::append_events(
            &self.state.exec_events,
            SessionPhase::Execute,
            &mut self.exec_cursor,
            &mut events,
        );
        Self::append_events(
            &self.state.review_events,
            SessionPhase::Review,
            &mut self.review_cursor,
            &mut events,
        );
        Self::append_events(
            &self.state.fix_events,
            SessionPhase::Fix,
            &mut self.fix_cursor,
            &mut events,
        );

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
                plan: self.state.claude_session_id.clone(),
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
                if section != StartupSection::Claude {
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
            self.plan_cursor,
            self.exec_cursor,
            self.review_cursor,
            self.fix_cursor,
            working_dir,
        );
        checkpoint.save()
    }

    /// Create a runtime from a saved checkpoint.
    pub fn from_checkpoint(cp: SessionCheckpoint) -> Self {
        let plan_cursor = cp.plan_cursor;
        let exec_cursor = cp.exec_cursor;
        let review_cursor = cp.review_cursor;
        let fix_cursor = cp.fix_cursor;

        Self {
            started_at: Instant::now(),
            state: StartupState::from_checkpoint(cp),
            plan_cursor,
            exec_cursor,
            review_cursor,
            fix_cursor,
        }
    }

    /// Load a runtime from a checkpoint file.
    pub fn load_checkpoint(session_id: &str) -> Result<Self, std::io::Error> {
        let checkpoint = SessionCheckpoint::load(session_id)?;
        Ok(Self::from_checkpoint(checkpoint))
    }

    /// Set the model for this runtime.
    pub fn set_model(&mut self, model: ClaudeModel) {
        self.state.model = model;
    }

    /// Get the current model.
    pub fn model(&self) -> ClaudeModel {
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
    pub fn reset(&mut self, model: ClaudeModel) {
        self.started_at = Instant::now();
        self.state = StartupState::with_model(model);
        self.plan_cursor = 0;
        self.exec_cursor = 0;
        self.review_cursor = 0;
        self.fix_cursor = 0;
    }

    fn append_events(
        source: &[ClaudeEvent],
        phase: SessionPhase,
        cursor: &mut usize,
        out: &mut Vec<SessionEvent>,
    ) {
        if *cursor >= source.len() {
            return;
        }

        for event in &source[*cursor..] {
            match event {
                ClaudeEvent::Text(content) => out.push(SessionEvent::Text {
                    phase,
                    content: content.clone(),
                }),
                ClaudeEvent::Tool {
                    name,
                    params,
                    done,
                    output,
                    is_error,
                } => out.push(SessionEvent::Tool {
                    phase,
                    name: name.clone(),
                    params: params.clone(),
                    done: *done,
                    output: output.clone(),
                    is_error: *is_error,
                }),
                ClaudeEvent::ToolProgress {
                    tool_name,
                    elapsed_secs,
                } => out.push(SessionEvent::ToolProgress {
                    phase,
                    tool_name: tool_name.clone(),
                    elapsed_secs: *elapsed_secs,
                }),
            }
        }

        *cursor = source.len();
    }
}

impl Default for AutopilotRuntime {
    fn default() -> Self {
        Self::new(ClaudeModel::default())
    }
}
