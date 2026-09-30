//! Desktop notifications for Coder's work: when a chat's Coder asks a
//! question, asks for approval, finishes, or fails while the window is not
//! in front, the desktop says so once.
//!
//! This decides *when*; the platform delivers (on Linux the desktop portal,
//! else `org.freedesktop.Notifications`). A chat seen for the first time is
//! only recorded, so opening the app on finished work notifies nothing, and
//! a status that does not change notifies nothing again. Nothing here reads
//! a message: the notice says what Coder is doing, under the chat's title.

use std::collections::BTreeMap;

/// What a chat's Coder is doing, as far as a notice cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Working, starting, stopped by the person, or nothing yet.
    Working,
    /// Waiting for an answer.
    Question,
    /// Waiting for approval to go ahead.
    Approval,
    /// The work finished.
    Finished,
    /// The work failed.
    Failed,
}

impl Status {
    /// The notice's text, `None` for a status that asks nothing of the
    /// person.
    #[must_use]
    pub fn body(self) -> Option<&'static str> {
        match self {
            Status::Working => None,
            Status::Question => Some("Coder asked a question"),
            Status::Approval => Some("Coder asked for approval"),
            Status::Finished => Some("Coder finished"),
            Status::Failed => Some("Coder stopped with an error"),
        }
    }
}

/// One notification to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Stable for the chat, so a newer notice replaces an older one.
    pub id: String,
    pub title: String,
    pub body: String,
    /// Coder waits for the person.
    pub urgent: bool,
}

/// The last status seen for each chat.
#[derive(Debug, Default)]
pub struct Notices {
    seen: BTreeMap<String, Status>,
}

impl Notices {
    /// Records each chat's `(id, title, status)` and returns the notices
    /// to show: chats whose status changed to one that asks for the
    /// person, while the window is not `focused`. Chats no longer listed
    /// are forgotten.
    pub fn observe(
        &mut self,
        chats: impl IntoIterator<Item = (String, String, Status)>,
        focused: bool,
    ) -> Vec<Notice> {
        let mut notices = vec![];
        let mut seen = BTreeMap::new();
        for (id, title, status) in chats {
            let before = self.seen.get(&id).copied();
            if !focused
                && before.is_some_and(|before| before != status)
                && let Some(body) = status.body()
            {
                notices.push(Notice {
                    id: format!("coder-{id}"),
                    title: if title.trim().is_empty() {
                        "OpenAgents".into()
                    } else {
                        title
                    },
                    body: body.into(),
                    urgent: matches!(status, Status::Question | Status::Approval),
                });
            }
            seen.insert(id, status);
        }
        self.seen = seen;
        notices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(status: Status) -> Vec<(String, String, Status)> {
        vec![("c1".into(), "Fix the login bug".into(), status)]
    }

    #[test]
    fn a_change_that_asks_for_the_person_notifies_once_while_the_window_is_away() {
        let mut notices = Notices::default();
        assert!(notices.observe(chat(Status::Working), false).is_empty());
        let asked = notices.observe(chat(Status::Approval), false);
        assert_eq!(
            asked,
            [Notice {
                id: "coder-c1".into(),
                title: "Fix the login bug".into(),
                body: "Coder asked for approval".into(),
                urgent: true,
            }]
        );
        // The same status again: nothing.
        assert!(notices.observe(chat(Status::Approval), false).is_empty());
        assert!(notices.observe(chat(Status::Working), false).is_empty());
        let done = notices.observe(chat(Status::Finished), false);
        assert_eq!(done[0].body, "Coder finished");
        assert!(!done[0].urgent);
    }

    #[test]
    fn nothing_notifies_while_the_window_is_in_front_or_for_a_chat_first_seen() {
        let mut notices = Notices::default();
        // Opening the app on finished work.
        assert!(notices.observe(chat(Status::Finished), false).is_empty());
        assert!(notices.observe(chat(Status::Working), true).is_empty());
        // In front: recorded, not shown, and not shown later either.
        assert!(notices.observe(chat(Status::Question), true).is_empty());
        assert!(notices.observe(chat(Status::Question), false).is_empty());
        let failed = notices.observe(chat(Status::Failed), false);
        assert_eq!(failed[0].body, "Coder stopped with an error");
        // A chat that goes away is forgotten, and comes back unannounced.
        assert!(notices.observe(vec![], false).is_empty());
        assert!(notices.observe(chat(Status::Approval), false).is_empty());
    }

    #[test]
    fn an_untitled_chat_is_named_for_the_app() {
        let mut notices = Notices::default();
        let untitled = |status| vec![("c2".to_owned(), "  ".to_owned(), status)];
        notices.observe(untitled(Status::Working), false);
        assert_eq!(
            notices.observe(untitled(Status::Question), false)[0].title,
            "OpenAgents"
        );
    }
}
