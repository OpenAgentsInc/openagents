use crate::components::atoms::CheckpointBadge;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

pub struct CheckpointRestore {
    id: Option<ComponentId>,
    checkpoints: Vec<String>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    on_restore: Option<Box<dyn FnMut(usize, &str)>>,
}

impl CheckpointRestore {
    pub fn new() -> Self {
        Self {
            id: None,
            checkpoints: Vec::new(),
            selected_index: None,
            hovered_index: None,
            on_restore: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn checkpoints(mut self, checkpoints: Vec<String>) -> Self {
        self.checkpoints = checkpoints;
        self
    }

    pub fn add_checkpoint(&mut self, label: impl Into<String>) {
        self.checkpoints.push(label.into());
    }

    pub fn on_restore<F>(mut self, f: F) -> Self
    where
        F: FnMut(usize, &str) + 'static,
    {
        self.on_restore = Some(Box::new(f));
        self
    }

    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn select(&mut self, index: usize) {
        if index < self.checkpoints.len() {
            self.selected_index = Some(index);
        }
    }

    fn item_width(&self) -> f32 {
        100.0
    }
}

impl Default for CheckpointRestore {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CheckpointRestore {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if self.checkpoints.is_empty() {
            return;
        }

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::SM;
        let font_size = theme::font_size::XS;
        let text_y = bounds.origin.y + padding;

        let text_run = cx.text.layout(
            "Restore to checkpoint:",
            Point::new(bounds.origin.x + padding, text_y),
            font_size,
            theme::text::MUTED,
        );
        cx.scene.draw_text(text_run);

        let items_y = text_y + font_size * 1.5;
        let item_height = 28.0;
        let item_width = self.item_width();
        let gap = theme::spacing::SM;

        let mut x = bounds.origin.x + padding;
        for (i, checkpoint) in self.checkpoints.iter().enumerate() {
            let is_selected = self.selected_index == Some(i);
            let is_hovered = self.hovered_index == Some(i);

            let item_bounds = Bounds::new(x, items_y, item_width, item_height);

            if is_hovered && !is_selected {
                cx.scene
                    .draw_quad(Quad::new(item_bounds).with_background(theme::bg::HOVER));
            }

            let mut badge = CheckpointBadge::new(checkpoint).active(is_selected);
            badge.paint(item_bounds, cx);

            x += item_width + gap;
            if x + item_width > bounds.origin.x + bounds.size.width - padding {
                break;
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if self.checkpoints.is_empty() {
            return EventResult::Ignored;
        }

        let padding = theme::spacing::SM;
        let font_size = theme::font_size::XS;
        let items_y = bounds.origin.y + padding + font_size * 1.5;
        let item_height = 28.0;
        let item_width = self.item_width();
        let gap = theme::spacing::SM;

        match event {
            InputEvent::MouseMove { x, y } => {
                if *y >= items_y && *y < items_y + item_height {
                    let relative_x = *x - bounds.origin.x - padding;
                    if relative_x >= 0.0 {
                        let index = (relative_x / (item_width + gap)) as usize;
                        if index < self.checkpoints.len() {
                            self.hovered_index = Some(index);
                            return EventResult::Handled;
                        }
                    }
                }
                self.hovered_index = None;
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && *y >= items_y && *y < items_y + item_height {
                    let relative_x = *x - bounds.origin.x - padding;
                    if relative_x >= 0.0 {
                        let index = (relative_x / (item_width + gap)) as usize;
                        if index < self.checkpoints.len() {
                            self.selected_index = Some(index);
                            if let Some(on_restore) = &mut self.on_restore {
                                on_restore(index, &self.checkpoints[index]);
                            }
                            return EventResult::Handled;
                        }
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
        let padding = theme::spacing::SM;
        let font_size = theme::font_size::XS;
        let item_height = 28.0;
        let height = padding * 2.0 + font_size * 1.5 + item_height;
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_restore_new() {
        let restore = CheckpointRestore::new();
        assert_eq!(restore.checkpoint_count(), 0);
        assert!(restore.selected_index().is_none());
    }

    #[test]
    fn test_checkpoint_restore_builder() {
        let restore = CheckpointRestore::new()
            .with_id(1)
            .checkpoints(vec!["v1".to_string(), "v2".to_string()]);

        assert_eq!(restore.id, Some(1));
        assert_eq!(restore.checkpoint_count(), 2);
    }

    #[test]
    fn test_add_checkpoint() {
        let mut restore = CheckpointRestore::new();
        restore.add_checkpoint("checkpoint-1");
        restore.add_checkpoint("checkpoint-2");
        assert_eq!(restore.checkpoint_count(), 2);
    }

    #[test]
    fn test_select() {
        let mut restore =
            CheckpointRestore::new().checkpoints(vec!["a".to_string(), "b".to_string()]);
        restore.select(1);
        assert_eq!(restore.selected_index(), Some(1));
    }
}
