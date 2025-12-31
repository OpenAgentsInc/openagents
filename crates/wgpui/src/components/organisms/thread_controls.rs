use crate::components::atoms::{Mode, Model};
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::{ModeSelector, ModelSelector};
use crate::components::{Button, ButtonVariant, Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct ThreadControls {
    id: Option<ComponentId>,
    mode: Mode,
    model: Model,
    mode_selector: ModeSelector,
    model_selector: ModelSelector,
    on_mode_change: Option<Box<dyn FnMut(Mode)>>,
    on_model_change: Option<Box<dyn FnMut(Model)>>,
    on_run: Option<Box<dyn FnMut()>>,
    on_stop: Option<Box<dyn FnMut()>>,
    is_running: bool,
}

impl ThreadControls {
    pub fn new() -> Self {
        Self {
            id: None,
            mode: Mode::Normal,
            model: Model::ClaudeSonnet,
            mode_selector: ModeSelector::new(Mode::Normal),
            model_selector: ModelSelector::new(Model::ClaudeSonnet),
            on_mode_change: None,
            on_model_change: None,
            on_run: None,
            on_stop: None,
            is_running: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self.mode_selector.set_mode(mode);
        self
    }

    pub fn model(mut self, model: Model) -> Self {
        self.model = model;
        self.model_selector.set_model(model);
        self
    }

    pub fn running(mut self, running: bool) -> Self {
        self.is_running = running;
        self
    }

    pub fn on_mode_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(Mode) + 'static,
    {
        self.on_mode_change = Some(Box::new(f));
        self
    }

    pub fn on_model_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(Model) + 'static,
    {
        self.on_model_change = Some(Box::new(f));
        self
    }

    pub fn on_run<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_run = Some(Box::new(f));
        self
    }

    pub fn on_stop<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_stop = Some(Box::new(f));
        self
    }

    pub fn current_mode(&self) -> Mode {
        self.mode
    }

    pub fn current_model(&self) -> Model {
        self.model
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

impl Default for ThreadControls {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ThreadControls {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let selector_width = 120.0;
        let selector_height = 28.0;
        let gap = theme::spacing::SM;

        self.mode_selector.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + (bounds.size.height - selector_height) / 2.0,
                selector_width,
                selector_height,
            ),
            cx,
        );

        self.model_selector.paint(
            Bounds::new(
                bounds.origin.x + padding + selector_width + gap,
                bounds.origin.y + (bounds.size.height - selector_height) / 2.0,
                selector_width,
                selector_height,
            ),
            cx,
        );

        if self.is_running {
            let btn_width = 60.0;
            let btn_height = 28.0;
            let mut stop_btn = Button::new("Stop").variant(ButtonVariant::Danger);
            stop_btn.paint(
                Bounds::new(
                    bounds.origin.x + bounds.size.width - padding - btn_width,
                    bounds.origin.y + (bounds.size.height - btn_height) / 2.0,
                    btn_width,
                    btn_height,
                ),
                cx,
            );
        } else {
            let btn_width = 60.0;
            let btn_height = 28.0;
            let mut run_btn = Button::new("Run").variant(ButtonVariant::Primary);
            run_btn.paint(
                Bounds::new(
                    bounds.origin.x + bounds.size.width - padding - btn_width,
                    bounds.origin.y + (bounds.size.height - btn_height) / 2.0,
                    btn_width,
                    btn_height,
                ),
                cx,
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = theme::spacing::SM;
        let selector_width = 120.0;
        let selector_height = 28.0;
        let gap = theme::spacing::SM;

        let mode_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + (bounds.size.height - selector_height) / 2.0,
            selector_width,
            selector_height,
        );

        let result = self.mode_selector.event(event, mode_bounds, cx);
        if result == EventResult::Handled {
            let new_mode = self.mode_selector.current_mode();
            if new_mode != self.mode {
                self.mode = new_mode;
                if let Some(callback) = &mut self.on_mode_change {
                    callback(new_mode);
                }
            }
            return result;
        }

        let model_bounds = Bounds::new(
            bounds.origin.x + padding + selector_width + gap,
            bounds.origin.y + (bounds.size.height - selector_height) / 2.0,
            selector_width,
            selector_height,
        );

        let result = self.model_selector.event(event, model_bounds, cx);
        if result == EventResult::Handled {
            let new_model = self.model_selector.current_model();
            if new_model != self.model {
                self.model = new_model;
                if let Some(callback) = &mut self.on_model_change {
                    callback(new_model);
                }
            }
            return result;
        }

        if self.is_running {
            let btn_width = 60.0;
            let btn_height = 28.0;
            let stop_bounds = Bounds::new(
                bounds.origin.x + bounds.size.width - padding - btn_width,
                bounds.origin.y + (bounds.size.height - btn_height) / 2.0,
                btn_width,
                btn_height,
            );

            if let InputEvent::MouseUp { x, y, .. } = event
                && stop_bounds.contains(Point::new(*x, *y))
            {
                if let Some(callback) = &mut self.on_stop {
                    callback();
                }
                return EventResult::Handled;
            }
        } else {
            let btn_width = 60.0;
            let btn_height = 28.0;
            let run_bounds = Bounds::new(
                bounds.origin.x + bounds.size.width - padding - btn_width,
                bounds.origin.y + (bounds.size.height - btn_height) / 2.0,
                btn_width,
                btn_height,
            );

            if let InputEvent::MouseUp { x, y, .. } = event
                && run_bounds.contains(Point::new(*x, *y))
            {
                if let Some(callback) = &mut self.on_run {
                    callback();
                }
                return EventResult::Handled;
            }
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(44.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MouseButton;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn test_thread_controls_new() {
        let controls = ThreadControls::new();
        assert_eq!(controls.current_mode(), Mode::Normal);
        assert_eq!(controls.current_model(), Model::ClaudeSonnet);
        assert!(!controls.is_running());
    }

    #[test]
    fn test_thread_controls_builder() {
        let controls = ThreadControls::new()
            .with_id(1)
            .mode(Mode::Plan)
            .model(Model::ClaudeOpus)
            .running(true);

        assert_eq!(controls.id, Some(1));
        assert_eq!(controls.current_mode(), Mode::Plan);
        assert_eq!(controls.current_model(), Model::ClaudeOpus);
        assert!(controls.is_running());
    }

    #[test]
    fn test_thread_controls_stop_callback() {
        let called = Rc::new(Cell::new(false));
        let called_clone = called.clone();

        let mut controls = ThreadControls::new().running(true).on_stop(move || {
            called_clone.set(true);
        });

        let bounds = Bounds::new(0.0, 0.0, 300.0, 44.0);
        let padding = theme::spacing::SM;
        let btn_width = 60.0;
        let btn_height = 28.0;
        let x = bounds.origin.x + bounds.size.width - padding - btn_width + btn_width / 2.0;
        let y = bounds.origin.y + (bounds.size.height - btn_height) / 2.0 + btn_height / 2.0;

        let event = InputEvent::MouseUp {
            button: MouseButton::Left,
            x,
            y,
        };
        let mut cx = EventContext::new();
        let result = controls.event(&event, bounds, &mut cx);

        assert_eq!(result, EventResult::Handled);
        assert!(called.get());
    }

    #[test]
    fn test_thread_controls_run_callback() {
        let called = Rc::new(Cell::new(false));
        let called_clone = called.clone();

        let mut controls = ThreadControls::new().running(false).on_run(move || {
            called_clone.set(true);
        });

        let bounds = Bounds::new(0.0, 0.0, 300.0, 44.0);
        let padding = theme::spacing::SM;
        let btn_width = 60.0;
        let btn_height = 28.0;
        let x = bounds.origin.x + bounds.size.width - padding - btn_width + btn_width / 2.0;
        let y = bounds.origin.y + (bounds.size.height - btn_height) / 2.0 + btn_height / 2.0;

        let event = InputEvent::MouseUp {
            button: MouseButton::Left,
            x,
            y,
        };
        let mut cx = EventContext::new();
        let result = controls.event(&event, bounds, &mut cx);

        assert_eq!(result, EventResult::Handled);
        assert!(called.get());
    }
}
