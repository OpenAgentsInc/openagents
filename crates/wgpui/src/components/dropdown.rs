use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::input::{Key, NamedKey};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

pub struct DropdownOption {
    pub label: String,
    pub value: String,
}

impl DropdownOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }

    pub fn simple(value: impl Into<String>) -> Self {
        let v = value.into();
        Self {
            label: v.clone(),
            value: v,
        }
    }
}

pub struct Dropdown {
    id: Option<ComponentId>,
    options: Vec<DropdownOption>,
    selected_index: Option<usize>,
    placeholder: String,
    open: bool,
    hovered: bool,
    hovered_option: Option<usize>,
    font_size: f32,
    padding: (f32, f32),
    max_visible_items: usize,
    background: Hsla,
    border_color: Hsla,
    hover_color: Hsla,
    text_color: Hsla,
    placeholder_color: Hsla,
    on_change: Option<Box<dyn FnMut(usize, &str)>>,
}

impl Dropdown {
    pub fn new(options: Vec<DropdownOption>) -> Self {
        Self {
            id: None,
            options,
            selected_index: None,
            placeholder: "Select...".to_string(),
            open: false,
            hovered: false,
            hovered_option: None,
            font_size: theme::font_size::SM,
            padding: (theme::spacing::SM, theme::spacing::XS),
            max_visible_items: 6,
            background: theme::bg::SURFACE,
            border_color: theme::border::DEFAULT,
            hover_color: theme::bg::MUTED,
            text_color: theme::text::PRIMARY,
            placeholder_color: theme::text::MUTED,
            on_change: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        if index < self.options.len() {
            self.selected_index = Some(index);
        }
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn padding(mut self, horizontal: f32, vertical: f32) -> Self {
        self.padding = (horizontal, vertical);
        self
    }

    pub fn max_visible_items(mut self, max: usize) -> Self {
        self.max_visible_items = max;
        self
    }

    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(usize, &str) + 'static,
    {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn selected_value(&self) -> Option<&str> {
        self.selected_index.map(|i| self.options[i].value.as_str())
    }

    pub fn selected_label(&self) -> Option<&str> {
        self.selected_index.map(|i| self.options[i].label.as_str())
    }

    pub fn set_selected(&mut self, index: Option<usize>) {
        if let Some(i) = index {
            if i < self.options.len() {
                self.selected_index = Some(i);
            }
        } else {
            self.selected_index = None;
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.hovered_option = self.selected_index;
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.hovered_option = None;
    }

    fn select(&mut self, index: usize) {
        if index < self.options.len() {
            self.selected_index = Some(index);
            self.close();
            if let Some(on_change) = &mut self.on_change {
                on_change(index, &self.options[index].value);
            }
        }
    }

    fn item_height(&self) -> f32 {
        self.font_size * 1.4 + self.padding.1 * 2.0
    }

    fn dropdown_height(&self) -> f32 {
        let count = self.options.len().min(self.max_visible_items);
        count as f32 * self.item_height()
    }

    fn option_at_y(&self, y: f32, dropdown_top: f32) -> Option<usize> {
        let relative_y = y - dropdown_top;
        if relative_y < 0.0 {
            return None;
        }
        let index = (relative_y / self.item_height()) as usize;
        if index < self.options.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl Default for Dropdown {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl Component for Dropdown {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let border = if self.open || self.hovered {
            theme::accent::PRIMARY
        } else {
            self.border_color
        };

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.background)
                .with_border(border, 1.0),
        );

        let display_text = self
            .selected_index
            .map(|i| self.options[i].label.as_str())
            .unwrap_or(&self.placeholder);

        let text_color = if self.selected_index.is_some() {
            self.text_color
        } else {
            self.placeholder_color
        };

        let text_x = bounds.origin.x + self.padding.0;
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout(
            display_text,
            Point::new(text_x, text_y),
            self.font_size,
            text_color,
        );
        cx.scene.draw_text(text_run);

        let arrow_x = bounds.origin.x + bounds.size.width - self.padding.0 - 8.0;
        let arrow_y = bounds.origin.y + bounds.size.height * 0.5 - self.font_size * 0.55;
        let arrow = if self.open { "^" } else { "v" };

        let arrow_run = cx.text.layout(
            arrow,
            Point::new(arrow_x, arrow_y),
            self.font_size,
            self.text_color,
        );
        cx.scene.draw_text(arrow_run);

        if self.open && !self.options.is_empty() {
            let dropdown_bounds = Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height,
                bounds.size.width,
                self.dropdown_height(),
            );

            cx.scene.draw_quad(
                Quad::new(dropdown_bounds)
                    .with_background(self.background)
                    .with_border(self.border_color, 1.0),
            );

            let item_height = self.item_height();
            for (i, option) in self.options.iter().enumerate().take(self.max_visible_items) {
                let item_y = dropdown_bounds.origin.y + i as f32 * item_height;
                let item_bounds = Bounds::new(
                    dropdown_bounds.origin.x,
                    item_y,
                    dropdown_bounds.size.width,
                    item_height,
                );

                let is_hovered = self.hovered_option == Some(i);
                let is_selected = self.selected_index == Some(i);

                if is_hovered || is_selected {
                    cx.scene
                        .draw_quad(Quad::new(item_bounds).with_background(self.hover_color));
                }

                let option_text_y = item_y + item_height * 0.5 - self.font_size * 0.55;
                let option_run = cx.text.layout(
                    &option.label,
                    Point::new(text_x, option_text_y),
                    self.font_size,
                    self.text_color,
                );
                cx.scene.draw_text(option_run);
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                self.hovered = bounds.contains(point);

                if self.open {
                    let dropdown_top = bounds.origin.y + bounds.size.height;
                    self.hovered_option = self.option_at_y(*y, dropdown_top);
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if bounds.contains(point) {
                        self.toggle();
                        return EventResult::Handled;
                    }

                    if self.open {
                        let dropdown_bounds = Bounds::new(
                            bounds.origin.x,
                            bounds.origin.y + bounds.size.height,
                            bounds.size.width,
                            self.dropdown_height(),
                        );

                        if dropdown_bounds.contains(point)
                            && let Some(index) = self.option_at_y(*y, dropdown_bounds.origin.y)
                        {
                            self.select(index);
                            return EventResult::Handled;
                        }

                        self.close();
                        return EventResult::Handled;
                    }
                }
            }

            InputEvent::KeyDown { key, .. } => {
                if !self.open {
                    return EventResult::Ignored;
                }

                match key {
                    Key::Named(NamedKey::Escape) => {
                        self.close();
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Some(index) = self.hovered_option {
                            self.select(index);
                        }
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        let current = self.hovered_option.unwrap_or(0);
                        self.hovered_option = Some(current.saturating_sub(1));
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        let current = self.hovered_option.unwrap_or(0);
                        let max = self.options.len().saturating_sub(1);
                        self.hovered_option = Some((current + 1).min(max));
                        return EventResult::Handled;
                    }
                    _ => {}
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
        let height = self.font_size * 1.4 + self.padding.1 * 2.0;
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dropdown_option() {
        let opt = DropdownOption::new("Display", "value");
        assert_eq!(opt.label, "Display");
        assert_eq!(opt.value, "value");

        let simple = DropdownOption::simple("both");
        assert_eq!(simple.label, "both");
        assert_eq!(simple.value, "both");
    }

    #[test]
    fn test_dropdown_new() {
        let options = vec![
            DropdownOption::simple("Option 1"),
            DropdownOption::simple("Option 2"),
        ];
        let dropdown = Dropdown::new(options);

        assert_eq!(dropdown.options.len(), 2);
        assert!(dropdown.selected_index.is_none());
        assert!(!dropdown.open);
    }

    #[test]
    fn test_dropdown_builder() {
        let options = vec![DropdownOption::simple("A"), DropdownOption::simple("B")];
        let dropdown = Dropdown::new(options)
            .with_id(42)
            .selected(1)
            .placeholder("Choose...");

        assert_eq!(dropdown.id, Some(42));
        assert_eq!(dropdown.selected_index, Some(1));
        assert_eq!(dropdown.placeholder, "Choose...");
    }

    #[test]
    fn test_dropdown_selected_value() {
        let options = vec![
            DropdownOption::new("First", "1"),
            DropdownOption::new("Second", "2"),
        ];
        let dropdown = Dropdown::new(options).selected(1);

        assert_eq!(dropdown.selected_value(), Some("2"));
        assert_eq!(dropdown.selected_label(), Some("Second"));
    }

    #[test]
    fn test_dropdown_toggle() {
        let mut dropdown = Dropdown::new(vec![DropdownOption::simple("A")]);
        assert!(!dropdown.is_open());

        dropdown.toggle();
        assert!(dropdown.is_open());

        dropdown.toggle();
        assert!(!dropdown.is_open());
    }

    #[test]
    fn test_dropdown_select() {
        let options = vec![DropdownOption::simple("A"), DropdownOption::simple("B")];
        let mut dropdown = Dropdown::new(options);
        dropdown.open = true;

        dropdown.select(1);
        assert_eq!(dropdown.selected_index, Some(1));
        assert!(!dropdown.open);
    }

    #[test]
    fn test_dropdown_set_selected() {
        let options = vec![DropdownOption::simple("A"), DropdownOption::simple("B")];
        let mut dropdown = Dropdown::new(options);

        dropdown.set_selected(Some(1));
        assert_eq!(dropdown.selected_index, Some(1));

        dropdown.set_selected(None);
        assert!(dropdown.selected_index.is_none());

        dropdown.set_selected(Some(999));
        assert!(dropdown.selected_index.is_none());
    }
}
