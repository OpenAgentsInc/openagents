//! File tree component - Project file browser with OANIX namespace

use gpui::*;
use gpui::prelude::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use crate::types::{ProjectFile, FileType, GitStatus};

/// Render the file tree sidebar
pub fn render_file_tree(files: &[ProjectFile], active_path: &Option<String>) -> impl IntoElement {
    div()
        .id("file-tree")
        .w(px(240.0))
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
                        .child("EXPLORER"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        // New file button
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("+"),
                        )
                        // Refresh button
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("R"),
                        ),
                ),
        )
        // File tree content
        .child(
            div()
                .id("file-tree-content")
                .flex_1()
                .overflow_y_scroll()
                .py(px(4.0))
                .children(files.iter().map(|file| {
                    render_file_item(file, active_path, 0)
                })),
        )
        // Footer with git status summary
        .child(
            div()
                .h(px(28.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(12.0))
                .border_t_1()
                .border_color(border::DEFAULT)
                .bg(bg::ELEVATED)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(render_git_stat("M", "4", status::WARNING))
                        .child(render_git_stat("A", "1", status::SUCCESS))
                        .child(render_git_stat("?", "1", text::MUTED)),
                ),
        )
}

/// Render a single file/directory item
fn render_file_item(file: &ProjectFile, active_path: &Option<String>, depth: u32) -> impl IntoElement {
    let is_active = active_path.as_ref().map_or(false, |p| p == &file.path);
    let indent = (depth as f32) * 12.0 + 8.0;

    let text_color = match file.git_status {
        GitStatus::Modified => status::WARNING,
        GitStatus::Added => status::SUCCESS,
        GitStatus::Deleted => status::ERROR,
        GitStatus::Untracked => text::MUTED,
        _ => text::PRIMARY,
    };

    div()
        .id(SharedString::from(format!("file-{}", file.path.replace('/', "-"))))
        .w_full()
        .child(
            // File/folder row
            div()
                .w_full()
                .h(px(24.0))
                .flex()
                .items_center()
                .pl(px(indent))
                .pr(px(8.0))
                .bg(if is_active { bg::SELECTED } else { Hsla::transparent_black() })
                .cursor_pointer()
                .hover(|s| s.bg(bg::HOVER))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .flex_1()
                        // Expand arrow for directories
                        .when(file.is_directory, |el| {
                            el.child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(text::MUTED)
                                    .child(if file.is_expanded { "v" } else { ">" }),
                            )
                        })
                        // File type indicator
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .min_w(px(16.0))
                                .child(file.file_type.indicator()),
                        )
                        // File name
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text_color)
                                .child(file.name.clone()),
                        ),
                )
                // Git status indicator
                .when(file.git_status != GitStatus::Unchanged, |el| {
                    el.child(
                        div()
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text_color)
                            .child(file.git_status.indicator()),
                    )
                }),
        )
        // Children for expanded directories
        .when(file.is_directory && file.is_expanded, |el| {
            el.children(file.children.iter().map(|child| {
                render_file_item(child, active_path, depth + 1)
            }))
        })
}

/// Render a git status stat in the footer
fn render_git_stat(indicator: &str, count: &str, color: Hsla) -> impl IntoElement {
    let indicator = indicator.to_string();
    let count = count.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(indicator),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(count),
        )
}
