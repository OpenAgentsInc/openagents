//! The conversation thread: messages in a centered column that scrolls
//! between the header and the docked composer.
//!
//! As in the ChatGPT UI reference's conversation view, the user's turns sit
//! in a bubble on the trailing side and the assistant's answer runs the full
//! column width as Markdown. The author heading stays for screen readers on
//! both; status turns (failures, unknown outcomes) show it.
//!
//! Layout classes the page applies around [`Message`]s (styled in
//! `static/components/thread.css`):
//!
//! - `oa-thread`: the scrolling region (`flex: 1`, `overflow-y: auto`).
//! - `oa-thread-column`: the centered column, `--thread-content-max-width`.
//! - `oa-thread-view`: the positioned box around a thread, which holds its
//!   [`super::ScrollToBottom`] button. The title lives in the header row's
//!   [`super::Breadcrumb`], not in the thread.
//! - `oa-message-thinking`: the model's thinking, dim and slanted.
//! - `oa-tool-step`: a tool step as one trace line (`content::TraceStep`)
//!   that opens to its input and output; a run of steps and thinking joins
//!   into one trace (`static/components/thread.css`).
//! - `oa-thread-notice`: a quiet line ("Showing messages 1-24 of 96").
//! - `oa-thread-status`: the live status line under the last message.
//! - `oa-thread-error`: a failure line.
//! - `oa-home-stage`: the new chat's middle, where its link cards sit
//!   until a conversation starts.
//! - `oa-composer-feedback`: a status line under a docked composer.

use maud::{Markup, Render, html};

use crate::content::{CodeBlock, CodeSize, Trace, TraceKind, TraceStatus, TraceStep};
use crate::icons::{Icon, IconSize};

/// Who wrote a [`Message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    /// The person: a bubble with the text as written (whitespace kept).
    User,
    /// The assistant: trusted rendered markup, usually a `MarkdownRoot`.
    Assistant,
    /// A system status turn: plain text with a visible author line.
    Status,
    /// The model's thinking: a "Thinking" row that opens to its words.
    Thinking,
    /// One tool step: a compact row that opens to what it ran.
    Step,
}

impl MessageRole {
    /// The `data-role` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Status => "status",
            Self::Thinking => "thinking",
            Self::Step => "step",
        }
    }
}

/// One turn in a thread: `article.oa-message[data-role]`, drawn with the
/// transcript components (#11621): the person's words in a
/// [`crate::content::Bubble`]-style bubble, an answer as a
/// [`crate::content::Turn`]-style turn with its author in a header, and a
/// tool step or the model's thinking as one [`TraceStep`] line, so a run
/// of steps reads as one trace.
#[derive(Clone, Debug)]
pub struct Message {
    role: MessageRole,
    author: String,
    id: Option<String>,
    body: Markup,
    quiet: bool,
    continued: bool,
    actions: Vec<Markup>,
}

impl Message {
    fn with(role: MessageRole, author: &str, body: Markup, quiet: bool) -> Self {
        Self {
            role,
            author: author.to_owned(),
            id: None,
            body,
            quiet,
            continued: false,
            actions: Vec::new(),
        }
    }

    /// A user turn; `text` is escaped and keeps its line breaks.
    #[must_use]
    pub fn user(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        Self::with(
            MessageRole::User,
            "You",
            html! { div class="oa-message-bubble oa-bubble" { (text) } },
            false,
        )
    }

    /// An assistant turn showing `content` (for example
    /// `MarkdownRoot::new(PreEscaped(rendered))`).
    #[must_use]
    pub fn assistant(content: impl Render) -> Self {
        Self::with(
            MessageRole::Assistant,
            "Assistant",
            html! { div class="oa-message-content oa-turn__body" { (content) } },
            false,
        )
    }

    /// A status turn; `text` is escaped and keeps its line breaks.
    #[must_use]
    pub fn status(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        Self::with(
            MessageRole::Status,
            "Status",
            html! { p class="oa-message-status" { (text) } },
            false,
        )
    }

    /// The model's thinking: one "Thinking" line that opens to its words,
    /// dim and slanted. `detail_id` names the line, unique on the page. Its
    /// heading is for screen readers only.
    #[must_use]
    pub fn thinking(text: impl AsRef<str>, detail_id: &str) -> Self {
        let text = text.as_ref();
        let first = text
            .lines()
            .map(|l| l.trim().trim_matches('*').trim())
            .find(|l| !l.is_empty())
            .unwrap_or_default();
        let line = TraceStep::new(TraceKind::Reasoning, "Thinking")
            .description(first.chars().take(140).collect::<String>())
            .body(html! { p.oa-message-thinking { (text) } })
            .id(detail_id.to_owned());
        Self::with(
            MessageRole::Thinking,
            "Thinking",
            Trace::new([line]).render(),
            true,
        )
    }

    /// One tool step ([`Step`]): a trace line with its status, name, and
    /// line, opening to its whole input and output when it has them.
    #[must_use]
    pub fn step(step: &Step) -> Self {
        Self::with(MessageRole::Step, "Step", step.render(), true)
    }

    /// The author heading ("You", the assistant's name, "Status").
    #[must_use]
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    /// A status turn whose author heading stays for screen readers only, for
    /// threads where status lines are progress notes ("Working…") rather
    /// than failures.
    #[must_use]
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    /// An answer that continues the turn above it (after its steps or an
    /// earlier answer): its author stays for screen readers only, so one
    /// turn shows one header.
    #[must_use]
    pub fn continued(mut self, continued: bool) -> Self {
        self.continued = continued;
        self
    }

