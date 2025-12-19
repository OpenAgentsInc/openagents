use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, result_arrow, status_dot};
use super::super::molecules::LineMeta;

pub struct RecallLine {
    pub queries: Vec<String>,
    pub matches: Vec<(String, String)>,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
}

impl RecallLine {
    pub fn new(queries: Vec<&str>) -> Self {
        Self {
            queries: queries.iter().map(|s| s.to_string()).collect(),
            matches: vec![],
            step: None,
            elapsed: None,
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

    pub fn add_match(mut self, session_id: &str, summary: &str) -> Self {
        self.matches
            .push((session_id.to_string(), summary.to_string()));
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
                div class=(LINE_HEADER_CLASS) {
                    div class="flex items-center gap-2" {
                        (status_dot(StatusState::Success))
                        (line_type_label(LineType::Recall))
                        span class="flex-1" {}
                        (meta.build())
                    }
                    div class="pl-6 mt-1" {
                        @for q in &self.queries {
                            span class="text-cyan text-xs mr-2" {
                                "\"" (q) "\""
                            }
                        }
                    }
                }
                div class="px-3 py-3 border-t border-border" {
                    div class="flex items-center gap-1 mb-2" {
                        (result_arrow())
                        span class="text-muted-foreground" { "[" (self.matches.len()) " matches]" }
                    }
                    @for (session_id, summary) in &self.matches {
                        div class="pl-4 text-xs text-muted-foreground py-1" {
                            span class="text-cyan" { (session_id) }
                            ": " (summary)
                        }
                    }
                }
            }
        }
    }
}
