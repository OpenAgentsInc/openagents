//! A small terminal emulator: the VT100 and xterm subset a shell and common
//! full-screen programs use, applied to a character grid a renderer draws.
//!
//! [`Terminal::feed`] takes output bytes, such as the data of NIP-TERM
//! output frames, in order. The `vte` crate parses them; this crate keeps
//! the grid, the cursor, and the modes:
//!
//! - Printing with autowrap, UTF-8 including wide and combining characters,
//!   insert mode, and the DEC line-drawing character set.
//! - Cursor movement, save and restore, tab stops, origin mode, and scroll
//!   regions, with scrollback kept for the primary screen.
//! - Erase and insert/delete of characters and lines, with background
//!   color erase.
//! - Select Graphic Rendition: bold, dim, italic, underline, blink,
//!   inverse, hidden, strike, and 16, 256, and 24-bit colors.
//! - The alternate screen (modes 47, 1047, and 1049), cursor visibility,
//!   style, and blink (DECSCUSR, mode 12), application cursor keys and
//!   keypad, bracketed paste (mode 2004), focus events (mode 1004), and
//!   mouse reporting (modes 9, 1000, 1002, and 1003 with the 1005, 1006,
//!   and 1015 encodings; [`Terminal::mouse`]).
//! - Device status, cursor position, and device attribute replies, queued
//!   for the client to send back as input ([`Terminal::take_replies`]).
//! - The window title (OSC 0 and 2), hyperlinks (OSC 8;
//!   [`Attrs::link`] and [`Terminal::link`]), and clipboard writes (OSC
//!   52; [`Terminal::take_clipboard`]), which the client may refuse.
//!
//! Output is untrusted. A program can ask to write the client's clipboard
//! but never to read it; other operating-system commands are ignored.
//! Every string and count is bounded.
//!
//! [`input`] encodes keys and pastes the way xterm does, and [`mouse`]
//! encodes mouse reports.

mod cell;
pub mod input;
pub mod mouse;
pub mod shell;

use std::collections::{HashMap, VecDeque};

pub use cell::{Attrs, Cell, Color, Flags, Row, Run};
pub use input::{Key, KeyModes, Modifiers, encode_key, encode_key_in, encode_paste};
pub use mouse::{MouseButton, MouseEncoding, MouseEvent, MouseKind, MouseMode, encode_mouse};

/// The largest grid a terminal accepts, in each dimension. NIP-TERM bounds
/// sizes the same way.
pub const MAX_SIZE: usize = 1024;
/// The most characters of a window title kept.
pub const MAX_TITLE: usize = 256;
/// The most reply bytes held for the client to send.
pub const MAX_REPLIES: usize = 4096;
/// The most times one repeat request (`REP`) repeats a character.
const MAX_REPEAT: usize = 4096;
/// The most bytes of one OSC 52 clipboard write, as base64.
pub const MAX_CLIPBOARD: usize = 1 << 20;
/// The most distinct hyperlinks a terminal keeps; later ones show as
/// plain text.
pub const MAX_LINKS: usize = 4096;
/// The longest hyperlink target kept, in bytes.
pub const MAX_LINK: usize = 2048;

/// The cursor's shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

/// How the program asked the cursor to look (DECSCUSR and mode 12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CursorStyle {
    pub shape: CursorShape,
    pub blink: bool,
}

/// A character set a terminal can designate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Charset {
    #[default]
    Ascii,
    /// DEC special graphics: line drawing.
    LineDrawing,
}

#[derive(Clone, Copy, Debug)]
struct Cursor {
    row: usize,
    col: usize,
    attrs: Attrs,
    /// The last column was written; the next printed character wraps first.
    pending_wrap: bool,
}

#[derive(Clone, Copy, Debug)]
struct Saved {
    cursor: Cursor,
    origin: bool,
    charsets: [Charset; 2],
    shift: usize,
}

#[derive(Clone, Copy, Debug)]
struct Modes {
    autowrap: bool,
    origin: bool,
    insert: bool,
    newline: bool,
    application_cursor: bool,
    application_keypad: bool,
    cursor_visible: bool,
    bracketed_paste: bool,
    focus_events: bool,
    mouse: MouseMode,
    mouse_encoding: MouseEncoding,
    cursor_style: CursorStyle,
}

impl Default for Modes {
    fn default() -> Self {
        Modes {
            autowrap: true,
            origin: false,
            insert: false,
            newline: false,
            application_cursor: false,
            application_keypad: false,
            cursor_visible: true,
            bracketed_paste: false,
            focus_events: false,
            mouse: MouseMode::Off,
            mouse_encoding: MouseEncoding::Default,
            cursor_style: CursorStyle::default(),
        }
    }
}

/// The emulator's state apart from the parser.
struct State {
    rows: usize,
    cols: usize,
    primary: Vec<Row>,
    alternate: Vec<Row>,
    alternate_active: bool,
    scrollback: VecDeque<Row>,
    scrollback_max: usize,
    cursor: Cursor,
    saved_primary: Option<Saved>,
    saved_alternate: Option<Saved>,
    top: usize,
    bottom: usize,
    tabs: Vec<bool>,
    modes: Modes,
    charsets: [Charset; 2],
    shift: usize,
    last_printed: Option<char>,
    title: String,
    bells: u64,
    replies: Vec<u8>,
    /// Lines that left the front of the scrollback, ever.
    dropped: u64,
    /// The last clipboard write a program asked for, not yet taken.
    clipboard: Option<String>,
    /// Hyperlink targets; [`Attrs::link`] `n` is `links[n - 1]`.
    links: Vec<String>,
    link_ids: HashMap<String, u16>,
    shell: shell::Metadata,
    damage: Vec<bool>,
}

