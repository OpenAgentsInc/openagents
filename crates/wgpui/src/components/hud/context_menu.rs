//! Context Menu Component
//!
//! Provides a right-click context menu with hierarchical items.

use crate::components::{Component, ComponentId, EventContext, EventResult, PaintContext};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, Key, MouseButton, Point, Quad, theme};

/// Menu item separator
pub const SEPARATOR: &str = "---";

/// A single menu item
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Unique identifier
    pub id: String,
    /// Display label
    pub label: String,
    /// Keyboard shortcut hint
    pub shortcut: Option<String>,
    /// Whether item is disabled
    pub disabled: bool,
    /// Whether item is checked (for toggleable items)
    pub checked: Option<bool>,
    /// Submenu items
    pub submenu: Vec<MenuItem>,
    /// Whether this is a separator
    pub is_separator: bool,
    /// Icon (optional, could be a character or identifier)
    pub icon: Option<String>,
}

impl MenuItem {
    /// Create a new menu item
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: Vec::new(),
            is_separator: false,
            icon: None,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self {
            id: SEPARATOR.to_string(),
            label: String::new(),
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: Vec::new(),
            is_separator: true,
            icon: None,
        }
    }

    /// Set keyboard shortcut
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set checked state
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Set icon
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Add submenu items
    pub fn submenu(mut self, items: Vec<MenuItem>) -> Self {
        self.submenu = items;
        self
    }

    /// Check if item has submenu
    pub fn has_submenu(&self) -> bool {
        !self.submenu.is_empty()
    }
}

/// Context menu component
#[derive(Debug, Clone)]
pub struct ContextMenu {
    id: Option<ComponentId>,
    /// Menu items
    items: Vec<MenuItem>,
    /// Whether menu is visible
    visible: bool,
    /// Position where menu was opened
    position: Point,
    /// Currently selected item index
    selected: Option<usize>,
    /// Currently hovered item index
    hovered: Option<usize>,
    /// Open submenu index
    open_submenu: Option<usize>,
    /// Item height
    item_height: f32,
    /// Separator height
    separator_height: f32,
    /// Padding
    padding: f32,
    /// Minimum width
    min_width: f32,
    /// Maximum width
    max_width: f32,
    /// Last selected item ID (for callbacks)
    last_selected: Option<String>,
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextMenu {
    /// Create a new context menu
    pub fn new() -> Self {
        Self {
            id: None,
            items: Vec::new(),
            visible: false,
            position: Point::new(0.0, 0.0),
            selected: None,
            hovered: None,
            open_submenu: None,
            item_height: 32.0,
            separator_height: 9.0,
            padding: 6.0,
            min_width: 180.0,
            max_width: 320.0,
            last_selected: None,
        }
    }

    /// Set component ID
    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set menu items
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    /// Add a single item
    pub fn add_item(&mut self, item: MenuItem) {
        self.items.push(item);
    }

    /// Set minimum width
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Open the menu at a position
    pub fn open(&mut self, position: Point) {
        self.visible = true;
        self.position = position;
        self.selected = None;
        self.hovered = None;
        self.open_submenu = None;
        self.last_selected = None;
    }

    /// Close the menu
    pub fn close(&mut self) {
        self.visible = false;
        self.selected = None;
        self.hovered = None;
        self.open_submenu = None;
    }

    /// Check if menu is open
    pub fn is_open(&self) -> bool {
        self.visible
    }

    /// Get last selected item ID (and clear it)
    pub fn take_selected(&mut self) -> Option<String> {
        self.last_selected.take()
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let selectable: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.is_separator && !item.disabled)
            .map(|(i, _)| i)
            .collect();

        if selectable.is_empty() {
            return;
        }

        let current = self.selected.unwrap_or(selectable[0]);
        let current_pos = selectable.iter().position(|&i| i == current).unwrap_or(0);
        let new_pos = if current_pos == 0 {
            selectable.len() - 1
        } else {
            current_pos - 1
        };
        self.selected = Some(selectable[new_pos]);
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let selectable: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.is_separator && !item.disabled)
            .map(|(i, _)| i)
            .collect();

        if selectable.is_empty() {
            return;
        }

