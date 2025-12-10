/// Agent Node Component - GPUI Implementation
///
/// Factorio-inspired agent node representing a processing machine.
/// Shows status, metrics, connections, and allows drag-to-connect.
///
/// Features:
/// - Visual status indicators (pulsing when busy)
/// - Real-time metrics (jobs/hr, cost, queue depth)
/// - Connection points for input/output
/// - Drag to reposition
/// - Click to configure

use gpui::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum AgentNodeStatus {
    Busy,
    Idle,
    Error,
    Offline,
}

impl AgentNodeStatus {
    fn color(&self) -> Hsla {
        match self {
            AgentNodeStatus::Busy => rgb(0x00ff00),    // Green
            AgentNodeStatus::Idle => rgb(0x4a9eff),    // Blue
            AgentNodeStatus::Error => rgb(0xff0000),   // Red
            AgentNodeStatus::Offline => rgb(0x666666), // Gray
        }
    }

    fn label(&self) -> &str {
        match self {
            AgentNodeStatus::Busy => "BUSY",
            AgentNodeStatus::Idle => "IDLE",
            AgentNodeStatus::Error => "ERROR",
            AgentNodeStatus::Offline => "OFFLINE",
        }
    }
}

#[derive(Clone)]
pub struct AgentNodeMetrics {
    pub jobs_per_hour: f64,
    pub avg_cost: f64,
    pub success_rate: f64,
    pub queue_depth: u32,
    pub uptime_hours: f64,
}

#[derive(Clone)]
pub struct AgentNodeState {
    pub id: String,
    pub name: String,
    pub agent_type: String, // "Code Gen", "Test", "Review", etc.
    pub status: AgentNodeStatus,
    pub metrics: AgentNodeMetrics,
    pub position: (f32, f32), // Canvas position
    pub input_connections: Vec<String>,  // Agent IDs
    pub output_connections: Vec<String>, // Agent IDs
}

// ============================================================================
// View
// ============================================================================

pub struct AgentNodeView {
    state: Entity<AgentNodeState>,
    dragging: bool,
}

impl AgentNodeView {
    pub fn new(state_data: AgentNodeState, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| state_data);
        Self {
            state,
            dragging: false,
        }
    }

    /// Render status indicator (pulsing dot)
    fn render_status_indicator(&self, status: &AgentNodeStatus) -> Div {
        let color = status.color();

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(10.0))
                    .h(px(10.0))
                    .rounded_full()
                    .bg(color)
                    .shadow_md()
                    // TODO: Add pulse animation for Busy status
                    .when(status == &AgentNodeStatus::Busy, |div| {
                        // In real implementation, add pulsing animation
                        div.border_2().border_color(color)
                    })
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(color)
                    .font_weight(FontWeight::BOLD)
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_transform(TextTransform::Uppercase)
                    .text(status.label())
            )
    }

    /// Render metrics panel
    fn render_metrics(&self, metrics: &AgentNodeMetrics) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(rgba(0x0a0a0a, 0.6))
            .rounded(px(4.0))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0x888888))
                            .text("Jobs/hr:")
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0xffffff))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(format!("{:.1}", metrics.jobs_per_hour))
                    )
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0x888888))
                            .text("Avg Cost:")
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0xffffff))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(format!("${:.2}", metrics.avg_cost))
                    )
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0x888888))
                            .text("Success:")
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0x00ff00))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(format!("{:.1}%", metrics.success_rate))
                    )
            )
            .when(metrics.queue_depth > 0, |div| {
                div.child(
                    div()
                        .flex()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(rgb(0x888888))
                                .text("Queue:")
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(rgb(0xffa500))
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text(format!("[{}]", metrics.queue_depth))
                        )
                )
            })
    }

    /// Render connection point
    fn render_connection_point(&self, is_input: bool, cx: &mut Context<Self>) -> Div {
        let state = self.state.read(cx);
        let connections = if is_input {
            &state.input_connections
        } else {
            &state.output_connections
        };
        let has_connections = !connections.is_empty();

        div()
            .absolute()
            .w(px(12.0))
            .h(px(12.0))
            .when(is_input, |div| div.left(px(-6.0)))
            .when(!is_input, |div| div.right(px(-6.0)))
            .top(px(50.0))
            .rounded_full()
            .bg(if has_connections {
                rgb(0x00ff00)
            } else {
                rgb(0x444444)
            })
            .border_2()
            .border_color(rgb(0x1a1a1a))
            .cursor_pointer()
            .hover(|style| style.scale(1.2))
            // TODO: Add drag-to-connect functionality
    }
}

impl Render for AgentNodeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let status_color = state.status.color();

        div()
            .absolute()
            .left(px(state.position.0))
            .top(px(state.position.1))
            .w(px(200.0))
            .bg(rgba(0x1a1a1a, 0.95))
            .border_2()
            .border_color(status_color)
            .rounded(px(8.0))
            .shadow_2xl()
            .cursor_pointer()
            // TODO: Add drag handlers
            // Header
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .border_b_1()
                    .border_color(rgba(0xffffff, 0.1))
                    .bg(rgba(0x000000, 0.4))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::BOLD)
                                    .text(&state.name)
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(rgb(0x888888))
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text(&state.agent_type)
                            )
                    )
                    .child(self.render_status_indicator(&state.status))
            )
            // Metrics
            .child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .child(self.render_metrics(&state.metrics))
            )
            // Actions
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .border_t_1()
                    .border_color(rgba(0xffffff, 0.1))
                    .child(
                        div()
                            .flex_1()
                            .text_align(TextAlign::Center)
                            .px(px(8.0))
                            .py(px(6.0))
                            .text_size(px(10.0))
                            .text_color(rgb(0xffffff))
                            .bg(rgba(0x333333, 0.6))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgba(0x444444, 0.8)))
                            .text("Start")
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_align(TextAlign::Center)
                            .px(px(8.0))
                            .py(px(6.0))
                            .text_size(px(10.0))
                            .text_color(rgb(0xffffff))
                            .bg(rgba(0x333333, 0.6))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgba(0x444444, 0.8)))
                            .text("Config")
                    )
            )
            // Connection points
            .child(self.render_connection_point(true, cx))  // Input
            .child(self.render_connection_point(false, cx)) // Output
    }
}

// ============================================================================
// Usage Example
// ============================================================================

#[cfg(test)]
mod example {
    use super::*;

    fn example_usage() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| {
                    AgentNodeView::new(
                        AgentNodeState {
                            id: "agent-1".to_string(),
                            name: "Code Generator".to_string(),
                            agent_type: "Code Gen".to_string(),
                            status: AgentNodeStatus::Busy,
                            metrics: AgentNodeMetrics {
                                jobs_per_hour: 47.3,
                                avg_cost: 0.12,
                                success_rate: 98.4,
                                queue_depth: 3,
                                uptime_hours: 14.5,
                            },
                            position: (100.0, 100.0),
                            input_connections: vec!["agent-0".to_string()],
                            output_connections: vec!["agent-2".to_string(), "agent-3".to_string()],
                        },
                        cx,
                    )
                })
            })
            .ok();
        });
    }
}
