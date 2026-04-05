use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};

const PLACEHOLDER: &str = "Type /chat [prompt]. Enter submits. Ctrl+J inserts a newline.";
const MAX_VISIBLE_COMPOSER_LINES: usize = 4;
const MAX_HISTORY_ENTRIES: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerSubmission {
    pub text: String,
    pub slash_command: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DraftSnapshot {
    text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ComposerState {
    text: String,
    cursor: usize,
    history: Vec<DraftSnapshot>,
    history_index: Option<usize>,
    stashed_snapshot: Option<DraftSnapshot>,
}

impl ComposerState {
    fn insert_char(&mut self, ch: char) {
        self.prepare_for_edit();
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    fn insert_newline(&mut self) {
        self.prepare_for_edit();
        self.text.insert(self.cursor, '\n');
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        self.prepare_for_edit();
        if let Some((start, end)) = previous_grapheme_bounds(self.text.as_str(), self.cursor) {
            self.text.replace_range(start..end, "");
            self.cursor = start;
        }
    }

    fn delete(&mut self) {
        self.prepare_for_edit();
        if let Some((start, end)) = next_grapheme_bounds(self.text.as_str(), self.cursor) {
            self.text.replace_range(start..end, "");
        }
    }

    fn move_left(&mut self) {
        if let Some((start, _)) = previous_grapheme_bounds(self.text.as_str(), self.cursor) {
            self.cursor = start;
        }
    }

    fn move_right(&mut self) {
        if let Some((_, end)) = next_grapheme_bounds(self.text.as_str(), self.cursor) {
            self.cursor = end;
        }
    }

    fn move_home(&mut self) {
        self.cursor = line_start(self.text.as_str(), self.cursor);
    }

    fn move_end(&mut self) {
        self.cursor = line_end(self.text.as_str(), self.cursor);
    }

    fn recall_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.stashed_snapshot = Some(self.snapshot());
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => {}
            Some(index) => self.history_index = Some(index.saturating_sub(1)),
        }
        if let Some(index) = self.history_index {
            self.restore(self.history[index].clone());
        }
    }

    fn recall_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            let next_index = index + 1;
            self.history_index = Some(next_index);
            self.restore(self.history[next_index].clone());
            return;
        }
        self.history_index = None;
        if let Some(snapshot) = self.stashed_snapshot.take() {
            self.restore(snapshot);
        }
    }

    fn submit(&mut self) -> Option<ComposerSubmission> {
        let text = self.text.trim().to_string();
        if text.is_empty() {
            return None;
        }

        self.history.push(DraftSnapshot { text: text.clone() });
        while self.history.len() > MAX_HISTORY_ENTRIES {
            self.history.remove(0);
        }

        let submission = ComposerSubmission {
            slash_command: slash_command(text.as_str()),
            text,
        };
        self.clear();
        Some(submission)
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.history_index = None;
        self.stashed_snapshot = None;
    }

    fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }

    fn visible_lines(&self) -> Vec<String> {
        let lines = if self.text.is_empty() {
            vec![String::new()]
        } else {
            self.text
                .lines()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        };
        let visible = lines.len().min(MAX_VISIBLE_COMPOSER_LINES);
        lines[lines.len().saturating_sub(visible)..].to_vec()
    }

    fn cursor_row_col(&self) -> (usize, usize) {
        let line_start = line_start(self.text.as_str(), self.cursor);
        let row = self.text[..line_start]
            .chars()
            .filter(|ch| *ch == '\n')
            .count();
        let col = self.text[line_start..self.cursor].chars().count();
        let visible_lines = self.visible_lines();
        let first_visible_row = self.line_count().saturating_sub(visible_lines.len());
        (row.saturating_sub(first_visible_row), col)
    }

    fn metadata_line(&self) -> String {
        match slash_command(self.text.as_str()) {
            Some(command) => format!("command: /{command}"),
            None => String::from("command: text"),
        }
    }

    fn snapshot(&self) -> DraftSnapshot {
        DraftSnapshot {
            text: self.text.clone(),
        }
    }

    fn restore(&mut self, snapshot: DraftSnapshot) {
        self.text = snapshot.text;
        self.cursor = self.text.len();
    }

    fn prepare_for_edit(&mut self) {
        if self.history_index.is_some() {
            self.history_index = None;
            self.stashed_snapshot = None;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BottomPane {
    composer: ComposerState,
}

impl BottomPane {
    pub fn height(&self) -> u16 {
        2 + self.composer.line_count().min(MAX_VISIBLE_COMPOSER_LINES) as u16
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ComposerSubmission> {
        match key.code {
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.composer.insert_newline();
                None
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.composer.insert_char(ch);
                None
            }
            KeyCode::Backspace => {
                self.composer.backspace();
                None
            }
            KeyCode::Delete => {
                self.composer.delete();
                None
            }
            KeyCode::Left => {
                self.composer.move_left();
                None
            }
            KeyCode::Right => {
                self.composer.move_right();
                None
            }
            KeyCode::Home => {
                self.composer.move_home();
                None
            }
            KeyCode::End => {
                self.composer.move_end();
                None
            }
            KeyCode::Up => {
                self.composer.recall_previous();
                None
            }
            KeyCode::Down => {
                self.composer.recall_next();
                None
            }
            KeyCode::Enter => self.composer.submit(),
            _ => None,
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        border: Style,
        _accent: Style,
        helper_text: Option<&str>,
    ) {
        let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).split(area);
        let composer_block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .title("─ Composer ")
            .style(border);
        let composer_inner = composer_block.inner(rows[1]);
        frame.render_widget(composer_block, rows[1]);

        let helper_copy = helper_text.unwrap_or(PLACEHOLDER);
        frame.render_widget(
            Paragraph::new(Line::from(format!(
                "{}  {}",
                self.composer.metadata_line(),
                helper_copy
            )))
            .style(Style::default().fg(Color::Rgb(0x8b, 0xc7, 0xff))),
            rows[0],
        );

        let body = if self.composer.text.is_empty() {
            Text::from(vec![Line::styled(
                helper_text.unwrap_or(PLACEHOLDER),
                Style::default()
                    .fg(Color::Rgb(0x5c, 0x7e, 0x9b))
                    .add_modifier(Modifier::ITALIC),
            )])
        } else {
            Text::from(
                self.composer
                    .visible_lines()
                    .into_iter()
                    .map(Line::from)
                    .collect::<Vec<_>>(),
            )
        };
        frame.render_widget(
            Paragraph::new(body).wrap(Wrap { trim: false }),
            composer_inner,
        );

        let (row, col) = self.composer.cursor_row_col();
        frame.set_cursor_position((
            composer_inner.x.saturating_add(col as u16),
            composer_inner.y.saturating_add(row as u16),
        ));
    }
}

pub fn slash_command(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let command = trimmed.strip_prefix('/')?;
    let command = command.split_whitespace().next()?;
    (!command.is_empty()).then(|| command.to_string())
}

fn previous_grapheme_bounds(text: &str, cursor: usize) -> Option<(usize, usize)> {
    text[..cursor]
        .char_indices()
        .next_back()
        .map(|(start, ch)| (start, start + ch.len_utf8()))
}

fn next_grapheme_bounds(text: &str, cursor: usize) -> Option<(usize, usize)> {
    text[cursor..]
        .char_indices()
        .next()
        .map(|(offset, ch)| (cursor + offset, cursor + offset + ch.len_utf8()))
}

fn line_start(text: &str, cursor: usize) -> usize {
    text[..cursor].rfind('\n').map_or(0, |index| index + 1)
}

fn line_end(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .find('\n')
        .map_or(text.len(), |offset| cursor + offset)
}

#[cfg(test)]
mod tests {
    use super::{BottomPane, slash_command};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn slash_command_extracts_command_name() {
        assert_eq!(slash_command("/chat hello").as_deref(), Some("chat"));
        assert_eq!(slash_command("hello"), None);
    }

    #[test]
    fn composer_submits_text_and_tracks_command() {
        let mut pane = BottomPane::default();
        pane.handle_key(key(KeyCode::Char('/')));
        pane.handle_key(key(KeyCode::Char('c')));
        pane.handle_key(key(KeyCode::Char('h')));
        pane.handle_key(key(KeyCode::Char('a')));
        pane.handle_key(key(KeyCode::Char('t')));
        pane.handle_key(key(KeyCode::Char(' ')));
        pane.handle_key(key(KeyCode::Char('h')));
        pane.handle_key(key(KeyCode::Char('i')));

        let submission = pane
            .handle_key(key(KeyCode::Enter))
            .expect("expected submission");
        assert_eq!(submission.text, "/chat hi");
        assert_eq!(submission.slash_command.as_deref(), Some("chat"));
    }
}
