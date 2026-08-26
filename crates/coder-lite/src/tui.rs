//! Full-screen coder TUI layout matching packages/openagents-cli/src/coder-ui.ts

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::markdown::theme::{BACKGROUND_COLOR, TEXT_COLOR};
use crate::osc8::PlacedLink;
use crate::transcript::MarkdownContent;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    You,
    Assistant,
    Tool,
    Reasoning,
    Notice,
}

/// One tool call captured for ATIF export.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub call_id: String,
    pub function_name: String,
    pub arguments: Value,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub role: Role,
    pub text: String,
    /// Tool output text, rendered as a ~5-line box split by newlines.
    pub output: Option<String>,
    pub tool: Option<ToolCall>,
    pub at: u64,
    /// Streaming markdown state for assistant entries.
    ///
    /// Built on first use and fed chunk by chunk, so the engine's checkpoint
    /// freezing survives across frames. `None` for every other role — those
    /// render as plain wrapped text and always did.
    md: Option<Box<MarkdownContent>>,
}

/// Current time as epoch milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Entry {
    /// An entry with no tool output, stamped with the current time.
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            output: None,
            tool: None,
            at: now_ms(),
            md: None,
        }
    }

    /// A tool-call entry with an (initially empty) output box.
    ///
    /// Named `tool_call` rather than `tool` because `Entry` also carries a
    /// `tool` field holding the ATIF [`ToolCall`] record; callers set that
    /// field after construction.
    pub fn tool_call(text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            text: text.into(),
            output: Some(String::new()),
            tool: None,
            at: now_ms(),
            md: None,
        }
    }

    /// Append a streamed chunk of assistant text.
    ///
    /// The chunk goes to both `text` (the verbatim source, which nothing
    /// rewrites) and the streaming markdown renderer. Keeping the source means
    /// a rendering failure can always fall back to showing what arrived, and
    /// it is what `/export` writes into the ATIF document.
    pub fn push_text(&mut self, chunk: &str) {
        if self.role == Role::Assistant {
            // Order matters. `markdown_mut` seeds a fresh renderer from
            // `self.text`, so the chunk must reach the renderer *before* it
            // joins `self.text` — otherwise the first chunk of a stream is
            // seeded and then pushed again, and the reader sees it twice.
            self.markdown_mut().push(chunk);
        }
        self.text.push_str(chunk);
    }

    /// Tell the markdown engine the stream ended, flushing any held-back bytes.
    pub fn finish_text(&mut self) {
        if self.role == Role::Assistant && self.md.is_some() {
            self.markdown_mut().finish();
        }
    }

    /// The streaming renderer, seeded from `text` if it does not exist yet.
    ///
    /// Seeding covers entries built whole rather than streamed (session
    /// replay, tests): they get the same rendering, just without the
    /// incremental saving there was nothing to save.
    fn markdown_mut(&mut self) -> &mut MarkdownContent {
        if self.md.is_none() {
            let mut content = MarkdownContent::new();
            if !self.text.is_empty() {
                content.push(&self.text);
            }
            self.md = Some(Box::new(content));
        }
        self.md.as_mut().expect("just inserted")
    }
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
    /// Hyperlinks on the last rendered frame, in absolute screen coordinates.
    ///
    /// The caller emits these as OSC 8 sequences after flushing the frame; see
    /// [`crate::osc8`].
    pub links: Vec<PlacedLink>,
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
            links: Vec::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.tick = self.tick.wrapping_add(1);
        let style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);

        // Fill the entire terminal with the background color first.
        let bg_line = Line::from(vec![Span::styled(" ".repeat(area.width as usize), style)]);
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
        let width = transcript_area.width as usize;

        let mut all_lines: Vec<Line<'static>> = Vec::new();
        let mut links = Vec::new();
        for index in 0..self.entries.len() {
            let offset = all_lines.len();
            let entry = &mut self.entries[index];
            let (lines, entry_links) = render_entry(entry, width);
            for mut link in entry_links {
                link.row += offset;
                links.push(link);
            }
            all_lines.extend(lines);
        }

        if self.loading {
            let spinner = SPINNER_FRAMES[self.tick as usize % SPINNER_FRAMES.len()];
            all_lines.push(Line::from(vec![Span::styled(spinner.to_string(), style)]));
        }

        let total = all_lines.len() as u16;
        self.transcript_height = transcript_area.height;
        self.scroll_max = total.saturating_sub(transcript_area.height);
        let start = self.effective_scroll(transcript_area.height, total);

        self.links = crate::osc8::place(&links, transcript_area, start as usize);

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

        let input = Paragraph::new(Text::from(input_lines)).style(style).block(
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

impl Default for CoderUi {
    fn default() -> Self {
        Self::new()
    }
}

/// Render one transcript entry to lines plus its hyperlinks.
///
/// Link rows are relative to the returned lines; the caller offsets them.
fn render_entry(
    entry: &mut Entry,
    width: usize,
) -> (Vec<Line<'static>>, Vec<crate::transcript::ScreenLink>) {
    let text_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);

    match entry.role {
        Role::Assistant if !entry.text.is_empty() => {
            let width = width.max(1);
            let md = entry.markdown_mut();
            let mut lines = md.lines(width).to_vec();
            let links = md.links(width).to_vec();

            // Trailing blank lines are layout, not content: the next entry
            // supplies its own separation.
            while lines
                .last()
                .is_some_and(|l| l.spans.iter().all(|s| s.content.trim().is_empty()))
            {
                lines.pop();
            }

            let links = links.into_iter().filter(|l| l.row < lines.len()).collect();
            (lines, links)
        }
        Role::Tool => {
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
                let clipped = text
                    .chars()
                    .take(width.saturating_sub(2))
                    .collect::<String>();
                lines.push(Line::from(vec![
                    Span::styled("│ ", text_style),
                    Span::styled(clipped, text_style),
                ]));
            }

            (lines, Vec::new())
        }
        _ => {
            let (first_prefix, marker, marker_space, rest_indent, first_body) = match entry.role {
                Role::You => ("", ">", " ", "  ", width.saturating_sub(2)),
                Role::Assistant => ("", "", "", "", width),
                // Flush left, matching 727ab02ece: a Notice or Reasoning
                // bullet sits at column 0 like the `>` of a user message.
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
            (lines, Vec::new())
        }
    }
}
