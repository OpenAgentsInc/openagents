use crate::slash_commands::{self, SlashCommandSpec};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};

const PLACEHOLDER: &str = "Ask Gemma or type /help";
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
struct SlashPaletteState {
    open: bool,
    query: String,
    selected_index: usize,
    dismissed_query: Option<String>,
    matches: Vec<&'static SlashCommandSpec>,
    replace_start: usize,
    replace_end: usize,
}

impl SlashPaletteState {
    fn sync_with_composer(&mut self, composer: &ComposerState) {
        let Some(context) = slash_commands::active_slash_query(composer.text.as_str(), composer.cursor)
        else {
            self.open = false;
            self.query.clear();
            self.selected_index = 0;
            self.dismissed_query = None;
            self.matches.clear();
            self.replace_start = 0;
            self.replace_end = 0;
            return;
        };

        let query_changed = self.query != context.query;
        self.query = context.query;
        self.replace_start = context.replace_start;
        self.replace_end = context.replace_end;
        self.matches = slash_commands::suggestions_for_query(self.query.as_str());
        if query_changed {
            self.selected_index = 0;
        }
        if self.selected_index >= self.matches.len() {
            self.selected_index = self.matches.len().saturating_sub(1);
        }
        self.open = self.dismissed_query.as_deref() != Some(self.query.as_str());
    }

    fn dismiss(&mut self) {
        if self.open {
            self.open = false;
            self.dismissed_query = Some(self.query.clone());
        }
    }

    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.selected_index + 1 < self.matches.len() {
            self.selected_index += 1;
        }
    }

    fn selected(&self) -> Option<&'static SlashCommandSpec> {
        self.matches.get(self.selected_index).copied()
    }

    fn visible_rows(&self) -> usize {
        if !self.open {
            return 0;
        }
        self.matches.len().clamp(1, 6)
    }
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
            None => String::new(),
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
    slash_palette: SlashPaletteState,
}

impl BottomPane {
    pub fn height(&self) -> u16 {
        3 + self.composer.visible_line_count() as u16 + self.palette_height()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ComposerSubmission> {
        if self.slash_palette.open {
            match key.code {
                KeyCode::Esc => {
                    self.slash_palette.dismiss();
                    return None;
                }
                KeyCode::Tab => {
                    self.accept_slash_selection();
                    return None;
                }
                KeyCode::Up => {
                    self.slash_palette.move_up();
                    return None;
                }
                KeyCode::Down => {
                    self.slash_palette.move_down();
                    return None;
                }
                KeyCode::Enter => {
                    self.accept_slash_selection();
                    return None;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.composer.insert_newline();
                self.sync_palette();
                None
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.composer.insert_char(ch);
                self.sync_palette();
                None
            }
            KeyCode::Backspace => {
                self.composer.backspace();
                self.sync_palette();
                None
            }
            KeyCode::Delete => {
                self.composer.delete();
                self.sync_palette();
                None
            }
            KeyCode::Left => {
                self.composer.move_left();
                self.sync_palette();
                None
            }
            KeyCode::Right => {
                self.composer.move_right();
                self.sync_palette();
                None
            }
            KeyCode::Home => {
                self.composer.move_home();
                self.sync_palette();
                None
            }
            KeyCode::End => {
                self.composer.move_end();
                self.sync_palette();
                None
            }
            KeyCode::Up => {
                self.composer.recall_previous();
                self.sync_palette();
                None
            }
            KeyCode::Down => {
                self.composer.recall_next();
                self.sync_palette();
                None
            }
            KeyCode::Esc => {
                self.slash_palette.dismiss();
                None
            }
            KeyCode::Enter => {
                let submission = self.composer.submit();
                self.sync_palette();
                submission
            }
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
        let rows = if self.slash_palette.open {
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(self.palette_height()),
                Constraint::Min(2),
            ])
            .split(area)
        } else {
            Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).split(area)
        };
        let composer_block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .title("─ Composer ")
            .style(border);
        let composer_row = rows.len().saturating_sub(1);
        let composer_inner = composer_block.inner(rows[composer_row]);
        frame.render_widget(composer_block, rows[composer_row]);

        let helper_copy = if self.slash_palette.open {
            "Use ↑↓ to choose, Enter to insert, Esc to close"
        } else {
            helper_text.unwrap_or(PLACEHOLDER)
        };
        let metadata = if self.slash_palette.open {
            format!("slash: /{}", self.slash_palette.query)
        } else {
            self.composer.metadata_line()
        };
        let helper_line = if metadata.is_empty() {
            helper_copy.to_string()
        } else {
            format!("{metadata}  {helper_copy}")
        };
        frame.render_widget(
            Paragraph::new(Line::from(helper_line)).style(Style::default().fg(Color::Rgb(0x8b, 0xc7, 0xff))),
            rows[0],
        );

        if self.slash_palette.open {
            frame.render_widget(self.palette_panel(border), rows[1]);
        }

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

    fn palette_panel(&self, border: Style) -> Paragraph<'static> {
        let rows = if self.slash_palette.matches.is_empty() {
            vec![Line::styled(
                "No matching commands. Keep typing or press Esc.",
                Style::default().fg(Color::Rgb(0xff, 0xcd, 0x6b)),
            )]
        } else {
            let visible = self.slash_palette.visible_rows();
            let start = self
                .slash_palette
                .selected_index
                .saturating_sub(visible.saturating_sub(1));
            self.slash_palette
                .matches
                .iter()
                .skip(start)
                .take(visible)
                .enumerate()
                .map(|(index, spec)| {
                    let absolute_index = start + index;
                    let prefix = if absolute_index == self.slash_palette.selected_index {
                        Span::styled("> ", Style::default().fg(Color::Rgb(0xf8, 0xf4, 0xe3)).add_modifier(Modifier::BOLD))
                    } else {
                        Span::raw("  ")
                    };
                    let usage = if absolute_index == self.slash_palette.selected_index {
                        Span::styled(
                            format!("{:<24}", spec.usage),
                            Style::default()
                                .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(
                            format!("{:<24}", spec.usage),
                            Style::default().fg(Color::Rgb(0x8b, 0xc7, 0xff)),
                        )
                    };
                    let summary = Span::raw(spec.summary);
                    Line::from(vec![prefix, usage, summary])
                })
                .collect()
        };

        Paragraph::new(Text::from(rows))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .title("─ Slash Commands ")
                    .style(border),
            )
            .wrap(Wrap { trim: false })
    }
}

