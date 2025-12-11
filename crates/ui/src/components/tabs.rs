//! Tabs component for tabbed navigation

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text};

/// A tab item definition
pub struct TabItem {
    pub id: SharedString,
    pub label: SharedString,
}

impl TabItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Tabs container component
///
/// # Example
/// ```
/// Tabs::new()
///     .tab(TabItem::new("tab1", "Account"))
///     .tab(TabItem::new("tab2", "Password"))
///     .active("tab1")
///     .on_change(|id, _, _| set_active(id))
/// ```
#[derive(IntoElement)]
pub struct Tabs {
    tabs: Vec<TabItem>,
    active: Option<SharedString>,
    on_change: Option<Box<dyn Fn(SharedString, &mut Window, &mut App) + 'static>>,
}

impl Tabs {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            on_change: None,
        }
    }

    pub fn tab(mut self, item: TabItem) -> Self {
        self.tabs.push(item);
        self
    }

    pub fn active(mut self, id: impl Into<SharedString>) -> Self {
        self.active = Some(id.into());
        self
    }

    pub fn on_change(mut self, handler: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(4.0))
            .rounded(px(6.0))
            .bg(bg::ELEVATED);

        let active_id = self.active.clone();
        let on_change = self.on_change;

        for tab in self.tabs {
            let is_active = active_id.as_ref().map(|a| a == &tab.id).unwrap_or(false);
            let tab_id = tab.id.clone();

            let mut tab_el = div()
                .id(ElementId::Name(tab.id))
                .px(px(12.0))
                .py(px(6.0))
                .rounded(px(4.0))
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer();

            if is_active {
                tab_el = tab_el
                    .bg(bg::SURFACE)
                    .text_color(text::PRIMARY);
            } else {
                tab_el = tab_el
                    .text_color(text::MUTED)
                    .hover(|s| s.text_color(text::PRIMARY));
            }

            if let Some(ref handler) = on_change {
                let handler = handler.as_ref() as *const _;
                let id_for_handler = tab_id.clone();
                tab_el = tab_el.on_click(move |_, window, cx| {
                    unsafe {
                        let handler: &dyn Fn(SharedString, &mut Window, &mut App) = &*handler;
                        handler(id_for_handler.clone(), window, cx);
                    }
                });
            }

            tab_el = tab_el.child(tab.label);
            container = container.child(tab_el);
        }

        container
    }
}

/// Tab content panel - shows content for active tab
#[derive(IntoElement)]
pub struct TabsContent {
    id: SharedString,
    active: SharedString,
    content: Option<AnyElement>,
}

impl TabsContent {
    pub fn new(id: impl Into<SharedString>, active: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            active: active.into(),
            content: None,
        }
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for TabsContent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if self.id == self.active {
            let mut el = div().pt(px(16.0));
            if let Some(content) = self.content {
                el = el.child(content);
            }
            el
        } else {
            div()
        }
    }
}
