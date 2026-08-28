//! Full-screen coder TUI layout matching packages/openagents-cli/src/coder-ui.ts

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph},
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
/// Live subagent lines kept visible in a delegate box. Older lines collapse
/// into one `+N earlier` counter, counted rather than stored as text.
pub const MAX_SUBAGENT_LINES: usize = 6;
const MAX_VISIBLE_COMMAND_SUGGESTIONS: usize = 8;
/// Idle "New in v0.1.1" card. Seven lines is the ceiling so the pair of
/// boxes still fits an ordinary terminal.
const WELCOME_WHAT_IS_NEW: &[&str] = &[
    "Improved subagent delegation",
    "Added streaming to thinking",
    "Grok is a first-class delegate",
    "Timing on each message",
    "ATIF export keeps subagent streams",
    "ATIF export keeps the swarm inbox",
    "Flash routes simple requests to Gemini 3.7 Flash",
];

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
    /// Whole seconds the turn that produced this entry ran, stamped when the
    /// turn settles (#216). `None` for anything without a measured turn: a
    /// restored entry whose summary recorded no duration, a fresh entry not
    /// yet answered.
    pub duration_seconds: Option<u64>,
    /// Tool output text, rendered as a box of up to four lines.
    pub output: Option<String>,
    /// Live lines from a delegated agent, clipped to [`MAX_SUBAGENT_LINES`].
    /// Presentation-only: the ATIF tool record keeps the final output.
    pub subagent_lines: Vec<String>,
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

