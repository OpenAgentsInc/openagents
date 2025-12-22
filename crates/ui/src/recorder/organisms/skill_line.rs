//! Skill line component for skill invocations.

use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_CONTENT_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::{LineMeta, ResultType, result_display};

/// A skill invocation line.
pub struct SkillLine {
    pub skill_name: String,
    pub args: Option<String>,
    pub result: ResultType,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub call_id: Option<String>,
    pub latency_ms: Option<u32>,
}

impl SkillLine {
    pub fn new(skill_name: &str, result: ResultType) -> Self {
        Self {
            skill_name: skill_name.to_string(),
            args: None,
            result,
            step: None,
            elapsed: None,
            call_id: None,
            latency_ms: None,
        }
    }

    pub fn args(mut self, args: &str) -> Self {
        self.args = Some(args.to_string());
        self
    }

    pub fn step(mut self, step: u32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn elapsed(mut self, h: u8, m: u8, s: u8) -> Self {
        self.elapsed = Some((h, m, s));
        self
    }

    pub fn call_id(mut self, id: &str) -> Self {
        self.call_id = Some(id.to_string());
        self
    }

    pub fn latency(mut self, ms: u32) -> Self {
        self.latency_ms = Some(ms);
        self
    }

    pub fn build(self) -> Markup {
        use super::super::atoms::CallType;

        let status = match &self.result {
            ResultType::Pending => StatusState::Pending,
            ResultType::Error(_) => StatusState::Error,
            _ => StatusState::Success,
        };

        let mut meta = LineMeta::new();
        if let Some(s) = self.step {
            meta = meta.step(s);
        }
        if let Some((h, m, s)) = self.elapsed {
            meta = meta.elapsed(h, m, s);
        }
        if let Some(ref id) = self.call_id {
            meta = meta.call_id(id, CallType::Skill);
        }
        if let Some(ms) = self.latency_ms {
            meta = meta.latency(ms);
        }

        html! {
            div class={ (LINE_CARD_CLASS) " border-l-2 border-magenta" } {
                div class={ "flex items-center gap-2 " (LINE_HEADER_CLASS) } {
                    (status_dot(status))
                    (line_type_label(LineType::Skill))
                    span class="text-magenta font-medium" { "/" (self.skill_name) }
                    @if let Some(ref args) = self.args {
                        span class="text-xs text-muted-foreground" { (args) }
                    }
                    span class="flex-1" {}
                    (meta.build())
                }
                div class=(LINE_CONTENT_CLASS) {
                    (result_display(self.result))
                }
            }
        }
    }
}
