//! NIP-30 event-scoped emoji references. This module never fetches images or
//! emits HTML; renderers decide how to display the returned inert tokens.

use super::{DomainError, Event, ReplacementAddress};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomEmoji {
    pub shortcode: String,
    pub image_url: String,
    pub set: Option<ReplacementAddress>,
}

/// A literal span or an emoji declared by this exact event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmojiToken {
    Text(String),
    Emoji(CustomEmoji),
}

/// The event fields NIP-30 permits consumers to emojify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiText {
    pub field: String,
    pub tokens: Vec<EmojiToken>,
}

/// Read NIP-30 fields and tags without fetching images.
///
/// Unknown shortcodes remain literal. Duplicate declarations refuse rather
/// than select a media target by tag order. Emoji-set addresses are retained
/// as provenance only and must name kind 30030.
///
/// # Errors
///
/// Returns an error for unsupported kinds, malformed declarations or profile
/// JSON, duplicate shortcodes, or a non-HTTP image URL.
pub fn emojify_event(event: &Event) -> Result<Vec<EmojiText>, DomainError> {
    if !matches!(event.kind, 0 | 1 | 1_111 | 7 | 30_315) {
        return Err(invalid("NIP-30 applies to kinds 0, 1, 7, 1111, and 30315"));
    }
    let mut emojis = BTreeMap::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("emoji")) {
        let values = tag.as_slice();
        if !(3..=4).contains(&values.len()) || crate::lane::emoji_shortcode(&values[1]).is_err() {
            return Err(invalid(
                "an emoji tag needs a valid shortcode and image URL",
            ));
        }
        let image_url = &values[2];
        let host = image_url
            .strip_prefix("https://")
            .or_else(|| image_url.strip_prefix("http://"));
        if host.is_none_or(|rest| {
            rest.split(['/', '?', '#'])
                .next()
                .unwrap_or_default()
                .is_empty()
        }) || image_url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(invalid("an emoji image URL must be HTTP or HTTPS"));
        }
        let set = values
            .get(3)
            .map(|value| {
                let address = ReplacementAddress::from_str(value)?;
                if address.kind != 30_030 {
                    return Err(invalid("an emoji set has kind 30030"));
                }
                Ok(address)
            })
            .transpose()?;
        let emoji = CustomEmoji {
            shortcode: values[1].clone(),
            image_url: image_url.clone(),
            set,
        };
        if emojis.insert(values[1].clone(), emoji).is_some() {
            return Err(invalid("an emoji shortcode is declared more than once"));
        }
    }
    if event.kind == 0 {
        let profile: serde_json::Value = serde_json::from_str(&event.content)
            .map_err(|_| invalid("profile content must be a JSON object"))?;
        let object = profile
            .as_object()
            .ok_or_else(|| invalid("profile content must be a JSON object"))?;
        ["name", "about"]
            .into_iter()
            .filter_map(|field| object.get(field).map(|value| (field, value)))
            .map(|(field, value)| {
                let text = value
                    .as_str()
                    .ok_or_else(|| invalid("profile emoji fields must be strings"))?;
                Ok(EmojiText {
                    field: field.into(),
                    tokens: tokenize(text, &emojis),
                })
            })
            .collect()
    } else {
        Ok(vec![EmojiText {
            field: "content".into(),
            tokens: tokenize(&event.content, &emojis),
        }])
    }
}

fn tokenize(text: &str, emojis: &BTreeMap<String, CustomEmoji>) -> Vec<EmojiToken> {
    let mut tokens = Vec::new();
    let mut literal = 0;
    let mut cursor = 0;
    while cursor < text.len() {
        let Some(start) = text[cursor..].find(':').map(|n| cursor + n) else {
            break;
        };
        let Some(end) = text[start + 1..].find(':').map(|n| start + 1 + n) else {
            break;
        };
        if let Some(emoji) = emojis.get(&text[start + 1..end]) {
            if literal < start {
                tokens.push(EmojiToken::Text(text[literal..start].into()));
            }
            tokens.push(EmojiToken::Emoji(emoji.clone()));
            literal = end + 1;
            cursor = literal;
        } else {
            cursor = start + 1;
        }
    }
    if literal < text.len() {
        tokens.push(EmojiToken::Text(text[literal..].into()));
    }
    tokens
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    #[test]
    fn comment_emoji_are_scoped_and_unknown_shortcodes_stay_text() {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        let key = signer.pubkey();
        let tags = vec![
            Tag::new(vec!["I".into(), "https://example.com/article".into()]),
            Tag::new(vec!["K".into(), "web".into()]),
            Tag::new(vec!["i".into(), "https://example.com/article".into()]),
            Tag::new(vec!["k".into(), "web".into()]),
            Tag::new(vec![
                "emoji".into(),
                "party".into(),
                "https://example.com/party.png".into(),
                format!("30030:{key}:set"),
            ]),
        ];
        let event = signer.sign(1, 1_111, tags, "Hi :party: :unknown: 🦀 :party:".into());
        event.validate_structure().unwrap();
        let fields = emojify_event(&event).unwrap();
        assert_eq!(
            fields[0]
                .tokens
                .iter()
                .filter(|token| matches!(token, EmojiToken::Emoji(_)))
                .count(),
            2
        );
        assert!(
            fields[0]
                .tokens
                .contains(&EmojiToken::Text(" :unknown: 🦀 ".into()))
        );
        let mut other = event.clone();
        other.tags.retain(|tag| tag.name() != Some("emoji"));
        assert_eq!(
            emojify_event(&other).unwrap()[0].tokens,
            vec![EmojiToken::Text(event.content.clone())]
        );
        let profile = signer.sign(
            1,
            0,
            event
                .tags
                .iter()
                .filter(|tag| tag.name() == Some("emoji"))
                .cloned()
                .collect(),
            r#"{"name":":party:","about":"hello","website":":party:"}"#.into(),
        );
        assert_eq!(emojify_event(&profile).unwrap().len(), 2);
    }

    #[test]
    fn ambiguous_shortcodes_and_unsafe_media_targets_refuse() {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        let tag = Tag::new(vec![
            "emoji".into(),
            "ok".into(),
            "https://example.com/ok.png".into(),
        ]);
        assert!(emojify_event(&signer.sign(1, 1, vec![tag.clone(), tag], ":ok:".into())).is_err());
        for url in [
            "javascript:alert(1)",
            "https://",
            "https://example.com/\nimage",
        ] {
            let tag = Tag::new(vec!["emoji".into(), "ok".into(), url.into()]);
            assert!(emojify_event(&signer.sign(1, 1, vec![tag], ":ok:".into())).is_err());
        }
    }
}
