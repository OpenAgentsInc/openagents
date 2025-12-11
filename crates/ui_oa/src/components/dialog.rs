//! Dialog/Modal component

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, text, border};

/// Dialog component for modal content
///
/// Note: This renders inline. Full modal behavior would need
/// GPUI overlay/portal support for proper layering.
///
/// # Example
/// ```
/// Dialog::new()
///     .open(is_open)
///     .child(DialogHeader::new()
///         .child(DialogTitle::new("Edit Profile"))
///         .child(DialogDescription::new("Make changes to your profile.")))
///     .child(DialogContent::new().child(form_content))
///     .child(DialogFooter::new()
///         .child(Button::new("Cancel"))
///         .child(Button::new("Save")))
/// ```
#[derive(IntoElement)]
pub struct Dialog {
    children: Vec<AnyElement>,
    open: bool,
}

impl Dialog {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            open: false,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for Dialog {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Dialog {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if !self.open {
            return div();
        }

        // Backdrop
        let backdrop = div()
            .absolute()
            .inset_0()
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5));

        // Dialog box
        let mut dialog_box = div()
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

        for child in self.children {
            dialog_box = dialog_box.child(child);
        }

        div()
            .relative()
            .child(backdrop)
            .child(dialog_box)
    }
}

/// Dialog header section
#[derive(IntoElement)]
pub struct DialogHeader {
    children: Vec<AnyElement>,
}

impl DialogHeader {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for DialogHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DialogHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .p(px(16.0))
            .pb(px(8.0));

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Dialog title
#[derive(IntoElement)]
pub struct DialogTitle {
    text: SharedString,
}

impl DialogTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for DialogTitle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_lg()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text::PRIMARY)
            .child(self.text)
    }
}

/// Dialog description
#[derive(IntoElement)]
pub struct DialogDescription {
    text: SharedString,
}

impl DialogDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for DialogDescription {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .text_color(text::MUTED)
            .child(self.text)
    }
}

/// Dialog content section
#[derive(IntoElement)]
pub struct DialogContent {
    children: Vec<AnyElement>,
}

impl DialogContent {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for DialogContent {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DialogContent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div().px(px(16.0)).py(px(8.0));

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Dialog footer section
#[derive(IntoElement)]
pub struct DialogFooter {
    children: Vec<AnyElement>,
}

impl DialogFooter {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for DialogFooter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DialogFooter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .p(px(16.0))
            .pt(px(8.0));

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}
