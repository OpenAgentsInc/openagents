//! Task Info Panel - Shows task info, backend selection, and current progress

use gpui::prelude::*;
use gpui::*;
use hillclimber::HillClimberBackend;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::types::{tasks, MechaSession, MechaStatus, MechaTask};

/// Action to switch backend
#[derive(Clone, Debug)]
pub struct SwitchBackend(pub HillClimberBackend);

/// Action to select a different task
#[derive(Clone, Debug)]
pub struct SelectTask(pub MechaTask);

impl EventEmitter<SwitchBackend> for TaskPanel {}
impl EventEmitter<SelectTask> for TaskPanel {}

/// Task info panel component
pub struct TaskPanel {
    session: MechaSession,
    focus_handle: FocusHandle,
    /// Available tasks (loaded from TB2)
    available_tasks: Vec<MechaTask>,
    /// Whether the task selector is expanded
    task_selector_expanded: bool,
}

impl TaskPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Load available TB2 tasks
        let available_tasks = tasks::all_tasks();

        Self {
            session: MechaSession::default(),
            focus_handle: cx.focus_handle(),
            available_tasks,
            task_selector_expanded: false,
        }
    }

    /// Toggle the task selector dropdown
    fn toggle_task_selector(&mut self, cx: &mut Context<Self>) {
        if self.session.status.is_busy() {
            return; // Don't allow task change while busy
        }
        self.task_selector_expanded = !self.task_selector_expanded;
        cx.notify();
    }

    /// Select a task from the dropdown
    fn select_task(&mut self, task: MechaTask, cx: &mut Context<Self>) {
        self.task_selector_expanded = false;
        cx.emit(SelectTask(task));
        cx.notify();
    }

    pub fn set_session(&mut self, session: MechaSession, cx: &mut Context<Self>) {
        self.session = session;
        cx.notify();
    }

    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Task name with target badge
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::BRIGHT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.session.task.name.clone()),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .bg(status::ERROR_BG)
                            .rounded(px(4.0))
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::ERROR)
                            .font_weight(FontWeight::BOLD)
                            .child("TARGET: 100%"),
                    ),
            )
            // Status indicator
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(self.render_status_badge())
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(format!("Turn {}/{}", self.session.turn, self.session.max_turns)),
                    ),
            )
    }

    fn render_status_badge(&self) -> impl IntoElement {
        let (bg_color, text_color) = match self.session.status {
            MechaStatus::Idle => (bg::ELEVATED, text::MUTED),
            MechaStatus::GeneratingTests | MechaStatus::Running => {
                (status::INFO_BG, status::RUNNING)
            }
            MechaStatus::WaitingInput => (status::WARNING_BG, status::WARNING),
            MechaStatus::Solved => (status::SUCCESS_BG, status::SUCCESS),
            MechaStatus::Failed => (status::ERROR_BG, status::ERROR),
        };

        div()
            .px(px(8.0))
            .py(px(3.0))
            .bg(bg_color)
            .rounded(px(4.0))
            .text_size(px(10.0))
            .font_family(FONT_FAMILY)
            .text_color(text_color)
            .font_weight(FontWeight::MEDIUM)
            .child(self.session.status.label())
    }

    fn render_backend_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_fm = self.session.backend == HillClimberBackend::FM;
        let is_busy = self.session.status.is_busy();

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Backend"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // FM button
                    .child({
                        let fm_selected = is_fm;
                        div()
                            .id("backend-fm")
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(if fm_selected { status::INFO_BG } else { bg::ELEVATED })
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(if fm_selected { status::INFO } else { border::SUBTLE })
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if fm_selected { status::INFO } else { text::MUTED })
                            .font_weight(if fm_selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                            .cursor_pointer()
                            .child("FM (Local)")
                            .when(!is_busy && !fm_selected, |el| {
                                el.on_click(cx.listener(|_, _event, _window, cx| {
                                    cx.emit(SwitchBackend(HillClimberBackend::FM));
                                }))
                            })
                    })
                    // CC button
                    .child({
                        let cc_selected = !is_fm;
                        div()
                            .id("backend-cc")
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(if cc_selected { status::INFO_BG } else { bg::ELEVATED })
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(if cc_selected { status::INFO } else { border::SUBTLE })
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if cc_selected { status::INFO } else { text::MUTED })
                            .font_weight(if cc_selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                            .cursor_pointer()
                            .child("CC (Claude)")
                            .when(!is_busy && !cc_selected, |el| {
                                el.on_click(cx.listener(|_, _event, _window, cx| {
                                    cx.emit(SwitchBackend(HillClimberBackend::CC));
                                }))
                            })
                    }),
            )
            // Backend description
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(if is_fm {
                        "Apple Foundation Model - local inference, no API cost"
                    } else {
                        "Claude Code SDK - cloud API, ~$0.01-0.10 per task"
                    }),
            )
    }

    fn render_progress(&self) -> impl IntoElement {
        let progress = self.session.best_progress;
        let progress_width = (progress * 180.0).max(4.0) as f32;
        let is_complete = progress >= 1.0;

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Large progress display
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(36.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_complete {
                                status::SUCCESS
                            } else if progress > 0.8 {
                                status::WARNING
                            } else {
                                text::PRIMARY
                            })
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{:.0}%", progress * 100.0)),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!(
                                "{}/{}",
                                self.session.tests_passed, self.session.tests_total
                            )),
                    ),
            )
            // Progress bar
            .child(
                div()
                    .w(px(180.0))
                    .h(px(8.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(progress_width))
                            .h_full()
                            .bg(if is_complete {
                                status::SUCCESS
                            } else {
                                status::INFO
                            })
                            .rounded(px(4.0)),
                    ),
            )
            // Cost (for CC backend)
            .when(self.session.backend == HillClimberBackend::CC && self.session.cost_usd > 0.0, |el| {
                el.child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::DISABLED)
                        .child(format!("Cost: ${:.4}", self.session.cost_usd)),
                )
            })
    }

    fn render_solution(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Current Solution"),
            )
            .child(
                div()
                    .p(px(10.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(border::SUBTLE)
                    .overflow_hidden()
                    .max_h(px(80.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(
                                self.session
                                    .solution
                                    .clone()
                                    .unwrap_or_else(|| "No solution yet".to_string()),
                            ),
                    ),
            )
    }

    fn render_task_description(&self) -> impl IntoElement {
        div()
            .id("task-description-scroll")
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Task Description"),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::SECONDARY)
                    .line_height(px(16.0))
                    .child(self.session.task.description.clone()),
            )
    }

    fn render_cwd(&self) -> impl IntoElement {
        // Show Docker environment for TB2 tasks
        let (cwd_display, env_label) = if self.session.task.is_tb2_task() {
            (
                "/app".to_string(),
                format!(
                    "Docker: {}",
                    self.session
                        .task
                        .docker_image
                        .as_deref()
                        .unwrap_or("unknown")
                ),
            )
        } else {
            let cwd = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "Unknown".to_string());
            (cwd, "Local".to_string())
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(16.0))
            .py(px(10.0))
            .bg(bg::ELEVATED)
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .font_weight(FontWeight::MEDIUM)
                            .child("Working Directory"),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(if self.session.task.is_tb2_task() {
                                status::INFO_BG
                            } else {
                                bg::SURFACE
                            })
                            .rounded(px(3.0))
                            .text_size(px(9.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if self.session.task.is_tb2_task() {
                                status::INFO
                            } else {
                                text::DISABLED
                            })
                            .child(env_label),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(status::INFO)
                    .font_weight(FontWeight::SEMIBOLD)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(cwd_display),
            )
    }

    fn render_task_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_busy = self.session.status.is_busy();
        let is_expanded = self.task_selector_expanded;
        let current_task_id = self.session.task.id.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Task"),
            )
            // Task selector button
            .child(
                div()
                    .id("task-selector")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(10.0))
                    .py(px(8.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(if is_expanded { status::INFO } else { border::SUBTLE })
                    .cursor_pointer()
                    .when(!is_busy, |el| {
                        el.on_click(cx.listener(|this, _event, _window, cx| {
                            this.toggle_task_selector(cx);
                        }))
                    })
                    .when(is_busy, |el| el.opacity(0.5))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(self.session.task.display_label()),
                            )
                            .when(self.session.task.category.is_some(), |el| {
                                el.child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::DISABLED)
                                        .child(
                                            self.session
                                                .task
                                                .category
                                                .clone()
                                                .unwrap_or_default(),
                                        ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(text::MUTED)
                            .child(if is_expanded { "▲" } else { "▼" }),
                    ),
            )
            // Dropdown list
            .when(is_expanded, |el| {
                el.child(
                    div()
                        .id("task-dropdown-list")
                        .flex()
                        .flex_col()
                        .bg(bg::SURFACE)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .rounded(px(4.0))
                        .max_h(px(200.0))
                        .overflow_y_scroll()
                        .children(self.available_tasks.iter().map(|task| {
                            let task_clone = task.clone();
                            let is_selected = task.id == current_task_id;

                            div()
                                .id(ElementId::from(SharedString::from(format!(
                                    "task-option-{}",
                                    task.id
                                ))))
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(10.0))
                                .py(px(8.0))
                                .bg(if is_selected { status::INFO_BG } else { bg::SURFACE })
                                .cursor_pointer()
                                .hover(|s| s.bg(bg::ELEVATED))
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.select_task(task_clone.clone(), cx);
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_family(FONT_FAMILY)
                                                .text_color(if is_selected {
                                                    status::INFO
                                                } else {
                                                    text::PRIMARY
                                                })
                                                .font_weight(if is_selected {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .child(task.display_label()),
                                        )
                                        .when(task.category.is_some(), |el| {
                                            el.child(
                                                div()
                                                    .text_size(px(9.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::DISABLED)
                                                    .child(task.category.clone().unwrap_or_default()),
                                            )
                                        }),
                                )
                                .when(is_selected, |el| {
                                    el.child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(status::SUCCESS)
                                            .child("✓")
                                    )
                                })
                        })),
                )
            })
    }
}

impl Focusable for TaskPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TaskPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::SURFACE)
            .child(self.render_cwd()) // Show CWD/Docker environment at top
            .child(self.render_task_selector(cx)) // Task selector dropdown
            .child(self.render_header())
            .child(self.render_backend_toggle(cx))
            .child(self.render_progress())
            .child(self.render_solution())
            .child(self.render_task_description())
    }
}
