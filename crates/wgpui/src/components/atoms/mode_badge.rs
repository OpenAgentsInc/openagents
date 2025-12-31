use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Plan,
    Act,
    Code,
    Chat,
}

impl Mode {
    fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Plan => "PLAN",
            Mode::Act => "ACT",
            Mode::Code => "CODE",
            Mode::Chat => "CHAT",
        }
    }

    fn color(&self) -> Hsla {
        match self {
            Mode::Normal => theme::text::MUTED,
            Mode::Plan => theme::accent::PURPLE,
            Mode::Act => theme::status::SUCCESS,
            Mode::Code => theme::accent::PRIMARY,
            Mode::Chat => theme::accent::SECONDARY,
        }
    }
}

pub struct ModeBadge {
    id: Option<ComponentId>,
    mode: Mode,
    font_size: f32,
}

impl ModeBadge {
    pub fn new(mode: Mode) -> Self {
        Self {
            id: None,
            mode,
            font_size: theme::font_size::XS,
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

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}

impl Default for ModeBadge {
    fn default() -> Self {
        Self::new(Mode::default())
    }
}

impl Component for ModeBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;

        let label = self.mode.label();
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
                .with_background(self.mode.color().with_alpha(0.2))
                .with_border(self.mode.color(), 1.0),
        );

        let text_x = badge_bounds.origin.x + padding_h;
        let text_y = badge_bounds.origin.y + badge_height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout(
            label,
            Point::new(text_x, text_y),
            self.font_size,
            self.mode.color(),
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
        let label = self.mode.label();
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
    fn test_mode_badge_new() {
        let badge = ModeBadge::new(Mode::Plan);
        assert_eq!(badge.mode(), Mode::Plan);
    }

    #[test]
    fn test_mode_labels() {
        assert_eq!(Mode::Plan.label(), "PLAN");
        assert_eq!(Mode::Act.label(), "ACT");
        assert_eq!(Mode::Code.label(), "CODE");
    }

    #[test]
    fn test_set_mode() {
        let mut badge = ModeBadge::new(Mode::Normal);
        badge.set_mode(Mode::Act);
        assert_eq!(badge.mode(), Mode::Act);
    }
}