/// Whole seconds, the way a person says a duration: `9s`, `1m30s`, `1h2m3s`.
///
/// One formatter for both the live stopwatch and the settled figure printed
/// beside a model name, so the two never disagree about what a turn's length
/// is called.
pub fn format_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}h{minutes}m{seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m{seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// The loading row's leading span: the spinner, then any waiting text,
/// single-spaced — `⠹ Waiting for first token` or, on an ordinary turn,
/// just `⠹`. The running count is appended by the caller in its own dimmer
/// span. Extracted because the first cut of the composition swallowed the
/// count on the exact row it existed for: a match over two emptiness flags
/// let the `(true, _)` arm win whenever `waiting` happened to be unset,
/// which is to say on every normal turn (#216).
pub fn loading_prefix(spinner: char, waiting: &str) -> String {
    let mut text = spinner.to_string();
    if !waiting.is_empty() {
        text.push(' ');
        text.push_str(waiting);
    }
    text
}

impl Entry {
    /// An entry with no tool output, stamped with the current time.
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            turn_id: None,
            model: None,
            duration_seconds: None,
            output: None,
            subagent_lines: Vec::new(),
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
            duration_seconds: None,
            output: Some(String::new()),
            subagent_lines: Vec::new(),
            tool: None,
            at: now_ms(),
            output_scroll_started: None,
            tool_settled_at: None,
            md: None,
            output_md: None,
            output_md_source: String::new(),
        }
    }

    /// Append one live line from a delegated agent.
    pub fn push_subagent_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        let line = line.trim_end();
        if line.is_empty() {
            return;
        }
        if line.starts_with("· ") {
            self.subagent_lines.push(line.to_string());
        } else {
            self.subagent_lines.push(format!("· {line}"));
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
        let raw = self.output.as_deref().unwrap_or("");
        let rendered = crate::delegate_result::displayed_delegate_output(raw);
        if self.output_md.is_none() || self.output_md_source != rendered {
            let mut content = MarkdownContent::new();
            content.push(&rendered);
            content.finish();
            self.output_md = Some(Box::new(content));
            self.output_md_source.clear();
            self.output_md_source.push_str(&rendered);
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
    /// Epoch millis the running turn started at, set when `loading` turns on
    /// and cleared when the turn settles. Feeds the live stopwatch on the
    /// loading row and, at settle, the duration printed beside the answer's
    /// model name (#216). `None` whenever no turn is running.
    pub turn_started_at: Option<u64>,
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
    /// The session-level autopilot mode (spec: `docs/coder/autopilot.md`).
    ///
    /// A mode, not a lane: the status cell renders beside the lane field only
    /// while engaged, so a reader who never engages sees no new pixels on the
    /// row. Held here as display state; the command surface owns the state
    /// itself (`coder/autopilot.rs`) and the frame loop keeps the two in step.
    pub autopilot_engaged: bool,
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
    /// Optional gym pane, rendered from a frozen `openagents.gym.*` document.
    pub gym_panel: Option<crate::gym::views::GymPanel>,
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
    /// In-app text selection over the transcript, driven by the mouse and
    /// copied with Ctrl+Y. See [`crate::coder::selection`].
    pub selection: crate::coder::selection::SelectionState,
    /// A transient toast: its text and the tick it disappears on. Clipboard
    /// feedback lives here rather than in the transcript — a copy is an
    /// action's result, not something anyone said.
    pub toast: Option<(String, u64)>,
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let mut word = word;
        let mut word_width = word.chars().count();
        // One unbroken run — a path, a hash, a long note with no spaces —
        // longer than the line is split where it lands rather than pushed
        // past the edge whole. A word wider than the line used to be kept
        // whole here, and ratatui clipped whatever ran past, so the tail of
        // exactly the text the header existed to show disappeared.
        while word_width > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            let take = width.saturating_sub(current_width);
            let end = word
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(word.chars().count()))
                .take_while(|i| *i <= take)
                .last()
                .unwrap_or(0);
            let (head, rest) = word.split_at(end);
            current.push_str(head);
            current_width += head.chars().count();
            lines.push(std::mem::take(&mut current));
            current_width = 0;
            word = rest;
            word_width = word.chars().count();
        }
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
            turn_started_at: None,
            waiting: None,
            activity: "Idle".to_string(),
            tick: 0,
            motion_enabled: std::env::var_os("OPENAGENTS_REDUCED_MOTION").is_none(),
            identity: Identity::Anonymous,
            endpoint: String::new(),
            lane: String::new(),
            autopilot_engaged: false,
            thread: None,
            local_session_id: None,
            local_session_path: None,
            cloud_history: false,
            last_usage: TurnUsage::default(),
            total_usage: TurnUsage::default(),
            credit: crate::coder::credit::CreditField::Unread,
            gym_panel: None,
            billed: None,
            agents: Vec::new(),
            links: Vec::new(),
            cursor: None,
            selection: crate::coder::selection::SelectionState::default(),
            toast: None,
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

    /// Start the running turn's clock. Called when `loading` turns on; a
    /// turn already timed restarts its clock, which matches what the state
    /// machine means — a queued prompt that starts is a new turn.
    pub fn turn_started(&mut self) {
        self.turn_started_at = Some(now_ms());
    }

    /// Stop the clock and stamp the duration, whole seconds, onto the turn's
    /// last assistant entry, beside the model that produced it. A turn with
    /// no assistant entry (a refused start, an instant failure) stamps
    /// nothing: there is nothing the figure would belong to.
    ///
    /// Whole seconds because that is the honest precision of this clock:
    /// the same wall time a reader would quote.
    pub fn turn_settled(&mut self) {
        let Some(started) = self.turn_started_at.take() else {
            return;
        };
        let seconds = now_ms().saturating_sub(started) / 1000;
        if let Some(entry) = self
            .entries
            .iter_mut()
            .rfind(|entry| entry.role == Role::Assistant)
        {
            entry.duration_seconds = Some(seconds);
        }
    }

    /// The live stopwatch text for the loading row: whole seconds so far on
    /// the running turn, formatted the way the settled duration will read.
    /// Empty when no turn is running — the row then carries the spinner
    /// alone, exactly as it did before this existed.
    pub fn stopwatch_text(&self) -> String {
        self.turn_started_at
            .map(|started| format_duration(now_ms().saturating_sub(started) / 1000))
            .unwrap_or_default()
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
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, until)| self.tick >= *until)
        {
            self.toast = None;
        }
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

        let body = if self.gym_panel.is_some() {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(42)])
                .split(main[0])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0)])
                .split(main[0])
        };
        let transcript_area = body[0];
        if self.gym_panel.is_some() {
            self.render_gym_panel(frame, body[1], style);
        }
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
            let stopwatch = self.stopwatch_text();
            // While nothing more specific is being waited on, the row says
            // what it is doing: `⠹ working (9s)`. A waiting message — a
            // cancel, a first-token wait, a retry — outranks it, and the
            // timer rides beside whichever is showing.
            //
            // The count reads at the 50% opacity DIM_TEXT_COLOR gives: the
            // 25% of the model labels (#216) was too quiet to be found on a
            // glance at the screen, and a timer nobody can see is a timer
            // that does not exist.
            let status = match self.waiting.as_deref() {
                Some(waiting) if !waiting.is_empty() => waiting,
                _ => "working",
            };
            let mut spans = Vec::with_capacity(2);
            spans.push(Span::styled(loading_prefix(spinner, status), style));
            if !stopwatch.is_empty() {
                spans.push(Span::styled(
                    format!(" ({stopwatch})"),
                    style.fg(DIM_TEXT_COLOR),
                ));
            }
            all_lines.push(Line::from(spans));
        }

        let total = all_lines.len() as u16;
        self.transcript_height = transcript_area.height;
        self.scroll_max = total.saturating_sub(transcript_area.height);
        let start = self.effective_scroll(transcript_area.height, total);

        self.links = crate::coder::osc8::place(&links, transcript_area, start as usize);

        // Record what each visible row says, in screen coordinates, so the
        // next mouse event can hit-test a selection and this frame can paint
        // its highlight. Row `start + i` renders at `area.y + i` — the same
        // arithmetic the Paragraph scroll applies, and the links above use.
        {
            let mut visible_rows = Vec::new();
            for index in 0..transcript_area.height {
                let Some(line) = all_lines.get(start as usize + index as usize) else {
                    break;
                };
                let plain: String = line
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect();
                // Trailing blanks are layout, not content: they must not
                // extend a row selection or answer a hit test.
                let trimmed = plain.trim_end();
                if trimmed.is_empty() {
                    continue;
                }
                visible_rows.push(crate::coder::selection::ScreenRow {
                    screen_y: transcript_area.y + index,
                    screen_x: transcript_area.x,
                    plain: trimmed.to_string(),
                });
            }
            self.selection.observe_rows(visible_rows);
        }

        let transcript = Paragraph::new(Text::from(all_lines))
            .scroll((start, 0))
            .style(style);
        frame.render_widget(transcript, transcript_area);

        // Paint the selection over the rendered cells. The transcript is
        // already laid out, so this is a restyle of exactly the cells the
        // selection covers — one pass over the selected rows' columns.
        if let Some(selection) = self.selection.selection().cloned() {
            let rows = self.selection.rows().to_vec();
            let highlight = Style::default().fg(BACKGROUND_COLOR).bg(TEXT_COLOR);
            for selected in selection.selected_rows(&rows) {
                let Some(row) = rows.get(selected.row_index) else {
                    continue;
                };
                let char_count = row.plain.chars().count();
                if selected.char_start >= selected.char_end {
                    continue;
                }
                let mut column = row.screen_x;
                let mut start_column = None;
                let mut end_column = row.screen_x;
                for (char_index, ch) in row.plain.char_indices() {
                    if char_index == selected.char_start {
                        start_column = Some(column);
                    }
                    if char_index == selected.char_end {
                        break;
                    }
                    column += UnicodeWidthStr::width(ch.to_string().as_str()) as u16;
                }
                if selected.char_end >= char_count {
                    end_column = column;
                }
                let Some(start_column) = start_column else {
                    continue;
                };
                for x in start_column..end_column.max(start_column + 1) {
                    frame.buffer_mut()[(x, row.screen_y)].set_style(highlight);
                }
            }
        }

        let conversation_started = self.entries.iter().any(|entry| {
            matches!(
                entry.role,
                Role::You | Role::Assistant | Role::Tool | Role::Output
            )
        });
        let slash_open = !slash_command_suggestions(self.composer.text()).is_empty();
        if self.show_welcome && !conversation_started && !slash_open {
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
        let mut lane = self.lane_field_within(
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
        // The autopilot cell sits beside the lane field, mode after model:
        // the lane answers which model answers, this answers who steers
        // between turns (spec §1). Empty while off, so the row costs no
        // columns for a reader who never engages, and the same `fits_left`
        // eviction as the goal text applies — the credit and the effective
        // model still govern, and the mode cell yields before them.
        if self.autopilot_engaged {
            let field = crate::coder::autopilot::AutopilotState {
                engaged: true,
                directive: None,
                discipline: crate::coder::autopilot::IterationDiscipline::default(),
            }
            .status_cell();
            let separator = if lane.is_empty() { "" } else { " · " };
            let cell = format!("{field}{separator}{lane}");
            if cell.chars().count() as u16 <= status_area.width {
                lane = cell;
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

        // The toast, when one is showing, rides over the status row's left
        // edge — the one place every eye already checks for live state, and
        // the one place a transient message can sit without pushing the
        // transcript. Clipboard feedback is the only thing that posts one.
        if let Some((message, _)) = self.toast.clone() {
            let width = message.chars().count() as u16 + 2;
            let toast_area = Rect {
                width: width.min(status_area.width),
                ..status_area
            };
            let toast_style = Style::default()
                .fg(BACKGROUND_COLOR)
                .bg(TEXT_COLOR)
                .add_modifier(Modifier::BOLD);
            frame.render_widget(Paragraph::new(message).style(toast_style), toast_area);
        }
    }

    fn render_gym_panel(&self, frame: &mut Frame, area: Rect, style: Style) {
        let Some(panel) = &self.gym_panel else {
            return;
        };
        let mut lines: Vec<Line<'static>> = panel
            .render_lines()
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let line_style = if index == 0 {
                    style.add_modifier(Modifier::BOLD)
                } else if line.starts_with("    confounder:")
                    || line.starts_with("  lanes vs")
                    || line.starts_with("  trend ")
                    || line.starts_with("  no comparable")
                {
                    style.fg(DIM_TEXT_COLOR)
                } else {
                    style
                };
                Line::from(Span::styled(line, line_style))
            })
            .collect();
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "unknown",
                style.fg(DIM_TEXT_COLOR),
            )));
        }
        let panel = Paragraph::new(Text::from(lines)).style(style).block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(style)
                .style(style),
        );
        frame.render_widget(panel, area);
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
        // Six content rows, borders, and the padding: the autopilot line
        // made it seven, and the seventh is within the ceiling this card
        // guards (`WELCOME_WHAT_IS_NEW` yields before this box does).
        let welcome_height = 8.min(area.height);
        let news_height = (WELCOME_WHAT_IS_NEW.len() as u16).saturating_add(2);
        let gap = 1u16;
        let stack_height = welcome_height
            .saturating_add(gap)
            .saturating_add(news_height);
        let show_news = area.height >= stack_height;
        let used_height = if show_news {
            stack_height
        } else {
            welcome_height
        };
        let origin_y = area.y + area.height.saturating_sub(used_height) / 2;
        let welcome_area = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: origin_y,
            width,
            height: welcome_height,
        };
        // Borders take one column each side; the inner padding takes one more.
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
            // The autopilot line lives inside the card rather than beside the
            // version, so a reader learns the mode exists before the first
            // prompt (spec §8). One line, and the card's `8`-line height
            // already counts it — the news box below is what yields, exactly
            // as it does for a short terminal.
            Line::from(Span::styled(
                crate::coder::autopilot::AutopilotState {
                    engaged: self.autopilot_engaged,
                    directive: None,
                    discipline: crate::coder::autopilot::IterationDiscipline::default(),
                }
                .card_line(),
                label_style,
            )),
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
            .style(value_style)
            .padding(Padding::horizontal(1));
        frame.render_widget(Paragraph::new(content).block(block), welcome_area);

        if show_news {
            // Wrap the longest line: one pad column and one border on each side.
            let news_width = {
                let content = WELCOME_WHAT_IS_NEW
                    .iter()
                    .map(|line| UnicodeWidthStr::width(*line))
                    .max()
                    .unwrap_or(0);
                (content as u16).saturating_add(4).min(area.width).max(3)
            };
            let news_area = Rect {
                x: area.x + area.width.saturating_sub(news_width) / 2,
                y: welcome_area
                    .y
                    .saturating_add(welcome_height)
                    .saturating_add(gap),
                width: news_width,
                height: news_height,
            };
            let news = Text::from(
                WELCOME_WHAT_IS_NEW
                    .iter()
                    .map(|line| Line::from(Span::styled(*line, label_style)))
                    .collect::<Vec<_>>(),
            );
            let news_block = Block::default()
                .title(" New in v0.1.1 ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(value_style)
                .style(value_style)
                .padding(Padding::horizontal(1));
            frame.render_widget(Paragraph::new(news).block(news_block), news_area);
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
            let done = entry.tool.as_ref().is_some_and(|tool| tool.done);

            // Render markdown-shaped tool output with the shared engine. Keep
            // ordinary command output line-oriented: CommonMark treats a
            // single newline as a space, which would destroy logs and tables
            // that are plain text rather than markdown.
            let output_width = width.saturating_sub(2).max(1);
            let live_subagent = !done && !entry.subagent_lines.is_empty();
            let raw_output = entry.output.as_deref().unwrap_or("");
            let output = crate::delegate_result::displayed_delegate_output(raw_output);
            let (mut out_lines, out_links, window_rows) = if live_subagent {
                let hidden = entry
                    .subagent_lines
                    .len()
                    .saturating_sub(MAX_SUBAGENT_LINES);
                let mut rendered = Vec::new();
                if hidden > 0 {
                    rendered.push(Line::from(Span::raw(format!("+{hidden} earlier"))));
                }
                for line in entry.subagent_lines.iter().skip(hidden) {
                    rendered.push(Line::from(Span::raw(line.clone())));
                }
                let len = rendered.len();
                (rendered, Vec::new(), len)
            } else if tool_output_is_markdown(&output) {
                let md = entry.output_markdown_mut();
                let rendered = md.lines(output_width).to_vec();
                let links = md.links(output_width).to_vec();
                (rendered, links, TOOL_OUTPUT_ROWS)
            } else {
                (
                    output
                        .lines()
                        .map(|line| Line::from(Span::raw(line.to_string())))
                        .collect(),
                    Vec::new(),
                    TOOL_OUTPUT_ROWS,
                )
            };
            while out_lines
                .last()
                .is_some_and(|line| line.spans.iter().all(|span| span.content.trim().is_empty()))
            {
                out_lines.pop();
            }
            // An in-flight tool with no output yet still needs a rail row.
            // Without it the wave has nowhere to draw and the box looks idle
            // until ToolOutput arrives, which for the generic runtime is the
            // same instant as ToolDone (#256).
            if !done && out_lines.is_empty() {
                out_lines.push(Line::from(Span::raw(String::new())));
            }
            let final_start = out_lines.len().saturating_sub(window_rows);
            let start = if live_subagent || final_start == 0 || !motion_enabled {
                final_start
            } else {
                let started = *entry.output_scroll_started.get_or_insert(tick);
                tool_output_scroll_start(final_start, tick.saturating_sub(started))
            };
            let end = (start + window_rows).min(out_lines.len());
            let window = &out_lines[start..end];

            // One-line tool call header, flush left.
            let swarm = entry
                .tool
                .as_ref()
                .is_some_and(|tool| crate::swarm::is_swarm_tool(&tool.function_name));
            // Swarm traffic uses a distinct marker so it cannot be read as
            // user speech (`>`) or as the model's own words (unmarked amber).
            let marker = if swarm {
                "↔ "
            } else if done {
                "⏺ "
            } else {
                "○ "
            };
            let marker_style = if swarm {
                text_style.fg(DIM_TEXT_COLOR)
            } else if done {
                text_style.fg(tool_settle_color(entry, tick, motion_enabled))
            } else if !motion_enabled {
                text_style
            } else {
                text_style.fg(tool_rail_wave_color(tick, 0))
            };
            let header_body = width.saturating_sub(2);
            let header_text = tool_header_text(entry);
            let duration = tool_duration_seconds(entry);
            let duration_reserve = match duration {
                Some((true, seconds)) => format!(" ({})", format_duration(seconds)).width(),
                Some((false, seconds)) => format_duration(seconds).width() + 1,
                None => 0,
            };
            // The whole header, wrapped: a call whose title runs long used to
            // lose everything past the first line, which for `checkpoint`
            // meant the note itself — the one part a reader needs. A multi-line
            // header indents its continuation under the text, not the marker.
            // Wrapping leaves room for the live inline count or the settled
            // right-edge figure, the same way the assistant body leaves room
            // for the model label.
            let header_chunks = wrap_text(
                &header_text,
                header_body
                    .min(width)
                    .saturating_sub(2)
                    .saturating_sub(duration_reserve)
                    .max(1),
            );
            let mut header_iter = header_chunks.iter();
            let header = header_iter.next().cloned().unwrap_or_default();
            let header_style = if swarm {
                text_style.fg(DIM_TEXT_COLOR)
            } else {
                text_style
            };
            let mut header_spans = vec![
                Span::styled(marker, marker_style),
                Span::styled(header, header_style),
            ];
            match duration {
                Some((true, seconds)) => {
                    header_spans.push(Span::styled(
                        format!(" ({})", format_duration(seconds)),
                        text_style.fg(DIM_TEXT_COLOR),
                    ));
                }
                Some((false, seconds)) => {
                    let label = format_duration(seconds);
                    let line_width = header_spans
                        .iter()
                        .map(|span| span.content.width())
                        .sum::<usize>();
                    let padding = width.saturating_sub(line_width + label.width());
                    header_spans.push(Span::styled(
                        " ".repeat(padding),
                        Style::default().fg(MODEL_TEXT_COLOR).bg(BACKGROUND_COLOR),
                    ));
                    header_spans.push(Span::styled(
                        label,
                        Style::default().fg(MODEL_TEXT_COLOR).bg(BACKGROUND_COLOR),
                    ));
                }
                None => {}
            }
            lines.push(Line::from(header_spans));
            let continuation = format!("{} ", " ".repeat(marker.chars().count()));
            for chunk in header_iter {
                lines.push(Line::from(vec![
                    Span::styled(continuation.clone(), marker_style),
                    Span::styled(chunk.clone(), header_style),
                ]));
            }

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
        // The measured duration rides beside the model that produced the
        // answer (#216), separated the way the status row separates fields.
        let label = match entry.duration_seconds {
            Some(seconds) => format!("{model} · {}", format_duration(seconds)),
            None => model,
        };
        let line_width = first
            .spans
            .iter()
            .map(|span| span.content.width())
            .sum::<usize>();
        let padding = width.saturating_sub(line_width + label.width());
        first.spans.push(Span::styled(
            " ".repeat(padding),
            Style::default().fg(MODEL_TEXT_COLOR).bg(BACKGROUND_COLOR),
        ));
        first.spans.push(Span::styled(
            label,
            Style::default().fg(MODEL_TEXT_COLOR).bg(BACKGROUND_COLOR),
        ));
    }

    (lines, links)
}

