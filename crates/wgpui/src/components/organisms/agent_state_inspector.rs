//! Agent state inspector organism for viewing agent runtime state.
//!
//! Provides a detailed view of an agent's goals, tasks, memory, and actions.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Agent goal with progress
#[derive(Debug, Clone)]
pub struct AgentGoal {
    pub id: String,
    pub description: String,
    pub progress: f32, // 0.0 - 1.0
    pub status: AgentGoalStatus,
}

impl AgentGoal {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            progress: 0.0,
            status: AgentGoalStatus::Active,
        }
    }

    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    pub fn status(mut self, status: AgentGoalStatus) -> Self {
        self.status = status;
        self
    }
}

/// Goal status
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AgentGoalStatus {
    #[default]
    Active,
    Completed,
    Blocked,
    Paused,
}

/// Agent action in history
#[derive(Debug, Clone)]
pub struct AgentAction {
    pub tool: String,
    pub description: String,
    pub timestamp: String,
    pub success: bool,
}

impl AgentAction {
    pub fn new(tool: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            description: description.into(),
            timestamp: String::new(),
            success: true,
        }
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = ts.into();
        self
    }

    pub fn success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }
}

/// Inspector tab
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InspectorTab {
    #[default]
    Goals,
    Actions,
    Memory,
    Resources,
}

/// Agent state inspector organism
pub struct AgentStateInspector {
    id: Option<ComponentId>,
    agent_name: String,
    #[allow(dead_code)]
    agent_id: String,
    active_tab: InspectorTab,
    goals: Vec<AgentGoal>,
    actions: Vec<AgentAction>,
    memory_items: Vec<(String, String)>,
    resource_usage: ResourceUsage,
    scroll_offset: f32,
    tab_hovered: Option<InspectorTab>,
}

/// Resource usage metrics
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub tokens_used: u64,
    pub tokens_limit: u64,
    pub actions_count: u32,
    pub runtime_seconds: u64,
}

