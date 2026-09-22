//! NIP-24 extra metadata fields and tags.
//!
//! A kind `0` event's content JSON may carry fields NIP-01 does not
//! name: `display_name`, `website`, `banner`, `bot`, and `birthday`.
//! The deprecated spellings `displayName` and `username` are ignored,
//! never mapped back onto the new names. A kind `3` event's relay
//! read/write object is likewise deprecated and ignored — the follow
//! list reads `p` tags only.
//!
//! Across kinds, a `t` tag is a hashtag and its value MUST be
//! lowercase; [`lowercase_hashtags`] is the check admission applies.

use serde_json::Value;

use super::{DomainError, Event};

/// A kind `0` author's declared birthday: each field optional.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Birthday {
    pub year: Option<u64>,
    pub month: Option<u64>,
    pub day: Option<u64>,
}

/// The extra kind `0` fields NIP-24 tracks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileExtras {
    /// The richer display name, distinct from `name`.
    pub display_name: Option<String>,
    /// A related web URL.
    pub website: Option<String>,
    /// A wide profile banner image URL.
    pub banner: Option<String>,
    /// Whether the content is partly or wholly automated.
    pub bot: Option<bool>,
    /// The declared birthday.
    pub birthday: Option<Birthday>,
}

/// Read the extra fields of a kind `0` event's content.
///
/// `displayName` and `username` are deprecated spellings: they are
/// ignored, not read, and not folded into the new field names.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, content that
/// is not a JSON object, or a field with the wrong type.
pub fn open_profile_extras(event: &Event) -> Result<ProfileExtras, DomainError> {
    if event.kind != 0 {
        return Err(invalid("profile extras belong to kind 0"));
    }
    let content: Value = serde_json::from_str(&event.content)
        .map_err(|_| invalid("kind 0 content is a JSON object"))?;
    let object = content
        .as_object()
        .ok_or_else(|| invalid("kind 0 content is a JSON object"))?;
    let mut extras = ProfileExtras::default();
    if let Some(value) = object.get("display_name") {
        extras.display_name = Some(
            value
                .as_str()
                .ok_or_else(|| invalid("display_name is a string"))?
                .to_string(),
        );
    }
    for field in ["website", "banner"] {
        if let Some(value) = object.get(field) {
            let url = value
                .as_str()
                .ok_or_else(|| invalid("a URL field is a string"))?;
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err(invalid("website and banner are http:// or https:// URLs"));
            }
            if field == "website" {
                extras.website = Some(url.to_string());
            } else {
                extras.banner = Some(url.to_string());
            }
        }
    }
    if let Some(value) = object.get("bot") {
        extras.bot = Some(value.as_bool().ok_or_else(|| invalid("bot is a boolean"))?);
    }
    if let Some(value) = object.get("birthday") {
        let birthday = value
            .as_object()
            .ok_or_else(|| invalid("birthday is an object"))?;
        let number = |name: &str| -> Result<Option<u64>, DomainError> {
            match birthday.get(name) {
                None => Ok(None),
                Some(value) => value
                    .as_u64()
                    .map(Some)
                    .ok_or_else(|| invalid("a birthday field is a number")),
            }
        };
        extras.birthday = Some(Birthday {
            year: number("year")?,
            month: number("month")?,
            day: number("day")?,
        });
    }
    Ok(extras)
}

/// Every `t` tag on an event is a hashtag and its value MUST be a
/// lowercase string.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for an empty or non-lowercase
/// `t` value.
pub fn lowercase_hashtags(event: &Event) -> Result<(), DomainError> {
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("t")) {
        match tag.value() {
            Some(value) if !value.is_empty() && value.chars().all(|c| !c.is_uppercase()) => {}
            _ => return Err(invalid("a t tag value is a lowercase hashtag")),
        }
    }
    Ok(())
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
    fn extra_profile_fields_parse_and_deprecated_ones_are_ignored() {
        let profile = sign(
            0,
            Vec::new(),
            r#"{"name": "alice", "display_name": "Alice ✨",
               "displayName": "old spelling", "username": "also old",
               "website": "https://alice.example", "banner": "https://cdn.example/b.png",
               "bot": true, "birthday": {"year": 1990, "day": 4}}"#,
        );
        let extras = open_profile_extras(&profile).unwrap();
        assert_eq!(extras.display_name.as_deref(), Some("Alice ✨"));
        assert_eq!(extras.website.as_deref(), Some("https://alice.example"));
        assert_eq!(extras.banner.as_deref(), Some("https://cdn.example/b.png"));
        assert_eq!(extras.bot, Some(true));
        assert_eq!(
            extras.birthday,
            Some(Birthday {
                year: Some(1990),
                month: None,
                day: Some(4),
            })
        );

        // The deprecated spellings alone produce no display name.
        let old_only = sign(
            0,
            Vec::new(),
            r#"{"displayName": "legacy", "username": "legacy2"}"#,
        );
        assert_eq!(open_profile_extras(&old_only).unwrap().display_name, None);
        // A kind 3 relay object is deprecated content and ignored by the
        // follow parser — nothing reads it back.
        let follows = sign(3, Vec::new(), r#"{"wss://r.example": {"read": true}}"#);
        assert!(crate::domain::parse_follow_list(&follows.tags).is_ok());
    }

    #[test]
    fn hashtags_are_lowercase() {
        let good = sign(
            1,
            vec![Tag::new(vec!["t".into(), "nostr".into()])],
            "hashtag",
        );
        assert!(lowercase_hashtags(&good).is_ok());
        let bad = sign(
            1,
            vec![Tag::new(vec!["t".into(), "Nostr".into()])],
            "hashtag",
        );
        assert!(lowercase_hashtags(&bad).is_err());
        let empty = sign(1, vec![Tag::new(vec!["t".into()])], "hashtag");
        assert!(lowercase_hashtags(&empty).is_err());
        assert!(lowercase_hashtags(&sign(1, Vec::new(), "none")).is_ok());
    }

    #[test]
    fn malformed_extras_are_refused() {
        assert!(open_profile_extras(&sign(1, Vec::new(), "note")).is_err());
        assert!(open_profile_extras(&sign(0, Vec::new(), "not json")).is_err());
        assert!(open_profile_extras(&sign(0, Vec::new(), r#"{"bot": "yes"}"#)).is_err());
        assert!(open_profile_extras(&sign(0, Vec::new(), r#"{"website": "ftp://x"}"#)).is_err());
        assert!(
            open_profile_extras(&sign(0, Vec::new(), r#"{"birthday": {"year": "x"}}"#)).is_err()
        );
    }
}
