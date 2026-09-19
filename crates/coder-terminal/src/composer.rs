//! The composer: the framed input box at the foot of the terminal.
//!
//! A hairline frame holds the draft's wrapped rows behind ` > `, two rails
//! run inside the rules, and the caret is the terminal's own block over an
//! unmarked cell. Status and location
//! ride the top rail, left and right; the token count rides the bottom
//! right. A rail the rule cannot hold whole is left out rather than cut.
//!
//! The composer draws into a ratatui [`Buffer`]; the shell decides where the
//! box sits and, if it wants a real terminal cursor, uses the caret position
//! [`render`][Composer::render] returns.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use crate::editor::Editor;
use crate::intensity::Intensity;
use crate::ladder::Ladder;

/// The cells the prompt and its padding take: border, space, prompt, space,
/// then text — so the draft's room is `width - GUTTER`.
pub const GUTTER: usize = 5;

/// The prompt glyph, drawn on the first row only.
pub const PROMPT: char = '>';

/// The caret glyph — the block every terminal knows.
pub const CARET: char = '█';

/// The framed input box, drawn from an [`Editor`].
pub struct Composer<'a> {
    editor: &'a mut Editor,
    ladder: Ladder,
    prompt: char,
    caret: bool,
    status: Option<&'a str>,
    location: Option<&'a str>,
    tokens: Option<&'a str>,
}

impl<'a> Composer<'a> {
    /// A composer for `editor` on `ladder`, showing [`PROMPT`].
    pub fn new(editor: &'a mut Editor, ladder: Ladder) -> Self {
        Self {
            editor,
            ladder,
            prompt: PROMPT,
            caret: true,
            status: None,
            location: None,
            tokens: None,
        }
    }

    /// The glyph the first row shows — [`PROMPT`] while the shell waits, a
    /// frame of [`crate::spinner`] while it works.
    pub fn prompt(mut self, prompt: char) -> Self {
        self.prompt = prompt;
        self
    }

    /// Whether the caret draws; a blurred box draws none.
    pub fn caret(mut self, caret: bool) -> Self {
        self.caret = caret;
        self
    }

    /// The top rail's left text — the working status, `working` or a tool
    /// name.
    pub fn status(mut self, status: &'a str) -> Self {
        self.status = Some(status);
        self
    }

    /// The top rail's right text — the current directory or branch.
    pub fn location(mut self, location: &'a str) -> Self {
        self.location = Some(location);
        self
    }

    /// The bottom rail's right text — the running token count.
    pub fn tokens(mut self, tokens: &'a str) -> Self {
        self.tokens = Some(tokens);
        self
    }

    /// The height the box needs at `width`: the draft's visible rows plus
    /// the two rules.
    pub fn height(&mut self, width: u16) -> u16 {
        let inner = inner_width(width);
        let window = self.editor.window(inner);
        (window.visible + 2) as u16
    }

    /// Draws the box into `buf` at `area` and returns the caret's `(x, y)`
    /// — a shell that shows a real terminal cursor sets it there.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) -> (u16, u16) {
        let inner = inner_width(area.width);
        let window = self.editor.window(inner);
        let amber = self.ladder.style(Intensity::Full);
        let dim = self.ladder.style(Intensity::Half);
        let bg = self.ladder.background();
        let base = Style::new().bg(bg);
        let height = (window.visible + 2) as u16;
        let area = Rect {
            height: height.min(area.height),
            ..area
        };

        // The field and the hairline frame around it.
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_style(base);
            }
        }
        frame(area, buf, amber);

        rail(area, buf, 0, dim, self.status, self.location);
        rail(area, buf, height - 1, dim, None, self.tokens);

        let mut caret_at = (area.left() + 4, area.top() + 1);
        for offset in 0..window.visible as u16 {
            let y = area.top() + 1 + offset;
            let prompt = if offset == 0 { self.prompt } else { ' ' };
            buf[(area.left() + 2, y)].set_char(prompt).set_style(amber);
            let Some(range) = window.rows.get(window.scroll + offset as usize) else {
                continue;
            };
            let text = &self.editor.text()[range.clone()];
            buf.set_string(area.left() + 4, y, text, amber);
        }

        if self.caret {
            let (mut row, mut column) = self.editor.caret_row_column(inner);
            // A caret at the end of a full row has no cell of its own there,
            // so it shows at the start of the row under it.
            if column >= inner {
                row += 1;
                column = 0;
            }
            let x = area.left() + 4 + column as u16;
            let y = area.top() + 1 + (row.saturating_sub(window.scroll) as u16).min(height - 2);
            // The cell draws nothing of its own: the terminal's hardware
            // cursor lands on `caret_at` and inverts the cell, so the block
            // burns amber. A drawn █ would invert to near-black and cover
            // the cell — that is what made the caret read dark.
            caret_at = (x, y);
        }
        caret_at
    }
}