/// Whole seconds this tool has been on screen, and whether it is still live.
///
/// In-flight calls count from [`Entry::at`]. Settled calls use the runtime's
/// `duration_ms`. The same formatter as the turn stopwatch, so a 9-second
/// read and a 9-second turn never disagree about what nine seconds is called.
fn tool_duration_seconds(entry: &Entry) -> Option<(bool, u64)> {
    let tool = entry.tool.as_ref()?;
    if tool.done {
        tool.duration_ms.map(|ms| (false, ms / 1000))
    } else {
        Some((true, now_ms().saturating_sub(entry.at) / 1000))
    }
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

#[cfg(test)]
mod swarm_style_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_swarm_header_names_the_sender_and_kind() {
        let mut entry = Entry::tool_call("swarm ← session-b [question]");
        entry.tool = Some(ToolCall {
            call_id: "swarm-inbox-1".to_string(),
            function_name: crate::swarm::INBOX_TOOL.to_string(),
            arguments: json!({"from": "session-b", "kind": "question", "count": 1}),
            output: None,
            error: None,
            done: true,
            duration_ms: Some(0),
        });
        assert_eq!(tool_header_text(&entry), "swarm ← session-b [question]");
        assert!(
            !entry.text.contains('>'),
            "a swarm header must not use the user-speech marker"
        );
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

    /// Seconds read the way a person says them: bare seconds under a minute,
    /// minutes-and-seconds from there on, hours only past the hour.
    #[test]
    fn durations_read_like_a_stopwatch() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(1), "1s");
        assert_eq!(format_duration(9), "9s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(60), "1m0s");
        assert_eq!(format_duration(90), "1m30s");
        assert_eq!(format_duration(3599), "59m59s");
        assert_eq!(format_duration(3723), "1h2m3s");
    }

    /// A settled turn stamps its whole-second duration on the turn's last
    /// assistant entry, and clears the clock so the spinner row stops
    /// counting.
    #[test]
    fn a_settled_turn_stamps_its_duration_on_the_answer() {
        let mut ui = CoderUi::new();
        ui.turn_started();
        ui.entries.push(Entry::new(Role::Assistant, "the answer"));
        ui.entries.last_mut().unwrap().model = Some("gemini-3.7-flash".to_string());
        // Simulate the elapsed time: started in the past.
        ui.turn_started_at = Some(now_ms() - 90_000);

        ui.turn_settled();

        assert_eq!(ui.turn_started_at, None);
        assert_eq!(ui.entries[0].duration_seconds, Some(90));
        // An already-settled clock stamps nothing further.
        ui.turn_settled();
        assert_eq!(ui.entries[0].duration_seconds, Some(90));
    }

    /// A turn with no assistant entry — refused at start, failed before any
    /// answer — settles the clock without stamping anything.
    #[test]
    fn a_turn_without_an_answer_settles_without_stamping() {
        let mut ui = CoderUi::new();
        ui.turn_started();
        ui.entries.push(Entry::new(Role::Notice, "boom"));

        ui.turn_settled();

        assert_eq!(ui.turn_started_at, None);
        assert!(ui.entries[0].duration_seconds.is_none());
    }

    /// While a turn runs, the stopwatch reports the elapsed time in the same
    /// words the settled figure uses; with no turn running it says nothing.
    #[test]
    fn the_stopwatch_counts_while_the_turn_runs_and_stops_when_it_settles() {
        let mut ui = CoderUi::new();
        assert_eq!(ui.stopwatch_text(), "");

        ui.turn_started();
        ui.turn_started_at = Some(now_ms() - 9_000);
        assert_eq!(ui.stopwatch_text(), "9s");

        ui.entries.push(Entry::new(Role::Assistant, "done"));
        ui.turn_settled();
        assert_eq!(ui.stopwatch_text(), "");
    }

    /// The rendered model label carries the duration when the entry has one,
    /// and the model alone when it does not.
    #[test]
    fn the_answer_line_shows_the_duration_beside_the_model() {
        let mut entry = Entry::new(Role::Assistant, "Answer text.");
        entry.model = Some("gemini-3.7-flash".to_string());
        entry.duration_seconds = Some(90);
        entry.finish_text();
        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            first.trim_end().ends_with("gemini-3.7-flash · 1m30s"),
            "duration missing beside the model: {first:?}"
        );

        // No measurement, no figure: the model alone, as restored entries
        // without one have always rendered.
        let mut entry = Entry::new(Role::Assistant, "Answer text.");
        entry.model = Some("gemini-3.7-flash".to_string());
        entry.finish_text();
        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            first.trim_end().ends_with("gemini-3.7-flash"),
            "model changed without a duration: {first:?}"
        );
        assert!(!first.contains("·"), "stray separator: {first:?}");
    }

    /// An in-flight tool counts beside its title; a settled one puts the
    /// figure on the right edge, the way the answer line does.
    #[test]
    fn a_tool_header_shows_a_live_count_then_a_settled_duration() {
        let mut entry = Entry::tool_call("read a.rs");
        entry.at = now_ms() - 9_000;
        entry.tool = Some(ToolCall {
            call_id: "c1".to_string(),
            function_name: "read".to_string(),
            arguments: serde_json::json!({"path": "a.rs"}),
            output: None,
            error: None,
            done: false,
            duration_ms: None,
        });
        let (lines, _) = render_entry(&mut entry, 80, 0, false);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            first.contains("read a.rs (9s)"),
            "live count missing inline: {first:?}"
        );

        entry.tool.as_mut().unwrap().done = true;
        entry.tool.as_mut().unwrap().duration_ms = Some(90_000);
        let (lines, _) = render_entry(&mut entry, 80, 0, false);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            first.trim_end().ends_with("1m30s"),
            "settled duration missing on the right: {first:?}"
        );
        assert!(
            !first.contains("(1m30s)"),
            "settled duration stayed inline: {first:?}"
        );
        assert!(first.contains("read a.rs"), "{first:?}");
    }

    /// The loading row carries the spinner, the waiting text, and the live
    /// count together — and the count survives when there is no waiting
    /// text, which is the ordinary turn. The first cut swallowed it there:
    /// the `(true, _)` arm of a flag match dropped the stopwatch whenever
    /// `waiting` was unset, which is every normal turn. This pins the
    /// composition the row actually renders (#216).
    #[test]
    fn the_loading_row_shows_the_spinner_and_the_running_count() {
        // The ordinary turn: no waiting text, count still visible.
        assert_eq!(loading_prefix('⠹', ""), "⠹");
        // A cancel or first-token wait rides in front of the count.
        assert_eq!(
            loading_prefix('⠹', "Canceling turn..."),
            "⠹ Canceling turn..."
        );
        // The UI-level path agrees: a running turn with `waiting` unset
        // still counts.
        let mut ui = CoderUi::new();
        ui.loading = true;
        ui.turn_started();
        ui.turn_started_at = Some(now_ms() - 9_000);
        assert_eq!(ui.stopwatch_text(), "9s");
    }

    /// A settled delegate box shows the trailer, not the JSON the parent
    /// model reads. The count survives even when a live box would clip.
    #[test]
    fn a_settled_delegate_box_renders_the_trailer_not_the_json() {
        let result = crate::delegate_result::DelegateAgentResult {
            status: crate::delegate_result::DelegateStatus::Done,
            agent: "coder-mini".to_string(),
            total_tool_uses: 14,
            duration_ms: 96_000,
            total_tokens: 41_234,
            model: Some("glm-5.3-flash".to_string()),
            session_id: Some("th_mini".to_string()),
            report: "the finding".to_string(),
            worktree: crate::delegate_result::WorktreeOutcome::Unused,
        };
        let mut entry = Entry::tool_call("delegate audit auth");
        entry.output = Some(result.to_json());
        entry.tool = Some(ToolCall {
            call_id: "d1".to_string(),
            function_name: "delegate".to_string(),
            arguments: serde_json::json!({"prompt": "audit auth"}),
            output: Some(result.to_json()),
            error: None,
            done: true,
            duration_ms: Some(96_000),
        });
        let (lines, _) = render_entry(&mut entry, 200, 0, false);
        let all = lines
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
            all.contains("Done · 14 tool uses"),
            "trailer missing from the box: {all}"
        );
        assert!(
            all.contains("the finding"),
            "report missing from the box: {all}"
        );
        assert!(
            !all.contains("\"total_tool_uses\""),
            "raw JSON leaked into the box: {all}"
        );
    }
}