        let current = self.selected.unwrap_or(selectable[selectable.len() - 1]);
        let current_pos = selectable
            .iter()
            .position(|&i| i == current)
            .unwrap_or(selectable.len() - 1);
        let new_pos = (current_pos + 1) % selectable.len();
        self.selected = Some(selectable[new_pos]);
    }

    /// Confirm selection
    pub fn confirm(&mut self) -> Option<String> {
        if let Some(idx) = self.selected
            && let Some(item) = self.items.get(idx)
            && !item.disabled
            && !item.is_separator
        {
            if item.has_submenu() {
                self.open_submenu = Some(idx);
                return None;
            }
            let id = item.id.clone();
            self.last_selected = Some(id.clone());
            self.close();
            return Some(id);
        }
        None
    }

    /// Calculate menu bounds
    fn calculate_bounds(&self, viewport: Bounds) -> Bounds {
        let mut width = self.min_width;

        // Monospace char width at font_size::SM (14)
        let mono_char_width = 8.4;

        // Calculate width based on longest item
        for item in &self.items {
            let label_width = item.label.len() as f32 * mono_char_width;
            let shortcut_width = item
                .shortcut
                .as_ref()
                .map(|s| s.len() as f32 * mono_char_width + 32.0) // gap between label and shortcut
                .unwrap_or(0.0);
            // 24 for left padding + 16 for right padding + checkbox/icon space
            let item_width = label_width + shortcut_width + 48.0;
            width = width.max(item_width);
        }
        width = width.min(self.max_width);

        // Calculate height
        let height: f32 = self
            .items
            .iter()
            .map(|item| {
                if item.is_separator {
                    self.separator_height
                } else {
                    self.item_height
                }
            })
            .sum::<f32>()
            + self.padding * 2.0;

        // Adjust position to stay within viewport
        let mut x = self.position.x;
        let mut y = self.position.y;

        if x + width > viewport.origin.x + viewport.size.width {
            x = viewport.origin.x + viewport.size.width - width;
        }
        if y + height > viewport.origin.y + viewport.size.height {
            y = viewport.origin.y + viewport.size.height - height;
        }
        x = x.max(viewport.origin.x);
        y = y.max(viewport.origin.y);

        Bounds::new(x, y, width, height)
    }

    /// Get item at point
    fn item_at_point(&self, point: Point, menu_bounds: Bounds) -> Option<usize> {
        if !menu_bounds.contains(point) {
            return None;
        }

        let mut y = menu_bounds.origin.y + self.padding;
        for (i, item) in self.items.iter().enumerate() {
            let height = if item.is_separator {
                self.separator_height
            } else {
                self.item_height
            };
            if point.y >= y && point.y < y + height && !item.is_separator {
                return Some(i);
            }
            y += height;
        }
        None
    }
}

impl Component for ContextMenu {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.visible {
            return;
        }

        let menu_bounds = self.calculate_bounds(bounds);

        // Draw shadow/backdrop first for depth
        let shadow_offset = 4.0;
        let shadow_bounds = Bounds::new(
            menu_bounds.origin.x + shadow_offset,
            menu_bounds.origin.y + shadow_offset,
            menu_bounds.size.width,
            menu_bounds.size.height,
        );
        cx.scene.draw_quad(
            Quad::new(shadow_bounds)
                .with_background(Hsla::new(0.0, 0.0, 0.0, 0.5))
                .with_corner_radius(4.0),
        );

        // Draw solid menu background - #0a0a0a
        let menu_bg = Hsla::new(0.0, 0.0, 0.039, 1.0); // #0a0a0a
        let menu_border = Hsla::new(0.0, 0.0, 0.2, 1.0);
        cx.scene.draw_quad(
            Quad::new(menu_bounds)
                .with_background(menu_bg)
                .with_border(menu_border, 1.0)
                .with_corner_radius(4.0),
        );

        // Draw items
        let mut y = menu_bounds.origin.y + self.padding;
        let content_width = menu_bounds.size.width - self.padding * 2.0;

