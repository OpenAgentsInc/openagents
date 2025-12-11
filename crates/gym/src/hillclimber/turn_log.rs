//! Turn-by-turn action log

use gpui_oa::prelude::*;
use gpui_oa::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

/// Turn action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnAction {
    FMGenerate,
    Verify,
    Decompose,
    TestGen,
}

impl TurnAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FMGenerate => "FM Generate",
            Self::Verify => "Verify",
            Self::Decompose => "Decompose",
            Self::TestGen => "TestGen",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::FMGenerate => "FM",
            Self::Verify => "V",
            Self::Decompose => "D",
            Self::TestGen => "TG",
        }
    }
}

/// A turn log entry
#[derive(Debug, Clone)]
pub struct TurnEntry {
    pub turn: u32,
    pub action: TurnAction,
    pub description: String,
    pub duration_ms: u32,
    pub success: bool,
}

/// Turn log component
pub struct TurnLog {
    entries: Vec<TurnEntry>,
    focus_handle: FocusHandle,
}

impl TurnLog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            entries: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_entries(&mut self, entries: Vec<TurnEntry>) {
        self.entries = entries;
    }

    pub fn add_entry(&mut self, entry: TurnEntry) {
        self.entries.push(entry);
    }

    fn render_entry(&self, entry: &TurnEntry, is_last: bool) -> impl IntoElement {
        let (icon_bg, icon_color) = match entry.action {
            TurnAction::FMGenerate => (status::INFO_BG, status::INFO),
            TurnAction::Verify => (status::WARNING_BG, status::WARNING),
            TurnAction::Decompose => (bg::ELEVATED, text::MUTED),
            TurnAction::TestGen => (status::SUCCESS_BG, status::SUCCESS),
        };

        let status_icon = if entry.success { "+" } else { "-" };
        let status_color = if entry.success { status::SUCCESS } else { status::ERROR };

        let description = entry.description.clone();
        let duration = if entry.duration_ms > 0 {
            format!("{}ms", entry.duration_ms)
        } else {
            "...".to_string()
        };

        div()
            .flex()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(bg::ROW)
            .when(!is_last, |el| {
                el.border_b_1().border_color(border::SUBTLE)
            })
            // Turn number
            .child(
                div()
                    .w(px(28.0))
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(format!("#{}", entry.turn))
            )
            // Action icon
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .bg(icon_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(icon_color)
                            .font_weight(FontWeight::BOLD)
                            .child(entry.action.icon())
                    )
            )
            // Description
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(entry.action.label())
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(status_color)
                                    .child(status_icon)
                            )
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(description)
                    )
            )
            // Duration
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(duration)
            )
    }
}

impl Focusable for TurnLog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TurnLog {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let entry_count = self.entries.len();

        div()
            .id("turn-log-scroll")
            .h_full()
            .w_full()
            .overflow_y_scroll()
            .bg(bg::SURFACE)
            .when(self.entries.is_empty(), |el| {
                el.flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("No turns yet")
                    )
            })
            .when(!self.entries.is_empty(), |el| {
                el.children(
                    self.entries.iter().enumerate().map(|(idx, entry)| {
                        self.render_entry(entry, idx == entry_count - 1)
                    })
                )
            })
    }
}
