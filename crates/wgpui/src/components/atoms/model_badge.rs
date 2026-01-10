use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Model {
    #[default]
    Claude,
    ClaudeSonnet,
    ClaudeOpus,
    ClaudeHaiku,
    Gpt4,
    Gpt4o,
    GptOss,
    Gemini,
    Local,
    Unknown,
}

impl Model {
    fn label(&self) -> &'static str {
        match self {
            Model::Claude => "Claude",
            Model::ClaudeSonnet => "Sonnet",
            Model::ClaudeOpus => "Opus",
            Model::ClaudeHaiku => "Haiku",
            Model::Gpt4 => "GPT-4",
            Model::Gpt4o => "GPT-4o",
            Model::GptOss => "GPT-OSS",
            Model::Gemini => "Gemini",
            Model::Local => "Local",
            Model::Unknown => "Unknown",
        }
    }

    fn color(&self) -> Hsla {
        match self {
            // Claude models: bright orange for high visibility
            Model::Claude | Model::ClaudeSonnet | Model::ClaudeOpus | Model::ClaudeHaiku => {
                Hsla::new(25.0, 0.9, 0.65, 1.0)
            }
            // GPT models: bright cyan/teal
            Model::Gpt4 | Model::Gpt4o | Model::GptOss => Hsla::new(160.0, 0.8, 0.55, 1.0),
            // Gemini: bright blue
            Model::Gemini => Hsla::new(220.0, 0.8, 0.65, 1.0),
            Model::Local => theme::text::SECONDARY,
            Model::Unknown => theme::text::MUTED,
        }
    }
}

pub struct ModelBadge {
    id: Option<ComponentId>,
    model: Model,
    font_size: f32,
    show_icon: bool,
}

impl ModelBadge {
    pub fn new(model: Model) -> Self {
        Self {
            id: None,
            model,
            font_size: theme::font_size::XS,
            show_icon: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    pub fn model(&self) -> Model {
        self.model
    }

    pub fn set_model(&mut self, model: Model) {
        self.model = model;
    }
}

impl Default for ModelBadge {
    fn default() -> Self {
        Self::new(Model::default())
    }
}

impl Component for ModelBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;

        let label = self.model.label();
        let text_width = label.len() as f32 * self.font_size * 0.6;
        let badge_width = text_width + padding_h * 2.0;
        let badge_height = self.font_size + padding_v * 2.0;

        let badge_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + (bounds.size.height - badge_height) / 2.0,
            badge_width,
            badge_height,
        );

        cx.scene.draw_quad(
            Quad::new(badge_bounds)
                .with_background(theme::bg::MUTED)
                .with_border(self.model.color(), 1.0),
        );

        let text_x = badge_bounds.origin.x + padding_h;
        let text_y = badge_bounds.origin.y + badge_height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout_mono(
            label,
            Point::new(text_x, text_y),
            self.font_size,
            self.model.color(),
        );
        cx.scene.draw_text(text_run);
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
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;
        let label = self.model.label();
        let text_width = label.len() as f32 * self.font_size * 0.6;
        (
            Some(text_width + padding_h * 2.0),
            Some(self.font_size + padding_v * 2.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_badge_new() {
        let badge = ModelBadge::new(Model::ClaudeSonnet);
        assert_eq!(badge.model(), Model::ClaudeSonnet);
    }

    #[test]
    fn test_model_labels() {
        assert_eq!(Model::ClaudeSonnet.label(), "Sonnet");
        assert_eq!(Model::Gpt4o.label(), "GPT-4o");
        assert_eq!(Model::GptOss.label(), "GPT-OSS");
    }

    #[test]
    fn test_set_model() {
        let mut badge = ModelBadge::new(Model::Claude);
        badge.set_model(Model::Gemini);
        assert_eq!(badge.model(), Model::Gemini);
    }
}
