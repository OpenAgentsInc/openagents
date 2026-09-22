//! NIP-36 content warnings.
//!
//! A `content-warning` tag marks an event's content as needing reader
//! approval before display; the tag's optional value is the reason.
//! `l` and `L` tags may qualify the warning further under NIP-32.
//! NIP-36 is a draft, so the tag is not added to the NIP-11 list and
//! admission does not require it — it binds client display only.

use super::Event;

/// A `content-warning` marking: the reason when the tag carries one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentWarning<'a> {
    /// Why the content is hidden, when the author said.
    pub reason: Option<&'a str>,
}

/// The content warning an event carries, if any. A bare
/// `content-warning` tag warns without a reason.
#[must_use]
pub fn content_warning(event: &Event) -> Option<ContentWarning<'_>> {
    event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("content-warning"))
        .map(|tag| ContentWarning {
            reason: tag.value().filter(|value| !value.is_empty()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(tags: Vec<Tag>) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, tags, "sensitive".to_string())
    }

    #[test]
    fn a_content_warning_hides_the_content_until_the_reader_acts() {
        let reasoned = sign(vec![
            Tag::new(vec!["t".into(), "hashtag".into()]),
            Tag::new(vec!["content-warning".into(), "flashing images".into()]),
        ]);
        assert_eq!(
            content_warning(&reasoned),
            Some(ContentWarning {
                reason: Some("flashing images"),
            })
        );

        let bare = sign(vec![Tag::new(vec!["content-warning".into()])]);
        assert_eq!(
            content_warning(&bare),
            Some(ContentWarning { reason: None })
        );

        assert_eq!(content_warning(&sign(Vec::new())), None);
    }
}
