use crate::components::atoms::{Mode, ModeBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

pub struct ModeSelector {
    id: Option<ComponentId>,
    current_mode: Mode,
    available_modes: Vec<Mode>,
    expanded: bool,
    hovered_index: Option<usize>,
    on_select: Option<Box<dyn FnMut(Mode)>>,
}

impl ModeSelector {
    pub fn new(current: Mode) -> Self {
        Self {
            id: None,
            current_mode: current,
            available_modes: vec![Mode::Normal, Mode::Plan, Mode::Act, Mode::Code, Mode::Chat],
            expanded: false,
            hovered_index: None,
            on_select: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn modes(mut self, modes: Vec<Mode>) -> Self {
        self.available_modes = modes;
        self
    }

    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: FnMut(Mode) + 'static,
    {
        self.on_select = Some(Box::new(f));
        self
    }

    pub fn current_mode(&self) -> Mode {
        self.current_mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.current_mode = mode;
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    fn item_height(&self) -> f32 {
        24.0
    }
}

impl Default for ModeSelector {
    fn default() -> Self {
        Self::new(Mode::Normal)
    }
}

impl Component for ModeSelector {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut badge = ModeBadge::new(self.current_mode);
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
            let dropdown_height = self.available_modes.len() as f32 * item_height;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x,
                    dropdown_y,
                    bounds.size.width,
                    dropdown_height,
                ))
                .with_background(theme::bg::ELEVATED)
                .with_border(theme::border::DEFAULT, 1.0),
            );

            for (i, mode) in self.available_modes.iter().enumerate() {
                let item_y = dropdown_y + i as f32 * item_height;
                let item_bounds =
                    Bounds::new(bounds.origin.x, item_y, bounds.size.width, item_height);

                if self.hovered_index == Some(i) {
                    cx.scene
                        .draw_quad(Quad::new(item_bounds).with_background(theme::bg::HOVER));
                }

                let mut mode_badge = ModeBadge::new(*mode);
                mode_badge.paint(
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
                            if index < self.available_modes.len() {
                                self.hovered_index = Some(index);
                                return EventResult::Handled;
                            }
                        }
                    }
                    self.hovered_index = None;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
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
                            self.available_modes.len() as f32 * item_height,
                        );

                        if dropdown_bounds.contains(click) {
                            let index = ((*y - dropdown_y) / item_height) as usize;
                            if index < self.available_modes.len() {
                                self.current_mode = self.available_modes[index];
                                if let Some(on_select) = &mut self.on_select {
                                    on_select(self.current_mode);
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
    fn test_mode_selector_new() {
        let selector = ModeSelector::new(Mode::Plan);
        assert_eq!(selector.current_mode(), Mode::Plan);
        assert!(!selector.is_expanded());
    }

    #[test]
    fn test_mode_selector_builder() {
        let selector = ModeSelector::new(Mode::Act)
            .with_id(1)
            .modes(vec![Mode::Plan, Mode::Act]);

        assert_eq!(selector.id, Some(1));
        assert_eq!(selector.available_modes.len(), 2);
    }

    #[test]
    fn test_set_mode() {
        let mut selector = ModeSelector::new(Mode::Normal);
        selector.set_mode(Mode::Code);
        assert_eq!(selector.current_mode(), Mode::Code);
    }
}
