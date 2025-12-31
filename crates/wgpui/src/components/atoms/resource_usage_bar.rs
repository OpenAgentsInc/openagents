//! Resource usage bar for autopilot system monitoring.
//!
//! Displays CPU, memory, or other resource utilization as a progress bar.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, Size, theme};

/// Resource type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceType {
    #[default]
    Memory,
    Cpu,
    Disk,
    Network,
}

impl ResourceType {
    pub fn label(&self) -> &'static str {
        match self {
            ResourceType::Memory => "MEM",
            ResourceType::Cpu => "CPU",
            ResourceType::Disk => "DISK",
            ResourceType::Network => "NET",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ResourceType::Memory => "▦",
            ResourceType::Cpu => "◆",
            ResourceType::Disk => "◉",
            ResourceType::Network => "↔",
        }
    }
}

/// Usage level thresholds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsageLevel {
    #[default]
    Normal,
    Warning,
    Critical,
}

impl UsageLevel {
    pub fn from_percent(pct: f32) -> Self {
        if pct >= 80.0 {
            UsageLevel::Critical
        } else if pct >= 60.0 {
            UsageLevel::Warning
        } else {
            UsageLevel::Normal
        }
    }

    pub fn color(&self) -> Hsla {
        // Hue is 0.0-1.0 range: green=0.333, gold=0.125, red=0.0
        match self {
            UsageLevel::Normal => Hsla::new(0.333, 0.7, 0.45, 1.0), // Green
            UsageLevel::Warning => Hsla::new(0.125, 0.8, 0.5, 1.0), // Gold
            UsageLevel::Critical => Hsla::new(0.0, 0.8, 0.55, 1.0), // Red
        }
    }
}

/// Bar showing resource usage
pub struct ResourceUsageBar {
    id: Option<ComponentId>,
    resource_type: ResourceType,
    percent: f32, // 0-100
    show_label: bool,
    show_value: bool,
    bar_width: f32,
}

impl ResourceUsageBar {
    pub fn new(resource_type: ResourceType, percent: f32) -> Self {
        Self {
            id: None,
            resource_type,
            percent: percent.clamp(0.0, 100.0),
            show_label: true,
            show_value: true,
            bar_width: 60.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = width;
        self
    }

    fn level(&self) -> UsageLevel {
        UsageLevel::from_percent(self.percent)
    }
}

impl Component for ResourceUsageBar {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let level = self.level();
        let color = level.color();
        let bg = Hsla::new(0.0, 0.0, 0.15, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(Hsla::new(0.0, 0.0, 0.3, 1.0), 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Icon
        let icon = self.resource_type.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);
        x += 14.0;

        // Label
        if self.show_label {
            let label = self.resource_type.label();
            let label_run = cx.text.layout(
                label,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);
            x += 32.0;
        }

        // Progress bar background
        let bar_height = 8.0;
        let bar_y = bounds.origin.y + (bounds.size.height - bar_height) / 2.0;
        let bar_bg = Hsla::new(0.0, 0.0, 0.2, 1.0);
        cx.scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(x, bar_y),
                size: Size::new(self.bar_width, bar_height),
            })
            .with_background(bar_bg),
        );

        // Progress bar fill
        let fill_width = self.bar_width * (self.percent / 100.0);
        if fill_width > 0.0 {
            cx.scene.draw_quad(
                Quad::new(Bounds {
                    origin: Point::new(x, bar_y),
                    size: Size::new(fill_width, bar_height),
                })
                .with_background(color),
            );
        }
        x += self.bar_width + 6.0;

        // Value
        if self.show_value {
            let value_text = format!("{:.0}%", self.percent);
            let value_run = cx.text.layout(
                &value_text,
                Point::new(x, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(value_run);
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
        let mut width = 12.0 + 14.0;
        if self.show_label {
            width += 32.0;
        }
        width += self.bar_width + 6.0;
        if self.show_value {
            width += 30.0;
        }
        (Some(width), Some(22.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_level() {
        assert_eq!(UsageLevel::from_percent(30.0), UsageLevel::Normal);
        assert_eq!(UsageLevel::from_percent(70.0), UsageLevel::Warning);
        assert_eq!(UsageLevel::from_percent(90.0), UsageLevel::Critical);
    }
}
