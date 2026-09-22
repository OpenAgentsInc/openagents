//! NIP-51 lists and sets.
//!
//! Public items ride the event's tags; private items are a JSON tag
//! array NIP-44-encrypted under the author's own conversation key and
//! stored in `content`. Standard lists are replaceable kinds with one
//! list per user; sets are addressable kinds a user holds many of, so
//! each carries a `d` identifier and may carry `title`, `image`, and
//! `description`. Kind `30007` is a kind-scoped mute set whose `d` is
//! the muted kind's number. The deprecated `d` values on kinds `30000`
//! and `30001` map to their standard replacements.
//! [`private_items`] reads only NIP-44 payloads — a legacy NIP-04
//! ciphertext (detected by its `?iv=` marker) is refused, and decrypting
//! it is a client's compatibility burden.

use secp256k1::{Keypair, Secp256k1, SecretKey};

use super::{DomainError, Event};

/// A typed list member — the first element of one public tag item or
/// one decrypted private item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListItem {
    /// A `p` pubkey with its optional relay hint and petname.
    Person {
        /// The 32-byte hex pubkey.
        pubkey: String,
        /// The optional relay hint.
        relay: Option<String>,
        /// The optional petname.
        petname: Option<String>,
    },
    /// An `e` event id.
    Event {
        /// The 32-byte hex event id.
        id: String,
    },
    /// An `a` `kind:pubkey:d` address.
    Address {
        /// The address string.
        address: String,
    },
    /// A `t` hashtag.
    Hashtag {
        /// The topic.
        topic: String,
    },
    /// A `word` mute word.
    Word {
        /// The lowercase string.
        word: String,
    },
    /// A `relay` URL.
    Relay {
        /// The relay URL.
        url: String,
    },
    /// An `emoji` shortcode and image URL pair.
    Emoji {
        /// The shortcode.
        shortcode: String,
        /// The image URL.
        url: String,
    },
    /// A `group` id, relay, and optional name (NIP-29 memberships).
    Group {
        /// The group identifier.
        id: String,
        /// The relay the group lives on.
        relay: String,
        /// The optional group name.
        name: Option<String>,
    },
    /// A `server` URL (Blossom servers, kind `10063`).
    Server {
        /// The server URL.
        url: String,
    },
    /// A `url` external URL (podcast RSS feeds).
    Url {
        /// The URL.
        url: String,
    },
    /// An `r` reference URL.
    Reference {
        /// The URL.
        url: String,
    },
    /// An item the pinned table does not name — carried, not refused.
    Other {
        /// The tag name.
        name: String,
        /// The tag's first value.
        value: String,
    },
}

