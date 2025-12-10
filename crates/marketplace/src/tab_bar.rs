//! Tab bar component - Navigation between Agents, Compute, Services

use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

use crate::types::MarketplaceTab;

/// Render the tab bar
#[allow(dead_code)]
pub fn render<F>(current_tab: MarketplaceTab, on_tab_click: F) -> impl IntoElement
where
    F: Fn(MarketplaceTab) -> Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static> + 'static,
{
    div()
        .h(px(48.0))
        .w_full()
        .flex()
        .items_center()
        .px(px(20.0))
        .gap(px(4.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::DEFAULT)
        .children(MarketplaceTab::all().iter().map(|&tab| {
            render_tab_button(tab, tab == current_tab, on_tab_click(tab))
        }))
}

/// Render a single tab button
#[allow(dead_code)]
fn render_tab_button(
    tab: MarketplaceTab,
    is_active: bool,
    on_click: Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
) -> impl IntoElement {
    let (bg_color, text_color, border_color) = if is_active {
        (bg::SELECTED, text::BRIGHT, border::SELECTED)
    } else {
        (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
    };

    div()
        .id(SharedString::from(format!("tab_{}", tab.label())))
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(10.0))
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY))
        .on_click(on_click)
        .child(
            div()
                .text_size(px(13.0))
                .child(tab.icon().to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(tab.label().to_string()),
        )
}

/// Render static tab bar (for when callbacks aren't needed)
#[allow(dead_code)]
pub fn render_static(current_tab: MarketplaceTab) -> impl IntoElement {
    div()
        .h(px(48.0))
        .w_full()
        .flex()
        .items_center()
        .px(px(20.0))
        .gap(px(4.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::DEFAULT)
        .children(MarketplaceTab::all().iter().map(|&tab| {
            render_tab_button_static(tab, tab == current_tab)
        }))
}

/// Render a static tab button (no click handler)
#[allow(dead_code)]
fn render_tab_button_static(tab: MarketplaceTab, is_active: bool) -> impl IntoElement {
    let (bg_color, text_color, border_color) = if is_active {
        (bg::SELECTED, text::BRIGHT, border::SELECTED)
    } else {
        (Hsla::transparent_black(), text::MUTED, Hsla::transparent_black())
    };

    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(10.0))
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .rounded(px(6.0))
        .child(
            div()
                .text_size(px(13.0))
                .child(tab.icon().to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(tab.label().to_string()),
        )
}
