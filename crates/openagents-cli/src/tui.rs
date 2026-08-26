//! Terminal chrome for `oa coder`.
//!
//! The identity comes from two places that already exist. The `/coder` page in
//! the website (`lib/openagents_web/live/coder_live.ex`) draws a box whose
//! title sits inside the top rule as a badge — `┌───── OpenAgents ─────┐`, the
//! rule dim and the badge bright — which is the WebTUI `box-`/`shear-` shape
//! written out in characters. The TypeScript coder UI
//! (`packages/openagents-cli/src/coder-ui.ts`) supplies the transcript's
//! grammar: one bullet per turn in a four-column gutter, a colour per role, and
//! a `›` composer under a rule.
//!
//! This module owns only the drawing. It holds no session state, so every
//! frame it produces is a function of the view it is handed, which is what
//! makes the frames assertable in a test.

use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use ratatui::buffer::Buffer;
use unicode_width::UnicodeWidthStr;

/// Reset every foreground and background in an area to the terminal's own.
///
/// What `--no-color` does. It runs over the finished buffer, so it covers
/// every widget this module draws — including any added after it was written —
/// rather than depending on each colour site to remember the flag.
pub fn drain_color(buffer: &mut Buffer, area: Rect) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell_mut(Position::new(x, y)) {
                cell.set_fg(Color::Reset);
                cell.set_bg(Color::Reset);
            }
        }
    }
}

/// Columns reserved for the bullet before a turn's first line.
pub const GUTTER: usize = 4;

/// The bullet a settled turn carries, and the one a streaming turn pulses to.
pub const BULLET_SETTLED: &str = "⏺";
pub const BULLET_PULSE: &str = "○";

/// The composer's prompt.
pub const PROMPT: &str = "› ";

/// Who wrote a transcript entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    You,
    Assistant,
    Tool,
    Notice,
    Error,
}

impl Role {
    /// The colours are the ones `coder-ui.ts` uses, so the two surfaces read
    /// the same: cyan for the reader, green for the model, magenta for a tool,
    /// yellow for a notice, red for a failure.
    pub fn color(self) -> Color {
        match self {
            Role::You => Color::Cyan,
            Role::Assistant => Color::Green,
            Role::Tool => Color::Magenta,
            Role::Notice => Color::Yellow,
            Role::Error => Color::Red,
        }
    }
}

/// One turn on the transcript.
///
/// `settled` is false while a reply is still arriving; the bullet pulses and
/// an entry with no text yet draws an ellipsis rather than nothing, so a turn
/// that has been sent is visible before its first chunk lands.
#[derive(Clone, Debug)]
pub struct Entry {
    pub role: Role,
    pub text: String,
    pub settled: bool,
}

impl Entry {
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            settled: true,
        }
    }

    pub fn streaming(role: Role) -> Self {
        Self {
            role,
            text: String::new(),
            settled: false,
        }
    }
}

/// Everything a frame needs. Borrowed, never owned.
pub struct ChromeView<'a> {
    pub title: &'a str,
    pub entries: &'a [Entry],
    /// The composer's text, already soft-wrapped to [`composer_text_width`].
    pub composer_rows: &'a [&'a str],
    /// Caret position in the composer, as (row index, display column).
    pub composer_cursor: (usize, usize),
    /// The model the last grant named, or `None` before a turn has opened one.
    pub model: Option<&'a str>,
    /// True while a turn is streaming: the composer stops taking keys and says so.
    pub busy: bool,
    /// Flips on a timer to animate the streaming bullet.
    pub pulse: bool,
    /// Rows the reader has scrolled back from the bottom of the transcript.
    pub scrollback: usize,
}

pub struct BoxFrame {
    pub title: String,
}

