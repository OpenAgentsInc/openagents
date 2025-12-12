//! Gym panel - Terminal-Bench integration panel
//!
//! Bloomberg Terminal-style panel for running Terminal-Bench tasks:
//! - Task selector dropdown
//! - Action buttons (Run TB2, Run TestGen)
//! - Active run progress
//! - Recent runs history

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use theme_oa::{bg, border, status, text, FONT_FAMILY};
use ui_oa::{Button, ButtonVariant};
use terminalbench::{TBTask, TBRunSummary, TBRunStatus, TBRunOutcome, TaskLoader, TBModelOption};

/// Gym panel component
pub struct GymPanel {
    /// Focus handle
    focus_handle: FocusHandle,
    /// Task loader
    task_loader: TaskLoader,
    /// Available tasks
    tasks: Vec<TBTask>,
    /// Currently selected task index
    selected_task_idx: Option<usize>,
    /// Recent runs
    recent_runs: Vec<TBRunSummary>,
    /// Active run state
    active_run: Option<ActiveRunState>,
    /// Selected model
    selected_model: TBModelOption,
}

/// Active run state
#[derive(Clone, Debug)]
pub struct ActiveRunState {
    pub run_id: String,
    pub task_id: String,
    pub task_name: String,
    pub turns: u32,
    pub max_turns: u32,
}

impl GymPanel {
    /// Create a new Gym panel
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let task_loader = TaskLoader::new();

        // Try to load tasks
        let tasks = task_loader.load_all_tasks();
        let selected_idx = if tasks.is_empty() { None } else { Some(0) };

