/// MC Tasks Component - GPUI Implementation
///
/// Displays MechaCoder ready tasks with assignment functionality.
/// Shows table with priority badges, type labels, and assign buttons.
///
/// Key conversions:
/// - Task list state → Vec<MCTask>
/// - Table rendering → nested div builders
/// - Async task loading → cx.spawn()
/// - Service injection → Arc<dyn SocketService>

use gpui::*;
use std::sync::Arc;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Debug)]
pub struct MCTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: u8,
    pub task_type: String,
    pub labels: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct MCTasksState {
    pub tasks: Vec<MCTask>,
    pub loading: bool,
    pub error: Option<String>,
    pub collapsed: bool,
    pub max_display: usize,
    pub assigning_id: Option<String>,
}

impl Default for MCTasksState {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            loading: false,
            error: None,
            collapsed: false,
            max_display: 20,
            assigning_id: None,
        }
    }
}

// ============================================================================
// Service Trait (placeholder)
// ============================================================================

/// Socket service for loading tasks and assigning to MechaCoder
pub trait SocketService: Send + Sync {
    fn load_ready_tasks(&self, limit: usize) -> impl std::future::Future<Output = Result<Vec<MCTask>, String>> + Send;
    fn assign_task_to_mc(&self, task_id: &str) -> impl std::future::Future<Output = Result<(), String>> + Send;
}

// ============================================================================
// View
// ============================================================================

pub struct MCTasksView {
    state: Entity<MCTasksState>,
    socket: Arc<dyn SocketService>,
}

