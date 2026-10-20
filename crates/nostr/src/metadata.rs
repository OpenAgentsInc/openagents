//! A person's or agent's public profile: kind 0 metadata (NIP-01, NIP-24)
//! and the kind 3 follow list (NIP-02), as the website, the CLI, and the
//! apps read and write them (#12254).
//!
//! Reading is lenient: a field of the wrong type, a URL that isn't
//! `https://`, or text past its cap is dropped, never an error, because a
//! profile written by any client should still show what it can. Writing
//! merges the edited fields onto the profile's earlier content, so fields
//! another client set (a banner, a birthday) survive an edit here.

use serde_json::{Map, Value};

use crate::domain::{Event, RelaySigner, Tag};

/// Kind 0, user metadata.
pub const METADATA_KIND: u16 = 0;
/// Kind 3, follow list.
pub const FOLLOWS_KIND: u16 = 3;

/// The longest name and display name, in characters.
pub const NAME_MAX: usize = 64;
/// The longest `about`, in characters.
pub const ABOUT_MAX: usize = 2_000;
/// The longest URL.
pub const URL_MAX: usize = 1_000;
/// The most follows read from one list.
pub const FOLLOWS_MAX: usize = 5_000;

/// The profile fields this repository reads and writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub about: Option<String>,
    /// An `https://` image URL.
    pub picture: Option<String>,
    /// An `https://` image URL.
    pub banner: Option<String>,
    /// An `http://` or `https://` URL.
    pub website: Option<String>,
    /// `name@domain`, as the profile claims it; a reader checks it.
    pub nip05: Option<String>,
    /// A Lightning address, `name@domain`.
    pub lud16: Option<String>,
    /// Whether an agent writes it (NIP-24).
    pub bot: Option<bool>,
}

fn text(object: &Map<String, Value>, field: &str, max: usize) -> Option<String> {
    let value = object.get(field)?.as_str()?.trim();
    // Only `about` keeps line breaks and tabs.
    let allowed = |c: char| field == "about" && matches!(c, '\n' | '\r' | '\t');
    (!value.is_empty()
        && value.chars().count() <= max
        && !value.chars().any(|c| c.is_control() && !allowed(c)))
    .then(|| value.to_string())
}

