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
//! - `oa-tool-step`: a tool step or the model's thinking as one compact row
//!   (`button[data-oa-tool-step]`, `aria-expanded`) that opens its detail
//!   (`static/components/tool-step.js`).
//! - `oa-thread-notice`: a quiet line ("Showing messages 1-24 of 96").
//! - `oa-thread-status`: the live status line under the last message.
//! - `oa-thread-error`: a failure line.
//! - `oa-home-stage`: the new chat's middle, where its link cards sit
//!   until a conversation starts.
//! - `oa-composer-feedback`: a status line under a docked composer.

use maud::{Markup, Render, html};

use crate::content::CodeBlock;

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

/// One turn in a thread: `article.oa-message[data-role]`.
#[derive(Clone, Debug)]
pub struct Message {
    role: MessageRole,
    author: String,
    id: Option<String>,
    body: Markup,
    quiet: bool,
}

impl Message {
    /// A user turn; `text` is escaped and keeps its line breaks.
    #[must_use]
    pub fn user(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        Self {
            role: MessageRole::User,
            author: "You".to_owned(),
            id: None,
            quiet: false,
            body: html! { div class="oa-message-bubble" { (text) } },
        }
    }

    /// An assistant turn showing `content` (for example
    /// `MarkdownRoot::new(PreEscaped(rendered))`).
    #[must_use]
    pub fn assistant(content: impl Render) -> Self {
        Self {
            role: MessageRole::Assistant,
            author: "Assistant".to_owned(),
            id: None,
            quiet: false,
            body: html! { div class="oa-message-content" { (content) } },
        }
    }

    /// A status turn; `text` is escaped and keeps its line breaks.
    #[must_use]
    pub fn status(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        Self {
            role: MessageRole::Status,
            author: "Status".to_owned(),
            id: None,
            quiet: false,
            body: html! { p class="oa-message-status" { (text) } },
        }
    }

    /// The model's thinking: one "Thinking" row that opens to its words,
    /// dim and slanted. `detail_id` names the opened part, unique on the
    /// page. Its heading is for screen readers only.
    #[must_use]
    pub fn thinking(text: impl AsRef<str>, detail_id: &str) -> Self {
        let text = text.as_ref();
        Self {
            role: MessageRole::Thinking,
            author: "Thinking".to_owned(),
            id: None,
            quiet: true,
            body: html! {
                div.oa-tool-step data-kind="thinking" {
                    button.oa-tool-step__row type="button" aria-expanded="false"
                        aria-controls=(detail_id) data-oa-tool-step {
                        span.oa-tool-step__name { "Thinking" }
                        span.oa-tool-step__chevron aria-hidden="true" {}
                    }
                    div.oa-tool-step__detail id=(detail_id) hidden {
                        p.oa-message-thinking { (text) }
                    }
                }
            },
        }
    }

    /// One tool step ([`Step`]): a compact row with its mark, name, and
    /// line, opening to its whole input and output when it has them.
    #[must_use]
    pub fn step(step: &Step) -> Self {
        Self {
            role: MessageRole::Step,
            author: "Step".to_owned(),
            id: None,
            quiet: true,
            body: step.render(),
        }
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
        html! {
            article class="oa-message" data-role=(self.role.as_str()) id=[self.id.as_deref()] {
                h2 class=(author_class) { (self.author) }
                (self.body)
            }
        }
    }
}

/// How a tool step went, as its mark shows it: running (a spinner), done
/// (a green dot), or failed (a red cross), the colors Coder's terminal uses.
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

    const fn words(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// One tool step as a compact row: a mark for how it went, the tool's
/// name, and its one line (cut with an ellipsis), opening to its whole
/// input and output in code blocks with a copy action. A step with neither
/// is a plain row that doesn't open.
#[derive(Clone, Debug)]
pub struct Step {
    /// The tool's name; empty for a row that is only a line.
    pub name: String,
    /// The line after the name.
    pub summary: String,
    pub state: StepState,
    pub input: String,
    pub output: String,
    /// The opened part's id, unique on the page.
    pub detail_id: String,
}

impl Render for Step {
    fn render(&self) -> Markup {
        let opens = !self.input.trim().is_empty() || !self.output.trim().is_empty();
        let mark = html! {
            span.oa-tool-step__mark aria-hidden="true" {
                @match self.state {
                    StepState::Running => {}
                    StepState::Done => { "●" }
                    StepState::Failed => { "×" }
                }
            }
        };
        let words = html! {
            (mark)
            @if !self.name.is_empty() { span.oa-tool-step__name { (self.name) } }
            @if !self.summary.is_empty() { span.oa-tool-step__summary { (self.summary) } }
            span.oa-visually-hidden { ", " (self.state.words()) }
        };
        html! {
            div.oa-tool-step data-state=(self.state.as_str()) {
                @if opens {
                    button.oa-tool-step__row type="button" aria-expanded="false"
                        aria-controls=(self.detail_id) data-oa-tool-step {
                        (words)
                        span.oa-tool-step__chevron aria-hidden="true" {}
                    }
                    div.oa-tool-step__detail id=(self.detail_id) hidden {
                        @if !self.input.trim().is_empty() {
                            p.oa-tool-step__label { "Input" }
                            (CodeBlock::new(self.input.clone()).wrap(true))
                        }
                        @if !self.output.trim().is_empty() {
                            p.oa-tool-step__label { "Output" }
                            (CodeBlock::new(self.output.clone()).wrap(true))
                        }
                    }
                } @else {
                    div.oa-tool-step__row { (words) }
                }
            }
        }
    }
}
