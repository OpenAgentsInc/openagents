//! Card component with sub-components

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// A card container component
///
/// # Example
/// ```
/// Card::new()
///     .child(CardHeader::new()
///         .child(CardTitle::new("Card Title"))
///         .child(CardDescription::new("Card description")))
///     .child(CardContent::new().child("Content here"))
///     .child(CardFooter::new().child(Button::new("Action")))
/// ```
#[derive(IntoElement)]
pub struct Card {
    children: Vec<AnyElement>,
}

impl Card {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .rounded(px(8.0))
            .border_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .overflow_hidden();

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Card header section
#[derive(IntoElement)]
pub struct CardHeader {
    children: Vec<AnyElement>,
}

impl CardHeader {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for CardHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for CardHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .p(px(16.0));

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Card title
#[derive(IntoElement)]
pub struct CardTitle {
    text: SharedString,
}

impl CardTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for CardTitle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_base()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text::PRIMARY)
            .child(self.text)
    }
}

/// Card description
#[derive(IntoElement)]
pub struct CardDescription {
    text: SharedString,
}

impl CardDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for CardDescription {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .text_color(text::MUTED)
            .child(self.text)
    }
}

/// Card content section
#[derive(IntoElement)]
pub struct CardContent {
    children: Vec<AnyElement>,
}

impl CardContent {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for CardContent {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for CardContent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div().px(px(16.0)).pb(px(16.0));

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Card footer section
#[derive(IntoElement)]
pub struct CardFooter {
    children: Vec<AnyElement>,
}

impl CardFooter {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for CardFooter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for CardFooter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(16.0))
            .pb(px(16.0));

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}
