//! The transcript's pieces: user bubbles, assistant turns (text, tool groups,
//! images, errors, question cards), and the small labels around them. The
//! demo and the signed-in chat both draw turns through [`assistant_body`],
//! from `openagents_doc` message parts.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use gpui::img;
use gpui::{
    AnyElement, App, ClickEvent, FontWeight, Hsla, Image, IntoElement, ObjectFit, ParentElement,
    SharedString, Styled, Window, div, prelude::*, px,
};
use openagents_doc::{MessagePart, MessageStatus};
#[cfg(target_family = "wasm")]
use openagents_gpui_web::img;
use openagents_proto::{ReasoningLevel, TodoStatus, ToolCall, UserInputQuestion};

use crate::markdown::{Markdown, Veil};
use crate::svg;
use crate::{now_ms, theme};

pub const SIDEBAR_WIDTH: f32 = 272.;
pub const CONTENT_WIDTH: f32 = 740.;

/// A text part whose id starts with this is a notice from the site (a
/// stopped answer, a failure) and draws as a quiet line, not markdown.
pub const NOTICE_PREFIX: &str = "notice";

/// A click handler built for one element.
pub type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// What an assistant turn needs from the app that draws it.
pub struct TurnView<'a> {
    /// Unique per chat and entry; prefixes the element ids.
    pub key: &'a str,
    pub parts: &'a [MessagePart],
    pub status: Option<MessageStatus>,
    pub duration_ms: Option<i64>,
    pub images: &'a HashMap<String, Arc<Image>>,
    pub expanded: &'a HashSet<String>,
    /// The fade on the last text part while it streams.
    pub veil: Option<Veil>,
}

/// One row of the transcript list, centered at the content width.
pub fn entry_row(first: bool, body: AnyElement) -> AnyElement {
    div()
        .w_full()
        .flex()
        .justify_center()
        .px(px(24.))
        .when(first, |row| row.pt(px(28.)))
        .pb(px(26.))
        .child(div().w_full().max_w(px(CONTENT_WIDTH)).child(body))
        .into_any_element()
}

/// A person's message: its attachments above a bubble of markdown.
pub fn user_bubble(id: &str, body: &str, attachments: Vec<AnyElement>) -> AnyElement {
    let markdown = Markdown {
        id: format!("u-{id}").into(),
    };
    div()
        .flex()
        .flex_col()
        .items_end()
        .gap(px(8.))
        .when(!attachments.is_empty(), |col| {
            col.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .justify_end()
                    .gap(px(8.))
                    .children(attachments),
            )
        })
        .child(
            div()
                .max_w(px(CONTENT_WIDTH * 0.82))
                .px(px(16.))
                .py(px(10.))
                .rounded(px(20.))
                .bg(theme::surface_raised())
                .text_size(px(theme::BODY))
                .line_height(px(theme::BODY_LINE))
                .child(markdown.render(body, false, None)),
        )
        .into_any_element()
}

/// An attached image as the user bubble shows it.
pub fn attachment_image(image: Arc<Image>) -> AnyElement {
    img(image)
        .h(px(120.))
        .max_w(px(220.))
        .rounded(px(12.))
        .object_fit(ObjectFit::Cover)
        .into_any_element()
}

