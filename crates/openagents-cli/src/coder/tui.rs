//! Full-screen coder TUI layout matching packages/openagents-cli/src/coder-ui.ts

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

use crate::composer::Composer;

use crate::coder::markdown::theme::{
    BACKGROUND_COLOR, DIM_TEXT_COLOR, MODEL_TEXT_COLOR, TEXT_COLOR, USER_TEXT_COLOR,
};
use crate::coder::osc8::PlacedLink;
use crate::coder::transcript::MarkdownContent;
use crate::runtime::{ImageAttachment, TurnUsage};

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const TOOL_RAIL_WAVE_ROWS: f32 = 32.0;
const TOOL_RAIL_WAVE_SPEED: f32 = 0.15;
const TOOL_OUTPUT_ROWS: usize = 4;
const TOOL_OUTPUT_SCROLL_FRAMES: u64 = 24;
const TOOL_SETTLE_FRAMES: u64 = 10;
const MAX_VISIBLE_COMMAND_SUGGESTIONS: usize = 8;

/// Who this session is signed in as.
///
/// Three states rather than a string, and the middle one is the reason this is
/// an enum. A token sitting in the credential file is a *claim* that the session is
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
        matches!(self, Role::Assistant | Role::Output | Role::Reasoning)
    }

    /// Whether this role renders with single newlines kept as line breaks.
    ///
    /// Reasoning summaries are written to be read with their layout intact —
    /// "1. assess\n2. plan" over three lines, not folded into one sentence.
    /// CommonMark's soft-break folding is for prose, so reasoning opts out.
    fn is_source_faithful(&self) -> bool {
        matches!(self, Role::Reasoning)
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
    /// Whether the runtime has settled this call.
    pub done: bool,
    /// Whole milliseconds the call held the session, as reported at settle.
    /// `None` while in flight; a call that never reached a tool stays 0.
    pub duration_ms: Option<u64>,
}

impl ToolCall {
    /// The duration the export carries: `0` until the call settles, so a
    /// document built mid-flight still validates against the schema.
    pub fn exported_duration_ms(&self) -> u64 {
        self.duration_ms.unwrap_or(0)
    }

