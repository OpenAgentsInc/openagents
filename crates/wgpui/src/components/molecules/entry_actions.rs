use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::text::FontStyle;
use crate::{Bounds, Button, ButtonVariant, InputEvent, Point, theme};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use web_time::Instant;

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
    copy_button: Button,
    retry_button: Button,
    edit_button: Button,
    delete_button: Button,
    on_action: Option<Box<dyn FnMut(EntryAction)>>,
    pending_action: Rc<RefCell<Option<EntryAction>>>,
    copy_feedback_until: Option<Instant>,
    copy_feedback_duration: Duration,
}

impl EntryActions {
    pub fn new() -> Self {
        let pending_action = Rc::new(RefCell::new(None));
        let copy_pending = pending_action.clone();
        let retry_pending = pending_action.clone();
        let edit_pending = pending_action.clone();
        let delete_pending = pending_action.clone();
        let copy_button = Button::new("Copy")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::XS)
            .padding(6.0, 2.0)
            .corner_radius(4.0)
            .on_click(move || {
                *copy_pending.borrow_mut() = Some(EntryAction::Copy);
            });
        let retry_button = Button::new("Retry")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::XS)
            .padding(6.0, 2.0)
            .corner_radius(4.0)
            .on_click(move || {
                *retry_pending.borrow_mut() = Some(EntryAction::Retry);
            });
        let edit_button = Button::new("Edit")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::XS)
            .padding(6.0, 2.0)
            .corner_radius(4.0)
            .on_click(move || {
                *edit_pending.borrow_mut() = Some(EntryAction::Edit);
            });
        let delete_button = Button::new("Del")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::XS)
            .padding(6.0, 2.0)
            .corner_radius(4.0)
            .on_click(move || {
                *delete_pending.borrow_mut() = Some(EntryAction::Delete);
            });

        Self {
            id: None,
            show_copy: true,
            show_retry: false,
            show_edit: false,
            show_delete: false,
            copy_button,
            retry_button,
            edit_button,
            delete_button,
            on_action: None,
            pending_action,
            copy_feedback_until: None,
            copy_feedback_duration: Duration::from_secs(1),
        }
    }

    /// Take the last triggered action (clears it).
    pub fn take_triggered_action(&mut self) -> Option<EntryAction> {
        self.pending_action.borrow_mut().take()
    }

    pub fn is_hovered(&self) -> bool {
        (self.show_copy && self.copy_button.is_hovered())
            || (self.show_retry && self.retry_button.is_hovered())
            || (self.show_edit && self.edit_button.is_hovered())
            || (self.show_delete && self.delete_button.is_hovered())
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

    pub fn set_copy_feedback(&mut self) {
        self.copy_feedback_until = Some(Instant::now() + self.copy_feedback_duration);
    }

    fn copy_label(&mut self) -> String {
        if let Some(until) = self.copy_feedback_until
            && Instant::now() < until
        {
            return "Copied".to_string();
        }
        self.copy_feedback_until = None;
        "Copy".to_string()
    }

    fn action_buttons(&mut self) -> Vec<(String, EntryAction)> {
        let mut buttons = Vec::new();
        if self.show_copy {
            let label = self.copy_label();
            self.copy_button.set_label(label.clone());
            buttons.push((label, EntryAction::Copy));
        }
        if self.show_retry {
            buttons.push(("Retry".to_string(), EntryAction::Retry));
        }
        if self.show_edit {
            buttons.push(("Edit".to_string(), EntryAction::Edit));
        }
        if self.show_delete {
            buttons.push(("Del".to_string(), EntryAction::Delete));
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

        for (label, action) in self.action_buttons() {
            let width = cx
                .text
                .measure_styled_mono(&label, font_size, FontStyle::default())
                + 12.0;
            let button_bounds = Bounds::new(x, bounds.origin.y, width, bounds.size.height);
            match action {
                EntryAction::Copy => self.copy_button.paint(button_bounds, cx),
                EntryAction::Retry => self.retry_button.paint(button_bounds, cx),
                EntryAction::Edit => self.edit_button.paint(button_bounds, cx),
                EntryAction::Delete => self.delete_button.paint(button_bounds, cx),
            }
            x += width + gap;
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let gap = theme::spacing::XS;
        let mut handled = false;
        let mut action_x = bounds.origin.x;
        let font_size = theme::font_size::XS;
        for (label, action) in self.action_buttons() {
            let width = font_size * 0.6 * label.len() as f32 + 12.0;
            let action_bounds = Bounds::new(action_x, bounds.origin.y, width, bounds.size.height);
            let action_handled = match action {
                EntryAction::Copy => self.copy_button.event(event, action_bounds, _cx),
                EntryAction::Retry => self.retry_button.event(event, action_bounds, _cx),
                EntryAction::Edit => self.edit_button.event(event, action_bounds, _cx),
                EntryAction::Delete => self.delete_button.event(event, action_bounds, _cx),
            };
            if action_handled.is_handled() {
                handled = true;
            }
            action_x += width + gap;
        }

        if handled {
            let pending_action = self.pending_action.borrow_mut().take();
            if let Some(action) = pending_action {
                if let Some(cb) = &mut self.on_action {
                    cb(action);
                }
                *self.pending_action.borrow_mut() = Some(action);
            }
            return EventResult::Handled;
        }

        if let InputEvent::MouseMove { x, y } = event {
            if !bounds.contains(Point::new(*x, *y)) {
                return EventResult::Ignored;
            }
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
    use crate::{Modifiers, MouseButton};
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
        let mut actions = EntryActions::new().on_action(move |action| {
            if action == EntryAction::Copy {
                called_clone.set(true);
            }
        });

        let bounds = Bounds::new(0.0, 0.0, 200.0, 24.0);
        let font_size = theme::font_size::XS;
        let width = "Copy".len() as f32 * font_size * 0.6;
        let x = bounds.origin.x + width / 2.0;
        let y = bounds.origin.y + bounds.size.height / 2.0;

        let event_down = InputEvent::MouseDown {
            button: MouseButton::Left,
            x,
            y,
            modifiers: Modifiers::default(),
        };
        let mut cx = EventContext::new();
        let result = actions.event(&event_down, bounds, &mut cx);
        assert_eq!(result, EventResult::Handled);

        let event_up = InputEvent::MouseUp {
            button: MouseButton::Left,
            x,
            y,
        };
        let result = actions.event(&event_up, bounds, &mut cx);

        assert_eq!(result, EventResult::Handled);
        assert!(called.get());
    }
}