impl BoxFrame {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
        }
    }

    /// The rule colour. Dim, like `foreground2` on the `/coder` page, so the
    /// badge and the content sit in front of the frame rather than inside it.
    fn rule() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    /// A title badge for the top rule: one space of air either side, bright,
    /// so it reads as set into the line rather than printed over it.
    fn badge(text: &str, color: Color) -> Line<'static> {
        Line::from(vec![Span::styled(
            format!(" {text} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )])
    }

    fn pane(title: &str, color: Color) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Self::rule())
            .title(Self::badge(title, color))
    }

    pub fn render(&self, f: &mut Frame, area: Rect, view: &ChromeView) {
        // The composer grows with what is typed, up to a third of the screen,
        // and the transcript pays for it. Two rows of frame plus at least one
        // row of text.
        let composer_rows = view.composer_rows.len().clamp(1, composer_cap(area.height));
        let composer_height = composer_rows as u16 + 2;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(composer_height),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_header(f, chunks[0]);
        render_transcript(f, chunks[1], view);
        render_composer(f, chunks[2], view);
        render_status(f, chunks[3], view);

        // `--no-color` is applied here rather than at each of the twenty-odd
        // places that name a colour, so a colour added later cannot escape it.
        // Bold and the other modifiers stay: they are structure, not colour,
        // and a terminal that renders no colour still renders them.
        if !crate::diag::color() {
            drain_color(f.buffer_mut(), area);
        }
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                " OpenAgents ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("│ ", Self::rule()),
            Span::styled(&self.title, Style::default().fg(Color::White)),
        ]))
        .block(Self::pane("Agent Context", Color::Cyan));
        f.render_widget(header, area);
    }
}

/// How many rows of composer text the screen can spare.
fn composer_cap(height: u16) -> usize {
    // The header, the status bar, the composer's own frame, and three rows of
    // transcript are not negotiable; whatever is left can go to the composer.
    let reserved = 3 + 3 + 2 + 3;
    usize::from(height.saturating_sub(reserved)).clamp(1, 8)
}

