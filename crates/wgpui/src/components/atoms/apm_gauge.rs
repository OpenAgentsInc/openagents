//! APM (Actions Per Minute) gauge for autopilot metrics.
//!
//! Displays the current actions-per-minute rate with visual indicators.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// APM level thresholds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApmLevel {
    #[default]
    Idle,
    Low,     // < 10 APM
    Normal,  // 10-30 APM
    High,    // 30-60 APM
    Intense, // > 60 APM
}

impl ApmLevel {
    pub fn from_apm(apm: f32) -> Self {
        if apm < 1.0 {
            ApmLevel::Idle
        } else if apm < 10.0 {
            ApmLevel::Low
        } else if apm < 30.0 {
            ApmLevel::Normal
        } else if apm < 60.0 {
            ApmLevel::High
        } else {
            ApmLevel::Intense
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ApmLevel::Idle => "Idle",
            ApmLevel::Low => "Low",
            ApmLevel::Normal => "Normal",
            ApmLevel::High => "High",
            ApmLevel::Intense => "Intense",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ApmLevel::Idle => "○",
            ApmLevel::Low => "▁",
            ApmLevel::Normal => "▃",
            ApmLevel::High => "▅",
            ApmLevel::Intense => "▇",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ApmLevel::Idle => Hsla::new(0.0, 0.0, 0.4, 1.0), // Dark gray
            ApmLevel::Low => Hsla::new(200.0, 0.5, 0.5, 1.0), // Muted blue
            ApmLevel::Normal => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            ApmLevel::High => Hsla::new(45.0, 0.8, 0.5, 1.0), // Gold
            ApmLevel::Intense => Hsla::new(0.0, 0.8, 0.55, 1.0), // Red
        }
    }
}

/// Gauge showing APM rate
pub struct ApmGauge {
    id: Option<ComponentId>,
    apm: f32,
    show_bars: bool,
    show_value: bool,
    compact: bool,
}

impl ApmGauge {
    pub fn new(apm: f32) -> Self {
        Self {
            id: None,
            apm,
            show_bars: true,
            show_value: true,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_bars(mut self, show: bool) -> Self {
        self.show_bars = show;
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    fn level(&self) -> ApmLevel {
        ApmLevel::from_apm(self.apm)
    }
}

impl Component for ApmGauge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let level = self.level();
        let color = level.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        if self.compact {
            // Just show bars and value
            if self.show_bars {
                let bars = self.draw_bars();
                let bars_run =
                    cx.text
                        .layout(&bars, Point::new(x, text_y), theme::font_size::SM, color);
                cx.scene.draw_text(bars_run);
                x += 30.0;
            }
            if self.show_value {
                let apm_text = format!("{:.0}", self.apm);
                let apm_run = cx.text.layout(
                    &apm_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    color,
                );
                cx.scene.draw_text(apm_run);
            }
        } else {
            // APM label
            let label = "APM";
            let label_run = cx.text.layout(
                label,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);
            x += 28.0;

            // Bars visualization
            if self.show_bars {
                let bars = self.draw_bars();
                let bars_run =
                    cx.text
                        .layout(&bars, Point::new(x, text_y), theme::font_size::SM, color);
                cx.scene.draw_text(bars_run);
                x += 32.0;
            }

            // Value
            if self.show_value {
                let apm_text = format!("{:.1}", self.apm);
                let apm_run = cx.text.layout(
                    &apm_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(apm_run);
                x += apm_text.len() as f32 * 6.5 + 6.0;
            }

            // Level label
            let level_label = level.label();
            let level_run = cx.text.layout(
                level_label,
                Point::new(x, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(level_run);
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
            (Some(70.0), Some(22.0))
        } else {
            (Some(140.0), Some(22.0))
        }
    }
}

impl ApmGauge {
    /// Draw bar visualization based on APM level
    fn draw_bars(&self) -> String {
        let filled = match self.level() {
            ApmLevel::Idle => 0,
            ApmLevel::Low => 1,
            ApmLevel::Normal => 2,
            ApmLevel::High => 3,
            ApmLevel::Intense => 4,
        };
        let mut bars = String::new();
        for i in 0..4 {
            if i < filled {
                bars.push('▮');
            } else {
                bars.push('▯');
            }
        }
        bars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apm_level() {
        assert_eq!(ApmLevel::from_apm(0.0), ApmLevel::Idle);
        assert_eq!(ApmLevel::from_apm(5.0), ApmLevel::Low);
        assert_eq!(ApmLevel::from_apm(20.0), ApmLevel::Normal);
        assert_eq!(ApmLevel::from_apm(45.0), ApmLevel::High);
        assert_eq!(ApmLevel::from_apm(100.0), ApmLevel::Intense);
    }

    #[test]
    fn test_bars() {
        let gauge = ApmGauge::new(25.0);
        assert_eq!(gauge.draw_bars(), "▮▮▯▯");
    }
}
