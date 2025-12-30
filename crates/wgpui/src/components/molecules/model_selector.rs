use crate::components::atoms::{Model, ModelBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

pub struct ModelSelector {
    id: Option<ComponentId>,
    current_model: Model,
    available_models: Vec<Model>,
    expanded: bool,
    hovered_index: Option<usize>,
    on_select: Option<Box<dyn FnMut(Model)>>,
}

impl ModelSelector {
    pub fn new(current: Model) -> Self {
        Self {
            id: None,
            current_model: current,
            available_models: vec![
                Model::ClaudeSonnet,
                Model::ClaudeOpus,
                Model::ClaudeHaiku,
                Model::Gpt4o,
                Model::GptOss,
                Model::Gemini,
            ],
            expanded: false,
            hovered_index: None,
            on_select: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn models(mut self, models: Vec<Model>) -> Self {
        self.available_models = models;
        self
    }

    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: FnMut(Model) + 'static,
    {
        self.on_select = Some(Box::new(f));
        self
    }

    pub fn current_model(&self) -> Model {
        self.current_model
    }

    pub fn set_model(&mut self, model: Model) {
        self.current_model = model;
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    fn item_height(&self) -> f32 {
        24.0
    }
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self::new(Model::ClaudeSonnet)
    }
}

impl Component for ModelSelector {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut badge = ModelBadge::new(self.current_model);
        badge.paint(bounds, cx);

        let arrow = if self.expanded { " v" } else { " >" };
        let (badge_w, _) = badge.size_hint();
        let arrow_x = bounds.origin.x + badge_w.unwrap_or(60.0);
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - theme::font_size::XS * 0.55;
        let text_run = cx.text.layout(
            arrow,
            Point::new(arrow_x, text_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(text_run);

        if self.expanded {
            let dropdown_y = bounds.origin.y + bounds.size.height;
            let item_height = self.item_height();
            let dropdown_height = self.available_models.len() as f32 * item_height;

            // Dropdown background - use SURFACE for better visibility
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x,
                    dropdown_y,
                    bounds.size.width,
                    dropdown_height,
                ))
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
            );

            for (i, model) in self.available_models.iter().enumerate() {
                let item_y = dropdown_y + i as f32 * item_height;
                let item_bounds = Bounds::new(bounds.origin.x, item_y, bounds.size.width, item_height);

                if self.hovered_index == Some(i) {
                    // Brighter hover state
                    cx.scene.draw_quad(
                        Quad::new(item_bounds).with_background(theme::bg::SELECTED),
                    );
                }

                let mut model_badge = ModelBadge::new(*model);
                model_badge.paint(
                    Bounds::new(
                        item_bounds.origin.x + theme::spacing::XS,
                        item_bounds.origin.y,
                        item_bounds.size.width - theme::spacing::XS * 2.0,
                        item_bounds.size.height,
                    ),
                    cx,
                );
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                if self.expanded {
                    let dropdown_y = bounds.origin.y + bounds.size.height;
                    let item_height = self.item_height();

                    if *x >= bounds.origin.x && *x <= bounds.origin.x + bounds.size.width {
                        let relative_y = *y - dropdown_y;
                        if relative_y >= 0.0 {
                            let index = (relative_y / item_height) as usize;
                            if index < self.available_models.len() {
                                self.hovered_index = Some(index);
                                return EventResult::Handled;
                            }
                        }
                    }
                    self.hovered_index = None;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let click = Point::new(*x, *y);

                    if bounds.contains(click) {
                        self.expanded = !self.expanded;
                        self.hovered_index = None;
                        return EventResult::Handled;
                    }

                    if self.expanded {
                        let dropdown_y = bounds.origin.y + bounds.size.height;
                        let item_height = self.item_height();
                        let dropdown_bounds = Bounds::new(
                            bounds.origin.x,
                            dropdown_y,
                            bounds.size.width,
                            self.available_models.len() as f32 * item_height,
                        );

                        if dropdown_bounds.contains(click) {
                            let index = ((*y - dropdown_y) / item_height) as usize;
                            if index < self.available_models.len() {
                                self.current_model = self.available_models[index];
                                if let Some(on_select) = &mut self.on_select {
                                    on_select(self.current_model);
                                }
                            }
                            self.expanded = false;
                            return EventResult::Handled;
                        }

                        self.expanded = false;
                    }
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(80.0), Some(24.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_selector_new() {
        let selector = ModelSelector::new(Model::ClaudeSonnet);
        assert_eq!(selector.current_model(), Model::ClaudeSonnet);
        assert!(!selector.is_expanded());
    }

    #[test]
    fn test_model_selector_builder() {
        let selector = ModelSelector::new(Model::Gpt4o)
            .with_id(1)
            .models(vec![Model::Gpt4o, Model::Gemini]);

        assert_eq!(selector.id, Some(1));
        assert_eq!(selector.available_models.len(), 2);
    }

    #[test]
    fn test_model_selector_includes_gpt_oss() {
        let selector = ModelSelector::new(Model::ClaudeSonnet);
        assert!(selector.available_models.contains(&Model::GptOss));
    }

    #[test]
    fn test_set_model() {
        let mut selector = ModelSelector::new(Model::Claude);
        selector.set_model(Model::Gemini);
        assert_eq!(selector.current_model(), Model::Gemini);
    }
}
