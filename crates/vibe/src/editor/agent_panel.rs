//! Agent panel component - AI agent task feed and controls

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{AgentMode, AgentTask, AgentTaskStatus};

/// Render the agent panel
pub fn render_agent_panel(mode: AgentMode, tasks: &[AgentTask]) -> impl IntoElement {
    let running_task = tasks.iter().find(|t| t.status == AgentTaskStatus::Running);
    let pending_count = tasks.iter().filter(|t| t.status == AgentTaskStatus::Pending).count();
    let completed_count = tasks.iter().filter(|t| t.status == AgentTaskStatus::Completed).count();

    div()
        .id("agent-panel")
        .w(px(320.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg::SURFACE)
        .border_l_1()
        .border_color(border::DEFAULT)
        // Header
        .child(
            div()
                .h(px(36.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("AGENT"),
                        )
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(if running_task.is_some() {
                                    Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }
                                } else {
                                    text::MUTED
                                }),
                        ),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("{} pending", pending_count)),
                ),
        )
        // Mode selector
        .child(
            div()
                .h(px(40.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(12.0))
                .gap(px(4.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(render_mode_button(AgentMode::Agent, mode))
                .child(render_mode_button(AgentMode::Chat, mode))
                .child(render_mode_button(AgentMode::Off, mode)),
        )
        // Current task (if running)
        .when(running_task.is_some(), |el| {
            let task = running_task.unwrap();
            el.child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .bg(bg::ELEVATED)
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    // Status header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .mb(px(8.0))
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                    .child("WORKING"),
                            ),
                    )
                    // Task description
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .mb(px(8.0))
                            .child(task.description.clone()),
                    )
                    // Progress bar
                    .child(
                        div()
                            .w_full()
                            .h(px(4.0))
                            .bg(bg::APP)
                            .mb(px(8.0))
                            .child(
                                div()
                                    .w(px(180.0)) // Mock progress
                                    .h_full()
                                    .bg(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }),
                            ),
                    )
                    // Files changed
                    .child(
                        div()
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} files modified", task.files_changed.len())),
                    ),
            )
        })
        // Prompt input
        .child(
            div()
                .w_full()
                .p(px(12.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .w_full()
                        .min_h(px(60.0))
                        .p(px(10.0))
                        .bg(bg::APP)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PLACEHOLDER)
                                .child("Describe what you want to build..."),
                        ),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("[ENTER] to send"),
                        )
                        .child(
                            div()
                                .px(px(12.0))
                                .py(px(4.0))
                                .bg(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                .cursor_pointer()
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(bg::APP)
                                        .child("SEND"),
                                ),
                        ),
                ),
        )
        // Task list
        .child(
            div()
                .id("agent-task-list")
                .flex_1()
                .overflow_y_scroll()
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(8.0))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .mb(px(8.0))
                                .child(format!("TASK FEED ({} completed)", completed_count)),
                        )
                        .children(tasks.iter().map(|task| {
                            render_task_item(task)
                        })),
                ),
        )
        // Footer with stats
        .child(
            div()
                .h(px(32.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .bg(bg::ELEVATED)
                .border_t_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(render_stat_badge("TOKENS", "4.2K"))
                        .child(render_stat_badge("COST", "$0.08")),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::LINK)
                        .cursor_pointer()
                        .child("VIEW ATIF"),
                ),
        )
}

/// Render a mode button (Agent/Chat/Off)
fn render_mode_button(button_mode: AgentMode, current_mode: AgentMode) -> impl IntoElement {
    let is_active = button_mode == current_mode;
    let (bg_color, text_color, border_color) = if is_active {
        (bg::SELECTED, text::PRIMARY, border::SELECTED)
    } else {
        (Hsla::transparent_black(), text::MUTED, border::DEFAULT)
    };

    div()
        .id(SharedString::from(format!("mode-{}", button_mode.label())))
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .py(px(6.0))
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(button_mode.label()),
        )
}

/// Render a task item in the task feed
fn render_task_item(task: &AgentTask) -> impl IntoElement {
    let (status_color, status_bg) = match task.status {
        AgentTaskStatus::Pending => (text::MUTED, Hsla::transparent_black()),
        AgentTaskStatus::Running => (Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }, Hsla { h: 0.14, s: 0.3, l: 0.2, a: 1.0 }),
        AgentTaskStatus::Completed => (status::SUCCESS, Hsla::transparent_black()),
        AgentTaskStatus::Failed => (status::ERROR, Hsla { h: 0.0, s: 0.3, l: 0.2, a: 1.0 }),
    };

    div()
        .id(SharedString::from(format!("task-{}", task.id)))
        .w_full()
        .p(px(10.0))
        .mb(px(6.0))
        .bg(status_bg)
        .border_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        // Header row
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .mb(px(4.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(status_color)
                        .child(task.status.indicator()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(task.started_at.clone()),
                ),
        )
        // Description
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(task.description.clone()),
        )
        // Footer
        .when(task.status == AgentTaskStatus::Completed, |el| {
            el.child(
                div()
                    .mt(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} files", task.files_changed.len())),
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} tokens", task.tokens_used)),
                    ),
            )
        })
}

/// Render a small stat badge
fn render_stat_badge(label: &str, value: &str) -> impl IntoElement {
    let label = label.to_string();
    let value = value.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(value),
        )
}
