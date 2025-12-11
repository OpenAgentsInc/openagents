//! Accordion component for collapsible sections

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, text, border};

/// A single accordion item
#[derive(IntoElement)]
pub struct AccordionItem {
    id: SharedString,
    trigger: SharedString,
    content: Option<AnyElement>,
    open: bool,
    on_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl AccordionItem {
    pub fn new(id: impl Into<SharedString>, trigger: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into(),
            content: None,
            open: false,
            on_toggle: None,
        }
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AccordionItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current_open = self.open;
        let chevron = if self.open { "▼" } else { "▶" };

        let mut container = div()
            .border_b_1()
            .border_color(border::DEFAULT);

        // Trigger row
        let mut trigger = div()
            .id(ElementId::Name(self.id))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .py(px(12.0))
            .cursor_pointer()
            .hover(|s| s.bg(bg::HOVER))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text::PRIMARY)
                    .child(self.trigger)
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text::MUTED)
                    .child(chevron)
            );

        if let Some(handler) = self.on_toggle {
            trigger = trigger.on_click(move |_, window, cx| {
                handler(!current_open, window, cx);
            });
        }

        container = container.child(trigger);

        // Content (shown when open)
        if self.open {
            if let Some(content) = self.content {
                container = container.child(
                    div()
                        .pb(px(12.0))
                        .text_sm()
                        .text_color(text::SECONDARY)
                        .child(content)
                );
            }
        }

        container
    }
}

/// Accordion container
///
/// # Example
/// ```
/// Accordion::new()
///     .child(AccordionItem::new("item1", "Section 1")
///         .content(div().child("Content 1"))
///         .open(true))
///     .child(AccordionItem::new("item2", "Section 2")
///         .content(div().child("Content 2")))
/// ```
#[derive(IntoElement)]
pub struct Accordion {
    children: Vec<AnyElement>,
}

impl Accordion {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for Accordion {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div().w_full();

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}
