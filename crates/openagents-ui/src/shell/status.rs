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
        // Working is a spinner alone, with its words for screen readers.
        if *self == Self::Working {
            return html! {
                span class="oa-chat-status" data-status=(self.key()) role="img" aria-label=(self.label()) {
                    (crate::actions::LoadingIndicator::new().decorative())
                }
            };
        }
        html! {
            span class="oa-chat-status" data-status=(self.key()) {
                span class="oa-chat-status-dot" aria-hidden="true" {}
                span class="oa-chat-status-label" { (self.label()) }
            }
        }
    }
}

/// Where a task started from a chat stands, in the words its row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// Running now: a spinner and "Working".
    Working,
    /// The task needs the person.
    WaitingForYou,
    /// Stopped for a usage limit; it continues by itself.
    Paused,
    Done,
    Failed,
    /// The person stopped it, or it ended without saying how.
    Stopped,
}

impl TaskStatus {
    /// The `data-status` value (colored like [`ChatStatus`]).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::WaitingForYou => "waiting",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    /// The words shown.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::WaitingForYou => "Waiting for you",
            Self::Paused => "Paused",
            Self::Done => "Done",
            Self::Failed => "Failed",
            Self::Stopped => "Stopped",
        }
    }
}

/// One task in a chat's thread as a compact row: an icon, what it is
/// ("Claude Code"), what it was asked, and its status at the end. With an
/// address the whole row links to the task's page.
/// Styles: the `oa-task-row` rules in `static/components/thread.css`.
///
/// ```
/// use maud::Render;
/// use openagents_ui::shell::{TaskRow, TaskStatus};
/// let html = TaskRow::new("Claude Code", TaskStatus::Working)
///     .detail("Fix the login redirect")
///     .href("/environments/e/runs/r")
///     .render()
///     .into_string();
/// assert!(html.starts_with(r#"<a class="oa-task-row""#));
/// assert!(html.contains("Working"));
/// ```
#[derive(Clone, Debug)]
pub struct TaskRow {
    title: String,
    detail: Option<String>,
    href: Option<String>,
    status: TaskStatus,
    id: Option<String>,
}

impl TaskRow {
    #[must_use]
    pub fn new(title: impl Into<String>, status: TaskStatus) -> Self {
        Self {
            title: title.into(),
            detail: None,
            href: None,
            status,
            id: None,
        }
    }

    /// What the task was asked, cut with an ellipsis.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Where the task's own page is.
    #[must_use]
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    fn inner(&self) -> Markup {
        let status = self.status;
        html! {
            span class="oa-task-row-icon" aria-hidden="true" {
                (crate::icons::Icon::Terminal.size(crate::icons::IconSize::Sm))
            }
            span class="oa-task-row-title" { (self.title) }
            @if let Some(detail) = &self.detail {
                span class="oa-task-row-detail" title=(detail) { (detail) }
            }
            span class="oa-chat-status oa-task-row-status" data-status=(status.key()) {
                @if status == TaskStatus::Working {
                    (crate::actions::LoadingIndicator::new().decorative())
                } @else {
                    span class="oa-chat-status-dot" aria-hidden="true" {}
                }
                span class="oa-chat-status-label" { (status.label()) }
            }
        }
    }
}

impl Render for TaskRow {
    fn render(&self) -> Markup {
        match &self.href {
            Some(href) => html! {
                a class="oa-task-row" href=(href) id=[self.id.as_deref()] data-status=(self.status.key()) {
                    (self.inner())
                }
            },
            None => html! {
                div class="oa-task-row" id=[self.id.as_deref()] data-status=(self.status.key()) {
                    (self.inner())
                }
            },
        }
    }
}
