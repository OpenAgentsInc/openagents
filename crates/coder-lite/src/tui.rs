//! Full-screen coder TUI layout matching packages/openagents-cli/src/coder-ui.ts

use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const TEXT_COLOR: Color = Color::Rgb(255, 176, 0);
const BACKGROUND_COLOR: Color = Color::Rgb(8, 6, 0);

#[derive(Debug, Clone)]
pub enum Role {
    You,
    Assistant,
    Tool,
    Reasoning,
    Notice,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub role: Role,
    pub text: String,
}

#[derive(Debug)]
pub struct CoderUi {
    pub composer: String,
    pub repo: String,
    pub branch: String,
    pub model: String,
    pub reasoning: Option<String>,
    pub running: bool,
    pub entries: Vec<Entry>,
    /// Manual scroll override; `None` means the viewport follows the bottom.
    pub scroll_override: Option<u16>,
    pub scroll_max: u16,
    pub transcript_height: u16,
    pub loading: bool,
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.chars().count();
        if current.is_empty() {
            current.push_str(word);
            current_width = word_width;
        } else if current_width + 1 + word_width <= width {
            current.push(' ');
            current.push_str(word);
            current_width += 1 + word_width;
        } else {
            lines.push(current);
            current = word.to_string();
            current_width = word_width;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(text.to_string());
    }

    lines
}

fn wrap_input(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0;
        for word in paragraph.split_whitespace() {
            let word_width = word.chars().count();
            if current.is_empty() {
                current.push_str(word);
                current_width = word_width;
            } else if current_width + 1 + word_width <= width {
                current.push(' ');
                current.push_str(word);
                current_width += 1 + word_width;
            } else {
                lines.push(current);
                current = word.to_string();
                current_width = word_width;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

impl CoderUi {
    pub fn new() -> Self {
        Self {
            composer: String::new(),
            repo: "~/work/openagents".to_string(),
            branch: "main".to_string(),
            model: "sol-high".to_string(),
            reasoning: Some("medium".to_string()),
            running: true,
            entries: vec![],
            scroll_override: None,
            scroll_max: 0,
            transcript_height: 0,
            loading: false,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);

        // Fill the entire terminal with the background color first.
        let bg_line = Line::from(vec![Span::styled(
            " ".repeat(area.width as usize),
            style,
        )]);
        let bg = Paragraph::new(Text::from(vec![bg_line; area.height as usize]));
        frame.render_widget(bg, area);

        // ---- composer input (grok-style) ----
        let input_width = (area.width as usize)
            .saturating_sub(2)
            .saturating_sub(3)
            .max(1);
        let input_chunks = wrap_input(&self.composer, input_width);
        let total_input_lines = input_chunks.len() as u16;
        let max_input_lines: u16 = 8;
        let visible_input_lines = total_input_lines.min(max_input_lines);
        let input_scroll = total_input_lines.saturating_sub(visible_input_lines);
        let input_box_height = visible_input_lines + 2;
        let status_height = if self.loading { 1 } else { 0 };
        let bottom_height = input_box_height + status_height;

        let main_bottom = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(bottom_height)])
            .split(area);

        let transcript_area = main_bottom[0];

        let mut all_lines = Vec::new();
        for entry in &self.entries {
            all_lines.extend(self.render_entry(entry, transcript_area.width as usize));
        }

        let total = all_lines.len() as u16;
        self.transcript_height = transcript_area.height;
        self.scroll_max = total.saturating_sub(transcript_area.height);
        let start = self.effective_scroll(transcript_area.height, total);

        let transcript = Paragraph::new(Text::from(all_lines))
            .scroll((start, 0))
            .style(style);
        frame.render_widget(transcript, transcript_area);

        let bottom = main_bottom[1];
        let bottom_chunks = if self.loading {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(input_box_height)])
                .split(bottom)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(input_box_height)])
                .split(bottom)
        };

        if self.loading {
            let status = Paragraph::new(Line::from(vec![Span::styled("● working…", style)]))
                .style(style);
            frame.render_widget(status, bottom_chunks[0]);
        }

        let input_idx = if self.loading { 1 } else { 0 };

        let mut input_lines = Vec::new();
        for (i, chunk) in input_chunks.iter().enumerate().skip(input_scroll as usize) {
            let prefix = if i == 0 { " > " } else { "   " };
            input_lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(chunk.clone(), style),
            ]));
        }

        let input = Paragraph::new(Text::from(input_lines))
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .style(style),
            );
        frame.render_widget(input, bottom_chunks[input_idx]);

        let input_area = bottom_chunks[input_idx];
        let last_chunk = input_chunks.last().map(|s| s.as_str()).unwrap_or("");
        let cursor_x = input_area.x + 1 + 3 + last_chunk.chars().count() as u16;
        let cursor_y = input_area.y + 1 + visible_input_lines.saturating_sub(1);
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }

    fn render_entry(&self, entry: &Entry, width: usize) -> Vec<Line<'static>> {
        let text_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);

        let (first_prefix, marker, marker_space, rest_indent, first_body) = match entry.role {
            Role::You => ("", ">", " ", "  ", width.saturating_sub(2)),
            Role::Assistant => ("", "", "", "", width),
            _ => ("  ", "⏺", " ", "     ", width.saturating_sub(4)),
        };

        let chunks = wrap_text(&entry.text, first_body);

        let mut lines = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(first_prefix, text_style),
                    Span::styled(marker, text_style),
                    Span::styled(marker_space, text_style),
                    Span::styled(chunk.clone(), text_style),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(rest_indent, text_style),
                    Span::styled(chunk.clone(), text_style),
                ]));
            }
        }
        lines
    }

    /// Calculate the scroll offset that keeps the viewport at the bottom
    /// unless the user has manually overridden it.
    ///
    /// Mirrors grok-build's `effective_scroll` pattern: a `None` override
    /// follows the content, while an explicit offset is clamped to the valid
    /// range and left in place.
    fn effective_scroll(&self, area_height: u16, total: u16) -> u16 {
        let max_scroll = total.saturating_sub(area_height);
        match self.scroll_override {
            Some(ovr) => ovr.min(max_scroll),
            None => max_scroll,
        }
    }

    /// Scroll the transcript by `delta` lines. Positive scrolls down, negative
    /// up. Reaching the bottom clears the override so the viewport resumes
    /// following new messages.
    pub fn scroll_by(&mut self, delta: i32) {
        let max = self.scroll_max;
        let current = self.scroll_override.unwrap_or(max);
        let new = if delta < 0 {
            current.saturating_sub((-delta) as u16)
        } else {
            current.saturating_add(delta as u16).min(max)
        };
        self.scroll_override = if new >= max { None } else { Some(new) };
    }
}
