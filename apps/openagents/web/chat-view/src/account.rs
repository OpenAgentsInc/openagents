//! Small account controls shared by the web shell and lazy Settings.

use gpui::{ClickEvent, IntoElement, KeyDownEvent, ParentElement, Styled, div, prelude::*, px};

/// Open the site's sign-out confirmation page.
pub fn sign_out(go: fn()) -> impl IntoElement {
    div()
        .id("account-sign-out")
        .role(gpui::Role::Button)
        .aria_label("Sign out")
        .tab_index(0)
        .px(px(18.))
        .py(px(10.))
        .text_size(px(13.5))
        .text_color(crate::theme::text_muted())
        .cursor_pointer()
        .hover(|style| {
            style
                .bg(crate::theme::element_hover())
                .text_color(crate::theme::text())
        })
        .on_click(move |_: &ClickEvent, _, _| go())
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                go();
                cx.stop_propagation();
            }
        })
        .child("Sign out")
}
