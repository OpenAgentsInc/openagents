//! The desktop sidebar's row geometry, labels, and scrolling list.
//! Hosts supply titles, status and action controls, and account operations.
use crate::{
    icons::{self, icon},
    motion, popover,
    theme::Theme,
};
use gpui::{AnyElement, Div, Hsla, ScrollHandle, SharedString, Stateful, div, prelude::*, px};

pub fn chat_row_height(shows_branch: bool, shows_pull_request: bool) -> f32 {
    let mut metadata_height: f32 = 0.0;
    if shows_branch {
        metadata_height = metadata_height.max(14.0);
    }
    if shows_pull_request {
        metadata_height = metadata_height.max(16.0);
    }
    if metadata_height == 0.0 {
        45.0
    } else {
        47.0 + metadata_height
    }
}
pub fn row_height(compact: bool, show_label: bool, branch: bool, pr: bool) -> f32 {
    if compact {
        29.0
    } else {
        chat_row_height(branch, pr) - if show_label { 0.0 } else { 16.0 }
    }
}

pub fn faded_label(id: SharedString, fill: bool, label: impl IntoElement) -> impl IntoElement {
    let overflow = gpui::ScrollHandle::new();
    crate::edge_fade::edge_faded(
        20.0,
        false,
        false,
        div()
            .id(id.clone())
            .debug_selector(move || id.to_string())
            .when(fill, |el| el.flex_1())
            .min_w_0()
            .overflow_hidden()
            .track_scroll(&overflow)
            .flex()
            .child(div().flex_none().whitespace_nowrap().child(label)),
    )
    .fade_right(true)
    .fade_label_overflow(&overflow)
}

/// The labels and layout choices a sidebar row draws.
pub struct RowData<'a> {
    pub id: SharedString,
    pub space_name: SharedString,
    pub branch: Option<SharedString>,
    pub compact: bool,
    pub show_label: bool,
    pub shows_metadata: bool,
    pub continued: bool,
    pub remote: bool,
    pub search_query: Option<&'a str>,
    pub subline: Hsla,
}
/// Host-owned controls placed in the desktop's row layout.
pub struct RowSlots {
    pub title: AnyElement,
    pub identity: Option<AnyElement>,
    pub project_icon: Option<AnyElement>,
    pub compact_status: Option<AnyElement>,
    pub corner: Option<AnyElement>,
}

pub fn row_content(
    row: Stateful<Div>,
    data: RowData<'_>,
    slots: RowSlots,
    theme: &Theme,
    compact_trailing: impl FnOnce(Div) -> Div,
    metadata: impl FnOnce(Div) -> Div,
) -> AnyElement {
    let RowData {
        id: content_id,
        space_name,
        branch,
        compact,
        show_label,
        shows_metadata,
        continued,
        remote,
        search_query,
        subline,
    } = data;
    let RowSlots {
        title,
        identity,
        project_icon,
        compact_status,
        mut corner,
    } = slots;
    row
        // Line 1: "project @ device", status word / time-ago right.
        .when(!compact && show_label, |el| {
            el.child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Theme::SPACE_SM))
                    .child(faded_label(
                        format!("chat-device-{content_id}").into(),
                        true,
                        div()
                            .text_size(crate::typography::ui_rems(11.0))
                            .line_height(px(14.0))
                            .text_color(subline)
                            .child(popover::search_highlight(space_name, search_query, theme)),
                    ))
                    .child(div().text_color(subline).children(corner.take())),
            )
        })
        // Line 2: harness identity belongs directly with the title,
        // instead of floating as unrelated metadata below it.
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(if compact { 4.0 } else { Theme::SPACE_SM }))
                .children(compact_status)
                .children(identity)
                .children(project_icon)
                .child(title)
                .when(continued && !compact, |el| {
                    el.child(
                        div()
                            .text_size(crate::typography::ui_rems(10.))
                            .text_color(theme.text_muted)
                            .child("Continued"),
                    )
                })
                .when(!compact && !show_label && remote, |el| {
                    el.child(
                        icon(icons::REMOTE_SERVER)
                            .size(px(13.0))
                            .flex_none()
                            .text_color(subline),
                    )
                })
                .when(!compact && !show_label, |el| {
                    el.child(
                        div()
                            .flex_none()
                            .text_color(subline)
                            .children(corner.take()),
                    )
                })
                // Compact trailing cluster, right-packed: the PR badge sits
                // LEFT of one fixed slot that shows the relative time, swaps
                // to the archive affordance while the row is hovered, and
                // shows the Cmd+N jump legend while the modifier is held.
                // The legend wins over the hover archive (user request).
                // There is no remote globe — the slot's fixed width keeps
                // the column aligned across rows regardless of a PR badge.
                .when(compact, compact_trailing),
        )
        // Line 3 is structural, not reserved whitespace: compact states
        // omit it completely when both Branch and Pull request are hidden.
        .when(!compact && shows_metadata, |row| {
            row.child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .when_some(branch, |el, branch| {
                        el.child(
                            icon(icons::GIT_BRANCH)
                                .size(px(11.0))
                                .flex_none()
                                .text_color(subline),
                        )
                        .child(faded_label(
                            format!("chat-branch-{content_id}").into(),
                            false,
                            div()
                                .text_size(crate::typography::ui_rems(11.0))
                                .line_height(px(14.0))
                                .text_color(subline)
                                .child(popover::search_highlight(branch, search_query, theme)),
                        ))
                    })
                    // Stable invisible spring keeps the optional PR badge
                    // pinned right without changing no-PR paint.
                    .child(div().flex_1().min_w_0())
                    .map(metadata),
            )
        })
        .into_any_element()
}

pub struct RowAppearance {
    pub height: f32,
    pub palette: bool,
    pub rest_text: Hsla,
    pub text: Hsla,
    pub rest_bg: Hsla,
    pub hover_bg: Hsla,
}
pub fn row_frame(row_id: String, appearance: RowAppearance) -> Stateful<Div> {
    let fade_key = format!("{row_id}-hover");
    div()
        .id(SharedString::from(row_id.clone()))
        .group("sidebar-session-row")
        .debug_selector(move || row_id.clone())
        .h(px(appearance.height))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .rounded(px(if appearance.palette {
            popover::PALETTE_ITEM_RADIUS
        } else {
            8.0
        }))
        .px(px(Theme::SPACE_SM))
        .py(px(6.0))
        .text_color(motion::hover_blend(
            &fade_key,
            appearance.rest_text,
            appearance.text,
        ))
        .bg(motion::hover_blend(
            &fade_key,
            appearance.rest_bg,
            appearance.hover_bg,
        ))
}

pub fn list_frame(scroll: &ScrollHandle) -> Stateful<Div> {
    div()
        .id("sidebar-lists")
        .relative()
        .size_full()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .px(px(Theme::SPACE_SM))
        .flex()
        .flex_col()
        .pt(px(4.0))
}
pub fn lists(scroll: &ScrollHandle, content: impl IntoElement) -> AnyElement {
    crate::edge_fade::edge_faded(
        24.0,
        true,
        true,
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(crate::overscroll::overscroll(
                "sidebar-overscroll",
                crate::overscroll::OverscrollSource::Handle(scroll.clone()),
                content,
            )),
    )
    .fade_overflow_y(scroll)
    .into_any_element()
}