/// Whether `url` is an `https://` URL of a sane length with a host.
#[must_use]
pub fn https_url(url: &str) -> bool {
    url.len() <= URL_MAX
        && url
            .strip_prefix("https://")
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn web_url(url: &str) -> bool {
    https_url(url)
        || (url.len() <= URL_MAX
            && url
                .strip_prefix("http://")
                .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
            && !url.chars().any(|c| c.is_whitespace() || c.is_control()))
}

fn address(object: &Map<String, Value>, field: &str) -> Option<String> {
    let value = object.get(field)?.as_str()?;
    let lower = value.trim().to_ascii_lowercase();
    let (name, domain) = lower.split_once('@')?;
    (!name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
        && domain.contains('.')
        && domain.len() <= 253
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-')))
    .then_some(lower)
}

impl Metadata {
    /// Read a kind 0 event's content. Content that isn't a JSON object
    /// reads as an empty profile.
    #[must_use]
    pub fn read(content: &str) -> Self {
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(content) else {
            return Self::default();
        };
        let url = |field: &str, check: fn(&str) -> bool| {
            object
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|u| check(u))
                .map(str::to_string)
        };
        Self {
            name: text(&object, "name", NAME_MAX),
            display_name: text(&object, "display_name", NAME_MAX),
            about: text(&object, "about", ABOUT_MAX),
            picture: url("picture", https_url),
            banner: url("banner", https_url),
            website: url("website", web_url),
            nip05: address(&object, "nip05"),
            lud16: address(&object, "lud16"),
            bot: object.get("bot").and_then(Value::as_bool),
        }
    }

    /// The name to show: the display name, else the name.
    #[must_use]
    pub fn shown_name(&self) -> Option<&str> {
        self.display_name.as_deref().or(self.name.as_deref())
    }

    /// Check the fields a person typed, before they're signed. Answers the
    /// first problem in plain words.
    ///
    /// # Errors
    ///
    /// A sentence naming the field to fix.
    pub fn check(&self) -> Result<(), &'static str> {
        let short = |v: &Option<String>, max: usize| {
            v.as_deref()
                .is_none_or(|v| v.chars().count() <= max && !v.chars().any(char::is_control))
        };
        if !short(&self.name, NAME_MAX) || !short(&self.display_name, NAME_MAX) {
            return Err("Keep your name under 64 characters.");
        }
        if self
            .about
            .as_deref()
            .is_some_and(|a| a.chars().count() > ABOUT_MAX)
        {
            return Err("Keep your bio under 2,000 characters.");
        }
        if self.picture.as_deref().is_some_and(|u| !https_url(u)) {
            return Err("Use an https:// link for your picture.");
        }
        if self.banner.as_deref().is_some_and(|u| !https_url(u)) {
            return Err("Use an https:// link for your banner.");
        }
        if self.website.as_deref().is_some_and(|u| !web_url(u)) {
            return Err("Use a full web address for your website.");
        }
        if self.lud16.as_deref().is_some_and(|value| {
            let object = Map::from_iter([("lud16".to_owned(), Value::String(value.to_owned()))]);
            address(&object, "lud16").is_none()
        }) {
            return Err("Use a Lightning address such as name@openagents.com.");
        }
        Ok(())
    }

    /// Kind 0 content: these fields written over `earlier` (the profile's
    /// previous content, if any), so fields this type doesn't know about
    /// stay. A field set to `None` here is left as `earlier` had it.
    #[must_use]
    pub fn merged_content(&self, earlier: Option<&str>) -> String {
        let mut object = earlier
            .and_then(|c| serde_json::from_str::<Value>(c).ok())
            .and_then(|v| match v {
                Value::Object(o) => Some(o),
                _ => None,
            })
            .unwrap_or_default();
        let mut set = |field: &str, value: &Option<String>| {
            if let Some(value) = value {
                if value.is_empty() {
                    object.remove(field);
                } else {
                    object.insert(field.to_string(), Value::String(value.clone()));
                }
            }
        };
        set("name", &self.name);
        set("display_name", &self.display_name);
        set("about", &self.about);
        set("picture", &self.picture);
        set("banner", &self.banner);
        set("website", &self.website);
        set("nip05", &self.nip05);
        set("lud16", &self.lud16);
        if let Some(bot) = self.bot {
            object.insert("bot".into(), Value::Bool(bot));
        }
        Value::Object(object).to_string()
    }

    /// A signed kind 0 event over [`Self::merged_content`].
    #[must_use]
    pub fn sign(&self, signer: &RelaySigner, created_at: u64, earlier: Option<&str>) -> Event {
        signer.sign(
            created_at,
            METADATA_KIND,
            Vec::new(),
            self.merged_content(earlier),
        )
    }
}

/// The newest event of `kind` by `pubkey` with a valid id and signature.
#[must_use]
pub fn newest<'a>(pubkey: &str, kind: u16, events: &'a [Event]) -> Option<&'a Event> {
    crate::payto::newest_signed(pubkey, kind, events)
}

/// The newest valid profile `pubkey` signed, with when.
#[must_use]
pub fn profile_of(pubkey: &str, events: &[Event]) -> Option<(Metadata, u64)> {
    newest(pubkey, METADATA_KIND, events).map(|e| (Metadata::read(&e.content), e.created_at))
}

