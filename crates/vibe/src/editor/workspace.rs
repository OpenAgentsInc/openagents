//! Editor workspace - Combines all editor panels into the main IDE view

use gpui::*;
use gpui::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::EditorTabState;
use crate::screen::VibeScreen;
use super::{
    render_file_tree,
    render_code_editor,
    render_preview_panel,
    render_terminal_panel,
    render_agent_panel,
};

/// Render the complete editor workspace with all panels
pub fn render_editor_workspace(state: &EditorTabState, _cx: &mut Context<VibeScreen>) -> impl IntoElement {
    div()
        .id("editor-workspace")
        .flex()
        .flex_col()
        .h_full()
        .w_full()
        .bg(bg::APP)
        // Toolbar
        .child(render_editor_toolbar(state))
        // Main content area
        .child(
            div()
                .id("editor-main")
                .flex()
                .flex_1()
                .overflow_hidden()
                // Left: File tree
                .child(render_file_tree(&state.file_tree, &state.active_file_path))
                // Center: Code editor + terminal
                .child(
                    div()
                        .id("editor-center")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        // Code editor
                        .child(render_code_editor(&state.open_tabs, &state.file_content))
                        // Terminal (conditionally shown)
                        .when(state.show_terminal, |el| {
                            el.child(render_terminal_panel(&state.terminal_lines, &state.terminal_input))
                        }),
                )
                // Right: Preview or Agent panel
                .when(state.show_preview, |el| {
                    el.child(render_preview_panel())
                })
                .when(state.show_agent_panel && !state.show_preview, |el| {
                    el.child(render_agent_panel(state.agent_mode, &state.agent_tasks))
                }),
        )
}

/// Render the editor toolbar with toggles and actions
fn render_editor_toolbar(state: &EditorTabState) -> impl IntoElement {
    div()
        .id("editor-toolbar")
        .h(px(36.0))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Left: View toggles
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(render_toolbar_toggle("EXPLORER", true))
                .child(render_toolbar_toggle("SEARCH", false))
                .child(render_toolbar_toggle("GIT", false))
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(16.0))
                        .bg(border::DEFAULT)
                        .mx(px(8.0)),
                )
                .child(render_toolbar_toggle("TERMINAL", state.show_terminal))
                .child(render_toolbar_toggle("PREVIEW", state.show_preview))
                .child(render_toolbar_toggle("AGENT", state.show_agent_panel)),
        )
        // Center: Agent status
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(if state.agent_mode == crate::types::AgentMode::Agent {
                            Hsla { h: 0.14, s: 0.3, l: 0.2, a: 1.0 }
                        } else {
                            Hsla::transparent_black()
                        })
                        .border_1()
                        .border_color(if state.agent_mode == crate::types::AgentMode::Agent {
                            Hsla { h: 0.14, s: 0.5, l: 0.4, a: 1.0 }
                        } else {
                            border::DEFAULT
                        })
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(match state.agent_mode {
                                    crate::types::AgentMode::Agent => Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 },
                                    crate::types::AgentMode::Chat => status::SUCCESS,
                                    crate::types::AgentMode::Off => text::MUTED,
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(state.agent_mode.label()),
                        ),
                ),
        )
        // Right: Actions
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                // Run button
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(10.0))
                        .py(px(4.0))
                        .bg(status::SUCCESS)
                        .cursor_pointer()
                        .hover(|s| s.bg(Hsla { h: 0.35, s: 0.7, l: 0.45, a: 1.0 }))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(bg::APP)
                                .child(">"),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(bg::APP)
                                .child("RUN"),
                        ),
                )
                // Deploy button
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(10.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text::MUTED)
                                .child("^"),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("DEPLOY"),
                        ),
                )
                // Settings
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text::MUTED)
                                .child("..."),
                        ),
                ),
        )
}

/// Render a toolbar toggle button
fn render_toolbar_toggle(label: &str, is_active: bool) -> impl IntoElement {
    let label_str = label.to_string();
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .id(SharedString::from(format!("toggle-{}", label.to_lowercase())))
        .px(px(8.0))
        .py(px(4.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(label_str),
        )
}
