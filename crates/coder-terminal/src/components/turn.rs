//! A turn in the transcript, drawn as grok-build draws one.
//!
//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `xai-grok-pager/src/scrollback/blocks/user.rs` (the prompt), `blocks/agent.rs`
//! with `xai-grok-pager-render/src/theme/md_style.rs` (a reply's Markdown
//! styles), and the entry layout of `scrollback/wrappers/entry_renderer.rs`
//! and `scrollback/state/layout.rs`. Changes: no fold of a long prompt,
//! the colors reach the terminal through the ladder's color level, and a
//! colorless ladder keeps only the modifiers.
//!
//! No turn has a label. The person's message is a prompt: `❯ ` and the
//! text on grok-build's raised band, a band row above and below. A reply is
//! Markdown at the content column, in Grok Night's (or Grok Day's) colors.
//! One blank row separates each turn from the next.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::{INDENT, cells, indent_at, wrap_hanging, wrap_paragraphs};
use crate::markdown::{self, Marks};
use crate::{Colors, Intensity, Ladder};

/// Who spoke a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    You,
    OpenAgents,
}

/// grok-build's prompt arrow (`glyphs::prompt_arrow`).
pub const PROMPT_ARROW: &str = "\u{276F} ";

/// The cell the prompt's band starts at: one cell left of the text, so the
/// band shows a cell before the arrow.
const BAND_LEAD: usize = INDENT.saturating_sub(1);
/// The cells right of the band: grok-build's outer padding, less the one
/// cell the screen keeps at its left edge, plus its scrollbar's column.
const BAND_TAIL: usize = 2;
/// The cells right of the text: the band's tail plus grok-build's
/// `block_pad_right`.
pub(crate) const CONTENT_TAIL: usize = BAND_TAIL + 2;

/// A finished turn, then the blank row that separates it from the next.
///
/// The person's text is a prompt on a band; a reply is Markdown: body text
/// in `md_text`, headings in their level's color, inline code in `md_code`
/// and bold, links underlined in `link_fg`, list bullets, quote bars, and
/// rules muted, and code blocks on `md_code_bg` with grok-build's syntax
/// colors.
pub fn turn(who: Who, text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let mut lines = match who {
        Who::You => prompt(text, width, ladder),
        Who::OpenAgents => reply(text, width, ladder),
    };
    lines.push(Line::default());
    lines
}

/// A reply still streaming: the reply as it stands, without the blank row
/// that ends a finished one.
pub fn streaming(text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    reply(text, width, ladder)
}

/// A note under a turn — an offer, a follow-up "[1] label", a notice —
/// at the content column at `intensity`. Wrapped rows hang under the text:
/// past a leading "[n] " marker when there is one.
pub fn note(text: &str, intensity: Intensity, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let style = ladder.style(intensity);
    let indent = indent_at(usize::from(width));
    let room = usize::from(width).saturating_sub(indent).max(1);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let hang = marker_width(paragraph).min(room.saturating_sub(1));
        for (continued, row) in wrap_paragraphs(paragraph, room, hang) {
            let lead = indent + if continued { hang } else { 0 };
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(lead)),
                Span::styled(row, style),
            ]));
        }
    }
    lines
}

/// The width of a leading "[n] " marker, or zero.
fn marker_width(text: &str) -> usize {
    let Some(rest) = text.strip_prefix('[') else {
        return 0;
    };
    match rest.find("] ") {
        Some(close) if close > 0 && close <= 4 => cells(&text[..close + 3]),
        _ => 0,
    }
}

