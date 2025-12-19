use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, result_arrow, status_dot};
use super::super::molecules::LineMeta;

pub struct QuestionLine {
    pub question: String,
    pub options: Vec<String>,
    pub selected: Option<String>,
    pub auto_reason: Option<String>,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
}

impl QuestionLine {
    pub fn new(question: &str) -> Self {
        Self {
            question: question.to_string(),
            options: vec![],
            selected: None,
            auto_reason: None,
            step: None,
            elapsed: None,
        }
    }

    pub fn options(mut self, options: Vec<&str>) -> Self {
        self.options = options.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn selected(mut self, selected: &str) -> Self {
        self.selected = Some(selected.to_string());
        self
    }

    pub fn auto_selected(mut self, selected: &str, reason: &str) -> Self {
        self.selected = Some(selected.to_string());
        self.auto_reason = Some(reason.to_string());
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

    pub fn build(self) -> Markup {
        let mut meta = LineMeta::new();
        if let Some(s) = self.step {
            meta = meta.step(s);
        }
        if let Some((h, m, s)) = self.elapsed {
            meta = meta.elapsed(h, m, s);
        }

        let status = if self.selected.is_some() {
            StatusState::Success
        } else {
            StatusState::Pending
        };

        html! {
            div class=(LINE_CARD_CLASS) {
                div class=(LINE_HEADER_CLASS) {
                    div class="flex items-center gap-2" {
                        (status_dot(status))
                        (line_type_label(LineType::Question))
                        span class="flex-1" {}
                        (meta.build())
                    }
                    div class="pl-6 text-sm text-foreground mt-1" {
                        "\"" (self.question) "\""
                    }
                }
                @if !self.options.is_empty() {
                    div class="px-3 py-3 border-t border-border" {
                        div class="text-xs text-muted-foreground mb-2" { "Options:" }
                        @for opt in &self.options {
                            @let is_selected = self.selected.as_ref() == Some(opt);
                            div class="pl-4 text-sm py-1" {
                                @if is_selected {
                                    span class="text-green" { "\u{25CF} " (opt) }
                                    span class="text-muted-foreground ml-2" { "\u{2190} selected" }
                                } @else {
                                    span class="text-muted-foreground" { "\u{25CB} " (opt) }
                                }
                            }
                        }
                    }
                }
                div class="px-3 py-2 border-t border-border" {
                    @if let Some(ref sel) = self.selected {
                        @if let Some(ref reason) = self.auto_reason {
                            div class="flex items-center gap-1" {
                                (result_arrow())
                                span class="text-yellow" { "[auto: " (sel) ", reason=\"" (reason) "\"]" }
                            }
                        } @else {
                            div class="flex items-center gap-1" {
                                (result_arrow())
                                span class="text-green" { "[selected: " (sel) "]" }
                            }
                        }
                    } @else {
                        div class="flex items-center gap-1" {
                            (result_arrow())
                            span class="text-yellow" { "[pending]" }
                        }
                    }
                }
            }
        }
    }
}