        Self {
            focus_handle,
            task_loader,
            tasks,
            selected_task_idx: selected_idx,
            recent_runs: vec![],
            active_run: None,
            selected_model: TBModelOption::ClaudeSonnet,
        }
    }

    /// Get the selected task
    pub fn selected_task(&self) -> Option<&TBTask> {
        self.selected_task_idx.and_then(|idx| self.tasks.get(idx))
    }

    /// Select a task by index
    pub fn select_task(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tasks.len() {
            self.selected_task_idx = Some(idx);
            cx.notify();
        }
    }

    /// Select next task
    pub fn select_next_task(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.selected_task_idx {
            let next = (idx + 1) % self.tasks.len();
            self.select_task(next, cx);
        }
    }

    /// Select previous task
    pub fn select_prev_task(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.selected_task_idx {
            let prev = if idx == 0 { self.tasks.len() - 1 } else { idx - 1 };
            self.select_task(prev, cx);
        }
    }

    /// Cycle to next model
    pub fn cycle_model(&mut self, cx: &mut Context<Self>) {
        self.selected_model = match self.selected_model {
            TBModelOption::ClaudeSonnet => TBModelOption::ClaudeHaiku,
            TBModelOption::ClaudeHaiku => TBModelOption::Gpt4o,
            TBModelOption::Gpt4o => TBModelOption::Gpt4oMini,
            TBModelOption::Gpt4oMini => TBModelOption::AppleFM,
            TBModelOption::AppleFM => TBModelOption::ClaudeSonnet,
        };
        cx.notify();
    }

    /// Check if a run is active
    pub fn is_running(&self) -> bool {
        self.active_run.is_some()
    }

    /// Reload tasks
    pub fn reload_tasks(&mut self, cx: &mut Context<Self>) {
        self.tasks = self.task_loader.load_all_tasks();
        if self.selected_task_idx.map_or(true, |idx| idx >= self.tasks.len()) {
            self.selected_task_idx = if self.tasks.is_empty() { None } else { Some(0) };
        }
        cx.notify();
    }

    /// Render the header
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .child(
                div()
                    .font_family(FONT_FAMILY)
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text::PRIMARY)
                    .child("GYM")
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .child("[Cmd+G to close]")
            )
    }

    /// Render the task selector section
    fn render_task_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .font_weight(gpui::FontWeight::BOLD)
                    .mb(px(4.0))
                    .child("TASK")
            )
            .child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .bg(bg::CARD)
                    .border_1()
                    .border_color(border::DEFAULT)
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.select_next_task(cx);
                    }))
                    .child(
                        if let Some(task) = self.selected_task() {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(text::PRIMARY)
                                        .child(task.id.clone())
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text::MUTED)
                                        .child("v")
                                )
                        } else {
                            div()
                                .text_sm()
                                .text_color(text::MUTED)
                                .child("No tasks found")
                        }
                    )
            )
    }

    /// Render the model selector
    fn render_model_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .font_weight(gpui::FontWeight::BOLD)
                    .mb(px(4.0))
                    .child("MODEL")
            )
            .child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .bg(bg::CARD)
                    .border_1()
                    .border_color(border::DEFAULT)
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.cycle_model(cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(text::PRIMARY)
                                    .child(self.selected_model.label())
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child("v")
                            )
                    )
            )
    }

    /// Render the action buttons
    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_task = self.selected_task().is_some();
        let is_running = self.is_running();

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .font_weight(gpui::FontWeight::BOLD)
                    .mb(px(4.0))
                    .child("ACTIONS")
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(
                        Button::new("Run TB2")
                            .variant(ButtonVariant::Default)
                            .disabled(!has_task || is_running)
                            .on_click(cx.listener(|this, _, _, cx| {
                                // TODO: Start TB2 run
                                log::info!("Run TB2 clicked");
                            }))
                    )
                    .child(
                        Button::new("TestGen")
                            .variant(ButtonVariant::Secondary)
                            .disabled(!has_task || is_running)
                            .on_click(cx.listener(|this, _, _, cx| {
                                // TODO: Start TestGen run
                                log::info!("TestGen clicked");
                            }))
                    )
            )
    }

    /// Render active run progress
    fn render_active_run(&self) -> impl IntoElement {
        if let Some(run) = &self.active_run {
            let progress = run.turns as f32 / run.max_turns as f32;
            let progress_width = (progress * 100.0).min(100.0);
            let bar_filled = (progress * 10.0) as usize;
            let bar_empty = 10 - bar_filled;
            let progress_bar = format!(
                "[{}{}]",
                "#".repeat(bar_filled),
                "-".repeat(bar_empty)
            );

            div()
                .px(px(12.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_xs()
                        .text_color(text::MUTED)
                        .font_weight(gpui::FontWeight::BOLD)
                        .mb(px(4.0))
                        .child("ACTIVE")
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(text::PRIMARY)
                        .child(format!("{} - Turn {}/{}", run.task_name, run.turns, run.max_turns))
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(text::SECONDARY)
                        .font_family(FONT_FAMILY)
                        .child(format!("{} {:.0}%", progress_bar, progress_width))
                )
        } else {
            div()
        }
    }

    /// Render recent runs list
    fn render_recent_runs(&self) -> impl IntoElement {
        div()
            .id("gym-recent-runs")
            .px(px(12.0))
            .py(px(8.0))
            .flex_1()
            .overflow_y_scroll()
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .font_weight(gpui::FontWeight::BOLD)
                    .mb(px(4.0))
                    .child("RECENT")
            )
            .when(self.recent_runs.is_empty(), |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(text::MUTED)
                        .child("No runs yet")
                )
            })
            .when(!self.recent_runs.is_empty(), |el| {
                el.children(self.recent_runs.iter().take(10).map(|run| {
                    let (symbol, color) = match run.outcome {
                        Some(TBRunOutcome::Success) => ("o", status::SUCCESS),
                        Some(TBRunOutcome::Failure) => ("x", status::ERROR),
                        Some(TBRunOutcome::Timeout) => ("t", status::WARNING),
                        Some(TBRunOutcome::Error) => ("!", status::ERROR),
                        Some(TBRunOutcome::Aborted) => ("-", text::MUTED),
                        None => ("*", text::SECONDARY),
                    };

                    let status_text = match run.outcome {
                        Some(TBRunOutcome::Success) => "PASS",
                        Some(TBRunOutcome::Failure) => "FAIL",
                        Some(TBRunOutcome::Timeout) => "TOUT",
                        Some(TBRunOutcome::Error) => "ERR",
                        Some(TBRunOutcome::Aborted) => "ABRT",
                        None => "RUN",
                    };

                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .py(px(2.0))
                        .child(
                            div()
                                .text_sm()
                                .text_color(color)
                                .font_family(FONT_FAMILY)
                                .child(symbol)
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(text::PRIMARY)
                                .flex_1()
                                .overflow_hidden()
                                .child(run.task_id.clone())
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(color)
                                .child(status_text)
                        )
                }))
            })
    }
}

impl Focusable for GymPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GymPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .bg(bg::APP)
            .font_family(FONT_FAMILY)
            .track_focus(&self.focus_handle)
            .child(self.render_header(cx))
            .child(self.render_task_selector(cx))
            .child(self.render_model_selector(cx))
            .child(self.render_actions(cx))
            .child(self.render_active_run())
            .child(self.render_recent_runs())
    }
}
