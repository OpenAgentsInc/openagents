//! NIP-14 subject tags.
//!
//! A `subject` tag on a kind `1` text event carries the thread title a
//! browser-style client lists instead of the first words of content. A
//! reply SHOULD replicate the parent's subject and MAY adorn it with a
//! `Re:` prefix — [`reply_subject`] does exactly that. Subjects should
//! stay under 80 characters; [`subject_fits`] is that bound.
//!
//! The relay stores kind `1` either way, and NIP-14 is a draft, so the
//! tag is not added to the NIP-11 list.

use super::Event;

/// The recommended subject bound: clients trim past this.
pub const SUBJECT_BOUND: usize = 80;

/// The subject a kind `1` event declares, when it declares one.
#[must_use]
pub fn subject(event: &Event) -> Option<&str> {
    if event.kind != 1 {
        return None;
    }
    event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("subject"))
        .and_then(|tag| tag.value())
}

/// Whether a subject fits the recommended bound.
#[must_use]
pub fn subject_fits(value: &str) -> bool {
    value.chars().count() <= SUBJECT_BOUND
}

/// The subject a reply carries: the parent's subject with the `Re:`
/// adornment the pinned text names, unless it is already there.
#[must_use]
pub fn reply_subject(parent_subject: &str) -> String {
    if parent_subject.starts_with("Re:") {
        parent_subject.to_string()
    } else {
        format!("Re: {parent_subject}")
    }
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
    fn a_reply_replicates_the_subject_with_a_re_prefix() {
        let root = sign(
            1,
            vec![Tag::new(vec!["subject".into(), "the plan".into()])],
            "first post",
        );
        assert_eq!(subject(&root), Some("the plan"));
        assert!(subject_fits("the plan"));

        let reply = sign(
            1,
            vec![
                Tag::new(vec!["e".into(), root.id.clone(), "".into(), "reply".into()]),
                Tag::new(vec!["subject".into(), reply_subject("the plan")]),
            ],
            "second post",
        );
        assert_eq!(subject(&reply), Some("Re: the plan"));
        // A second reply does not double the adornment.
        assert_eq!(reply_subject("Re: the plan"), "Re: the plan");
        // An 81-character subject misses the recommended bound.
        assert!(!subject_fits(&"x".repeat(81)));
        // Another kind carries no subject under this NIP.
        let other = sign(
            30_023,
            vec![Tag::new(vec!["subject".into(), "not a note".into()])],
            "article",
        );
        assert_eq!(subject(&other), None);
    }
}
