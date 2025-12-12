//! Gym panel - Terminal-Bench integration panel
//!
//! Bloomberg Terminal-style panel for running Terminal-Bench tasks:
//! - Task selector dropdown
//! - Action buttons (Run TB2, Run TestGen)
//! - Active run progress
//! - Recent runs history

use gpui::{
    deferred, div, prelude::*, px, App, Context, ElementId, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use harbor::StreamEvent;
use theme_oa::{bg, border, status, text, FONT_FAMILY};
use ui_oa::{Button, ButtonVariant};
use terminalbench::{TBTask, TBRunSummary, TBRunOutcome, TaskLoader, TBModelOption};

/// Events emitted by GymPanel
#[derive(Clone, Debug)]
pub enum GymPanelEvent {
    /// Start a TB2 run
    StartTB2Run {
        run_id: String,
        task: TBTask,
        model: TBModelOption,
    },
    /// TB2 stream event received
    TB2StreamEvent {
        run_id: String,
        event: StreamEvent,
    },
    /// TB2 run completed
    TB2RunComplete {
        run_id: String,
        success: bool,
        turns: u32,
        cost: Option<f64>,
        error: Option<String>,
    },
}

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
    /// Whether model dropdown is open
    model_dropdown_open: bool,
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
        log::info!("GymPanel: Loaded {} tasks", tasks.len());
        if tasks.is_empty() {
            log::warn!("GymPanel: No tasks loaded! Available suites: {:?}",
                task_loader.list_available_suites());
        }
        let selected_idx = if tasks.is_empty() { None } else { Some(0) };

        Self {
            focus_handle,
            task_loader,
            tasks,
            selected_task_idx: selected_idx,
            recent_runs: vec![],
            active_run: None,
            selected_model: TBModelOption::ClaudeSonnet45,
            model_dropdown_open: false,
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

    /// Toggle the model dropdown
    pub fn toggle_model_dropdown(&mut self, cx: &mut Context<Self>) {
        self.model_dropdown_open = !self.model_dropdown_open;
        cx.notify();
    }

    /// Select a specific model
    pub fn select_model(&mut self, model: TBModelOption, cx: &mut Context<Self>) {
        self.selected_model = model;
        self.model_dropdown_open = false;
        cx.notify();
    }

    /// Check if a run is active
    pub fn is_running(&self) -> bool {
        self.active_run.is_some()
    }

    /// Start a TB2 run for the selected task
    pub fn start_tb2_run(&mut self, cx: &mut Context<Self>) {
        if self.is_running() {
            log::warn!("Already running a task");
            return;
        }

        let task = match self.selected_task() {
            Some(t) => t.clone(),
            None => {
                log::warn!("No task selected");
                return;
            }
        };

        let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let model = self.selected_model;

        // Update active run state
        self.active_run = Some(ActiveRunState {
            run_id: run_id.clone(),
            task_id: task.id.clone(),
            task_name: task.name.clone(),
            turns: 0,
            max_turns: task.max_turns,
        });

        log::info!("Starting TB2 run: {} ({})", task.id, run_id);

        // Emit event to start the run
        cx.emit(GymPanelEvent::StartTB2Run {
            run_id,
            task,
            model,
        });

        cx.notify();
    }

    /// Handle a TB2 stream event
    pub fn handle_tb2_event(&mut self, run_id: &str, event: &StreamEvent, cx: &mut Context<Self>) {
        // Only process if this is our active run
        if self.active_run.as_ref().map(|r| r.run_id.as_str()) != Some(run_id) {
            return;
        }

        // Update turns from assistant events
        if let StreamEvent::Assistant { turn, .. } = event {
            if let Some(ref mut run) = self.active_run {
                run.turns = *turn;
            }
        }

        // Forward the event
        cx.emit(GymPanelEvent::TB2StreamEvent {
            run_id: run_id.to_string(),
            event: event.clone(),
        });

        cx.notify();
    }

    /// Handle TB2 run completion
    pub fn handle_tb2_complete(
        &mut self,
        run_id: &str,
        success: bool,
        turns: u32,
        cost: Option<f64>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Only process if this is our active run
        if self.active_run.as_ref().map(|r| r.run_id.as_str()) != Some(run_id) {
            return;
        }

        // Get task info before clearing active run
        let (task_id, task_name) = self.active_run.as_ref()
            .map(|r| (r.task_id.clone(), r.task_name.clone()))
            .unwrap_or_default();

        // Clear active run
        self.active_run = None;

        // Add to recent runs
        let summary = TBRunSummary {
            id: run_id.to_string(),
            task_id,
            task_name,
            status: terminalbench::TBRunStatus::Completed,
            outcome: Some(if success {
                TBRunOutcome::Success
            } else if error.is_some() {
                TBRunOutcome::Error
            } else {
                TBRunOutcome::Failure
            }),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: Some(chrono::Utc::now().to_rfc3339()),
            duration_ms: None,
            steps_count: turns,
            tokens_used: None,
        };
        self.recent_runs.insert(0, summary);

        // Emit completion event
        cx.emit(GymPanelEvent::TB2RunComplete {
            run_id: run_id.to_string(),
            success,
            turns,
            cost,
            error,
        });

        cx.notify();
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
    fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        // Use Ctrl on Linux, Cmd on macOS
        let close_hint = if cfg!(target_os = "macos") {
            "[Cmd+G to close]"
        } else {
            "[Ctrl+G to close]"
        };

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
                    .child(close_hint)
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
        let is_open = self.model_dropdown_open;
        let all_models = [
            TBModelOption::ClaudeSonnet45,
            TBModelOption::ClaudeHaiku45,
            TBModelOption::ClaudeOpus45,
            TBModelOption::Gpt4o,
            TBModelOption::Gpt4oMini,
            TBModelOption::AppleFM,
        ];

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
                    .relative()
                    // Trigger button
                    .child(
                        div()
                            .id("model-selector-trigger")
                            .px(px(8.0))
                            .py(px(6.0))
                            
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(if is_open { border::FOCUS } else { border::DEFAULT })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_model_dropdown(cx);
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
                                            .child(if is_open { "▲" } else { "▼" })
                                    )
                            )
                    )
                    // Dropdown options (when open) - use deferred for proper z-order
                    .when(is_open, |el| {
                        el.child(
                            deferred(
                                div()
                                    .absolute()
                                    .top(px(36.0))
                                    .left_0()
                                    .right_0()
                                    .py(px(4.0))
                                    
                                    .border_1()
                                    .border_color(border::DEFAULT)
                                    .bg(bg::ELEVATED)
                                    .occlude()
                                    .children(all_models.iter().map(|model| {
                                        let model = *model;
                                        let is_selected = self.selected_model == model;
                                        div()
                                            .id(ElementId::Name(model.id().into()))
                                            .px(px(8.0))
                                            .py(px(6.0))
                                            .text_sm()
                                            .cursor_pointer()
                                            .when(is_selected, |el| {
                                                el.bg(bg::HOVER)
                                                    .text_color(text::PRIMARY)
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                            })
                                            .when(!is_selected, |el| {
                                                el.text_color(text::SECONDARY)
                                                    .hover(|s| s.bg(bg::HOVER))
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_model(model, cx);
                                            }))
                                            .child(model.label())
                                    }))
                            ).with_priority(1)
                        )
                    })
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
                                this.start_tb2_run(cx);
                            }))
                    )
                    .child(
                        Button::new("TestGen")
                            .variant(ButtonVariant::Secondary)
                            .disabled(!has_task || is_running)
                            .on_click(cx.listener(|_this, _, _, _cx| {
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

impl EventEmitter<GymPanelEvent> for GymPanel {}

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
