//! NIP-25 reactions.
//!
//! A kind `7` reaction points at a native event: the last `e` tag names
//! the target, the last `p` tag names its author, and an `a` tag may
//! carry the target's address when the target is addressable. The
//! content is the verdict: `+` or empty is a like, `-` is a dislike,
//! and a single `:shortcode:` is a NIP-30 custom emoji backed by one
//! `emoji` tag. A `k` tag may carry the reacted kind as a string.
//!
//! A kind `17` reaction targets external content and must carry the
//! NIP-73 `i` and `k` tags that identify it.
//!
//! The relay stores both kinds like any regular event. NIP-25 is a
//! draft, so they are not added to the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const REACTION_KIND: u16 = 7;
const EXTERNAL_REACTION_KIND: u16 = 17;

/// What a reaction's content says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionVerdict {
    /// `+` or empty: a like.
    Like,
    /// `-`: a dislike.
    Dislike,
    /// An emoji, a `:shortcode:`, or any other content.
    Custom,
}

/// A kind `7` reaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reaction {
    /// The content's verdict.
    pub verdict: ReactionVerdict,
    /// The reacted event id — the last `e` tag.
    pub target: String,
    /// The reacted author's pubkey — the last `p` tag, when present.
    pub author: Option<String>,
    /// The target's `kind:pubkey:d` address, when the `a` tag carries it.
    pub address: Option<String>,
    /// The reacted kind the `k` tag declares, when present.
    pub kind_hint: Option<u16>,
    /// The custom emoji shortcode, when the content is `:shortcode:`.
    pub emoji: Option<String>,
}

/// The verdict a reaction's content carries.
#[must_use]
pub fn reaction_verdict(content: &str) -> ReactionVerdict {
    match content {
        "" | "+" => ReactionVerdict::Like,
        "-" => ReactionVerdict::Dislike,
        _ => ReactionVerdict::Custom,
    }
}

/// Read a kind `7` event into a reaction.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, no `e` tag, a
/// malformed target or author, a `k` tag that is not a kind number, or
/// a `:shortcode:` content without exactly one matching `emoji` tag.
pub fn open_reaction(event: &Event) -> Result<Reaction, DomainError> {
    if event.kind != REACTION_KIND {
        return Err(invalid("a reaction is kind 7"));
    }
    let target = last_tag_value(event, "e")
        .filter(|id| decode_lower_hex::<32>(id, "e").is_ok())
        .ok_or_else(|| invalid("a reaction names the reacted event in its last e tag"))?;
    let author = match last_tag_value(event, "p") {
        Some(pubkey) if decode_lower_hex::<32>(pubkey, "p").is_ok() => Some(pubkey.to_string()),
        Some(_) => return Err(invalid("the last p tag is the reacted author's pubkey")),
        None => None,
    };
    let address = last_tag_value(event, "a").map(|value| {
        let parts: Vec<&str> = value.split(':').collect();
        if parts.len() == 3
            && parts[0].parse::<u16>().is_ok()
            && decode_lower_hex::<32>(parts[1], "a pubkey").is_ok()
            && !parts[2].is_empty()
        {
            value.to_string()
        } else {
            String::new()
        }
    });
    if address.as_deref() == Some("") {
        return Err(invalid("an a tag is kind:pubkey:d"));
    }
    let kind_hint = match last_tag_value(event, "k") {
        Some(value) => Some(
            value
                .parse::<u16>()
                .map_err(|_| invalid("a k tag is the reacted kind as a string"))?,
        ),
        None => None,
    };
    let verdict = reaction_verdict(&event.content);
    let emoji = shortcode(&event.content);
    if let Some(code) = &emoji {
        let tags: Vec<&super::Tag> = event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some("emoji"))
            .collect();
        if tags.len() != 1
            || tags[0].value() != Some(code.as_str())
            || tags[0].0.get(2).is_none_or(|url| url.is_empty())
        {
            return Err(invalid(
                "a :shortcode: reaction names that code in one emoji tag with a URL",
            ));
        }
    }
    Ok(Reaction {
        verdict,
        target: target.to_string(),
        author,
        address,
        kind_hint,
        emoji,
    })
}

/// Check a kind `17` external-content reaction: at least one `i` tag
/// and one `k` tag identifying the target per NIP-73.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or a missing
/// external reference.
pub fn open_external_reaction(event: &Event) -> Result<ReactionVerdict, DomainError> {
    if event.kind != EXTERNAL_REACTION_KIND {
        return Err(invalid("an external reaction is kind 17"));
    }
    let ids = super::external_id::open_external_ids(event)?;
    if ids.is_empty() {
        return Err(invalid("kind 17 requires NIP-73 i and k tags"));
    }
    Ok(reaction_verdict(&event.content))
}

