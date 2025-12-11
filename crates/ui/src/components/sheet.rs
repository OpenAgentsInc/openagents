//! Sheet component for side panels

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Sheet side/position
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SheetSide {
    #[default]
    Right,
    Left,
    Top,
    Bottom,
}

/// Sheet/side panel component
///
/// # Example
/// ```
/// Sheet::new()
///     .open(is_open)
///     .side(SheetSide::Right)
///     .child(SheetHeader::new()
///         .child(SheetTitle::new("Edit Profile")))
///     .child(SheetContent::new().child(form_content))
/// ```
#[derive(IntoElement)]
pub struct Sheet {
    children: Vec<AnyElement>,
    open: bool,
    side: SheetSide,
}

impl Sheet {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            open: false,
            side: SheetSide::Right,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn side(mut self, side: SheetSide) -> Self {
        self.side = side;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Sheet {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if !self.open {
            return div();
        }

        // Backdrop
        let backdrop = div()
            .absolute()
            .inset_0()
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5));

        // Sheet panel
        let mut panel = div()
            .absolute()
            .bg(bg::SURFACE)
            .border_color(border::DEFAULT);

        // Position and size based on side
        panel = match self.side {
            SheetSide::Right => panel
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(350.0))
                .border_l_1(),
            SheetSide::Left => panel
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(350.0))
                .border_r_1(),
            SheetSide::Top => panel
                .top_0()
                .left_0()
                .right_0()
                .h(px(300.0))
                .border_b_1(),
            SheetSide::Bottom => panel
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(300.0))
                .border_t_1(),
        };

        for child in self.children {
            panel = panel.child(child);
        }

        div()
            .relative()
            .w_full()
            .h_full()
            .child(backdrop)
            .child(panel)
    }
}

/// Sheet header section
#[derive(IntoElement)]
pub struct SheetHeader {
    children: Vec<AnyElement>,
}

impl SheetHeader {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for SheetHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for SheetHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .p(px(16.0))
            .border_b_1()
            .border_color(border::DEFAULT);

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Sheet title
#[derive(IntoElement)]
pub struct SheetTitle {
    text: SharedString,
}

impl SheetTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for SheetTitle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_lg()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text::PRIMARY)
            .child(self.text)
    }
}

/// Sheet description
#[derive(IntoElement)]
pub struct SheetDescription {
    text: SharedString,
}

impl SheetDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for SheetDescription {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .text_color(text::MUTED)
            .child(self.text)
    }
}

/// Sheet content section
#[derive(IntoElement)]
pub struct SheetContent {
    children: Vec<AnyElement>,
}

impl SheetContent {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for SheetContent {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for SheetContent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div().p(px(16.0)).flex_1();

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}

/// Sheet footer section
#[derive(IntoElement)]
pub struct SheetFooter {
    children: Vec<AnyElement>,
}

impl SheetFooter {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for SheetFooter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for SheetFooter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .p(px(16.0))
            .border_t_1()
            .border_color(border::DEFAULT);

        for child in self.children {
            el = el.child(child);
        }

        el
    }
}
