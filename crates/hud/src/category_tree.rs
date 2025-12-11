//! Category Tree Widget: Collapsible task tree view
//!
//! Displays TB (Terminal-Bench) tasks grouped by category in a collapsible tree.
//! Supports expand/collapse all, task status icons, and category statistics.
//!
//! # User Stories
//!
//! - HUD-060: Display tasks grouped by category
//! - HUD-061: Color-code tasks by status (pending, running, passed, failed)
//! - HUD-062: Show category statistics (passed/failed counts)
//! - HUD-063: Support expand/collapse categories
//! - HUD-064: Support task selection

use gpui::{
    div, prelude::*, px, Hsla, Render, Window, Context, Entity,
    IntoElement, InteractiveElement, StatefulInteractiveElement, SharedString,
};
use std::collections::{HashMap, HashSet};
use theme_oa::hud;

/// Task status in Terminal-Bench
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Passed,
    Failed,
    Timeout,
    Error,
}

impl TaskStatus {
    /// Get the icon for this status
    pub fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "○",
            TaskStatus::Running => "▶",
            TaskStatus::Passed => "✓",
            TaskStatus::Failed => "✗",
            TaskStatus::Timeout => "⏱",
            TaskStatus::Error => "⚠",
        }
    }

    /// Get the color for this status
    pub fn color(&self) -> Hsla {
        match self {
            TaskStatus::Pending => hud::STATUS_PENDING,
            TaskStatus::Running => hud::STATUS_RUNNING,
            TaskStatus::Passed => hud::STATUS_PASSED,
            TaskStatus::Failed => hud::STATUS_FAILED,
            TaskStatus::Timeout => hud::STATUS_TIMEOUT,
            TaskStatus::Error => hud::STATUS_ERROR,
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => TaskStatus::Pending,
            "running" => TaskStatus::Running,
            "passed" => TaskStatus::Passed,
            "failed" => TaskStatus::Failed,
            "timeout" => TaskStatus::Timeout,
            "error" => TaskStatus::Error,
            _ => TaskStatus::Pending,
        }
    }
}

/// Task difficulty level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskDifficulty {
    #[default]
    Unknown,
    Easy,
    Medium,
    Hard,
}

impl TaskDifficulty {
    /// Get background color for this difficulty
    pub fn bg_color(&self) -> Hsla {
        match self {
            TaskDifficulty::Easy => hud::DIFFICULTY_EASY_BG,
            TaskDifficulty::Medium => hud::DIFFICULTY_MEDIUM_BG,
            TaskDifficulty::Hard => hud::DIFFICULTY_HARD_BG,
            TaskDifficulty::Unknown => hud::DIFFICULTY_UNKNOWN_BG,
        }
    }

    /// Get text color for this difficulty
    pub fn text_color(&self) -> Hsla {
        match self {
            TaskDifficulty::Easy => hud::DIFFICULTY_EASY_TEXT,
            TaskDifficulty::Medium => hud::DIFFICULTY_MEDIUM_TEXT,
            TaskDifficulty::Hard => hud::DIFFICULTY_HARD_TEXT,
            TaskDifficulty::Unknown => hud::DIFFICULTY_UNKNOWN_TEXT,
        }
    }

    /// Get single-letter badge
    pub fn badge(&self) -> &'static str {
        match self {
            TaskDifficulty::Easy => "E",
            TaskDifficulty::Medium => "M",
            TaskDifficulty::Hard => "H",
            TaskDifficulty::Unknown => "?",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "easy" => TaskDifficulty::Easy,
            "medium" => TaskDifficulty::Medium,
            "hard" => TaskDifficulty::Hard,
            _ => TaskDifficulty::Unknown,
        }
    }
}

/// Task data
#[derive(Debug, Clone)]
pub struct TaskData {
    /// Task ID
    pub id: String,
    /// Task name
    pub name: String,
    /// Difficulty level
    pub difficulty: TaskDifficulty,
    /// Category name
    pub category: String,
    /// Current status
    pub status: TaskStatus,
}

impl TaskData {
    /// Create a new task
    pub fn new(id: impl Into<String>, name: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            difficulty: TaskDifficulty::Unknown,
            category: category.into(),
            status: TaskStatus::Pending,
        }
    }

    /// Set difficulty
    pub fn with_difficulty(mut self, difficulty: TaskDifficulty) -> Self {
        self.difficulty = difficulty;
        self
    }

    /// Set status
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self
    }
}

