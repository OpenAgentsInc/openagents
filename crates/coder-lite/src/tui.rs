//! Full-screen coder TUI layout matching packages/openagents-cli/src/coder-ui.ts

use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use ratatui_markdown::markdown::MarkdownRenderer;
use ratatui_markdown::theme::{CodeColors, Generation, RichTextTheme};

const TEXT_COLOR: Color = Color::Rgb(255, 176, 0);
const BACKGROUND_COLOR: Color = Color::Rgb(8, 6, 0);
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Debug, Clone, Copy)]
struct CoderTheme;

impl RichTextTheme for CoderTheme {
    fn generation(&self) -> Generation {
        Generation(1)
    }
    fn get_text_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_muted_text_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_primary_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_popup_selected_background(&self) -> Color {
        BACKGROUND_COLOR
    }
    fn get_border_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_focused_border_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_secondary_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_info_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_json_key_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_json_string_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_json_number_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_json_bool_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_json_null_color(&self) -> Color {
        TEXT_COLOR
    }
    fn get_accent_yellow(&self) -> Color {
        TEXT_COLOR
    }
    fn get_background_color(&self) -> Color {
        BACKGROUND_COLOR
    }
    fn get_code_colors(&self) -> CodeColors {
        CodeColors {
            comment: TEXT_COLOR,
            keyword: TEXT_COLOR,
            string: TEXT_COLOR,
            string_escape: TEXT_COLOR,
            number: TEXT_COLOR,
            constant: TEXT_COLOR,
            function: TEXT_COLOR,
            r#type: TEXT_COLOR,
            variable: TEXT_COLOR,
            property: TEXT_COLOR,
            operator: TEXT_COLOR,
            punctuation: TEXT_COLOR,
            attribute: TEXT_COLOR,
            tag: TEXT_COLOR,
            label: TEXT_COLOR,
            error: TEXT_COLOR,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Tool output text, rendered as a ~5-line box split by newlines.
    pub output: Option<String>,
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
    pub tick: u64,
    pub agents: Vec<crate::acp::Agent>,
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return Vec::new();
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
            tick: 0,
            agents: Vec::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.tick = self.tick.wrapping_add(1);
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

        let main_bottom = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(input_box_height)])
            .split(area);

        let transcript_area = main_bottom[0];

        let mut all_lines = Vec::new();
        for entry in &self.entries {
            all_lines.extend(self.render_entry(entry, transcript_area.width as usize));
        }

        if self.loading {
            let spinner = SPINNER_FRAMES[self.tick as usize % SPINNER_FRAMES.len()];
            all_lines.push(Line::from(vec![Span::styled(spinner.to_string(), style)]));
        }

        let total = all_lines.len() as u16;
        self.transcript_height = transcript_area.height;
        self.scroll_max = total.saturating_sub(transcript_area.height);
        let start = self.effective_scroll(transcript_area.height, total);

        let transcript = Paragraph::new(Text::from(all_lines))
            .scroll((start, 0))
            .style(style);
        frame.render_widget(transcript, transcript_area);

        let input_area = main_bottom[1];

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
        frame.render_widget(input, input_area);

        let last_chunk = input_chunks.last().map(|s| s.as_str()).unwrap_or("");
        let cursor_x = input_area.x + 1 + 3 + last_chunk.chars().count() as u16;
        let cursor_y = input_area.y + 1 + visible_input_lines.saturating_sub(1);
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }

    fn render_entry(&self, entry: &Entry, width: usize) -> Vec<Line<'static>> {
        match entry.role {
            Role::Assistant if !entry.text.is_empty() => {
                let renderer = MarkdownRenderer::new(width.max(1));
                let blocks = renderer.parse(&entry.text);
                let mut lines = renderer.render(&blocks, &CoderTheme);

                for line in &mut lines {
                    let mapped = line
                        .spans
                        .drain(..)
                        .map(|span| {
                            let style = span.style.fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
                            Span::styled(span.content.to_string(), style)
                        })
                        .collect::<Vec<_>>();
                    *line = Line::from(mapped);
                }

                while lines.last().map_or(false, |l| {
                    l.spans.iter().all(|s| s.content.is_empty())
                }) {
                    lines.pop();
                }

                lines
            }
            Role::Tool => {
                let text_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
                let mut lines = Vec::new();

                // One-line tool call header, flush left.
                let header_body = width.saturating_sub(2);
                let header_chunks = wrap_text(&entry.text, header_body);
                let header = header_chunks.first().cloned().unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled("⏺ ", text_style),
                    Span::styled(header, text_style),
                ]));

                // ~5-line output box, split by actual newlines.
                let out = entry.output.as_deref().unwrap_or("");
                let out_lines: Vec<&str> = out.lines().collect();
                let start = out_lines.len().saturating_sub(5);
                let window = &out_lines[start..];
                for i in 0..5 {
                    let text = if i < window.len() { window[i] } else { "" };
                    let clipped = text.chars().take(width.saturating_sub(2)).collect::<String>();
                    lines.push(Line::from(vec![
                        Span::styled("│ ", text_style),
                        Span::styled(clipped, text_style),
                    ]));
                }

                lines
            }
            _ => {
                let text_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);

                let (first_prefix, marker, marker_space, rest_indent, first_body) = match entry.role {
                    Role::You => ("", ">", " ", "  ", width.saturating_sub(2)),
                    Role::Assistant => ("", "", "", "", width),
                    _ => ("", "⏺", " ", "  ", width.saturating_sub(2)),
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
        }
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
