use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffType {
    #[default]
    Unified,
    SideBySide,
}

pub struct DiffHeader {
    id: Option<ComponentId>,
    file_path: String,
    additions: usize,
    deletions: usize,
    diff_type: DiffType,
}

impl DiffHeader {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            id: None,
            file_path: file_path.into(),
            additions: 0,
            deletions: 0,
            diff_type: DiffType::Unified,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn additions(mut self, count: usize) -> Self {
        self.additions = count;
        self
    }

    pub fn deletions(mut self, count: usize) -> Self {
        self.deletions = count;
        self
    }

    pub fn diff_type(mut self, dt: DiffType) -> Self {
        self.diff_type = dt;
        self
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn get_additions(&self) -> usize {
        self.additions
    }

    pub fn get_deletions(&self) -> usize {
        self.deletions
    }
}

impl Default for DiffHeader {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for DiffHeader {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::SM;
        let mut x = bounds.origin.x + padding;
        let font_size = theme::font_size::SM;
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;

        let text_run = cx.text.layout(
            &self.file_path,
            Point::new(x, text_y),
            font_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(text_run);
        x += self.file_path.len() as f32 * font_size * 0.6 + theme::spacing::MD;

        if self.additions > 0 {
            let add_str = format!("+{}", self.additions);
            let text_run = cx.text.layout(
                &add_str,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::status::SUCCESS,
            );
            cx.scene.draw_text(text_run);
            x += add_str.len() as f32 * theme::font_size::XS * 0.6 + theme::spacing::XS;
        }

        if self.deletions > 0 {
            let del_str = format!("-{}", self.deletions);
            let text_run = cx.text.layout(
                &del_str,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::status::ERROR,
            );
            cx.scene.draw_text(text_run);
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
        (None, Some(28.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_header_new() {
        let header = DiffHeader::new("src/lib.rs");
        assert_eq!(header.file_path(), "src/lib.rs");
    }

    #[test]
    fn test_diff_header_builder() {
        let header = DiffHeader::new("main.rs")
            .with_id(1)
            .additions(10)
            .deletions(5)
            .diff_type(DiffType::SideBySide);

        assert_eq!(header.id, Some(1));
        assert_eq!(header.get_additions(), 10);
        assert_eq!(header.get_deletions(), 5);
        assert_eq!(header.diff_type, DiffType::SideBySide);
    }
}
