//! Plan line component for planning steps.

use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_CONTENT_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::LineMeta;

/// A plan step line showing planning output.
pub struct PlanLine {
    pub content: String,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub phase: Option<String>,
}

impl PlanLine {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            step: None,
            elapsed: None,
            phase: None,
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

    pub fn phase(mut self, phase: &str) -> Self {
        self.phase = Some(phase.to_string());
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
            div class={ (LINE_CARD_CLASS) " border-l-2 border-cyan" } {
                div class={ "flex items-center gap-2 " (LINE_HEADER_CLASS) } {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Plan))
                    @if let Some(ref phase) = self.phase {
                        span class="text-xs text-cyan uppercase" { (phase) }
                    }
                    span class="flex-1" {}
                    (meta.build())
                }
                div class={ (LINE_CONTENT_CLASS) " whitespace-pre-wrap" } {
                    (self.content)
                }
            }
        }
    }
}
