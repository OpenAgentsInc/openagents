use wgpui::{Bounds, InputEvent, Key, MouseButton, NamedKey, PaintContext, Point, Quad, Text, theme};

pub(crate) struct FocusDemo {
    items: Vec<&'static str>,
    focused: usize,
    active: Vec<bool>,
    hovered: Option<usize>,
}

impl FocusDemo {
    pub(crate) fn new() -> Self {
        let items = vec!["Focus A", "Focus B", "Focus C"];
        Self {
            focused: 0,
            active: vec![false; items.len()],
            items,
            hovered: None,
        }
    }

    pub(crate) fn paint(&self, bounds: Bounds, cx: &mut PaintContext) {
        let hint_height = 18.0;
        let mut hint = Text::new("Tab or Shift+Tab to move. Enter toggles.")
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        hint.paint(
            Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                hint_height,
            ),
            cx,
        );

        let items_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + hint_height + 10.0,
            bounds.size.width,
            bounds.size.height - hint_height - 10.0,
        );
        let gap = 12.0;
        let item_width = ((items_bounds.size.width - gap * (self.items.len() as f32 - 1.0))
            / self.items.len() as f32)
            .max(0.0);
        let item_height = 36.0;

        for (index, label) in self.items.iter().enumerate() {
            let x = items_bounds.origin.x + index as f32 * (item_width + gap);
            let y = items_bounds.origin.y;
            let item_bounds = Bounds::new(x, y, item_width, item_height);

            let is_focused = self.focused == index;
            let is_active = self.active[index];
            let is_hovered = self.hovered == Some(index);

            let border = if is_focused {
                theme::accent::PRIMARY
            } else {
                theme::border::DEFAULT
            };
            let bg = if is_active {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::SURFACE
            };

            cx.scene.draw_quad(
                Quad::new(item_bounds)
                    .with_background(bg)
                    .with_border(border, 1.0),
            );

            let mut text = Text::new(*label)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            text.paint(
                Bounds::new(
                    item_bounds.origin.x + 8.0,
                    item_bounds.origin.y + 8.0,
                    item_bounds.size.width - 16.0,
                    item_bounds.size.height,
                ),
                cx,
            );
        }
    }

    pub(crate) fn handle_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let hint_height = 18.0;
        let items_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + hint_height + 10.0,
            bounds.size.width,
            bounds.size.height - hint_height - 10.0,
        );
        let gap = 12.0;
        let item_width = ((items_bounds.size.width - gap * (self.items.len() as f32 - 1.0))
            / self.items.len() as f32)
            .max(0.0);
        let item_height = 36.0;

        match event {
            InputEvent::MouseMove { x, y } => {
                let mut hover = None;
                for i in 0..self.items.len() {
                    let item_bounds = Bounds::new(
                        items_bounds.origin.x + i as f32 * (item_width + gap),
                        items_bounds.origin.y,
                        item_width,
                        item_height,
                    );
                    if item_bounds.contains(Point::new(*x, *y)) {
                        hover = Some(i);
                        break;
                    }
                }
                if hover != self.hovered {
                    self.hovered = hover;
                    return true;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    for i in 0..self.items.len() {
                        let item_bounds = Bounds::new(
                            items_bounds.origin.x + i as f32 * (item_width + gap),
                            items_bounds.origin.y,
                            item_width,
                            item_height,
                        );
                        if item_bounds.contains(Point::new(*x, *y)) {
                            self.focused = i;
                            self.active[i] = !self.active[i];
                            return true;
                        }
                    }
                }
            }
            InputEvent::KeyDown { key, modifiers } => match key {
                Key::Named(NamedKey::Tab) => {
                    if modifiers.shift {
                        self.focused = (self.focused + self.items.len() - 1) % self.items.len();
                    } else {
                        self.focused = (self.focused + 1) % self.items.len();
                    }
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    self.active[self.focused] = !self.active[self.focused];
                    return true;
                }
                _ => {}
            },
            _ => {}
        }
        false
    }
}