    /// The settle path records what the run cost, on the entry itself.
    pub fn settle_duration_ms(&mut self, duration_ms: u64) {
        self.duration_ms = Some(duration_ms);
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub role: Role,
    pub text: String,
    /// The local turn generation this entry terminates, when applicable.
    pub turn_id: Option<u64>,
    /// The model that produced this assistant entry, once the runtime reports it.
    pub model: Option<String>,
    /// Tool output text, rendered as a box of up to four lines.
    pub output: Option<String>,
    pub tool: Option<ToolCall>,
    pub at: u64,
    /// The first frame of the one-shot preview sweep for long tool output.
    output_scroll_started: Option<u64>,
    /// The frame on which this tool finished, for its one-shot settle fade.
    tool_settled_at: Option<u64>,
    /// Streaming markdown state for assistant entries.
    ///
    /// Built on first use and fed chunk by chunk, so the engine's checkpoint
    /// freezing survives across frames. `None` for every other role — those
    /// render as plain wrapped text and always did.
    md: Option<Box<MarkdownContent>>,
    /// Markdown state for tool output, rebuilt only when its source changes.
    output_md: Option<Box<MarkdownContent>>,
    output_md_source: String,
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
            turn_id: None,
            model: None,
            output: None,
            tool: None,
            at: now_ms(),
            output_scroll_started: None,
            tool_settled_at: None,
            md: None,
            output_md: None,
            output_md_source: String::new(),
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
            turn_id: None,
            model: None,
            output: Some(String::new()),
            tool: None,
            at: now_ms(),
            output_scroll_started: None,
            tool_settled_at: None,
            md: None,
            output_md: None,
            output_md_source: String::new(),
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

    /// Start the one-shot visual transition from active to completed.
    pub(crate) fn settle_tool(&mut self, tick: u64) {
        self.tool_settled_at = Some(tick);
    }

    /// The streaming renderer, seeded from `text` if it does not exist yet.
    ///
    /// Seeding covers entries built whole rather than streamed (session
    /// replay, tests): they get the same rendering, just without the
    /// incremental saving there was nothing to save.
    fn markdown_mut(&mut self) -> &mut MarkdownContent {
        if self.md.is_none() {
            let mut content = if self.role.is_source_faithful() {
                MarkdownContent::source_faithful()
            } else {
                MarkdownContent::new()
            };
            if !self.text.is_empty() {
                content.push(&self.text);
            }
            self.md = Some(Box::new(content));
        }
        self.md.as_mut().expect("just inserted")
    }

    /// Render the current tool output through the same markdown engine as an
    /// assistant turn. Tool output arrives as a growing snapshot, so rebuild
    /// the renderer only when that snapshot changes.
    fn output_markdown_mut(&mut self) -> &mut MarkdownContent {
        let source = self.output.as_deref().unwrap_or("");
        if self.output_md.is_none() || self.output_md_source != source {
            let mut content = MarkdownContent::new();
            content.push(source);
            content.finish();
            self.output_md = Some(Box::new(content));
            self.output_md_source.clear();
            self.output_md_source.push_str(source);
        }
        self.output_md.as_mut().expect("just inserted")
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
    /// Images represented by path-free markers in the current draft.
    pub images: Vec<ImageAttachment>,
    next_image_id: usize,
    pub repo: String,
    pub branch: String,
    /// The model that answered the last turn, as its grant pinned it.
    ///
    /// Empty until one has. Never a guess: `--lane` says what was asked for,
    /// and this says what answered, and the two are not the same fact.
    pub model: String,
    pub entries: Vec<Entry>,
    /// The current session goal, when one exists.
    pub goal: Option<crate::coder::goal::Goal>,
    /// Whether the centered startup summary is still visible.
    ///
    /// It is UI chrome, not a transcript entry, and disappears when the first
    /// prompt is sent.
    pub show_welcome: bool,
    /// The directory Coder was started in, shown in the startup summary.
    pub cwd: String,
    /// Manual scroll override; `None` means the viewport follows the bottom.
    pub scroll_override: Option<u16>,
    pub scroll_max: u16,
    pub transcript_height: u16,
    pub loading: bool,
    /// Temporary first-response state shown beside the spinner.
    pub waiting: Option<String>,
    /// Current turn and prompt-queue state, rendered in the status row.
    pub activity: String,
    pub tick: u64,
    /// Whether active-state motion advances. Set `OPENAGENTS_REDUCED_MOTION`
    /// to keep the same state markers with a static rail.
    pub motion_enabled: bool,
    /// Who the session is signed in as, as the server confirmed it.
    ///
    /// Defaults to [`Identity::Anonymous`], which is the honest starting
    /// state: nothing has confirmed anything yet. `/info` displays it.
    pub identity: Identity,
    /// The `/api/v1` base this session talks to, as it was resolved.
    ///
    /// Empty until something sets it. `/info` displays the resolved endpoint.
    pub endpoint: String,
    /// The lane the session was opened on — what was asked for, where
    /// [`Self::model`] is what answered.
    pub lane: String,
    /// The thread this session holds, once the server has opened one.
    pub thread: Option<String>,
    /// The local source-of-truth session and its directory.
    pub local_session_id: Option<String>,
    pub local_session_path: Option<String>,
    /// Whether this invocation opted in to durable server transcript storage.
    pub cloud_history: bool,
    /// What the last turn spent, as the server reported it for that turn.
    pub last_usage: TurnUsage,
    pub total_usage: TurnUsage,
    /// What the last read of the account's credit found, or that it found
    /// nothing.
    ///
    /// Nothing here derives it from [`Self::total_usage`]: that total is this
    /// session's, and the credit is the account's. See [`crate::coder::credit`] for
    /// why a failed read clears this rather than leaving the last figure up.
    ///
    /// Drawn on the left of the row under the composer.
    pub credit: crate::coder::credit::CreditField,
    /// What the server billed this session's thread, once it has said.
    ///
    /// `None` until a figure arrives, and `/info` says "not reported yet"
    /// rather than printing a zero: a zero here reads as a measurement and it
    /// would not be one.
    pub billed: Option<u64>,
    pub agents: Vec<crate::coder::acp::Agent>,
    /// Hyperlinks on the last rendered frame, in absolute screen coordinates.
    ///
    /// The caller emits these as OSC 8 sequences after flushing the frame; see
    /// [`crate::coder::osc8`].
    pub links: Vec<PlacedLink>,
    /// Where the frame left the cursor: the composer caret, in absolute screen
    /// coordinates.
    ///
    /// The caller must put the cursor back here after the OSC 8 pass parks it
    /// at the last link (#187). `None` when the frame hid the cursor.
    pub cursor: Option<Position>,
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
            images: Vec::new(),
            next_image_id: 1,
            repo: "~/work/openagents".to_string(),
            branch: "main".to_string(),
            model: String::new(),
            entries: vec![],
            goal: None,
            show_welcome: true,
            cwd: String::new(),
            scroll_override: None,
            scroll_max: 0,
            transcript_height: 0,
            loading: false,
            waiting: None,
            activity: "Idle".to_string(),
            tick: 0,
            motion_enabled: std::env::var_os("OPENAGENTS_REDUCED_MOTION").is_none(),
            identity: Identity::Anonymous,
            endpoint: String::new(),
            lane: String::new(),
            thread: None,
            local_session_id: None,
            local_session_path: None,
            cloud_history: false,
            last_usage: TurnUsage::default(),
            total_usage: TurnUsage::default(),
            credit: crate::coder::credit::CreditField::Unread,
            billed: None,
            agents: Vec::new(),
            links: Vec::new(),
            cursor: None,
        }
    }

