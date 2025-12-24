use crate::color::Hsla;
use crate::geometry::Point;
use crate::scene::TextRun;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
    Bold,
    BoldItalic,
}

pub struct TextSystem {
    _private: (),
}

impl TextSystem {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn layout(
        &self,
        _text: &str,
        _origin: Point,
        _font_size: f32,
        _color: Hsla,
    ) -> TextRun {
        TextRun { glyphs: Vec::new() }
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}