/// Category data with aggregated statistics
#[derive(Debug, Clone, Default)]
pub struct CategoryData {
    /// Category name
    pub category: String,
    /// Tasks in this category
    pub task_ids: Vec<String>,
    /// Passed count
    pub passed: usize,
    /// Failed count (includes error, timeout)
    pub failed: usize,
    /// Pending count
    pub pending: usize,
    /// Running count
    pub running: usize,
    /// Total count
    pub total: usize,
}

impl CategoryData {
    /// Create a new category
    pub fn new(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            ..Default::default()
        }
    }

    /// Add a task to this category
    pub fn add_task(&mut self, task: &TaskData) {
        self.task_ids.push(task.id.clone());
        self.total += 1;
        match task.status {
            TaskStatus::Passed => self.passed += 1,
            TaskStatus::Failed | TaskStatus::Error | TaskStatus::Timeout => self.failed += 1,
            TaskStatus::Pending => self.pending += 1,
            TaskStatus::Running => self.running += 1,
        }
    }
}

/// Category tree state
#[derive(Debug, Clone, Default)]
pub struct CategoryTreeState {
    /// All tasks indexed by ID
    pub tasks: HashMap<String, TaskData>,
    /// Collapsed categories
    pub collapsed_categories: HashSet<String>,
    /// Whether the tree is visible
    pub visible: bool,
    /// Selected task ID
    pub selected_task_id: Option<String>,
}

impl CategoryTreeState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self {
            visible: true,
            ..Default::default()
        }
    }

    /// Add or update a task
    pub fn upsert_task(&mut self, task: TaskData) {
        self.tasks.insert(task.id.clone(), task);
    }

    /// Update task status
    pub fn update_task_status(&mut self, task_id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = status;
        }
    }

    /// Clear all tasks
    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
    }

    /// Toggle category collapsed state
    pub fn toggle_category(&mut self, category: &str) {
        if self.collapsed_categories.contains(category) {
            self.collapsed_categories.remove(category);
        } else {
            self.collapsed_categories.insert(category.to_string());
        }
    }

    /// Expand all categories
    pub fn expand_all(&mut self) {
        self.collapsed_categories.clear();
    }

    /// Collapse all categories
    pub fn collapse_all(&mut self) {
        for cat in self.get_categories().keys() {
            self.collapsed_categories.insert(cat.clone());
        }
    }

    /// Select a task
    pub fn select_task(&mut self, task_id: Option<String>) {
        self.selected_task_id = task_id;
    }

    /// Toggle visibility
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    /// Get tasks grouped by category
    pub fn get_categories(&self) -> HashMap<String, CategoryData> {
        let mut categories: HashMap<String, CategoryData> = HashMap::new();

        for task in self.tasks.values() {
            let cat_name = if task.category.is_empty() {
                "uncategorized".to_string()
            } else {
                task.category.clone()
            };

            let cat = categories.entry(cat_name.clone()).or_insert_with(|| {
                CategoryData::new(&cat_name)
            });
            cat.add_task(task);
        }

        categories
    }

    /// Check if a category is collapsed
    pub fn is_collapsed(&self, category: &str) -> bool {
        self.collapsed_categories.contains(category)
    }

    /// Get total passed count across all tasks
    pub fn total_passed(&self) -> usize {
        self.tasks.values().filter(|t| t.status == TaskStatus::Passed).count()
    }

    /// Get total failed count across all tasks
    pub fn total_failed(&self) -> usize {
        self.tasks.values().filter(|t| matches!(t.status, TaskStatus::Failed | TaskStatus::Error | TaskStatus::Timeout)).count()
    }
}

/// Category Tree GPUI widget
pub struct CategoryTree {
    /// Current state
    state: CategoryTreeState,
}

