//! A multiline clipboard paste that waits for the person's Enter (#10730).
//!
//! A program that has not turned on bracketed paste (mode 2004), such as a
//! shell at its prompt without line editing, runs each pasted line as it
//! arrives. So a clipboard paste with a line break, bound for such a program
//! on the primary screen, is held: the help line shows its exact line count,
//! Enter sends the exact text once, and Escape drops it. A program that asked
//! for bracketed paste, and a full-screen program, receive the paste at once,
//! as negotiated. The input line of the fixed sheet, which joins pasted lines,
//! and a request draft hold nothing either. Text an agent sends through the
//! control socket is not a clipboard paste and is never held.

use crate::KeyIn;
use crate::application::Application;
use crate::input::KeyCode;
use crate::layout::PaneId;

/// The longest clipboard paste the terminal sends, in bytes. A longer one
/// is refused rather than cut, so no partial command ever runs.
pub const MAX_PASTE: usize = 1024 * 1024;

/// A clipboard paste waiting for Enter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub pane: PaneId,
    pub text: String,
    pub lines: usize,
}

/// The lines `text` sends: each line break ends one, whether `\n`, `\r`,
/// or `\r\n`, and text after the last break is one more.
#[must_use]
pub fn lines(text: &str) -> usize {
    let mut count = 0;
    let mut previous_cr = false;
    let mut open = false;
    for character in text.chars() {
        match character {
            '\n' if previous_cr => {}
            '\n' | '\r' => {
                count += 1;
                open = false;
            }
            _ => open = true,
        }
        previous_cr = character == '\r';
    }
    count + usize::from(open)
}

/// Whether `text` holds a line break.
#[must_use]
pub fn multiline(text: &str) -> bool {
    text.contains(['\n', '\r'])
}

impl Application {
    /// Pastes the clipboard's `text` into the focused pane, holding a
    /// multiline paste for Enter when its program has not negotiated
    /// bracketed paste and is not full screen.
    pub fn paste_clipboard(&mut self, text: &str) {
        if text.len() > MAX_PASTE {
            self.notice = Some(format!(
                "The clipboard holds more than {} KB; nothing was pasted.",
                MAX_PASTE / 1024
            ));
            return;
        }
        if let Some(held) = self.hold(text) {
            self.notice = Some(prompt(held.lines));
            self.paste_hold = Some(held);
            return;
        }
        self.paste(text);
    }

    /// The paste to hold, or `None` when `text` goes on at once.
    fn hold(&self, text: &str) -> Option<Held> {
        if !multiline(text) {
            return None;
        }
        let pane = self.focus_id()?;
        if self.paper.on && (self.paper.studio.open || !self.paper_running()) {
            // The sheet's input line joins the lines; Enter decides.
            return None;
        }
        if self
            .smart
            .draft
            .as_ref()
            .is_some_and(|draft| draft.pane == pane)
        {
            return None;
        }
        let vt = &self.panes.get(&pane)?.session.vt;
        if vt.bracketed_paste() || vt.alternate_screen() {
            return None;
        }
        Some(Held {
            pane,
            text: text.to_owned(),
            lines: lines(text),
        })
    }

    /// The held paste's prompt, while one waits for the focused pane.
    #[must_use]
    pub fn paste_prompt(&self) -> Option<String> {
        self.paste_hold
            .as_ref()
            .filter(|held| Some(held.pane) == self.focus_id())
            .map(|held| prompt(held.lines))
    }

    /// Answers a held paste: Enter sends it once, Escape drops it, and any
    /// other key does nothing while it waits. Returns whether a paste was
    /// held for the focused pane. A synthetic or repeated Enter never sends.
    pub fn paste_key(&mut self, key: &KeyIn) -> bool {
        let Some(held) = &self.paste_hold else {
            return false;
        };
        if Some(held.pane) != self.focus_id() {
            return false;
        }
        match key.code {
            KeyCode::Escape => {
                self.paste_hold = None;
                self.notice = Some("Paste cancelled; nothing was sent.".into());
            }
            KeyCode::Enter | KeyCode::NumpadEnter if !key.synthetic && !key.repeat => {
                let Some(held) = self.paste_hold.take() else {
                    return true;
                };
                self.notice = None;
                let bytes = self
                    .panes
                    .get(&held.pane)
                    .map(|pane| pane.session.vt.paste(&held.text));
                if let Some(bytes) = bytes {
                    self.send_to(held.pane, &bytes);
                }
            }
            _ => {}
        }
        true
    }
}

fn prompt(lines: usize) -> String {
    let noun = if lines == 1 { "line" } else { "lines" };
    format!("Paste {lines} {noun}? Enter sends them; Escape cancels.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_line_break_once() {
        assert_eq!(lines(""), 0);
        assert_eq!(lines("ls"), 1);
        assert_eq!(lines("ls\n"), 1);
        assert_eq!(lines("ls\npwd"), 2);
        assert_eq!(lines("ls\r\npwd\r\n"), 2);
        assert_eq!(lines("ls\rpwd"), 2);
        assert_eq!(lines("\n\n"), 2);
        assert_eq!(lines("é\nü"), 2);
        assert!(!multiline("one line"));
        assert!(multiline("one\r\n"));
    }
}