impl ListItem {
    /// Classify one tag-shaped item. `fields[0]` is the tag name.
    fn from_fields(fields: &[String]) -> Option<Self> {
        let (name, rest) = fields.split_first()?;
        let value = || {
            rest.first()
                .map(String::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let item = match name.as_str() {
            "p" => Self::Person {
                pubkey: value(),
                relay: rest.get(1).cloned(),
                petname: rest.get(2).cloned(),
            },
            "e" => Self::Event { id: value() },
            "a" => Self::Address { address: value() },
            "t" => Self::Hashtag { topic: value() },
            "word" => Self::Word { word: value() },
            "relay" => Self::Relay { url: value() },
            "emoji" => Self::Emoji {
                shortcode: value(),
                url: rest.get(1).cloned().unwrap_or_default(),
            },
            "group" => Self::Group {
                id: value(),
                relay: rest.get(1).cloned().unwrap_or_default(),
                name: rest.get(2).cloned(),
            },
            "server" => Self::Server { url: value() },
            "url" => Self::Url { url: value() },
            "r" => Self::Reference { url: value() },
            other => Self::Other {
                name: other.to_string(),
                value: value(),
            },
        };
        Some(item)
    }
}

/// The encryption a list's `content` carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateItemsEncoding {
    /// Empty content — no private items.
    None,
    /// NIP-44 v2 ciphertext.
    Nip44,
    /// Deprecated NIP-04 ciphertext, detected by its `?iv=` marker.
    Nip04Legacy,
}

/// Whether a list's `content` holds private items, and under which
/// scheme. NIP-44 has no magic bytes; a non-empty content without the
/// `?iv=` NIP-04 marker is treated as NIP-44.
#[must_use]
pub fn private_items_encoding(content: &str) -> PrivateItemsEncoding {
    if content.is_empty() {
        PrivateItemsEncoding::None
    } else if content.contains("?iv=") {
        PrivateItemsEncoding::Nip04Legacy
    } else {
        PrivateItemsEncoding::Nip44
    }
}

/// The standard replaceable list kinds the pinned table defines — one
/// list per kind per user.
#[must_use]
pub fn is_standard_list(kind: u16) -> bool {
    kind == 3
        || matches!(
            kind,
            10_000..=10_020 | 10_030 | 10_050 | 10_054 | 10_063 | 10_064 | 10_101 | 10_102
        )
}

/// The addressable set kinds the pinned table defines — each carries a
/// `d` identifier. Kind `31_924` is the calendar set NIP-52 already
/// opens under `domain::calendar`.
#[must_use]
pub fn is_set_kind(kind: u16) -> bool {
    matches!(
        kind,
        30_000 | 30_002..=30_008 | 30_015 | 30_030 | 30_063 | 30_267 | 39_089 | 39_092
    )
}

/// An opened list or set: the optional `d` identifier, the set
/// decoration tags, the public items, and whether private items are
/// encrypted into the content.
#[derive(Clone, Debug)]
pub struct List {
    /// The `d` identifier — required on set kinds, absent on standard
    /// lists.
    pub identifier: Option<String>,
    /// The `title` tag, when present.
    pub title: Option<String>,
    /// The `image` tag, when present.
    pub image: Option<String>,
    /// The `description` tag, when present.
    pub description: Option<String>,
    /// The public tag items, in order.
    pub items: Vec<ListItem>,
    /// The content's private-items encryption.
    pub private: PrivateItemsEncoding,
}

/// Read any NIP-51 list or set event.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` when the kind is neither a
/// standard list nor a set kind, when a set lacks its `d` identifier,
/// or when a kind `30007` mute set's `d` is not a kind number.
pub fn open_list(event: &Event) -> Result<List, DomainError> {
    if !is_standard_list(event.kind) && !is_set_kind(event.kind) {
        return Err(invalid("the kind is not a NIP-51 list or set"));
    }
    let identifier = event.tag_values("d").next().map(str::to_string);
    if is_set_kind(event.kind) && identifier.as_deref().unwrap_or_default().is_empty() {
        return Err(invalid("a set names itself in a d tag"));
    }
    if event.kind == 30_007
        && identifier
            .as_deref()
            .is_some_and(|d| d.parse::<u16>().is_err())
    {
        return Err(invalid("a kind-mute set's d tag is the muted kind number"));
    }
    let text = |name: &str| event.tag_values(name).next().map(str::to_string);
    Ok(List {
        identifier,
        title: text("title"),
        image: text("image"),
        description: text("description"),
        items: event
            .tags
            .iter()
            .filter(|tag| !matches!(tag.name(), Some("d" | "title" | "image" | "description")))
            .filter_map(|tag| ListItem::from_fields(tag.as_slice()))
            .collect(),
        private: private_items_encoding(&event.content),
    })
}

/// The standard list kind a deprecated set shape should migrate to —
/// kind `30000` `"mute"` → `10000`, and kind `30001` `"pin"`,
/// `"bookmark"`, `"communities"` → `10001`, `10003`, `10004`.
#[must_use]
pub fn deprecated_standard_list(event: &Event) -> Option<u16> {
    let d = event.tag_values("d").next()?;
    match (event.kind, d) {
        (30_000, "mute") => Some(10_000),
        (30_001, "pin") => Some(10_001),
        (30_001, "bookmark") => Some(10_003),
        (30_001, "communities") => Some(10_004),
        _ => None,
    }
}

/// Decrypt and parse a list's private items. The conversation key is
/// the author's own — secret with own pubkey — per the pinned scheme.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` when the content is a legacy
/// NIP-04 ciphertext, the NIP-44 payload fails shape or MAC checks, or
/// the plaintext is not a JSON array of tag arrays.
pub fn private_items(event: &Event, secret: &SecretKey) -> Result<Vec<ListItem>, DomainError> {
    match private_items_encoding(&event.content) {
        PrivateItemsEncoding::None => return Ok(Vec::new()),
        PrivateItemsEncoding::Nip04Legacy => {
            return Err(invalid("private items use the deprecated NIP-04 scheme"));
        }
        PrivateItemsEncoding::Nip44 => {}
    }
    crate::nip44::payload_shape(&event.content)
        .map_err(|_| invalid("private items are not a NIP-44 v2 payload"))?;
    let secp = Secp256k1::new();
    let own = Keypair::from_secret_key(&secp, secret)
        .x_only_public_key()
        .0;
    let key = crate::nip44::conversation_key(secret, &own);
    let plaintext = crate::nip44::decrypt(&event.content, &key)
        .map_err(|_| invalid("private items fail NIP-44 authentication"))?;
    let value: serde_json::Value = serde_json::from_str(&plaintext)
        .map_err(|_| invalid("private items are a JSON tag array"))?;
    let rows = value
        .as_array()
        .ok_or_else(|| invalid("private items are a JSON tag array"))?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let fields: Vec<String> = row
            .as_array()
            .ok_or_else(|| invalid("a private item is a tag array"))?
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| invalid("a private item's fields are strings"))
            })
            .collect::<Result<_, _>>()?;
        items.push(
            ListItem::from_fields(&fields).ok_or_else(|| invalid("a private item is empty"))?,
        );
    }
    Ok(items)
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag, hex::decode_lower_hex};
    use secp256k1::SecretKey;

    const SECRET: &str = "4242424242424242424242424242424242424242424242424242424242424242";

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(SECRET).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    #[test]
    fn lists_and_sets_open_with_typed_items() {
        let mute = sign(
            10_000,
            vec![
                Tag::new(vec!["p".into(), "ab".repeat(32)]),
                Tag::new(vec!["t".into(), "spam".into()]),
                Tag::new(vec!["word".into(), "scam".into()]),
                Tag::new(vec!["e".into(), "cd".repeat(32)]),
            ],
            "",
        );
        let list = open_list(&mute).unwrap();
        assert_eq!(list.identifier, None);
        assert_eq!(list.items.len(), 4);
        assert!(matches!(list.items[0], ListItem::Person { .. }));
        assert!(matches!(list.items[2], ListItem::Word { .. }));
        assert_eq!(list.private, PrivateItemsEncoding::None);

        let set = sign(
            30_004,
            vec![
                Tag::new(vec!["d".into(), "yaks".into()]),
                Tag::new(vec!["title".into(), "Yaks".into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("30023:{}:article", "ef".repeat(32)),
                ]),
                Tag::new(vec!["e".into(), "12".repeat(32)]),
            ],
            "",
        );
        let list = open_list(&set).unwrap();
        assert_eq!(list.identifier.as_deref(), Some("yaks"));
        assert_eq!(list.title.as_deref(), Some("Yaks"));
        assert_eq!(list.items.len(), 2);
        assert!(matches!(list.items[0], ListItem::Address { .. }));

        let kind_mute = sign(
            30_007,
            vec![
                Tag::new(vec!["d".into(), "1".into()]),
                Tag::new(vec!["p".into(), "ab".repeat(32)]),
            ],
            "",
        );
        assert_eq!(
            open_list(&kind_mute).unwrap().identifier.as_deref(),
            Some("1")
        );
    }

    #[test]
    fn private_items_round_trip_through_nip44() {
        let secret =
            SecretKey::from_byte_array(decode_lower_hex::<32>(SECRET, "secret").unwrap()).unwrap();
        let secp = Secp256k1::new();
        let own = Keypair::from_secret_key(&secp, &secret)
            .x_only_public_key()
            .0;
        let key = crate::nip44::conversation_key(&secret, &own);
        let plaintext = serde_json::json!([
            [
                "p",
                "07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9"
            ],
            ["word", "nsfw"],
        ])
        .to_string();
        let content = crate::nip44::encrypt(&plaintext, &key, [7u8; 32]).unwrap();
        let mute = sign(
            10_000,
            vec![Tag::new(vec!["p".into(), "ab".repeat(32)])],
            &content,
        );
        let list = open_list(&mute).unwrap();
        assert_eq!(list.private, PrivateItemsEncoding::Nip44);
        assert_eq!(list.items.len(), 1);

        let items = private_items(&mute, &secret).unwrap();
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], ListItem::Person { .. }));
        assert!(matches!(&items[1], ListItem::Word { word } if word == "nsfw"));

        let wrong = SecretKey::from_byte_array([9u8; 32]).unwrap();
        assert!(private_items(&mute, &wrong).is_err());

        let legacy = sign(10_000, vec![], "ciphertext?iv=abc123");
        assert_eq!(
            private_items_encoding(&legacy.content),
            PrivateItemsEncoding::Nip04Legacy
        );
        assert!(private_items(&legacy, &secret).is_err());
    }

    #[test]
    fn deprecated_set_shapes_map_to_standard_lists() {
        let mute = sign(30_000, vec![Tag::new(vec!["d".into(), "mute".into()])], "");
        assert_eq!(deprecated_standard_list(&mute), Some(10_000));
        let pin = sign(30_001, vec![Tag::new(vec!["d".into(), "pin".into()])], "");
        assert_eq!(deprecated_standard_list(&pin), Some(10_001));
        let plain = sign(30_000, vec![Tag::new(vec!["d".into(), "team".into()])], "");
        assert_eq!(deprecated_standard_list(&plain), None);
    }

    #[test]
    fn malformed_lists_are_refused() {
        assert!(open_list(&sign(1, vec![], "")).is_err());
        assert!(open_list(&sign(30_000, vec![], "")).is_err());
        assert!(
            open_list(&sign(
                30_007,
                vec![Tag::new(vec!["d".into(), "not-a-kind".into()])],
                "",
            ))
            .is_err()
        );
    }
}
