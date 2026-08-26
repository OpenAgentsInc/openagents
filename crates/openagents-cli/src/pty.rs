//! Running a command under a pseudoterminal, inside the coder frame.
//!
//! Adapted from `ptyctl` in grok-build, which is Apache-2.0 and is what
//! `crates/openagents-cli/src/composer/LICENSE-APACHE-xai` covers. What is
//! taken is the shape: `portable-pty` for the terminal pair, the master
//! dismantled into a reader thread, a writer, and a resize handle, and the
//! child's killer cloned off so it can be stopped without waiting on it. What
//! is not taken is grok's websocket
//! server, its session registry, or its rendering — output here lands in the
//! OpenAgents box frame.
//!
//! ## Why a pseudoterminal and not a pipe
//!
//! A pipe is not a terminal. A program asked whether its output is a terminal
//! answers no, and the ones worth watching change what they do: `git` drops
//! its colour, `top` and `vim` refuse to draw at all, and anything that reads
//! its width from the kernel gets nothing to read. A pseudoterminal answers
//! yes, carries a window size the child can ask for, and delivers `SIGWINCH`
//! when that size changes. [`PtySession::resize`] is what makes the last of
//! those true, and it is wired to the frame's own size.
//!
//! ## The pieces
//!
//! - [`PtySession`] owns the child and the terminal pair. It is I/O, so it is
//!   built only where there is a real terminal.
//! - [`PtyScreen`] is a terminal emulator over the bytes that come back: it
//!   holds the grid the child has drawn and renders it into the frame. It is
//!   pure — bytes in, cells out — so the tests below drive it directly.
//! - [`encode_key`] turns a key the frame received into the bytes a terminal
//!   would have sent for it.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

/// The key that takes the keyboard back from a running child.
///
/// Everything else goes to the child, including Esc and Ctrl+C, because a
/// full-screen program needs both. `Ctrl+]` is the telnet escape and is not
/// bound by the shells or editors this is likely to be running.
pub const DETACH: KeyEvent = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL);
pub const DETACH_LABEL: &str = "Ctrl+]";

/// What a running child sends back.
#[derive(Debug)]
pub enum PtyEvent {
    /// Bytes the child wrote. Not necessarily whole lines or whole sequences.
    Output(Vec<u8>),
    /// The child ended, with its exit code.
    Exit(u32),
}

/// What the frame can do to a running child.
///
/// A trait rather than a struct so a test can drive the pane with a recording
/// stand-in and assert that a resize actually reached the child's side.
pub trait PtyControl: Send + Sync + std::fmt::Debug {
    /// Send bytes to the child's input.
    fn write(&self, bytes: &[u8]);
    /// Tell the child its terminal is now this size.
    fn resize(&self, cols: u16, rows: u16);
    /// End the child.
    fn kill(&self);
}

/// A command running under a pseudoterminal.
pub struct PtySession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl PtySession {
    /// Start `command` under a pseudoterminal of `cols` × `rows`.
    ///
    /// Returns the handle the frame keeps and the stream of what the child
    /// writes. A thread is started rather than a task because reading a
    /// terminal blocks on a file descriptor that cannot be polled.
    pub fn spawn(
        command: &[String],
        cwd: Option<PathBuf>,
        cols: u16,
        rows: u16,
    ) -> std::io::Result<(Arc<PtySession>, UnboundedReceiver<PtyEvent>)> {
        let Some(program) = command.first() else {
            return Err(oops("no command to run"));
        };

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| oops(&format!("could not open a pseudoterminal: {error}")))?;

        let mut builder = CommandBuilder::new(program);
        builder.args(&command[1..]);
        if let Some(cwd) = cwd {
            builder.cwd(cwd);
        }
        // The child is told it is on a terminal that can do colour. It has
        // one: `PtyScreen` renders every attribute this claims.
        builder.env("TERM", "xterm-256color");
        // The reader's `--no-color` reaches the child the same way it reaches
        // a delegated harness.
        if !crate::diag::color() {
            builder.env("NO_COLOR", "1");
        }

        // Whether the command exists is worth answering before spawning it.
        // The failure the reader will hit most often is a typo, and the error
        // the operating system hands back through `portable-pty` arrives
        // wrapped in a debug print of the whole command — the entire inherited
        // environment included, thousands of characters of `PATH` between the
        // reader and the two words that matter.
        if find_program(program).is_none() {
            return Err(oops(&format!(
                "could not start `{program}`: there is no such command on this machine"
            )));
        }

        let mut child = pair
            .slave
            .spawn_command(builder)
            .map_err(|error| oops(&format!("could not start `{program}`: {}", brief(&error))))?;
        let killer = child.clone_killer();

