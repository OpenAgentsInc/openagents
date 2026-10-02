//! The rail of delegations under the composer: one row per Coder run the
//! thread started, with its number, its agent, and what it is doing.
//!
//! Ported from Coder Terminal's delegation rail (`bins/coder-terminal/src/
//! delegation_panel.rs`, 0.4.0 "A rail under the composer"). A running row
//! leads with grok-build's spinner and ends with the timer of what it is
//! doing; a finished one leads with a dot and ends with "done". The
//! selected row leads with a chevron and draws at full white. Pure: the
//! caller says what each row is and the tick, and gets one line per row.

use std::time::Duration;

use ratatui::text::{Line, Span};

use super::{cells, cut, sanitize};
use crate::{Intensity, Ladder, grok_spinner};

/// The most rows the rail draws.
pub const MAX_ROWS: usize = 5;

/// One delegation as the rail shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailRow {
    /// Its number, from one: what Alt+number and `/open` take.
    pub number: usize,
    /// Who runs it: "Codex", "Claude Code", "Grok Build".
    pub agent: String,
    /// What it is doing now, as its live status says: "Thinking…",
    /// "Run cargo test".
    pub doing: String,
    /// How long it has been doing that; `None` once it finished.
    pub elapsed: Option<Duration>,
    /// The keyboard is on this row.
    pub selected: bool,
}

/// One line per row, each at most `width` cells.
#[must_use]
pub fn lines(rows: &[RailRow], width: usize, tick: u64, ladder: Ladder) -> Vec<Line<'static>> {
    rows.iter()
        .map(|row| line(row, width, tick, ladder))
        .collect()
}

fn line(row: &RailRow, width: usize, tick: u64, ladder: Ladder) -> Line<'static> {
    let running = row.elapsed.is_some();
    let text = ladder.style(if row.selected {
        Intensity::Full
    } else if running {
        Intensity::ThreeQuarters
    } else {
        Intensity::Half
    });
    let dim = ladder.style(Intensity::Half);
    let pointer = if row.selected { "›" } else { " " };
    let (mark, mark_style) = if running {
        (grok_spinner::frame(tick), grok_spinner::style(ladder))
    } else {
        ("·", dim)
    };
    let lead = format!("{} {} · ", row.number, sanitize(&row.agent));
    let end = match row.elapsed {
        Some(elapsed) => grok_spinner::timer(elapsed),
        None => "done".to_owned(),
    };
    // pointer, space, mark, space: four cells before the number.
    let room = width.saturating_sub(4);
    let lead = cut(&lead, room);
    let left = room.saturating_sub(cells(&lead));
    let end = if cells(&end) + 1 <= left {
        end
    } else {
        String::new()
    };
    let doing = cut(
        &sanitize(&row.doing),
        left.saturating_sub(if end.is_empty() { 0 } else { cells(&end) + 2 }),
    );
    let gap = left.saturating_sub(cells(&doing) + cells(&end));
    let mut spans = vec![
        Span::styled(pointer.to_owned(), text),
        Span::raw(" "),
        Span::styled(mark.to_owned(), mark_style),
        Span::raw(" "),
        Span::styled(lead, text),
        Span::styled(doing, text),
    ];
    if !end.is_empty() {
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(end, mark_style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Colors;

    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn row(number: usize, running: bool, selected: bool) -> RailRow {
        RailRow {
            number,
            agent: "Codex".into(),
            doing: "Run cargo test".into(),
            elapsed: running.then(|| Duration::from_secs(12)),
            selected,
        }
    }

    #[test]
    fn a_row_reads_number_agent_doing_and_timer() {
        let ladder = Ladder::new(Colors::None);
        let shown = lines(&[row(1, true, false), row(2, false, true)], 40, 0, ladder);
        assert_eq!(text(&shown[0]), "  ⠋ 1 Codex · Run cargo test         12s");
        assert_eq!(text(&shown[1]), "› · 2 Codex · Run cargo test        done");
        for line in &shown {
            assert!(cells(&text(line)) <= 40);
        }
    }

    #[test]
    fn a_narrow_rail_cuts_the_summary_first() {
        let ladder = Ladder::new(Colors::None);
        for width in 0..30 {
            let shown = lines(&[row(3, true, false)], width, 0, ladder);
            assert!(cells(&text(&shown[0])) <= width.max(4), "{width}");
        }
        let shown = lines(&[row(3, true, false)], 22, 0, ladder);
        assert_eq!(text(&shown[0]), "  ⠋ 3 Codex · Run  12s");
    }
}
