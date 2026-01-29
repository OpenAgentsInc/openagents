use std::time::Duration;

use web_time::Instant;

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, EventResult};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

#[derive(Clone, Debug)]
pub struct HotbarSlot {
    pub slot: u8,
    pub icon: String,
    pub title: String,
    pub active: bool,
    pub ghost: bool,
}

impl HotbarSlot {
    pub fn new(slot: u8, icon: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            slot,
            icon: icon.into(),
            title: title.into(),
            active: false,
            ghost: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn ghost(mut self, ghost: bool) -> Self {
        self.ghost = ghost;
        self
    }
}

pub struct Hotbar {
    items: Vec<HotbarSlot>,
    item_bounds: Vec<Bounds>,
    hovered_index: Option<usize>,
    pressed_index: Option<usize>,
    flash_index: Option<usize>,
    flash_started: Option<Instant>,
    flash_duration: Duration,
    pending_clicks: Vec<u8>,
    item_size: f32,
    gap: f32,
    padding: f32,
    corner_radius: f32,
    font_scale: f32,
}

impl Hotbar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            item_bounds: Vec::new(),
            hovered_index: None,
            pressed_index: None,
            flash_index: None,
            flash_started: None,
            flash_duration: Duration::from_millis(90),
            pending_clicks: Vec::new(),
            item_size: 36.0,
            gap: 6.0,
            padding: 6.0,
            corner_radius: 6.0,
            font_scale: 1.0,
        }
    }

    pub fn items(mut self, items: Vec<HotbarSlot>) -> Self {
        self.items = items;
        self
    }

    pub fn set_items(&mut self, items: Vec<HotbarSlot>) {
        self.items = items;
    }

    pub fn item_size(mut self, size: f32) -> Self {
        self.item_size = size;
        self
    }

    pub fn set_item_size(&mut self, size: f32) {
        self.item_size = size;
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub fn set_gap(&mut self, gap: f32) {
        self.gap = gap;
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn set_padding(&mut self, padding: f32) {
        self.padding = padding;
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radius = radius;
    }

    pub fn font_scale(mut self, scale: f32) -> Self {
        self.font_scale = scale.max(0.1);
        self
    }

    pub fn set_font_scale(&mut self, scale: f32) {
        self.font_scale = scale.max(0.1);
    }

    pub fn is_hovered(&self) -> bool {
        self.hovered_index.is_some()
    }

    pub fn take_clicked_slots(&mut self) -> Vec<u8> {
        let mut clicks = Vec::new();
        std::mem::swap(&mut clicks, &mut self.pending_clicks);
        clicks
    }

    pub fn flash_slot(&mut self, slot: u8) {
        if let Some((index, _)) = self
            .items
            .iter()
            .enumerate()
            .find(|(_, item)| item.slot == slot)
        {
            self.flash_index = Some(index);
            self.flash_started = Some(Instant::now());
        }
    }

    pub fn is_flashing(&mut self) -> bool {
        self.clear_expired_flash();
        self.flash_index.is_some()
    }

    fn clear_expired_flash(&mut self) {
        let Some(started) = self.flash_started else {
            return;
        };
        if Instant::now().duration_since(started) >= self.flash_duration {
            self.flash_index = None;
            self.flash_started = None;
        }
    }

    fn layout_items(&mut self, bounds: Bounds) {
        self.item_bounds.clear();
        let mut x = bounds.origin.x + self.padding;
        let y = bounds.origin.y + (bounds.size.height - self.item_size) * 0.5;

        for _ in &self.items {
            self.item_bounds
                .push(Bounds::new(x, y, self.item_size, self.item_size));
            x += self.item_size + self.gap;
        }
    }

    fn item_index_at(&self, point: Point) -> Option<usize> {
        self.item_bounds
            .iter()
            .position(|bounds| bounds.contains(point))
    }

    fn item_colors(&self, item: &HotbarSlot, hovered: bool, pressed: bool) -> (Hsla, Hsla, Hsla) {
        let mut bg = theme::bg::APP;
        let mut border = theme::border::DEFAULT;
        let mut text = theme::text::PRIMARY;

        if item.active {
            bg = theme::bg::SURFACE;
            border = theme::accent::PRIMARY;
        }

        if pressed {
            bg = bg.darken(0.1);
            border = border.darken(0.1);
        } else if hovered {
            bg = bg.darken(0.05);
        }

        if item.ghost {
            bg = bg.with_alpha(bg.a * 0.35);
            border = border.with_alpha(border.a * 0.35);
            text = text.with_alpha(text.a * 0.35);
        }

        (bg, border, text)
    }
}

impl Default for Hotbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Hotbar {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if self.items.is_empty() {
            return;
        }

        self.clear_expired_flash();
        self.layout_items(bounds);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE.with_alpha(0.6))
                .with_border(theme::border::DEFAULT.with_alpha(0.4), 1.0)
                .with_corner_radius(self.corner_radius),
        );

        let icon_font = theme::font_size::SM * self.font_scale;
        let number_font = theme::font_size::XS * 0.85 * self.font_scale;

        for (idx, item) in self.items.iter().enumerate() {
            let item_bounds = self.item_bounds[idx];
            let hovered = self.hovered_index == Some(idx);
            let pressed = self.pressed_index == Some(idx);
            let (bg, border, text_color) = self.item_colors(item, hovered, pressed);
            let flash_active = self.flash_index == Some(idx);

            cx.scene.draw_quad(
                Quad::new(item_bounds)
                    .with_background(bg)
                    .with_border(border, 1.0)
                    .with_corner_radius(4.0),
            );
            if flash_active {
                cx.scene.draw_quad(
                    Quad::new(item_bounds)
                        .with_background(theme::accent::PRIMARY.with_alpha(0.08))
                        .with_corner_radius(4.0),
                );
            }

            let icon_text = item.icon.as_str();
            if !icon_text.is_empty() {
                let icon_width = icon_text.len() as f32 * icon_font * 0.6;
                let icon_x = item_bounds.origin.x + (item_bounds.size.width - icon_width) * 0.5;
                let icon_y =
                    item_bounds.origin.y + item_bounds.size.height * 0.5 - icon_font * 0.55;
                let icon_run = cx.text.layout_styled_mono(
                    icon_text,
                    Point::new(icon_x, icon_y),
                    icon_font,
                    text_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(icon_run);
            }

            if !item.ghost {
                let overlay_width = number_font * 1.6;
                let overlay_height = number_font * 1.6;
                let overlay_x = item_bounds.origin.x + item_bounds.size.width - overlay_width - 2.0;
                let overlay_y =
                    item_bounds.origin.y + item_bounds.size.height - overlay_height - 2.0;
                let overlay_bounds =
                    Bounds::new(overlay_x, overlay_y, overlay_width, overlay_height);
                cx.scene.draw_quad(
                    Quad::new(overlay_bounds)
                        .with_background(theme::bg::APP.with_alpha(0.7))
                        .with_corner_radius(2.0),
                );
                let shortcut = format!("{}", item.slot);
                let text_width = shortcut.len() as f32 * number_font * 0.55;
                let text_x = overlay_bounds.origin.x + (overlay_bounds.size.width - text_width) * 0.5;
                let text_y =
                    overlay_bounds.origin.y + overlay_bounds.size.height * 0.5 - number_font * 0.5;
                let shortcut_run = cx.text.layout_styled_mono(
                    &shortcut,
                    Point::new(text_x, text_y),
                    number_font,
                    theme::text::MUTED,
                    FontStyle::default(),
                );
                cx.scene.draw_text(shortcut_run);
            }
        }
    }

    fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        if self.items.is_empty() {
            return EventResult::Ignored;
        }

        self.layout_items(bounds);

        let inside = if let InputEvent::MouseMove { x, y }
        | InputEvent::MouseDown { x, y, .. }
        | InputEvent::MouseUp { x, y, .. } = event
        {
            bounds.contains(Point::new(*x, *y))
        } else {
            false
        };

        if let InputEvent::MouseMove { x, y } = event {
            if inside {
                self.hovered_index = self.item_index_at(Point::new(*x, *y));
            } else {
                self.hovered_index = None;
            }
        }

        if let InputEvent::MouseDown { x, y, button, .. } = event {
            if !inside || *button != MouseButton::Left {
                return EventResult::Ignored;
            }
            let idx = self.item_index_at(Point::new(*x, *y));
            if let Some(index) = idx {
                if !self.items[index].ghost {
                    self.pressed_index = Some(index);
                    return EventResult::Handled;
                }
            }
        }

        if let InputEvent::MouseUp { x, y, button, .. } = event {
            if *button != MouseButton::Left {
                return EventResult::Ignored;
            }
            if let Some(pressed) = self.pressed_index.take() {
                let idx = self.item_index_at(Point::new(*x, *y));
                if idx == Some(pressed) && !self.items[pressed].ghost {
                    let slot = self.items[pressed].slot;
                    self.pending_clicks.push(slot);
                    return EventResult::Handled;
                }
            }
        }

        if inside {
            EventResult::Handled
        } else {
            EventResult::Ignored
        }
    }
}