impl CategoryTree {
    /// Create a new category tree widget
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            state: CategoryTreeState::new(),
        }
    }

    /// Update entire state
    pub fn update_state(&mut self, state: CategoryTreeState, cx: &mut Context<Self>) {
        self.state = state;
        cx.notify();
    }

    /// Add or update a task
    pub fn upsert_task(&mut self, task: TaskData, cx: &mut Context<Self>) {
        self.state.upsert_task(task);
        cx.notify();
    }

    /// Update task status
    pub fn update_task_status(&mut self, task_id: &str, status: TaskStatus, cx: &mut Context<Self>) {
        self.state.update_task_status(task_id, status);
        cx.notify();
    }

    /// Load tasks from a list
    pub fn load_tasks(&mut self, tasks: Vec<TaskData>, cx: &mut Context<Self>) {
        self.state.clear_tasks();
        for task in tasks {
            self.state.upsert_task(task);
        }
        cx.notify();
    }

    /// Toggle category
    pub fn toggle_category(&mut self, category: &str, cx: &mut Context<Self>) {
        self.state.toggle_category(category);
        cx.notify();
    }

    /// Expand all categories
    pub fn expand_all(&mut self, cx: &mut Context<Self>) {
        self.state.expand_all();
        cx.notify();
    }

    /// Collapse all categories
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.state.collapse_all();
        cx.notify();
    }

    /// Select a task
    pub fn select_task(&mut self, task_id: Option<String>, cx: &mut Context<Self>) {
        self.state.select_task(task_id);
        cx.notify();
    }

    /// Toggle visibility
    pub fn toggle_visibility(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_visibility();
        cx.notify();
    }

    /// Get current state
    pub fn state(&self) -> &CategoryTreeState {
        &self.state
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.state.visible
    }

    /// Get selected task
    pub fn selected_task(&self) -> Option<&TaskData> {
        self.state.selected_task_id.as_ref().and_then(|id| self.state.tasks.get(id))
    }
}

impl Render for CategoryTree {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if !self.state.visible {
            return div().into_any_element();
        }

        let categories = self.state.get_categories();

        // If no tasks, show empty state
        if categories.is_empty() {
            return div()
                .absolute()
                .top(px(80.0))
                .right(px(16.0))
                .w(px(288.0))
                .bg(hud::PANEL_BG)
                .border_1()
                .border_color(hud::PANEL_BORDER)
                .rounded_lg()
                .overflow_hidden()
                .child(self.render_header())
                .child(
                    div()
                        .p(px(16.0))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .text_color(theme_oa::text::MUTED)
                                .text_size(px(14.0))
                                .child("No tasks loaded")
                        )
                )
                .into_any_element();
        }

        // Sort categories alphabetically
        let mut sorted_cats: Vec<_> = categories.into_iter().collect();
        sorted_cats.sort_by(|a, b| a.0.cmp(&b.0));

        // Build category elements
        let category_elements: Vec<_> = sorted_cats
            .iter()
            .map(|(cat_name, cat_data)| self.render_category(cat_name, cat_data))
            .collect();

        div()
            .id("category-tree")
            .absolute()
            .top(px(80.0))
            .right(px(16.0))
            .w(px(288.0))
            .max_h(px(560.0))
            .flex()
            .flex_col()
            .bg(hud::PANEL_BG)
            .border_1()
            .border_color(hud::PANEL_BORDER)
            .rounded_lg()
            .overflow_hidden()
            .child(self.render_header())
            .child(
                div()
                    .id("category-tree-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(category_elements)
            )
            .into_any_element()
    }
}

