/// Minimap Component - GPUI Implementation
///
/// StarCraft-inspired minimap showing all agents at a glance.
/// Always visible in lower-left corner, colored dots by status.
///
/// Features:
/// - Click to jump to agent
/// - Persistent (can't be closed)
/// - Color-coded by status (green=busy, blue=idle, red=error, gray=offline)
/// - Hover shows agent name + status

use gpui::*;
use std::sync::Arc;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum AgentStatus {
    Busy,
    Idle,
    Error,
    Offline,
}

impl AgentStatus {
    fn color(&self) -> Hsla {
        match self {
            AgentStatus::Busy => rgb(0x00ff00),    // Bright green
            AgentStatus::Idle => rgb(0x4a9eff),    // Blue
            AgentStatus::Error => rgb(0xff0000),   // Bright red
            AgentStatus::Offline => rgb(0x666666), // Dark gray
        }
    }

    fn label(&self) -> &str {
        match self {
            AgentStatus::Busy => "BUSY",
            AgentStatus::Idle => "IDLE",
            AgentStatus::Error => "ERROR",
            AgentStatus::Offline => "OFFLINE",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentMinimapData {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub x: f32, // Normalized position (0.0 - 1.0)
    pub y: f32,
}

#[derive(Clone)]
pub struct MinimapState {
    pub agents: Vec<AgentMinimapData>,
    pub hovered_agent: Option<String>,
    pub expanded: bool,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            agents: Vec::new(),
            hovered_agent: None,
            expanded: false,
        }
    }
}

// ============================================================================
// View
// ============================================================================

pub struct MinimapView {
    state: Entity<MinimapState>,
}

impl MinimapView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| MinimapState::default());
        Self { state }
    }

    /// Add/update agent on minimap
    pub fn update_agent(&mut self, agent: AgentMinimapData, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            // Remove old entry if exists
            state.agents.retain(|a| a.id != agent.id);
            // Add new entry
            state.agents.push(agent);
            cx.notify();
        });
    }

    /// Remove agent from minimap
    pub fn remove_agent(&mut self, agent_id: &str, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.agents.retain(|a| a.id != agent_id);
            cx.notify();
        });
    }

    /// Render a single agent dot
    fn render_agent_dot(&self, agent: &AgentMinimapData, size: f32, cx: &mut Context<Self>) -> Div {
        let color = agent.status.color();
        let is_hovered = self.state.read(cx).hovered_agent.as_ref() == Some(&agent.id);

        let dot_size = if is_hovered { size * 1.5 } else { size };

        let agent_id = agent.id.clone();

        div()
            .absolute()
            .left(relative(agent.x))
            .top(relative(agent.y))
            .w(px(dot_size))
            .h(px(dot_size))
            .rounded_full()
            .bg(color)
            .shadow_md()
            .when(is_hovered, |div| {
                div.border_2().border_color(rgb(0xffffff))
            })
            .cursor_pointer()
            .on_mouse_enter(cx.listener(move |this, _event, _window, cx| {
                this.state.update(cx, |state, cx| {
                    state.hovered_agent = Some(agent_id.clone());
                    cx.notify();
                });
            }))
            .on_mouse_leave(cx.listener(|this, _event, _window, cx| {
                this.state.update(cx, |state, cx| {
                    state.hovered_agent = None;
                    cx.notify();
                });
            }))
            .on_click(cx.listener(move |_this, _event, _window, _cx| {
                // In real app, this would jump to agent detail view
                // Emit event or navigate
            }))
    }

    /// Render tooltip for hovered agent
    fn render_tooltip(&self, agent: &AgentMinimapData) -> Div {
        div()
            .absolute()
            .bottom_full()
            .left_0()
            .mb(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .bg(rgba(0x1a1a1a, 0.95))
            .border_1()
            .border_color(rgba(0xffffff, 0.2))
            .rounded(px(6.0))
            .shadow_lg()
            .z_index(100)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(14.0))
                            .text(&agent.name)
                    )
                    .child(
                        div()
                            .text_color(agent.status.color())
                            .text_size(px(12.0))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(agent.status.label())
                    )
            )
    }
}

impl Render for MinimapView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let size = if state.expanded { 200.0 } else { 160.0 };
        let dot_size = if state.expanded { 8.0 } else { 6.0 };

        // Render agent dots
        let mut dots_container = div()
            .relative()
            .w(px(size))
            .h(px(size));

        for agent in &state.agents {
            dots_container = dots_container.child(self.render_agent_dot(agent, dot_size, cx));
        }

        // Add tooltip if hovering
        if let Some(hovered_id) = &state.hovered_agent {
            if let Some(agent) = state.agents.iter().find(|a| &a.id == hovered_id) {
                dots_container = dots_container.child(self.render_tooltip(agent));
            }
        }

        // Main minimap container
        div()
            .fixed()
            .bottom(px(16.0))
            .left(px(16.0))
            .w(px(size))
            .h(px(size))
            .bg(rgba(0x0a0a0a, 0.8))
            .border_1()
            .border_color(rgba(0xffffff, 0.2))
            .rounded(px(8.0))
            .shadow_2xl()
            .overflow_hidden()
            // Grid background
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x1a1a1a, 0.3))
                    // In real implementation, render grid lines here
            )
            // Dots
            .child(dots_container)
            // Header
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .px(px(8.0))
                    .py(px(6.0))
                    .bg(rgba(0x000000, 0.6))
                    .border_b_1()
                    .border_color(rgba(0xffffff, 0.1))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_transform(TextTransform::Uppercase)
                                    .text("MINIMAP")
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x888888))
                                    .text_size(px(10.0))
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text(format!("{} agents", state.agents.len()))
                            )
                    )
            )
            // Expand/collapse button
            .child(
                div()
                    .absolute()
                    .bottom(px(4.0))
                    .right(px(4.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .bg(rgba(0x1a1a1a, 0.8))
                    .border_1()
                    .border_color(rgba(0xffffff, 0.2))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgba(0x2a2a2a, 0.8)))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.state.update(cx, |state, cx| {
                            state.expanded = !state.expanded;
                            cx.notify();
                        });
                    }))
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(10.0))
                            .text(if state.expanded { "−" } else { "+" })
                    )
            )
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
                let mut view = cx.new(MinimapView::new);

                // Add some sample agents
                view.update(cx, |view, cx| {
                    view.update_agent(
                        AgentMinimapData {
                            id: "agent-1".to_string(),
                            name: "Code Gen #1".to_string(),
                            status: AgentStatus::Busy,
                            x: 0.2,
                            y: 0.3,
                        },
                        cx,
                    );

                    view.update_agent(
                        AgentMinimapData {
                            id: "agent-2".to_string(),
                            name: "Test Runner #1".to_string(),
                            status: AgentStatus::Idle,
                            x: 0.6,
                            y: 0.5,
                        },
                        cx,
                    );

                    view.update_agent(
                        AgentMinimapData {
                            id: "agent-3".to_string(),
                            name: "Review Agent #1".to_string(),
                            status: AgentStatus::Error,
                            x: 0.8,
                            y: 0.7,
                        },
                        cx,
                    );
                });

                view
            })
            .ok();
        });
    }
}
