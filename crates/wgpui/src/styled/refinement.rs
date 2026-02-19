use crate::Hsla;
use crate::layout::LayoutStyle;

use super::Style;

#[derive(Clone, Debug, Default)]
pub struct StyleRefinement {
    pub layout: LayoutStyle,
    pub background: Option<Hsla>,
    pub border_color: Option<Hsla>,
    pub border_width: Option<f32>,
    pub corner_radius: Option<f32>,
    pub text_color: Option<Hsla>,
    pub font_size: Option<f32>,
}

impl StyleRefinement {
    pub fn apply_to(&self, style: &mut Style) {
        style.layout = self.layout.clone();

        if let Some(background) = self.background {
            style.background = Some(background);
        }
        if let Some(border_color) = self.border_color {
            style.border_color = Some(border_color);
        }
        if let Some(border_width) = self.border_width {
            style.border_width = border_width;
        }
        if let Some(corner_radius) = self.corner_radius {
            style.corner_radius = corner_radius;
        }
        if let Some(text_color) = self.text_color {
            style.text_color = Some(text_color);
        }
        if let Some(font_size) = self.font_size {
            style.font_size = Some(font_size);
        }
    }

    pub fn resolve(&self) -> Style {
        let mut style = Style::default();
        self.apply_to(&mut style);
        style
    }
}
