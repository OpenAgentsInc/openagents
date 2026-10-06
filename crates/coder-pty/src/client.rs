//! A client's view of one terminal: the frames it has applied, the ranges
//! it knows it missed, and a bounded screen buffer a renderer draws.
//!
//! This is portable state. It builds without the host feature and makes no
//! system call, so the desktop, web, and mobile clients share it.
//!
//! A host delivers an attachment's frames in sequence order. When a frame
//! arrives past the next expected sequence number without a gap frame
//! before it, a transport lost something in between. The state then stops
//! applying output and reports [`Applied::Behind`]; the client reattaches
//! with `after` set to [`TerminalState::resume_after`], and the host either
//! replays the frames or says with a gap frame that it discarded them.
//!
//! The [`Screen`] is a plain-text scrollback, not a terminal emulator: it
//! keeps printable text, applies carriage return, line feed, backspace,
//! and tab, and drops escape sequences. A renderer that needs full
//! emulation reads the output bytes from [`Applied::Output`] instead.

use std::collections::VecDeque;

use crate::wire::{Body, Detached, Exit, Frame, Reason, Refusal, TerminalRef};

/// The most missed ranges a state remembers; older ones are merged into a
/// count.
const MISSING_MAX: usize = 64;

/// What applying one frame did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    /// Output was applied to the screen.
    Output { seq: u64, bytes: usize },
    /// A frame this state already applied. Nothing changed.
    Duplicate,
    /// The host reported frames it discarded before this client read them.
    Gap {
        from: u64,
        to: u64,
        bytes: Option<u64>,
    },
    /// A frame arrived ahead of the next expected one: something between
    /// them was lost in transit. Nothing was applied; reattach after
    /// [`TerminalState::resume_after`].
    Behind { expected: u64, got: u64 },
    /// The terminal's process ended.
    Exit(Exit),
    /// The host ended this attachment.
    Detached(Detached),
    /// The host's emulator reported an effect of the output (the effects
    /// feature). Nothing on the screen changed.
    Effect(crate::ext::Effect),
    /// The typist role moved (the typist feature): the typist's
    /// attachment, or none, and the size the terminal runs at.
    Typist {
        typist: Option<String>,
        size: crate::wire::Size,
    },
    /// Sharing paused or resumed (the shares feature). Blank the pane
    /// while paused; the output from the pause arrives as a gap.
    Paused(bool),
    /// The frame failed validation or names another terminal.
    Refused(Refusal),
}

/// A client's state for one terminal.
#[derive(Clone, Debug)]
pub struct TerminalState {
    terminal: TerminalRef,
    applied: u64,
    missing: VecDeque<(u64, u64)>,
    missed_bytes: u64,
    missed_unknown: bool,
    behind: bool,
    exit: Option<Exit>,
    detached: Option<Detached>,
    screen: Screen,
}

impl TerminalState {
    /// State for `terminal` with a screen of at most `lines` lines of at
    /// most `columns` characters.
    #[must_use]
    pub fn new(terminal: TerminalRef, lines: usize, columns: usize) -> Self {
        TerminalState {
            terminal,
            applied: 0,
            missing: VecDeque::new(),
            missed_bytes: 0,
            missed_unknown: false,
            behind: false,
            exit: None,
            detached: None,
            screen: Screen::new(lines, columns),
        }
    }

    /// The same state for a client that attaches with `after` set: it
    /// expects frame `after + 1` next and treats earlier ones as seen.
    #[must_use]
    pub fn starting_after(mut self, after: u64) -> Self {
        self.applied = after;
        self
    }