        let mut reader = pair.master.try_clone_reader().map_err(|error| {
            oops(&format!(
                "could not read the pseudoterminal: {}",
                brief(&error)
            ))
        })?;
        let writer = pair.master.take_writer().map_err(|error| {
            oops(&format!(
                "could not write the pseudoterminal: {}",
                brief(&error)
            ))
        })?;

        let (tx, rx) = unbounded_channel();

        // One thread reads and then waits, rather than two racing to report.
        // The last thing a program writes is often the whole answer, and a
        // separate waiter can win the race to the channel and close the pane
        // over the top of output that has not been delivered yet.
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(PtyEvent::Output(buffer[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                }
            }
            let code = child.wait().map(|status| status.exit_code()).unwrap_or(1);
            let _ = tx.send(PtyEvent::Exit(code));
        });

        Ok((
            Arc::new(PtySession {
                master: Mutex::new(pair.master),
                writer: Mutex::new(writer),
                killer: Mutex::new(killer),
            }),
            rx,
        ))
    }
}

impl std::fmt::Debug for PtySession {
    /// The handles inside are a terminal pair and a process; none of them has
    /// a useful debug form, and the identity of the session is all a caller
    /// printing a message about one needs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PtySession")
    }
}

impl PtyControl for PtySession {
    fn write(&self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    fn resize(&self, cols: u16, rows: u16) {
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    fn kill(&self) {
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
    }
}

fn oops(message: &str) -> std::io::Error {
    std::io::Error::other(message.to_string())
}

/// As much of an error as belongs on one line of a transcript.
fn brief(error: impl std::fmt::Display) -> String {
    let text = error.to_string();
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= 160 {
        return line.to_string();
    }
    format!("{}…", line.chars().take(160).collect::<String>())
}

/// Where `program` would be found, or `None` if it would not be.
///
/// A name with a separator in it is a path and is taken as one; anything else
/// is looked for on `PATH`, which is the same rule the spawn itself follows.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let runnable = |path: &std::path::Path| path.is_file();
    if program.contains('/') {
        let path = PathBuf::from(program);
        return runnable(&path).then_some(path);
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|directory| directory.join(program))
        .find(|candidate| runnable(candidate))
}

/// Split a command line into words, honouring quotes.
///
/// `/run git log --oneline -n 5` is words. `/run echo "one two"` is three.
/// Anything past that — pipes, redirection, globbing — belongs to a shell, and
/// [`shell_command`] is how a caller asks for one.
pub fn split_command(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;

    for ch in line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => word.push(ch),
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                any = true;
            }
            None if ch.is_whitespace() => {
                if !word.is_empty() || any {
                    words.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            None => word.push(ch),
        }
    }
    if !word.is_empty() || any {
        words.push(word);
    }
    words
}

/// Whether `line` needs a shell to mean what it says.
pub fn needs_a_shell(line: &str) -> bool {
    line.contains(['|', '>', '<', '&', ';', '*', '$', '`', '('])
}

/// The command that runs `line` under this machine's shell.
pub fn shell_command(line: &str) -> Vec<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    vec![shell, "-c".to_string(), line.to_string()]
}

// ------------------------------------------------------------------ screen

/// The grid a child has drawn, and the renderer for it.
pub struct PtyScreen {
    parser: vt100::Parser,
    cols: u16,
    rows: u16,
}

/// The smallest emulated screen this will build.
///
/// `vt100` subtracts a character's width from the screen's width when it
/// decides whether to wrap, and subtracts one from the row count when it
/// scrolls; on a screen one row or one column across, both underflow and the
/// process aborts. A terminal emulator crashing because its pane got small is
/// not a trade worth making, so nothing narrower than this is ever built and
/// the frame clips instead.
///
/// This is not hypothetical: a frame whose terminal reports no size at all —
/// which is what a pseudoterminal opened without a window size does, and what
/// `expect` hands a program by default — lands exactly here.
pub const MIN_SCREEN: u16 = 2;