impl CategoryTree {
    /// Render the header with controls
    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(hud::PANEL_BORDER)
            .bg(hud::HEADER_BG)
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme_oa::text::PRIMARY)
                    .child("Categories")
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme_oa::text::SECONDARY)
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(hud::BUTTON_BORDER)
                            .cursor_pointer()
                            .hover(|s| s.border_color(hud::BUTTON_BORDER_HOVER))
                            .child("Expand")
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme_oa::text::SECONDARY)
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(hud::BUTTON_BORDER)
                            .cursor_pointer()
                            .hover(|s| s.border_color(hud::BUTTON_BORDER_HOVER))
                            .child("Collapse")
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(theme_oa::text::MUTED)
                            .cursor_pointer()
                            .hover(|s| s.text_color(theme_oa::text::PRIMARY))
                            .child("×")
                    )
            )
    }

    /// Render a single category with its tasks
    fn render_category(&self, cat_name: &str, cat_data: &CategoryData) -> impl IntoElement {
        let is_collapsed = self.state.is_collapsed(cat_name);
        let chevron = if is_collapsed { "▶" } else { "▼" };

        // Stats display
        let stats = if cat_data.passed > 0 || cat_data.failed > 0 {
            Some((cat_data.passed, cat_data.failed))
        } else {
            None
        };

        // Build task elements if expanded
        let task_elements: Vec<_> = if is_collapsed {
            vec![]
        } else {
            cat_data.task_ids.iter()
                .filter_map(|id| self.state.tasks.get(id))
                .map(|task| self.render_task(task))
                .collect()
        };

        div()
            .border_b_1()
            .border_color(hud::DIVIDER)
            .child(
                // Category header
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(hud::ROW_HOVER))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme_oa::text::MUTED)
                            .child(chevron)
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme_oa::text::PRIMARY)
                            .child(cat_name.to_string())
                    )
                    .when_some(stats, |this, (passed, failed)| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(hud::STATUS_PASSED)
                                        .child(format!("✓{}", passed))
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(hud::STATUS_FAILED)
                                        .child(format!("✗{}", failed))
                                )
                        )
                    })
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme_oa::text::MUTED)
                            .child(format!("{}", cat_data.total))
                    )
            )
            .when(!is_collapsed && !task_elements.is_empty(), |this| {
                this.child(
                    div()
                        .pb(px(4.0))
                        .children(task_elements)
                )
            })
    }

    /// Render a single task item
    fn render_task(&self, task: &TaskData) -> impl IntoElement {
        let is_selected = self.state.selected_task_id.as_ref() == Some(&task.id);
        let is_running = task.status == TaskStatus::Running;
        let status_color = task.status.color();
        let status_icon = task.status.icon();

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(6.0))
            .cursor_pointer()
            .when(is_selected, |s| s.bg(hud::ROW_SELECTED))
            .when(!is_selected, |s| s.hover(|s| s.bg(hud::ROW_HOVER)))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(status_color)
                    .child(status_icon)
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(theme_oa::text::PRIMARY)
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(is_running, |s| s.text_color(hud::STATUS_RUNNING))
                    .child(task.name.clone())
            )
            .when(task.difficulty != TaskDifficulty::Unknown, |this| {
                this.child(
                    div()
                        .text_size(px(9.0))
                        .px(px(4.0))
                        .py(px(2.0))
                        .rounded(px(2.0))
                        .bg(task.difficulty.bg_color())
                        .text_color(task.difficulty.text_color())
                        .child(task.difficulty.badge())
                )
            })
    }
}

