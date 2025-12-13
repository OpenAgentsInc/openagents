//! Pi Panel - Pi agent configuration and status panel
//!
//! Displays Pi agent information:
//! - Cost tracking (current session)
//! - Model info
//! - Session info (ID, working directory)
//! - Available tools

use gpui::{
    div, prelude::*, px, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::panels::CostTracker;

/// Events emitted by PiPanel
#[derive(Clone, Debug)]
pub enum PiPanelEvent {
    /// Model was changed
    ModelChanged { model: String },
    /// Session reset requested
    SessionReset,
}

/// Pi Panel component
pub struct PiPanel {
    /// Focus handle
    focus_handle: FocusHandle,
    /// Cost tracking data
    cost_tracker: CostTracker,
    /// Current model
    model: Option<String>,
    /// Current session ID
    session_id: Option<String>,
    /// Working directory
    working_dir: Option<String>,
    /// Available tools
    tools: Vec<String>,
    /// Section expansion state
    cost_expanded: bool,
    model_expanded: bool,
    session_expanded: bool,
    tools_expanded: bool,
}

impl PiPanel {
    /// Create a new Pi panel
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            cost_tracker: CostTracker::default(),
            model: None,
            session_id: None,
            working_dir: None,
            tools: vec![
                "bash".to_string(),
                "read".to_string(),
                "write".to_string(),
                "edit".to_string(),
            ],
            // Expand all sections by default
            cost_expanded: true,
            model_expanded: true,
            session_expanded: true,
            tools_expanded: true,
        }
    }

    /// Update the session ID
    pub fn set_session_id(&mut self, session_id: Option<String>, cx: &mut Context<Self>) {
        self.session_id = session_id;
        cx.notify();
    }

    /// Update the model
    pub fn set_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.model = model;
        cx.notify();
    }

    /// Update the working directory
    pub fn set_working_dir(&mut self, working_dir: Option<String>, cx: &mut Context<Self>) {
        self.working_dir = working_dir;
        cx.notify();
    }

    /// Update cost tracking
    pub fn update_cost(
        &mut self,
        total: f64,
        input_tokens: u64,
        output_tokens: u64,
        cx: &mut Context<Self>,
    ) {
        self.cost_tracker.total_cost_usd = total;
        self.cost_tracker.total_input_tokens = input_tokens;
        self.cost_tracker.total_output_tokens = output_tokens;
        cx.notify();
    }

    /// Toggle cost section
    fn toggle_cost(&mut self, cx: &mut Context<Self>) {
        self.cost_expanded = !self.cost_expanded;
        cx.notify();
    }

    /// Toggle model section
    fn toggle_model(&mut self, cx: &mut Context<Self>) {
        self.model_expanded = !self.model_expanded;
        cx.notify();
    }

    /// Toggle session section
    fn toggle_session(&mut self, cx: &mut Context<Self>) {
        self.session_expanded = !self.session_expanded;
        cx.notify();
    }

    /// Toggle tools section
    fn toggle_tools(&mut self, cx: &mut Context<Self>) {
        self.tools_expanded = !self.tools_expanded;
        cx.notify();
    }

    /// Render the header
    fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let close_hint = if cfg!(target_os = "macos") {
            "[Cmd+P to close]"
        } else {
            "[Ctrl+P to close]"
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .child(
                div()
                    .font_family(FONT_FAMILY)
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text::PRIMARY)
                    .child("PI AGENT"),
            )
            .child(div().text_xs().text_color(text::MUTED).child(close_hint))
    }

    /// Render the cost section
    fn render_cost_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.cost_expanded;

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_cost(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("COST"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" }),
                    ),
            )
            .when(is_open, |el| {
                let total = self.cost_tracker.total_cost_usd;
                let input_tokens = self.cost_tracker.total_input_tokens;
                let output_tokens = self.cost_tracker.total_output_tokens;
                let has_usage = total > 0.0 || input_tokens > 0 || output_tokens > 0;

                el.mt(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(if has_usage {
                                status::SUCCESS
                            } else {
                                text::MUTED
                            })
                            .child(format!("${:.4}", total)),
                    )
                    .when(input_tokens > 0 || output_tokens > 0, |el| {
                        let total_tokens = input_tokens + output_tokens;
                        el.mt(px(8.0)).child(
                            div()
                                .text_xs()
                                .text_color(text::MUTED)
                                .child(format!(
                                    "{:.1}K tokens ({:.1}K in / {:.1}K out)",
                                    total_tokens as f64 / 1000.0,
                                    input_tokens as f64 / 1000.0,
                                    output_tokens as f64 / 1000.0
                                )),
                        )
                    })
            })
    }

    /// Render the model section
    fn render_model_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.model_expanded;
        let model_display = self
            .model
            .as_ref()
            .map(|m| m.clone())
            .unwrap_or_else(|| "claude-sonnet-4".to_string());

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_model(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("MODEL"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" }),
                    ),
            )
            .when(is_open, |el| {
                el.mt(px(8.0)).child(
                    div()
                        .px(px(8.0))
                        .py(px(6.0))
                        .bg(bg::CARD)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .text_xs()
                        .text_color(text::PRIMARY)
                        .font_family(FONT_FAMILY)
                        .child(model_display),
                )
            })
    }

    /// Render the session section
    fn render_session_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.session_expanded;
        let has_session = self.session_id.is_some();

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_session(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("SESSION"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" }),
                    ),
            )
            .when(is_open, |el| {
                if has_session {
                    el.mt(px(8.0)).child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(text::MUTED)
                                            .child("Session ID"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(text::PRIMARY)
                                            .font_family(FONT_FAMILY)
                                            .child(self.session_id.as_ref().unwrap().clone()),
                                    ),
                            )
                            .when_some(self.working_dir.as_ref(), |el, dir| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(text::MUTED)
                                                .child("Working Dir"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(text::PRIMARY)
                                                .font_family(FONT_FAMILY)
                                                .overflow_hidden()
                                                .child(dir.clone()),
                                        ),
                                )
                            })
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .bg(bg::CARD)
                                    .border_1()
                                    .border_color(border::DEFAULT)
                                    .text_xs()
                                    .text_color(text::PRIMARY)
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|_this, _, _, cx| {
                                            cx.emit(PiPanelEvent::SessionReset);
                                        }),
                                    )
                                    .child("[Reset Session]"),
                            ),
                    )
                } else {
                    el.mt(px(8.0)).child(
                        div()
                            .px(px(8.0))
                            .py(px(6.0))
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .text_sm()
                            .text_color(text::SECONDARY)
                            .child("No active session"),
                    )
                }
            })
    }

    /// Render the tools section
    fn render_tools_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.tools_expanded;

        div()
            .px(px(12.0))
            .py(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_tools(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("TOOLS"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text::MUTED)
                            .child(if is_open { "[-]" } else { "[+]" }),
                    ),
            )
            .when(is_open, |el| {
                el.mt(px(8.0)).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .children(self.tools.iter().map(|tool| {
                            div()
                                .text_xs()
                                .text_color(text::PRIMARY)
                                .child(format!("* {}", tool))
                        })),
                )
            })
    }
}

impl EventEmitter<PiPanelEvent> for PiPanel {}

impl Focusable for PiPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PiPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .bg(bg::APP)
            .font_family(FONT_FAMILY)
            .track_focus(&self.focus_handle)
            .child(self.render_header(cx))
            .child(self.render_cost_section(cx))
            .child(self.render_model_section(cx))
            .child(self.render_session_section(cx))
            .child(self.render_tools_section(cx))
    }
}
