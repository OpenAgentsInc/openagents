//! HillClimber controls (Start/Stop, mode selector, session dropdown)

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::monitor::{HCMode, HCSessionStatus};

/// Control panel for HillClimber sessions
pub struct HCControls {
    /// Currently selected mode
    selected_mode: HCMode,
    /// Current session status
    session_status: HCSessionStatus,
    focus_handle: FocusHandle,
}

impl HCControls {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            selected_mode: HCMode::Standard,
            session_status: HCSessionStatus::Idle,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_mode(&mut self, mode: HCMode) {
        self.selected_mode = mode;
    }

    pub fn set_status(&mut self, status: HCSessionStatus) {
        self.session_status = status;
    }

    fn render_mode_button(&self, mode: HCMode) -> impl IntoElement {
        let is_selected = self.selected_mode == mode;
        let label = mode.label().to_string();

        div()
            .px(px(12.0))
            .py(px(6.0))
            .bg(if is_selected { bg::CARD } else { bg::ELEVATED })
            .border_1()
            .border_color(if is_selected { border::SELECTED } else { border::DEFAULT })
            .rounded(px(4.0))
            .text_size(px(11.0))
            .font_family(FONT_FAMILY)
            .text_color(if is_selected { text::PRIMARY } else { text::MUTED })
            .cursor_pointer()
            .hover(|el| {
                if is_selected {
                    el
                } else {
                    el.bg(bg::HOVER).border_color(border::STRONG)
                }
            })
            .child(label)
    }

    fn render_start_button(&self) -> impl IntoElement {
        let is_running = self.session_status == HCSessionStatus::Running;
        let (label, bg_color, hover_color) = if is_running {
            ("Stop", status::ERROR, status::ERROR.opacity(0.8))
        } else {
            ("Start", status::SUCCESS, status::SUCCESS.opacity(0.8))
        };

        div()
            .px(px(20.0))
            .py(px(8.0))
            .bg(bg_color)
            .rounded(px(6.0))
            .text_size(px(12.0))
            .font_family(FONT_FAMILY)
            .text_color(text::BRIGHT)
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .hover(|el| el.bg(hover_color))
            .child(label)
    }

    fn render_pause_button(&self) -> impl IntoElement {
        let is_running = self.session_status == HCSessionStatus::Running;
        let is_paused = self.session_status == HCSessionStatus::Paused;
        let label = if is_paused { "Resume" } else { "Pause" };

        div()
            .px(px(16.0))
            .py(px(8.0))
            .bg(bg::HOVER)
            .border_1()
            .border_color(border::DEFAULT)
            .rounded(px(6.0))
            .text_size(px(12.0))
            .font_family(FONT_FAMILY)
            .text_color(text::PRIMARY)
            .cursor_pointer()
            .when(is_running || is_paused, |el| {
                el.hover(|el| el.bg(bg::CARD).border_color(border::SELECTED))
            })
            .when(!is_running && !is_paused, |el| {
                el.opacity(0.5)
            })
            .child(label)
    }
}

impl Focusable for HCControls {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HCControls {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.0))
            .py(px(12.0))
            .bg(bg::SURFACE)
            .border_b_1()
            .border_color(border::DEFAULT)
            // Mode selector
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("Mode:")
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(self.render_mode_button(HCMode::Quick))
                            .child(self.render_mode_button(HCMode::Standard))
                            .child(self.render_mode_button(HCMode::Full))
                    )
            )
            // Action buttons
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(self.render_pause_button())
                    .child(self.render_start_button())
            )
    }
}
