use crate::components::atoms::{Mode, ModeBadge, Model, ModelBadge, Status, StatusDot};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusBarPosition {
    #[default]
    Bottom,
    Top,
}

#[derive(Clone)]
pub struct StatusItem {
    pub id: String,
    pub content: StatusItemContent,
    pub alignment: StatusItemAlignment,
}

#[derive(Clone)]
pub enum StatusItemContent {
    Text(String),
    Mode(Mode),
    Model(Model),
    Status(Status),
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusItemAlignment {
    #[default]
    Left,
    Center,
    Right,
}

impl StatusItem {
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: StatusItemContent::Text(text.into()),
            alignment: StatusItemAlignment::Left,
        }
    }

    pub fn mode(id: impl Into<String>, mode: Mode) -> Self {
        Self {
            id: id.into(),
            content: StatusItemContent::Mode(mode),
            alignment: StatusItemAlignment::Left,
        }
    }

    pub fn model(id: impl Into<String>, model: Model) -> Self {
        Self {
            id: id.into(),
            content: StatusItemContent::Model(model),
            alignment: StatusItemAlignment::Left,
        }
    }

    pub fn status(id: impl Into<String>, status: Status) -> Self {
        Self {
            id: id.into(),
            content: StatusItemContent::Status(status),
            alignment: StatusItemAlignment::Left,
        }
    }

    pub fn align(mut self, alignment: StatusItemAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn left(self) -> Self {
        self.align(StatusItemAlignment::Left)
    }

    pub fn center(self) -> Self {
        self.align(StatusItemAlignment::Center)
    }

    pub fn right(self) -> Self {
        self.align(StatusItemAlignment::Right)
    }
}

pub struct StatusBar {
    id: Option<ComponentId>,
    items: Vec<StatusItem>,
    position: StatusBarPosition,
    height: f32,
    on_item_click: Option<Box<dyn FnMut(&str)>>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            id: None,
            items: Vec::new(),
            position: StatusBarPosition::Bottom,
            height: 28.0,
            on_item_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn items(mut self, items: Vec<StatusItem>) -> Self {
        self.items = items;
        self
    }

    pub fn position(mut self, position: StatusBarPosition) -> Self {
        self.position = position;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn on_item_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_item_click = Some(Box::new(f));
        self
    }

    pub fn add_item(&mut self, item: StatusItem) {
        self.items.push(item);
    }

    pub fn set_items(&mut self, items: Vec<StatusItem>) {
        self.items = items;
    }

