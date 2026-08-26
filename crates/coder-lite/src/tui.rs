//! Full-screen coder TUI layout matching packages/openagents-cli/src/coder-ui.ts

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_cli::composer::Composer;

use crate::markdown::theme::{BACKGROUND_COLOR, DIM_TEXT_COLOR, TEXT_COLOR};
use crate::osc8::PlacedLink;
use crate::transcript::MarkdownContent;
use openagents_cli::runtime::TurnUsage;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Columns the row under the composer keeps clear on the right.
///
/// That row is shared: the identity draws on the left and the credit balance
/// draws on the right, because both change what the reader should do next.
/// Reserving the columns rather than painting the whole width is what keeps
/// the two fields from erasing each other.
///
/// 26 is measured against the strings [`crate::credit::CreditField::status`]
/// actually formats, not chosen round. The widest is the unpriced-call state:
/// `credit: 3 unpriced calls` is 24 columns and `credit: 12 unpriced calls` is
/// 25, and a benchmark run on this lane came back with 12 unpriced calls — so
/// 24 would truncate the very case the field exists to report. 26 leaves a
/// column of air between the identity and the figure.
const BALANCE_COLUMNS: u16 = 26;

/// Who this session is signed in as.
///
/// Three states rather than a string, and the middle one is the reason this is
/// an enum. A token sitting in the keychain is a *claim* that the session is
/// signed in, not a fact: it may be revoked, expired, or for another
/// deployment. Rendering that claim as a normal-looking account row is how a
/// reader watches every turn fail while the screen says they are `AtlantisPleb`.
///
/// So [`Identity::Named`] is built only from what `GET /api/v1/user` answered
/// **this session**. A credential nobody confirmed is [`Identity::Unverified`]
/// and says so; no credential at all is [`Identity::Anonymous`] and says that.
/// The three read differently on screen, on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Identity {
    /// No credential at all. What the row says, and where the sign-in prompt
    /// belongs.
    #[default]
    Anonymous,
    /// A credential this session holds that the server has not confirmed —
    /// unreachable, refused, or expired. Never rendered as an account: an
    /// unconfirmed login name is worse than no name, because it reads as a
    /// working session.
    Unverified,
    /// The account the server named for this session's credential, this
    /// session.
    Named {
        login: String,
        id: i64,
        /// The namespaces this account may act in, as the server listed them.
        namespaces: Vec<String>,
        /// When the credential stops working, as the server stated it.
        expires_at: String,
    },
}

