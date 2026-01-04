//! Panel container with title

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Color, Point, Size};

/// A panel container with optional title
pub struct Panel {
    title: Option<String>,
    title_height: f32,
    bg_color: Color,
    border_color: Color,
    title_color: Color,
    padding: f32,
}

impl Panel {
    pub fn new() -> Self {
        Self {
            title: None,
            title_height: 20.0,
            bg_color: Color::from_rgba(0.08, 0.08, 0.08, 1.0),
            border_color: Color::from_rgba(0.2, 0.2, 0.2, 1.0),
            title_color: Color::from_rgba(0.6, 0.6, 0.6, 1.0),
            padding: 8.0,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_bg_color(mut self, color: Color) -> Self {
        self.bg_color = color;
        self
    }

    /// Get the content bounds (area inside the panel after title and padding)
    pub fn content_bounds(&self, bounds: Bounds) -> Bounds {
        let title_offset = if self.title.is_some() {
            self.title_height
        } else {
            0.0
        };

        Bounds {
            origin: Point {
                x: bounds.origin.x + self.padding,
                y: bounds.origin.y + self.padding + title_offset,
            },
            size: Size {
                width: bounds.size.width - self.padding * 2.0,
                height: bounds.size.height - self.padding * 2.0 - title_offset,
            },
        }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Panel {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.fill_rect(bounds, self.bg_color);

        // Border (top, left, bottom, right as thin rects)
        let border_width = 1.0;

        // Top border
        cx.scene.fill_rect(
            Bounds {
                origin: bounds.origin,
                size: Size {
                    width: bounds.size.width,
                    height: border_width,
                },
            },
            self.border_color,
        );

        // Left border
        cx.scene.fill_rect(
            Bounds {
                origin: bounds.origin,
                size: Size {
                    width: border_width,
                    height: bounds.size.height,
                },
            },
            self.border_color,
        );

        // Bottom border
        cx.scene.fill_rect(
            Bounds {
                origin: Point {
                    x: bounds.origin.x,
                    y: bounds.origin.y + bounds.size.height - border_width,
                },
                size: Size {
                    width: bounds.size.width,
                    height: border_width,
                },
            },
            self.border_color,
        );

        // Right border
        cx.scene.fill_rect(
            Bounds {
                origin: Point {
                    x: bounds.origin.x + bounds.size.width - border_width,
                    y: bounds.origin.y,
                },
                size: Size {
                    width: border_width,
                    height: bounds.size.height,
                },
            },
            self.border_color,
        );

        // Title (if present)
        if let Some(ref title) = self.title {
            let title_bounds = Bounds {
                origin: Point {
                    x: bounds.origin.x + self.padding,
                    y: bounds.origin.y + 4.0,
                },
                size: Size {
                    width: bounds.size.width - self.padding * 2.0,
                    height: self.title_height,
                },
            };

            cx.text.draw(
                cx.scene,
                title,
                title_bounds.origin,
                12.0,
                self.title_color,
            );

            // Title separator line
            cx.scene.fill_rect(
                Bounds {
                    origin: Point {
                        x: bounds.origin.x + self.padding,
                        y: bounds.origin.y + self.title_height,
                    },
                    size: Size {
                        width: bounds.size.width - self.padding * 2.0,
                        height: 1.0,
                    },
                },
                self.border_color,
            );
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}