/// The `:shortcode:` a content is, when it is exactly one.
fn shortcode(content: &str) -> Option<String> {
    let inner = content.strip_prefix(':')?.strip_suffix(':')?;
    if inner.is_empty() || inner.contains(':') || inner.chars().any(char::is_whitespace) {
        return None;
    }
    Some(inner.to_string())
}

fn last_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    event
        .tags
        .iter()
        .rev()
        .find(|tag| tag.name() == Some(name))
        .and_then(|tag| tag.value())
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
    fn a_reaction_names_its_target_as_the_last_e_tag() {
        let target = "ab".repeat(32);
        let author = "cd".repeat(32);
        let mention = "ef".repeat(32);
        let like = sign(
            REACTION_KIND,
            vec![
                Tag::new(vec!["e".into(), mention.clone()]),
                Tag::new(vec!["e".into(), target.clone(), "wss://r.example".into()]),
                Tag::new(vec!["p".into(), mention.clone()]),
                Tag::new(vec!["p".into(), author.clone()]),
                Tag::new(vec!["a".into(), format!("1:{author}:note")]),
                Tag::new(vec!["k".into(), "1".into()]),
            ],
            "+",
        );
        let reaction = open_reaction(&like).unwrap();
        assert_eq!(reaction.verdict, ReactionVerdict::Like);
        assert_eq!(reaction.target, target);
        assert_eq!(reaction.author.as_deref(), Some(author.as_str()));
        assert_eq!(
            reaction.address.as_deref(),
            Some(format!("1:{author}:note").as_str())
        );
        assert_eq!(reaction.kind_hint, Some(1));

        let empty = sign(
            REACTION_KIND,
            vec![Tag::new(vec!["e".into(), target.clone()])],
            "",
        );
        assert_eq!(
            open_reaction(&empty).unwrap().verdict,
            ReactionVerdict::Like
        );
        let dislike = sign(REACTION_KIND, vec![Tag::new(vec!["e".into(), target])], "-");
        assert_eq!(
            open_reaction(&dislike).unwrap().verdict,
            ReactionVerdict::Dislike
        );

        let emoji = sign(
            REACTION_KIND,
            vec![
                Tag::new(vec!["e".into(), "ab".repeat(32)]),
                Tag::new(vec![
                    "emoji".into(),
                    "soapbox".into(),
                    "https://e.example/soapbox.png".into(),
                ]),
            ],
            ":soapbox:",
        );
        let custom = open_reaction(&emoji).unwrap();
        assert_eq!(custom.verdict, ReactionVerdict::Custom);
        assert_eq!(custom.emoji.as_deref(), Some("soapbox"));
    }

    #[test]
    fn an_external_reaction_carries_i_and_k_tags() {
        let web = sign(
            EXTERNAL_REACTION_KIND,
            vec![
                Tag::new(vec!["k".into(), "web".into()]),
                Tag::new(vec!["i".into(), "https://example.com".into()]),
            ],
            "⭐",
        );
        assert_eq!(
            open_external_reaction(&web).unwrap(),
            ReactionVerdict::Custom
        );
        let bare = sign(EXTERNAL_REACTION_KIND, Vec::new(), "+");
        assert!(open_external_reaction(&bare).is_err());
        let only_i = sign(
            EXTERNAL_REACTION_KIND,
            vec![Tag::new(vec!["i".into(), "https://example.com".into()])],
            "+",
        );
        assert!(open_external_reaction(&only_i).is_err());
    }

    #[test]
    fn malformed_reactions_are_refused() {
        assert!(open_reaction(&sign(REACTION_KIND, Vec::new(), "+")).is_err());
        assert!(open_reaction(&sign(1, Vec::new(), "note")).is_err());
        let bad_k = sign(
            REACTION_KIND,
            vec![
                Tag::new(vec!["e".into(), "ab".repeat(32)]),
                Tag::new(vec!["k".into(), "kind one".into()]),
            ],
            "+",
        );
        assert!(open_reaction(&bad_k).is_err());
        let shortcode_no_tag = sign(
            REACTION_KIND,
            vec![Tag::new(vec!["e".into(), "ab".repeat(32)])],
            ":fire:",
        );
        assert!(open_reaction(&shortcode_no_tag).is_err());
        let two_emoji = sign(
            REACTION_KIND,
            vec![
                Tag::new(vec!["e".into(), "ab".repeat(32)]),
                Tag::new(vec!["emoji".into(), "a".into(), "https://e/a.png".into()]),
                Tag::new(vec!["emoji".into(), "b".into(), "https://e/b.png".into()]),
            ],
            ":a:",
        );
        assert!(open_reaction(&two_emoji).is_err());
    }
}
