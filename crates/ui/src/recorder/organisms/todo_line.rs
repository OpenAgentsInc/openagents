//! Todo line component for todo list updates.

use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_CONTENT_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::LineMeta;

/// Status of a todo item.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    fn icon(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "\u{25CB}",    // ○
            TodoStatus::InProgress => "\u{25D4}", // ◔
            TodoStatus::Completed => "\u{25CF}",  // ●
        }
    }

    fn class(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "text-muted-foreground",
            TodoStatus::InProgress => "text-yellow",
            TodoStatus::Completed => "text-green",
        }
    }
}

/// A single todo item.
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

/// A todo list update line.
pub struct TodoLine {
    pub items: Vec<TodoItem>,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
}

impl TodoLine {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            step: None,
            elapsed: None,
        }
    }

    pub fn add_item(mut self, content: &str, status: TodoStatus) -> Self {
        self.items.push(TodoItem {
            content: content.to_string(),
            status,
        });
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

        let completed = self.items.iter().filter(|i| i.status == TodoStatus::Completed).count();
        let total = self.items.len();

        html! {
            div class=(LINE_CARD_CLASS) {
                div class={ "flex items-center gap-2 " (LINE_HEADER_CLASS) } {
                    (status_dot(StatusState::Success))
                    (line_type_label(LineType::Todo))
                    span class="text-xs text-muted-foreground" {
                        (completed) "/" (total) " completed"
                    }
                    span class="flex-1" {}
                    (meta.build())
                }
                div class=(LINE_CONTENT_CLASS) {
                    ul class="space-y-1" {
                        @for item in &self.items {
                            li class="flex items-center gap-2 text-sm" {
                                span class=(item.status.class()) { (item.status.icon()) }
                                span class={
                                    @if item.status == TodoStatus::Completed { "line-through text-muted-foreground" }
                                    @else { "text-foreground" }
                                } {
                                    (item.content)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for TodoLine {
    fn default() -> Self {
        Self::new()
    }
}
