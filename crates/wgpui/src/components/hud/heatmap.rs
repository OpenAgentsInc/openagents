use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

/// A 2D heatmap visualization.
///
/// # Deprecation
/// This component is deprecated. Use [`viz::heat::Matrix`] instead, which provides
/// the same functionality with a unified visualization grammar.
#[deprecated(since = "0.1.0", note = "Use viz::heat::Matrix instead")]
pub struct Heatmap {
    id: Option<ComponentId>,
    rows: usize,
    cols: usize,
    data: Vec<f32>,
    min: f32,
    max: f32,
    auto_range: bool,
    low_color: Hsla,
    mid_color: Option<Hsla>,
    high_color: Hsla,
    gap: f32,
}

impl Heatmap {
    pub fn new() -> Self {
        Self {
            id: None,
            rows: 0,
            cols: 0,
            data: Vec::new(),
            min: 0.0,
            max: 1.0,
            auto_range: true,
            low_color: Hsla::from_hex(0x0b1b2b),
            mid_color: Some(Hsla::from_hex(0x2ec4d6)),
            high_color: Hsla::from_hex(0xf5faff),
            gap: 1.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn data(mut self, rows: usize, cols: usize, data: Vec<f32>) -> Self {
        self.rows = rows;
        self.cols = cols;
        self.data = data;
        self
    }

    pub fn set_data(&mut self, rows: usize, cols: usize, data: Vec<f32>) {
        self.rows = rows;
        self.cols = cols;
        self.data = data;
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self.auto_range = false;
        self
    }

    pub fn auto_range(mut self, enabled: bool) -> Self {
        self.auto_range = enabled;
        self
    }

    pub fn low_color(mut self, color: Hsla) -> Self {
        self.low_color = color;
        self
    }

    pub fn mid_color(mut self, color: Option<Hsla>) -> Self {
        self.mid_color = color;
        self
    }

    pub fn high_color(mut self, color: Hsla) -> Self {
        self.high_color = color;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    fn color_for(&self, value: f32, min: f32, max: f32) -> Hsla {
        let span = (max - min).abs().max(1e-6);
        let mut t = ((value - min) / span).clamp(0.0, 1.0);
        if let Some(mid) = self.mid_color {
            if t < 0.5 {
                t *= 2.0;
                lerp_color(self.low_color, mid, t)
            } else {
                t = (t - 0.5) * 2.0;
                lerp_color(mid, self.high_color, t)
            }
        } else {
            lerp_color(self.low_color, self.high_color, t)
        }
    }
}

impl Default for Heatmap {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Heatmap {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let rows = self.rows.max(1);
        let cols = self.cols.max(1);

        if self.data.is_empty() || self.rows == 0 || self.cols == 0 {
            return;
        }

        let (min, max) = if self.auto_range {
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for v in &self.data {
                min = min.min(*v);
                max = max.max(*v);
            }
            if !min.is_finite() || !max.is_finite() {
                (0.0, 1.0)
            } else {
                (min, max)
            }
        } else {
            (self.min, self.max)
        };

        let gap = self.gap;
        let cell_w = (bounds.size.width / cols as f32).max(1.0);
        let cell_h = (bounds.size.height / rows as f32).max(1.0);
        let inset = gap * 0.5;

        for row in 0..rows {
            for col in 0..cols {
                let idx = row * cols + col;
                if idx >= self.data.len() {
                    continue;
                }
                let value = self.data[idx];
                let color = self.color_for(value, min, max);
                let x = bounds.origin.x + col as f32 * cell_w + inset;
                let y = bounds.origin.y + row as f32 * cell_h + inset;
                let w = (cell_w - gap).max(0.0);
                let h = (cell_h - gap).max(0.0);
                cx.scene
                    .draw_quad(Quad::new(Bounds::new(x, y, w, h)).with_background(color));
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
}

fn lerp_color(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    Hsla::new(
        a.h + (b.h - a.h) * t,
        a.s + (b.s - a.s) * t,
        a.l + (b.l - a.l) * t,
        a.a + (b.a - a.a) * t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_builder() {
        let map = Heatmap::new()
            .with_id(7)
            .data(2, 2, vec![0.1, 0.2, 0.3, 0.4])
            .gap(2.0)
            .range(0.0, 1.0);

        assert_eq!(map.id, Some(7));
        assert_eq!(map.rows, 2);
        assert_eq!(map.cols, 2);
        assert_eq!(map.data.len(), 4);
        assert_eq!(map.gap, 2.0);
    }
}
