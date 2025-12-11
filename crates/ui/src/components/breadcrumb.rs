//! Breadcrumb navigation component

use gpui::prelude::*;
use gpui::*;
use theme::text;

/// A breadcrumb item
pub struct BreadcrumbItem {
    pub label: SharedString,
    pub href: Option<SharedString>,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            href: None,
        }
    }

    pub fn href(mut self, href: impl Into<SharedString>) -> Self {
        self.href = Some(href.into());
        self
    }
}

/// Breadcrumb navigation
///
/// # Example
/// ```
/// Breadcrumb::new()
///     .item(BreadcrumbItem::new("Home").href("/"))
///     .item(BreadcrumbItem::new("Products").href("/products"))
///     .item(BreadcrumbItem::new("Widget"))  // Current page, no href
/// ```
#[derive(IntoElement)]
pub struct Breadcrumb {
    items: Vec<BreadcrumbItem>,
    separator: SharedString,
    on_navigate: Option<Box<dyn Fn(SharedString, &mut Window, &mut App) + 'static>>,
}

impl Breadcrumb {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            separator: "/".into(),
            on_navigate: None,
        }
    }

    pub fn item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn separator(mut self, sep: impl Into<SharedString>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn on_navigate(mut self, handler: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_navigate = Some(Box::new(handler));
        self
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_sm();

        let total = self.items.len();
        let on_navigate = self.on_navigate;

        for (i, item) in self.items.into_iter().enumerate() {
            let is_last = i == total - 1;

            // Add separator before item (except first)
            if i > 0 {
                container = container.child(
                    div()
                        .text_color(text::DIM)
                        .child(self.separator.clone())
                );
            }

            // Breadcrumb item
            if is_last || item.href.is_none() {
                // Current page (no link)
                container = container.child(
                    div()
                        .text_color(text::PRIMARY)
                        .font_weight(FontWeight::MEDIUM)
                        .child(item.label)
                );
            } else {
                // Link item
                let href = item.href.clone().unwrap_or_default();
                let mut link = div()
                    .id(ElementId::Name(format!("breadcrumb-{}", i).into()))
                    .text_color(text::MUTED)
                    .cursor_pointer()
                    .hover(|s| s.text_color(text::PRIMARY))
                    .child(item.label);

                if let Some(ref handler) = on_navigate {
                    let handler = handler.as_ref() as *const _;
                    let href_for_handler = href.clone();
                    link = link.on_click(move |_, window, cx| {
                        unsafe {
                            let handler: &dyn Fn(SharedString, &mut Window, &mut App) = &*handler;
                            handler(href_for_handler.clone(), window, cx);
                        }
                    });
                }

                container = container.child(link);
            }
        }

        container
    }
}
