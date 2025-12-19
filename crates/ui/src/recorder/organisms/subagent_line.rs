use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, result_arrow, status_dot, tid_badge};
use super::super::molecules::LineMeta;

pub struct SubagentLine {
    pub agent_type: String,
    pub task: String,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub tid: Option<u8>,
    pub session_id: Option<String>,
    pub summary: Option<String>,
}

impl SubagentLine {
    pub fn new(agent_type: &str, task: &str) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            task: task.to_string(),
            step: None,
            elapsed: None,
            tid: None,
            session_id: None,
            summary: None,
        }
    }

    pub fn step(mut self, step: u32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn elapsed(mut self, h: u8, m: u8, s: u8) -> Self {
        self.elapsed = Some((h, m, s));
        self
    }

    pub fn tid(mut self, tid: u8) -> Self {
        self.tid = Some(tid);
        self
    }

    pub fn session_id(mut self, id: &str) -> Self {
        self.session_id = Some(id.to_string());
        self
    }

    pub fn summary(mut self, summary: &str) -> Self {
        self.summary = Some(summary.to_string());
        self
    }

    pub fn build(self) -> Markup {
        let mut meta = LineMeta::new();
        if let Some(s) = self.step {
            meta = meta.step(s);
        }
        if let Some((h, m, s)) = self.elapsed {
            meta = meta.elapsed(h, m, s);
        }

        html! {
            div class={ (LINE_CARD_CLASS) " border-l-2 border-red" } {
                div class=(LINE_HEADER_CLASS) {
                    div class="flex items-center gap-2" {
                        (status_dot(StatusState::Running))
                        (line_type_label(LineType::Subagent))
                        span class="text-red font-medium" { (self.agent_type) }
                        span class="flex-1" {}
                        (meta.build())
                    }
                    div class="pl-6 text-sm text-muted-foreground mt-1 italic" {
                        "\"" (self.task) "\""
                    }
                }
                div class="px-3 py-3 border-t border-border" {
                    div class="flex items-center gap-4 text-xs mb-2" {
                        @if let Some(t) = self.tid {
                            (tid_badge(t))
                        }
                        @if let Some(ref id) = self.session_id {
                            span class="text-muted-foreground" { "session_id=" (id) }
                        }
                    }
                    @if let Some(ref summary) = self.summary {
                        div class="flex items-center gap-1" {
                            (result_arrow())
                            span class="text-muted-foreground" { "summary: \"" (summary) "\"" }
                        }
                        a href="#" class="text-cyan text-xs ml-6" {
                            "[View full trajectory \u{2192}]"
                        }
                    }
                }
            }
        }
    }
}
