//! Database dashboard - Combines all database components

use gpui::*;
use gpui::prelude::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use crate::types::DatabaseTabState;
use crate::screen::VibeScreen;
use super::{render_table_browser, render_sql_editor, render_schema_view};

/// Render the complete database dashboard
pub fn render_database_dashboard(state: &DatabaseTabState, _cx: &mut Context<VibeScreen>) -> impl IntoElement {
    div()
        .id("database-dashboard")
        .flex()
        .flex_col()
        .h_full()
        .w_full()
        .bg(bg::APP)
        // Toolbar
        .child(render_database_toolbar(state))
        // Main content
        .child(
            div()
                .flex()
                .flex_1()
                .overflow_hidden()
                .when(!state.show_schema, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            // Table browser
                            .child(render_table_browser(&state.tables, &state.selected_table, &state.table_rows))
                            // SQL editor
                            .child(render_sql_editor(&state.sql_query, &state.query_results))
                    )
                })
                .when(state.show_schema, |el| {
                    el.child(render_schema_view(&state.tables))
                }),
        )
}

/// Render the database toolbar
fn render_database_toolbar(state: &DatabaseTabState) -> impl IntoElement {
    div()
        .id("database-toolbar")
        .h(px(44.0))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Left: View toggles
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(render_view_toggle("DATA", !state.show_schema))
                .child(render_view_toggle("SCHEMA", state.show_schema))
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(16.0))
                        .bg(border::DEFAULT)
                        .mx(px(8.0)),
                )
                .child(render_view_toggle("SQL", true)),
        )
        // Center: Connection status
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(status::SUCCESS),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("CONNECTED"),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child("data.sqlite"),
                ),
        )
        // Right: Actions
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                // Refresh
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("REFRESH"),
                        ),
                )
                // Import
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("IMPORT"),
                        ),
                )
                // Export
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("EXPORT"),
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

/// Render a view toggle button
fn render_view_toggle(label: &str, is_active: bool) -> impl IntoElement {
    let label_owned = label.to_string();
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .id(SharedString::from(format!("db-toggle-{}", label.to_lowercase())))
        .px(px(10.0))
        .py(px(4.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(label_owned),
        )
}
