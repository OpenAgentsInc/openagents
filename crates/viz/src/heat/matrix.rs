//! Matrix - 2D heatmap visualization

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Heat, Palette, VizPrimitive};

/// A 2D heatmap
pub struct Matrix {
    data: Vec<f32>,
    width: usize,
    height: usize,
    palette: Palette,
    cell_gap: f32,
}

impl Matrix {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: vec![0.0; width * height],
            width,
            height,
            palette: Palette::viridis(),
            cell_gap: 1.0,
        }
    }

    pub fn with_gap(mut self, gap: f32) -> Self {
        self.cell_gap = gap;
        self
    }

    /// Set data with dimensions (builder pattern)
    pub fn with_data(mut self, rows: usize, cols: usize, data: Vec<f32>) -> Self {
        self.height = rows;
        self.width = cols;
        self.data = data;
        self
    }

    /// Set palette (builder pattern)
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Create a 3-color gradient palette (low, mid, high) for wgpui Heatmap compatibility
    pub fn with_gradient(mut self, low: Hsla, mid: Option<Hsla>, high: Hsla) -> Self {
        let low_rgba = low.to_rgba();
        let high_rgba = high.to_rgba();
        let colors = if let Some(mid) = mid {
            let mid_rgba = mid.to_rgba();
            vec![low_rgba, mid_rgba, high_rgba]
        } else {
            vec![low_rgba, high_rgba]
        };
        self.palette = Palette { colors };
        self
    }

    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = value;
        }
    }
}

impl Component for Matrix {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        let cell_width = (bounds.size.width - self.cell_gap * (self.width - 1) as f32)
            / self.width as f32;
        let cell_height = (bounds.size.height - self.cell_gap * (self.height - 1) as f32)
            / self.height as f32;

        for y in 0..self.height {
            for x in 0..self.width {
                let value = self.data[y * self.width + x];
                let color_arr = self.palette.sample(value);
                let color = Hsla::from_rgb(color_arr[0], color_arr[1], color_arr[2]).with_alpha(color_arr[3]);

                let cell_bounds = Bounds {
                    origin: Point {
                        x: bounds.origin.x + x as f32 * (cell_width + self.cell_gap),
                        y: bounds.origin.y + y as f32 * (cell_height + self.cell_gap),
                    },
                    size: Size {
                        width: cell_width,
                        height: cell_height,
                    },
                };

                cx.scene.draw_quad(Quad::new(cell_bounds).with_background(color));
            }
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let w = self.width as f32 * 16.0 + (self.width - 1) as f32 * self.cell_gap;
        let h = self.height as f32 * 16.0 + (self.height - 1) as f32 * self.cell_gap;
        (Some(w), Some(h))
    }
}

impl VizPrimitive for Matrix {
    fn update(&mut self, _value: f32) {
        // Matrix updates via set_data
    }

    fn animate_to(&mut self, _value: f32, _duration_ms: u32) {
        // Matrix updates via set_data
    }
}

impl Heat for Matrix {
    fn set_data(&mut self, data: &[f32]) {
        let len = data.len().min(self.data.len());
        self.data[..len].copy_from_slice(&data[..len]);
    }

    fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }
}
