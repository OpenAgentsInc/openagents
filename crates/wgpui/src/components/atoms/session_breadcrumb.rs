//! Session breadcrumb for navigation between sessions.
//!
//! Shows the session navigation trail and allows jumping to parent sessions.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// A single breadcrumb item
#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub id: String,
    pub label: String,
    pub is_current: bool,
}

impl BreadcrumbItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            is_current: false,
        }
    }

    pub fn current(mut self, is_current: bool) -> Self {
        self.is_current = is_current;
        self
    }
}

/// Session breadcrumb navigation component
pub struct SessionBreadcrumb {
    id: Option<ComponentId>,
    items: Vec<BreadcrumbItem>,
    hovered_index: Option<usize>,
    on_navigate: Option<Box<dyn FnMut(String)>>,
}

impl SessionBreadcrumb {
    pub fn new() -> Self {
        Self {
            id: None,
            items: Vec::new(),
            hovered_index: None,
            on_navigate: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn items(mut self, items: Vec<BreadcrumbItem>) -> Self {
        self.items = items;
        self
    }

    pub fn push_item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn on_navigate<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_navigate = Some(Box::new(f));
        self
    }

    fn item_bounds(&self, bounds: &Bounds) -> Vec<Bounds> {
        let mut result = Vec::new();
        let padding = 8.0;
        let separator_width = 20.0;
        let font_size = theme::font_size::SM;
        let mut x = bounds.origin.x;

        for item in &self.items {
            let width = item.label.len() as f32 * font_size * 0.55 + padding * 2.0;
            result.push(Bounds::new(x, bounds.origin.y, width, bounds.size.height));
            x += width + separator_width;
        }

        result
    }
}

impl Default for SessionBreadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SessionBreadcrumb {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let font_size = theme::font_size::SM;
        let text_y = bounds.origin.y + (bounds.size.height - font_size) / 2.0;
        let item_bounds = self.item_bounds(&bounds);

        for (idx, (item, item_bounds)) in self.items.iter().zip(item_bounds.iter()).enumerate() {
            // Draw separator before non-first items
            if idx > 0 {
                let sep_x = item_bounds.origin.x - 16.0;
                let sep_run = cx.text.layout(
                    "›",
                    Point::new(sep_x, text_y),
                    font_size,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(sep_run);
            }

            // Draw item background on hover (except current)
            let is_hovered = self.hovered_index == Some(idx);
            if is_hovered && !item.is_current {
                cx.scene
                    .draw_quad(Quad::new(*item_bounds).with_background(theme::bg::HOVER));
            }

            // Draw item text
            let text_color = if item.is_current {
                theme::text::PRIMARY
            } else if is_hovered {
                theme::accent::PRIMARY
            } else {
                theme::text::MUTED
            };

            let text_run = cx.text.layout(
                &item.label,
                Point::new(item_bounds.origin.x + 8.0, text_y),
                font_size,
                text_color,
            );
            cx.scene.draw_text(text_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let item_bounds = self.item_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered_index;
                self.hovered_index = None;

                for (idx, bounds) in item_bounds.iter().enumerate() {
                    if bounds.contains(point) && !self.items[idx].is_current {
                        self.hovered_index = Some(idx);
                        break;
                    }
                }

                if was_hovered != self.hovered_index {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    for (idx, bounds) in item_bounds.iter().enumerate() {
                        if bounds.contains(point) && !self.items[idx].is_current {
                            if let Some(callback) = &mut self.on_navigate {
                                callback(self.items[idx].id.clone());
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
        (None, Some(28.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breadcrumb_item() {
        let item = BreadcrumbItem::new("sess-123", "Session #123").current(true);
        assert_eq!(item.id, "sess-123");
        assert!(item.is_current);
    }

    #[test]
    fn test_session_breadcrumb_new() {
        let breadcrumb = SessionBreadcrumb::new()
            .push_item(BreadcrumbItem::new("root", "Root"))
            .push_item(BreadcrumbItem::new("child", "Child").current(true));
        assert_eq!(breadcrumb.items.len(), 2);
    }
}
