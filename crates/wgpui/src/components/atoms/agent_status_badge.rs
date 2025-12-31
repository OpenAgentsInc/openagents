//! Agent status badge for NIP-SA (Sovereign Agents).
//!
//! Displays the operational status of a sovereign agent.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Agent operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStatus {
    #[default]
    Offline,
    Online,
    Busy,
    Idle,
    Error,
}

impl AgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            AgentStatus::Offline => "Offline",
            AgentStatus::Online => "Online",
            AgentStatus::Busy => "Busy",
            AgentStatus::Idle => "Idle",
            AgentStatus::Error => "Error",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            AgentStatus::Offline => Hsla::new(0.0, 0.0, 0.4, 1.0), // Dark gray
            AgentStatus::Online => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            AgentStatus::Busy => Hsla::new(200.0, 0.8, 0.5, 1.0),  // Blue
            AgentStatus::Idle => Hsla::new(45.0, 0.8, 0.5, 1.0),   // Gold
            AgentStatus::Error => Hsla::new(0.0, 0.8, 0.5, 1.0),   // Red
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AgentStatus::Offline => "○",
            AgentStatus::Online => "●",
            AgentStatus::Busy => "◐",
            AgentStatus::Idle => "◑",
            AgentStatus::Error => "✕",
        }
    }
}

/// Agent type (threshold configuration)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentType {
    #[default]
    Human,
    Sovereign,
    Custodial,
}

impl AgentType {
    pub fn label(&self) -> &'static str {
        match self {
            AgentType::Human => "Human",
            AgentType::Sovereign => "Agent",
            AgentType::Custodial => "Custodial",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AgentType::Human => "👤",
            AgentType::Sovereign => "🤖",
            AgentType::Custodial => "🔒",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            AgentType::Human => Hsla::new(280.0, 0.6, 0.5, 1.0), // Purple
            AgentType::Sovereign => Hsla::new(140.0, 0.7, 0.5, 1.0), // Green
            AgentType::Custodial => Hsla::new(30.0, 0.7, 0.5, 1.0), // Orange
        }
    }
}

/// Badge displaying agent status
pub struct AgentStatusBadge {
    id: Option<ComponentId>,
    status: AgentStatus,
    agent_type: Option<AgentType>,
    show_dot: bool,
}

impl AgentStatusBadge {
    pub fn new(status: AgentStatus) -> Self {
        Self {
            id: None,
            status,
            agent_type: None,
            show_dot: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn agent_type(mut self, agent_type: AgentType) -> Self {
        self.agent_type = Some(agent_type);
        self
    }

    pub fn show_dot(mut self, show: bool) -> Self {
        self.show_dot = show;
        self
    }
}

impl Component for AgentStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let mut x = bounds.origin.x + padding;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;

        // Status dot
        if self.show_dot {
            let dot_size = 8.0;
            let dot_y = bounds.origin.y + (bounds.size.height - dot_size) / 2.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, dot_y, dot_size, dot_size)).with_background(color),
            );
            x += dot_size + 6.0;
        }

        // Agent type icon (if present)
        if let Some(agent_type) = self.agent_type {
            let icon = agent_type.icon();
            let icon_run = cx.text.layout(
                icon,
                Point::new(x, text_y - 1.0),
                theme::font_size::SM,
                agent_type.color(),
            );
            cx.scene.draw_text(icon_run);
            x += 16.0;
        }

        // Status label
        let label = self.status.label();
        let label_run = cx
            .text
            .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
        cx.scene.draw_text(label_run);
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let mut width = 12.0; // padding
        if self.show_dot {
            width += 14.0;
        }
        if self.agent_type.is_some() {
            width += 18.0;
        }
        width += self.status.label().len() as f32 * 6.5;
        (Some(width), Some(24.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status() {
        assert_eq!(AgentStatus::Online.label(), "Online");
        assert_eq!(AgentType::Sovereign.label(), "Agent");
    }
}