/// Break `text` into rows no wider than `width` columns, preferring to break
/// at a space. Hard newlines in the text are kept.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut rows = Vec::new();
    for hard in text.split('\n') {
        if hard.is_empty() {
            rows.push(String::new());
            continue;
        }
        let mut row = String::new();
        let mut row_width = 0usize;
        // The last column at which a break would land between words, and how
        // wide the row was there, so a break can be rewound to it.
        let mut break_at: Option<(usize, usize)> = None;
        for ch in hard.chars() {
            let w = ch.to_string().width().max(1);
            if row_width + w > width {
                match break_at {
                    Some((byte, _)) if byte < row.len() => {
                        let tail = row.split_off(byte);
                        rows.push(row.trim_end().to_string());
                        row = tail.trim_start().to_string();
                        row_width = row.width();
                    }
                    _ => {
                        rows.push(std::mem::take(&mut row));
                        row_width = 0;
                    }
                }
                break_at = None;
            }
            if ch == ' ' {
                break_at = Some((row.len(), row_width));
            }
            row.push(ch);
            row_width += w;
        }
        rows.push(row);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

/// The transcript, wrapped and windowed so the newest rows stay in view.
fn render_transcript(f: &mut Frame, area: Rect, view: &ChromeView) {
    let block = BoxFrame::pane("Transcript", Color::Cyan);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let body = usize::from(inner.width).saturating_sub(GUTTER).max(8);
    let mut rows: Vec<Line> = Vec::new();

    for entry in view.entries {
        // A settled entry with nothing in it is a turn that produced no text.
        // Drawing its bullet would say something happened when nothing did.
        if entry.settled && entry.text.is_empty() {
            continue;
        }
        if !rows.is_empty() {
            rows.push(Line::from(""));
        }
        let glyph = if entry.settled || view.pulse {
            BULLET_SETTLED
        } else {
            BULLET_PULSE
        };
        let head = Span::styled(
            format!("  {glyph} "),
            Style::default().fg(entry.role.color()),
        );
        let text_style = match entry.role {
            Role::Notice => Style::default().fg(Color::DarkGray),
            Role::Error => Style::default().fg(Color::Red),
            _ => Style::default(),
        };

        let wrapped = if entry.text.is_empty() {
            vec!["…".to_string()]
        } else {
            wrap(&entry.text, body)
        };
        for (index, row) in wrapped.into_iter().enumerate() {
            let lead = if index == 0 {
                head.clone()
            } else {
                Span::raw(" ".repeat(GUTTER))
            };
            rows.push(Line::from(vec![lead, Span::styled(row, text_style)]));
        }
    }

    let height = usize::from(inner.height);
    let top = rows
        .len()
        .saturating_sub(height)
        .saturating_sub(view.scrollback);
    let window: Vec<Line> = rows.into_iter().skip(top).take(height).collect();
    f.render_widget(Paragraph::new(window), inner);
}

/// The composer: a `›` prompt, the text, and a caret the reader can see.
fn render_composer(f: &mut Frame, area: Rect, view: &ChromeView) {
    let (title, color) = if view.busy {
        ("Message · waiting for the reply", Color::DarkGray)
    } else {
        ("Message", Color::Cyan)
    };
    let block = BoxFrame::pane(title, color);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text_style = if view.busy {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let prompt_style = Style::default().fg(if view.busy {
        Color::DarkGray
    } else {
        Color::Cyan
    });

    // The window follows the caret, so a composer taller than its pane still
    // shows the row being typed.
    let height = usize::from(inner.height).max(1);
    let first = view.composer_cursor.0.saturating_sub(height - 1);
    let lines: Vec<Line> = view
        .composer_rows
        .iter()
        .enumerate()
        .skip(first)
        .take(height)
        .map(|(index, row)| {
            // The prompt marks the composer, not each row, so a wrapped
            // continuation is indented under it rather than repeating it.
            let lead = if index == 0 {
                Span::styled(PROMPT, prompt_style)
            } else {
                Span::raw(" ".repeat(PROMPT.width()))
            };
            Line::from(vec![lead, Span::styled(*row, text_style)])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);

    if !view.busy {
        let (row, column) = view.composer_cursor;
        let x = inner.x + (PROMPT.width() + column) as u16;
        let y = inner.y + (row.saturating_sub(first)) as u16;
        if x < inner.right() && y < inner.bottom() {
            f.set_cursor_position(Position::new(x, y));
        }
    }
}

/// How wide the composer's text is, given the whole frame's width.
///
/// The composer wraps to this before the view is built, so the caller and the
/// renderer agree on where the rows break.
pub fn composer_text_width(frame_width: u16) -> usize {
    usize::from(frame_width)
        .saturating_sub(2 + PROMPT.width())
        .max(4)
}

/// The keys the status bar offers, most useful first.
///
/// Both of the ones the old bar advertised are gone, and neither could have
/// been made to work.
///
/// `Tab: effort` toggled nothing: it appended the words `[Toggled reasoning
/// effort]` to the transcript, and `execute_turn` has no effort field to send
/// even if it had meant it. `Shift+Tab: lane` was never handled at all.
///
/// A lane control here is buildable — `POST /api/v1/threads` does take a
/// `model`, and `oa coder --lane` uses it — but it would have to open a new
/// thread to change one, since the grant a thread returns pins the model for
/// that thread's whole life. Mid-session cycling is therefore a session
/// decision, not a keystroke. Until that exists, the bar reports the model the
/// grant named, which is a fact rather than a request.
const HINTS: [&str; 4] = [
    "Enter: send",
    "Esc: exit",
    "Alt+Enter: newline",
    "PgUp/PgDn: scroll",
];

/// Fit as many hints as the row holds, dropping them from the end.
///
/// A row written past the last column is truncated by the renderer, and half a
/// hint is worse than none: `PgU` names no key. A window too narrow for even
/// the first hint gets no hint row at all, and keeps its status and lane.
fn hint_row(hints: &[&str], budget: usize) -> String {
    let mut shown = hints.len();
    while shown > 0 {
        let row = hints[..shown].join(" · ");
        if row.width() <= budget {
            return row;
        }
        shown -= 1;
    }
    String::new()
}

/// The status bar. Every key it names is a key that works.
fn render_status(f: &mut Frame, area: Rect, view: &ChromeView) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(BoxFrame::rule());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (state, state_color) = if view.busy {
        ("streaming", Color::Yellow)
    } else {
        ("ready", Color::Green)
    };
    // While a turn streams the composer is on hold, so the keys that reach it
    // are not offered.
    let hints: &[&str] = if view.busy {
        &["Esc: exit", "PgUp/PgDn: scroll"]
    } else {
        &HINTS
    };

    const SEPARATOR: &str = " │ ";
    let mut spans = vec![
        Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(state, Style::default().fg(state_color)),
        Span::styled(SEPARATOR, BoxFrame::rule()),
        Span::styled("Model: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            view.model.unwrap_or("not yet granted"),
            Style::default().fg(if view.model.is_some() {
                Color::White
            } else {
                Color::DarkGray
            }),
        ),
    ];
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    let budget = usize::from(inner.width).saturating_sub(used + SEPARATOR.width());

    let row = hint_row(hints, budget);
    if !row.is_empty() {
        spans.push(Span::styled(SEPARATOR, BoxFrame::rule()));
        spans.push(Span::styled(row, Style::default().fg(Color::DarkGray)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(
            wrap("the quick brown fox", 10),
            vec!["the quick".to_string(), "brown fox".to_string()]
        );
    }

    #[test]
    fn wrap_splits_a_word_too_long_for_the_row() {
        assert_eq!(
            wrap("abcdefghij", 4),
            vec!["abcd".to_string(), "efgh".to_string(), "ij".to_string()]
        );
    }

    #[test]
    fn wrap_keeps_hard_newlines() {
        assert_eq!(wrap("a\nb", 10), vec!["a".to_string(), "b".to_string()]);
    }

    /// Draw the whole chrome into a buffer and report every foreground colour
    /// that is not the terminal's own.
    fn foregrounds(colour: bool) -> std::collections::BTreeSet<String> {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // The flag is process-wide, so it is set for the length of the draw
        // and put back. These two cases never run at the same time because
        // both live in this one test.
        crate::diag::set_color(colour);
        let entries = vec![
            Entry::new(Role::You, "a question"),
            Entry::new(Role::Assistant, "an answer"),
            Entry::new(Role::Error, "a failure"),
        ];
        let rows = ["typing"];
        let view = ChromeView {
            title: "openagents coder",
            entries: &entries,
            composer_rows: &rows,
            composer_cursor: (0, 6),
            model: Some("ox-alpha"),
            busy: false,
            pulse: true,
            scrollback: 0,
        };
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("a test terminal");
        terminal
            .draw(|frame| BoxFrame::new("openagents coder").render(frame, frame.area(), &view))
            .expect("draw the chrome");
        let buffer = terminal.backend().buffer().clone();
        // Put the flag back before anything else reads it.
        crate::diag::set_color(true);

        let mut seen = std::collections::BTreeSet::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if let Some(cell) = buffer.cell(Position::new(x, y)) {
                    if cell.fg != Color::Reset {
                        seen.insert(format!("{:?}", cell.fg));
                    }
                    if cell.bg != Color::Reset {
                        seen.insert(format!("{:?}", cell.bg));
                    }
                }
            }
        }
        seen
    }

    /// `--no-color` has to leave no colour in the drawn frame.
    ///
    /// Asserted against the same frame drawn both ways, so a test that could
    /// pass by the chrome having been colourless all along cannot: the
    /// coloured draw is required to carry several distinct colours.
    #[test]
    fn no_color_leaves_no_colour_in_the_frame() {
        let coloured = foregrounds(true);
        assert!(
            coloured.len() >= 3,
            "the coloured frame should carry several colours, it carried {coloured:?}"
        );
        assert!(
            coloured.contains("Cyan"),
            "the coloured frame lost its cyan: {coloured:?}"
        );

        let drained = foregrounds(false);
        assert!(
            drained.is_empty(),
            "--no-color left colours in the frame: {drained:?}"
        );
    }
}
