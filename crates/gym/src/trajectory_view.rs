//! Trajectory View component
//!
//! Displays trajectory list and detail viewer for browsing ATIF trajectories.
//! This component will eventually integrate trajectory rendering to provide
//! a full-featured trajectory browser with search, pagination, and detail view.

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

pub struct TrajectoryView {
    /// Focus handle
    focus_handle: FocusHandle,

    // TODO: Add state for:
    // - trajectory list (from atif-store)
    // - selected trajectory ID
    // - pagination state (page, page_size)
    // - search query
    // - expanded step IDs (for accordion view)
    // - loading/error states
}

impl TrajectoryView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    /// Render placeholder until full implementation
    fn render_placeholder(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .h_full()
            .w_full()
            .bg(bg::APP)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(32.0))
                            .child("📊")
                    )
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child("Trajectory Viewer")
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("Browse and analyze ATIF trajectories")
                    )
                    .child(
                        div()
                            .mt(px(24.0))
                            .px(px(16.0))
                            .py(px(8.0))
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .rounded(px(6.0))
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("Coming soon: Full trajectory browser with search, pagination, and step-by-step detail view")
                    )
            )
    }
}

impl Focusable for TrajectoryView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TrajectoryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.render_placeholder()
    }
}
