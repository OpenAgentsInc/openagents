//! NIP-08 `#[index]` mention resolution.
//!
//! A mention inside a text note's content is the placeholder
//! `#[index]`, where `index` is the 0-based position of the mentioned
//! `e` or `p` tag in the tags array. A reader may replace the
//! placeholder with the tag's value; an index outside the array, or
//! pointing at a tag that is not `e` or `p`, is normal text and MUST
//! NOT be replaced. NIP-08 is unrecommended — NIP-27 `nostr:` links
//! supersede it — so this is compatibility surface only and is not
//! added to the NIP-11 list.

use super::{Event, Tag};

/// A resolved `#[index]` mention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexMention<'a> {
    /// The placeholder's tag position.
    pub index: usize,
    /// The tag name the index points at: `e` or `p`.
    pub tag: &'a str,
    /// The mentioned event id or pubkey — the tag's value.
    pub target: &'a str,
}

/// Resolve every `#[index]` placeholder in `content` against `tags`.
///
/// Placeholders whose index is out of range or whose tag is not `e` or
/// `p` are skipped: they render as normal text.
#[must_use]
pub fn resolve_mentions<'a>(content: &str, tags: &'a [Tag]) -> Vec<IndexMention<'a>> {
    let mut mentions = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("#[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(']') else { break };
        let (digits, after) = rest.split_at(end);
        rest = &after[1..];
        let Ok(index) = digits.parse::<usize>() else {
            continue;
        };
        let Some(tag) = tags.get(index) else { continue };
        let Some(name) = tag.name() else { continue };
        if name != "e" && name != "p" {
            continue;
        }
        let Some(target) = tag.value() else { continue };
        mentions.push(IndexMention {
            index,
            tag: name,
            target,
        });
    }
    mentions
}

/// The resolved mentions of a kind `1` text note.
#[must_use]
pub fn note_mentions(event: &Event) -> Vec<IndexMention<'_>> {
    if event.kind != 1 {
        return Vec::new();
    }
    resolve_mentions(&event.content, &event.tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RelaySigner;

    fn sign(tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, tags, content.to_string())
    }

    #[test]
    fn an_index_mention_resolves_to_its_e_or_p_tag_only() {
        let pubkey = "27".repeat(32);
        let note = sign(
            vec![
                Tag::new(vec!["p".into(), pubkey.clone()]),
                Tag::new(vec!["t".into(), "nostr".into()]),
                Tag::new(vec!["e".into(), "ab".repeat(32)]),
            ],
            "hello #[0] re #[2] — #[1] and #[9] stay literal",
        );
        let mentions = note_mentions(&note);
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].tag, "p");
        assert_eq!(mentions[0].target, pubkey);
        assert_eq!(mentions[1].tag, "e");
        // #[1] names a t tag and #[9] is out of range: normal text.
        assert!(mentions.iter().all(|m| m.index != 1 && m.index != 9));
        assert_eq!(note_mentions(&sign(Vec::new(), "no mentions")), vec![]);
        let non_note = {
            let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
            signer.sign(1_700_000_000, 4, Vec::new(), "#[0]".to_string())
        };
        assert_eq!(note_mentions(&non_note), vec![]);
    }
}