impl AgentStateInspector {
    pub fn new(agent_name: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            id: None,
            agent_name: agent_name.into(),
            agent_id: agent_id.into(),
            active_tab: InspectorTab::Goals,
            goals: Vec::new(),
            actions: Vec::new(),
            memory_items: Vec::new(),
            resource_usage: ResourceUsage::default(),
            scroll_offset: 0.0,
            tab_hovered: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn goals(mut self, goals: Vec<AgentGoal>) -> Self {
        self.goals = goals;
        self
    }

    pub fn actions(mut self, actions: Vec<AgentAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn memory(mut self, items: Vec<(String, String)>) -> Self {
        self.memory_items = items;
        self
    }

    pub fn resources(mut self, usage: ResourceUsage) -> Self {
        self.resource_usage = usage;
        self
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 50.0)
    }

    fn tabs_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 50.0,
            bounds.size.width,
            36.0,
        )
    }

    fn content_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 86.0,
            bounds.size.width,
            bounds.size.height - 86.0,
        )
    }

    fn tab_bounds(&self, bounds: &Bounds, tab: InspectorTab) -> Bounds {
        let tabs = self.tabs_bounds(bounds);
        let tab_width = tabs.size.width / 4.0;
        let idx = match tab {
            InspectorTab::Goals => 0,
            InspectorTab::Actions => 1,
            InspectorTab::Memory => 2,
            InspectorTab::Resources => 3,
        };
        Bounds::new(
            tabs.origin.x + idx as f32 * tab_width,
            tabs.origin.y,
            tab_width,
            36.0,
        )
    }

    fn tab_from_index(idx: usize) -> Option<InspectorTab> {
        match idx {
            0 => Some(InspectorTab::Goals),
            1 => Some(InspectorTab::Actions),
            2 => Some(InspectorTab::Memory),
            3 => Some(InspectorTab::Resources),
            _ => None,
        }
    }

    fn paint_goals(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let mut y = content.origin.y + padding - self.scroll_offset;

        if self.goals.is_empty() {
            let empty_run = cx.text.layout(
                "No active goals",
                Point::new(content.origin.x + padding, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);
            return;
        }

        for goal in &self.goals {
            let row_height = 60.0;

            // Goal description
            let desc_run = cx.text.layout(
                &goal.description,
                Point::new(content.origin.x + padding, y + 8.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(desc_run);

            // Status indicator
            let status_color = match goal.status {
                AgentGoalStatus::Active => Hsla::new(200.0, 0.7, 0.5, 1.0),
                AgentGoalStatus::Completed => Hsla::new(120.0, 0.6, 0.45, 1.0),
                AgentGoalStatus::Blocked => Hsla::new(0.0, 0.7, 0.5, 1.0),
                AgentGoalStatus::Paused => Hsla::new(45.0, 0.7, 0.5, 1.0),
            };
            let status_text = match goal.status {
                AgentGoalStatus::Active => "Active",
                AgentGoalStatus::Completed => "Done",
                AgentGoalStatus::Blocked => "Blocked",
                AgentGoalStatus::Paused => "Paused",
            };
            let status_run = cx.text.layout(
                status_text,
                Point::new(
                    content.origin.x + content.size.width - padding - 60.0,
                    y + 8.0,
                ),
                theme::font_size::XS,
                status_color,
            );
            cx.scene.draw_text(status_run);

            // Progress bar
            let bar_y = y + 32.0;
            let bar_width = content.size.width - padding * 2.0 - 80.0;
            let bar_bounds = Bounds::new(content.origin.x + padding, bar_y, bar_width, 8.0);
            cx.scene
                .draw_quad(Quad::new(bar_bounds).with_background(theme::bg::MUTED));

            let fill_width = bar_width * goal.progress;
            let fill_bounds = Bounds::new(content.origin.x + padding, bar_y, fill_width, 8.0);
            cx.scene
                .draw_quad(Quad::new(fill_bounds).with_background(status_color));

            // Progress percentage
            let pct = format!("{:.0}%", goal.progress * 100.0);
            let pct_run = cx.text.layout(
                &pct,
                Point::new(content.origin.x + padding + bar_width + 8.0, bar_y - 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(pct_run);

            y += row_height;
        }
    }

    fn paint_actions(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let mut y = content.origin.y + padding - self.scroll_offset;

        if self.actions.is_empty() {
            let empty_run = cx.text.layout(
                "No actions recorded",
                Point::new(content.origin.x + padding, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);
            return;
        }

        for action in &self.actions {
            let row_height = 44.0;

            // Tool name
            let tool_color = if action.success {
                theme::accent::PRIMARY
            } else {
                Hsla::new(0.0, 0.7, 0.5, 1.0)
            };
            let tool_run = cx.text.layout(
                &action.tool,
                Point::new(content.origin.x + padding, y + 4.0),
                theme::font_size::XS,
                tool_color,
            );
            cx.scene.draw_text(tool_run);

            // Description
            let desc_run = cx.text.layout(
                &action.description,
                Point::new(content.origin.x + padding, y + 20.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc_run);

            // Timestamp
            if !action.timestamp.is_empty() {
                let ts_run = cx.text.layout(
                    &action.timestamp,
                    Point::new(
                        content.origin.x + content.size.width - padding - 60.0,
                        y + 12.0,
                    ),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(ts_run);
            }

            y += row_height;
        }
    }

    fn paint_memory(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let mut y = content.origin.y + padding - self.scroll_offset;

        if self.memory_items.is_empty() {
            let empty_run = cx.text.layout(
                "No memory items",
                Point::new(content.origin.x + padding, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);
            return;
        }

        for (key, value) in &self.memory_items {
            let row_height = 36.0;

            // Key
            let key_run = cx.text.layout(
                key,
                Point::new(content.origin.x + padding, y + 10.0),
                theme::font_size::XS,
                theme::accent::PRIMARY,
            );
            cx.scene.draw_text(key_run);

            // Value (truncated)
            let display_value = if value.len() > 40 {
                format!("{}...", &value[..37])
            } else {
                value.clone()
            };
            let value_run = cx.text.layout(
                &display_value,
                Point::new(content.origin.x + padding + 120.0, y + 10.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(value_run);

            y += row_height;
        }
    }

    fn paint_resources(&self, content: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let y = content.origin.y + padding;

        // Token usage
        let tokens_label = cx.text.layout(
            "Tokens Used",
            Point::new(content.origin.x + padding, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(tokens_label);

        let tokens_value = format!(
            "{} / {}",
            self.resource_usage.tokens_used, self.resource_usage.tokens_limit
        );
        let tokens_run = cx.text.layout(
            &tokens_value,
            Point::new(content.origin.x + padding, y + 18.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(tokens_run);

        // Token progress bar
        let bar_y = y + 42.0;
        let bar_width = content.size.width - padding * 2.0;
        let bar_bounds = Bounds::new(content.origin.x + padding, bar_y, bar_width, 8.0);
        cx.scene
            .draw_quad(Quad::new(bar_bounds).with_background(theme::bg::MUTED));

        let usage_ratio = if self.resource_usage.tokens_limit > 0 {
            self.resource_usage.tokens_used as f32 / self.resource_usage.tokens_limit as f32
        } else {
            0.0
        };
        let fill_color = if usage_ratio > 0.9 {
            Hsla::new(0.0, 0.7, 0.5, 1.0)
        } else if usage_ratio > 0.7 {
            Hsla::new(45.0, 0.7, 0.5, 1.0)
        } else {
            Hsla::new(120.0, 0.6, 0.45, 1.0)
        };
        let fill_bounds = Bounds::new(
            content.origin.x + padding,
            bar_y,
            bar_width * usage_ratio.min(1.0),
            8.0,
        );
        cx.scene
            .draw_quad(Quad::new(fill_bounds).with_background(fill_color));

        // Actions count
        let actions_y = y + 70.0;
        let actions_label = cx.text.layout(
            "Actions Taken",
            Point::new(content.origin.x + padding, actions_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(actions_label);

        let actions_value = format!("{}", self.resource_usage.actions_count);
        let actions_run = cx.text.layout(
            &actions_value,
            Point::new(content.origin.x + padding, actions_y + 18.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(actions_run);

        // Runtime
        let runtime_y = y + 120.0;
        let runtime_label = cx.text.layout(
            "Runtime",
            Point::new(content.origin.x + padding, runtime_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(runtime_label);

        let mins = self.resource_usage.runtime_seconds / 60;
        let secs = self.resource_usage.runtime_seconds % 60;
        let runtime_value = format!("{}m {}s", mins, secs);
        let runtime_run = cx.text.layout(
            &runtime_value,
            Point::new(content.origin.x + padding, runtime_y + 18.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(runtime_run);
    }
}

impl Component for AgentStateInspector {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header = self.header_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(header).with_background(theme::bg::MUTED));

        // Title
        let title_run = cx.text.layout(
            "State Inspector",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 10.0),
            theme::font_size::BASE,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Agent name
        let agent_run = cx.text.layout(
            &self.agent_name,
            Point::new(bounds.origin.x + padding, bounds.origin.y + 30.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(agent_run);

        // Tabs
        let tabs = self.tabs_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(tabs).with_background(theme::bg::APP));

        let tab_labels = ["Goals", "Actions", "Memory", "Resources"];
        for (i, label) in tab_labels.iter().enumerate() {
            let tab = Self::tab_from_index(i).unwrap();
            let tab_bounds = self.tab_bounds(&bounds, tab);

            let is_active = self.active_tab == tab;
            let is_hovered = self.tab_hovered == Some(tab);

            if is_active {
                cx.scene
                    .draw_quad(Quad::new(tab_bounds).with_background(theme::bg::SURFACE));
            } else if is_hovered {
                cx.scene
                    .draw_quad(Quad::new(tab_bounds).with_background(theme::bg::HOVER));
            }

            let text_color = if is_active {
                theme::accent::PRIMARY
            } else {
                theme::text::MUTED
            };
            let label_run = cx.text.layout(
                label,
                Point::new(
                    tab_bounds.origin.x + tab_bounds.size.width / 2.0 - 24.0,
                    tab_bounds.origin.y + 10.0,
                ),
                theme::font_size::XS,
                text_color,
            );
            cx.scene.draw_text(label_run);
        }

        // Content area
        let content = self.content_bounds(&bounds);
        cx.scene.push_clip(content);

        match self.active_tab {
            InspectorTab::Goals => self.paint_goals(content, cx),
            InspectorTab::Actions => self.paint_actions(content, cx),
            InspectorTab::Memory => self.paint_memory(content, cx),
            InspectorTab::Resources => self.paint_resources(content, cx),
        }

        cx.scene.pop_clip();
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let old_hovered = self.tab_hovered;

                self.tab_hovered = None;
                for i in 0..4 {
                    let tab = Self::tab_from_index(i).unwrap();
                    if self.tab_bounds(&bounds, tab).contains(point) {
                        self.tab_hovered = Some(tab);
                        break;
                    }
                }

                if old_hovered != self.tab_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    for i in 0..4 {
                        let tab = Self::tab_from_index(i).unwrap();
                        if self.tab_bounds(&bounds, tab).contains(point) {
                            self.active_tab = tab;
                            self.scroll_offset = 0.0;
                            return EventResult::Handled;
                        }
                    }
                }
            }
            InputEvent::Scroll { dy, .. } => {
                let content = self.content_bounds(&bounds);
                let content_height = match self.active_tab {
                    InspectorTab::Goals => self.goals.len() as f32 * 60.0,
                    InspectorTab::Actions => self.actions.len() as f32 * 44.0,
                    InspectorTab::Memory => self.memory_items.len() as f32 * 36.0,
                    InspectorTab::Resources => 160.0,
                };
                let max_scroll = (content_height - content.size.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy).clamp(0.0, max_scroll);
                return EventResult::Handled;
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(400.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_inspector() {
        let inspector = AgentStateInspector::new("Test Agent", "agent-123")
            .goals(vec![
                AgentGoal::new("g1", "Complete task A")
                    .progress(0.5)
                    .status(AgentGoalStatus::Active),
            ])
            .actions(vec![
                AgentAction::new("Read", "Reading file.rs").timestamp("12:34"),
            ]);
        assert_eq!(inspector.agent_name, "Test Agent");
        assert_eq!(inspector.goals.len(), 1);
        assert_eq!(inspector.actions.len(), 1);
    }

    #[test]
    fn test_goal_status() {
        let goal = AgentGoal::new("g1", "Test goal")
            .progress(0.75)
            .status(AgentGoalStatus::Completed);
        assert_eq!(goal.progress, 0.75);
        assert_eq!(goal.status, AgentGoalStatus::Completed);
    }

    #[test]
    fn test_agent_state_resource_usage() {
        let usage = ResourceUsage {
            tokens_used: 1200,
            tokens_limit: 4096,
            actions_count: 7,
            runtime_seconds: 3600,
        };

        let inspector = AgentStateInspector::new("Test Agent", "agent-123").resources(usage);

        assert_eq!(inspector.resource_usage.tokens_used, 1200);
        assert_eq!(inspector.resource_usage.tokens_limit, 4096);
        assert_eq!(inspector.resource_usage.actions_count, 7);
        assert_eq!(inspector.resource_usage.runtime_seconds, 3600);
    }
}
