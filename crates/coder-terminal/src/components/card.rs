//! A framed card in the transcript: the welcome card, a pairing code, help.
//!
//! The card is drawn as transcript lines, not into a buffer, so it scrolls
//! with the turns around it. Its frame is the same hairline as
//! [`crate::frame`], drawn as text: the rule at `Quarter`, the title set into
//! the top rule at `Full`, and the key hints set into the bottom rule.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::{cells, clip, cut, indent_at, wrap_paragraphs};
use crate::{Intensity, Ladder};

/// The widest a card draws, in cells, unless its art needs more.
pub const CARD_WIDTH_MAX: usize = 76;
/// Below this width a card drops its frame and draws its parts as plain
/// lines.
pub const CARD_WIDTH_MIN: usize = 20;

/// A framed card in the transcript: the welcome card, a pairing QR, help.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub title: String,
    /// Label/value rows; labels padded to the widest label, label at Half,
    /// value at ThreeQuarters; values wrap with a hanging indent.
    pub rows: Vec<(String, String)>,
    /// Prose paragraphs after the rows, wrapped, at ThreeQuarters, separated
    /// from rows by one blank row inside the frame.
    pub body: Vec<String>,
    /// Pre-drawn rows (a text QR code) drawn verbatim at Intensity::Full,
    /// never wrapped; clipped when wider than the card; centered.
    pub art: Vec<String>,
    /// Key hints, e.g. ("Enter", "send"), drawn on the bottom row as
    /// "Enter send · Ctrl+T threads" at Half.
    pub keys: Vec<(String, String)>,
}

