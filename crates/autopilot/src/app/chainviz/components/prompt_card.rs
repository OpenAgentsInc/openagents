use super::ChainTheme;
use wgpui::{Bounds, Point, Quad, Scene, TextSystem};

const CARD_PADDING: f32 = 12.0;
const FONT_SIZE_LABEL: f32 = 10.0;
const FONT_SIZE_PROMPT: f32 = 14.0;
const CORNER_RADIUS: f32 = 6.0;

pub struct PromptCard {
    prompt: String,
}

impl PromptCard {
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
        }
    }

    pub fn height(&self, _width: f32, _text: &mut TextSystem, _scale: f32) -> f32 {
        // Label + prompt + padding
        CARD_PADDING * 2.0 + 14.0 + 18.0
    }

    pub fn paint(
        &self,
        bounds: Bounds,
        scene: &mut Scene,
        text: &mut TextSystem,
        theme: &ChainTheme,
        _scale: f32,
    ) {
        let x = bounds.origin.x;
        let y = bounds.origin.y;

        // Card background
        scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme.prompt_bg)
                .with_border(theme.prompt_border, 1.0)
                .with_corner_radius(CORNER_RADIUS),
        );

        // Label
        let label_run = text.layout_mono(
            "USER PROMPT",
            Point::new(x + CARD_PADDING, y + CARD_PADDING),
            FONT_SIZE_LABEL,
            theme.prompt_label,
        );
        scene.draw_text(label_run);

        // Prompt text
        let prompt_run = text.layout_mono(
            &self.prompt,
            Point::new(x + CARD_PADDING, y + CARD_PADDING + 16.0),
            FONT_SIZE_PROMPT,
            theme.prompt_text,
        );
        scene.draw_text(prompt_run);
    }
}
