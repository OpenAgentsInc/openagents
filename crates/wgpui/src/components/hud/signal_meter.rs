use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

/// A multi-bar vertical level indicator (like WiFi strength).
///
/// Legacy note: prefer [`viz::fill::Meter`] for the unified visualization grammar.
pub struct SignalMeter {
    id: Option<ComponentId>,
    bars: usize,
    gap: f32,
    level: f32,
    min_bar_height: f32,
    active_color: Hsla,
    inactive_color: Hsla,
}

impl SignalMeter {
    pub fn new() -> Self {
        Self {
            id: None,
            bars: 5,
            gap: 4.0,
            level: 0.6,
            min_bar_height: 0.2,
            active_color: Hsla::new(190.0, 0.8, 0.6, 0.9),
            inactive_color: Hsla::new(190.0, 0.3, 0.3, 0.4),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn bars(mut self, bars: usize) -> Self {
        self.bars = bars.max(1);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn level(mut self, level: f32) -> Self {
        self.level = level.clamp(0.0, 1.0);
        self
    }

    pub fn min_bar_height(mut self, min: f32) -> Self {
        self.min_bar_height = min.clamp(0.0, 1.0);
        self
    }

    pub fn active_color(mut self, color: Hsla) -> Self {
        self.active_color = color;
        self
    }

    pub fn inactive_color(mut self, color: Hsla) -> Self {
        self.inactive_color = color;
        self
    }

    pub fn set_level(&mut self, level: f32) {
        self.level = level.clamp(0.0, 1.0);
    }
}

impl Default for SignalMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SignalMeter {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let bars = self.bars.max(1);
        let gap = self.gap.max(0.0);
        let total_gap = gap * (bars.saturating_sub(1) as f32);
        let bar_width = if bars == 0 {
            bounds.size.width
        } else {
            ((bounds.size.width - total_gap) / bars as f32).max(0.0)
        };
        let min_height = self.min_bar_height.clamp(0.0, 1.0);

        for i in 0..bars {
            let factor = (i + 1) as f32 / bars as f32;
            let height = bounds.size.height * (min_height + (1.0 - min_height) * factor);
            let x = bounds.origin.x + i as f32 * (bar_width + gap);
            let y = bounds.origin.y + bounds.size.height - height;
            let is_active = self.level + f32::EPSILON >= factor;
            let color = if is_active {
                self.active_color
            } else {
                self.inactive_color
            };

            cx.scene
                .draw_quad(Quad::new(Bounds::new(x, y, bar_width, height)).with_background(color));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_meter_builder() {
        let meter = SignalMeter::new()
            .with_id(4)
            .bars(8)
            .gap(2.0)
            .level(0.9)
            .min_bar_height(0.3);

        assert_eq!(meter.id, Some(4));
        assert_eq!(meter.bars, 8);
        assert_eq!(meter.gap, 2.0);
        assert_eq!(meter.level, 0.9);
        assert_eq!(meter.min_bar_height, 0.3);
    }

    #[test]
    fn test_signal_meter_clamps() {
        let meter = SignalMeter::new()
            .bars(0)
            .gap(-4.0)
            .level(1.5)
            .min_bar_height(-1.0);

        assert_eq!(meter.bars, 1);
        assert_eq!(meter.gap, 0.0);
        assert_eq!(meter.level, 1.0);
        assert_eq!(meter.min_bar_height, 0.0);
    }
}
