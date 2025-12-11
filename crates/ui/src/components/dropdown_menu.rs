//! Dropdown menu component

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Menu item type
pub enum DropdownMenuItem {
    Item {
        label: SharedString,
        shortcut: Option<SharedString>,
        disabled: bool,
        on_select: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    },
    Separator,
    Label(SharedString),
}

impl DropdownMenuItem {
    pub fn item(label: impl Into<SharedString>) -> Self {
        Self::Item {
            label: label.into(),
            shortcut: None,
            disabled: false,
            on_select: None,
        }
    }

    pub fn shortcut(self, shortcut: impl Into<SharedString>) -> Self {
        match self {
            Self::Item { label, disabled, on_select, .. } => Self::Item {
                label,
                shortcut: Some(shortcut.into()),
                disabled,
                on_select,
            },
            other => other,
        }
    }

    pub fn disabled(self, disabled: bool) -> Self {
        match self {
            Self::Item { label, shortcut, on_select, .. } => Self::Item {
                label,
                shortcut,
                disabled,
                on_select,
            },
            other => other,
        }
    }

    pub fn on_select(self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        match self {
            Self::Item { label, shortcut, disabled, .. } => Self::Item {
                label,
                shortcut,
                disabled,
                on_select: Some(Box::new(handler)),
            },
            other => other,
        }
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn label(text: impl Into<SharedString>) -> Self {
        Self::Label(text.into())
    }
}

/// Dropdown menu component
///
/// # Example
/// ```
/// DropdownMenu::new()
///     .trigger(Button::new("Open Menu"))
///     .open(is_open)
///     .item(DropdownMenuItem::item("Profile").shortcut("⌘P"))
///     .item(DropdownMenuItem::item("Settings").shortcut("⌘,"))
///     .item(DropdownMenuItem::separator())
///     .item(DropdownMenuItem::item("Log out"))
/// ```
#[derive(IntoElement)]
pub struct DropdownMenu {
    trigger: Option<AnyElement>,
    items: Vec<DropdownMenuItem>,
    open: bool,
}

impl DropdownMenu {
    pub fn new() -> Self {
        Self {
            trigger: None,
            items: Vec::new(),
            open: false,
        }
    }

    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn item(mut self, item: DropdownMenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
}

impl Default for DropdownMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DropdownMenu {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div().relative();

        // Trigger
        if let Some(trigger) = self.trigger {
            container = container.child(trigger);
        }

        // Menu dropdown
        if self.open {
            let mut menu = div()
                .absolute()
                .top(px(40.0))
                .left_0()
                .min_w(px(180.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(border::DEFAULT)
                .bg(bg::ELEVATED);

            for (i, item) in self.items.into_iter().enumerate() {
                match item {
                    DropdownMenuItem::Item { label, shortcut, disabled, on_select } => {
                        let mut item_el = div()
                            .id(ElementId::Name(format!("menu-item-{}", i).into()))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(12.0))
                            .py(px(6.0))
                            .text_sm();

                        if disabled {
                            item_el = item_el
                                .opacity(0.5)
                                .cursor_not_allowed()
                                .text_color(text::DISABLED);
                        } else {
                            item_el = item_el
                                .cursor_pointer()
                                .text_color(text::PRIMARY)
                                .hover(|s| s.bg(bg::HOVER));

                            if let Some(handler) = on_select {
                                item_el = item_el.on_click(move |_, window, cx| {
                                    handler(window, cx);
                                });
                            }
                        }

                        item_el = item_el.child(div().child(label));

                        if let Some(sc) = shortcut {
                            item_el = item_el.child(
                                div()
                                    .text_xs()
                                    .text_color(text::MUTED)
                                    .child(sc)
                            );
                        }

                        menu = menu.child(item_el);
                    }
                    DropdownMenuItem::Separator => {
                        menu = menu.child(
                            div()
                                .my(px(4.0))
                                .h(px(1.0))
                                .bg(border::DEFAULT)
                        );
                    }
                    DropdownMenuItem::Label(text) => {
                        menu = menu.child(
                            div()
                                .px(px(12.0))
                                .py(px(6.0))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text::MUTED)
                                .child(text)
                        );
                    }
                }
            }

            container = container.child(menu);
        }

        container
    }
}
