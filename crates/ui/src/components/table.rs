//! Table component for data display

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Table container
///
/// # Example
/// ```
/// Table::new()
///     .child(TableHeader::new()
///         .child(TableRow::new()
///             .child(TableHead::new("Name"))
///             .child(TableHead::new("Status"))))
///     .child(TableBody::new()
///         .child(TableRow::new()
///             .child(TableCell::new("Item 1"))
///             .child(TableCell::new("Active"))))
/// ```
#[derive(IntoElement)]
pub struct Table {
    children: Vec<AnyElement>,
}

impl Table {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Table {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .w_full()
            .overflow_hidden()
            .rounded(px(8.0))
            .border_1()
            .border_color(border::DEFAULT);

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Table header section
#[derive(IntoElement)]
pub struct TableHeader {
    children: Vec<AnyElement>,
}

impl TableHeader {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for TableHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for TableHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div().bg(bg::ELEVATED);

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Table body section
#[derive(IntoElement)]
pub struct TableBody {
    children: Vec<AnyElement>,
}

impl TableBody {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for TableBody {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for TableBody {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div();

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Table row
#[derive(IntoElement)]
pub struct TableRow {
    children: Vec<AnyElement>,
}

impl TableRow {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for TableRow {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for TableRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .w_full()
            .border_b_1()
            .border_color(border::SUBTLE);

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Table header cell
#[derive(IntoElement)]
pub struct TableHead {
    text: SharedString,
}

impl TableHead {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for TableHead {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex_1()
            .px(px(12.0))
            .py(px(8.0))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(text::MUTED)
            .child(self.text)
    }
}

/// Table data cell
#[derive(IntoElement)]
pub struct TableCell {
    content: Option<AnyElement>,
}

impl TableCell {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            content: Some(div().child(text.into()).into_any_element()),
        }
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for TableCell {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex_1()
            .px(px(12.0))
            .py(px(8.0))
            .text_sm()
            .text_color(text::PRIMARY);

        if let Some(content) = self.content {
            el = el.child(content);
        }

        el
    }
}
