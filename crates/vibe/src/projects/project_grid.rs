//! Project grid component - Grid of project cards with search and filters

use gpui::*;
use theme_oa::{bg, border, text, FONT_FAMILY};

use crate::types::ProjectsTabState;
use crate::screen::VibeScreen;
use super::project_card::render_project_card;

/// Render the project grid with search and actions
pub fn render_project_grid(state: &ProjectsTabState, cx: &mut Context<VibeScreen>) -> impl IntoElement {
    let project_count = state.projects.len();

    div()
        .id("project-grid-container")
        .flex()
        .flex_col()
        .h_full()
        .w_full()
        .bg(bg::APP)
        // Header bar
        .child(
            div()
                .id("projects-header")
                .h(px(56.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(20.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                // Left: title and count
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_family(FONT_FAMILY)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text::PRIMARY)
                                .child("YOUR PROJECTS"),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.0))
                                .bg(bg::ELEVATED)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child(format!("{} projects", project_count)),
                                ),
                        ),
                )
                // Right: search and actions
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        // Search box
                        .child(
                            div()
                                .w(px(240.0))
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .px(px(12.0))
                                .py(px(8.0))
                                .bg(bg::ELEVATED)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(text::MUTED)
                                        .child("SEARCH"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::PLACEHOLDER)
                                        .child("Search projects..."),
                                ),
                        )
                        // New project button
                        .child(
                            div()
                                .id("new-project-btn")
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .px(px(16.0))
                                .py(px(8.0))
                                .bg(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                .cursor_pointer()
                                .hover(|s| s.bg(Hsla { h: 0.14, s: 1.0, l: 0.55, a: 1.0 }))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.toggle_templates(cx);
                                }))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .child("+"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(FONT_FAMILY)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(bg::APP)
                                        .child("NEW PROJECT"),
                                ),
                        ),
                ),
        )
        // Grid content
        .child(
            div()
                .id("projects-grid-scroll")
                .flex_1()
                .p(px(20.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .children(state.projects.iter().map(|project| {
                            div()
                                .w(px(360.0))
                                .child(render_project_card(project))
                        })),
                ),
        )
        // Quick stats footer
        .child(
            div()
                .id("projects-footer")
                .h(px(40.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(20.0))
                .bg(bg::SURFACE)
                .border_t_1()
                .border_color(border::DEFAULT)
                // Stats
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(24.0))
                        .child(render_footer_stat("DEPLOYED", "3"))
                        .child(render_footer_stat("BUILDING", "1"))
                        .child(render_footer_stat("ERRORS", "1"))
                        .child(render_footer_stat("TOTAL FILES", "497")),
                )
                // Help text
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("[N] New Project  |  [ENTER] Open  |  [D] Deploy"),
                ),
        )
}

/// Render a footer stat
fn render_footer_stat(label: &str, value: &str) -> impl IntoElement {
    let label = label.to_string();
    let value = value.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(value),
        )
}
