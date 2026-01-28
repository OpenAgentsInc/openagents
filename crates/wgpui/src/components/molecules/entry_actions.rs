use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    Copy,
    Retry,
    Edit,
    Delete,
}

pub struct EntryActions {
    id: Option<ComponentId>,
    show_copy: bool,
    show_retry: bool,
    show_edit: bool,
    show_delete: bool,
    hovered_action: Option<EntryAction>,
    on_action: Option<Box<dyn FnMut(EntryAction)>>,
    /// Last triggered action (for polling instead of callbacks).
    triggered_action: Option<EntryAction>,
}

impl EntryActions {
    pub fn new() -> Self {
        Self {
            id: None,
            show_copy: true,
            show_retry: false,
            show_edit: false,
            show_delete: false,
            hovered_action: None,
            on_action: None,
            triggered_action: None,
        }
    }

    /// Take the last triggered action (clears it).
    pub fn take_triggered_action(&mut self) -> Option<EntryAction> {
        self.triggered_action.take()
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_copy(mut self, show: bool) -> Self {
        self.show_copy = show;
        self
    }

    pub fn show_retry(mut self, show: bool) -> Self {
        self.show_retry = show;
        self
    }

    pub fn show_edit(mut self, show: bool) -> Self {
        self.show_edit = show;
        self
    }

    pub fn show_delete(mut self, show: bool) -> Self {
        self.show_delete = show;
        self
    }

    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: FnMut(EntryAction) + 'static,
    {
        self.on_action = Some(Box::new(f));
        self
    }

    fn action_buttons(&self) -> Vec<(&str, EntryAction)> {
        let mut buttons = Vec::new();
        if self.show_copy {
            buttons.push(("Copy", EntryAction::Copy));
        }
        if self.show_retry {
            buttons.push(("Retry", EntryAction::Retry));
        }
        if self.show_edit {
            buttons.push(("Edit", EntryAction::Edit));
        }
        if self.show_delete {
            buttons.push(("Del", EntryAction::Delete));
        }
        buttons
    }
}

impl Default for EntryActions {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for EntryActions {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut x = bounds.origin.x;
        let gap = theme::spacing::XS;

        let font_size = theme::font_size::XS;
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;

        for (label, action) in self.action_buttons() {
            let text_color = if self.hovered_action == Some(action) {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            };

            let text_run = cx
                .text
                .layout(label, Point::new(x, text_y), font_size, text_color);
            cx.scene.draw_text(text_run);
            x += label.len() as f32 * font_size * 0.6 + gap;
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let gap = theme::spacing::XS;

        match event {
            InputEvent::MouseMove { x, y } => {
                let click = Point::new(*x, *y);
                if !bounds.contains(click) {
                    self.hovered_action = None;
                    return EventResult::Ignored;
                }

                let mut action_x = bounds.origin.x;

                let font_size = theme::font_size::XS;
                for (label, action) in self.action_buttons() {
                    let width = label.len() as f32 * font_size * 0.6;
                    if *x >= action_x && *x < action_x + width {
                        self.hovered_action = Some(action);
                        return EventResult::Handled;
                    }
                    action_x += width + gap;
                }
                self.hovered_action = None;
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let click = Point::new(*x, *y);

                    let mut current_x = bounds.origin.x;

                    let font_size = theme::font_size::XS;
                    for (label, action) in self.action_buttons() {
                        let width = label.len() as f32 * font_size * 0.6;
                        let action_bounds =
                            Bounds::new(current_x, bounds.origin.y, width, bounds.size.height);
                        if action_bounds.contains(click) {
                            // Store for polling
                            self.triggered_action = Some(action);
                            // Also call callback if set
                            if let Some(cb) = &mut self.on_action {
                                cb(action);
                            }
                            return EventResult::Handled;
                        }
                        current_x += width + gap;
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
        (None, Some(24.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Modifiers;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn test_entry_actions_new() {
        let actions = EntryActions::new();
        assert!(actions.show_copy);
    }

    #[test]
    fn test_entry_actions_builder() {
        let actions = EntryActions::new()
            .with_id(1)
            .show_retry(true)
            .show_delete(true);

        assert_eq!(actions.id, Some(1));
        assert!(actions.show_retry);
        assert!(actions.show_delete);
    }

    #[test]
    fn test_entry_actions_copy_callback() {
        let called = Rc::new(Cell::new(false));
        let called_clone = called.clone();
        let mut actions = EntryActions::new()
            .on_action(move |action| {
                if action == EntryAction::Copy {
                    called_clone.set(true);
                }
            });

        let bounds = Bounds::new(0.0, 0.0, 200.0, 24.0);
        let font_size = theme::font_size::XS;
        let width = "Copy".len() as f32 * font_size * 0.6;
        let x = bounds.origin.x + width / 2.0;
        let y = bounds.origin.y + bounds.size.height / 2.0;

        let event = InputEvent::MouseDown {
            button: MouseButton::Left,
            x,
            y,
            modifiers: Modifiers::default(),
        };
        let mut cx = EventContext::new();
        let result = actions.event(&event, bounds, &mut cx);

        assert_eq!(result, EventResult::Handled);
        assert!(called.get());
    }
}