/// An assistant turn. `toggle` builds the click handler that opens or closes
/// the tool group with the given key; `questions` draws a question card from
/// its part index, questions, and whether it was answered.
pub fn assistant_body(
    view: TurnView<'_>,
    toggle: &dyn Fn(String) -> OnClick,
    questions: &mut dyn FnMut(usize, &[UserInputQuestion], bool) -> AnyElement,
) -> AnyElement {
    let mut blocks: Vec<AnyElement> = Vec::new();
    let parts = view.parts;
    let mut ix = 0;
    while ix < parts.len() {
        match &parts[ix] {
            MessagePart::Tool { .. } | MessagePart::Reasoning { .. } => {
                let start = ix;
                while ix < parts.len()
                    && matches!(
                        parts[ix],
                        MessagePart::Tool { .. } | MessagePart::Reasoning { .. }
                    )
                {
                    ix += 1;
                }
                let key = format!("{}/{start}", view.key);
                let expanded = view.expanded.contains(&key);
                let on_click = toggle(key.clone());
                blocks.push(tool_group(&key, &parts[start..ix], expanded, on_click));
                continue;
            }
            MessagePart::Text { id, text } if id.starts_with(NOTICE_PREFIX) => {
                blocks.push(footer_text(text.clone()).into_any_element());
            }
            MessagePart::Text { id, text } => {
                if !text.is_empty() {
                    let last = ix + 1 == parts.len();
                    let streaming = last && view.status == Some(MessageStatus::Streaming);
                    let veil = view.veil.as_ref().filter(|_| last);
                    let markdown = Markdown {
                        id: format!("{}-{id}-{ix}", view.key).into(),
                    };
                    blocks.push(markdown.render(text, streaming, veil));
                }
            }
            MessagePart::Image { path, name, .. } => {
                if let Some(image) = view.images.get(path) {
                    blocks.push(
                        img(image.clone())
                            .max_w(px(420.))
                            .rounded(px(12.))
                            .into_any_element(),
                    );
                } else {
                    blocks.push(chip(
                        "icons/document.svg",
                        name.clone(),
                        theme::text_muted(),
                    ));
                }
            }
            MessagePart::Visual { title, .. } => blocks.push(visual_card(title)),
            MessagePart::Input {
                questions: asked,
                resolved,
                ..
            } => blocks.push(questions(ix, asked, *resolved)),
            MessagePart::Error { message, .. } => blocks.push(error_block(message)),
            MessagePart::Fork { source_title, .. } => blocks.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .text_size(px(12.))
                    .text_color(theme::text_faint())
                    .child(div().flex_1().h(px(1.)).bg(theme::border()))
                    .child(format!("Forked from {source_title}"))
                    .child(div().flex_1().h(px(1.)).bg(theme::border()))
                    .into_any_element(),
            ),
        }
        ix += 1;
    }
    let footer: Option<AnyElement> = match view.status {
        Some(MessageStatus::Streaming) => Some(working_label().into_any_element()),
        Some(MessageStatus::Aborted) => Some(footer_text("Stopped").into_any_element()),
        _ => view.duration_ms.map(|ms| {
            footer_text(format!("Worked for {}", duration(ms.max(0) as u64))).into_any_element()
        }),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .text_size(px(theme::BODY))
        .line_height(px(theme::BODY_LINE))
        .children(blocks)
        .children(footer)
        .into_any_element()
}

/// A run of tool calls and thoughts, folded into one summary line.
pub fn tool_group(
    key: &str,
    parts: &[MessagePart],
    expanded: bool,
    on_click: OnClick,
) -> AnyElement {
    let mut pairs: Vec<(ToolCall, bool)> = Vec::new();
    let mut thoughts = 0usize;
    let mut running = false;
    let (mut additions, mut deletions) = (0u64, 0u64);
    for part in parts {
        match part {
            MessagePart::Tool {
                call,
                is_error,
                resolved,
                diff_stats,
                ..
            } => {
                pairs.push((call.clone(), *is_error));
                running |= !resolved;
                for stat in diff_stats.iter().flatten() {
                    additions += stat.additions;
                    deletions += stat.deletions;
                }
            }
            MessagePart::Reasoning { .. } => thoughts += 1,
            _ => {}
        }
    }
    // The desktop's `tool_group_summary`: thoughts ride the shared rule.
    let mut segments: Vec<String> = Vec::new();
    match thoughts {
        0 => {}
        1 => segments.push("thought process".into()),
        n => segments.push(format!("thought {n} times")),
    }
    if !pairs.is_empty() {
        segments.push(openagents_proto::view::tool_group_summary(&pairs));
    }
    let mut summary = segments.join(" · ");
    if let Some(first) = summary.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    let pulse = if running {
        0.55 + 0.45 * ((now_ms() as f32 / 260.).sin() * 0.5 + 0.5)
    } else {
        1.0
    };
    let header = div()
        .id(SharedString::from(format!("group-{key}")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .cursor_pointer()
        .text_size(px(13.5))
        .text_color(theme::text_muted().opacity(pulse))
        .hover(|style| style.text_color(theme::text()))
        .on_click(on_click)
        .child(
            div()
                .size(px(20.))
                .rounded(px(6.))
                .border_1()
                .border_color(theme::border())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(if expanded {
                            "icons/alt-arrow-down.svg"
                        } else {
                            "icons/alt-arrow-right.svg"
                        })
                        .size(px(13.))
                        .text_color(theme::text_muted()),
                ),
        )
        .child(summary)
        .when(additions + deletions > 0, |row| {
            row.child(diff_stat(additions, deletions))
        });
    let mut group = div().flex().flex_col().gap(px(4.)).child(header);
    if expanded {
        group = group.child(
            div()
                .ml(px(9.))
                .pl(px(18.))
                .border_l_1()
                .border_color(theme::border())
                .flex()
                .flex_col()
                .gap(px(8.))
                .py(px(4.))
                .children(parts.iter().map(tool_row)),
        );
    }
    group.into_any_element()
}

fn diff_stat(additions: u64, deletions: u64) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .gap(px(5.))
        .font_family(theme::MONO)
        .text_size(px(12.))
        .child(
            div()
                .text_color(theme::diff_add())
                .child(format!("+{additions}")),
        )
        .child(
            div()
                .text_color(theme::diff_del())
                .child(format!("−{deletions}")),
        )
}

