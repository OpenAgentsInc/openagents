use crate::components::context::{EventContext, PaintContext};
use crate::components::{AnyComponent, Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

pub struct Tab {
    pub label: String,
    pub content: Option<AnyComponent>,
}

impl Tab {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            content: None,
        }
    }

    pub fn content<C: Component + 'static>(mut self, component: C) -> Self {
        self.content = Some(AnyComponent::new(component));
        self
    }
}

pub struct Tabs {
    id: Option<ComponentId>,
    tabs: Vec<Tab>,
    active_index: usize,
    hovered_index: Option<usize>,
    font_size: f32,
    tab_padding: (f32, f32),
    tab_height: f32,
    background: Hsla,
    tab_background: Hsla,
    active_tab_background: Hsla,
    hover_tab_background: Hsla,
    border_color: Hsla,
    text_color: Hsla,
    active_text_color: Hsla,
    indicator_color: Hsla,
    on_change: Option<Box<dyn FnMut(usize)>>,
}

impl Tabs {
    pub fn new(tabs: Vec<Tab>) -> Self {
        Self {
            id: None,
            tabs,
            active_index: 0,
            hovered_index: None,
            font_size: theme::font_size::SM,
            tab_padding: (theme::spacing::MD, theme::spacing::SM),
            tab_height: 40.0,
            background: theme::bg::APP,
            tab_background: Hsla::transparent(),
            active_tab_background: theme::bg::SURFACE,
            hover_tab_background: theme::bg::MUTED,
            border_color: theme::border::DEFAULT,
            text_color: theme::text::MUTED,
            active_text_color: theme::text::PRIMARY,
            indicator_color: theme::accent::PRIMARY,
            on_change: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn active(mut self, index: usize) -> Self {
        if index < self.tabs.len() {
            self.active_index = index;
        }
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn tab_height(mut self, height: f32) -> Self {
        self.tab_height = height;
        self
    }

    pub fn tab_padding(mut self, horizontal: f32, vertical: f32) -> Self {
        self.tab_padding = (horizontal, vertical);
        self
    }

    pub fn indicator_color(mut self, color: Hsla) -> Self {
        self.indicator_color = color;
        self
    }

    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(usize) + 'static,
    {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_index {
            self.active_index = index;
            if let Some(on_change) = &mut self.on_change {
                on_change(index);
            }
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    fn tab_width(&self, label: &str) -> f32 {
        label.chars().count() as f32 * self.font_size * 0.6 + self.tab_padding.0 * 2.0
    }

    fn tab_at_x(&self, x: f32, start_x: f32) -> Option<usize> {
        let mut current_x = start_x;
        for (i, tab) in self.tabs.iter().enumerate() {
            let width = self.tab_width(&tab.label);
            if x >= current_x && x < current_x + width {
                return Some(i);
            }
            current_x += width;
        }
        None
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl Component for Tabs {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let tab_bar_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            self.tab_height,
        );

        cx.scene
            .draw_quad(Quad::new(tab_bar_bounds).with_background(self.background));

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + self.tab_height - 1.0,
                bounds.size.width,
                1.0,
            ))
            .with_background(self.border_color),
        );

        let mut tab_x = bounds.origin.x;
        for (i, tab) in self.tabs.iter().enumerate() {
            let tab_width = self.tab_width(&tab.label);
            let tab_bounds = Bounds::new(tab_x, bounds.origin.y, tab_width, self.tab_height);

            let is_active = i == self.active_index;
            let is_hovered = self.hovered_index == Some(i);

            let bg = if is_active {
                self.active_tab_background
            } else if is_hovered {
                self.hover_tab_background
            } else {
                self.tab_background
            };

            if bg.a > 0.0 {
                cx.scene
                    .draw_quad(Quad::new(tab_bounds).with_background(bg));
            }

            let text_color = if is_active {
                self.active_text_color
            } else {
                self.text_color
            };

            let text_x = tab_x + self.tab_padding.0;
            let text_y = bounds.origin.y + self.tab_height * 0.5 - self.font_size * 0.55;

            let text_run = cx.text.layout(
                &tab.label,
                Point::new(text_x, text_y),
                self.font_size,
                text_color,
            );
            cx.scene.draw_text(text_run);

            if is_active {
                let indicator_height = 2.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        tab_x,
                        bounds.origin.y + self.tab_height - indicator_height,
                        tab_width,
                        indicator_height,
                    ))
                    .with_background(self.indicator_color),
                );
            }

            tab_x += tab_width;
        }

        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + self.tab_height,
            bounds.size.width,
            bounds.size.height - self.tab_height,
        );

        if let Some(tab) = self.tabs.get_mut(self.active_index)
            && let Some(content) = &mut tab.content
        {
            content.paint(content_bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let tab_bar_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            self.tab_height,
        );

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                if tab_bar_bounds.contains(point) {
                    self.hovered_index = self.tab_at_x(*x, bounds.origin.x);
                    return EventResult::Handled;
                } else {
                    self.hovered_index = None;
                }
            }

            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    if tab_bar_bounds.contains(point)
                        && let Some(index) = self.tab_at_x(*x, bounds.origin.x)
                    {
                        self.set_active(index);
                        return EventResult::Handled;
                    }
                }
            }

            _ => {}
        }

        let content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + self.tab_height,
            bounds.size.width,
            bounds.size.height - self.tab_height,
        );

        if let Some(tab) = self.tabs.get_mut(self.active_index)
            && let Some(content) = &mut tab.content
        {
            return content.event(event, content_bounds, cx);
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Div;

    #[test]
    fn test_tab_new() {
        let tab = Tab::new("Test Tab");
        assert_eq!(tab.label, "Test Tab");
        assert!(tab.content.is_none());
    }

    #[test]
    fn test_tab_with_content() {
        let content = Div::new().background(theme::bg::MUTED);
        let tab = Tab::new("Test").content(content);
        assert!(tab.content.is_some());
    }

    #[test]
    fn test_tabs_new() {
        let tabs = Tabs::new(vec![Tab::new("Tab 1"), Tab::new("Tab 2")]);

        assert_eq!(tabs.tab_count(), 2);
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn test_tabs_builder() {
        let tabs = Tabs::new(vec![Tab::new("A"), Tab::new("B"), Tab::new("C")])
            .with_id(42)
            .active(1)
            .tab_height(50.0);

        assert_eq!(tabs.id, Some(42));
        assert_eq!(tabs.active_index, 1);
        assert_eq!(tabs.tab_height, 50.0);
    }

    #[test]
    fn test_tabs_set_active() {
        let mut tabs = Tabs::new(vec![Tab::new("A"), Tab::new("B")]);

        assert_eq!(tabs.active_index(), 0);

        tabs.set_active(1);
        assert_eq!(tabs.active_index(), 1);

        tabs.set_active(999);
        assert_eq!(tabs.active_index(), 1);
    }

    #[test]
    fn test_tabs_active_unchanged() {
        let mut tabs = Tabs::new(vec![Tab::new("A")]);
        tabs.set_active(0);
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn test_tab_width_calculation() {
        let tabs = Tabs::new(vec![]).font_size(14.0).tab_padding(16.0, 8.0);
        let width = tabs.tab_width("Test");
        assert!(width > 32.0);
    }
}