fn inner_width(width: u16) -> usize {
    usize::from(width).saturating_sub(GUTTER).max(1)
}

/// The hairline: `─` rules top and bottom, `│` walls, corners `┌┐└┘`.
fn frame(area: Rect, buf: &mut Buffer, style: Style) {
    let (top, bottom) = (area.top(), area.bottom() - 1);
    let (left, right) = (area.left(), area.right() - 1);
    for x in left + 1..right {
        buf[(x, top)].set_char('─').set_style(style);
        buf[(x, bottom)].set_char('─').set_style(style);
    }
    for y in top + 1..bottom {
        buf[(left, y)].set_char('│').set_style(style);
        buf[(right, y)].set_char('│').set_style(style);
    }
    buf[(left, top)].set_char('┌').set_style(style);
    buf[(right, top)].set_char('┐').set_style(style);
    buf[(left, bottom)].set_char('└').set_style(style);
    buf[(right, bottom)].set_char('┘').set_style(style);
}

/// Writes `left` and `right` into the rule at `offset`, each padded with a
/// space on both sides so the text never touches a corner. A rail wider
/// than the room left is dropped rather than cut.
fn rail(
    area: Rect,
    buf: &mut Buffer,
    offset: u16,
    style: Style,
    left: Option<&str>,
    right: Option<&str>,
) {
    let y = area.top() + offset;
    // The room the two rails share: the width less the corners and the rule
    // cell inside each.
    let mut room = usize::from(area.width).saturating_sub(4);
    if let Some(named) = left.map(|text| format!(" {text} ")) {
        let taken = named.width();
        if taken <= room {
            room -= taken;
            buf.set_string(area.left() + 2, y, &named, style);
        }
    }
    if let Some(named) = right.map(|text| format!(" {text} ")) {
        let taken = named.width();
        if taken <= room {
            buf.set_string(area.right() - 2 - taken as u16, y, &named, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{ROWS_MAX, ROWS_MIN};
    use crate::ladder::Colors;

    fn draw(editor: &mut Editor, width: u16) -> String {
        let mut composer = Composer::new(editor, Ladder::new(Colors::None));
        let height = composer.height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);
        buffer_text(&buf, area)
    }

    fn buffer_text(buf: &Buffer, area: Rect) -> String {
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn an_empty_composer_draws_two_rows_and_the_prompt() {
        let mut editor = Editor::new();
        let text = draw(&mut editor, 20);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "┌──────────────────┐");
        assert!(lines[1].starts_with("│ > "));
        assert!(!lines[1].contains(CARET));
        assert!(lines[2].starts_with("│   "));
        assert_eq!(lines[3], "└──────────────────┘");
    }

    #[test]
    fn a_wrapped_draft_breaks_at_the_last_space() {
        let mut editor = Editor::new();
        editor.insert_str("the quick brown fox jumps");
        let text = draw(&mut editor, 20);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[1].starts_with("│ > the quick"));
        assert!(lines[2].starts_with("│   brown fox jumps"));
    }

    #[test]
    fn rails_draw_left_and_right_when_they_fit() {
        let mut editor = Editor::new();
        let mut composer = Composer::new(&mut editor, Ladder::new(Colors::None))
            .status("working")
            .location("~/oa")
            .tokens("1200");
        let area = Rect::new(0, 0, 40, composer.height(40));
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);
        let text = buffer_text(&buf, area);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains(" working "));
        assert!(lines[0].contains(" ~/oa "));
        assert!(lines[3].contains(" 1200 "));
    }

    #[test]
    fn a_rail_too_wide_is_left_out() {
        let mut editor = Editor::new();
        let mut composer = Composer::new(&mut editor, Ladder::new(Colors::None))
            .status("a status far too long for this box");
        let area = Rect::new(0, 0, 20, composer.height(20));
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);
        assert!(!buffer_text(&buf, area).contains("status"));
    }

    #[test]
    fn the_caret_follows_the_draft() {
        let mut editor = Editor::new();
        editor.insert_str("hi");
        let mut composer = Composer::new(&mut editor, Ladder::new(Colors::None));
        let area = Rect::new(0, 0, 20, composer.height(20));
        let mut buf = Buffer::empty(area);
        let (x, y) = composer.render(area, &mut buf);
        assert_eq!((x, y), (6, 1));
    }

    #[test]
    fn a_long_draft_scrolls_its_window() {
        let mut editor = Editor::new();
        editor.insert_str(&"word ".repeat(30));
        let mut composer = Composer::new(&mut editor, Ladder::new(Colors::None));
        let height = composer.height(20);
        assert_eq!(height, (ROWS_MAX + 2) as u16);
        assert!(height <= (ROWS_MAX + 2) as u16);
        let _ = ROWS_MIN;
    }
}
