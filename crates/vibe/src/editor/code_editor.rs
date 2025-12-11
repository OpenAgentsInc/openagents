//! Code editor component - Syntax-highlighted code editing area

use gpui::*;
use gpui::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{EditorTab, FileType};

/// Render the code editor with tabs and content
pub fn render_code_editor(tabs: &[EditorTab], content: &str) -> impl IntoElement {
    let active_tab = tabs.iter().find(|t| t.is_active);
    let line_count = content.lines().count().max(1);

    div()
        .id("code-editor")
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .bg(bg::APP)
        // Tab bar
        .child(
            div()
                .id("editor-tabs")
                .h(px(32.0))
                .w_full()
                .flex()
                .items_center()
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .overflow_x_scroll()
                .children(tabs.iter().map(|tab| {
                    render_editor_tab(tab)
                })),
        )
        // Breadcrumb / path bar
        .child(
            div()
                .id("editor-breadcrumb")
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
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(active_tab.map_or("No file open".to_string(), |t| t.path.clone())),
                ),
        )
        // Editor content with line numbers
        .child(
            div()
                .id("editor-content")
                .flex()
                .flex_1()
                .overflow_y_scroll()
                .overflow_x_scroll()
                // Line numbers gutter
                .child(
                    div()
                        .id("line-numbers")
                        .w(px(48.0))
                        .flex()
                        .flex_col()
                        .py(px(8.0))
                        .pr(px(8.0))
                        .bg(bg::SURFACE)
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .children((1..=line_count).map(|n| {
                            div()
                                .h(px(20.0))
                                .flex()
                                .items_center()
                                .justify_end()
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child(n.to_string()),
                                )
                        })),
                )
                // Code content
                .child(
                    div()
                        .id("code-content")
                        .flex_1()
                        .py(px(8.0))
                        .pl(px(12.0))
                        .children(content.lines().enumerate().map(|(i, line)| {
                            render_code_line(i + 1, line)
                        })),
                ),
        )
        // Status bar
        .child(
            div()
                .id("editor-status")
                .h(px(24.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .bg(bg::SURFACE)
                .border_t_1()
                .border_color(border::DEFAULT)
                // Left: file info
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(active_tab.map_or("--".to_string(), |t| t.file_type.indicator().to_string())),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!("{} lines", line_count)),
                        ),
                )
                // Right: cursor position
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!(
                                    "Ln {}, Col {}",
                                    active_tab.map_or(1, |t| t.cursor_line),
                                    active_tab.map_or(1, |t| t.cursor_column)
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("UTF-8"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("LF"),
                        ),
                ),
        )
}

/// Render a single editor tab
fn render_editor_tab(tab: &EditorTab) -> impl IntoElement {
    let (bg_color, text_color, border_color) = if tab.is_active {
        (bg::APP, text::PRIMARY, Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
    } else {
        (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
    };

    div()
        .id(SharedString::from(format!("tab-{}", tab.path.replace('/', "-"))))
        .h_full()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(12.0))
        .bg(bg_color)
        .border_t_2()
        .border_color(border_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        // File type indicator
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(tab.file_type.indicator()),
        )
        // File name
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(tab.name.clone()),
        )
        // Modified indicator
        .when(tab.is_modified, |el| {
            el.child(
                div()
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(status::WARNING),
            )
        })
        // Close button
        .child(
            div()
                .text_size(px(10.0))
                .text_color(text::MUTED)
                .cursor_pointer()
                .hover(|s| s.text_color(text::PRIMARY))
                .child("x"),
        )
}

/// Render a single line of code with basic syntax highlighting
fn render_code_line(line_num: usize, content: &str) -> impl IntoElement {
    // Simple syntax highlighting based on content
    let parts = tokenize_line(content);

    div()
        .id(SharedString::from(format!("line-{}", line_num)))
        .h(px(20.0))
        .flex()
        .items_center()
        .children(parts.into_iter().map(|(text, token_type)| {
            let color = match token_type {
                TokenType::Keyword => Hsla { h: 0.83, s: 0.7, l: 0.7, a: 1.0 }, // Purple
                TokenType::String => Hsla { h: 0.3, s: 0.6, l: 0.6, a: 1.0 }, // Green
                TokenType::Number => Hsla { h: 0.08, s: 0.8, l: 0.7, a: 1.0 }, // Orange
                TokenType::Comment => text::MUTED,
                TokenType::Type => Hsla { h: 0.55, s: 0.6, l: 0.7, a: 1.0 }, // Cyan
                TokenType::Function => Hsla { h: 0.14, s: 0.9, l: 0.6, a: 1.0 }, // Yellow
                TokenType::Operator => text::MUTED,
                TokenType::Plain => text::PRIMARY,
            };

            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(text)
        }))
}

#[derive(Clone, Copy)]
enum TokenType {
    Keyword,
    String,
    Number,
    Comment,
    Type,
    Function,
    Operator,
    Plain,
}

/// Simple tokenizer for basic syntax highlighting
fn tokenize_line(content: &str) -> Vec<(String, TokenType)> {
    let keywords = ["import", "export", "from", "const", "let", "var", "function", "return", "if", "else", "for", "while", "async", "await", "default", "type"];
    let types = ["React", "useState", "useEffect", "String", "number", "boolean", "void", "Promise"];

    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_char = '"';
    let mut in_comment = false;

    for (i, c) in content.chars().enumerate() {
        // Check for comment start
        if !in_string && c == '/' && content.chars().nth(i + 1) == Some('/') {
            if !current.is_empty() {
                result.push((current.clone(), classify_word(&current, &keywords, &types)));
                current.clear();
            }
            result.push((content[i..].to_string(), TokenType::Comment));
            return result;
        }

        // Handle strings
        if !in_comment && (c == '"' || c == '\'' || c == '`') {
            if in_string && c == string_char {
                current.push(c);
                result.push((current.clone(), TokenType::String));
                current.clear();
                in_string = false;
            } else if !in_string {
                if !current.is_empty() {
                    result.push((current.clone(), classify_word(&current, &keywords, &types)));
                    current.clear();
                }
                string_char = c;
                in_string = true;
                current.push(c);
            } else {
                current.push(c);
            }
            continue;
        }

        if in_string {
            current.push(c);
            continue;
        }

        // Handle word boundaries
        if c.is_whitespace() || "(){}[]<>,;:.=+-*/&|!".contains(c) {
            if !current.is_empty() {
                result.push((current.clone(), classify_word(&current, &keywords, &types)));
                current.clear();
            }
            if "=+-*/&|!<>".contains(c) {
                result.push((c.to_string(), TokenType::Operator));
            } else {
                result.push((c.to_string(), TokenType::Plain));
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        result.push((current.clone(), classify_word(&current, &keywords, &types)));
    }

    result
}

fn classify_word(word: &str, keywords: &[&str], types: &[&str]) -> TokenType {
    if keywords.contains(&word) {
        TokenType::Keyword
    } else if types.contains(&word) {
        TokenType::Type
    } else if word.chars().all(|c| c.is_ascii_digit() || c == '.') && !word.is_empty() {
        TokenType::Number
    } else if word.ends_with('(') || word.chars().next().map_or(false, |c| c.is_uppercase()) {
        TokenType::Function
    } else {
        TokenType::Plain
    }
}