    /// Applies one frame.
    pub fn apply(&mut self, frame: &Frame) -> Applied {
        if let Err(refusal) = frame.check() {
            return Applied::Refused(refusal);
        }
        if frame.terminal != self.terminal {
            return Applied::Refused(Refusal::new(
                Reason::Malformed,
                "the frame names a different terminal",
            ));
        }
        match &frame.body {
            Body::Output { seq, data } => {
                if let Some(applied) = self.sequenced(*seq) {
                    return applied;
                }
                self.screen.feed(data);
                Applied::Output {
                    seq: *seq,
                    bytes: data.len(),
                }
            }
            Body::Exit { seq, exit } => {
                if let Some(applied) = self.sequenced(*seq) {
                    return applied;
                }
                self.exit = Some(*exit);
                Applied::Exit(*exit)
            }
            Body::Gap { from, to, bytes } => {
                if *to <= self.applied {
                    return Applied::Duplicate;
                }
                if *from > self.applied + 1 {
                    self.behind = true;
                    return Applied::Behind {
                        expected: self.applied + 1,
                        got: *from,
                    };
                }
                self.behind = false;
                self.record_missing(self.applied + 1, *to);
                match bytes {
                    Some(bytes) => self.missed_bytes += bytes,
                    None => self.missed_unknown = true,
                }
                self.applied = *to;
                self.screen.gap(*bytes);
                Applied::Gap {
                    from: *from,
                    to: *to,
                    bytes: *bytes,
                }
            }
            Body::Detached { reason } => {
                self.detached = Some(*reason);
                Applied::Detached(*reason)
            }
            Body::Effect { effect, .. } => Applied::Effect(effect.clone()),
            Body::Typist { typist, size } => Applied::Typist {
                typist: typist.clone(),
                size: *size,
            },
            Body::Paused { paused } => Applied::Paused(*paused),
        }
    }

    /// Checks a sequenced frame's position: `None` when it is the next one
    /// and has been counted, otherwise what to report instead.
    fn sequenced(&mut self, seq: u64) -> Option<Applied> {
        if seq <= self.applied {
            return Some(Applied::Duplicate);
        }
        if seq > self.applied + 1 {
            self.behind = true;
            return Some(Applied::Behind {
                expected: self.applied + 1,
                got: seq,
            });
        }
        self.behind = false;
        self.applied = seq;
        None
    }

    fn record_missing(&mut self, from: u64, to: u64) {
        if let Some(last) = self.missing.back_mut()
            && last.1 + 1 == from
        {
            last.1 = to;
            return;
        }
        self.missing.push_back((from, to));
        if self.missing.len() > MISSING_MAX {
            self.missing.pop_front();
        }
    }

    /// The sequence number to reattach after: every frame through it has
    /// been applied or reported missing.
    #[must_use]
    pub fn resume_after(&self) -> u64 {
        self.applied
    }

    /// Whether a frame arrived ahead of the expected one and the client
    /// needs to reattach to repair it.
    #[must_use]
    pub fn behind(&self) -> bool {
        self.behind
    }

    /// The ranges of sequence numbers the host reported discarded, most
    /// recent last, up to the last 64.
    pub fn missing(&self) -> impl Iterator<Item = (u64, u64)> + '_ {
        self.missing.iter().copied()
    }

    /// Output bytes the host reported discarded, and whether some gaps had
    /// an unknown size on top of that.
    #[must_use]
    pub fn missed_bytes(&self) -> (u64, bool) {
        (self.missed_bytes, self.missed_unknown)
    }

    /// How the process ended, once it has.
    #[must_use]
    pub fn exit(&self) -> Option<Exit> {
        self.exit
    }

    /// Why the host ended the attachment, if it did.
    #[must_use]
    pub fn detached(&self) -> Option<Detached> {
        self.detached
    }

    /// The terminal this state follows.
    #[must_use]
    pub fn terminal(&self) -> &TerminalRef {
        &self.terminal
    }

    #[must_use]
    pub fn screen(&self) -> &Screen {
        &self.screen
    }
}

/// One line of the screen buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Text(String),
    /// Output the host discarded before this client read it. `bytes` is
    /// how much, when the host knew.
    Gap {
        bytes: Option<u64>,
    },
}

/// Where an escape sequence parse stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Escape {
    None,
    /// After `ESC`.
    Start,
    /// Inside a control sequence (`ESC [`), until a final byte.
    Control,
    /// Inside an operating-system command (`ESC ]`), until `BEL` or `ESC \`.
    Command,
    /// After `ESC` inside an operating-system command.
    CommandEnd,
    /// One more byte to drop, after a character-set designator.
    Designator,
}

/// A bounded plain-text scrollback.
#[derive(Clone, Debug)]
pub struct Screen {
    lines: VecDeque<Line>,
    current: Vec<char>,
    column: usize,
    max_lines: usize,
    max_columns: usize,
    utf8: Vec<u8>,
    escape: Escape,
    dropped: u64,
}