/// One row inside the frame: spans and the cells they take.
type Row = (Vec<Span<'static>>, usize);

impl Card {
    /// The card as transcript lines at `width`: a hairline frame (corners
    /// like [`crate::frame`], rule at Quarter) whose width is
    /// `min(width, 76)` but at least wide enough for `art` when `width`
    /// allows, the title set into the top rule (Full), one cell of padding
    /// inside. Under width 20 it degrades to unframed lines rather than
    /// panicking. Key hints ride the bottom rule, or sit as the last rows
    /// inside the frame when the rule cannot hold them whole. There is no
    /// trailing blank line.
    pub fn lines(&self, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
        let width = usize::from(width);
        if width < CARD_WIDTH_MIN {
            return self.unframed(width, ladder);
        }
        let art_width = self.art.iter().map(|row| cells(row)).max().unwrap_or(0);
        let mut outer = width.min(CARD_WIDTH_MAX).max((art_width + 4).min(width));
        // A card of facts alone (no prose, art, or keys) is as wide as its
        // longest line, as the old Coder Terminal's welcome card was.
        if self.body.is_empty() && self.art.is_empty() && self.keys.is_empty() {
            let widest = self
                .rows
                .iter()
                .map(|(label, _)| cells(label))
                .max()
                .unwrap_or(0);
            let value = self
                .rows
                .iter()
                .map(|(_, value)| cells(value))
                .max()
                .unwrap_or(0);
            let natural = (widest + 2 + value).max(cells(&self.title) + 4) + 4;
            outer = outer.min(natural.max(CARD_WIDTH_MIN));
        }
        let inner = outer - 4;

        let mut content = self.content(inner, ladder);
        let keys = self.keys_text();
        // The bottom rule holds "└─ keys ─┘": six cells around the text.
        let keys_on_rule = !keys.is_empty() && cells(&keys) + 6 <= outer;
        if !keys.is_empty() && !keys_on_rule {
            if !content.is_empty() {
                content.push((Vec::new(), 0));
            }
            content.extend(prose(&keys, inner, ladder.style(Intensity::Half)));
        }

        let rule = ladder.style(Intensity::Quarter);
        let mut lines = Vec::with_capacity(content.len() + 2);
        lines.push(rule_line(
            '┌',
            '┐',
            &self.title,
            ladder.style(Intensity::Full),
            outer,
            rule,
        ));
        for (spans, used) in content {
            let mut line = vec![Span::styled("│ ", rule)];
            line.extend(spans);
            line.push(Span::raw(" ".repeat(inner.saturating_sub(used))));
            line.push(Span::styled(" │", rule));
            lines.push(Line::from(line));
        }
        let bottom = if keys_on_rule { keys.as_str() } else { "" };
        lines.push(rule_line(
            '└',
            '┘',
            bottom,
            ladder.style(Intensity::Half),
            outer,
            rule,
        ));
        lines
    }

    /// The hints as one line: "Enter send · Ctrl+T threads".
    fn keys_text(&self) -> String {
        self.keys
            .iter()
            .map(|(key, action)| format!("{key} {action}"))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// The rows inside the frame, `inner` cells wide at most: the label
    /// rows, the body, and the art, each section a blank row apart.
    fn content(&self, inner: usize, ladder: Ladder) -> Vec<Row> {
        let mut sections: Vec<Vec<Row>> = Vec::new();
        if !self.rows.is_empty() {
            sections.push(self.label_rows(inner, ladder));
        }
        for paragraph in &self.body {
            sections.push(prose(
                paragraph,
                inner,
                ladder.style(Intensity::ThreeQuarters),
            ));
        }
        if !self.art.is_empty() {
            sections.push(self.art_rows(inner, ladder));
        }
        let mut out = Vec::new();
        for (index, section) in sections.into_iter().enumerate() {
            if index > 0 {
                out.push((Vec::new(), 0));
            }
            out.extend(section);
        }
        out
    }

    fn label_rows(&self, inner: usize, ladder: Ladder) -> Vec<Row> {
        let label_style = ladder.style(Intensity::Half);
        let value_style = ladder.style(Intensity::ThreeQuarters);
        let widest = self
            .rows
            .iter()
            .map(|(label, _)| cells(label))
            .max()
            .unwrap_or(0);
        let column = widest.min(inner / 2);
        let room = inner.saturating_sub(column + 2).max(1);
        let mut out = Vec::new();
        for (label, value) in &self.rows {
            let label = clip(label, column);
            let pad = column - cells(&label);
            for (index, (_, row)) in wrap_paragraphs(value, room, 0).into_iter().enumerate() {
                let used = column + 2 + cells(&row);
                let lead = if index == 0 {
                    Span::styled(format!("{label}{}  ", " ".repeat(pad)), label_style)
                } else {
                    Span::raw(" ".repeat(column + 2))
                };
                out.push((vec![lead, Span::styled(row, value_style)], used));
            }
        }
        out
    }

    fn art_rows(&self, inner: usize, ladder: Ladder) -> Vec<Row> {
        let style = ladder.style(Intensity::Full);
        let art_width = self.art.iter().map(|row| cells(row)).max().unwrap_or(0);
        let left = inner.saturating_sub(art_width) / 2;
        self.art
            .iter()
            .map(|row| {
                let row = cut(row, inner - left);
                let used = left + cells(&row);
                (
                    vec![Span::raw(" ".repeat(left)), Span::styled(row, style)],
                    used,
                )
            })
            .collect()
    }

    /// The card without a frame, for widths too narrow to hold one.
    fn unframed(&self, width: usize, ladder: Ladder) -> Vec<Line<'static>> {
        let width = width.max(1);
        let mut lines = Vec::new();
        let mut push = |rows: Vec<Row>| {
            lines.extend(rows.into_iter().map(|(spans, _)| Line::from(spans)));
        };
        if !self.title.is_empty() {
            push(prose(&self.title, width, ladder.style(Intensity::Full)));
        }
        for (label, value) in &self.rows {
            let mut rows = prose(label, width, ladder.style(Intensity::Half));
            let indent = indent_at(width);
            rows.extend(
                wrap_paragraphs(value, width.saturating_sub(indent).max(1), 0)
                    .into_iter()
                    .map(|(_, row)| {
                        let used = indent + cells(&row);
                        (
                            vec![
                                Span::raw(" ".repeat(indent)),
                                Span::styled(row, ladder.style(Intensity::ThreeQuarters)),
                            ],
                            used,
                        )
                    }),
            );
            push(rows);
        }
        for paragraph in &self.body {
            push(prose(
                paragraph,
                width,
                ladder.style(Intensity::ThreeQuarters),
            ));
        }
        let art = ladder.style(Intensity::Full);
        push(
            self.art
                .iter()
                .map(|row| {
                    let row = cut(row, width);
                    let used = cells(&row);
                    (vec![Span::styled(row, art)], used)
                })
                .collect(),
        );
        let keys = self.keys_text();
        if !keys.is_empty() {
            push(prose(&keys, width, ladder.style(Intensity::Half)));
        }
        lines
    }
}

/// `text` wrapped to `width` at `style`, one row per wrapped row.
fn prose(text: &str, width: usize, style: Style) -> Vec<Row> {
    wrap_paragraphs(text, width, 0)
        .into_iter()
        .map(|(_, row)| {
            let used = cells(&row);
            (vec![Span::styled(row, style)], used)
        })
        .collect()
}

/// A top or bottom rule `outer` cells wide with `text` set into it after
/// one rule cell: "┌─ text ───┐". Text too long for the rule is clipped.
fn rule_line(
    left: char,
    right: char,
    text: &str,
    text_style: Style,
    outer: usize,
    rule: Style,
) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{left}─"), rule)];
    let mut used = 2;
    if !text.is_empty() && outer >= 7 {
        let text = clip(text, outer - 6);
        used += cells(&text) + 2;
        spans.push(Span::styled(format!(" {text} "), text_style));
    }
    spans.push(Span::styled(
        format!("{}{right}", "─".repeat(outer.saturating_sub(used + 1))),
        rule,
    ));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card {
            title: "OpenAgents".into(),
            rows: vec![("model".into(), "a long value that wraps".into())],
            body: vec!["Ask anything.".into()],
            art: vec!["#".repeat(30)],
            keys: vec![("Enter".into(), "send".into())],
        }
    }

    #[test]
    fn a_card_of_facts_is_as_wide_as_its_longest_line() {
        let facts = Card {
            title: "OpenAgents v1.0.0".into(),
            rows: vec![
                ("Project".into(), "openagents".into()),
                ("Agents".into(), "Codex · Claude Code".into()),
            ],
            body: Vec::new(),
            art: Vec::new(),
            keys: Vec::new(),
        };
        let lines = facts.lines(120, Ladder::new(crate::Colors::None));
        let width = |line: &Line<'_>| {
            cells(
                &line
                    .spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>(),
            )
        };
        // "Agents   Codex · Claude Code": 7 + 2 + 19, inside "│ … │".
        assert_eq!(width(&lines[0]), 7 + 2 + 19 + 4);
        assert!(lines.iter().all(|line| width(line) == width(&lines[0])));
    }

    #[test]
    fn every_line_is_exactly_the_frame_width() {
        let ladder = Ladder::default();
        for width in [20u16, 33, 40, 80, 120] {
            let lines = card().lines(width, ladder);
            let outer = usize::from(width).min(CARD_WIDTH_MAX);
            for line in &lines {
                assert_eq!(line.width(), outer, "{line}");
            }
        }
    }

    #[test]
    fn narrow_widths_drop_the_frame_without_panicking() {
        for width in 0u16..CARD_WIDTH_MIN as u16 {
            for line in card().lines(width, Ladder::default()) {
                assert!(line.width() <= usize::from(width).max(1), "{line}");
                assert!(!line.to_string().contains('│'));
            }
        }
    }

    #[test]
    fn art_widens_the_card_when_the_width_allows() {
        let card = Card {
            art: vec!["x".repeat(90)],
            ..Card::default()
        };
        assert_eq!(card.lines(120, Ladder::default())[0].width(), 94);
        assert_eq!(card.lines(80, Ladder::default())[0].width(), 80);
    }
}
