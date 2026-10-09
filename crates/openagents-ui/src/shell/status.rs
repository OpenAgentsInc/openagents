//! The small status a chat row shows at its end (see `docs/web/sidebar.md`).
//! Styles: the `oa-chat-status` rules in `static/components/shell.css`.

use maud::{Markup, Render, html};

/// What a chat is doing, in the words the sidebar shows. A chat with nothing
/// going on shows no status at all; callers pass one only when it helps.
///
/// ```
/// use maud::Render;
/// use openagents_ui::shell::{ChatList, ChatStatus, NavItem};
/// let html = ChatList::new()
///     .item(
///         NavItem::new("Fix the build", "/chat/1")
///             .detail("openagents/openagents · main")
///             .trailing(ChatStatus::Working),
///     )
///     .render()
///     .into_string();
/// assert!(html.contains(r#"data-status="working""#));
/// assert!(html.contains("openagents/openagents · main"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatStatus {
    /// An answer or a task is running now.
    Working,
    /// The chat needs the person: a question, an approval, or a sign-in.
    WaitingForYou,
    /// A task stopped for a usage limit and continues by itself; the text is
    /// the time it continues, as the caller formats it ("3:40 PM").
    PausedUntil(String),
    /// A long task finished and the person has not opened the chat since.
    Done,
    /// The last answer or task did not finish.
    Failed,
}

impl ChatStatus {
    /// The `data-status` value the stylesheet colors by.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::WaitingForYou => "waiting",
            Self::PausedUntil(_) => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    /// The words shown.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Working => "Working".to_owned(),
            Self::WaitingForYou => "Waiting for you".to_owned(),
            Self::PausedUntil(time) => format!("Paused until {time}"),
            Self::Done => "Done".to_owned(),
            Self::Failed => "Failed".to_owned(),
        }
    }
}

impl Render for ChatStatus {
    fn render(&self) -> Markup {
        html! {
            span class="oa-chat-status" data-status=(self.key()) {
                span class="oa-chat-status-dot" aria-hidden="true" {}
                span class="oa-chat-status-label" { (self.label()) }
            }
        }
    }
}
