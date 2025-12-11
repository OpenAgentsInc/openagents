//! Schema view component - Visual schema editor

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, text, FONT_FAMILY};

use crate::types::{DatabaseTable, DatabaseColumn, ColumnType};

/// Render the schema view with visual table representations
pub fn render_schema_view(tables: &[DatabaseTable]) -> impl IntoElement {
    div()
        .id("schema-view")
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .bg(bg::APP)
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
                                .child("SCHEMA"),
                        )
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .bg(bg::ELEVATED)
                                .border_1()
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
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        // Add table button
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
                                        .child("+"),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("ADD TABLE"),
                                ),
                        )
                        // Generate migration button
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .bg(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                .cursor_pointer()
                                .hover(|s| s.bg(Hsla { h: 0.14, s: 1.0, l: 0.55, a: 1.0 }))
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(bg::APP)
                                        .child("GENERATE MIGRATION"),
                                ),
                        ),
                ),
        )
        // Schema canvas
        .child(
            div()
                .id("schema-canvas")
                .flex_1()
                .p(px(20.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(20.0))
                        .children(tables.iter().map(|table| {
                            render_schema_table(table, tables)
                        })),
                ),
        )
        // Footer with legend
        .child(
            div()
                .h(px(32.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .bg(bg::SURFACE)
                .border_t_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .child(render_legend_item("PK", "Primary Key", Hsla { h: 0.14, s: 0.8, l: 0.5, a: 1.0 }))
                        .child(render_legend_item("FK", "Foreign Key", Hsla { h: 0.55, s: 0.6, l: 0.6, a: 1.0 }))
                        .child(render_legend_item("NN", "Not Null", text::PRIMARY))
                        .child(render_legend_item("UQ", "Unique", Hsla { h: 0.83, s: 0.6, l: 0.6, a: 1.0 })),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("Drag to reposition | Double-click to edit"),
                ),
        )
}

/// Render a single table in the schema view
fn render_schema_table(table: &DatabaseTable, _all_tables: &[DatabaseTable]) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("schema-{}", table.name)))
        .w(px(240.0))
        .flex()
        .flex_col()
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        // Table header
        .child(
            div()
                .w_full()
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_between()
                .px(px(10.0))
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text::PRIMARY)
                        .child(table.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!("{} cols", table.columns.len())),
                        )
                        // Edit button
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(text::MUTED)
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("..."),
                        ),
                ),
        )
        // Columns
        .children(table.columns.iter().map(|col| {
            render_schema_column(col)
        }))
        // Add column footer
        .child(
            div()
                .w_full()
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(bg::APP)
                .border_t_1()
                .border_color(border::DEFAULT)
                .cursor_pointer()
                .hover(|s| s.bg(bg::HOVER))
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("+ Add Column"),
                ),
        )
}

/// Render a column row in the schema table
fn render_schema_column(col: &DatabaseColumn) -> impl IntoElement {
    let type_color = match col.column_type {
        ColumnType::Text => Hsla { h: 0.3, s: 0.5, l: 0.6, a: 1.0 },
        ColumnType::Integer | ColumnType::Real => Hsla { h: 0.08, s: 0.6, l: 0.6, a: 1.0 },
        ColumnType::Boolean => Hsla { h: 0.55, s: 0.5, l: 0.6, a: 1.0 },
        ColumnType::Timestamp => Hsla { h: 0.83, s: 0.5, l: 0.6, a: 1.0 },
        ColumnType::Uuid => Hsla { h: 0.6, s: 0.5, l: 0.6, a: 1.0 },
        ColumnType::Json => Hsla { h: 0.14, s: 0.5, l: 0.6, a: 1.0 },
        ColumnType::Blob => text::MUTED,
    };

    div()
        .w_full()
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_between()
        .px(px(10.0))
        .border_b_1()
        .border_color(border::DEFAULT)
        .hover(|s| s.bg(bg::HOVER))
        // Column name and indicators
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                // Primary key indicator
                .when(col.is_primary_key, |el| {
                    el.child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(Hsla { h: 0.14, s: 0.8, l: 0.5, a: 1.0 })
                            .child("PK"),
                    )
                })
                // Foreign key indicator
                .when(col.foreign_key.is_some(), |el| {
                    el.child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(Hsla { h: 0.55, s: 0.6, l: 0.6, a: 1.0 })
                            .child("FK"),
                    )
                })
                // Column name
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(col.name.clone()),
                ),
        )
        // Type and constraints
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                // Type
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(type_color)
                        .child(col.column_type.label()),
                )
                // Nullable indicator
                .when(!col.is_nullable, |el| {
                    el.child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("NN"),
                    )
                })
                // Unique indicator
                .when(col.is_unique && !col.is_primary_key, |el| {
                    el.child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(Hsla { h: 0.83, s: 0.6, l: 0.6, a: 1.0 })
                            .child("UQ"),
                    )
                }),
        )
}

/// Render a legend item
fn render_legend_item(abbrev: &str, label: &str, color: Hsla) -> impl IntoElement {
    let abbrev = abbrev.to_string();
    let label = label.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(abbrev),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label),
        )
}