    /// Attach a paste made entirely of supported image paths.
    ///
    /// Returns `false` for ordinary text, which the caller should insert
    /// unchanged.
    pub fn attach_dropped_images(&mut self, text: &str) -> Result<bool, String> {
        let Some(images) = crate::coder::image::read_dropped_images(text, &mut self.next_image_id)?
        else {
            return Ok(false);
        };
        if !self.composer.is_empty()
            && !self
                .composer
                .text()
                .chars()
                .last()
                .is_some_and(char::is_whitespace)
        {
            self.composer.insert_str(" ");
        }
        let markers = images
            .iter()
            .map(|image| format!("[Image #{}]", image.id))
            .collect::<Vec<_>>()
            .join(" ");
        self.composer.insert_str(&markers);
        self.composer.insert_str(" ");
        self.images.extend(images);
        Ok(true)
    }

    /// Remove this draft's image state and return only attachments whose
    /// markers remain in the submitted text.
    pub fn take_referenced_images(&mut self, text: &str) -> Vec<ImageAttachment> {
        self.images
            .drain(..)
            .filter(|image| text.contains(&format!("[Image #{}]", image.id)))
            .collect()
    }

    /// Record a turn's reported usage: the last turn, and the running total.
    ///
    /// Both are kept because `/info` reports both, and because the total is
    /// the figure the server's billed spend is held against.
    pub fn add_usage(&mut self, usage: TurnUsage) {
        self.last_usage = usage;
        self.total_usage.add(usage);
    }

    /// The left field of the row under the composer: what the account has left.
    ///
    /// The credit stays separate from the lane/model because each refreshes at
    /// a different time. The credit is the account's, read from the server and
    /// possibly moved by another terminal since. A field with nothing to
    /// report contributes an empty string, so the row never carries a
    /// placeholder that could be read as a value; see
    /// [`crate::coder::credit::CreditField::status`].
    ///
    /// The token counts this row used to carry are not here. They are this
    /// session's spend, not the account's, and they moved to `/info`.
    pub fn balance_line(&self) -> String {
        self.credit.status()
    }

    /// What the row says about the version, lane, and model on the right.
    ///
    /// The build version leads — `v0.1.1-rc1 · Coder Flash` — so a frame, and
    /// therefore a screenshot, always says which binary produced the session.
    /// Then the lane name alone until a model has answered, and the lane name
    /// plus **the model that answered** from then on — [`Self::model`] is what
    /// the grant pinned, never what the lane asked for. That is the whole
    /// point of putting the lane here: `Coder Flash` while Gemini is answering
    /// is the defect this row exists to prevent, so the two are always drawn
    /// together and the reader never has to trust that the label still means
    /// what it meant at open.
    ///
    /// Empty only when even the version will not fit, so the field costs no
    /// columns on a frame that has nothing to say.
    pub fn lane_field(&self) -> String {
        self.lane_field_within(u16::MAX)
    }

    /// The same field, narrowed to what will fit beside the credit figure.
    ///
    /// It gives up the lane name first, then the version, because the model
    /// is the load-bearing half: `Coder Flash` while Gemini is answering is
    /// the defect this field exists to prevent, whereas a bare
    /// `gemini-3.7-flash` is still true. And it renders **nothing at all**
    /// rather than a form that would not fit whole — never a truncated id,
    /// and never a lane name standing alone once a model has answered, which
    /// would be the forbidden state written out. `/info` carries the version,
    /// lane, and model either way, so what is dropped here is recoverable
    /// rather than lost.
    fn lane_field_within(&self, columns: u16) -> String {
        let columns = columns as usize;
        let fits = |text: &str| text.chars().count() <= columns;
        // The running build, from the same constant the welcome box titles.
        let version = format!("v{}", crate::VERSION);

        // No lane recorded yet: the version is still worth its columns.
        if self.lane.is_empty() {
            return match fits(&version) {
                true => version,
                false => String::new(),
            };
        }

        // Nothing has answered yet, so nothing is claimed about a model and
        // the lane name alone cannot mislead.
        if self.model.is_empty() {
            let full = format!("{} · {}", version, self.lane);
            if fits(&full) {
                return full;
            }
            if fits(&self.lane) {
                return self.lane.clone();
            }
            return match fits(&version) {
                true => version,
                false => String::new(),
            };
        }

        let full = format!("{} · {} · {}", version, self.lane, self.model);
        if fits(&full) {
            return full;
        }
        // The model is the load-bearing half; the version rides with it as
        // long as both fit. The lane name is the nicety.
        let modelled = format!("{} · {}", version, self.model);
        if fits(&modelled) {
            return modelled;
        }
        if fits(&self.model) {
            return self.model.clone();
        }
        if fits(&version) {
            return version;
        }
        String::new()
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
            let entry = &mut self.entries[index];
            if entry.role == Role::Tool {
                all_lines.push(Line::default());
            }
            let offset = all_lines.len();
            let (lines, entry_links) = render_entry(entry, width, self.tick, self.motion_enabled);
            for mut link in entry_links {
                link.row += offset;
                links.push(link);
            }
            all_lines.extend(lines);
        }

