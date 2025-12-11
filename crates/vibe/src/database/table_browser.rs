//! Table browser component - Browse tables and records

use gpui::*;
use gpui::prelude::*;
use theme::{bg, border, text, FONT_FAMILY};

use crate::types::{DatabaseTable, DatabaseRow};

/// Render the table browser (table list + data view)
pub fn render_table_browser(
    tables: &[DatabaseTable],
    selected_table: &Option<String>,
    rows: &[DatabaseRow],
) -> impl IntoElement {
    let active_table = selected_table.as_ref().and_then(|name| {
        tables.iter().find(|t| &t.name == name)
    });

    div()
        .id("table-browser")
        .flex()
        .flex_1()
        .h_full()
        // Table list sidebar
        .child(
            div()
                .id("table-list")
                .w(px(200.0))
                .h_full()
                .flex()
                .flex_col()
                .bg(bg::SURFACE)
                .border_r_1()
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
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("TABLES"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("+"),
                        ),
                )
                // Table list
                .child(
                    div()
                        .id("table-list-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .py(px(4.0))
                        .children(tables.iter().map(|table| {
                            let is_selected = selected_table.as_ref().map_or(false, |s| s == &table.name);
                            render_table_item(table, is_selected)
                        })),
                )
                // Stats footer
                .child(
                    div()
                        .h(px(28.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .px(px(12.0))
                        .bg(bg::ELEVATED)
                        .border_t_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!("{} tables", tables.len())),
                        ),
                ),
        )
        // Data view
        .child(
            div()
                .id("data-view")
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .bg(bg::APP)
                // Table header (if table selected)
                .when(active_table.is_some(), |el| {
                    let table = active_table.unwrap();
                    el.child(
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
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text::PRIMARY)
                                            .child(table.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child(format!("{} rows", table.row_count)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    // Add row
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
                                                    .child("+ ROW"),
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
                                    ),
                            ),
                    )
                })
                // Data grid
                .child(
                    div()
                        .id("data-view-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .when(active_table.is_some(), |el| {
                            let table = active_table.unwrap();
                            el.child(render_data_grid(table, rows))
                        })
                        .when(active_table.is_none(), |el| {
                            el.child(
                                div()
                                    .w_full()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("Select a table to view data"),
                                    ),
                            )
                        }),
                ),
        )
}

/// Render a table item in the list
fn render_table_item(table: &DatabaseTable, is_selected: bool) -> impl IntoElement {
    let bg_color = if is_selected { bg::SELECTED } else { Hsla::transparent_black() };
    let text_color = if is_selected { text::PRIMARY } else { text::MUTED };

    div()
        .id(SharedString::from(format!("table-{}", table.name)))
        .w_full()
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(text::MUTED)
                        .child("T"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text_color)
                        .child(table.name.clone()),
                ),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format!("{}", table.row_count)),
        )
}

/// Render the data grid with column headers and rows
fn render_data_grid(table: &DatabaseTable, rows: &[DatabaseRow]) -> impl IntoElement {
    let columns: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
    let rows_data: Vec<Vec<String>> = rows.iter().map(|row| {
        columns.iter().map(|col| {
            row.get(col).cloned().unwrap_or_else(|| "--".to_string())
        }).collect()
    }).collect();

    div()
        .id("data-grid")
        .min_w_full()
        // Column headers
        .child(
            div()
                .flex()
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .children(columns.iter().map(|col| {
                    div()
                        .w(px(140.0))
                        .h(px(32.0))
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text::MUTED)
                                .child(col.to_uppercase()),
                        )
                })),
        )
        // Data rows
        .children(rows_data.into_iter().enumerate().map(|(i, row_values)| {
            let bg_color = if i % 2 == 0 { Hsla::transparent_black() } else { bg::ELEVATED };

            div()
                .flex()
                .bg(bg_color)
                .border_b_1()
                .border_color(border::DEFAULT)
                .hover(|s| s.bg(bg::HOVER))
                .children(row_values.into_iter().map(|value| {
                    div()
                        .w(px(140.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .overflow_hidden()
                                .child(value),
                        )
                }))
        }))
}
