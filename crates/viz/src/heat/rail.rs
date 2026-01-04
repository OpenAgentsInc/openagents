//! Rail - 1D heat strip (timeline intensity)

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Heat, Palette, VizPrimitive};

/// A 1D heat strip for timeline intensity
pub struct Rail {
    data: Vec<f32>,
    palette: Palette,
    vertical: bool,
    cell_gap: f32,
}

impl Rail {
    pub fn new(segments: usize) -> Self {
        Self {
            data: vec![0.0; segments],
            palette: Palette::inferno(),
            vertical: false,
            cell_gap: 1.0,
        }
    }

    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    pub fn push(&mut self, value: f32) {
        // Shift left and add new value
        if !self.data.is_empty() {
            self.data.remove(0);
            self.data.push(value);
        }
    }

    pub fn set_segment(&mut self, index: usize, value: f32) {
        if index < self.data.len() {
            self.data[index] = value;
        }
    }
}

impl Component for Rail {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let count = self.data.len();
        if count == 0 {
            return;
        }

        if self.vertical {
            let cell_height =
                (bounds.size.height - self.cell_gap * (count - 1) as f32) / count as f32;

            for (i, &value) in self.data.iter().enumerate() {
                let color_arr = self.palette.sample(value);
                let color = Hsla::from_rgb(color_arr[0], color_arr[1], color_arr[2]).with_alpha(color_arr[3]);

                let cell_bounds = Bounds {
                    origin: Point {
                        x: bounds.origin.x,
                        y: bounds.origin.y + i as f32 * (cell_height + self.cell_gap),
                    },
                    size: Size {
                        width: bounds.size.width,
                        height: cell_height,
                    },
                };

                cx.scene.draw_quad(Quad::new(cell_bounds).with_background(color));
            }
        } else {
            let cell_width =
                (bounds.size.width - self.cell_gap * (count - 1) as f32) / count as f32;

            for (i, &value) in self.data.iter().enumerate() {
                let color_arr = self.palette.sample(value);
                let color = Hsla::from_rgb(color_arr[0], color_arr[1], color_arr[2]).with_alpha(color_arr[3]);

                let cell_bounds = Bounds {
                    origin: Point {
                        x: bounds.origin.x + i as f32 * (cell_width + self.cell_gap),
                        y: bounds.origin.y,
                    },
                    size: Size {
                        width: cell_width,
                        height: bounds.size.height,
                    },
                };

                cx.scene.draw_quad(Quad::new(cell_bounds).with_background(color));
            }
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.vertical {
            (Some(24.0), Some(100.0))
        } else {
            (Some(100.0), Some(24.0))
        }
    }
}

impl VizPrimitive for Rail {
    fn update(&mut self, value: f32) {
        self.push(value);
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.push(value);
    }
}

impl Heat for Rail {
    fn set_data(&mut self, data: &[f32]) {
        let len = data.len().min(self.data.len());
        self.data[..len].copy_from_slice(&data[..len]);
    }

    fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }
}