        if self.loading {
            let spinner = SPINNER_FRAMES[self.tick as usize % SPINNER_FRAMES.len()];
            let status = self.waiting.as_deref().unwrap_or_default();
            let text = if status.is_empty() {
                spinner.to_string()
            } else {
                format!("{spinner} {status}")
            };
            all_lines.push(Line::from(vec![Span::styled(text, style)]));
        }

        let total = all_lines.len() as u16;
        self.transcript_height = transcript_area.height;
        self.scroll_max = total.saturating_sub(transcript_area.height);
        let start = self.effective_scroll(transcript_area.height, total);

        self.links = crate::coder::osc8::place(&links, transcript_area, start as usize);

        let transcript = Paragraph::new(Text::from(all_lines))
            .scroll((start, 0))
            .style(style);
        frame.render_widget(transcript, transcript_area);

        let conversation_started = self
            .entries
            .iter()
            .any(|entry| matches!(entry.role, Role::You | Role::Assistant | Role::Tool));
        if self.show_welcome && !conversation_started {
            self.render_welcome(frame, transcript_area);
        }

        self.render_slash_suggestions(frame, transcript_area);

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
        let cursor_y =
            input_area.y + 1 + caret_screen_row.min(visible_input_lines.saturating_sub(1));
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
        // Recorded for the caller: the OSC 8 pass runs after the frame and
        // must put the cursor back here (#187).
        self.cursor = Some(Position::new(cursor_x, cursor_y));

        // The row under the composer shows the two live facts that matter
        // during a session: the remaining credit on the left and the active
        // lane/model on the right. Account and endpoint details stay in
        // `/info`, where they do not consume persistent screen space.
        let status_area = main[2];
        // The status row is supporting context beneath the composer. Keep it
        // at the same 50% amber intensity as notices so the active transcript
        // and input remain the visual focus.
        let status_style = Style::default().fg(DIM_TEXT_COLOR).bg(BACKGROUND_COLOR);
        // The lane takes exactly the columns it needs and is right-aligned to
        // the edge. The balance gets the remaining columns on the left; no
        // fixed gutter survives when either field is short.
        let mut balance = self.balance_line();
        let balance_width = (balance.chars().count() as u16).min(status_area.width);
        let gap = u16::from(balance_width > 0);
        let lane = self.lane_field_within(
            status_area
                .width
                .saturating_sub(balance_width.saturating_add(gap)),
        );
        let lane_width = (lane.chars().count() as u16).min(status_area.width);
        let fits_left = |text: &str| {
            let text_width = text.chars().count() as u16;
            let gap = u16::from(text_width > 0 && lane_width > 0);
            text_width.saturating_add(gap).saturating_add(lane_width) <= status_area.width
        };
        // Activity and goal text are useful progress details, but they must
        // not evict the credit or effective model that govern the next turn.
        if !self.activity.is_empty() {
            let separator = if balance.is_empty() { "" } else { " · " };
            let candidate = format!("{balance}{separator}{}", self.activity);
            if fits_left(&candidate) {
                balance = candidate;
            }
        }
        if let Some(goal) = self
            .goal
            .as_ref()
            .filter(|goal| goal.status == crate::coder::goal::GoalStatus::Active)
        {
            let snippet = if goal.objective.chars().count() > 25 {
                format!("{}…", goal.objective.chars().take(22).collect::<String>())
            } else {
                goal.objective.clone()
            };
            let field = format!("goal: \"{snippet}\"");
            let separator = if balance.is_empty() { "" } else { " · " };
            let candidate = format!("{balance}{separator}{field}");
            if fits_left(&candidate) {
                balance = candidate;
            }
        }
        let balance_area = Rect {
            width: status_area.width.saturating_sub(lane_width),
            ..status_area
        };
        let lane_area = Rect {
            x: status_area.x + balance_area.width,
            width: lane_width,
            ..status_area
        };
        let balance_widget = Paragraph::new(balance).style(status_style);
        frame.render_widget(balance_widget, balance_area);
        let lane_widget = Paragraph::new(lane)
            .style(status_style)
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(lane_widget, lane_area);
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