/// A terminal emulator for one grid.
pub struct Terminal {
    parser: vte::Parser,
    state: State,
    generation: u64,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("rows", &self.state.rows)
            .field("cols", &self.state.cols)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Terminal {
    /// A terminal of `rows` by `cols`, each clamped to 1 through
    /// [`MAX_SIZE`], keeping at most `scrollback` lines that scroll off the
    /// primary screen.
    #[must_use]
    pub fn new(rows: usize, cols: usize, scrollback: usize) -> Self {
        let rows = rows.clamp(1, MAX_SIZE);
        let cols = cols.clamp(1, MAX_SIZE);
        let blank = Row::blank(cols, Attrs::default());
        Terminal {
            parser: vte::Parser::new(),
            state: State {
                rows,
                cols,
                primary: vec![blank.clone(); rows],
                alternate: vec![blank; rows],
                alternate_active: false,
                scrollback: VecDeque::new(),
                scrollback_max: scrollback,
                cursor: Cursor {
                    row: 0,
                    col: 0,
                    attrs: Attrs::default(),
                    pending_wrap: false,
                },
                saved_primary: None,
                saved_alternate: None,
                top: 0,
                bottom: rows - 1,
                tabs: default_tabs(cols),
                modes: Modes::default(),
                charsets: [Charset::Ascii; 2],
                shift: 0,
                last_printed: None,
                title: String::new(),
                bells: 0,
                replies: Vec::new(),
                dropped: 0,
                clipboard: None,
                links: Vec::new(),
                link_ids: HashMap::new(),
                shell: shell::Metadata::default(),
                damage: vec![true; rows],
            },
            generation: 0,
        }
    }

    /// Applies output bytes. A sequence split across calls continues where
    /// the previous call stopped.
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.parser.advance(&mut self.state, bytes);
        self.generation += 1;
    }

    /// Writes a line the client composed, such as a note that the host
    /// discarded output, on a line of its own, marked with [`Flags::MARKER`].
    /// Any escape sequence the output was in the middle of is abandoned,
    /// since the bytes that would finish it are gone.
    pub fn mark(&mut self, text: &str) {
        self.parser = vte::Parser::new();
        let line =
            self.state.dropped + self.state.scrollback.len() as u64 + self.state.cursor.row as u64;
        self.state
            .shell
            .push(line, self.state.cursor.col, shell::Event::Gap);
        let state = &mut self.state;
        state.charsets = [Charset::Ascii; 2];
        state.shift = 0;
        if state.cursor.col > 0 || state.cursor.pending_wrap {
            state.carriage_return();
            state.linefeed();
        }
        state.cursor.attrs = Attrs {
            flags: Flags::MARKER,
            ..Attrs::default()
        };
        for character in text.chars().filter(|c| !c.is_control()) {
            state.print_char(character);
        }
        // The rendition the lost output left is unknown; start plain.
        state.cursor.attrs = Attrs::default();
        state.carriage_return();
        state.linefeed();
        self.generation += 1;
    }