impl PtyScreen {
    pub fn new(cols: u16, rows: u16) -> Self {
        let cols = cols.max(MIN_SCREEN);
        let rows = rows.max(MIN_SCREEN);
        Self {
            // No scrollback: the pane shows what the child currently has on
            // its screen, which is what a terminal of this size would show.
            parser: vt100::Parser::new(rows, cols, 0),
            cols,
            rows,
        }
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    /// Resize the emulated screen. Returns whether the size actually changed,
    /// so the caller only pays for the `SIGWINCH` when there is one to send.
    pub fn resize(&mut self, cols: u16, rows: u16) -> bool {
        let cols = cols.max(MIN_SCREEN);
        let rows = rows.max(MIN_SCREEN);
        if (cols, rows) == (self.cols, self.rows) {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        self.parser.set_size(rows, cols);
        true
    }

    /// The title the child set with an `OSC 0` sequence, if it set one.
    pub fn title(&self) -> Option<&str> {
        let title = self.parser.screen().title();
        (!title.is_empty()).then_some(title)
    }

    /// Whether the child is asking for the arrow keys in application mode.
    pub fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    /// Where the child's own cursor sits, as (column, row) within the pane.
    pub fn cursor(&self) -> Option<(u16, u16)> {
        let screen = self.parser.screen();
        if screen.hide_cursor() {
            return None;
        }
        let (row, col) = screen.cursor_position();
        Some((col, row))
    }

    /// The screen as text, one row per line, trailing blanks trimmed.
    ///
    /// What a test asserts against, and what a reader would see if the frame
    /// carried no colour.
    pub fn text(&self) -> String {
        let screen = self.parser.screen();
        (0..self.rows)
            .map(|row| {
                let line: String = (0..self.cols)
                    .map(|col| {
                        screen
                            .cell(row, col)
                            .map(vt100::Cell::contents)
                            .filter(|c| !c.is_empty())
                            .unwrap_or_else(|| " ".to_string())
                    })
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Paint the child's grid into `area`.
    pub fn render(&self, area: Rect, buffer: &mut Buffer) {
        let screen = self.parser.screen();
        for row in 0..area.height.min(self.rows) {
            for col in 0..area.width.min(self.cols) {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                // A wide character's second half is drawn by the first; a cell
                // written over it would cut the glyph in two.
                if cell.is_wide_continuation() {
                    continue;
                }
                let Some(target) = buffer.cell_mut(Position::new(area.x + col, area.y + row))
                else {
                    continue;
                };
                let contents = cell.contents();
                target.set_symbol(if contents.is_empty() { " " } else { &contents });
                let mut style = Style::default()
                    .fg(convert(cell.fgcolor()))
                    .bg(convert(cell.bgcolor()));
                if cell.bold() {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if cell.italic() {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if cell.underline() {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                if cell.inverse() {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                target.set_style(style);
            }
        }
    }
}

/// A colour the child asked for, as a colour ratatui can draw.
fn convert(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(0) => Color::Black,
        vt100::Color::Idx(1) => Color::Red,
        vt100::Color::Idx(2) => Color::Green,
        vt100::Color::Idx(3) => Color::Yellow,
        vt100::Color::Idx(4) => Color::Blue,
        vt100::Color::Idx(5) => Color::Magenta,
        vt100::Color::Idx(6) => Color::Cyan,
        vt100::Color::Idx(7) => Color::Gray,
        vt100::Color::Idx(8) => Color::DarkGray,
        vt100::Color::Idx(9) => Color::LightRed,
        vt100::Color::Idx(10) => Color::LightGreen,
        vt100::Color::Idx(11) => Color::LightYellow,
        vt100::Color::Idx(12) => Color::LightBlue,
        vt100::Color::Idx(13) => Color::LightMagenta,
        vt100::Color::Idx(14) => Color::LightCyan,
        vt100::Color::Idx(15) => Color::White,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

// --------------------------------------------------------------------- keys

/// The bytes a terminal would send for `key`, or `None` for a key it has no
/// encoding for.
///
/// `application` is the child's own cursor-key mode, which a full-screen
/// program sets and which changes what the arrow keys are: `ESC O A` rather
/// than `ESC [ A`. Sending the wrong one is how arrow keys stop working inside
/// an editor while working fine at a shell prompt.
pub fn encode_key(key: &KeyEvent, application: bool) -> Option<Vec<u8>> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    let arrow = |final_byte: u8| {
        let lead = if application { b'O' } else { b'[' };
        Some(vec![0x1b, lead, final_byte])
    };

    let body = match key.code {
        KeyCode::Char(c) if control => {
            let byte = match c.to_ascii_lowercase() {
                c @ 'a'..='z' => c as u8 - b'a' + 1,
                ' ' | '@' => 0,
                '[' => 27,
                '\\' => 28,
                ']' => 29,
                '^' => 30,
                '_' | '?' => 31,
                _ => return None,
            };
            vec![byte]
        }
        KeyCode::Char(c) => {
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
            bytes
        }
        // A terminal sends carriage return for Enter; the line discipline is
        // what turns it into a newline. Sending `\n` skips that and looks to
        // the child like a literal line feed.
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => return Some(vec![0x1b, b'[', b'Z']),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => return prefix(arrow(b'A'), alt),
        KeyCode::Down => return prefix(arrow(b'B'), alt),
        KeyCode::Right => return prefix(arrow(b'C'), alt),
        KeyCode::Left => return prefix(arrow(b'D'), alt),
        KeyCode::Home => return prefix(arrow(b'H'), alt),
        KeyCode::End => return prefix(arrow(b'F'), alt),
        KeyCode::Insert => return Some(b"\x1b[2~".to_vec()),
        KeyCode::Delete => return Some(b"\x1b[3~".to_vec()),
        KeyCode::PageUp => return Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => return Some(b"\x1b[6~".to_vec()),
        KeyCode::F(n @ 1..=4) => return Some(vec![0x1b, b'O', b'P' + (n - 1)]),
        KeyCode::F(n @ 5..=12) => {
            // The gaps in this table are the ones DEC left; they are not a
            // mistake being copied.
            let code = match n {
                5 => 15,
                6..=10 => 17 + (n - 6),
                11 => 23,
                _ => 24,
            };
            return Some(format!("\x1b[{code}~").into_bytes());
        }
        _ => return None,
    };
    prefix(Some(body), alt)
}

/// Alt is sent as an escape before the key, which is what a terminal does.
fn prefix(bytes: Option<Vec<u8>>, alt: bool) -> Option<Vec<u8>> {
    let bytes = bytes?;
    if !alt {
        return Some(bytes);
    }
    let mut out = vec![0x1b];
    out.extend(bytes);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen_of(bytes: &[u8], cols: u16, rows: u16) -> PtyScreen {
        let mut screen = PtyScreen::new(cols, rows);
        screen.feed(bytes);
        screen
    }

    #[test]
    fn plain_output_lands_on_the_grid() {
        let screen = screen_of(b"hello\r\nworld", 20, 4);
        assert_eq!(screen.text(), "hello\nworld\n\n");
    }

    #[test]
    fn a_cursor_move_puts_text_where_the_child_asked_for_it() {
        // `ESC [ 3 ; 5 H` is row three, column five.
        let screen = screen_of(b"\x1b[3;5Hhere", 20, 3);
        assert_eq!(screen.text(), "\n\n    here");
    }

    #[test]
    fn a_clear_takes_the_screen_back() {
        let screen = screen_of(b"old text\x1b[2J\x1b[Hnew", 20, 3);
        assert_eq!(screen.text(), "new\n\n");
    }

    #[test]
    fn colour_the_child_asked_for_is_the_colour_that_is_drawn() {
        let screen = screen_of(b"\x1b[31mred\x1b[0m plain", 20, 2);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 2));
        screen.render(Rect::new(0, 0, 20, 2), &mut buffer);
        assert_eq!(buffer[(0, 0)].fg, Color::Red);
        assert_eq!(buffer[(0, 0)].symbol(), "r");
        assert_eq!(buffer[(4, 0)].fg, Color::Reset);
    }

    #[test]
    fn bold_and_reverse_survive_the_trip_to_the_frame() {
        let screen = screen_of(b"\x1b[1mB\x1b[0m\x1b[7mR", 8, 1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        screen.render(Rect::new(0, 0, 8, 1), &mut buffer);
        assert!(buffer[(0, 0)].modifier.contains(Modifier::BOLD));
        assert!(buffer[(1, 0)].modifier.contains(Modifier::REVERSED));
    }

    /// A pane too small to emulate does not take the process down with it.
    ///
    /// `vt100` underflows on a screen one row or one column across, and a
    /// terminal that reports no size at all — a pseudoterminal opened without
    /// a window size, which is what `expect` gives a program — asks for
    /// exactly that. This is the regression test for a real crash: the
    /// session aborted inside its own alternate screen the first time it was
    /// driven under one.
    #[test]
    fn a_pane_too_small_to_emulate_does_not_bring_the_session_down() {
        for (cols, rows) in [(0u16, 0u16), (1, 1), (1, 40), (40, 1), (2, 1)] {
            let mut screen = PtyScreen::new(cols, rows);
            screen.feed("a long line of output that must wrap, plus 漢字\r\n\thi\r\n".as_bytes());
            let (c, r) = screen.size();
            assert!(c >= 2 && r >= 2, "a {c}×{r} screen was built");
            let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
            screen.render(Rect::new(0, 0, 4, 2), &mut buffer);
        }

        // And the same on the way down from a workable size.
        let mut screen = PtyScreen::new(80, 24);
        screen.feed(b"hello");
        screen.resize(1, 1);
        screen.feed(b"still here without aborting");
        assert_eq!(screen.size(), (2, 2));
    }

    #[test]
    fn a_resize_is_reported_only_when_the_size_actually_changed() {
        let mut screen = PtyScreen::new(20, 5);
        assert!(!screen.resize(20, 5));
        assert!(screen.resize(30, 5));
        assert_eq!(screen.size(), (30, 5));
    }

    #[test]
    fn the_pane_is_painted_at_the_offset_it_was_given() {
        let screen = screen_of(b"ab", 4, 1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        screen.render(Rect::new(3, 1, 4, 1), &mut buffer);
        assert_eq!(buffer[(3, 1)].symbol(), "a");
        assert_eq!(buffer[(4, 1)].symbol(), "b");
        // Outside the pane nothing was touched.
        assert_eq!(buffer[(0, 0)].symbol(), " ");
    }

    #[test]
    fn a_title_the_child_set_is_reported_and_an_unset_one_is_not() {
        assert_eq!(screen_of(b"nothing", 10, 1).title(), None);
        assert_eq!(
            screen_of(b"\x1b]0;a title\x07", 10, 1).title(),
            Some("a title")
        );
    }

    // ----------------------------------------------------------------- keys

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn enter_is_a_carriage_return_not_a_line_feed() {
        assert_eq!(
            encode_key(&press(KeyCode::Enter), false),
            Some(b"\r".to_vec())
        );
    }

    #[test]
    fn control_letters_become_their_control_bytes() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encode_key(&ctrl_c, false), Some(vec![3]));
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(encode_key(&ctrl_d, false), Some(vec![4]));
    }

    #[test]
    fn the_arrow_keys_follow_the_childs_own_cursor_mode() {
        assert_eq!(
            encode_key(&press(KeyCode::Up), false),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            encode_key(&press(KeyCode::Up), true),
            Some(b"\x1bOA".to_vec())
        );
    }

    #[test]
    fn alt_is_sent_as_an_escape_before_the_key() {
        let alt_b = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(encode_key(&alt_b, false), Some(vec![0x1b, b'b']));
    }

    #[test]
    fn backspace_is_delete_which_is_what_terminals_send() {
        assert_eq!(
            encode_key(&press(KeyCode::Backspace), false),
            Some(vec![0x7f])
        );
    }

    #[test]
    fn a_key_with_no_terminal_encoding_sends_nothing() {
        assert_eq!(encode_key(&press(KeyCode::CapsLock), false), None);
    }

    #[test]
    fn a_multibyte_character_is_sent_as_its_utf8() {
        assert_eq!(
            encode_key(&press(KeyCode::Char('é')), false),
            Some("é".as_bytes().to_vec())
        );
    }

    // ------------------------------------------------------------- commands

    #[test]
    fn a_command_line_splits_into_words_and_quotes_hold_together() {
        assert_eq!(
            split_command("git log --oneline -n 5"),
            vec!["git", "log", "--oneline", "-n", "5"]
        );
        assert_eq!(split_command("echo \"one two\""), vec!["echo", "one two"]);
        assert_eq!(split_command("  "), Vec::<String>::new());
        // An empty quoted word is still a word.
        assert_eq!(split_command("echo ''"), vec!["echo", ""]);
    }

    #[test]
    fn a_program_that_is_not_installed_is_reported_before_it_is_spawned() {
        assert!(find_program("sh").is_some(), "`sh` is on every PATH");
        assert!(find_program("this-command-does-not-exist-anywhere").is_none());
        // A name with a separator is a path, and is not looked for on PATH.
        assert!(find_program("./this-is-not-here").is_none());
    }

    /// The operating system's message about a failed spawn arrives wrapped in
    /// a debug print of the whole command, environment included. One line, and
    /// a bounded one, is what belongs on a transcript.
    #[test]
    fn an_error_is_cut_to_one_bounded_line() {
        assert_eq!(brief("short and single"), "short and single");
        assert_eq!(brief("first line\nsecond line"), "first line");
        let long = "x".repeat(400);
        let cut = brief(&long);
        assert_eq!(cut.chars().count(), 161);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn a_line_a_shell_would_change_the_meaning_of_is_given_to_a_shell() {
        assert!(needs_a_shell("ls | wc -l"));
        assert!(needs_a_shell("echo $HOME"));
        assert!(!needs_a_shell("git status"));
        assert_eq!(shell_command("ls | wc -l").len(), 3);
        assert_eq!(shell_command("ls").last().map(String::as_str), Some("ls"));
    }
}
