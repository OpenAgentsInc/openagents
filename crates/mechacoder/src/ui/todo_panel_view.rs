//! Todo panel view for plan mode progress display.

use crate::sdk_thread::{TodoItem, TodoState, TodoStatus};
use gpui::{div, px, IntoElement, ParentElement, Styled};
use theme_oa::{bg, border, status, text};

/// Render the todo panel showing plan mode progress.
pub fn render_todo_panel(state: &TodoState) -> impl IntoElement {
    if state.items.is_empty() {
        return div().into_any_element();
    }

    let completed = state.items.iter().filter(|t| t.status == TodoStatus::Completed).count();
    let total = state.items.len();
    let progress = if total > 0 { completed as f32 / total as f32 } else { 0.0 };

    div()
        .w_full()
        .bg(bg::CARD)
        .border_b_1()
        .border_color(border::DEFAULT)
        .px(px(16.0))
        .py(px(12.0))
        // Header row
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(text::PRIMARY)
                        .child("Plan"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(text::SECONDARY)
                        .child(format!("{}/{}", completed, total)),
                ),
        )
        // Progress bar
        .child(
            div()
                .w_full()
                .h(px(4.0))
                .mt(px(8.0))
                .bg(bg::SURFACE)
                .rounded(px(2.0))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(progress))
                        .bg(status::SUCCESS)
                        .rounded(px(2.0)),
                ),
        )
        // Todo items
        .child(
            div()
                .mt(px(8.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .children(state.items.iter().map(render_todo_item)),
        )
        .into_any_element()
}

/// Render a single todo item.
fn render_todo_item(item: &TodoItem) -> impl IntoElement {
    let (status_color, status_icon) = match item.status {
        TodoStatus::Pending => (status::PENDING, "○"),
        TodoStatus::InProgress => (status::RUNNING, "◐"),
        TodoStatus::Completed => (status::SUCCESS, "●"),
    };

    // Use activeForm when in progress, otherwise use content
    let display_text = match item.status {
        TodoStatus::InProgress => &item.active_form,
        _ => &item.content,
    };

    let text_color = match item.status {
        TodoStatus::Completed => text::SECONDARY,
        _ => text::PRIMARY,
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .py(px(2.0))
        // Status icon
        .child(
            div()
                .w(px(16.0))
                .text_color(status_color)
                .child(status_icon),
        )
        // Task text
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(text_color)
                .child(display_text.clone()),
        )
}