    /// Draw command matches above the composer while you type a slash command.
    ///
    /// This follows grok-build's prompt-anchored dropdown: the list is derived
    /// from the current draft, overlays the transcript temporarily, and never
    /// becomes a transcript entry. Names use primary amber and descriptions
    /// use the secondary amber intensity.
    fn render_slash_suggestions(&self, frame: &mut Frame, area: Rect) {
        let matches = slash_command_suggestions(self.composer.text());
        if matches.is_empty() || area.width < 24 || area.height < 3 {
            return;
        }

        let visible = matches
            .iter()
            .take(MAX_VISIBLE_COMMAND_SUGGESTIONS)
            .copied()
            .collect::<Vec<_>>();
        let height = (visible.len() as u16 + 2).min(area.height);
        let dropdown = Rect {
            x: area.x,
            y: area.bottom().saturating_sub(height),
            width: area.width,
            height,
        };
        let label_width = visible
            .iter()
            .map(|(name, _)| name.len() + 1)
            .max()
            .unwrap_or(0);
        let name_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
        let description_style = Style::default().fg(DIM_TEXT_COLOR).bg(BACKGROUND_COLOR);
        let rows = visible
            .iter()
            .map(|(name, description)| {
                let label = format!("/{name}");
                Line::from(vec![
                    Span::styled(format!(" {label:<label_width$}  "), name_style),
                    Span::styled((*description).to_string(), description_style),
                ])
            })
            .collect::<Vec<_>>();
        let title = if matches.len() > visible.len() {
            format!(" Commands · {} matches ", matches.len())
        } else {
            " Commands ".to_string()
        };
        let block = Block::default()
            .title(Span::styled(title, description_style))
            .borders(Borders::ALL)
            .border_style(description_style)
            .style(name_style);
        frame.render_widget(Paragraph::new(Text::from(rows)).block(block), dropdown);
    }