/// The keys a kind 3 event follows: valid lowercase hex `p` tags, in the
/// list's order, without repeats, at most [`FOLLOWS_MAX`].
#[must_use]
pub fn follows(event: &Event) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("p"))
        .filter_map(Tag::value)
        .map(str::to_ascii_lowercase)
        .filter(|key| key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()))
        .filter(|key| seen.insert(key.clone()))
        .take(FOLLOWS_MAX)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"07".repeat(32)).unwrap()
    }

    #[test]
    fn reading_keeps_what_is_good_and_drops_the_rest() {
        let read = Metadata::read(
            r#"{"name":"pat","display_name":"Pat","about":"Hi\nthere","picture":"https://x.test/p.png",
                "banner":"http://x.test/b.png","website":"http://pat.test","nip05":"Pat@OpenAgents.com",
                "lud16":"not an address","bot":false,"extra":1}"#,
        );
        assert_eq!(read.name.as_deref(), Some("pat"));
        assert_eq!(read.shown_name(), Some("Pat"));
        assert_eq!(read.about.as_deref(), Some("Hi\nthere"));
        assert_eq!(read.picture.as_deref(), Some("https://x.test/p.png"));
        assert_eq!(read.banner, None, "banner must be https");
        assert_eq!(read.website.as_deref(), Some("http://pat.test"));
        assert_eq!(read.nip05.as_deref(), Some("pat@openagents.com"));
        assert_eq!(read.lud16, None);
        assert_eq!(read.bot, Some(false));
        assert_eq!(Metadata::read("not json"), Metadata::default());
        assert_eq!(Metadata::read(r#"{"name":5}"#).name, None);
        let long = format!(r#"{{"name":"{}"}}"#, "a".repeat(65));
        assert_eq!(Metadata::read(&long).name, None);
    }

    #[test]
    fn javascript_and_data_urls_never_read() {
        for bad in [
            "javascript:alert(1)",
            "data:image/png;base64,AA",
            "https://",
            "//x.test/p",
        ] {
            let read = Metadata::read(&format!(r#"{{"picture":"{bad}"}}"#));
            assert_eq!(read.picture, None, "{bad}");
        }
    }

    #[test]
    fn an_edit_keeps_fields_it_does_not_touch() {
        let edit = Metadata {
            name: Some("pat".into()),
            about: Some(String::new()),
            ..Metadata::default()
        };
        let content = edit.merged_content(Some(
            r#"{"name":"old","about":"bye","birthday":{"year":1}}"#,
        ));
        let value: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value["name"], "pat");
        assert!(value.get("about").is_none(), "an empty field clears it");
        assert_eq!(value["birthday"]["year"], 1);
        let event = edit.sign(&signer(), 10, None);
        assert_eq!(event.kind, 0);
        event.validate_crypto().unwrap();
        let (read, at) = profile_of(&event.pubkey, std::slice::from_ref(&event)).unwrap();
        assert_eq!(read.name.as_deref(), Some("pat"));
        assert_eq!(at, 10);
    }

    #[test]
    fn checks_name_the_field() {
        let mut edit = Metadata {
            picture: Some("http://x.test/p.png".into()),
            ..Metadata::default()
        };
        assert_eq!(edit.check(), Err("Use an https:// link for your picture."));
        edit.picture = Some("https://x.test/p.png".into());
        assert_eq!(edit.check(), Ok(()));
        edit.name = Some("a".repeat(65));
        assert!(edit.check().is_err());
    }

    #[test]
    fn follows_are_valid_keys_once_each() {
        let a = "a".repeat(64);
        let tags = vec![
            Tag::new(vec!["p".into(), a.clone()]),
            Tag::new(vec!["p".into(), a.to_uppercase()]),
            Tag::new(vec!["p".into(), "short".into()]),
            Tag::new(vec!["e".into(), "b".repeat(64)]),
            Tag::new(vec!["p".into(), "c".repeat(64), "wss://r.test".into()]),
        ];
        let event = signer().sign(1, 3, tags, String::new());
        assert_eq!(follows(&event), vec![a, "c".repeat(64)]);
    }
}
