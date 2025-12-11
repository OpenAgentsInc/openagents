//! Story helper utilities for consistent storybook styling

use gpui_oa::{Div, SharedString, div, prelude::*, px};
use theme_oa::{bg, border, text};

/// Helper struct for building story UI
pub struct Story;

impl Story {
    /// Main container for a story
    pub fn container() -> gpui_oa::Stateful<Div> {
        div()
            .id("story_container")
            .overflow_y_scroll()
            .w_full()
            .min_h_full()
            .flex()
            .flex_col()
            .p(px(20.0))
            .gap(px(20.0))
    }

    /// Story title
    pub fn title(title: impl Into<SharedString>) -> Div {
        div()
            .text_xl()
            .font_weight(gpui_oa::FontWeight::BOLD)
            .text_color(text::BRIGHT)
            .pb(px(8.0))
            .border_b_1()
            .border_color(border::STRONG)
            .child(title.into())
    }

    /// Section title
    pub fn section_title(title: impl Into<SharedString>) -> Div {
        div()
            .text_lg()
            .font_weight(gpui_oa::FontWeight::MEDIUM)
            .text_color(text::PRIMARY)
            .pt(px(12.0))
            .pb(px(4.0))
            .child(title.into())
    }

    /// Label for an item
    pub fn label(label: impl Into<SharedString>) -> Div {
        div()
            .text_sm()
            .text_color(text::SECONDARY)
            .child(label.into())
    }

    /// Description text
    pub fn description(desc: impl Into<SharedString>) -> Div {
        div()
            .text_sm()
            .text_color(text::MUTED)
            .child(desc.into())
    }

    /// A section container with border
    pub fn section() -> Div {
        div()
            .p(px(16.0))
            .border_1()
            .border_color(border::STRONG)
            .rounded(px(8.0))
            .bg(bg::ELEVATED)
    }

    /// Horizontal flex row
    pub fn row() -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
    }

    /// Vertical flex column
    pub fn column() -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
    }

    /// A demo item with label
    pub fn item(label: impl Into<SharedString>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::label(label))
    }

    /// Divider line
    #[allow(dead_code)]
    pub fn divider() -> Div {
        div()
            .h(px(1.0))
            .w_full()
            .bg(border::STRONG)
            .my(px(12.0))
    }
}
