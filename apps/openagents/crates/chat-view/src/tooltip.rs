//! The app's one-line hover note: a frosted chip below or above the pointer.

use gpui::{AppContext as _, IntoElement, ParentElement, Render, SharedString, Styled, div, px};

use crate::theme::Theme;

/// A one-line hover note for settings controls (reset times, icon-only
/// actions), in the same frosted chip as the rest of the app's tooltips.
pub struct TextTooltip {
    text: SharedString,
    above: bool,
}

/// How far a [`text_tooltip_above`] chip sits over the pointer; clears a
/// small control the pointer is anywhere inside.
const TOOLTIP_ABOVE_GAP: f32 = 20.0;

impl Render for TextTooltip {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::of(cx);
        let card = div()
            .max_w(px(320.0))
            .px(px(9.0))
            .py(px(6.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(crate::popover::surface_bg(theme))
            .text_size(px(11.0))
            .text_color(theme.text_muted)
            .child(self.text.clone());
        let card = crate::frost::frosted(6.0, crate::frost::MENU_BLUR, card);
        if !self.above {
            return card.into_any_element();
        }
        // gpui hangs a tooltip below-right of the pointer. A zero-height box
        // with end-aligned content makes the chip overflow upward instead.
        div()
            .h(px(0.0))
            .flex()
            .flex_col()
            .justify_end()
            .child(div().relative().bottom(px(TOOLTIP_ABOVE_GAP)).child(card))
            .into_any_element()
    }
}

/// `.tooltip(...)` builder for a [`TextTooltip`].
pub fn text_tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |_, cx| {
        cx.new(|_| TextTooltip {
            text: text.clone(),
            above: false,
        })
        .into()
    }
}

/// Like [`text_tooltip`], shown above the pointer instead of below it, for
/// controls with content underneath that the chip would otherwise cover.
pub fn text_tooltip_above(
    text: impl Into<SharedString>,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |_, cx| {
        cx.new(|_| TextTooltip {
            text: text.clone(),
            above: true,
        })
        .into()
    }
}