/// One tool call or thought inside an open group.
pub fn tool_row(part: &MessagePart) -> AnyElement {
    match part {
        MessagePart::Reasoning { text, .. } => div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme::text_muted())
                    .child("Thought"),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .italic()
                    .text_color(theme::text_faint())
                    .child(text.clone()),
            )
            .into_any_element(),
        MessagePart::Tool {
            call,
            is_error,
            resolved,
            output,
            diff_stats,
            ..
        } => {
            let (verb, target) = tool_label(call);
            let mut row = div().flex().flex_col().gap(px(3.)).child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(7.))
                    .min_w_0()
                    .text_size(px(13.))
                    .child(
                        div()
                            .flex_none()
                            .text_color(if *is_error {
                                theme::danger()
                            } else {
                                theme::text_muted()
                            })
                            .child(if *resolved {
                                verb.to_owned()
                            } else {
                                format!("{} …", running_verb(verb))
                            }),
                    )
                    .when(!target.is_empty(), |row| {
                        row.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_family(theme::MONO)
                                .text_size(px(12.))
                                .text_color(theme::text())
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(5.))
                                .bg(theme::input_bg())
                                .border_1()
                                .border_color(theme::border())
                                .child(target),
                        )
                    })
                    .children(
                        diff_stats
                            .iter()
                            .flatten()
                            .map(|stat| diff_stat(stat.additions, stat.deletions)),
                    ),
            );
            if let ToolCall::Todo { items } = call {
                row = row.child(div().flex().flex_col().gap(px(3.)).pt(px(2.)).children(
                    items.iter().map(|item| {
                        let done = item.done || item.status == Some(TodoStatus::Completed);
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(13.))
                            .text_color(if done {
                                theme::text_faint()
                            } else {
                                theme::text_muted()
                            })
                            .child(choice_mark(done, true))
                            .child(item.text.clone())
                    }),
                ));
            }
            if let Some(output) = output.as_deref().filter(|o| !o.trim().is_empty()) {
                row = row.child(
                    div()
                        .font_family(theme::MONO)
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(if *is_error {
                            theme::danger().opacity(0.85)
                        } else {
                            theme::text_faint()
                        })
                        .child(truncate(output.lines().next().unwrap_or(""), 160)),
                );
            }
            row.into_any_element()
        }
        _ => div().into_any_element(),
    }
}

pub fn tool_label(call: &ToolCall) -> (&'static str, String) {
    match call {
        ToolCall::Exec { command } => ("Ran", command.clone()),
        ToolCall::ReadFile { path } => ("Read", path.clone()),
        ToolCall::WriteFile { path, .. } => ("Wrote", path.clone()),
        ToolCall::EditFile { path, .. } => ("Edited", path.clone()),
        ToolCall::ApplyPatch { path } => ("Patched", path.clone().unwrap_or_default()),
        ToolCall::Search { pattern, path } => (
            "Searched",
            match path {
                Some(path) => format!("{pattern} in {path}"),
                None => pattern.clone(),
            },
        ),
        ToolCall::Glob { pattern } => ("Listed", pattern.clone()),
        ToolCall::WebFetch { url, .. } => ("Fetched", url.clone()),
        ToolCall::WebSearch { query } => ("Searched the web", query.clone()),
        ToolCall::Todo { .. } => ("Updated the plan", String::new()),
        ToolCall::Mcp { server, tool, .. } => ("Used", format!("{server} · {tool}")),
        ToolCall::Unknown { name, .. } => ("Used", name.clone()),
    }
}

fn running_verb(verb: &str) -> &str {
    match verb {
        "Ran" => "Running",
        "Read" => "Reading",
        "Edited" => "Editing",
        "Wrote" => "Writing",
        "Searched" => "Searching",
        "Fetched" => "Fetching",
        _ => verb,
    }
}

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

pub fn error_block(message: &str) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.))
        .px(px(12.))
        .py(px(9.))
        .rounded(px(10.))
        .bg(theme::danger().opacity(0.08))
        .border_1()
        .border_color(theme::danger().opacity(0.25))
        .text_size(px(13.5))
        .text_color(theme::danger())
        .child(
            svg()
                .mt(px(2.))
                .path("icons/danger-triangle.svg")
                .size(px(15.))
                .text_color(theme::danger()),
        )
        .child(message.to_owned())
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

fn visual_card(title: &str) -> AnyElement {
    info_card(
        "icons/widget.svg",
        title.to_owned(),
        "Interactive visual. Open it in the desktop app to use it.".into(),
    )
}

pub fn footer_text(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(12.5))
        .text_color(theme::text_faint())
        .child(text.into())
}

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

pub fn duration(ms: u64) -> String {
    let secs = (ms / 1000).max(1);
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
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
        assert_eq!(duration(61_000), "1m 1s");
    }
}
