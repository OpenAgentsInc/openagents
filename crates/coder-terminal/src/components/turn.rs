//! A turn in the transcript: who spoke, then what they said.
//!
//! The person's text is plain and draws at the top of the ladder; a reply
//! is Markdown and draws at the step [`crate::markdown`] gives each line.
//! Both indent two cells under a quiet label row.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::{cells, cut, indent_at, wrap_hanging, wrap_paragraphs};
use crate::markdown::{self, Marks};
use crate::{Intensity, Ladder};

/// Who spoke a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    You,
    OpenAgents,
}

impl Who {
    /// The label row's text.
    pub const fn label(self) -> &'static str {
        match self {
            Who::You => "you",
            Who::OpenAgents => "openagents",
        }
    }
}

/// A finished turn: a label row ("you" or "openagents", `Intensity::Half`),
/// then the text indented two cells and wrapped to `width`, then a blank
/// line that separates it from the next turn.
///
/// The person's text is plain at `Intensity::Full`. A reply is Markdown:
/// each rendered line draws at the step the Markdown layout gives it (body
/// text at `ThreeQuarters`), code at `Full`, and bold, italic, strike, and
/// link marks as modifiers.
pub fn turn(who: Who, text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let mut lines = vec![label(who, None, width, ladder)];
    lines.extend(body(who, text, width, ladder));
    lines.push(Line::default());
    lines
}

/// A reply still streaming: like `turn(Who::OpenAgents, ..)`, but the label
/// row ends with a space and `spinner` (a spinner frame), and there is no
/// trailing blank line yet.
pub fn streaming(text: &str, spinner: char, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let mut lines = vec![label(Who::OpenAgents, Some(spinner), width, ladder)];
    lines.extend(body(Who::OpenAgents, text, width, ladder));
    lines
}

/// A note under a turn — an offer, a follow-up "[1] label", a notice —
/// indented two cells at `intensity`. Wrapped rows hang under the text:
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

fn label(who: Who, spinner: Option<char>, width: u16, ladder: Ladder) -> Line<'static> {
    let width = usize::from(width);
    let name = cut(who.label(), width);
    let mut spans = vec![Span::styled(name, ladder.style(Intensity::Half))];
    if let Some(spinner) = spinner.filter(|_| width >= who.label().len() + 2) {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            spinner.to_string(),
            ladder.style(Intensity::ThreeQuarters),
        ));
    }
    Line::from(spans)
}

fn body(who: Who, text: &str, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let cells_in = indent_at(usize::from(width));
    let room = usize::from(width).saturating_sub(cells_in).max(1);
    let indent = || Span::raw(" ".repeat(cells_in));
    match who {
        Who::You => {
            let style = ladder.style(Intensity::Full);
            text.split('\n')
                .flat_map(|line| wrap_hanging(&super::sanitize(line), room, 0))
                .map(|row| Line::from(vec![indent(), Span::styled(row, style)]))
                .collect()
        }
        Who::OpenAgents => markdown::wrapped(text, room)
            .into_iter()
            .map(|rendered| {
                let base = ladder.style(rendered.intensity);
                let mut spans = vec![indent()];
                for (run, marks) in rendered.marked.runs_in(0..rendered.marked.text.len()) {
                    spans.push(Span::styled(run, marked_style(base, ladder, &marks)));
                }
                Line::from(spans)
            })
            .collect(),
    }
}

/// A reply's inline marks over its line's base style: code at the top of
/// the ladder, the rest as modifiers.
pub(crate) fn marked_style(base: Style, ladder: Ladder, marks: &Marks) -> Style {
    let mut style = if marks.code {
        ladder.style(Intensity::Full)
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
    use crate::Colors;

    fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(|line| line.to_string()).collect()
    }

    #[test]
    fn a_turn_ends_with_a_blank_line_and_streaming_does_not() {
        let ladder = Ladder::new(Colors::True);
        let done = turn(Who::OpenAgents, "hi", 40, ladder);
        assert_eq!(text(&done), ["openagents", "  hi", ""]);
        let live = streaming("hi", '⠋', 40, ladder);
        assert_eq!(text(&live), ["openagents ⠋", "  hi"]);
    }

    #[test]
    fn a_follow_up_hangs_under_its_text() {
        let lines = note(
            "[1] a follow up that wraps",
            Intensity::Half,
            16,
            Ladder::default(),
        );
        assert_eq!(
            text(&lines),
            ["  [1] a follow", "      up that", "      wraps"]
        );
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
