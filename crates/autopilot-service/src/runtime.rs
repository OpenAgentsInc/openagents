use std::time::Instant;

use autopilot::{ClaudeEvent, ClaudeModel, LogLine, StartupPhase, StartupState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Plan,
    Execute,
    Review,
    Fix,
}

#[derive(Debug, Clone)]
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
    },
}

#[derive(Debug, Clone)]
pub struct RuntimeSnapshot {
    pub phase: StartupPhase,
    pub model: ClaudeModel,
    pub lines: Vec<LogLine>,
    pub events: Vec<SessionEvent>,
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

        RuntimeSnapshot {
            phase: self.state.phase,
            model: self.state.model,
            lines: self.state.lines.clone(),
            events,
        }
    }

    pub fn state(&self) -> &StartupState {
        &self.state
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
                ClaudeEvent::Tool { name, params, done } => out.push(SessionEvent::Tool {
                    phase,
                    name: name.clone(),
                    params: params.clone(),
                    done: *done,
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