    /// An action under an answer, such as a copy button.
    #[must_use]
    pub fn action(mut self, action: impl Render) -> Self {
        self.actions.push(action.render());
        self
    }

    /// An action under an answer when `when` holds; `action` is only built
    /// then.
    #[must_use]
    pub fn action_if<R: Render>(self, when: bool, action: impl FnOnce() -> R) -> Self {
        if when { self.action(action()) } else { self }
    }

    /// The element id, for links to one message.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// The role.
    #[must_use]
    pub fn role(&self) -> MessageRole {
        self.role
    }
}

impl Render for Message {
    fn render(&self) -> Markup {
        let author_class = if self.role == MessageRole::Status && !self.quiet {
            "oa-message-author"
        } else {
            "oa-message-author oa-visually-hidden"
        };
        let from = match self.role {
            MessageRole::User => Some("person"),
            MessageRole::Assistant => Some("agent"),
            _ => None,
        };
        let header = self.role == MessageRole::Assistant && !self.continued;
        html! {
            article class=(if from.is_some() { "oa-message oa-turn" } else { "oa-message" })
                data-role=(self.role.as_str()) data-from=[from] id=[self.id.as_deref()] {
                @if header {
                    header.oa-turn__header {
                        span.oa-turn__avatar aria-hidden="true" { (Icon::Agent.size(IconSize::Sm)) }
                        h2 class="oa-message-author oa-turn__author" { (self.author) }
                    }
                } @else {
                    h2 class=(author_class) { (self.author) }
                }
                (self.body)
                @if !self.actions.is_empty() {
                    footer.oa-turn__footer {
                        div.oa-turn__actions role="toolbar" aria-label="Message actions" {
                            @for action in &self.actions { (action) }
                        }
                    }
                }
            }
        }
    }
}

/// How a tool step went: running (its title shimmers), done, or failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Running,
    Done,
    Failed,
}

impl StepState {
    /// The `data-state` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    const fn status(self) -> TraceStatus {
        match self {
            Self::Running => TraceStatus::Running,
            Self::Done => TraceStatus::Done,
            Self::Failed => TraceStatus::Error,
        }
    }
}

/// One tool step as a trace line: its status, the tool's name, and its
/// one line, opening to its whole input and output in small code blocks
/// with a copy action (an edit's input as a diff). A step with neither is
/// a plain line that doesn't open.
#[derive(Clone, Debug)]
pub struct Step {
    /// The tool's name; empty for a row that is only a line.
    pub name: String,
    /// The line after the name.
    pub summary: String,
    pub state: StepState,
    pub input: String,
    pub output: String,
    /// The line's id, unique on the page.
    pub detail_id: String,
}

/// The kind of trace line a tool's name reads as.
fn step_kind(name: &str) -> TraceKind {
    let lower = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["bash", "run", "shell", "exec", "terminal", "command"]) {
        TraceKind::Command
    } else if has(&["edit", "write", "patch", "replace"]) {
        TraceKind::Edit
    } else if has(&["read", "view", "cat"]) {
        TraceKind::Read
    } else if has(&["grep", "glob", "search", "find"]) {
        TraceKind::Search
    } else if has(&["fetch", "browse", "web"]) {
        TraceKind::Source
    } else if has(&["todo", "plan"]) {
        TraceKind::Plan
    } else if has(&["task", "agent", "delegate"]) {
        TraceKind::Agent
    } else {
        TraceKind::Tool
    }
}

/// An edit's input as a diff, when it names what it replaced.
fn edit_diff(input: &str) -> Option<String> {
    let text = |v: &str| {
        let key = format!("\"{v}\"");
        let at = input.find(&key)?;
        let rest = input[at + key.len()..]
            .trim_start()
            .strip_prefix(':')?
            .trim_start();
        let mut out = String::new();
        let mut chars = rest.strip_prefix('"')?.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    other => out.push(other),
                },
                other => out.push(other),
            }
        }
        None
    };
    let new = text("new_string").or_else(|| text("new_str"))?;
    let old = text("old_string")
        .or_else(|| text("old_str"))
        .unwrap_or_default();
    Some(crate::content::replacement_diff(&old, &new).0)
}

impl Render for Step {
    fn render(&self) -> Markup {
        let title = if self.name.is_empty() {
            self.summary.clone()
        } else {
            self.name.clone()
        };
        let mut line = TraceStep::new(step_kind(&self.name), title)
            .status(self.state.status())
            .id(self.detail_id.clone());
        if !self.name.is_empty() {
            line = line.detail(self.summary.clone());
        }
        let input = self.input.trim();
        let output = self.output.trim();
        if !input.is_empty() || !output.is_empty() {
            let diff = (step_kind(&self.name) == TraceKind::Edit)
                .then(|| edit_diff(input))
                .flatten();
            line = line.body(html! {
                @if let Some(diff) = &diff {
                    (CodeBlock::new(diff.clone()).title("Change").language("diff").diff(true).size(CodeSize::Sm).wrap_toggle(true))
                } @else if !input.is_empty() {
                    (CodeBlock::new(self.input.clone()).title("Input").size(CodeSize::Sm).wrap_toggle(true))
                }
                @if !output.is_empty() {
                    (CodeBlock::new(self.output.clone()).title("Output").size(CodeSize::Sm).wrap_toggle(true))
                }
            });
        }
        html! { div.oa-tool-step data-state=(self.state.as_str()) { (Trace::new([line])) } }
    }
}
