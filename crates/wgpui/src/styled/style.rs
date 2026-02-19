use crate::Hsla;
use crate::layout::LayoutStyle;

#[derive(Clone, Debug)]
pub struct Style {
    pub layout: LayoutStyle,
    pub background: Option<Hsla>,
    pub border_color: Option<Hsla>,
    pub border_width: f32,
    pub corner_radius: f32,
    pub text_color: Option<Hsla>,
    pub font_size: Option<f32>,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            layout: LayoutStyle::default(),
            background: None,
            border_color: None,
            border_width: 0.0,
            corner_radius: 0.0,
            text_color: None,
            font_size: None,
        }
    }
}
