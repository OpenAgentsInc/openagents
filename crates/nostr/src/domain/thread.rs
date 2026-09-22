//! NIP-7D forum threads.
//!
//! A kind `11` event is a forum thread; it SHOULD carry a `title`
//! tag. Replies are NIP-22 kind `1111` comments whose root is the
//! thread — never a nested reply — so `is_thread_reply` checks the
//! comment's root `E` scope names the thread and its kind hint is
//! `11`. NIP-7D is a draft, so the kind is not added to the NIP-11
//! list.

use super::comment::{CommentScope, open_comment};
use super::{DomainError, Event};

const THREAD_KIND: u16 = 11;

/// A kind `11` forum thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumThread {
    /// The `title` tag when the author set one — recommended, not
    /// required.
    pub title: Option<String>,
    /// The thread's opening text.
    pub content: String,
}

/// Read a kind `11` event into a thread.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or an empty
/// `title` tag.
pub fn open_thread(event: &Event) -> Result<ForumThread, DomainError> {
    if event.kind != THREAD_KIND {
        return Err(invalid("a forum thread is kind 11"));
    }
    let title = event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("title"))
        .map(|tag| {
            tag.value()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| invalid("a title tag names the thread"))
        })
        .transpose()?;
    Ok(ForumThread {
        title,
        content: event.content.clone(),
    })
}

/// Whether a kind `1111` comment replies to this thread: its root
/// scope is an `E` tag naming the thread's id with kind hint `11`.
/// Replies always point at the root so hierarchies never nest.
#[must_use]
pub fn is_thread_reply(comment: &Event, thread: &Event) -> bool {
    if comment.kind != 1111 || thread.kind != THREAD_KIND {
        return false;
    }
    let Ok(opened) = open_comment(comment) else {
        return false;
    };
    matches!(
        &opened.root,
        CommentScope::Event { id, .. } if *id == thread.id
    ) && comment
        .tags
        .iter()
        .any(|tag| tag.name() == Some("K") && tag.value() == Some("11"))
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    #[test]
    fn a_thread_carries_its_title_and_a_reply_points_at_the_root() {
        let thread = sign(
            THREAD_KIND,
            vec![Tag::new(vec!["title".into(), "GM".into()])],
            "Good morning",
        );
        let opened = open_thread(&thread).unwrap();
        assert_eq!(opened.title.as_deref(), Some("GM"));
        assert_eq!(opened.content, "Good morning");

        let reply = sign(
            1111,
            vec![
                Tag::new(vec![
                    "E".into(),
                    thread.id.clone(),
                    "wss://r.example".into(),
                ]),
                Tag::new(vec!["K".into(), "11".into()]),
                Tag::new(vec![
                    "P".into(),
                    thread.pubkey.clone(),
                    "wss://r.example".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    thread.id.clone(),
                    "wss://r.example".into(),
                    thread.pubkey.clone(),
                ]),
                Tag::new(vec!["k".into(), "11".into()]),
                Tag::new(vec![
                    "p".into(),
                    thread.pubkey.clone(),
                    "wss://r.example".into(),
                ]),
            ],
            "Cool beans",
        );
        assert!(is_thread_reply(&reply, &thread));

        // A comment rooted elsewhere is not a thread reply.
        let other_id = "ab".repeat(32);
        let other = sign(
            1111,
            vec![
                Tag::new(vec!["E".into(), other_id.clone()]),
                Tag::new(vec!["K".into(), "11".into()]),
                Tag::new(vec!["P".into(), thread.pubkey.clone()]),
                Tag::new(vec![
                    "e".into(),
                    other_id,
                    "wss://r.example".into(),
                    thread.pubkey.clone(),
                ]),
                Tag::new(vec!["k".into(), "11".into()]),
                Tag::new(vec!["p".into(), thread.pubkey.clone()]),
            ],
            "elsewhere",
        );
        assert!(!is_thread_reply(&other, &thread));
        // A kind 1 note is never a thread reply.
        let note = sign(1, Vec::new(), "just a note");
        assert!(!is_thread_reply(&note, &thread));
    }

    #[test]
    fn malformed_threads_are_refused() {
        assert!(open_thread(&sign(1, Vec::new(), "note")).is_err());
        let empty_title = sign(
            THREAD_KIND,
            vec![Tag::new(vec!["title".into()])],
            "untitled",
        );
        assert!(open_thread(&empty_title).is_err());
        // Title is recommended, not required.
        assert!(open_thread(&sign(THREAD_KIND, Vec::new(), "GM")).is_ok());
    }
}