impl Screen {
    /// A screen of at most `lines` lines, wrapping at `columns`
    /// characters. Both are at least one.
    #[must_use]
    pub fn new(lines: usize, columns: usize) -> Self {
        Screen {
            lines: VecDeque::new(),
            current: Vec::new(),
            column: 0,
            max_lines: lines.max(1),
            max_columns: columns.max(1),
            utf8: Vec::new(),
            escape: Escape::None,
            dropped: 0,
        }
    }

    /// The lines, oldest first, ending with the line being written.
    pub fn lines(&self) -> impl Iterator<Item = Line> + '_ {
        self.lines
            .iter()
            .cloned()
            .chain(std::iter::once(Line::Text(self.current.iter().collect())))
    }

    /// The text lines joined with newlines, gaps shown as empty lines. For
    /// tests and simple renderers.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines()
            .map(|line| match line {
                Line::Text(text) => text,
                Line::Gap { .. } => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Lines dropped off the top to keep the bound.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    fn push_line(&mut self, line: Line) {
        self.lines.push_back(line);
        while self.lines.len() + 1 > self.max_lines {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    fn newline(&mut self) {
        let text: String = self.current.drain(..).collect();
        self.column = 0;
        self.push_line(Line::Text(text));
    }

    /// Records a gap on a line of its own.
    pub fn gap(&mut self, bytes: Option<u64>) {
        if !self.current.is_empty() {
            self.newline();
        }
        self.utf8.clear();
        self.escape = Escape::None;
        self.push_line(Line::Gap { bytes });
    }

    /// Applies output bytes.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.byte(byte);
        }
    }

    fn byte(&mut self, byte: u8) {
        match self.escape {
            Escape::None => {}
            Escape::Start => {
                self.escape = match byte {
                    b'[' => Escape::Control,
                    b']' => Escape::Command,
                    b'(' | b')' | b'*' | b'+' | b'#' => Escape::Designator,
                    _ => Escape::None,
                };
                return;
            }
            Escape::Control => {
                if (0x40..=0x7e).contains(&byte) {
                    self.escape = Escape::None;
                }
                return;
            }
            Escape::Command => {
                self.escape = match byte {
                    0x07 => Escape::None,
                    0x1b => Escape::CommandEnd,
                    _ => Escape::Command,
                };
                return;
            }
            Escape::CommandEnd => {
                self.escape = if byte == b'\\' {
                    Escape::None
                } else {
                    Escape::Command
                };
                return;
            }
            Escape::Designator => {
                self.escape = Escape::None;
                return;
            }
        }
        if byte >= 0x80 || !self.utf8.is_empty() {
            self.continue_utf8(byte);
            return;
        }
        match byte {
            0x1b => self.escape = Escape::Start,
            b'\n' => self.newline(),
            b'\r' => self.column = 0,
            0x08 => self.column = self.column.saturating_sub(1),
            b'\t' => {
                let stop = (self.column / 8 + 1) * 8;
                while self.column < stop.min(self.max_columns) {
                    self.put(' ');
                }
            }
            0x20..=0x7e => self.put(char::from(byte)),
            _ => {}
        }
    }

    fn continue_utf8(&mut self, byte: u8) {
        if byte < 0x80 {
            // A multi-byte character cut short by an ASCII byte.
            self.utf8.clear();
            self.put(char::REPLACEMENT_CHARACTER);
            self.byte(byte);
            return;
        }
        self.utf8.push(byte);
        match std::str::from_utf8(&self.utf8) {
            Ok(text) => {
                let characters: Vec<char> = text.chars().collect();
                self.utf8.clear();
                for character in characters {
                    self.put(character);
                }
            }
            Err(error) if error.error_len().is_none() && self.utf8.len() < 4 => {}
            Err(_) => {
                self.utf8.clear();
                self.put(char::REPLACEMENT_CHARACTER);
            }
        }
    }

    fn put(&mut self, character: char) {
        if self.column >= self.max_columns {
            self.newline();
        }
        while self.current.len() < self.column {
            self.current.push(' ');
        }
        if self.column < self.current.len() {
            self.current[self.column] = character;
        } else {
            self.current.push(character);
        }
        self.column += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Cause;

    const ID: &str = "0202020202020202020202020202020202020202020202020202020202020202";

    fn terminal() -> TerminalRef {
        TerminalRef {
            generation: ID.into(),
            terminal: ID.into(),
        }
    }

    fn frame(body: Body) -> Frame {
        Frame::new(terminal(), ID, body)
    }

    fn output(seq: u64, data: &str) -> Frame {
        frame(Body::Output {
            seq,
            data: data.as_bytes().to_vec(),
        })
    }

    #[test]
    fn output_applies_in_order_and_duplicates_are_ignored() {
        let mut state = TerminalState::new(terminal(), 100, 80);
        assert_eq!(
            state.apply(&output(1, "one\r\n")),
            Applied::Output { seq: 1, bytes: 5 }
        );
        assert_eq!(
            state.apply(&output(2, "two")),
            Applied::Output { seq: 2, bytes: 3 }
        );
        assert_eq!(state.apply(&output(2, "two")), Applied::Duplicate);
        assert_eq!(state.screen().text(), "one\ntwo");
        assert_eq!(state.resume_after(), 2);
    }

    #[test]
    fn a_frame_ahead_of_the_expected_one_is_held_back() {
        let mut state = TerminalState::new(terminal(), 100, 80);
        state.apply(&output(1, "a"));
        assert_eq!(
            state.apply(&output(3, "c")),
            Applied::Behind {
                expected: 2,
                got: 3
            }
        );
        assert!(state.behind());
        assert_eq!(state.screen().text(), "a");
        assert_eq!(state.resume_after(), 1);
        // The replay after reattaching repairs it.
        state.apply(&output(2, "b"));
        state.apply(&output(3, "c"));
        assert!(!state.behind());
        assert_eq!(state.screen().text(), "abc");
    }

    #[test]
    fn a_gap_frame_is_recorded_and_advances_the_resume_point() {
        let mut state = TerminalState::new(terminal(), 100, 80);
        let gap = frame(Body::Gap {
            from: 1,
            to: 4,
            bytes: Some(900),
        });
        assert_eq!(
            state.apply(&gap),
            Applied::Gap {
                from: 1,
                to: 4,
                bytes: Some(900)
            }
        );
        state.apply(&output(5, "tail"));
        assert_eq!(state.resume_after(), 5);
        assert_eq!(state.missing().collect::<Vec<_>>(), vec![(1, 4)]);
        assert_eq!(state.missed_bytes(), (900, false));
        let lines: Vec<Line> = state.screen().lines().collect();
        assert_eq!(lines[0], Line::Gap { bytes: Some(900) });
        assert_eq!(lines[1], Line::Text("tail".into()));
    }

    #[test]
    fn exit_is_sequenced_like_output() {
        let mut state = TerminalState::new(terminal(), 100, 80);
        state.apply(&output(1, "x"));
        let exit = Exit {
            cause: Cause::Exited,
            code: Some(3),
            signal: None,
        };
        assert_eq!(
            state.apply(&frame(Body::Exit { seq: 2, exit })),
            Applied::Exit(exit)
        );
        assert_eq!(state.exit(), Some(exit));
    }

    #[test]
    fn a_frame_for_another_terminal_is_refused() {
        let mut state = TerminalState::new(terminal(), 100, 80);
        let mut other = output(1, "x");
        other.terminal.terminal = "03".repeat(32);
        assert!(matches!(state.apply(&other), Applied::Refused(_)));
    }

    #[test]
    fn the_screen_drops_escape_sequences_and_applies_carriage_returns() {
        let mut screen = Screen::new(10, 80);
        screen.feed(b"\x1b[1;32mgreen\x1b[0m\r\nabc\rX\x1b]0;title\x07\n");
        screen.feed("caf\u{e9}".as_bytes());
        assert_eq!(screen.text(), "green\nXbc\ncaf\u{e9}");
    }

    #[test]
    fn a_character_split_across_frames_is_kept_whole() {
        let mut screen = Screen::new(10, 80);
        let bytes = "\u{e9}".as_bytes();
        screen.feed(&bytes[..1]);
        screen.feed(&bytes[1..]);
        assert_eq!(screen.text(), "\u{e9}");
    }

    #[test]
    fn the_screen_is_bounded_in_lines_and_columns() {
        let mut screen = Screen::new(3, 4);
        screen.feed(b"abcdefgh\n1\n2\n3\n");
        assert_eq!(screen.text(), "2\n3\n");
        assert_eq!(screen.dropped(), 3);
        assert!(screen.lines().count() <= 3);
    }
}
