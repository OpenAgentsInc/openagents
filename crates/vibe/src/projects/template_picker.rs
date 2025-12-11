//! Template picker component - Browse and select starter templates

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, text, FONT_FAMILY};

use crate::types::{ProjectsTabState, ProjectTemplate, TemplateCategory};
use crate::screen::VibeScreen;

/// Render the template picker view
pub fn render_template_picker(state: &ProjectsTabState, cx: &mut Context<VibeScreen>) -> impl IntoElement {
    let categories = TemplateCategory::all();
    let selected_category = state.selected_category;

    // Filter templates by category
    let filtered_templates: Vec<&ProjectTemplate> = state.templates.iter()
        .filter(|t| selected_category == TemplateCategory::All || t.category == selected_category)
        .collect();

    div()
        .id("template-picker-container")
        .flex()
        .flex_col()
        .h_full()
        .w_full()
        .bg(bg::APP)
        // Header
        .child(
            div()
                .id("template-picker-header")
                .h(px(56.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(20.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                // Left: back button and title
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        // Back button
                        .child(
                            div()
                                .id("back-to-projects-btn")
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .px(px(12.0))
                                .py(px(6.0))
                                .bg(bg::ELEVATED)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .cursor_pointer()
                                .hover(|s| s.bg(bg::HOVER))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.toggle_templates(cx);
                                }))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(text::MUTED)
                                        .child("<"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("BACK"),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_family(FONT_FAMILY)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text::PRIMARY)
                                .child("START A NEW PROJECT"),
                        ),
                )
                // Right: search
                .child(
                    div()
                        .w(px(280.0))
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
                                .child("Search templates..."),
                        ),
                ),
        )
        // Category tabs
        .child(
            div()
                .id("template-categories")
                .h(px(44.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(20.0))
                .gap(px(4.0))
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .children(categories.iter().map(|&cat| {
                    render_category_tab(cat, cat == selected_category)
                })),
        )
        // Featured section
        .child(
            div()
                .id("featured-section")
                .px(px(20.0))
                .py(px(16.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .mb(px(12.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("FEATURED"),
                        )
                        .child(
                            div()
                                .w(px(24.0))
                                .h(px(1.0))
                                .bg(border::DEFAULT),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(12.0))
                        .children(state.templates.iter().take(3).map(|template| {
                            render_featured_template(template)
                        })),
                ),
        )
        // Template grid
        .child(
            div()
                .id("templates-scroll")
                .flex_1()
                .p(px(20.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .mb(px(16.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!("ALL TEMPLATES ({})", filtered_templates.len())),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .children(filtered_templates.iter().map(|template| {
                            div()
                                .w(px(320.0))
                                .child(render_template_card(template))
                        })),
                ),
        )
        // Footer with blank project option
        .child(
            div()
                .id("template-footer")
                .h(px(56.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(20.0))
                .bg(bg::SURFACE)
                .border_t_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("Or start with a blank project"),
                        )
                        .child(
                            div()
                                .id("blank-project-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .bg(bg::ELEVATED)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .cursor_pointer()
                                .hover(|s| s.bg(bg::HOVER))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::PRIMARY)
                                        .child("BLANK PROJECT"),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("[ENTER] Select  |  [ESC] Cancel"),
                ),
        )
}

/// Render a category tab
fn render_category_tab(category: TemplateCategory, is_selected: bool) -> impl IntoElement {
    let (bg_color, text_color, border_color) = if is_selected {
        (bg::SELECTED, text::BRIGHT, border::SELECTED)
    } else {
        (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
    };

    div()
        .id(SharedString::from(format!("cat-tab-{}", category.label())))
        .px(px(12.0))
        .py(px(6.0))
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY))
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(category.label()),
        )
}

/// Render a featured template card (larger, more prominent)
fn render_featured_template(template: &ProjectTemplate) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("featured-{}", template.id)))
        .flex_1()
        .flex()
        .flex_col()
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(Hsla { h: 0.14, s: 0.8, l: 0.4, a: 1.0 })
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        // Header
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text::PRIMARY)
                        .child(template.name.clone()),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .bg(Hsla { h: 0.14, s: 0.8, l: 0.3, a: 1.0 })
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                .child("FEATURED"),
                        ),
                ),
        )
        // Description
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(12.0))
                .child(template.description.clone()),
        )
        // Features
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(4.0))
                .children(template.features.iter().map(|feature| {
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .bg(bg::SURFACE)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(feature.clone()),
                        )
                })),
        )
        // Use count
        .child(
            div()
                .mt(px(12.0))
                .pt(px(8.0))
                .border_t_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("{} projects created", format_number(template.use_count))),
                ),
        )
}

/// Render a standard template card
fn render_template_card(template: &ProjectTemplate) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("template-{}", template.id)))
        .flex()
        .flex_col()
        .p(px(14.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        // Header
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(6.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text::PRIMARY)
                        .child(template.name.clone()),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .bg(bg::SURFACE)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(template.framework.label()),
                        ),
                ),
        )
        // Description
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(10.0))
                .child(template.description.clone()),
        )
        // Features row
        .child(
            div()
                .flex()
                .gap(px(4.0))
                .children(template.features.iter().take(3).map(|feature| {
                    div()
                        .px(px(4.0))
                        .py(px(1.0))
                        .bg(bg::SURFACE)
                        .child(
                            div()
                                .text_size(px(8.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(feature.clone()),
                        )
                }))
                .when(template.features.len() > 3, |el| {
                    el.child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("+{}", template.features.len() - 3)),
                    )
                }),
        )
        // Use count
        .child(
            div()
                .mt(px(8.0))
                .pt(px(6.0))
                .border_t_1()
                .border_color(border::DEFAULT)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("{} uses", format_number(template.use_count))),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::LINK)
                        .child("USE"),
                ),
        )
}

/// Format a number with K/M suffixes
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
