//! Story helper utilities for consistent storybook styling

use gpui::{Div, SharedString, div, hsla, prelude::*, px};

/// Helper struct for building story UI
pub struct Story;

impl Story {
    /// Main container for a story
    pub fn container() -> gpui::Stateful<Div> {
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
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(hsla(0.0, 0.0, 1.0, 1.0))
            .pb(px(8.0))
            .border_b_1()
            .border_color(hsla(0.0, 0.0, 0.3, 1.0))
            .child(title.into())
    }

    /// Section title
    pub fn section_title(title: impl Into<SharedString>) -> Div {
        div()
            .text_lg()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(hsla(0.0, 0.0, 0.9, 1.0))
            .pt(px(12.0))
            .pb(px(4.0))
            .child(title.into())
    }

    /// Label for an item
    pub fn label(label: impl Into<SharedString>) -> Div {
        div()
            .text_sm()
            .text_color(hsla(0.0, 0.0, 0.7, 1.0))
            .child(label.into())
    }

    /// Description text
    pub fn description(desc: impl Into<SharedString>) -> Div {
        div()
            .text_sm()
            .text_color(hsla(0.0, 0.0, 0.6, 1.0))
            .child(desc.into())
    }

    /// A section container with border
    pub fn section() -> Div {
        div()
            .p(px(16.0))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.3, 1.0))
            .rounded(px(8.0))
            .bg(hsla(0.0, 0.0, 0.1, 1.0))
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
            .bg(hsla(0.0, 0.0, 0.3, 1.0))
            .my(px(12.0))
    }
}
