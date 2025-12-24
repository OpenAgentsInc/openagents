use crate::{Point, Scene, TextSystem};

pub struct PaintContext<'a> {
    pub scene: &'a mut Scene,
    pub text: &'a mut TextSystem,
    pub scale_factor: f32,
    pub scroll_offset: Point,
}

impl<'a> PaintContext<'a> {
    pub fn new(scene: &'a mut Scene, text: &'a mut TextSystem, scale_factor: f32) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset: Point::ZERO,
        }
    }

    pub fn with_scroll_offset(
        scene: &'a mut Scene,
        text: &'a mut TextSystem,
        scale_factor: f32,
        scroll_offset: Point,
    ) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset,
        }
    }
}

pub struct EventContext {
    pub focused: Option<u64>,
    pub hovered: Option<u64>,
    pub scroll_offset: Point,
}

impl EventContext {
    pub fn new() -> Self {
        Self {
            focused: None,
            hovered: None,
            scroll_offset: Point::ZERO,
        }
    }

    pub fn set_focus(&mut self, id: u64) {
        self.focused = Some(id);
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    pub fn has_focus(&self, id: u64) -> bool {
        self.focused == Some(id)
    }

    pub fn set_hover(&mut self, id: u64) {
        self.hovered = Some(id);
    }

    pub fn clear_hover(&mut self) {
        self.hovered = None;
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_context_focus() {
        let mut cx = EventContext::new();

        assert!(cx.focused.is_none());

        cx.set_focus(42);
        assert!(cx.has_focus(42));
        assert!(!cx.has_focus(99));

        cx.clear_focus();
        assert!(cx.focused.is_none());
    }

    #[test]
    fn test_event_context_hover() {
        let mut cx = EventContext::new();

        assert!(cx.hovered.is_none());

        cx.set_hover(42);
        assert_eq!(cx.hovered, Some(42));

        cx.clear_hover();
        assert!(cx.hovered.is_none());
    }
}
