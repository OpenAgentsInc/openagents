//! Parallel agent badge for multi-agent autopilot sessions.
//!
//! Shows the status of individual agents in a parallel execution context.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Parallel agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParallelAgentStatus {
    #[default]
    Idle,
    Initializing,
    Running,
    Waiting,
    Completed,
    Failed,
}

impl ParallelAgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ParallelAgentStatus::Idle => "Idle",
            ParallelAgentStatus::Initializing => "Init",
            ParallelAgentStatus::Running => "Running",
            ParallelAgentStatus::Waiting => "Waiting",
            ParallelAgentStatus::Completed => "Done",
            ParallelAgentStatus::Failed => "Failed",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ParallelAgentStatus::Idle => "○",
            ParallelAgentStatus::Initializing => "◔",
            ParallelAgentStatus::Running => "▶",
            ParallelAgentStatus::Waiting => "◐",
            ParallelAgentStatus::Completed => "✓",
            ParallelAgentStatus::Failed => "✕",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ParallelAgentStatus::Idle => Hsla::new(0.0, 0.0, 0.4, 1.0), // Dark gray
            ParallelAgentStatus::Initializing => Hsla::new(200.0, 0.6, 0.5, 1.0), // Muted blue
            ParallelAgentStatus::Running => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
            ParallelAgentStatus::Waiting => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            ParallelAgentStatus::Completed => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            ParallelAgentStatus::Failed => Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
        }
    }
}

/// Badge showing parallel agent status
pub struct ParallelAgentBadge {
    id: Option<ComponentId>,
    agent_index: u8,
    status: ParallelAgentStatus,
    current_task: Option<String>,
    compact: bool,
}

impl ParallelAgentBadge {
    pub fn new(agent_index: u8, status: ParallelAgentStatus) -> Self {
        Self {
            id: None,
            agent_index,
            status,
            current_task: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn current_task(mut self, task: impl Into<String>) -> Self {
        self.current_task = Some(task.into());
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for ParallelAgentBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Agent index badge
        let index_text = format!("A{}", self.agent_index);
        let index_run = cx.text.layout_mono(
            &index_text,
            Point::new(x, text_y),
            theme::font_size::XS,
            color,
        );
        cx.scene.draw_text(index_run);
        x += 18.0;

        // Status icon
        let icon = self.status.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);

        if !self.compact {
            x += 14.0;
            // Status label
            let label = self.status.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Current task (truncated)
            if let Some(task) = &self.current_task {
                let max_len = 20;
                let display_task = if task.len() > max_len {
                    format!("{}...", &task[..max_len - 3])
                } else {
                    task.clone()
                };
                let task_run = cx.text.layout_mono(
                    &display_task,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(task_run);
            }
        }
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
        if self.compact {
            (Some(50.0), Some(22.0))
        } else {
            let mut width = 12.0 + 18.0 + 14.0 + self.status.label().len() as f32 * 6.5;
            if self.current_task.is_some() {
                width += 140.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_agent_status() {
        assert_eq!(ParallelAgentStatus::Running.label(), "Running");
        assert_eq!(ParallelAgentStatus::Completed.label(), "Done");
    }
}
