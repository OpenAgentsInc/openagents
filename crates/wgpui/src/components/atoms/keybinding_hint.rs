use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct KeybindingHint {
    id: Option<ComponentId>,
    keys: Vec<String>,
    font_size: f32,
    gap: f32,
}

impl KeybindingHint {
    pub fn new(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: None,
            keys: keys.into_iter().map(|k| k.into()).collect(),
            font_size: theme::font_size::XS,
            gap: 4.0,
        }
    }

    pub fn single(key: impl Into<String>) -> Self {
        Self::new([key])
    }

    pub fn combo(keys: &[&str]) -> Self {
        Self::new(keys.iter().copied())
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}

impl Default for KeybindingHint {
    fn default() -> Self {
        Self {
            id: None,
            keys: Vec::new(),
            font_size: theme::font_size::XS,
            gap: 4.0,
        }
    }
}

impl Component for KeybindingHint {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;
        let key_height = self.font_size + padding_v * 2.0;

        let mut x = bounds.origin.x;
        let y = bounds.origin.y + (bounds.size.height - key_height) / 2.0;

        for (i, key) in self.keys.iter().enumerate() {
            if i > 0 {
                let plus_width = self.font_size * 0.6;
                let plus_y = y + key_height * 0.5 - self.font_size * 0.55;
                let text_run = cx.text.layout_mono(
                    "+",
                    Point::new(x + self.gap / 2.0, plus_y),
                    self.font_size,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(text_run);
                x += plus_width + self.gap;
            }

            let text_width = key.len() as f32 * self.font_size * 0.6;
            let key_width = text_width + padding_h * 2.0;

            let key_bounds = Bounds::new(x, y, key_width, key_height);

            cx.scene.draw_quad(
                Quad::new(key_bounds)
                    .with_background(theme::bg::MUTED)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            let text_x = key_bounds.origin.x + padding_h;
            let text_y = key_bounds.origin.y + key_height * 0.5 - self.font_size * 0.55;

            let text_run = cx.text.layout_mono(
                key,
                Point::new(text_x, text_y),
                self.font_size,
                theme::text::MUTED,
            );
            cx.scene.draw_text(text_run);

            x += key_width + self.gap;
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
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;

        let mut total_width = 0.0;
        for (i, key) in self.keys.iter().enumerate() {
            if i > 0 {
                total_width += self.font_size * 0.6 + self.gap;
            }
            let text_width = key.len() as f32 * self.font_size * 0.6;
            total_width += text_width + padding_h * 2.0 + self.gap;
        }

        (Some(total_width), Some(self.font_size + padding_v * 2.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybinding_hint_single() {
        let hint = KeybindingHint::single("Esc");
        assert_eq!(hint.keys(), &["Esc"]);
    }

    #[test]
    fn test_keybinding_hint_combo() {
        let hint = KeybindingHint::combo(&["Ctrl", "Shift", "P"]);
        assert_eq!(hint.keys(), &["Ctrl", "Shift", "P"]);
    }

    #[test]
    fn test_keybinding_hint_builder() {
        let hint = KeybindingHint::single("Enter")
            .with_id(1)
            .font_size(14.0)
            .gap(8.0);

        assert_eq!(hint.id, Some(1));
        assert_eq!(hint.font_size, 14.0);
        assert_eq!(hint.gap, 8.0);
    }
}
