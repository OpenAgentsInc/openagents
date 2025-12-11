//! Command palette component

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Command item
pub struct CommandItem {
    pub id: SharedString,
    pub label: SharedString,
    pub shortcut: Option<SharedString>,
    pub group: Option<SharedString>,
}

impl CommandItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            group: None,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }
}

/// Command palette component
///
/// # Example
/// ```
/// Command::new()
///     .open(is_open)
///     .placeholder("Type a command or search...")
///     .item(CommandItem::new("new-file", "New File").shortcut("⌘N").group("File"))
///     .item(CommandItem::new("open-file", "Open File").shortcut("⌘O").group("File"))
///     .item(CommandItem::new("settings", "Settings").shortcut("⌘,").group("Preferences"))
///     .on_select(|id, _, _| handle_command(id))
/// ```
#[derive(IntoElement)]
pub struct Command {
    items: Vec<CommandItem>,
    open: bool,
    placeholder: SharedString,
    search: SharedString,
    on_select: Option<Box<dyn Fn(SharedString, &mut Window, &mut App) + 'static>>,
}

impl Command {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            open: false,
            placeholder: "Type a command...".into(),
            search: "".into(),
            on_select: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn search(mut self, text: impl Into<SharedString>) -> Self {
        self.search = text.into();
        self
    }

    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn on_select(mut self, handler: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }
}

impl Default for Command {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Command {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if !self.open {
            return div();
        }

        // Backdrop
        let backdrop = div()
            .absolute()
            .inset_0()
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5));

        // Command dialog
        let mut dialog = div()
            .absolute()
            .top(px(100.0))
            .left(px(50.0))
            .right(px(50.0))
            .max_w(px(500.0))
            .mx_auto()
            .rounded(px(8.0))
            .border_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .overflow_hidden();

        // Search input
        let search_input = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .child(
                div()
                    .text_sm()
                    .text_color(text::MUTED)
                    .child("⌘")
            )
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(if self.search.is_empty() { text::MUTED } else { text::PRIMARY })
                    .child(if self.search.is_empty() { self.placeholder.clone() } else { self.search.clone() })
            );

        dialog = dialog.child(search_input);

        // Group items by group
        let mut current_group: Option<SharedString> = None;
        let mut items_container = div()
            .max_h(px(300.0))
            .overflow_hidden()
            .py(px(4.0));

        for item in self.items {
            // Group header
            if item.group != current_group {
                if let Some(ref group) = item.group {
                    items_container = items_container.child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text::MUTED)
                            .child(group.clone())
                    );
                }
                current_group = item.group.clone();
            }

            // Item
            let item_id = item.id.clone();
            let mut item_el = div()
                .id(ElementId::Name(item.id))
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .py(px(8.0))
                .cursor_pointer()
                .hover(|s| s.bg(bg::HOVER));

            if let Some(ref handler) = self.on_select {
                let handler_ptr = handler.as_ref() as *const _;
                item_el = item_el.on_click(move |_, window, cx| {
                    unsafe {
                        let handler: &dyn Fn(SharedString, &mut Window, &mut App) = &*handler_ptr;
                        handler(item_id.clone(), window, cx);
                    }
                });
            }

            item_el = item_el.child(
                div()
                    .text_sm()
                    .text_color(text::PRIMARY)
                    .child(item.label)
            );

            if let Some(shortcut) = item.shortcut {
                item_el = item_el.child(
                    div()
                        .text_xs()
                        .text_color(text::MUTED)
                        .child(shortcut)
                );
            }

            items_container = items_container.child(item_el);
        }

        dialog = dialog.child(items_container);

        // Footer hint
        let footer = div()
            .flex()
            .items_center()
            .gap(px(16.0))
            .px(px(12.0))
            .py(px(8.0))
            .border_t_1()
            .border_color(border::DEFAULT)
            .text_xs()
            .text_color(text::DIM)
            .child(div().child("↑↓ Navigate"))
            .child(div().child("↵ Select"))
            .child(div().child("esc Close"));

        dialog = dialog.child(footer);

        div()
            .relative()
            .child(backdrop)
            .child(dialog)
    }
}
