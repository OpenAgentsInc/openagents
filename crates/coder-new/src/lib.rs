//! A local screen preview, with no agent, registry, or wallet connection.

pub mod agents;
pub mod snapshot;
pub mod theme;
pub mod ui;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    #[default]
    Conversation,
}

#[derive(Default)]
pub struct App {
    pub screen: Screen,
    pub draft: Draft,
    pub messages: Vec<String>,
    pub scroll: u16,
    pub agents: agents::Agents,
}

impl App {
    /// Returns false when the preview should close.
    pub fn handle(&mut self, event: Event) -> bool {
        match event {
            Event::Paste(text)
                if !matches!(
                    self.agents.view,
                    agents::AgentView::List | agents::AgentView::Detail
                ) =>
            {
                self.agents.view = agents::AgentView::Composer;
                self.draft.insert(&text);
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl && key.code == KeyCode::Char('c') {
                    return false;
                }
                if self.agents.handle(key) {
                    return true;
                }
                match key.code {
                    KeyCode::Tab | KeyCode::BackTab => {
                        self.screen = match self.screen {
                            Screen::Welcome => Screen::Conversation,
                            Screen::Conversation => Screen::Welcome,
                        };
                        self.scroll = 0;
                    }
                    KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
                    KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        self.draft.insert("\n");
                    }
                    KeyCode::Enter if !ctrl => {
                        if !self.draft.text.trim().is_empty() {
                            self.messages.push(std::mem::take(&mut self.draft.text));
                            self.draft.cursor = 0;
                            self.screen = Screen::Conversation;
                            self.scroll = u16::MAX;
                        }
                    }
                    KeyCode::Char(ch)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        self.draft.insert(&ch.to_string());
                    }
                    KeyCode::Backspace => self.draft.backspace(),
                    KeyCode::Delete => self.draft.delete(),
                    KeyCode::Left => self.draft.cursor = self.draft.previous(),
                    KeyCode::Right => self.draft.cursor = self.draft.next(),
                    KeyCode::Home => self.draft.cursor = 0,
                    KeyCode::End => self.draft.cursor = self.draft.text.len(),
                    _ => {}
                }
            }
            _ => {}
        }
        true
    }
}

#[derive(Default)]
pub struct Draft {
    pub text: String,
    pub cursor: usize,
}

impl Draft {
    fn insert(&mut self, text: &str) {
        let clean: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\t', "    ")
            .chars()
            .filter(|ch| *ch == '\n' || !ch.is_control())
            .collect();
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
        self.snap_cursor();
    }

    fn snap_cursor(&mut self) {
        // Edits can join surrounding graphemes; keep the cursor at a boundary.
        while self.cursor < self.text.len()
            && !self
                .text
                .grapheme_indices(true)
                .any(|(offset, _)| offset == self.cursor)
        {
            self.cursor += self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        }
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(offset, _)| offset)
    }

    fn next(&self) -> usize {
        self.cursor
            + self.text[self.cursor..]
                .graphemes(true)
                .next()
                .map_or(0, str::len)
    }

    fn backspace(&mut self) {
        let previous = self.previous();
        self.text.drain(previous..self.cursor);
        self.cursor = previous;
        self.snap_cursor();
    }

    fn delete(&mut self) {
        self.text.drain(self.cursor..self.next());
        self.snap_cursor();
    }

    /// Hard-wraps by terminal cells so the rendered cursor matches the draft.
    pub fn wrapped(&self, width: u16) -> (Vec<String>, (u16, u16)) {
        let width = usize::from(width.max(1));
        let mut lines = vec![String::new()];
        let mut column = 0;
        let mut cursor = (0, 0);
        for (offset, grapheme) in self.text.grapheme_indices(true) {
            let cells = grapheme.width();
            if grapheme != "\n" && column + cells > width && column > 0 {
                lines.push(String::new());
                column = 0;
            }
            if offset == self.cursor {
                cursor = (column as u16, (lines.len() - 1) as u16);
            }
            if grapheme == "\n" {
                lines.push(String::new());
                column = 0;
            } else {
                lines
                    .last_mut()
                    .expect("the draft always has a line")
                    .push_str(grapheme);
                column += cells;
            }
        }
        if self.cursor == self.text.len() {
            if column >= width {
                lines.push(String::new());
                column = 0;
            }
            cursor = (column as u16, (lines.len() - 1) as u16);
        }
        (lines, cursor)
    }
}
