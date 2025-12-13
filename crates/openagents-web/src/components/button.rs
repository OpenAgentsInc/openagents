//! Button component

use super::{Bounds, Component, GpuQuad, Primitive, RenderContext, TextRun};
use crate::theme::{self, Color};

/// Button visual style
#[derive(Clone, Copy, Debug, Default)]
pub enum ButtonStyle {
    /// White background, black text - primary action
    #[default]
    Default,
    /// Gray background - secondary action
    Secondary,
    /// Transparent background - tertiary action
    Ghost,
    /// Border only - alternative styling
    Outline,
    /// Red background - destructive action
    Destructive,
}

/// Button size
#[derive(Clone, Copy, Debug, Default)]
pub enum ButtonSize {
    /// 32px height
    Large,
    /// 28px height
    Medium,
    /// 22px height
    #[default]
    Default,
    /// 18px height
    Compact,
}

impl ButtonSize {
    pub fn height(&self) -> f32 {
        match self {
            ButtonSize::Large => 32.0,
            ButtonSize::Medium => 28.0,
            ButtonSize::Default => 22.0,
            ButtonSize::Compact => 18.0,
        }
    }

    pub fn font_size(&self) -> f32 {
        match self {
            ButtonSize::Large => 12.0,
            ButtonSize::Medium => 11.0,
            ButtonSize::Default => 11.0,
            ButtonSize::Compact => 10.0,
        }
    }

    pub fn padding_x(&self) -> f32 {
        match self {
            ButtonSize::Large => 12.0,
            ButtonSize::Medium => 10.0,
            ButtonSize::Default => 8.0,
            ButtonSize::Compact => 6.0,
        }
    }
}

/// Button component
pub struct Button {
    pub label: String,
    pub style: ButtonStyle,
    pub size: ButtonSize,
    pub origin: [f32; 2],
    pub width: Option<f32>,
    pub disabled: bool,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            style: ButtonStyle::Default,
            size: ButtonSize::Default,
            origin: [0.0, 0.0],
            width: None,
            disabled: false,
        }
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.origin = [x, y];
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn colors(&self) -> (Color, Color, Option<Color>) {
        if self.disabled {
            return (
                theme::bg::ELEVATED,
                theme::text::DISABLED,
                Some(theme::border::DEFAULT),
            );
        }

        match self.style {
            ButtonStyle::Default => (
                theme::ui::button::DEFAULT_BG,
                theme::ui::button::DEFAULT_TEXT,
                None,
            ),
            ButtonStyle::Secondary => (
                theme::ui::button::SECONDARY_BG,
                theme::ui::button::SECONDARY_TEXT,
                None,
            ),
            ButtonStyle::Ghost => (
                theme::ui::button::GHOST_BG,
                theme::ui::button::GHOST_TEXT,
                None,
            ),
            ButtonStyle::Outline => (
                theme::ui::button::OUTLINE_BG,
                theme::ui::button::OUTLINE_TEXT,
                Some(theme::ui::button::OUTLINE_BORDER),
            ),
            ButtonStyle::Destructive => (
                theme::ui::button::DESTRUCTIVE_BG,
                theme::ui::button::DESTRUCTIVE_TEXT,
                None,
            ),
        }
    }
}

impl Component for Button {
    fn render(&self, ctx: &mut RenderContext) -> Vec<Primitive> {
        let (bg_color, text_color, border_color) = self.colors();
        let height = self.size.height();
        let font_size = self.size.font_size();
        let padding_x = self.size.padding_x();

        // Measure text to calculate width
        let text_width = ctx.text_system.measure(&self.label, font_size);
        let width = self.width.unwrap_or(text_width + padding_x * 2.0);
        let corner_radius = 4.0;

        let mut primitives = Vec::new();

        // Background quad
        let mut quad = GpuQuad::new(self.origin[0], self.origin[1], width, height)
            .bg(bg_color)
            .radius(corner_radius);

        if let Some(border) = border_color {
            quad = quad.border(border, 1.0);
        }

        primitives.push(Primitive::Quad(quad));

        // Text label (centered)
        let text_x = self.origin[0] + (width - text_width) / 2.0;
        let text_y = self.origin[1] + (height - font_size) / 2.0 + font_size * 0.8;

        primitives.push(Primitive::Text(TextRun {
            text: self.label.clone(),
            position: [text_x, text_y],
            size: font_size,
            color: text_color,
        }));

        primitives
    }

    fn bounds(&self) -> Bounds {
        let height = self.size.height();
        Bounds {
            origin: self.origin,
            size: [self.width.unwrap_or(100.0), height],
        }
    }
}
