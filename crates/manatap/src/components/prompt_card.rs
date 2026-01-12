use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

const CARD_PADDING: f32 = 12.0;
const FONT_SIZE_LABEL: f32 = 10.0;
const FONT_SIZE_PROMPT: f32 = 14.0;
const CORNER_RADIUS: f32 = 6.0;

const PROMPT_BG: Hsla = Hsla {
    h: 220.0,
    s: 0.3,
    l: 0.15,
    a: 1.0,
};
const PROMPT_BORDER: Hsla = Hsla {
    h: 220.0,
    s: 0.4,
    l: 0.35,
    a: 1.0,
};
const TEXT_LABEL: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.5,
    a: 1.0,
};
const TEXT_PROMPT: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.95,
    a: 1.0,
};

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

    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text: &mut TextSystem, _scale: f32) {
        let x = bounds.origin.x;
        let y = bounds.origin.y;

        // Card background
        scene.draw_quad(
            Quad::new(bounds)
                .with_background(PROMPT_BG)
                .with_border(PROMPT_BORDER, 1.0)
                .with_corner_radius(CORNER_RADIUS),
        );

        // Label
        let label_run = text.layout_mono(
            "USER PROMPT",
            Point::new(x + CARD_PADDING, y + CARD_PADDING),
            FONT_SIZE_LABEL,
            TEXT_LABEL,
        );
        scene.draw_text(label_run);

        // Prompt text
        let prompt_run = text.layout_mono(
            &self.prompt,
            Point::new(x + CARD_PADDING, y + CARD_PADDING + 16.0),
            FONT_SIZE_PROMPT,
            TEXT_PROMPT,
        );
        scene.draw_text(prompt_run);
    }
}
