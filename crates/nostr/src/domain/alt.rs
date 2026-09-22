//! NIP-31 `alt` fallback summaries.
//!
//! A custom event kind that is not meant to be read as text can carry
//! an `alt` tag: a short human-readable plaintext summary a
//! `kind:1`-centric client can show when the event surfaces in a
//! timeline. NIP-31 is unrecommended and a draft, so the tag is not
//! added to the NIP-11 list and admission does not require it.

use super::Event;

/// The `alt` tag's human-readable summary, when present and non-empty.
#[must_use]
pub fn alt_text(event: &Event) -> Option<&str> {
    event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("alt"))
        .and_then(|tag| tag.value())
        .filter(|value| !value.is_empty())
}

/// Whether an event is one a text-only client needs the `alt` fallback
/// for — every kind but `1`, which clients already render.
#[must_use]
pub fn needs_fallback(event: &Event) -> bool {
    event.kind != 1
}

/// The summary a text-only client should display for an event: the
/// `alt` text when the kind is not rendered natively, otherwise none.
#[must_use]
pub fn fallback_summary(event: &Event) -> Option<&str> {
    needs_fallback(event).then(|| alt_text(event)).flatten()
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
    fn an_alt_tag_summarizes_a_kind_a_text_client_does_not_render() {
        let custom = sign(
            30_300,
            vec![Tag::new(vec![
                "alt".into(),
                "Board update in the Block game".into(),
            ])],
            "{}",
        );
        assert!(needs_fallback(&custom));
        assert_eq!(
            fallback_summary(&custom),
            Some("Board update in the Block game")
        );
        assert_eq!(alt_text(&custom), fallback_summary(&custom));

        // A kind 1 renders natively — the alt text is not a fallback.
        let note = sign(
            1,
            vec![Tag::new(vec!["alt".into(), "redundant".into()])],
            "text",
        );
        assert!(!needs_fallback(&note));
        assert_eq!(fallback_summary(&note), None);
        assert_eq!(alt_text(&note), Some("redundant"));

        let bare = sign(30_300, vec![Tag::new(vec!["alt".into()])], "{}");
        assert_eq!(fallback_summary(&bare), None);
    }
}