impl ComposerState {
    fn visible_line_count(&self) -> usize {
        self.line_count().min(MAX_VISIBLE_COMPOSER_LINES)
    }
}

impl BottomPane {
    fn sync_palette(&mut self) {
        self.slash_palette.sync_with_composer(&self.composer);
    }

    fn palette_height(&self) -> u16 {
        if !self.slash_palette.open {
            return 0;
        }
        self.slash_palette.visible_rows() as u16 + 2
    }

    fn accept_slash_selection(&mut self) {
        let Some(spec) = self.slash_palette.selected() else {
            return;
        };
        let insertion = slash_commands::insertion_text(spec);
        self.composer.prepare_for_edit();
        self.composer
            .text
            .replace_range(self.slash_palette.replace_start..self.slash_palette.replace_end, insertion.as_str());
        self.composer.cursor = self.slash_palette.replace_start + insertion.len();
        self.slash_palette.dismissed_query = None;
        self.sync_palette();
        self.slash_palette.open = false;
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

    #[test]
    fn composer_keeps_a_visible_input_row() {
        let pane = BottomPane::default();
        assert_eq!(pane.height(), 4);
    }

    #[test]
    fn slash_command_extracts_help_command() {
        assert_eq!(slash_command("/help").as_deref(), Some("help"));
    }

    #[test]
    fn slash_palette_opens_and_filters_as_user_types() {
        let mut pane = BottomPane::default();
        pane.handle_key(key(KeyCode::Char('/')));
        assert!(pane.slash_palette.open);
        assert!(!pane.slash_palette.matches.is_empty());

        pane.handle_key(key(KeyCode::Char('p')));
        assert!(pane.slash_palette.open);
        assert_eq!(pane.slash_palette.query, "p");
        assert_eq!(pane.slash_palette.selected().map(|spec| spec.name), Some("provider"));
    }

    #[test]
    fn slash_palette_accepts_selection_without_submitting() {
        let mut pane = BottomPane::default();
        pane.handle_key(key(KeyCode::Char('/')));
        let submission = pane.handle_key(key(KeyCode::Enter));
        assert!(submission.is_none());
        assert_eq!(pane.composer.text, "/help");
        assert!(!pane.slash_palette.open);
    }

    #[test]
    fn slash_palette_uses_arrow_keys_and_escape() {
        let mut pane = BottomPane::default();
        pane.handle_key(key(KeyCode::Char('/')));
        pane.handle_key(key(KeyCode::Down));
        assert_eq!(pane.slash_palette.selected_index, 1);
        pane.handle_key(key(KeyCode::Esc));
        assert!(!pane.slash_palette.open);
        assert_eq!(pane.composer.text, "/");
    }

    #[test]
    fn slash_palette_tab_accepts_selection_and_prevents_sidebar_conflicts() {
        let mut pane = BottomPane::default();
        pane.handle_key(key(KeyCode::Char('/')));
        pane.handle_key(key(KeyCode::Char('p')));
        let submission = pane.handle_key(key(KeyCode::Tab));
        assert!(submission.is_none());
        assert_eq!(pane.composer.text, "/provider ");
        assert!(!pane.slash_palette.open);
    }
}