impl MCTasksView {
    pub fn new(socket: Arc<dyn SocketService>, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| MCTasksState::default());
        Self { state, socket }
    }

    /// Get priority label (P0-P4)
    fn get_priority_label(priority: u8) -> String {
        format!("P{}", priority)
    }

    /// Get priority colors for badge
    fn get_priority_colors(priority: u8) -> (Hsla, Hsla, Hsla) {
        // Returns (bg, text, border)
        match priority {
            0 => (rgba(0x7f1d1d, 0.4), rgb(0xfca5a5), rgba(0xb91c1c, 0.5)), // red
            1 => (rgba(0x7c2d12, 0.4), rgb(0xfdba74), rgba(0xc2410c, 0.5)), // orange
            2 => (rgba(0x27272a, 0.5), rgb(0xe4e4e7), rgba(0x52525b, 0.5)), // zinc
            3 => (rgba(0x27272a, 0.3), rgb(0xd4d4d8), rgba(0x3f3f46, 0.4)), // zinc lighter
            4 => (rgba(0x18181b, 0.3), rgb(0xa1a1aa), rgba(0x3f3f46, 0.3)), // zinc lightest
            _ => (rgba(0x27272a, 0.4), rgb(0xd4d4d8), rgba(0x52525b, 0.4)), // default
        }
    }

    /// Get type badge color
    fn get_type_color(task_type: &str) -> Hsla {
        match task_type {
            "bug" => rgb(0xf87171),     // red-400
            "feature" => rgb(0x34d399), // emerald-400
            "task" => rgb(0x60a5fa),    // blue-400
            "epic" => rgb(0xa78bfa),    // violet-400
            "chore" => rgb(0xa1a1aa),   // zinc-400
            _ => rgb(0xa1a1aa),         // zinc-400
        }
    }

    /// Render header
    fn render_header(&self, state: &MCTasksState, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(rgba(0x27272a, 0.6))
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.update(cx, |state, cx| {
                    state.collapsed = !state.collapsed;
                    cx.notify();
                });
            }))
            .child(
                div()
                    .text_color(rgb(0xfafafa))
                    .font_weight(FontWeight::BOLD)
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_size(px(18.0))
                    .text(format!("Ready Tasks ({})", state.tasks.len()))
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0xa1a1aa))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(rgba(0x3f3f46, 0.5))
                            .cursor_pointer()
                            .hover(|style| {
                                style.border_color(rgba(0x52525b, 0.6))
                                    .text_color(rgb(0xe4e4e7))
                            })
                            .on_click(cx.listener(|this, event, _window, cx| {
                                event.stop_propagation();
                                this.load_tasks(cx);
                            }))
                            .text(if state.loading { "Loading..." } else { "Refresh" })
                    )
                    .child(
                        div()
                            .text_color(rgb(0x737373))
                            .text(if state.collapsed { "▼" } else { "▲" })
                    )
            )
    }

    /// Render task row
    fn render_task_row(&self, task: &MCTask, assigning_id: &Option<String>, cx: &mut Context<Self>) -> Div {
        let (prio_bg, prio_text, prio_border) = Self::get_priority_colors(task.priority);
        let prio_label = Self::get_priority_label(task.priority);
        let type_color = Self::get_type_color(&task.task_type);
        let labels_str = task.labels.iter().take(2).map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
        let is_assigning = assigning_id.as_ref().map(|id| id == &task.id).unwrap_or(false);

        let task_id = task.id.clone();

        div()
            .border_b_1()
            .border_color(rgba(0x27272a, 0.4))
            .hover(|style| style.bg(rgba(0x18181b, 0.3)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .py(px(8.0))
                    .px(px(12.0))
                    // Priority badge
                    .child(
                        div()
                            .w(px(48.0))
                            .child(
                                div()
                                    .inline_flex()
                                    .items_center()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .rounded(px(4.0))
                                    .border_1()
                                    .bg(prio_bg)
                                    .text_color(prio_text)
                                    .border_color(prio_border)
                                    .text(&prio_label)
                            )
                    )
                    // Task ID
                    .child(
                        div()
                            .w(px(96.0))
                            .px(px(12.0))
                            .child(
                                div()
                                    .text_color(rgb(0x737373))
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_size(px(10.0))
                                    .text(&task.id)
                            )
                    )
                    // Title
                    .child(
                        div()
                            .flex_1()
                            .px(px(12.0))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(rgb(0xfafafa))
                                    .text_size(px(14.0))
                                    .text(if task.title.len() > 50 {
                                        format!("{}...", &task.title[..50])
                                    } else {
                                        task.title.clone()
                                    })
                            )
                    )
                    // Type
                    .child(
                        div()
                            .w(px(80.0))
                            .px(px(12.0))
                            .child(
                                div()
                                    .text_color(type_color)
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_size(px(12.0))
                                    .text(&task.task_type)
                            )
                    )
                    // Labels
                    .child(
                        div()
                            .w(px(128.0))
                            .px(px(12.0))
                            .child(
                                div()
                                    .text_color(rgb(0xa1a1aa))
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_size(px(12.0))
                                    .text(&labels_str)
                            )
                    )
                    // Assign button
                    .child(
                        div()
                            .w(px(96.0))
                            .px(px(12.0))
                            .child(
                                div()
                                    .inline_flex()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .px(px(12.0))
                                    .py(px(4.0))
                                    .text_size(px(10.0))
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_transform(TextTransform::Uppercase)
                                    .rounded(px(4.0))
                                    .when(is_assigning, |div| {
                                        div.border_color(rgb(0x52525b))
                                            .text_color(rgb(0x737373))
                                            .bg(rgba(0x27272a, 0.4))
                                            .cursor(CursorStyle::Default)
                                    })
                                    .when(!is_assigning, |div| {
                                        div.border_color(rgb(0x3f3f46))
                                            .text_color(rgb(0xfafafa))
                                            .bg(rgba(0x18181b, 0.8))
                                            .cursor_pointer()
                                            .hover(|style| style.bg(rgba(0x18181b, 0.95)))
                                            .on_click(cx.listener(move |this, event, _window, cx| {
                                                event.stop_propagation();
                                                this.assign_task(&task_id, cx);
                                            }))
                                    })
                                    .text(if is_assigning { "Starting..." } else { "Assign" })
                            )
                    )
            )
    }

    /// Load tasks from service
    fn load_tasks(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.loading = true;
            state.error = None;
            cx.notify();
        });

        let socket = self.socket.clone();
        cx.spawn(|this, mut cx| async move {
            match socket.load_ready_tasks(50).await {
                Ok(tasks) => {
                    this.update(&mut cx, |this, cx| {
                        this.state.update(cx, |state, cx| {
                            state.tasks = tasks;
                            state.loading = false;
                            cx.notify();
                        });
                    }).ok();
                }
                Err(e) => {
                    this.update(&mut cx, |this, cx| {
                        this.state.update(cx, |state, cx| {
                            state.error = Some(e);
                            state.loading = false;
                            cx.notify();
                        });
                    }).ok();
                }
            }
        }).detach();
    }

    /// Assign task to MechaCoder
    fn assign_task(&mut self, task_id: &str, cx: &mut Context<Self>) {
        let task_id_owned = task_id.to_string();

        self.state.update(cx, |state, cx| {
            state.assigning_id = Some(task_id_owned.clone());
            cx.notify();
        });

        let socket = self.socket.clone();
        cx.spawn(|this, mut cx| async move {
            match socket.assign_task_to_mc(&task_id_owned).await {
                Ok(_) => {
                    this.update(&mut cx, |this, cx| {
                        this.state.update(cx, |state, cx| {
                            state.assigning_id = None;
                            state.tasks.retain(|t| t.id != task_id_owned);
                            cx.notify();
                        });
                    }).ok();
                }
                Err(e) => {
                    this.update(&mut cx, |this, cx| {
                        this.state.update(cx, |state, cx| {
                            state.assigning_id = None;
                            state.error = Some(e);
                            cx.notify();
                        });
                    }).ok();
                }
            }
        }).detach();
    }
}

