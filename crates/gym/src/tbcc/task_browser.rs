//! TBCC Task Browser Tab - Browse and run benchmark tasks

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::types::{TBTask, TBDifficulty};

pub struct TaskBrowserView {
    tasks: Vec<TBTask>,
    selected_task_id: Option<String>,
    difficulty_filter: Option<TBDifficulty>,
    focus_handle: FocusHandle,
}

impl TaskBrowserView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Sample tasks from TB2
        let tasks = vec![
            TBTask {
                id: "regex-log".to_string(),
                name: "Regex Log Parser".to_string(),
                description: "Parse and extract structured data from log files using regular expressions. Handle multiple log formats and edge cases.".to_string(),
                difficulty: TBDifficulty::Medium,
                timeout_ms: 120_000,
                max_turns: 15,
                tags: vec!["regex".to_string(), "parsing".to_string()],
            },
            TBTask {
                id: "file-ops".to_string(),
                name: "File Operations".to_string(),
                description: "Perform file system operations including reading, writing, moving, and organizing files efficiently.".to_string(),
                difficulty: TBDifficulty::Easy,
                timeout_ms: 60_000,
                max_turns: 10,
                tags: vec!["files".to_string(), "io".to_string()],
            },
            TBTask {
                id: "api-client".to_string(),
                name: "REST API Client".to_string(),
                description: "Build a client that correctly handles REST API requests, authentication, error handling, and retries.".to_string(),
                difficulty: TBDifficulty::Hard,
                timeout_ms: 180_000,
                max_turns: 20,
                tags: vec!["http".to_string(), "api".to_string()],
            },
            TBTask {
                id: "data-transform".to_string(),
                name: "Data Transformation".to_string(),
                description: "Transform and normalize data between different formats (JSON, CSV, XML) while preserving data integrity.".to_string(),
                difficulty: TBDifficulty::Medium,
                timeout_ms: 90_000,
                max_turns: 12,
                tags: vec!["json".to_string(), "csv".to_string(), "xml".to_string()],
            },
            TBTask {
                id: "db-query".to_string(),
                name: "Database Query Builder".to_string(),
                description: "Construct complex SQL queries with joins, aggregations, and filtering based on natural language requirements.".to_string(),
                difficulty: TBDifficulty::Expert,
                timeout_ms: 240_000,
                max_turns: 25,
                tags: vec!["sql".to_string(), "database".to_string()],
            },
        ];

        Self {
            tasks,
            selected_task_id: None,
            difficulty_filter: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn render_difficulty_badge(&self, difficulty: TBDifficulty) -> impl IntoElement {
        let (bg_color, text_color, label) = match difficulty {
            TBDifficulty::Easy => (status::SUCCESS_BG, status::SUCCESS, "Easy"),
            TBDifficulty::Medium => (status::INFO_BG, status::INFO, "Medium"),
            TBDifficulty::Hard => (status::WARNING_BG, status::WARNING, "Hard"),
            TBDifficulty::Expert => (status::ERROR_BG, status::ERROR, "Expert"),
            TBDifficulty::Unknown => (bg::ELEVATED, text::MUTED, "Unknown"),
        };

        div()
            .px(px(8.0))
            .py(px(3.0))
            .bg(bg_color)
            .text_color(text_color)
            .text_size(px(10.0))
            .font_family(FONT_FAMILY)
            .font_weight(FontWeight::MEDIUM)
            .rounded(px(4.0))
            .child(label)
    }

    fn render_task_item(&self, task: &TBTask) -> impl IntoElement {
        let is_selected = self.selected_task_id.as_deref() == Some(&task.id);

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(12.0))
            .bg(if is_selected { bg::SELECTED } else { bg::ROW })
            .border_b_1()
            .border_color(border::SUBTLE)
            .when(is_selected, |el| {
                el.border_l_2()
                    .border_color(border::SELECTED)
            })
            .hover(|el| el.bg(if is_selected { bg::SELECTED } else { bg::HOVER }))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_selected { text::BRIGHT } else { text::PRIMARY })
                            .font_weight(FontWeight::MEDIUM)
                            .child(task.name.clone())
                    )
                    .child(self.render_difficulty_badge(task.difficulty))
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .line_height(px(16.0))
                    .child(task.description.clone())
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(format!("⏱ {}s timeout", task.timeout_ms / 1000))
                    .child("•")
                    .child(format!("🔄 {} max turns", task.max_turns))
            )
    }

    fn render_task_detail(&self) -> impl IntoElement {
        if let Some(task_id) = &self.selected_task_id {
            if let Some(task) = self.tasks.iter().find(|t| &t.id == task_id) {
                return div()
                    .flex()
                    .flex_col()
                    .gap(px(20.0))
                    .p(px(20.0))
                    .h_full()
                    .bg(bg::APP)
                    // Header
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::BRIGHT)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(task.name.clone())
                                    )
                                    .child(self.render_difficulty_badge(task.difficulty))
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::SECONDARY)
                                    .line_height(px(20.0))
                                    .child(task.description.clone())
                            )
                    )
                    // Metadata
                    .child(
                        div()
                            .flex()
                            .gap(px(16.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("Timeout")
                                    )
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(format!("{}s", task.timeout_ms / 1000))
                                    )
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("Max Turns")
                                    )
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(format!("{}", task.max_turns))
                                    )
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("Task ID")
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::DISABLED)
                                            .child(task.id.clone())
                                    )
                            )
                    )
                    // Run buttons
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Run Task")
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .px(px(16.0))
                                            .py(px(10.0))
                                            .bg(bg::HOVER)
                                            .border_1()
                                            .border_color(border::DEFAULT)
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .hover(|el| el.bg(bg::CARD).border_color(border::SELECTED))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::PRIMARY)
                                                    .child("Quick Run")
                                            )
                                    )
                                    .child(
                                        div()
                                            .px(px(16.0))
                                            .py(px(10.0))
                                            .bg(bg::HOVER)
                                            .border_1()
                                            .border_color(border::DEFAULT)
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .hover(|el| el.bg(bg::CARD).border_color(border::SELECTED))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::PRIMARY)
                                                    .child("Standard Run")
                                            )
                                    )
                                    .child(
                                        div()
                                            .px(px(16.0))
                                            .py(px(10.0))
                                            .bg(bg::HOVER)
                                            .border_1()
                                            .border_color(border::DEFAULT)
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .hover(|el| el.bg(bg::CARD).border_color(border::SELECTED))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::PRIMARY)
                                                    .child("Full Run")
                                            )
                                    )
                            )
                    )
                    .into_any_element();
            }
        }

        // Empty state
        div()
            .flex()
            .items_center()
            .justify_center()
            .h_full()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child("Select a task to view details")
            )
            .into_any_element()
    }
}

impl Focusable for TaskBrowserView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TaskBrowserView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let filtered_tasks: Vec<&TBTask> = self.tasks.iter()
            .filter(|task| {
                if let Some(filter) = self.difficulty_filter {
                    task.difficulty == filter
                } else {
                    true
                }
            })
            .collect();

        div()
            .flex()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Task list (left)
            .child(
                div()
                    .w(px(400.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(bg::SURFACE)
                    .border_r_1()
                    .border_color(border::DEFAULT)
                    // Header with filters
                    .child(
                        div()
                            .px(px(16.0))
                            .py(px(12.0))
                            .border_b_1()
                            .border_color(border::DEFAULT)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!("Tasks ({})", filtered_tasks.len()))
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("🔍 Filter")
                                    )
                            )
                    )
                    // Task list
                    .child(
                        div()
                            .id("task-list-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .children(filtered_tasks.iter().map(|task| self.render_task_item(task)))
                    )
            )
            // Task detail (right)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .child(self.render_task_detail())
            )
    }
}