    pub fn update_item(&mut self, id: &str, content: StatusItemContent) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.content = content;
        }
    }

    pub fn remove_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }

    fn left_items(&self) -> impl Iterator<Item = &StatusItem> {
        self.items
            .iter()
            .filter(|i| i.alignment == StatusItemAlignment::Left)
    }

    fn center_items(&self) -> impl Iterator<Item = &StatusItem> {
        self.items
            .iter()
            .filter(|i| i.alignment == StatusItemAlignment::Center)
    }

    fn right_items(&self) -> impl Iterator<Item = &StatusItem> {
        self.items
            .iter()
            .filter(|i| i.alignment == StatusItemAlignment::Right)
    }

    fn paint_item(&mut self, item: &StatusItem, x: f32, y: f32, cx: &mut PaintContext) -> f32 {
        let spacing = theme::spacing::SM;
        let font_size = theme::font_size::XS;

        match &item.content {
            StatusItemContent::Text(text) => {
                let width = text.len() as f32 * font_size * 0.6;
                let text_run = cx.text.layout_mono(
                    text,
                    Point::new(x, y + (self.height - font_size) / 2.0),
                    font_size,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(text_run);
                width + spacing
            }
            StatusItemContent::Mode(mode) => {
                let mut badge = ModeBadge::new(*mode);
                let (w, h) = badge.size_hint();
                let badge_bounds = Bounds::new(
                    x,
                    y + (self.height - h.unwrap_or(20.0)) / 2.0,
                    w.unwrap_or(50.0),
                    h.unwrap_or(20.0),
                );
                badge.paint(badge_bounds, cx);
                w.unwrap_or(50.0) + spacing
            }
            StatusItemContent::Model(model) => {
                let mut badge = ModelBadge::new(*model);
                let (w, h) = badge.size_hint();
                let badge_bounds = Bounds::new(
                    x,
                    y + (self.height - h.unwrap_or(20.0)) / 2.0,
                    w.unwrap_or(80.0),
                    h.unwrap_or(20.0),
                );
                badge.paint(badge_bounds, cx);
                w.unwrap_or(80.0) + spacing
            }
            StatusItemContent::Status(status) => {
                let mut dot = StatusDot::new(*status);
                let (w, h) = dot.size_hint();
                let dot_bounds = Bounds::new(
                    x,
                    y + (self.height - h.unwrap_or(8.0)) / 2.0,
                    w.unwrap_or(8.0),
                    h.unwrap_or(8.0),
                );
                dot.paint(dot_bounds, cx);
                w.unwrap_or(8.0) + spacing
            }
            StatusItemContent::Custom(symbol) => {
                let width = font_size;
                let text_run = cx.text.layout_mono(
                    symbol,
                    Point::new(x, y + (self.height - font_size) / 2.0),
                    font_size,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(text_run);
                width + spacing
            }
        }
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StatusBar {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let bar_y = match self.position {
            StatusBarPosition::Bottom => bounds.origin.y + bounds.size.height - self.height,
            StatusBarPosition::Top => bounds.origin.y,
        };

        let bar_bounds = Bounds::new(bounds.origin.x, bar_y, bounds.size.width, self.height);

        cx.scene.draw_quad(
            Quad::new(bar_bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::SM;

        let mut left_x = bar_bounds.origin.x + padding;
        let left_items: Vec<_> = self.left_items().cloned().collect();
        for item in &left_items {
            let width = self.paint_item(item, left_x, bar_y, cx);
            left_x += width;
        }

        let mut right_x = bar_bounds.origin.x + bar_bounds.size.width - padding;
        let right_items: Vec<_> = self.right_items().cloned().collect();
        for item in right_items.iter().rev() {
            let item_width = match &item.content {
                StatusItemContent::Text(t) => t.len() as f32 * theme::font_size::XS * 0.6,
                StatusItemContent::Mode(_) => 50.0,
                StatusItemContent::Model(_) => 80.0,
                StatusItemContent::Status(_) => 8.0,
                StatusItemContent::Custom(_) => theme::font_size::XS,
            };
            right_x -= item_width + theme::spacing::SM;
            self.paint_item(item, right_x, bar_y, cx);
        }

        let center_items: Vec<_> = self.center_items().cloned().collect();
        if !center_items.is_empty() {
            let mut total_width = 0.0;
            for item in &center_items {
                total_width += match &item.content {
                    StatusItemContent::Text(t) => t.len() as f32 * theme::font_size::XS * 0.6,
                    StatusItemContent::Mode(_) => 50.0,
                    StatusItemContent::Model(_) => 80.0,
                    StatusItemContent::Status(_) => 8.0,
                    StatusItemContent::Custom(_) => theme::font_size::XS,
                } + theme::spacing::SM;
            }

            let mut center_x = bar_bounds.origin.x + (bar_bounds.size.width - total_width) / 2.0;
            for item in &center_items {
                let width = self.paint_item(item, center_x, bar_y, cx);
                center_x += width;
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let bar_y = match self.position {
            StatusBarPosition::Bottom => bounds.origin.y + bounds.size.height - self.height,
            StatusBarPosition::Top => bounds.origin.y,
        };

        let bar_bounds = Bounds::new(bounds.origin.x, bar_y, bounds.size.width, self.height);

        if let InputEvent::MouseUp { x, y, .. } = event {
            let point = Point::new(*x, *y);
            if bar_bounds.contains(point) {
                return EventResult::Handled;
            }
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(self.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, EventContext, InputEvent, MouseButton};

    #[test]
    fn test_status_item_text() {
        let item = StatusItem::text("branch", "main");
        assert_eq!(item.id, "branch");
        matches!(item.content, StatusItemContent::Text(_));
    }

    #[test]
    fn test_status_item_mode() {
        let item = StatusItem::mode("mode", Mode::Plan);
        assert_eq!(item.id, "mode");
        matches!(item.content, StatusItemContent::Mode(Mode::Plan));
    }

    #[test]
    fn test_status_item_alignment() {
        let item = StatusItem::text("test", "Test").right();
        assert_eq!(item.alignment, StatusItemAlignment::Right);

        let item = StatusItem::text("test", "Test").center();
        assert_eq!(item.alignment, StatusItemAlignment::Center);
    }

    #[test]
    fn test_status_bar_new() {
        let bar = StatusBar::new();
        assert_eq!(bar.height, 28.0);
        assert!(bar.items.is_empty());
    }

    #[test]
    fn test_status_bar_builder() {
        let bar = StatusBar::new()
            .with_id(1)
            .height(32.0)
            .position(StatusBarPosition::Top)
            .items(vec![
                StatusItem::mode("mode", Mode::Normal).left(),
                StatusItem::text("file", "main.rs").center(),
                StatusItem::model("model", Model::Claude).right(),
            ]);

        assert_eq!(bar.id, Some(1));
        assert_eq!(bar.height, 32.0);
        assert_eq!(bar.position, StatusBarPosition::Top);
        assert_eq!(bar.items.len(), 3);
    }

    #[test]
    fn test_status_bar_add_remove() {
        let mut bar = StatusBar::new();
        bar.add_item(StatusItem::text("test", "Test"));
        assert_eq!(bar.items.len(), 1);

        bar.remove_item("test");
        assert!(bar.items.is_empty());
    }

    #[test]
    fn test_status_bar_update_item() {
        let mut bar = StatusBar::new().items(vec![StatusItem::text("count", "0")]);

        bar.update_item("count", StatusItemContent::Text("42".to_string()));

        if let StatusItemContent::Text(t) = &bar.items[0].content {
            assert_eq!(t, "42");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_status_bar_size_hint() {
        let bar = StatusBar::new().height(24.0);
        let (w, h) = bar.size_hint();
        assert!(w.is_none());
        assert_eq!(h, Some(24.0));
    }

    #[test]
    fn test_status_bar_alignment_filters() {
        let bar = StatusBar::new().items(vec![
            StatusItem::text("left", "Left").left(),
            StatusItem::text("center", "Center").center(),
            StatusItem::text("right", "Right").right(),
        ]);

        let left_ids: Vec<_> = bar.left_items().map(|item| item.id.as_str()).collect();
        let center_ids: Vec<_> = bar.center_items().map(|item| item.id.as_str()).collect();
        let right_ids: Vec<_> = bar.right_items().map(|item| item.id.as_str()).collect();

        assert_eq!(left_ids, vec!["left"]);
        assert_eq!(center_ids, vec!["center"]);
        assert_eq!(right_ids, vec!["right"]);
    }

    #[test]
    fn test_status_bar_event_inside_outside_bottom() {
        let mut bar = StatusBar::new()
            .height(20.0)
            .position(StatusBarPosition::Bottom);
        let bounds = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let mut cx = EventContext::new();

        let inside = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: 10.0,
            y: 95.0,
        };
        let result = bar.event(&inside, bounds, &mut cx);
        assert_eq!(result, EventResult::Handled);

        let outside = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: 10.0,
            y: 10.0,
        };
        let result = bar.event(&outside, bounds, &mut cx);
        assert_eq!(result, EventResult::Ignored);
    }

    #[test]
    fn test_status_bar_event_inside_top() {
        let mut bar = StatusBar::new()
            .height(20.0)
            .position(StatusBarPosition::Top);
        let bounds = Bounds::new(0.0, 0.0, 200.0, 100.0);
        let mut cx = EventContext::new();

        let inside = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: 10.0,
            y: 5.0,
        };
        let result = bar.event(&inside, bounds, &mut cx);
        assert_eq!(result, EventResult::Handled);
    }
}