    /// Draw the startup facts as centered UI chrome, outside the transcript.
    fn render_welcome(&self, frame: &mut Frame, area: Rect) {
        if area.width < 20 || area.height < 7 {
            return;
        }

        let width = area.width.saturating_sub(4).min(100);
        let height = 8.min(area.height);
        let welcome_area = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        };
        let body_width = width.saturating_sub(4) as usize;
        let value_width = body_width.saturating_sub("Working directory  ".len());
        let agents = if self.agents.is_empty() {
            "None installed".to_string()
        } else {
            self.agents
                .iter()
                .map(|agent| agent.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let fit = |value: &str| {
            let truncated = value.chars().count() > value_width;
            let mut value = value.chars().take(value_width).collect::<String>();
            if truncated && value_width > 0 {
                value.pop();
                value.push('…');
            }
            value
        };
        let label_style = Style::default().fg(DIM_TEXT_COLOR).bg(BACKGROUND_COLOR);
        let value_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
        let row = |label: &'static str, value: String| {
            Line::from(vec![
                Span::styled(label, label_style),
                Span::styled(value, value_style),
            ])
        };
        let content = Text::from(vec![
            row("Working directory  ", fit(&self.cwd)),
            row("Endpoint           ", fit(&self.endpoint)),
            row("ACP agents         ", fit(&agents)),
            Line::default(),
            Line::from(Span::styled(
                "Type /help for commands and keys.",
                label_style,
            )),
        ]);
        let block = Block::default()
            .title(format!(" Coder v{} ", crate::VERSION))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(value_style)
            .style(value_style);
        frame.render_widget(Paragraph::new(content).block(block), welcome_area);
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

/// Commands matching the command-name token currently being typed.
fn slash_command_suggestions(text: &str) -> Vec<(&'static str, &'static str)> {
    let trimmed = text.trim_start();
    let Some(body) = trimmed.strip_prefix('/') else {
        return Vec::new();
    };
    if body.chars().any(char::is_whitespace)
        || body.contains('/')
        || body.contains('.')
        || body.starts_with('~')
    {
        return Vec::new();
    }

    super::commands::COMMANDS
        .iter()
        .copied()
        .filter(|(name, _)| name.starts_with(body))
        .collect()
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
    tick: u64,
    motion_enabled: bool,
) -> (
    Vec<Line<'static>>,
    Vec<crate::coder::transcript::ScreenLink>,
) {
    let text_style = Style::default().fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
    let model = (entry.role == Role::Assistant)
        .then(|| entry.model.clone())
        .flatten()
        .filter(|model| model.width() + 2 <= width);
    let body_width = model
        .as_ref()
        .map(|model| width.saturating_sub(model.width() + 1))
        .unwrap_or(width);

    let (mut lines, links) = match entry.role {
        Role::Reasoning if !entry.text.is_empty() => {
            // The reasoning bullet keeps the column it always had; the
            // markdown that follows it wraps inside what is left. Blocks sit
            // flush at the indent, inline styles indent only their text, and
            // one dim repaint unifies the block with the role it renders.
            let bullet_body = width.saturating_sub(2).max(1);
            let md = entry.markdown_mut();
            let raw_lines = md.lines(bullet_body).to_vec();
            let raw_links = md.links(bullet_body).to_vec();
            let mut lines = Vec::with_capacity(raw_lines.len());
            let mut row_prefix_widths = Vec::with_capacity(raw_lines.len());
            for (row, line) in raw_lines.into_iter().enumerate() {
                let (prefix, indent) = if row == 0 { ("⏺ ", "  ") } else { ("", "  ") };
                let gutter = format!("{prefix}{indent}");
                row_prefix_widths.push(UnicodeWidthStr::width(gutter.as_str()));
                let mut spans = Vec::with_capacity(line.spans.len() + 1);
                spans.push(Span::styled(gutter, text_style.fg(DIM_TEXT_COLOR)));
                for mut span in line.spans {
                    span.style.fg = Some(DIM_TEXT_COLOR);
                    span.style.bg = Some(BACKGROUND_COLOR);
                    spans.push(span);
                }
                lines.push(Line::from(spans));
            }
            // Link columns were measured inside the wrapped body; shift them
            // past the gutter this role draws in front of each row.
            let links = raw_links
                .into_iter()
                .map(|mut link| {
                    let shift = row_prefix_widths.get(link.row).copied().unwrap_or(0);
                    link.col_start += shift;
                    link.col_end += shift;
                    link
                })
                .collect();
            (lines, links)
        }
        Role::Assistant if !entry.text.is_empty() && entry.text.starts_with("[error:") => {
            let mut lines = Vec::new();
            for chunk in wrap_text(&entry.text, body_width) {
                lines.push(Line::from(vec![Span::styled(
                    chunk,
                    text_style.add_modifier(Modifier::BOLD),
                )]));
            }
            (lines, Vec::new())
        }
        ref role if role.is_markdown() && !entry.text.is_empty() => {
            let width = if *role == Role::Assistant {
                body_width
            } else {
                width
            }
            .max(1);
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

            // Render markdown-shaped tool output with the shared engine. Keep
            // ordinary command output line-oriented: CommonMark treats a
            // single newline as a space, which would destroy logs and tables
            // that are plain text rather than markdown.
            let output_width = width.saturating_sub(2).max(1);
            let output = entry.output.as_deref().unwrap_or("").to_string();
            let (mut out_lines, out_links) = if tool_output_is_markdown(&output) {
                let md = entry.output_markdown_mut();
                let rendered = md.lines(output_width).to_vec();
                let links = md.links(output_width).to_vec();
                (rendered, links)
            } else {
                (
                    output
                        .lines()
                        .map(|line| Line::from(Span::raw(line.to_string())))
                        .collect(),
                    Vec::new(),
                )
            };
            while out_lines
                .last()
                .is_some_and(|line| line.spans.iter().all(|span| span.content.trim().is_empty()))
            {
                out_lines.pop();
            }
            let final_start = out_lines.len().saturating_sub(TOOL_OUTPUT_ROWS);
            let start = if final_start == 0 || !motion_enabled {
                final_start
            } else {
                let started = *entry.output_scroll_started.get_or_insert(tick);
                tool_output_scroll_start(final_start, tick.saturating_sub(started))
            };
            let end = (start + TOOL_OUTPUT_ROWS).min(out_lines.len());
            let window = &out_lines[start..end];

            // One-line tool call header, flush left.
            let done = entry.tool.as_ref().is_some_and(|tool| tool.done);
            let marker = if done { "⏺ " } else { "○ " };
            let marker_style = if done {
                text_style.fg(tool_settle_color(entry, tick, motion_enabled))
            } else if !motion_enabled {
                text_style
            } else {
                text_style.fg(tool_rail_wave_color(tick, 0))
            };
            let header_body = width.saturating_sub(2);
            let header_text = tool_header_text(entry);
            let header_chunks = wrap_text(&header_text, header_body);
            let header = header_chunks.first().cloned().unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(marker, marker_style),
                Span::styled(header, text_style),
            ]));

            // Only draw the vertical bar for lines that actually exist.
            for (row, line) in window.iter().enumerate() {
                let rail_style = if done {
                    text_style.fg(tool_settle_color(entry, tick, motion_enabled))
                } else if !motion_enabled {
                    text_style
                } else {
                    text_style.fg(tool_rail_wave_color(
                        tick,
                        u16::try_from(start + row + 1).unwrap_or(u16::MAX),
                    ))
                };
                let mut spans = Vec::with_capacity(line.spans.len() + 1);
                spans.push(Span::styled("│ ", rail_style));
                spans.extend(line.spans.iter().cloned().map(|mut span| {
                    span.style.fg = Some(DIM_TEXT_COLOR);
                    span.style.bg = Some(BACKGROUND_COLOR);
                    span
                }));
                lines.push(Line::from(spans));
            }

            let links = out_links
                .into_iter()
                .filter(|link| (start..end).contains(&link.row))
                .map(|mut link| {
                    link.row = link.row - start + 1;
                    link.col_start += 2;
                    link.col_end += 2;
                    link
                })
                .collect();
            (lines, links)
        }
        _ => {
            let (first_prefix, marker, marker_space, rest_indent, first_body) = match entry.role {
                Role::You => ("", ">", " ", "  ", width.saturating_sub(2)),
                Role::Assistant => ("", "", "", "", width),
                // Flush left, matching 727ab02ece: a Notice or Reasoning
                // bullet sits at column 0 like the `>` of a user message.
                _ => ("", "⏺", " ", "  ", width.saturating_sub(2)),
            };

            let role_style = match entry.role {
                Role::Notice | Role::Reasoning => {
                    Style::default().fg(DIM_TEXT_COLOR).bg(BACKGROUND_COLOR)
                }
                Role::You => Style::default().fg(USER_TEXT_COLOR).bg(BACKGROUND_COLOR),
                _ => text_style,
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
    };

    if let (Some(model), Some(first)) = (model, lines.first_mut()) {
        let line_width = first
            .spans
            .iter()
            .map(|span| span.content.width())
            .sum::<usize>();
        let padding = width.saturating_sub(line_width + model.width());
        first.spans.push(Span::styled(
            " ".repeat(padding),
            Style::default().fg(MODEL_TEXT_COLOR).bg(BACKGROUND_COLOR),
        ));
        first.spans.push(Span::styled(
            model,
            Style::default().fg(MODEL_TEXT_COLOR).bg(BACKGROUND_COLOR),
        ));
    }

    (lines, links)
}

/// Use a prompt marker for shell calls without changing their stored tool name.
fn tool_header_text(entry: &Entry) -> String {
    if entry
        .tool
        .as_ref()
        .is_some_and(|tool| tool.function_name == "shell")
    {
        format!(
            ">{}",
            entry.text.strip_prefix("shell").unwrap_or(&entry.text)
        )
    } else {
        entry.text.clone()
    }
}

/// Whether tool output carries CommonMark structure instead of plain logs.
fn tool_output_is_markdown(output: &str) -> bool {
    let block_markdown = output.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("# ")
            || trimmed.starts_with("## ")
            || trimmed.starts_with("### ")
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("> ")
            || trimmed.starts_with("```")
            || trimmed.starts_with("| ")
            || trimmed
                .split_once(". ")
                .is_some_and(|(number, _)| number.chars().all(|c| c.is_ascii_digit()))
    });
    if block_markdown {
        return true;
    }

    // A short prose result can consist only of inline markdown. Do not apply
    // this test to longer terminal output: CLI help commonly contains
    // backticks and angle-bracket placeholders that are literal syntax, not a
    // markdown document.
    output.lines().count() <= 3
        && (output.contains("**")
            || output.contains("__")
            || output.contains("~~")
            || output.contains('`')
            || (output.contains('[') && output.contains("](")))
}

