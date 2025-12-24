use crate::geometry::Bounds;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Vertical,
    Horizontal,
    Both,
}

pub struct ScrollContainer {
    pub bounds: Bounds,
    pub content_size: (f32, f32),
    pub scroll_offset: (f32, f32),
    pub direction: ScrollDirection,
}

impl ScrollContainer {
    pub fn new(bounds: Bounds, direction: ScrollDirection) -> Self {
        Self {
            bounds,
            content_size: (0.0, 0.0),
            scroll_offset: (0.0, 0.0),
            direction,
        }
    }

    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        match self.direction {
            ScrollDirection::Vertical => {
                self.scroll_offset.1 += dy;
            }
            ScrollDirection::Horizontal => {
                self.scroll_offset.0 += dx;
            }
            ScrollDirection::Both => {
                self.scroll_offset.0 += dx;
                self.scroll_offset.1 += dy;
            }
        }
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max_x = (self.content_size.0 - self.bounds.width).max(0.0);
        let max_y = (self.content_size.1 - self.bounds.height).max(0.0);
        self.scroll_offset.0 = self.scroll_offset.0.clamp(0.0, max_x);
        self.scroll_offset.1 = self.scroll_offset.1.clamp(0.0, max_y);
    }
}