impl Identity {
    /// The account, in the words the row uses.
    ///
    /// Three distinct sentences, because two of these states are failures and
    /// a reader has to be able to tell which one they are looking at.
    pub fn line(&self) -> String {
        match self {
            Identity::Anonymous => "not signed in".to_string(),
            Identity::Unverified => "credential unverified".to_string(),
            Identity::Named { login, .. } => login.clone(),
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
    /// What one of the session's own commands printed — `/diff`, `/help`,
    /// `/resume`.
    ///
    /// Rendered through the markdown engine like an assistant turn, so a diff
    /// or a table reads the way it should, and exported as a notice rather
    /// than as a model step: the session wrote it, not a model, and a
    /// trajectory that says otherwise is a trajectory that lies about who
    /// produced what.
    Output,
}

impl Role {
    /// Whether this role's text goes through the streaming markdown engine.
    fn is_markdown(&self) -> bool {
        matches!(self, Role::Assistant | Role::Output)
    }
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
        if self.role.is_markdown() {
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
        if self.role.is_markdown() && self.md.is_some() {
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
    /// The line being typed.
    ///
    /// The grok-derived multi-line editor from `openagents-cli`, so the
    /// readline chords — Ctrl+A, Ctrl+E, Ctrl+W, Ctrl+K, Ctrl+U, Alt+B, Alt+F
    /// — and word motions work here, and the caret is where the caret is
    /// rather than always at the end of the text.
    pub composer: Composer,
    pub repo: String,
    pub branch: String,
    /// The model that answered the last turn, as its grant pinned it.
    ///
    /// Empty until one has. Never a guess: `--lane` says what was asked for,
    /// and this says what answered, and the two are not the same fact.
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
    /// Who the session is signed in as, as the server confirmed it.
    ///
    /// The left field of the row under the composer. Defaults to
    /// [`Identity::Anonymous`], which is the honest starting state: nothing has
    /// confirmed anything yet.
    pub identity: Identity,
    /// The `/api/v1` base this session talks to, as it was resolved.
    ///
    /// Empty until something sets it, and an empty one is left off the row
    /// rather than drawn as a dangling separator.
    pub endpoint: String,
    /// The lane the session was opened on — what was asked for, where
    /// [`Self::model`] is what answered.
    pub lane: String,
    /// The thread this session holds, once the server has opened one.
    pub thread: Option<String>,
    /// What the last turn spent, as the server reported it for that turn.
    pub last_usage: TurnUsage,
    pub total_usage: TurnUsage,
    /// What the last read of the account's credit found, or that it found
    /// nothing.
    ///
    /// Nothing here derives it from [`Self::total_usage`]: that total is this
    /// session's, and the credit is the account's. See [`crate::credit`] for
    /// why a failed read clears this rather than leaving the last figure up.
    ///
    /// Drawn on the right of the row under the composer, in the columns
    /// [`BALANCE_COLUMNS`] keeps clear of the identity.
    pub credit: crate::credit::CreditField,
    /// What the server billed this session's thread, once it has said.
    ///
    /// `None` until a figure arrives, and `/info` says "not reported yet"
    /// rather than printing a zero: a zero here reads as a measurement and it
    /// would not be one.
    pub billed: Option<u64>,
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

impl CoderUi {
    pub fn new() -> Self {
        Self {
            composer: Composer::new(),
            repo: "~/work/openagents".to_string(),
            branch: "main".to_string(),
            model: String::new(),
            reasoning: None,
            running: true,
            entries: vec![],
            scroll_override: None,
            scroll_max: 0,
            transcript_height: 0,
            loading: false,
            tick: 0,
            identity: Identity::Anonymous,
            endpoint: String::new(),
            lane: String::new(),
            thread: None,
            last_usage: TurnUsage::default(),
            total_usage: TurnUsage::default(),
            credit: crate::credit::CreditField::Unread,
            billed: None,
            agents: Vec::new(),
            links: Vec::new(),
        }
    }

    /// Record a turn's reported usage: the last turn, and the running total.
    ///
    /// Both are kept because `/info` reports both, and because the total is
    /// the figure the server's billed spend is held against.
    pub fn add_usage(&mut self, usage: TurnUsage) {
        self.last_usage = usage;
        self.total_usage.add(usage);
    }

    /// The left field of the row under the composer: who, and where.
    ///
    /// Never a token count — those moved to `/info`, where they can be read
    /// deliberately instead of glanced past. Never a name the server did not
    /// confirm this session; see [`Identity`].
    ///
    /// The endpoint appears as its host, because that is the part that tells a
    /// deployment from a laptop and the row shares its width with the balance.
    /// `/info` carries the whole URL.
    pub fn status_line(&self) -> String {
        let who = self.identity.line();
        match self.endpoint_host() {
            Some(host) => format!("{who} · {host}"),
            None => who,
        }
    }

    /// The right field of the same row: what the account has left.
    ///
    /// The identity and the balance stay separate fields rather than one
    /// joined string because they are facts about different things and are
    /// drawn into different rectangles — the identity is this session's, and
    /// the credit is the account's, read from the server and possibly moved by
    /// another terminal since. A field with nothing to report contributes an
    /// empty string, so the row never carries a placeholder that could be read
    /// as a value; see [`crate::credit::CreditField::status`].
    ///
    /// The token counts this row used to carry are not here. They are this
    /// session's spend, not the account's, and they moved to `/info`.
    pub fn balance_line(&self) -> String {
        self.credit.status()
    }

    /// The host of [`Self::endpoint`], scheme and path removed.
    ///
    /// `None` when nothing set an endpoint, so the row draws the identity
    /// alone rather than a separator with nothing after it.
    pub fn endpoint_host(&self) -> Option<&str> {
        let host = self
            .endpoint
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default();
        (!host.is_empty()).then_some(host)
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
        let input_chunks: Vec<String> = self
            .composer
            .rows(input_width)
            .into_iter()
            .map(str::to_string)
            .collect();
        let (caret_row, caret_col) = self.composer.cursor_rowcol(input_width);
        let total_input_lines = input_chunks.len() as u16;
        let max_input_lines: u16 = 8;
        let visible_input_lines = total_input_lines.min(max_input_lines);
        let input_scroll = total_input_lines.saturating_sub(visible_input_lines);
        let input_box_height = visible_input_lines + 2;

        let main = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(input_box_height),
                Constraint::Length(1),
            ])
            .split(area);

        let transcript_area = main[0];
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

        let input_area = main[1];

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

        // Where the caret actually is, not where the text ends: Ctrl+A and the
        // word motions move it, and a caret drawn at the end of the line while
        // the next character lands in the middle is a frame that lies.
        let caret_screen_row = (caret_row as u16).saturating_sub(input_scroll);
        let cursor_x = input_area.x + 1 + 3 + caret_col as u16;
        let cursor_y = input_area.y + 1 + caret_screen_row.min(visible_input_lines.saturating_sub(1));
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));

        // The row under the composer is what you can do next, not what you
        // already spent: who you are acting as, and — on the right, drawn by
        // the balance field — whether you can afford the next turn. The token
        // counts that used to live here moved to `/info`.
        //
        // The identity takes the left columns and stops there. A full-width
        // paragraph would paint over the balance, and a right-aligned one over
        // this, so the two fields have their own rectangles.
        let status_area = main[2];
        // The split is exact rather than two overlapping full-width
        // paragraphs: the identity gets everything up to the reserved columns
        // and the balance gets the rest, so the two rectangles cannot overlap
        // at any width. On a terminal narrower than BALANCE_COLUMNS the
        // identity is squeezed to nothing and the balance keeps the row,
        // because the balance is the field that changes what you do next.
        let identity_width = status_area.width.saturating_sub(BALANCE_COLUMNS);
        let identity_area = Rect {
            width: identity_width,
            ..status_area
        };
        let balance_area = Rect {
            x: status_area.x + identity_width,
            width: status_area.width - identity_width,
            ..status_area
        };
        let status_widget = Paragraph::new(self.status_line()).style(style);
        frame.render_widget(status_widget, identity_area);
        let balance_widget = Paragraph::new(self.balance_line())
            .style(style)
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(balance_widget, balance_area);
        // A block cursor, as grok-build's textarea draws one: the hardware
        // cursor alone is easy to lose in the alternate screen, and a trailing
        // space with nothing over it looks like a line that ends earlier than
        // it does. `REVERSED` swaps the two palette colours for one cell, so
        // the block is ground-on-amber and the palette is untouched.
        let buf = frame.buffer_mut();
        if let Some(cell) = buf.cell_mut((cursor_x, cursor_y)) {
            cell.modifier.insert(Modifier::REVERSED);
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
        Role::Assistant if !entry.text.is_empty() && entry.text.starts_with("[error:") => {
            let mut lines = Vec::new();
            for chunk in wrap_text(&entry.text, width) {
                lines.push(Line::from(vec![Span::styled(
                    chunk,
                    text_style.add_modifier(Modifier::BOLD),
                )]));
            }
            (lines, Vec::new())
        }
        ref role if role.is_markdown() && !entry.text.is_empty() => {
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

            let role_style = if matches!(entry.role, Role::Notice | Role::Reasoning) {
                Style::default().fg(DIM_TEXT_COLOR).bg(BACKGROUND_COLOR)
            } else {
                text_style
            };

            let chunks = wrap_text(&entry.text, first_body);

            let mut lines = Vec::new();
            for (i, chunk) in chunks.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(first_prefix, role_style),
                        Span::styled(marker, role_style),
                        Span::styled(marker_space, role_style),
                        Span::styled(chunk.clone(), role_style),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(rest_indent, role_style),
                        Span::styled(chunk.clone(), role_style),
                    ]));
                }
            }
            (lines, Vec::new())
        }
    }
}