/// Grok's active-tool rail: a brightness wave that travels down the rows.
fn tool_rail_wave_color(tick: u64, row: u16) -> Color {
    use std::f32::consts::PI;

    let phase = (row as f32 / TOOL_RAIL_WAVE_ROWS) * 2.0 * PI;
    let brightness = (tick as f32 * TOOL_RAIL_WAVE_SPEED + phase).sin().powi(2);
    blend_rgb(BACKGROUND_COLOR, TEXT_COLOR, brightness)
}

/// Fade a completed tool's marker and rail into the resting secondary amber.
fn tool_settle_color(entry: &Entry, tick: u64, motion_enabled: bool) -> Color {
    if !motion_enabled {
        return DIM_TEXT_COLOR;
    }
    let age = entry
        .tool_settled_at
        .map(|started| tick.saturating_sub(started))
        .unwrap_or(TOOL_SETTLE_FRAMES);
    let remaining = 1.0 - (age.min(TOOL_SETTLE_FRAMES) as f32 / TOOL_SETTLE_FRAMES as f32);
    let eased = remaining * remaining;
    blend_rgb(DIM_TEXT_COLOR, TEXT_COLOR, eased)
}

/// Move a long output preview from its first rows to its final rows once.
fn tool_output_scroll_start(final_start: usize, age: u64) -> usize {
    if age >= TOOL_OUTPUT_SCROLL_FRAMES {
        return final_start;
    }

    let progress = age as f64 / TOOL_OUTPUT_SCROLL_FRAMES as f64;
    let eased = 1.0 - (1.0 - progress).powi(5);
    (final_start as f64 * eased).round() as usize
}