    /// Changes the grid size, each dimension clamped to 1 through
    /// [`MAX_SIZE`]. Rows that no longer fit above the cursor move to the
    /// scrollback; the scroll region resets to the whole screen.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let rows = rows.clamp(1, MAX_SIZE);
        let cols = cols.clamp(1, MAX_SIZE);
        if rows == self.state.rows && cols == self.state.cols {
            return;
        }
        self.state.resize(rows, cols);
        self.generation += 1;
    }

    /// A counter that changes whenever the grid might have changed.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn rows(&self) -> usize {
        self.state.rows
    }

    #[must_use]
    pub fn cols(&self) -> usize {
        self.state.cols
    }

    /// The visible rows, top to bottom.
    #[must_use]
    pub fn screen(&self) -> &[Row] {
        self.state.grid()
    }

    /// One visible row.
    #[must_use]
    pub fn row(&self, row: usize) -> Option<&Row> {
        self.state.grid().get(row)
    }

    /// The visible text: one line per row, trailing blanks trimmed.
    #[must_use]
    pub fn text(&self) -> String {
        self.state
            .grid()
            .iter()
            .map(Row::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Lines that scrolled off the top of the primary screen, oldest first.
    pub fn scrollback(&self) -> impl ExactSizeIterator<Item = &Row> {
        self.state.scrollback.iter()
    }

    /// Lines in the scrollback.
    #[must_use]
    pub fn scrollback_len(&self) -> usize {
        self.state.scrollback.len()
    }

    /// Line `index` of the scrollback followed by the screen: 0 is the
    /// oldest scrollback line, and the screen's top row follows the
    /// newest.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<&Row> {
        let back = self.state.scrollback.len();
        if index < back {
            self.state.scrollback.get(index)
        } else {
            self.state.grid().get(index - back)
        }
    }

    /// How many lines ever left the front of the scrollback. A line's
    /// index in [`Terminal::line`] plus this count names it for as long as
    /// it is kept, however much output follows.
    #[must_use]
    pub fn history_dropped(&self) -> u64 {
        self.state.dropped
    }

    /// The cursor's shape and blink.
    #[must_use]
    pub fn cursor_style(&self) -> CursorStyle {
        self.state.modes.cursor_style
    }

    /// Which mouse events the program asked for.
    #[must_use]
    pub fn mouse_mode(&self) -> MouseMode {
        self.state.modes.mouse
    }

    /// The report a mouse event sends to the program, or `None` when it
    /// did not ask for it.
    #[must_use]
    pub fn mouse(&self, event: MouseEvent) -> Option<Vec<u8>> {
        encode_mouse(
            event,
            self.state.modes.mouse,
            self.state.modes.mouse_encoding,
        )
    }

    /// The report a focus change sends, when the program asked for focus
    /// events (mode 1004).
    #[must_use]
    pub fn focus(&self, focused: bool) -> Option<Vec<u8>> {
        self.state.modes.focus_events.then(|| {
            if focused {
                b"\x1b[I".to_vec()
            } else {
                b"\x1b[O".to_vec()
            }
        })
    }

    /// The text of the last clipboard write the program asked for (OSC
    /// 52), if any. Taking it clears it; the client decides whether to
    /// honor it. A program can never read the clipboard.
    pub fn take_clipboard(&mut self) -> Option<String> {
        self.state.clipboard.take()
    }

    /// The target of hyperlink `id` ([`Attrs::link`]).
    #[must_use]
    pub fn link(&self, id: u16) -> Option<&str> {
        let index = usize::from(id).checked_sub(1)?;
        self.state.links.get(index).map(String::as_str)
    }

    /// The cursor's row and column.
    #[must_use]
    pub fn cursor(&self) -> (usize, usize) {
        (self.state.cursor.row, self.state.cursor.col)
    }

    /// Whether the program shows the cursor (mode 25).
    #[must_use]
    pub fn cursor_visible(&self) -> bool {
        self.state.modes.cursor_visible
    }

    /// Whether the alternate screen is showing.
    #[must_use]
    pub fn alternate_screen(&self) -> bool {
        self.state.alternate_active
    }

    /// Whether cursor keys send application sequences (mode 1, DECCKM).
    #[must_use]
    pub fn application_cursor(&self) -> bool {
        self.state.modes.application_cursor
    }

    /// Whether the program asked for bracketed paste (mode 2004).
    #[must_use]
    pub fn bracketed_paste(&self) -> bool {
        self.state.modes.bracketed_paste
    }

    /// The window title the program set, bounded to [`MAX_TITLE`]
    /// characters.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.state.title
    }

    /// How many times the program rang the bell.
    #[must_use]
    pub fn bells(&self) -> u64 {
        self.state.bells
    }

    /// Encodes a key under the terminal's current cursor and keypad key
    /// modes.
    #[must_use]
    pub fn key(&self, key: Key, modifiers: Modifiers) -> Vec<u8> {
        encode_key_in(
            key,
            modifiers,
            KeyModes {
                application_cursor: self.state.modes.application_cursor,
                application_keypad: self.state.modes.application_keypad,
            },
        )
    }

    /// Encodes a paste under the terminal's current bracketed paste mode.
    #[must_use]
    pub fn paste(&self, text: &str) -> Vec<u8> {
        encode_paste(text, self.state.modes.bracketed_paste)
    }

    /// Takes advisory shell marks. They never authorize input or execution.
    pub fn take_shell_marks(&mut self) -> Vec<shell::Mark> {
        self.state.shell.events.drain(..).collect()
    }

    /// Takes the visible rows changed since the previous call.
    pub fn take_damage(&mut self) -> Vec<usize> {
        self.state
            .damage
            .iter_mut()
            .enumerate()
            .filter_map(|(row, dirty)| std::mem::take(dirty).then_some(row))
            .collect()
    }

    /// Replies the program asked for, such as its cursor position, for the
    /// client to send back as input. Taking them clears them.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.state.replies)
    }
}

fn default_tabs(cols: usize) -> Vec<bool> {
    (0..cols).map(|col| col > 0 && col % 8 == 0).collect()
}

/// The DEC special graphics glyph for `character`, when it has one.
fn line_drawing(character: char) -> char {
    match character {
        '`' => '◆',
        'a' => '▒',
        'b' => '␉',
        'c' => '␌',
        'd' => '␍',
        'e' => '␊',
        'f' => '°',
        'g' => '±',
        'h' => '␤',
        'i' => '␋',
        'j' => '┘',
        'k' => '┐',
        'l' => '┌',
        'm' => '└',
        'n' => '┼',
        'o' => '⎺',
        'p' => '⎻',
        'q' => '─',
        'r' => '⎼',
        's' => '⎽',
        't' => '├',
        'u' => '┤',
        'v' => '┴',
        'w' => '┬',
        'x' => '│',
        'y' => '≤',
        'z' => '≥',
        '{' => 'π',
        '|' => '≠',
        '}' => '£',
        '~' => '·',
        other => other,
    }
}

/// The first value of each parameter, with subparameters dropped.
fn values(params: &vte::Params) -> Vec<u16> {
    params
        .iter()
        .map(|param| param.first().copied().unwrap_or(0))
        .collect()
}

/// Parameter `index`, or `default` when it is absent or zero.
fn arg(values: &[u16], index: usize, default: usize) -> usize {
    match values.get(index) {
        Some(&value) if value > 0 => usize::from(value),
        _ => default,
    }
}

impl State {
    fn grid(&self) -> &[Row] {
        if self.alternate_active {
            &self.alternate
        } else {
            &self.primary
        }
    }

    fn dirty_row(&mut self, row: usize) -> &mut Row {
        self.damage[row] = true;
        if self.alternate_active {
            &mut self.alternate[row]
        } else {
            &mut self.primary[row]
        }
    }

    fn grid_mut(&mut self) -> &mut Vec<Row> {
        self.damage.fill(true);
        if self.alternate_active {
            &mut self.alternate
        } else {
            &mut self.primary
        }
    }

    fn blank(&self) -> Cell {
        Cell::blank(self.cursor.attrs.erased())
    }

    fn reply(&mut self, bytes: &[u8]) {
        if self.replies.len() + bytes.len() <= MAX_REPLIES {
            self.replies.extend_from_slice(bytes);
        }
    }

    // Printing.