        for (i, item) in self.items.iter().enumerate() {
            if item.is_separator {
                // Just add spacing, no visible line
                y += self.separator_height;
                continue;
            }

            let is_selected = self.selected == Some(i) || self.hovered == Some(i);
            let item_bounds = Bounds::new(
                menu_bounds.origin.x + self.padding,
                y,
                content_width,
                self.item_height,
            );

            // Draw selection highlight
            if is_selected && !item.disabled {
                let hover_bg = Hsla::new(0.0, 0.0, 0.22, 1.0); // lighter than menu bg
                cx.scene.draw_quad(
                    Quad::new(item_bounds)
                        .with_background(hover_bg)
                        .with_corner_radius(3.0),
                );
            }

            // Draw checkbox/radio if present
            let mut text_x = item_bounds.origin.x + 12.0;
            let text_y = y + (self.item_height - theme::font_size::SM) / 2.0;

            if let Some(checked) = item.checked {
                let check_char = if checked { "✓" } else { " " };
                let check_color = if item.disabled {
                    theme::text::MUTED
                } else {
                    theme::accent::PRIMARY
                };
                let check_run = cx.text.layout_styled_mono(
                    check_char,
                    Point::new(text_x, text_y),
                    theme::font_size::SM,
                    check_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(check_run);
                text_x += 20.0;
            }

            // Draw icon if present
            if let Some(ref icon) = item.icon {
                let icon_color = if item.disabled {
                    theme::text::MUTED
                } else {
                    theme::text::SECONDARY
                };
                let icon_run = cx.text.layout_styled_mono(
                    icon,
                    Point::new(text_x, text_y),
                    theme::font_size::SM,
                    icon_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(icon_run);
                text_x += 20.0;
            }

            // Draw label
            let text_color = if item.disabled {
                theme::text::MUTED
            } else {
                theme::text::PRIMARY
            };
            let label_run = cx.text.layout_styled_mono(
                &item.label,
                Point::new(text_x, text_y),
                theme::font_size::SM,
                text_color,
                FontStyle::default(),
            );
            cx.scene.draw_text(label_run);

            // Draw shortcut on right side
            if let Some(ref shortcut) = item.shortcut {
                let shortcut_x = item_bounds.origin.x + item_bounds.size.width
                    - 12.0
                    - shortcut.len() as f32 * 7.5;
                let shortcut_run = cx.text.layout_styled_mono(
                    shortcut,
                    Point::new(shortcut_x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                    FontStyle::default(),
                );
                cx.scene.draw_text(shortcut_run);
            }

            // Draw submenu arrow if has submenu
            if item.has_submenu() {
                let arrow_x = item_bounds.origin.x + item_bounds.size.width - 18.0;
                let arrow_color = if item.disabled {
                    theme::text::MUTED
                } else {
                    theme::text::PRIMARY
                };
                let arrow_run = cx.text.layout_styled_mono(
                    "›",
                    Point::new(arrow_x, text_y),
                    theme::font_size::SM,
                    arrow_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(arrow_run);
            }

            y += self.item_height;
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }

        let menu_bounds = self.calculate_bounds(bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let position = Point::new(*x, *y);
                self.hovered = self.item_at_point(position, menu_bounds);
                if self.hovered.is_some() {
                    self.selected = self.hovered;
                }
                EventResult::Handled
            }
            InputEvent::MouseDown { x, y, button, .. } => {
                let position = Point::new(*x, *y);
                if !menu_bounds.contains(position) {
                    self.close();
                    return EventResult::Handled;
                }

                if *button == MouseButton::Left
                    && let Some(idx) = self.item_at_point(position, menu_bounds)
                {
                    self.selected = Some(idx);
                    self.confirm();
                }
                EventResult::Handled
            }
            InputEvent::KeyDown { key, .. } => {
                match key {
                    Key::Named(crate::NamedKey::Escape) => {
                        self.close();
                        EventResult::Handled
                    }
                    Key::Named(crate::NamedKey::ArrowUp) => {
                        self.select_prev();
                        EventResult::Handled
                    }
                    Key::Named(crate::NamedKey::ArrowDown) => {
                        self.select_next();
                        EventResult::Handled
                    }
                    Key::Named(crate::NamedKey::Enter) => {
                        self.confirm();
                        EventResult::Handled
                    }
                    Key::Named(crate::NamedKey::ArrowRight) => {
                        // Open submenu
                        if let Some(idx) = self.selected
                            && self
                                .items
                                .get(idx)
                                .map(|i| i.has_submenu())
                                .unwrap_or(false)
                        {
                            self.open_submenu = Some(idx);
                        }
                        EventResult::Handled
                    }
                    Key::Named(crate::NamedKey::ArrowLeft) => {
                        // Close submenu
                        self.open_submenu = None;
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }
            _ => EventResult::Ignored,
        }
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, EventContext, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point};

    #[test]
    fn test_menu_item_creation() {
        let item = MenuItem::new("edit.copy", "Copy")
            .shortcut("Cmd+C")
            .icon("📋");

        assert_eq!(item.id, "edit.copy");
        assert_eq!(item.label, "Copy");
        assert_eq!(item.shortcut, Some("Cmd+C".to_string()));
        assert!(!item.disabled);
    }

    #[test]
    fn test_separator() {
        let sep = MenuItem::separator();
        assert!(sep.is_separator);
    }

    #[test]
    fn test_submenu() {
        let item = MenuItem::new("file", "File").submenu(vec![
            MenuItem::new("file.new", "New"),
            MenuItem::new("file.open", "Open"),
        ]);

        assert!(item.has_submenu());
        assert_eq!(item.submenu.len(), 2);
    }

    #[test]
    fn test_context_menu_open_close() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("copy", "Copy"),
            MenuItem::new("paste", "Paste"),
        ]);

        assert!(!menu.is_open());

        menu.open(Point::new(100.0, 100.0));
        assert!(menu.is_open());
        assert_eq!(menu.position, Point::new(100.0, 100.0));

        menu.close();
        assert!(!menu.is_open());
    }

    #[test]
    fn test_context_menu_navigation() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("a", "Item A"),
            MenuItem::separator(),
            MenuItem::new("b", "Item B"),
            MenuItem::new("c", "Item C").disabled(true),
            MenuItem::new("d", "Item D"),
        ]);
        menu.open(Point::new(0.0, 0.0));

        // Select next should skip separator and disabled
        menu.select_next();
        assert_eq!(menu.selected, Some(0)); // Item A

        menu.select_next();
        assert_eq!(menu.selected, Some(2)); // Item B (skips separator)

        menu.select_next();
        assert_eq!(menu.selected, Some(4)); // Item D (skips disabled C)

        menu.select_next();
        assert_eq!(menu.selected, Some(0)); // Wrap to A

        menu.select_prev();
        assert_eq!(menu.selected, Some(4)); // Item D
    }

    #[test]
    fn test_context_menu_confirm() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("copy", "Copy"),
            MenuItem::new("paste", "Paste"),
        ]);
        menu.open(Point::new(0.0, 0.0));

        menu.selected = Some(0);
        let result = menu.confirm();

        assert_eq!(result, Some("copy".to_string()));
        assert!(!menu.is_open());
    }

    #[test]
    fn test_context_menu_disabled_confirm() {
        let mut menu =
            ContextMenu::new().items(vec![MenuItem::new("disabled", "Disabled").disabled(true)]);
        menu.open(Point::new(0.0, 0.0));

        menu.selected = Some(0);
        let result = menu.confirm();

        assert_eq!(result, None);
        assert!(menu.is_open()); // Should stay open
    }

    #[test]
    fn test_checked_item() {
        let item = MenuItem::new("toggle", "Toggle Option").checked(true);
        assert_eq!(item.checked, Some(true));
    }

    #[test]
    fn test_context_menu_bounds_clamp_to_viewport() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("long", "Very Long Menu Item"),
            MenuItem::new("short", "Short"),
        ]);
        let viewport = Bounds::new(0.0, 0.0, 300.0, 200.0);

        menu.open(Point::new(290.0, 190.0));
        let bounds = menu.calculate_bounds(viewport);

        assert!(bounds.origin.x >= viewport.origin.x);
        assert!(bounds.origin.y >= viewport.origin.y);
        assert!(bounds.origin.x + bounds.size.width <= viewport.origin.x + viewport.size.width);
        assert!(bounds.origin.y + bounds.size.height <= viewport.origin.y + viewport.size.height);
    }

    #[test]
    fn test_context_menu_item_at_point_skips_separator() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("a", "Item A"),
            MenuItem::separator(),
            MenuItem::new("b", "Item B"),
        ]);
        let viewport = Bounds::new(0.0, 0.0, 200.0, 200.0);
        menu.open(Point::new(10.0, 10.0));
        let menu_bounds = menu.calculate_bounds(viewport);

        let separator_y =
            menu_bounds.origin.y + menu.padding + menu.item_height + menu.separator_height / 2.0;
        let sep_point = Point::new(menu_bounds.origin.x + menu.padding + 1.0, separator_y);
        assert!(menu.item_at_point(sep_point, menu_bounds).is_none());

        let item_b_y = menu_bounds.origin.y
            + menu.padding
            + menu.item_height
            + menu.separator_height
            + menu.item_height / 2.0;
        let item_b_point = Point::new(menu_bounds.origin.x + menu.padding + 1.0, item_b_y);
        assert_eq!(menu.item_at_point(item_b_point, menu_bounds), Some(2));
    }

    #[test]
    fn test_context_menu_mouse_hover_and_click() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("a", "Item A"),
            MenuItem::new("b", "Item B"),
        ]);
        let viewport = Bounds::new(0.0, 0.0, 300.0, 200.0);
        menu.open(Point::new(20.0, 20.0));
        let menu_bounds = menu.calculate_bounds(viewport);

        let point = Point::new(
            menu_bounds.origin.x + menu.padding + 2.0,
            menu_bounds.origin.y + menu.padding + menu.item_height / 2.0,
        );

        let mut cx = EventContext::new();
        let move_event = InputEvent::MouseMove {
            x: point.x,
            y: point.y,
        };
        let result = menu.event(&move_event, viewport, &mut cx);
        assert_eq!(result, EventResult::Handled);
        assert_eq!(menu.hovered, Some(0));
        assert_eq!(menu.selected, Some(0));

        let click_event = InputEvent::MouseDown { button: MouseButton::Left, x: point.x, y: point.y, modifiers: Modifiers::default() };
        menu.event(&click_event, viewport, &mut cx);
        assert!(!menu.is_open());
        assert_eq!(menu.take_selected(), Some("a".to_string()));
    }

    #[test]
    fn test_context_menu_mouse_click_outside_closes() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("a", "Item A"),
            MenuItem::new("b", "Item B"),
        ]);
        let viewport = Bounds::new(0.0, 0.0, 300.0, 200.0);
        menu.open(Point::new(30.0, 30.0));

        let mut cx = EventContext::new();
        let click_event = InputEvent::MouseDown { button: MouseButton::Left, x: 0.0, y: 0.0, modifiers: Modifiers::default() };
        let result = menu.event(&click_event, viewport, &mut cx);
        assert_eq!(result, EventResult::Handled);
        assert!(!menu.is_open());
    }

    #[test]
    fn test_context_menu_keyboard_submenu_toggle() {
        let mut menu = ContextMenu::new().items(vec![
            MenuItem::new("parent", "Parent").submenu(vec![MenuItem::new("child", "Child")]),
            MenuItem::new("solo", "Solo"),
        ]);
        let viewport = Bounds::new(0.0, 0.0, 300.0, 200.0);
        menu.open(Point::new(10.0, 10.0));
        menu.selected = Some(0);

        let mut cx = EventContext::new();
        let right = InputEvent::KeyDown {
            key: Key::Named(NamedKey::ArrowRight),
            modifiers: Modifiers::default(),
        };
        menu.event(&right, viewport, &mut cx);
        assert_eq!(menu.open_submenu, Some(0));

        let left = InputEvent::KeyDown {
            key: Key::Named(NamedKey::ArrowLeft),
            modifiers: Modifiers::default(),
        };
        menu.event(&left, viewport, &mut cx);
        assert!(menu.open_submenu.is_none());
    }

    #[test]
    fn test_context_menu_escape_closes() {
        let mut menu = ContextMenu::new().items(vec![MenuItem::new("a", "Item A")]);
        let viewport = Bounds::new(0.0, 0.0, 300.0, 200.0);
        menu.open(Point::new(10.0, 10.0));
        let mut cx = EventContext::new();
        let escape = InputEvent::KeyDown {
            key: Key::Named(NamedKey::Escape),
            modifiers: Modifiers::default(),
        };
        menu.event(&escape, viewport, &mut cx);
        assert!(!menu.is_open());
    }
}
