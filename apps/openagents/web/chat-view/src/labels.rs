//! The small pieces the web apps draw around the chat: widths, buttons,
//! chips and cards, footers, and the labels for times, models, and
//! reasoning. The transcript itself is the desktop's
//! (`crate::desktop_transcript`).

use gpui::{
    AnyElement, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px,
};
use openagents_proto::ReasoningLevel;

use crate::svg;
use crate::{now_ms, theme};

pub const SIDEBAR_WIDTH: f32 = 272.;

pub const CONTENT_WIDTH: f32 = 740.;

thread_local! {
    static WIDTH: std::cell::Cell<f32> = const { std::cell::Cell::new(CONTENT_WIDTH) };
}

/// The conversation's column width now: [`CONTENT_WIDTH`] until Settings →
/// Appearance sets another (#12289).
pub fn content_width() -> f32 {
    WIDTH.with(std::cell::Cell::get)
}

pub fn set_content_width(width: f32) {
    WIDTH.with(|w| w.set(width));
}

/// A text part whose id starts with this is a notice from the site (a
/// stopped answer, a failure) and draws as a quiet line, not markdown.
pub const NOTICE_PREFIX: &str = "notice";

pub fn choice_mark(selected: bool, square: bool) -> impl IntoElement {
    let base = div()
        .flex_none()
        .size(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(if square { px(4.) } else { px(7.) });
    if selected {
        base.bg(theme::accent()).child(
            svg()
                .path("icons/check.svg")
                .size(px(11.))
                .text_color(theme::on_solid()),
        )
    } else {
        base.border_1().border_color(theme::border_strong())
    }
}

pub fn chip(icon: &'static str, label: String, color: Hsla) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .px(px(8.))
        .h(px(24.))
        .rounded(px(12.))
        .border_1()
        .border_color(theme::border())
        .text_size(px(12.))
        .text_color(color)
        .child(svg().path(icon).size(px(13.)).text_color(color))
        .child(label)
        .into_any_element()
}

/// A card the transcript names but doesn't draw here: its title, a line
/// under it, and an icon.
pub fn info_card(icon: &'static str, title: String, line: String) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .p(px(14.))
        .rounded(px(14.))
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface_card())
        .child(
            div()
                .flex_none()
                .size(px(36.))
                .rounded(px(10.))
                .bg(theme::accent_wash())
                .flex()
                .items_center()
                .justify_center()
                .child(svg().path(icon).size(px(18.)).text_color(theme::accent())),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(div().text_size(px(14.)).child(title))
                .when(!line.is_empty(), |col| {
                    col.child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme::text_faint())
                            .child(line),
                    )
                }),
        )
        .into_any_element()
}

pub fn footer_text(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(12.5))
        .text_color(theme::text_faint())
        .child(text.into())
}

/// "Working", with dots that count up: a header's sign that a reply is
/// being written.
pub fn working_label() -> impl IntoElement {
    let phase = (now_ms() / 350) % 4;
    div()
        .text_size(px(12.5))
        .text_color(theme::accent())
        .child(format!("Working{}", ".".repeat(phase as usize)))
}

/// A pill button: solid when `primary`, outlined otherwise.
pub fn button(
    id: impl Into<SharedString>,
    label: &'static str,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .px(px(14.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(16.))
        .text_size(px(13.5))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(primary, |b| {
            b.bg(theme::solid()).text_color(theme::on_solid())
        })
        .when(!primary, |b| {
            b.border_1()
                .border_color(theme::border_strong())
                .text_color(theme::text())
                .hover(|style| style.bg(theme::element_hover()))
        })
        .child(label)
}

pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// A short age: `5s`, `3m`, `2h`, `4d`.
pub fn ago(ms: i64) -> String {
    let secs = (ms / 1000).max(0);
    match secs {
        0..=59 => format!("{}s", secs.max(1)),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86_399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

pub fn model_label(model: &str) -> String {
    let name = model.strip_prefix("claude-").unwrap_or(model);
    if let Some(rest) = name.strip_prefix("gpt-") {
        let mut parts = rest.splitn(2, '-');
        let version = parts.next().unwrap_or("");
        return match parts.next() {
            Some(variant) => format!("GPT-{version} {}", capitalize(variant)),
            None => format!("GPT-{version}"),
        };
    }
    name.split('-')
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn reasoning_label(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Minimal => "Minimal",
        ReasoningLevel::Low => "Low",
        ReasoningLevel::Medium => "Medium",
        ReasoningLevel::High => "High",
        ReasoningLevel::XHigh => "Extra high",
        ReasoningLevel::Max => "Max",
        ReasoningLevel::Ultra => "Ultra",
        ReasoningLevel::Ultracode => "Ultracode",
        ReasoningLevel::Ultrathink => "Ultrathink",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_read_plainly() {
        assert_eq!(model_label("claude-fable-5"), "Fable 5");
        assert_eq!(model_label("gpt-5.6-terra"), "GPT-5.6 Terra");
        assert_eq!(ago(90_000), "1m");
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