    fn print_char(&mut self, character: char) {
        let character = if self.charsets[self.shift] == Charset::LineDrawing {
            line_drawing(character)
        } else {
            character
        };
        let Some(width) = unicode_width::UnicodeWidthChar::width(character) else {
            return;
        };
        if width == 0 {
            self.combine(character);
            return;
        }
        let width = width.min(2);
        if self.cursor.pending_wrap && self.modes.autowrap {
            self.wrap();
        }
        if width == 2 && self.cursor.col + 1 >= self.cols {
            if self.cols < 2 {
                return;
            }
            if self.modes.autowrap {
                // A wide character that does not fit goes to the next line.
                let (row, col) = (self.cursor.row, self.cursor.col);
                let blank = self.blank();
                self.clear_wide(row, col);
                self.dirty_row(row).cells[col] = blank;
                self.wrap();
            } else {
                self.cursor.col = self.cols - 2;
            }
        }
        let (row, col) = (self.cursor.row, self.cursor.col);
        if self.modes.insert {
            self.insert_cells(width);
        }
        self.clear_wide(row, col);
        if width == 2 {
            self.clear_wide(row, col + 1);
        }
        let attrs = self.cursor.attrs;
        let cells = &mut self.dirty_row(row).cells;
        cells[col] = Cell {
            ch: character,
            combining: Vec::new(),
            attrs,
            width: width as u8,
        };
        if width == 2 {
            cells[col + 1] = Cell::spacer(attrs);
        }
        self.last_printed = Some(character);
        if col + width >= self.cols {
            self.cursor.col = self.cols - 1;
            self.cursor.pending_wrap = self.modes.autowrap;
        } else {
            self.cursor.col = col + width;
            self.cursor.pending_wrap = false;
        }
    }

    /// Adds a zero-width character to the character before the cursor.
    fn combine(&mut self, character: char) {
        let row = self.cursor.row;
        let mut col = if self.cursor.pending_wrap {
            self.cursor.col
        } else if self.cursor.col > 0 {
            self.cursor.col - 1
        } else {
            return;
        };
        let cells = &mut self.dirty_row(row).cells;
        if cells[col].width == 0 && col > 0 {
            col -= 1;
        }
        // Bound what one cell can accumulate.
        if cells[col].combining.len() < 8 {
            cells[col].combining.push(character);
        }
    }

    /// Clears the other half of a wide character that `col` is part of.
    fn clear_wide(&mut self, row: usize, col: usize) {
        if col >= self.cols {
            return;
        }
        let width = self.grid()[row].cells[col].width;
        if width == 1 {
            return;
        }
        let blank = self.blank();
        let cells = &mut self.dirty_row(row).cells;
        match width {
            0 if col > 0 => cells[col - 1] = blank,
            2 if col + 1 < cells.len() => cells[col + 1] = blank,
            _ => {}
        }
    }

    fn wrap(&mut self) {
        let row = self.cursor.row;
        self.dirty_row(row).wrapped = true;
        self.cursor.col = 0;
        self.cursor.pending_wrap = false;
        self.linefeed();
    }

    // Cursor movement.

    fn carriage_return(&mut self) {
        self.cursor.col = 0;
        self.cursor.pending_wrap = false;
    }

    /// Moves down a line, scrolling the region at its bottom margin.
    fn linefeed(&mut self) {
        self.cursor.pending_wrap = false;
        if self.cursor.row == self.bottom {
            self.scroll_up(1);
        } else if self.cursor.row + 1 < self.rows {
            self.cursor.row += 1;
        }
    }

