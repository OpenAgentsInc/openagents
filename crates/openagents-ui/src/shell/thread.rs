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
//! - `oa-thread-notice`: a quiet line ("Showing messages 1-24 of 96").
//! - `oa-thread-status`: the live status line under the last message.
//! - `oa-thread-error`: a failure line.
//! - `oa-home-stage`: the new chat's middle, where its link cards sit
//!   until a conversation starts.
//! - `oa-composer-feedback`: a status line under a docked composer.

use maud::{Markup, Render, html};

/// Who wrote a [`Message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    /// The person: a bubble with the text as written (whitespace kept).
    User,
    /// The assistant: trusted rendered markup, usually a `MarkdownRoot`.
    Assistant,
    /// A system status turn: plain text with a visible author line.
    Status,
}

impl MessageRole {
    /// The `data-role` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Status => "status",
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