/// The person's prompt, as grok-build's `UserPromptBlock` draws it: a band
/// row, `❯ ` in `accent_user` then the text in `text_primary`, continuation
/// rows under the text, a band row; the band (`bg_light`) runs from the
/// accent column to the right padding. On a colorless ladder there is no
/// band and the arrow and text are bold, as grok-build's terminal theme
/// draws them.
fn prompt(text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let width = usize::from(width);
    let colors = transcript(ladder);
    let colorless = ladder.colors() == Colors::None;
    let band = (!colorless).then_some(colors.bg_light);
    let content = indent_at(width);
    let lead = BAND_LEAD.min(content);
    let end = width
        .saturating_sub(BAND_TAIL)
        .max(content + 1)
        .min(width.max(1));
    let text_end = width.saturating_sub(CONTENT_TAIL).max(content + 1).min(end);
    let room = text_end.saturating_sub(content).max(1);
    let arrow = cells(PROMPT_ARROW).min(room.saturating_sub(1));
    let mut arrow_style = fg(colors.accent_user, ladder);
    let mut text_style = fg(colors.text_primary, ladder);
    if colorless {
        arrow_style = arrow_style.add_modifier(Modifier::BOLD);
        text_style = text_style.add_modifier(Modifier::BOLD);
    }
    let banded = |style: Style| match band {
        Some(color) => style.bg(color),
        None => style,
    };
    let row = |spans: Vec<Span<'static>>, used: usize| {
        let mut out = vec![
            Span::raw(" ".repeat(lead)),
            Span::styled(" ".repeat(content - lead), banded(Style::new())),
        ];
        out.extend(spans);
        if band.is_some() {
            out.push(Span::styled(
                " ".repeat(end.saturating_sub(content + used)),
                banded(Style::new()),
            ));
        }
        Line::from(out)
    };
    let mut lines = vec![row(Vec::new(), 0)];
    let mut first = true;
    for logical in text.split('\n') {
        for piece in wrap_hanging(
            &super::sanitize(logical),
            room.saturating_sub(arrow).max(1),
            0,
        ) {
            let lead = if first {
                super::cut(PROMPT_ARROW, arrow)
            } else {
                " ".repeat(arrow)
            };
            first = false;
            let used = arrow + cells(&piece);
            lines.push(row(
                vec![
                    Span::styled(lead, banded(arrow_style)),
                    Span::styled(piece, banded(text_style)),
                ],
                used,
            ));
        }
    }
    lines.push(row(Vec::new(), 0));
    if band.is_none() {
        // grok-build's bandless prompt keeps no padding rows.
        lines.remove(0);
        lines.pop();
    }
    lines
}

/// A reply's Markdown at the content column, in grok-build's styles.
fn reply(text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let content = indent_at(usize::from(width));
    let end = usize::from(width)
        .saturating_sub(CONTENT_TAIL)
        .max(content + 1);
    let room = end.saturating_sub(content).max(1);
    markdown_body(text, room as u16, ladder)
        .into_iter()
        .map(|mut line| {
            if !line.spans.is_empty() {
                line.spans.insert(0, Span::raw(" ".repeat(content)));
            }
            line
        })
        .collect()
}