    /// Moves up a line, scrolling the region down at its top margin.
    fn reverse_index(&mut self) {
        self.cursor.pending_wrap = false;
        if self.cursor.row == self.top {
            self.scroll_down(1);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
    }

    fn goto(&mut self, row: usize, col: usize) {
        let (low, high) = if self.modes.origin {
            (self.top, self.bottom)
        } else {
            (0, self.rows - 1)
        };
        self.cursor.row = (low + row).min(high);
        self.cursor.col = col.min(self.cols - 1);
        self.cursor.pending_wrap = false;
    }

    fn up(&mut self, count: usize) {
        let limit = if self.cursor.row >= self.top {
            self.top
        } else {
            0
        };
        self.cursor.row = self.cursor.row.saturating_sub(count).max(limit);
        self.cursor.pending_wrap = false;
    }

    fn down(&mut self, count: usize) {
        let limit = if self.cursor.row <= self.bottom {
            self.bottom
        } else {
            self.rows - 1
        };
        self.cursor.row = (self.cursor.row + count).min(limit);
        self.cursor.pending_wrap = false;
    }

    fn tab_forward(&mut self, count: usize) {
        for _ in 0..count {
            let next = (self.cursor.col + 1..self.cols).find(|&col| self.tabs[col]);
            self.cursor.col = next.unwrap_or(self.cols - 1);
        }
        self.cursor.pending_wrap = false;
    }

    fn tab_backward(&mut self, count: usize) {
        for _ in 0..count {
            let previous = (0..self.cursor.col).rev().find(|&col| self.tabs[col]);
            self.cursor.col = previous.unwrap_or(0);
        }
        self.cursor.pending_wrap = false;
    }

    // Scrolling.

    /// Scrolls the region up by `count` lines. Lines leaving the top of a
    /// whole-screen region on the primary screen go to the scrollback.
    ///
    /// Rows are rotated rather than reallocated, and a full scrollback
    /// gives its oldest row's allocation to the new blank line.
    fn scroll_up(&mut self, count: usize) {
        let (top, bottom, cols) = (self.top, self.bottom, self.cols);
        let count = count.min(bottom - top + 1);
        let to_scrollback = top == 0 && !self.alternate_active && self.scrollback_max > 0;
        let erased = self.cursor.attrs.erased();
        let grid = if self.alternate_active {
            &mut self.alternate
        } else {
            &mut self.primary
        };
        self.damage[top..=bottom].fill(true);
        grid[top..=bottom].rotate_left(count);
        for row in &mut grid[bottom + 1 - count..=bottom] {
            if to_scrollback {
                let fresh = if self.scrollback.len() >= self.scrollback_max {
                    self.dropped += 1;
                    self.scrollback.pop_front().map(|mut old| {
                        old.reset(cols, erased);
                        old
                    })
                } else {
                    None
                };
                let fresh = fresh.unwrap_or_else(|| Row::blank(cols, erased));
                self.scrollback.push_back(std::mem::replace(row, fresh));
            } else {
                row.reset(cols, erased);
            }
        }
    }

    fn scroll_down(&mut self, count: usize) {
        let (top, bottom, cols) = (self.top, self.bottom, self.cols);
        let count = count.min(bottom - top + 1);
        let erased = self.cursor.attrs.erased();
        let grid = self.grid_mut();
        grid[top..=bottom].rotate_right(count);
        for row in &mut grid[top..top + count] {
            row.reset(cols, erased);
        }
    }

    // Editing.

    fn insert_cells(&mut self, count: usize) {
        let (row, col, cols) = (self.cursor.row, self.cursor.col, self.cols);
        let count = count.min(cols - col);
        let blank = self.blank();
        self.clear_wide(row, col);
        let cells = &mut self.dirty_row(row).cells;
        cells.truncate(cols - count);
        for _ in 0..count {
            cells.insert(col, blank.clone());
        }
        // A wide character pushed off the end loses its spacer.
        if cells[cols - 1].width == 2 {
            cells[cols - 1] = blank;
        }
    }

    fn delete_cells(&mut self, count: usize) {
        let (row, col, cols) = (self.cursor.row, self.cursor.col, self.cols);
        let count = count.min(cols - col);
        let blank = self.blank();
        self.clear_wide(row, col);
        let cells = &mut self.dirty_row(row).cells;
        cells.drain(col..col + count);
        cells.extend(std::iter::repeat_n(blank.clone(), count));
        // The right half of a wide character whose left half was deleted.
        if cells[col].width == 0 {
            cells[col] = blank;
        }
        self.cursor.pending_wrap = false;
    }

    fn erase_cells(&mut self, row: usize, from: usize, to: usize) {
        let to = to.min(self.cols);
        if from >= to {
            return;
        }
        self.clear_wide(row, from);
        self.clear_wide(row, to - 1);
        let blank = self.blank();
        for cell in &mut self.dirty_row(row).cells[from..to] {
            *cell = blank.clone();
        }
    }

    fn erase_rows(&mut self, from: usize, to: usize) {
        let (cols, erased) = (self.cols, self.cursor.attrs.erased());
        for row in &mut self.grid_mut()[from..to] {
            row.reset(cols, erased);
        }
    }

    fn insert_lines(&mut self, count: usize) {
        let row = self.cursor.row;
        if row < self.top || row > self.bottom {
            return;
        }
        let saved = self.top;
        self.top = row;
        self.scroll_down(count);
        self.top = saved;
        self.carriage_return();
    }

    fn delete_lines(&mut self, count: usize) {
        let row = self.cursor.row;
        if row < self.top || row > self.bottom {
            return;
        }
        let (saved, scrollback) = (self.top, self.scrollback_max);
        self.top = row;
        // Deleted lines never enter the scrollback.
        self.scrollback_max = 0;
        self.scroll_up(count);
        self.top = saved;
        self.scrollback_max = scrollback;
        self.carriage_return();
    }

    fn erase_display(&mut self, mode: usize) {
        let (row, col) = (self.cursor.row, self.cursor.col);
        match mode {
            0 => {
                self.erase_cells(row, col, self.cols);
                self.erase_rows(row + 1, self.rows);
            }
            1 => {
                self.erase_rows(0, row);
                self.erase_cells(row, 0, col + 1);
            }
            2 => self.erase_rows(0, self.rows),
            3 => {
                self.erase_rows(0, self.rows);
                self.dropped += self.scrollback.len() as u64;
                self.scrollback.clear();
            }
            _ => {}
        }
        self.cursor.pending_wrap = false;
    }

    fn erase_line(&mut self, mode: usize) {
        let (row, col) = (self.cursor.row, self.cursor.col);
        match mode {
            0 => self.erase_cells(row, col, self.cols),
            1 => self.erase_cells(row, 0, col + 1),
            2 => self.erase_cells(row, 0, self.cols),
            _ => {}
        }
        self.cursor.pending_wrap = false;
    }

    // Modes and state.

    fn save_cursor(&mut self) {
        let saved = Saved {
            cursor: self.cursor,
            origin: self.modes.origin,
            charsets: self.charsets,
            shift: self.shift,
        };
        if self.alternate_active {
            self.saved_alternate = Some(saved);
        } else {
            self.saved_primary = Some(saved);
        }
    }

    fn restore_cursor(&mut self) {
        let saved = if self.alternate_active {
            self.saved_alternate
        } else {
            self.saved_primary
        };
        match saved {
            Some(saved) => {
                self.cursor = saved.cursor;
                self.cursor.row = self.cursor.row.min(self.rows - 1);
                self.cursor.col = self.cursor.col.min(self.cols - 1);
                self.modes.origin = saved.origin;
                self.charsets = saved.charsets;
                self.shift = saved.shift;
            }
            None => {
                self.cursor.row = 0;
                self.cursor.col = 0;
                self.cursor.attrs = Attrs::default();
                self.cursor.pending_wrap = false;
                self.modes.origin = false;
            }
        }
    }

    fn enter_alternate(&mut self, clear: bool) {
        if !self.alternate_active {
            self.damage.fill(true);
            self.alternate_active = true;
            if clear {
                self.erase_rows(0, self.rows);
            }
        }
    }

    fn leave_alternate(&mut self) {
        self.damage.fill(true);
        self.alternate_active = false;
    }

    fn set_mode(&mut self, private: bool, mode: u16, on: bool) {
        if !private {
            match mode {
                4 => self.modes.insert = on,
                20 => self.modes.newline = on,
                _ => {}
            }
            return;
        }
        match mode {
            1 => self.modes.application_cursor = on,
            6 => {
                self.modes.origin = on;
                self.goto(0, 0);
            }
            7 => {
                self.modes.autowrap = on;
                self.cursor.pending_wrap = false;
            }
            9 | 1000 | 1002 | 1003 => {
                let mode = match mode {
                    9 => MouseMode::Press,
                    1000 => MouseMode::Click,
                    1002 => MouseMode::Drag,
                    _ => MouseMode::Motion,
                };
                if on {
                    self.modes.mouse = mode;
                } else if self.modes.mouse == mode {
                    self.modes.mouse = MouseMode::Off;
                }
            }
            1005 | 1006 | 1015 => {
                let encoding = match mode {
                    1005 => MouseEncoding::Utf8,
                    1006 => MouseEncoding::Sgr,
                    _ => MouseEncoding::Urxvt,
                };
                if on {
                    self.modes.mouse_encoding = encoding;
                } else if self.modes.mouse_encoding == encoding {
                    self.modes.mouse_encoding = MouseEncoding::Default;
                }
            }
            1004 => self.modes.focus_events = on,
            12 => self.modes.cursor_style.blink = on,
            25 => self.modes.cursor_visible = on,
            47 => {
                if on {
                    self.enter_alternate(false);
                } else {
                    self.leave_alternate();
                }
            }
            1047 => {
                if on {
                    self.enter_alternate(true);
                } else {
                    if self.alternate_active {
                        self.erase_rows(0, self.rows);
                    }
                    self.leave_alternate();
                }
            }
            1048 => {
                if on {
                    self.save_cursor();
                } else {
                    self.restore_cursor();
                }
            }
            1049 => {
                if on {
                    if !self.alternate_active {
                        self.save_cursor();
                        self.enter_alternate(true);
                    }
                } else if self.alternate_active {
                    self.leave_alternate();
                    self.restore_cursor();
                }
            }
            2004 => self.modes.bracketed_paste = on,
            _ => {}
        }
    }

    fn set_region(&mut self, top: usize, bottom: usize) {
        let bottom = bottom.min(self.rows);
        if top < bottom {
            self.top = top - 1;
            self.bottom = bottom - 1;
            self.goto(0, 0);
        }
    }

    fn soft_reset(&mut self) {
        self.modes = Modes {
            bracketed_paste: self.modes.bracketed_paste,
            ..Modes::default()
        };
        self.top = 0;
        self.bottom = self.rows - 1;
        self.cursor.attrs = Attrs::default();
        self.cursor.pending_wrap = false;
        self.charsets = [Charset::Ascii; 2];
        self.shift = 0;
        self.saved_primary = None;
        self.saved_alternate = None;
    }

    fn full_reset(&mut self) {
        let (rows, cols, scrollback, bells) =
            (self.rows, self.cols, self.scrollback_max, self.bells);
        let dropped = self.dropped + self.scrollback.len() as u64;
        let mut metadata = std::mem::take(&mut self.shell);
        metadata.push(
            dropped + self.cursor.row as u64,
            self.cursor.col,
            shell::Event::Gap,
        );
        let fresh = Terminal::new(rows, cols, scrollback).state;
        *self = fresh;
        self.shell = metadata;
        self.bells = bells;
        self.dropped = dropped;
    }

    /// The hyperlink ID for `target`, adding it while there is room.
    fn link_id(&mut self, target: &str) -> u16 {
        if let Some(&id) = self.link_ids.get(target) {
            return id;
        }
        if self.links.len() >= MAX_LINKS {
            return 0;
        }
        self.links.push(target.to_owned());
        let id = self.links.len() as u16;
        self.link_ids.insert(target.to_owned(), id);
        id
    }

    fn resize(&mut self, rows: usize, cols: usize) {
        let erased = self.cursor.attrs.erased();
        for grid in [&mut self.primary, &mut self.alternate] {
            for row in grid.iter_mut() {
                row.cells.resize(cols, Cell::blank(erased));
                if let Some(last) = row.cells.last_mut()
                    && last.width == 2
                {
                    *last = Cell::blank(erased);
                }
                if cols != self.cols {
                    row.wrapped = false;
                }
            }
        }
        // Rows above the cursor that no longer fit leave through the top.
        let excess = (self.cursor.row + 1).saturating_sub(rows);
        if excess > 0 {
            let removed: Vec<Row> = self.primary.drain(..excess).collect();
            if !self.alternate_active && self.scrollback_max > 0 {
                self.scrollback.extend(removed);
                while self.scrollback.len() > self.scrollback_max {
                    self.scrollback.pop_front();
                    self.dropped += 1;
                }
            }
            self.alternate.drain(..excess.min(self.alternate.len()));
            self.cursor.row -= excess;
        }
        for grid in [&mut self.primary, &mut self.alternate] {
            grid.truncate(rows);
            while grid.len() < rows {
                grid.push(Row::blank(cols, Attrs::default()));
            }
        }
        for row in &mut self.scrollback {
            row.cells.resize(cols, Cell::default());
        }
        self.damage = vec![true; rows];
        self.rows = rows;
        self.cols = cols;
        self.top = 0;
        self.bottom = rows - 1;
        let mut tabs = default_tabs(cols);
        for (col, stop) in self.tabs.iter().enumerate().take(cols) {
            tabs[col] = *stop;
        }
        self.tabs = tabs;
        self.cursor.row = self.cursor.row.min(rows - 1);
        self.cursor.col = self.cursor.col.min(cols - 1);
        self.cursor.pending_wrap = false;
        for saved in [&mut self.saved_primary, &mut self.saved_alternate]
            .into_iter()
            .flatten()
        {
            saved.cursor.row = saved.cursor.row.min(rows - 1);
            saved.cursor.col = saved.cursor.col.min(cols - 1);
        }
    }

    fn graphic_rendition(&mut self, params: &vte::Params) {
        let attrs = &mut self.cursor.attrs;
        let groups: Vec<&[u16]> = params.iter().collect();
        if groups.is_empty() {
            *attrs = Attrs::default();
            return;
        }
        let mut index = 0;
        while index < groups.len() {
            let group = groups[index];
            index += 1;
            let code = group.first().copied().unwrap_or(0);
            match code {
                0 => *attrs = Attrs::default(),
                1 => attrs.flags.insert(Flags::BOLD),
                2 => attrs.flags.insert(Flags::DIM),
                3 => attrs.flags.insert(Flags::ITALIC),
                4 => {
                    // `4:0` turns underline off; other styles count as on.
                    if group.get(1) == Some(&0) {
                        attrs.flags.remove(Flags::UNDERLINE);
                    } else {
                        attrs.flags.insert(Flags::UNDERLINE);
                    }
                }
                5 | 6 => attrs.flags.insert(Flags::BLINK),
                7 => attrs.flags.insert(Flags::INVERSE),
                8 => attrs.flags.insert(Flags::HIDDEN),
                9 => attrs.flags.insert(Flags::STRIKE),
                21 => attrs.flags.insert(Flags::UNDERLINE),
                22 => {
                    attrs.flags.remove(Flags::BOLD);
                    attrs.flags.remove(Flags::DIM);
                }
                23 => attrs.flags.remove(Flags::ITALIC),
                24 => attrs.flags.remove(Flags::UNDERLINE),
                25 => attrs.flags.remove(Flags::BLINK),
                27 => attrs.flags.remove(Flags::INVERSE),
                28 => attrs.flags.remove(Flags::HIDDEN),
                29 => attrs.flags.remove(Flags::STRIKE),
                30..=37 => attrs.fg = Color::Indexed((code - 30) as u8),
                38 => {
                    if let Some(color) = extended_color(group, &groups, &mut index) {
                        attrs.fg = color;
                    }
                }
                39 => attrs.fg = Color::Default,
                40..=47 => attrs.bg = Color::Indexed((code - 40) as u8),
                48 => {
                    if let Some(color) = extended_color(group, &groups, &mut index) {
                        attrs.bg = color;
                    }
                }
                49 => attrs.bg = Color::Default,
                90..=97 => attrs.fg = Color::Indexed((code - 90 + 8) as u8),
                100..=107 => attrs.bg = Color::Indexed((code - 100 + 8) as u8),
                _ => {}
            }
        }
    }
}

/// Reads a 256-color or 24-bit color after 38 or 48, in either the colon
/// form (`38:5:n` as one group) or the semicolon form (`38;5;n` as several
/// groups, advancing `index` past them).
fn extended_color(group: &[u16], groups: &[&[u16]], index: &mut usize) -> Option<Color> {
    let clamp = |value: u16| value.min(255) as u8;
    if group.len() > 1 {
        return match group[1] {
            5 => group.get(2).map(|&n| Color::Indexed(clamp(n))),
            // `38:2::r:g:b` may carry a color space before the components.
            2 if group.len() >= 6 => Some(Color::Rgb(
                clamp(group[3]),
                clamp(group[4]),
                clamp(group[5]),
            )),
            2 if group.len() == 5 => Some(Color::Rgb(
                clamp(group[2]),
                clamp(group[3]),
                clamp(group[4]),
            )),
            _ => None,
        };
    }
    let next = |offset: usize| groups.get(*index + offset).and_then(|g| g.first().copied());
    match next(0)? {
        5 => {
            let color = next(1).map(|n| Color::Indexed(clamp(n)));
            *index += 2;
            color
        }
        2 => {
            let color = match (next(1), next(2), next(3)) {
                (Some(r), Some(g), Some(b)) => Some(Color::Rgb(clamp(r), clamp(g), clamp(b))),
                _ => None,
            };
            *index += 4;
            color
        }
        _ => {
            *index += 1;
            None
        }
    }
}

impl vte::Perform for State {
    fn print(&mut self, character: char) {
        self.print_char(character);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.bells += 1,
            0x08 => {
                self.cursor.col = self.cursor.col.saturating_sub(1);
                self.cursor.pending_wrap = false;
            }
            0x09 => self.tab_forward(1),
            0x0a..=0x0c => {
                if self.modes.newline {
                    self.carriage_return();
                }
                self.linefeed();
            }
            0x0d => self.carriage_return(),
            0x0e => self.shift = 1,
            0x0f => self.shift = 0,
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if !self.alternate_active
            && let Some(event) = shell::parse(params)
        {
            let line = self.dropped + self.scrollback.len() as u64 + self.cursor.row as u64;
            self.shell.push(line, self.cursor.col, event);
            return;
        }
        // The parser splits at every `;`, which a title or a link target
        // may contain; join the rest back.
        let rest = |from: usize| params.get(from..).unwrap_or_default().join(&b';');
        match params.first().copied() {
            Some(b"0" | b"2") if params.len() > 1 => {
                self.title = String::from_utf8_lossy(&rest(1))
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(MAX_TITLE)
                    .collect();
            }
            Some(b"8") if params.len() > 2 => {
                let target = rest(2);
                self.cursor.attrs.link = match std::str::from_utf8(&target) {
                    Ok(target)
                        if !target.is_empty()
                            && target.len() <= MAX_LINK
                            && !target.chars().any(char::is_control) =>
                    {
                        self.link_id(target)
                    }
                    _ => 0,
                };
            }
            // Writes only: a query (`?`) is never answered.
            Some(b"52") if params.len() > 2 => {
                let data = rest(2);
                if data.len() <= MAX_CLIPBOARD
                    && data != b"?"
                    && let Some(bytes) = base64(&data)
                    && let Ok(text) = String::from_utf8(bytes)
                {
                    self.clipboard = Some(
                        text.chars()
                            .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                            .collect(),
                    );
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }
        let values = values(params);
        let n = arg(&values, 0, 1);
        let private = intermediates.first() == Some(&b'?');
        match (intermediates, action) {
            ([], 'A') => self.up(n),
            ([], 'B' | 'e') => self.down(n),
            ([], 'C' | 'a') => {
                self.cursor.col = (self.cursor.col + n).min(self.cols - 1);
                self.cursor.pending_wrap = false;
            }
            ([], 'D') => {
                self.cursor.col = self.cursor.col.saturating_sub(n);
                self.cursor.pending_wrap = false;
            }
            ([], 'E') => {
                self.down(n);
                self.carriage_return();
            }
            ([], 'F') => {
                self.up(n);
                self.carriage_return();
            }
            ([], 'G' | '`') => {
                self.cursor.col = (n - 1).min(self.cols - 1);
                self.cursor.pending_wrap = false;
            }
            ([], 'H' | 'f') => self.goto(arg(&values, 0, 1) - 1, arg(&values, 1, 1) - 1),
            ([], 'd') => {
                let col = self.cursor.col;
                self.goto(n - 1, col);
            }
            ([], 'I') => self.tab_forward(n),
            ([], 'Z') => self.tab_backward(n),
            ([] | [b'?'], 'J') => self.erase_display(arg(&values, 0, 0)),
            ([] | [b'?'], 'K') => self.erase_line(arg(&values, 0, 0)),
            ([], '@') => self.insert_cells(n),
            ([], 'P') => self.delete_cells(n),
            ([], 'X') => {
                let (row, col) = (self.cursor.row, self.cursor.col);
                self.erase_cells(row, col, col + n);
                self.cursor.pending_wrap = false;
            }
            ([], 'L') => self.insert_lines(n),
            ([], 'M') => self.delete_lines(n),
            ([], 'S') => self.scroll_up(n),
            // Five parameters is a mouse highlight request, not a scroll.
            ([], 'T') if values.len() <= 1 => self.scroll_down(n),
            ([], 'b') => {
                if let Some(character) = self.last_printed {
                    for _ in 0..n.min(MAX_REPEAT) {
                        self.print_char(character);
                    }
                }
            }
            ([], 'c') if arg(&values, 0, 0) == 0 => self.reply(b"\x1b[?1;2c"),
            ([b'>'], 'c') if arg(&values, 0, 0) == 0 => self.reply(b"\x1b[>0;0;0c"),
            ([], 'n') => match arg(&values, 0, 0) {
                5 => self.reply(b"\x1b[0n"),
                6 => {
                    let row = if self.modes.origin {
                        self.cursor.row.saturating_sub(self.top)
                    } else {
                        self.cursor.row
                    };
                    let answer = format!("\x1b[{};{}R", row + 1, self.cursor.col + 1);
                    self.reply(answer.as_bytes());
                }
                _ => {}
            },
            ([], 'g') => match arg(&values, 0, 0) {
                0 => {
                    let col = self.cursor.col;
                    self.tabs[col] = false;
                }
                3 => self.tabs.iter_mut().for_each(|stop| *stop = false),
                _ => {}
            },
            ([] | [b'?'], 'h' | 'l') => {
                let on = action == 'h';
                for &mode in &values {
                    self.set_mode(private, mode, on);
                }
            }
            ([], 'm') => self.graphic_rendition(params),
            ([], 'r') => {
                let rows = self.rows;
                self.set_region(arg(&values, 0, 1), arg(&values, 1, rows));
            }
            ([], 's') => self.save_cursor(),
            ([], 'u') => self.restore_cursor(),
            ([b'!'], 'p') => self.soft_reset(),
            ([b' '], 'q') => {
                let (shape, blink) = match arg(&values, 0, 0) {
                    0 | 1 => (CursorShape::Block, true),
                    2 => (CursorShape::Block, false),
                    3 => (CursorShape::Underline, true),
                    4 => (CursorShape::Underline, false),
                    5 => (CursorShape::Bar, true),
                    6 => (CursorShape::Bar, false),
                    _ => return,
                };
                self.modes.cursor_style = CursorStyle { shape, blink };
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore {
            return;
        }
        match (intermediates, byte) {
            ([], b'7') => self.save_cursor(),
            ([], b'8') => self.restore_cursor(),
            ([], b'D') => self.linefeed(),
            ([], b'E') => {
                self.carriage_return();
                self.linefeed();
            }
            ([], b'M') => self.reverse_index(),
            ([], b'H') => {
                let col = self.cursor.col;
                self.tabs[col] = true;
            }
            ([], b'c') => self.full_reset(),
            ([], b'=') => self.modes.application_keypad = true,
            ([], b'>') => self.modes.application_keypad = false,
            ([b'('], set) => self.charsets[0] = charset(set),
            ([b')'], set) => self.charsets[1] = charset(set),
            ([b'#'], b'8') => {
                // DECALN: fill the screen with `E`.
                let attrs = Attrs::default();
                for row in self.grid_mut() {
                    for cell in &mut row.cells {
                        *cell = Cell {
                            ch: 'E',
                            ..Cell::blank(attrs)
                        };
                    }
                }
                self.goto(0, 0);
            }
            _ => {}
        }
    }
}

/// Decodes standard base64, padding optional; `None` for anything else.
fn base64(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0;
    for &byte in data.iter().take_while(|&&b| b != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

fn charset(designator: u8) -> Charset {
    if designator == b'0' {
        Charset::LineDrawing
    } else {
        Charset::Ascii
    }
}

#[cfg(test)]
mod tests;