fn blend_rgb(from: Color, to: Color, amount: f32) -> Color {
    let (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) = (from, to) else {
        return to;
    };
    let channel = |start: u8, end: u8| {
        (start as f32 + (end as f32 - start as f32) * amount.clamp(0.0, 1.0)).round() as u8
    };
    Color::Rgb(channel(fr, tr), channel(fg, tg), channel(fb, tb))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact shape reported in issue #181: a reasoning summary whose
    /// markers and list layout arrived intact but rendered as one folded
    /// paragraph of literal `**bold**` text.
    #[test]
    fn reasoning_renders_markdown_structure_and_keeps_line_breaks() {
        let mut entry = Entry::new(
            Role::Reasoning,
            "The user wants me to assess the options:\n\n1. **Option A: Hub daemon** — one socket\n2. **Option B: Peer-to-peer** — N sockets\n\nThen assess `swarm` naming.",
        );
        entry.finish_text();

        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let text = |row: usize| -> String {
            lines[row]
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect()
        };
        let all = (0..lines.len()).map(text).collect::<Vec<_>>().join("\n");

        // Inline markers are consumed by the engine, not printed.
        assert!(!all.contains("**"), "raw bold markers survived: {all}");
        assert!(!all.contains("`"), "raw code ticks survived: {all}");

        // "Option A" is bold: it survives with the BOLD modifier.
        let bold_somewhere = lines.iter().any(|line| {
            line.spans.iter().any(|s| {
                s.content.contains("Option A") && s.style.add_modifier.contains(Modifier::BOLD)
            })
        });
        assert!(bold_somewhere, "bold never rendered: {all}");

        // The two list items sit on their own rows, in order, with the
        // soft breaks between them kept.
        let item_a = lines
            .iter()
            .position(|line| line.spans.iter().any(|s| s.content.contains("Option A")))
            .expect("option A row missing");
        let item_b = lines
            .iter()
            .position(|line| line.spans.iter().any(|s| s.content.contains("Option B")))
            .expect("option B row missing");
        assert!(item_b > item_a, "list items folded into one row: {all}");
    }

    /// Line breaks are respected even without blank lines: a reasoning
    /// summary that never uses list syntax still gets one row per line.
    #[test]
    fn reasoning_soft_breaks_render_as_line_breaks_not_spaces() {
        let mut entry = Entry::new(
            Role::Reasoning,
            "First thought,\nsecond thought,\nthird thought.",
        );
        entry.finish_text();

        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect();
        let joined = texts.join("\n");
        assert!(texts.len() >= 3, "breaks collapsed: {joined}");
        assert!(joined.contains("First thought,"));
        assert!(joined.contains("second thought,"));
    }

    /// The reasoning bullet survives the markdown arm: the block still opens
    /// with `⏺` and the dim colour the role has always carried.
    #[test]
    fn reasoning_keeps_its_bullet_and_dim_colour() {
        let mut entry = Entry::new(Role::Reasoning, "Thinking about it.\n\nSecond line.");
        entry.finish_text();

        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let first = &lines[0];
        assert_eq!(first.spans[0].content.as_ref(), "⏺   ");
        assert!(
            first
                .spans
                .iter()
                .any(|s| s.style.fg == Some(DIM_TEXT_COLOR)),
            "reasoning no longer dim: {:?}",
            first.spans
        );
    }

    /// Assistant prose keeps CommonMark folding: this regression is what the
    /// reasoning switch must not leak into.
    #[test]
    fn assistant_soft_breaks_still_collapse_to_spaces() {
        let mut entry = Entry::new(Role::Assistant, "Line one,\nline two,");
        entry.finish_text();

        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let joined = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("Line one, line two,"),
            "assistant folding changed: {joined}"
        );
    }

    /// The status row's right field leads with the build version, then the
    /// lane, then the model that answered — so any frame, and any screenshot
    /// of one, says which binary produced the session.
    #[test]
    fn lane_field_leads_with_the_version() {
        let mut ui = CoderUi::new();
        ui.lane = "Coder Flash".to_string();
        ui.model = "gemini-3.7-flash".to_string();

        let field = ui.lane_field();
        assert_eq!(field, "v0.0.0-dev · Coder Flash · gemini-3.7-flash");

        // Before anything has answered, the version still leads the lane.
        ui.model = String::new();
        assert_eq!(ui.lane_field(), "v0.0.0-dev · Coder Flash");
    }

    /// Narrow columns drop the version and lane before the model: the
    /// answering model is the load-bearing half and the version is one of the
    /// niceties, and a bare model id is still true.
    #[test]
    fn lane_field_narrows_to_the_model_before_dropping_anything() {
        let mut ui = CoderUi::new();
        ui.lane = "Coder Flash".to_string();
        ui.model = "gemini-3.7-flash".to_string();

        let narrow = ui.lane_field_within("v0.0.0-dev · gemini-3.7-flash".len() as u16);
        assert_eq!(narrow, "v0.0.0-dev · gemini-3.7-flash");

        let bare = ui.lane_field_within(ui.model.chars().count() as u16);
        assert_eq!(bare, "gemini-3.7-flash");

        // Nothing fits whole: nothing at all renders, never a truncation.
        assert_eq!(ui.lane_field_within(3), "");
    }

    /// A session with no lane recorded still shows the version it is running.
    #[test]
    fn lane_field_shows_the_version_when_no_lane_is_recorded() {
        let ui = CoderUi::new();
        assert_eq!(ui.lane_field(), "v0.0.0-dev");
    }
}