/// Render Markdown in grok-build's transcript styles across `width` cells.
///
/// The caller owns gutters and spacing between messages. Code backgrounds
/// fill the supplied width; other rows have no trailing padding.
pub fn markdown_body(text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let room = usize::from(width).max(1);
    let colors = transcript(ladder);
    markdown::wrapped(text, room)
        .into_iter()
        .map(|rendered| {
            if rendered.marked.text.is_empty() && !rendered.code {
                return Line::default();
            }
            let mut spans = Vec::new();
            let mut used = 0;
            for (run, marks) in rendered.marked.runs_in(0..rendered.marked.text.len()) {
                used += cells(&run);
                let mut style = if rendered.code && marks.syntax.is_none() {
                    // A fence in no language grok-build knows: body text.
                    fg(colors.md_text, ladder)
                } else {
                    reply_style(&marks, ladder, &colors)
                };
                if rendered.code && ladder.colors() != Colors::None {
                    style = style.bg(colors.md_code_bg);
                }
                spans.push(Span::styled(run, style));
            }
            if rendered.code && ladder.colors() != Colors::None {
                spans.push(Span::styled(
                    " ".repeat(room.saturating_sub(used)),
                    Style::new().bg(colors.md_code_bg),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

/// grok-build's transcript colors at the ladder's color level.
pub(crate) fn transcript(ladder: Ladder) -> code_highlight::grok::TranscriptColors {
    use coder_ui::source_theme as t;
    let apply = |color: rust_native::style::Color| {
        code_highlight::grok::color::quantize_color(
            Color::Rgb(color.red, color.green, color.blue),
            level(ladder),
        )
    };
    code_highlight::grok::TranscriptColors {
        bg_light: apply(t::BG_LIGHT),
        accent_user: apply(t::TEXT_SECONDARY),
        text_primary: apply(t::TEXT_PRIMARY),
        md_text: apply(t::TEXT_SECONDARY),
        md_code: apply(t::MD_CODE),
        md_code_bg: apply(t::BG_DARK),
        md_muted: apply(t::GRAY),
        link_fg: apply(t::ACCENT_SKILL),
        headings: [
            t::ACCENT_SKILL,
            t::ACCENT_SKILL,
            t::ACCENT_DELEGATE,
            t::GRAY_BRIGHT,
            t::GRAY,
            t::GRAY_DIM,
        ]
        .map(apply),
        task_checked: apply(t::ACCENT_SUCCESS),
        task_unchecked: apply(t::TEXT_SECONDARY),
        gray_dim: apply(t::GRAY_DIM),
        gray: apply(t::GRAY),
        gray_bright: apply(t::GRAY_BRIGHT),
    }
}

/// The color level the transcript's colors draw at.
pub(crate) fn level(ladder: Ladder) -> code_highlight::grok::ColorLevel {
    use code_highlight::grok::ColorLevel;
    match ladder.colors() {
        // A 256-color ladder never draws RGB, whatever the terminal says.
        Colors::Indexed => markdown::syntax_level(ladder).min(ColorLevel::Ansi256),
        _ => markdown::syntax_level(ladder),
    }
}

/// `color` as a foreground, or nothing on a colorless ladder.
fn fg(color: Color, ladder: Ladder) -> Style {
    match ladder.colors() {
        Colors::None => Style::new(),
        _ => Style::new().fg(color),
    }
}

/// One run of a reply in grok-build's Markdown styles (`md_style.rs`).
pub(crate) fn reply_style(
    marks: &Marks,
    ladder: Ladder,
    colors: &code_highlight::grok::TranscriptColors,
) -> Style {
    let mut style = if let Some(level) = marks.heading {
        let index = usize::from(level.clamp(1, 6) - 1);
        let style = fg(colors.headings[index], ladder);
        if level < 6 {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    } else if marks.code && marks.syntax.is_some() {
        markdown::code_style(marks.syntax, ladder)
    } else if marks.code && !marks.muted {
        // Inline code is md_code and bold; a fence with no known language
        // draws as body text on its band.
        fg(colors.md_code, ladder).add_modifier(Modifier::BOLD)
    } else if marks.link.is_some() || marks.image.is_some() {
        fg(colors.link_fg, ladder).add_modifier(Modifier::UNDERLINED)
    } else if marks.muted {
        fg(colors.md_muted, ladder)
    } else {
        fg(colors.md_text, ladder)
    };
    if marks.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if marks.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if marks.strike {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if marks.dim {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

/// A reply's inline marks over its line's base style: code in grok-build's
/// syntax colors (the top of the ladder when unhighlighted), the rest as
/// modifiers. Run rows (thoughts, a result's summary) draw with it.
pub(crate) fn marked_style(base: Style, ladder: Ladder, marks: &Marks) -> Style {
    let mut style = if marks.code {
        markdown::code_style(marks.syntax, ladder)
    } else {
        base
    };
    if marks.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if marks.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if marks.strike {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if marks.link.is_some() || marks.image.is_some() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Colors, ladder::rgb};

    fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(|line| line.to_string()).collect()
    }

    #[test]
    fn a_turn_has_no_label_and_ends_with_a_blank_line() {
        let ladder = Ladder::new(Colors::True);
        let done = turn(Who::OpenAgents, "hi", 40, ladder);
        assert_eq!(text(&done), [" hi", ""]);
        let live = streaming("hi", 40, ladder);
        assert_eq!(text(&live), [" hi"]);
        let asked = turn(Who::You, "fix it", 20, ladder);
        assert_eq!(
            text(&asked),
            [
                "                  ",
                " \u{276F} fix it         ",
                "                  ",
                ""
            ]
        );
        // The band is grok-build's bg_light from the accent column on.
        let band = rgb(coder_ui::coder_noir::SURFACE_RAISED);
        assert_eq!(asked[1].spans[0].style.bg, None);
        assert!(
            asked[1].spans[1..]
                .iter()
                .all(|span| span.style.bg == Some(band))
        );
        let arrow = &asked[1].spans[2];
        assert_eq!(arrow.style.fg, Some(rgb(coder_ui::coder_noir::CONTENT)));
        assert_eq!(
            asked[1].spans[3].style.fg,
            Some(rgb(coder_ui::coder_noir::ANSI[15]))
        );
    }

    #[test]
    fn a_reply_draws_grok_builds_markdown_styles() {
        let ladder = Ladder::new(Colors::True);
        let lines = turn(
            Who::OpenAgents,
            "## Plan\n\nRun `cargo test`, see [docs](https://x.dev).\n\n- one\n\n```\nplain\n```",
            60,
            ladder,
        );
        assert_eq!(
            text(&lines),
            [
                " Plan".to_owned(),
                String::new(),
                " Run cargo test, see docs (https://x.dev).".to_owned(),
                String::new(),
                " • one".to_owned(),
                String::new(),
                // The code band runs to the content's right edge.
                format!(" plain{}", " ".repeat(60 - 1 - 4 - 5)),
                String::new(),
            ]
        );
        let span = |needle: &str| {
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .find(|span| span.content.trim() == needle)
                .unwrap_or_else(|| panic!("{needle}"))
                .clone()
        };
        let heading = span("Plan");
        assert_eq!(heading.style.fg, Some(rgb(coder_ui::coder_noir::INFO)));
        assert!(heading.style.add_modifier.contains(Modifier::BOLD));
        let code = span("cargo test");
        assert_eq!(code.style.fg, Some(rgb(coder_ui::coder_noir::ANSI[6])));
        assert!(code.style.add_modifier.contains(Modifier::BOLD));
        let link = span("docs");
        assert_eq!(link.style.fg, Some(rgb(coder_ui::coder_noir::INFO)));
        assert!(link.style.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(
            span("•").style.fg,
            Some(rgb(coder_ui::coder_noir::CONTENT_TERTIARY))
        );
        assert_eq!(
            span("(https://x.dev)").style.fg,
            Some(rgb(coder_ui::coder_noir::CONTENT_TERTIARY))
        );
        assert_eq!(
            span("Run").style.fg,
            Some(rgb(coder_ui::coder_noir::CONTENT))
        );
        assert_eq!(
            span("plain").style.bg,
            Some(rgb(coder_ui::coder_noir::SURFACE_SUBTLE))
        );
    }

    #[test]
    fn a_markdown_body_uses_the_callers_full_width_without_turn_padding() {
        let ladder = Ladder::new(Colors::True);
        let source = "## Plan\n\n**one** two three\n\n```rust\nlet n = 1;\n```";
        let body = markdown_body(source, 13, ladder);
        assert_eq!(
            text(&body),
            ["Plan", "", "one two three", "", "let n = 1;   "]
        );
        assert!(body[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(
            body[0].spans[0].style.fg,
            Some(rgb(coder_ui::coder_noir::INFO))
        );
        assert!(
            body[2]
                .spans
                .iter()
                .find(|span| span.content == "one")
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
        let code = &body[4];
        assert_eq!(code.width(), 13);
        assert!(
            code.spans
                .iter()
                .all(|span| span.style.bg == Some(rgb(coder_ui::coder_noir::SURFACE_SUBTLE)))
        );
        let foregrounds: std::collections::HashSet<_> =
            code.spans.iter().filter_map(|span| span.style.fg).collect();
        assert!(foregrounds.len() > 1, "Rust tokens keep syntax colors");

        // The turn wrapper still reserves its content gutter and tail.
        let reply = streaming("one two three", 13, ladder);
        assert_eq!(text(&reply), [" one two", " three"]);
        assert_eq!(
            text(&markdown_body("one two three", 13, ladder)),
            ["one two three"]
        );
    }

    #[test]
    fn a_follow_up_hangs_under_its_text() {
        let lines = note(
            "[1] a follow up that wraps",
            Intensity::Half,
            18,
            Ladder::default(),
        );
        assert_eq!(text(&lines), [" [1] a follow up", "     that wraps"]);
    }

    #[test]
    fn nothing_is_wider_than_the_width() {
        let ladder = Ladder::default();
        for width in [1u16, 2, 3, 10, 40] {
            for line in turn(Who::You, "日本語 text that wraps", width, ladder) {
                // A wide character takes its two cells even in a one-cell row.
                assert!(line.width() <= usize::from(width).max(2), "{line}");
            }
        }
    }
}
