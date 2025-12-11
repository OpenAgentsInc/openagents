//! TBCC Task Browser Tab - Browse and run benchmark tasks

use gpui_oa::prelude::*;
use gpui_oa::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::{TBTask, TBDifficulty};
use crate::services::TaskLoader;

pub struct TaskBrowserView {
    tasks: Vec<TBTask>,
    selected_task_id: Option<String>,
    difficulty_filter: Option<TBDifficulty>,
    loading: bool,
    error: Option<String>,
    task_loader: TaskLoader,
    focus_handle: FocusHandle,
}

impl TaskBrowserView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let task_loader = TaskLoader::new();

        // Load tasks from available suites
        let tasks = task_loader.load_all_tasks();

        Self {
            tasks,
            selected_task_id: None,
            difficulty_filter: None,
            loading: false,
            error: None,
            task_loader,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Refresh tasks from disk
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.tasks = self.task_loader.load_all_tasks();
        self.loading = false;
        cx.notify();
    }

    /// Load tasks from a specific suite file
    pub fn load_suite(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        self.loading = true;
        match self.task_loader.load_suite(path) {
            Ok(suite) => {
                self.tasks = suite.tasks;
                self.error = None;
            }
            Err(e) => {
                self.error = Some(e.to_string());
            }
        }
        self.loading = false;
        cx.notify();
    }

    /// Get task count info
    pub fn task_count(&self) -> (usize, usize) {
        let filtered = self.filtered_tasks().len();
        (filtered, self.tasks.len())
    }

    /// Get filtered tasks based on current filter
    fn filtered_tasks(&self) -> Vec<&TBTask> {
        self.tasks.iter()
            .filter(|task| {
                if let Some(filter) = self.difficulty_filter {
                    task.difficulty == filter
                } else {
                    true
                }
            })
            .collect()
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