/// Create a CategoryTree entity
pub fn category_tree(cx: &mut gpui::App) -> Entity<CategoryTree> {
    cx.new(|cx| CategoryTree::new(cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_icon() {
        assert_eq!(TaskStatus::Pending.icon(), "○");
        assert_eq!(TaskStatus::Running.icon(), "▶");
        assert_eq!(TaskStatus::Passed.icon(), "✓");
        assert_eq!(TaskStatus::Failed.icon(), "✗");
        assert_eq!(TaskStatus::Timeout.icon(), "⏱");
        assert_eq!(TaskStatus::Error.icon(), "⚠");
    }

    #[test]
    fn test_task_status_from_str() {
        assert_eq!(TaskStatus::from_str("pending"), TaskStatus::Pending);
        assert_eq!(TaskStatus::from_str("RUNNING"), TaskStatus::Running);
        assert_eq!(TaskStatus::from_str("Passed"), TaskStatus::Passed);
        assert_eq!(TaskStatus::from_str("unknown"), TaskStatus::Pending);
    }

    #[test]
    fn test_task_difficulty_badge() {
        assert_eq!(TaskDifficulty::Easy.badge(), "E");
        assert_eq!(TaskDifficulty::Medium.badge(), "M");
        assert_eq!(TaskDifficulty::Hard.badge(), "H");
        assert_eq!(TaskDifficulty::Unknown.badge(), "?");
    }

    #[test]
    fn test_task_data_builder() {
        let task = TaskData::new("task-1", "Test Task", "testing")
            .with_difficulty(TaskDifficulty::Medium)
            .with_status(TaskStatus::Running);

        assert_eq!(task.id, "task-1");
        assert_eq!(task.name, "Test Task");
        assert_eq!(task.category, "testing");
        assert_eq!(task.difficulty, TaskDifficulty::Medium);
        assert_eq!(task.status, TaskStatus::Running);
    }

    #[test]
    fn test_category_data_add_task() {
        let mut cat = CategoryData::new("regex");

        let task1 = TaskData::new("t1", "Task 1", "regex").with_status(TaskStatus::Passed);
        let task2 = TaskData::new("t2", "Task 2", "regex").with_status(TaskStatus::Failed);
        let task3 = TaskData::new("t3", "Task 3", "regex").with_status(TaskStatus::Pending);

        cat.add_task(&task1);
        cat.add_task(&task2);
        cat.add_task(&task3);

        assert_eq!(cat.total, 3);
        assert_eq!(cat.passed, 1);
        assert_eq!(cat.failed, 1);
        assert_eq!(cat.pending, 1);
        assert_eq!(cat.task_ids.len(), 3);
    }

    #[test]
    fn test_state_upsert_task() {
        let mut state = CategoryTreeState::new();

        let task = TaskData::new("task-1", "Test", "cat-1");
        state.upsert_task(task);

        assert_eq!(state.tasks.len(), 1);
        assert!(state.tasks.contains_key("task-1"));
    }

    #[test]
    fn test_state_update_task_status() {
        let mut state = CategoryTreeState::new();

        let task = TaskData::new("task-1", "Test", "cat-1");
        state.upsert_task(task);
        state.update_task_status("task-1", TaskStatus::Passed);

        assert_eq!(state.tasks.get("task-1").unwrap().status, TaskStatus::Passed);
    }

    #[test]
    fn test_state_toggle_category() {
        let mut state = CategoryTreeState::new();

        assert!(!state.is_collapsed("cat-1"));

        state.toggle_category("cat-1");
        assert!(state.is_collapsed("cat-1"));

        state.toggle_category("cat-1");
        assert!(!state.is_collapsed("cat-1"));
    }

    #[test]
    fn test_state_get_categories() {
        let mut state = CategoryTreeState::new();

        state.upsert_task(TaskData::new("t1", "Task 1", "regex"));
        state.upsert_task(TaskData::new("t2", "Task 2", "regex"));
        state.upsert_task(TaskData::new("t3", "Task 3", "json"));

        let categories = state.get_categories();
        assert_eq!(categories.len(), 2);
        assert!(categories.contains_key("regex"));
        assert!(categories.contains_key("json"));
        assert_eq!(categories.get("regex").unwrap().total, 2);
        assert_eq!(categories.get("json").unwrap().total, 1);
    }

    #[test]
    fn test_state_expand_collapse_all() {
        let mut state = CategoryTreeState::new();

        state.upsert_task(TaskData::new("t1", "Task 1", "cat-1"));
        state.upsert_task(TaskData::new("t2", "Task 2", "cat-2"));
        state.upsert_task(TaskData::new("t3", "Task 3", "cat-3"));

        // Initially all expanded
        assert!(!state.is_collapsed("cat-1"));
        assert!(!state.is_collapsed("cat-2"));
        assert!(!state.is_collapsed("cat-3"));

        // Collapse all
        state.collapse_all();
        assert!(state.is_collapsed("cat-1"));
        assert!(state.is_collapsed("cat-2"));
        assert!(state.is_collapsed("cat-3"));

        // Expand all
        state.expand_all();
        assert!(!state.is_collapsed("cat-1"));
        assert!(!state.is_collapsed("cat-2"));
        assert!(!state.is_collapsed("cat-3"));
    }

    #[test]
    fn test_state_select_task() {
        let mut state = CategoryTreeState::new();
        state.upsert_task(TaskData::new("task-1", "Test", "cat-1"));

        assert!(state.selected_task_id.is_none());

        state.select_task(Some("task-1".to_string()));
        assert_eq!(state.selected_task_id, Some("task-1".to_string()));

        state.select_task(None);
        assert!(state.selected_task_id.is_none());
    }

    #[test]
    fn test_state_visibility() {
        let mut state = CategoryTreeState::new();
        assert!(state.visible);

        state.toggle_visibility();
        assert!(!state.visible);

        state.toggle_visibility();
        assert!(state.visible);
    }

    #[test]
    fn test_state_total_counts() {
        let mut state = CategoryTreeState::new();

        state.upsert_task(TaskData::new("t1", "T1", "c").with_status(TaskStatus::Passed));
        state.upsert_task(TaskData::new("t2", "T2", "c").with_status(TaskStatus::Passed));
        state.upsert_task(TaskData::new("t3", "T3", "c").with_status(TaskStatus::Failed));
        state.upsert_task(TaskData::new("t4", "T4", "c").with_status(TaskStatus::Error));
        state.upsert_task(TaskData::new("t5", "T5", "c").with_status(TaskStatus::Pending));

        assert_eq!(state.total_passed(), 2);
        assert_eq!(state.total_failed(), 2); // Failed + Error
    }

    #[test]
    fn test_uncategorized_tasks() {
        let mut state = CategoryTreeState::new();

        state.upsert_task(TaskData::new("t1", "T1", ""));
        state.upsert_task(TaskData::new("t2", "T2", ""));

        let categories = state.get_categories();
        assert_eq!(categories.len(), 1);
        assert!(categories.contains_key("uncategorized"));
        assert_eq!(categories.get("uncategorized").unwrap().total, 2);
    }
}
