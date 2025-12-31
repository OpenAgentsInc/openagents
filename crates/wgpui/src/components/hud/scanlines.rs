use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

pub struct Scanlines {
    id: Option<ComponentId>,
    line_color: Hsla,
    scan_color: Hsla,
    line_width: f32,
    spacing: f32,
    scan_width: f32,
    scan_progress: f32,
    opacity: f32,
}

impl Scanlines {
    pub fn new() -> Self {
        Self {
            id: None,
            line_color: Hsla::new(190.0, 0.35, 0.55, 0.2),
            scan_color: Hsla::new(190.0, 0.7, 0.7, 0.35),
            line_width: 1.0,
            spacing: 12.0,
            scan_width: 24.0,
            scan_progress: 0.5,
            opacity: 1.0,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn line_color(mut self, color: Hsla) -> Self {
        self.line_color = color;
        self
    }

    pub fn scan_color(mut self, color: Hsla) -> Self {
        self.scan_color = color;
        self
    }

    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(1.0);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(4.0);
        self
    }

    pub fn scan_width(mut self, width: f32) -> Self {
        self.scan_width = width.max(self.line_width);
        self
    }

    pub fn scan_progress(mut self, progress: f32) -> Self {
        self.scan_progress = progress.clamp(0.0, 1.0);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn set_scan_progress(&mut self, progress: f32) {
        self.scan_progress = progress.clamp(0.0, 1.0);
    }
}

impl Default for Scanlines {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Scanlines {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.line_color.with_alpha(self.line_color.a * self.opacity);
        let line_width = self.line_width.max(1.0);
        let spacing = self.spacing.max(line_width + 1.0);
        let height = bounds.size.height.max(0.0);
        let max_y = bounds.origin.y + height;
        let mut y = bounds.origin.y;

        while y <= max_y {
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x,
                    y,
                    bounds.size.width,
                    line_width,
                ))
                .with_background(color),
            );
            y += spacing;
        }

        let scan_width = self.scan_width.min(height).max(line_width);
        if scan_width > 0.0 {
            let scan_color = self.scan_color.with_alpha(self.scan_color.a * self.opacity);
            let scan_y =
                bounds.origin.y + (height - scan_width) * self.scan_progress.clamp(0.0, 1.0);
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x,
                    scan_y,
                    bounds.size.width,
                    scan_width,
                ))
                .with_background(scan_color),
            );

            let core_alpha = (scan_color.a * 1.35).min(1.0);
            let core_y = scan_y + scan_width * 0.5 - line_width * 0.5;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x,
                    core_y,
                    bounds.size.width,
                    line_width,
                ))
                .with_background(scan_color.with_alpha(core_alpha)),
            );
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
    fn test_scanlines_builder() {
        let scanlines = Scanlines::new()
            .with_id(9)
            .line_width(2.0)
            .spacing(8.0)
            .scan_width(18.0)
            .scan_progress(0.8)
            .opacity(0.6);

        assert_eq!(scanlines.id, Some(9));
        assert_eq!(scanlines.line_width, 2.0);
        assert_eq!(scanlines.spacing, 8.0);
        assert_eq!(scanlines.scan_width, 18.0);
        assert_eq!(scanlines.scan_progress, 0.8);
        assert_eq!(scanlines.opacity, 0.6);
    }

    #[test]
    fn test_scanlines_clamps() {
        let scanlines = Scanlines::new()
            .line_width(0.2)
            .spacing(1.0)
            .scan_width(0.5)
            .scan_progress(-1.0)
            .opacity(1.5);

        assert_eq!(scanlines.line_width, 1.0);
        assert_eq!(scanlines.spacing, 4.0);
        assert_eq!(scanlines.scan_width, 1.0);
        assert_eq!(scanlines.scan_progress, 0.0);
        assert_eq!(scanlines.opacity, 1.0);
    }
}