impl Render for MCTasksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        let container = div()
            .rounded(px(16.0))
            .border_1()
            .border_color(rgba(0x27272a, 0.6))
            .bg(rgba(0x09090b, 0.8))
            .shadow_2xl()
            .max_h(relative(0.7)) // 70vh
            .overflow_hidden();

        let header = self.render_header(&state, cx);

        // Collapsed - just show header
        if state.collapsed {
            return container.child(header);
        }

        // Loading state
        if state.loading && state.tasks.is_empty() {
            return container
                .child(header)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py(px(32.0))
                        .child(
                            div()
                                .text_color(rgb(0xa1a1aa))
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_size(px(14.0))
                                .text("Loading ready tasks...")
                        )
                );
        }

        // Error state
        if let Some(error) = &state.error {
            if state.tasks.is_empty() {
                return container
                    .child(header)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .py(px(32.0))
                            .child(
                                div()
                                    .text_color(rgb(0xf87171))
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_size(px(14.0))
                                    .text(error)
                            )
                    );
            }
        }

        // Empty state
        if state.tasks.is_empty() {
            return container
                .child(header)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py(px(32.0))
                        .child(
                            div()
                                .text_color(rgb(0x737373))
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_size(px(14.0))
                                .text("No ready tasks found")
                        )
                );
        }

        // Task table
        let mut table = div()
            .overflow_x_auto()
            .max_h(px(700.0))
            .overflow_y_auto()
            .child(
                div()
                    .min_w_full()
                    // Table header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_color(rgb(0x737373))
                            .text_transform(TextTransform::Uppercase)
                            .text_size(px(9.0))
                            .bg(rgba(0x18181b, 0.4))
                            .py(px(8.0))
                            .child(div().w(px(48.0)).px(px(12.0)).text("PRI"))
                            .child(div().w(px(96.0)).px(px(12.0)).text("ID"))
                            .child(div().flex_1().px(px(12.0)).text("TITLE"))
                            .child(div().w(px(80.0)).px(px(12.0)).text("TYPE"))
                            .child(div().w(px(128.0)).px(px(12.0)).text("LABELS"))
                            .child(div().w(px(96.0)).px(px(12.0)).text("ACTION"))
                    )
            );

        // Task rows
        for task in state.tasks.iter().take(state.max_display) {
            table = table.child(self.render_task_row(task, &state.assigning_id, cx));
        }

        let mut result = container.child(header).child(table);

        // More tasks indicator
        if state.tasks.len() > state.max_display {
            result = result.child(
                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(rgba(0x27272a, 0.6))
                    .text_align(TextAlign::Center)
                    .text_size(px(12.0))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_color(rgb(0x737373))
                    .text(format!("+ {} more tasks...", state.tasks.len() - state.max_display))
            );
        }

        result
    }
}

// ============================================================================
// Usage Example
// ============================================================================

#[cfg(test)]
mod example {
    use super::*;

    /// Mock socket service for testing
    struct MockSocketService;

    impl SocketService for MockSocketService {
        async fn load_ready_tasks(&self, _limit: usize) -> Result<Vec<MCTask>, String> {
            Ok(vec![
                MCTask {
                    id: "task-1".to_string(),
                    title: "Fix authentication bug".to_string(),
                    description: "Users can't log in".to_string(),
                    status: "ready".to_string(),
                    priority: 0,
                    task_type: "bug".to_string(),
                    labels: vec!["auth".to_string(), "critical".to_string()],
                    created_at: "2025-12-09".to_string(),
                    updated_at: "2025-12-09".to_string(),
                },
            ])
        }

        async fn assign_task_to_mc(&self, _task_id: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn example_usage() {
        Application::new().run(|cx: &mut App| {
            let socket = Arc::new(MockSocketService);

            cx.open_window(
                WindowOptions::default(),
                move |_, cx| {
                    cx.new(|cx| MCTasksView::new(socket.clone(), cx))
                },
            ).ok();
        });
    }
}
