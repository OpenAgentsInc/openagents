use crate::components::{Button, Div, Text};
use crate::layout::{pct as layout_pct, px as layout_px};
use taffy::prelude::Dimension;

pub fn div() -> Div {
    Div::new()
}

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

pub fn v_flex() -> Div {
    div().flex().flex_col()
}

pub fn text(content: impl Into<String>) -> Text {
    Text::new(content)
}

pub fn button(label: impl Into<String>) -> Button {
    Button::new(label)
}

pub fn px(value: f32) -> Dimension {
    layout_px(value)
}

pub fn pct(value: f32) -> Dimension {
    layout_pct(value)
}
