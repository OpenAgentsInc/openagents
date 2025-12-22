//! Thinking line component for Claude's thinking blocks.

use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_CONTENT_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::LineMeta;

/// A thinking block line showing Claude's internal reasoning.
pub struct ThinkingLine {
    pub content: String,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub collapsed: bool,
}

impl ThinkingLine {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            step: None,
            elapsed: None,
            collapsed: true, // Collapsed by default
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

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.collapsed = !expanded;
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

        // Truncate for summary
        let summary: String = self.content.chars().take(80).collect();
        let has_more = self.content.len() > 80;

        html! {
            details class={ (LINE_CARD_CLASS) " border-l-2 border-yellow opacity-75" }
                open[!self.collapsed] {
                summary class="cursor-pointer list-none" {
                    div class={ "flex items-center gap-2 " (LINE_HEADER_CLASS) } {
                        (status_dot(StatusState::Success))
                        (line_type_label(LineType::Thinking))
                        span class="text-muted-foreground italic text-sm truncate flex-1" {
                            (summary)
                            @if has_more { "..." }
                        }
                        (meta.build())
                    }
                }
                div class={ (LINE_CONTENT_CLASS) " text-muted-foreground italic whitespace-pre-wrap" } {
                    (self.content)
                }
            }
        }
    }
}
