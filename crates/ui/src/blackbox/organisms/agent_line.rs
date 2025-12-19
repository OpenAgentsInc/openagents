use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_CONTENT_CLASS, LINE_FOOTER_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::{LineMeta, metrics_footer};

pub struct AgentLine {
    pub message: String,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub metrics: Option<(u32, u32, Option<u32>, f64)>,
}

impl AgentLine {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            step: None,
            elapsed: None,
            metrics: None,
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

    pub fn metrics(mut self, prompt: u32, completion: u32, cached: Option<u32>, cost: f64) -> Self {
        self.metrics = Some((prompt, completion, cached, cost));
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
            div class=(LINE_CARD_CLASS) {
                div class={ "flex items-center gap-2 " (LINE_HEADER_CLASS) } {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Agent))
                    span class="flex-1" {}
                    (meta.build())
                }
                div class=(LINE_CONTENT_CLASS) {
                    (self.message)
                }
                @if let Some((prompt, completion, cached, cost)) = self.metrics {
                    div class=(LINE_FOOTER_CLASS) {
                        (metrics_footer(prompt, completion, cached, cost))
                    }
                }
            }
        }
    }
}
