//! SQL editor component - Execute SQL queries

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::DatabaseRow;

/// Render the SQL editor with query input and results
pub fn render_sql_editor(query: &str, results: &[DatabaseRow]) -> impl IntoElement {
    div()
        .id("sql-editor")
        .h(px(280.0))
        .w_full()
        .flex()
        .flex_col()
        .bg(bg::APP)
        .border_t_1()
        .border_color(border::DEFAULT)
        // Header
        .child(
            div()
                .h(px(32.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .bg(bg::SURFACE)
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
                                .child("SQL EDITOR"),
                        )
                        // Query tabs
                        .child(render_query_tab("Query 1", true))
                        .child(render_query_tab("Query 2", false))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("+"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        // Format button
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
                                        .child("FORMAT"),
                                ),
                        )
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
                        ),
                ),
        )
        // Query input area
        .child(
            div()
                .h(px(100.0))
                .w_full()
                .flex()
                .border_b_1()
                .border_color(border::DEFAULT)
                // Line numbers
                .child(
                    div()
                        .w(px(32.0))
                        .h_full()
                        .bg(bg::SURFACE)
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .py(px(8.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .w_full()
                                .flex()
                                .justify_end()
                                .pr(px(8.0))
                                .child("1"),
                        ),
                )
                // Query text
                .child(
                    div()
                        .flex_1()
                        .p(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child(render_sql_syntax(query)),
                        ),
                ),
        )
        // Results area
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                // Results header
                .child(
                    div()
                        .h(px(28.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .px(px(12.0))
                        .bg(bg::ELEVATED)
                        .border_b_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(if results.is_empty() {
                                    "RESULTS (run query to see results)".to_string()
                                } else {
                                    format!("RESULTS ({} rows)", results.len())
                                }),
                        ),
                )
                // Results content
                .child(
                    div()
                        .id("sql-results-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .when(results.is_empty(), |el| {
                            el.child(
                                div()
                                    .w_full()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("Press RUN or Ctrl+Enter to execute query"),
                                    ),
                            )
                        })
                        .when(!results.is_empty(), |el| {
                            el.child(render_results_table(results))
                        }),
                ),
        )
}

/// Render a query tab
fn render_query_tab(name: &str, is_active: bool) -> impl IntoElement {
    let name = name.to_string();
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .px(px(8.0))
        .py(px(2.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(name),
        )
}

/// Basic SQL syntax highlighting
fn render_sql_syntax(query: &str) -> impl IntoElement {
    let keywords = ["SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "IN", "LIKE", "ORDER", "BY", "LIMIT", "GROUP", "HAVING", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON", "AS", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP", "TABLE", "INDEX", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "NULL", "TRUE", "FALSE"];

    let parts: Vec<(String, bool)> = query.split_whitespace()
        .map(|word| {
            let upper = word.to_uppercase();
            let is_keyword = keywords.iter().any(|&kw| {
                upper.starts_with(kw) || upper.ends_with(kw) || upper == kw
            });
            (word.to_string(), is_keyword)
        })
        .collect();

    div()
        .flex()
        .flex_wrap()
        .gap_x(px(4.0))
        .children(parts.into_iter().map(|(word, is_keyword)| {
            let color = if is_keyword {
                Hsla { h: 0.55, s: 0.6, l: 0.7, a: 1.0 } // Cyan for keywords
            } else if word.starts_with('\'') || word.starts_with('"') {
                Hsla { h: 0.3, s: 0.6, l: 0.6, a: 1.0 } // Green for strings
            } else if word.chars().all(|c| c.is_ascii_digit() || c == '.') {
                Hsla { h: 0.08, s: 0.8, l: 0.7, a: 1.0 } // Orange for numbers
            } else {
                text::PRIMARY
            };

            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(word)
        }))
}

/// Render results table
fn render_results_table(results: &[DatabaseRow]) -> impl IntoElement {
    if results.is_empty() {
        return div();
    }

    // Get column names from first row as owned strings
    let columns: Vec<String> = results[0].keys().cloned().collect();
    let rows_data: Vec<Vec<String>> = results.iter().map(|row| {
        columns.iter().map(|col| {
            row.get(col).cloned().unwrap_or_else(|| "--".to_string())
        }).collect()
    }).collect();

    div()
        .min_w_full()
        // Headers
        .child(
            div()
                .flex()
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .children(columns.iter().map(|col| {
                    div()
                        .w(px(120.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text::MUTED)
                                .child(col.to_uppercase()),
                        )
                })),
        )
        // Rows
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
                        .w(px(120.0))
                        .h(px(24.0))
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
