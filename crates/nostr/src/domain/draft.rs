//! NIP-37 draft wraps.
//!
//! Kind `31234` stores an unsigned draft encrypted to the author's own key.
//! One `d` tag identifies the draft and one `k` tag names its kind. An empty
//! `content` deletes that draft. Kind `1234` is a checkpoint of one wrap.
//! Kind `10013` is the replaceable list of relays for that private content.
//!
//! The relay does not decrypt. It checks the tags and that a non-empty
//! content has NIP-44 framing. A private-storage relay is not required to
//! demand authentication by this module.

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use crate::nip44;

use super::{DomainError, Event, ReplacementAddress, Tag};
use std::str::FromStr;

const WRAP_KIND: u16 = 31_234;
const CHECKPOINT_KIND: u16 = 1_234;
const RELAY_LIST_KIND: u16 = 10_013;
/// The pinned recommendation for a draft expiration, measured from now.
pub const RECOMMENDED_DRAFT_TTL_SECONDS: u64 = 90 * 24 * 60 * 60;

/// An unsigned draft of any kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsignedDraft {
    pub kind: u16,
    pub content: String,
    pub tags: Vec<Tag>,
    pub created_at: Option<u64>,
}

/// A kind `31234` wrap after the author opens it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftWrap {
    Stored {
        identifier: String,
        draft: UnsignedDraft,
    },
    Deleted {
        identifier: String,
        draft_kind: u16,
    },
}

/// A kind `1234` checkpoint of one draft wrap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub parent: ReplacementAddress,
    pub draft: UnsignedDraft,
}

/// Relays from a decrypted kind `10013` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateRelayList {
    pub relays: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftWire {
    kind: u16,
    content: String,
    tags: Vec<Tag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    created_at: Option<u64>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn xonly(secret: &SecretKey) -> XOnlyPublicKey {
    let secp = Secp256k1::signing_only();
    Keypair::from_secret_key(&secp, secret)
        .x_only_public_key()
        .0
}

fn self_key(secret: &SecretKey) -> [u8; 32] {
    nip44::conversation_key(secret, &xonly(secret))
}

fn sign(secret: &SecretKey, created_at: u64, kind: u16, tags: Vec<Tag>, content: String) -> Event {
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content,
        sig: "0".repeat(128),
    };
    let id = event
        .computed_id_bytes()
        .expect("serializing an owned event cannot fail");
    event.id = event
        .computed_id()
        .expect("serializing an owned event cannot fail");
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

fn one<'a>(tags: &'a [Tag], name: &str) -> Result<Option<&'a str>, DomainError> {
    let mut found = tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(invalid("a draft wrap repeats a single-value tag"));
    }
    tag.value()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| invalid("a draft wrap tag is empty"))
}

fn expiration(tags: &[Tag]) -> Result<(), DomainError> {
    let mut found = tags.iter().filter(|tag| tag.name() == Some("expiration"));
    let Some(tag) = found.next() else {
        return Ok(());
    };
    if found.next().is_some() {
        return Err(invalid("a draft wrap has one expiration"));
    }
    let Some(value) = tag.value() else {
        return Err(invalid("a draft expiration must be unix seconds"));
    };
    if value.parse::<u64>().is_err() {
        return Err(invalid("a draft expiration must be unix seconds"));
    }
    Ok(())
}

fn ciphertext(content: &str) -> Result<(), DomainError> {
    nip44::payload_shape(content).map_err(|_| invalid("draft content must be NIP-44 ciphertext"))
}

impl UnsignedDraft {
    fn to_json(&self) -> Result<String, DomainError> {
        serde_json::to_string(&DraftWire {
            kind: self.kind,
            content: self.content.clone(),
            tags: self.tags.clone(),
            created_at: self.created_at,
        })
        .map_err(|error| DomainError::Serialization(error.to_string()))
    }

    fn from_json(text: &str) -> Result<Self, DomainError> {
        let wire: DraftWire = serde_json::from_str(text)
            .map_err(|_| invalid("a draft must be an unsigned event object"))?;
        Ok(Self {
            kind: wire.kind,
            content: wire.content,
            tags: wire.tags,
            created_at: wire.created_at,
        })
    }
}

/// Kind `31234` admission. Empty content is a deletion. Any other content
/// must be NIP-44 framing. The relay does not decrypt it.
pub fn validate_draft_wrap(event: &Event) -> Result<(), DomainError> {
    if event.kind != WRAP_KIND {
        return Err(invalid("a draft wrap has kind 31234"));
    }
    if one(&event.tags, "d")?.is_none() {
        return Err(invalid("a draft wrap requires one d tag"));
    }
    let Some(kind) = one(&event.tags, "k")? else {
        return Err(invalid("a draft wrap requires one k tag"));
    };
    if kind.parse::<u16>().is_err() {
        return Err(invalid("a draft k tag must be an event kind"));
    }
    expiration(&event.tags)?;
    if !event.content.is_empty() {
        ciphertext(&event.content)?;
    }
    Ok(())
}

/// Encrypt `draft` to `author` and sign the kind `31234` wrap.
pub fn seal_draft(
    author: &SecretKey,
    created_at: u64,
    identifier: &str,
    draft: &UnsignedDraft,
    expiration_at: Option<u64>,
    nonce: [u8; 32],
) -> Result<Event, DomainError> {
    if identifier.is_empty() {
        return Err(invalid("a draft wrap requires one d tag"));
    }
    let mut tags = vec![
        Tag::new(vec!["d".into(), identifier.to_owned()]),
        Tag::new(vec!["k".into(), draft.kind.to_string()]),
    ];
    if let Some(expiration_at) = expiration_at {
        tags.push(Tag::new(vec![
            "expiration".into(),
            expiration_at.to_string(),
        ]));
    }
    let content = nip44::encrypt(&draft.to_json()?, &self_key(author), nonce)
        .map_err(|_| invalid("draft content must be NIP-44 ciphertext"))?;
    Ok(sign(author, created_at, WRAP_KIND, tags, content))
}

/// Decrypt a kind `31234` wrap with the author's key.
pub fn open_draft_wrap(event: &Event, author: &SecretKey) -> Result<DraftWrap, DomainError> {
    validate_draft_wrap(event)?;
    let identifier = one(&event.tags, "d")?
        .ok_or_else(|| invalid("a draft wrap requires one d tag"))?
        .to_owned();
    let draft_kind = one(&event.tags, "k")?
        .ok_or_else(|| invalid("a draft wrap requires one k tag"))?
        .parse::<u16>()
        .map_err(|_| invalid("a draft k tag must be an event kind"))?;
    if event.content.is_empty() {
        return Ok(DraftWrap::Deleted {
            identifier,
            draft_kind,
        });
    }
    if event.pubkey != xonly(author).to_string() {
        return Err(invalid("a draft wrap is encrypted to its author"));
    }
    let plaintext = nip44::decrypt(&event.content, &self_key(author))
        .map_err(|_| invalid("a draft wrap cannot be decrypted"))?;
    let draft = UnsignedDraft::from_json(&plaintext)?;
    if draft.kind != draft_kind {
        return Err(invalid("a draft kind must match the k tag"));
    }
    Ok(DraftWrap::Stored { identifier, draft })
}

/// Kind `1234` admission. The `a` tag names one kind `31234` address.
pub fn validate_checkpoint(event: &Event) -> Result<(), DomainError> {
    if event.kind != CHECKPOINT_KIND {
        return Err(invalid("a draft checkpoint has kind 1234"));
    }
    let Some(value) = one(&event.tags, "a")? else {
        return Err(invalid("a draft checkpoint requires one a tag"));
    };
    let address = ReplacementAddress::from_str(value)?;
    if address.kind != WRAP_KIND || address.identifier.is_empty() {
        return Err(invalid("a draft checkpoint addresses a kind 31234 draft"));
    }
    ciphertext(&event.content)?;
    Ok(())
}

/// Encrypt a checkpoint of `draft` under the parent draft address.
pub fn seal_checkpoint(
    author: &SecretKey,
    created_at: u64,
    identifier: &str,
    draft: &UnsignedDraft,
    nonce: [u8; 32],
) -> Result<Event, DomainError> {
    if identifier.is_empty() {
        return Err(invalid("a draft checkpoint addresses a kind 31234 draft"));
    }
    let pubkey = xonly(author).to_string();
    let address = format!("{WRAP_KIND}:{pubkey}:{identifier}");
    let content = nip44::encrypt(&draft.to_json()?, &self_key(author), nonce)
        .map_err(|_| invalid("draft content must be NIP-44 ciphertext"))?;
    Ok(sign(
        author,
        created_at,
        CHECKPOINT_KIND,
        vec![Tag::new(vec!["a".into(), address])],
        content,
    ))
}

/// Decrypt a kind `1234` checkpoint.
pub fn open_checkpoint(event: &Event, author: &SecretKey) -> Result<Checkpoint, DomainError> {
    validate_checkpoint(event)?;
    if event.pubkey != xonly(author).to_string() {
        return Err(invalid("a draft checkpoint is encrypted to its author"));
    }
    let value =
        one(&event.tags, "a")?.ok_or_else(|| invalid("a draft checkpoint requires one a tag"))?;
    let parent = ReplacementAddress::from_str(value)?;
    let plaintext = nip44::decrypt(&event.content, &self_key(author))
        .map_err(|_| invalid("a draft checkpoint cannot be decrypted"))?;
    Ok(Checkpoint {
        parent,
        draft: UnsignedDraft::from_json(&plaintext)?,
    })
}

fn relay_url(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

/// Kind `10013` admission. Relay URLs stay inside the ciphertext.
pub fn validate_private_relays(event: &Event) -> Result<(), DomainError> {
    if event.kind != RELAY_LIST_KIND {
        return Err(invalid("a private relay list has kind 10013"));
    }
    if event.tags.iter().any(|tag| tag.name() == Some("relay")) {
        return Err(invalid(
            "a private relay list keeps relay URLs in the content",
        ));
    }
    ciphertext(&event.content)?;
    Ok(())
}

/// Encrypt a kind `10013` relay list to the author.
pub fn seal_private_relays(
    author: &SecretKey,
    created_at: u64,
    relays: &[String],
    nonce: [u8; 32],
) -> Result<Event, DomainError> {
    if relays.is_empty() || relays.iter().any(|relay| !relay_url(relay)) {
        return Err(invalid(
            "a private relay list contains ws:// or wss:// URLs",
        ));
    }
    let tags: Vec<Vec<String>> = relays
        .iter()
        .map(|relay| vec!["relay".to_owned(), relay.clone()])
        .collect();
    let plaintext = serde_json::to_string(&tags)
        .map_err(|error| DomainError::Serialization(error.to_string()))?;
    let content = nip44::encrypt(&plaintext, &self_key(author), nonce)
        .map_err(|_| invalid("draft content must be NIP-44 ciphertext"))?;
    Ok(sign(
        author,
        created_at,
        RELAY_LIST_KIND,
        Vec::new(),
        content,
    ))
}

/// Decrypt a kind `10013` relay list.
pub fn open_private_relays(
    event: &Event,
    author: &SecretKey,
) -> Result<PrivateRelayList, DomainError> {
    validate_private_relays(event)?;
    if event.pubkey != xonly(author).to_string() {
        return Err(invalid("a private relay list is encrypted to its author"));
    }
    let plaintext = nip44::decrypt(&event.content, &self_key(author))
        .map_err(|_| invalid("a private relay list cannot be decrypted"))?;
    let rows: Vec<Vec<String>> = serde_json::from_str(&plaintext)
        .map_err(|_| invalid("a private relay list contains ws:// or wss:// URLs"))?;
    if rows.is_empty() {
        return Err(invalid(
            "a private relay list contains ws:// or wss:// URLs",
        ));
    }
    let mut relays = Vec::with_capacity(rows.len());
    for row in rows {
        if row.len() != 2 || row[0] != "relay" || !relay_url(&row[1]) {
            return Err(invalid(
                "a private relay list contains ws:// or wss:// URLs",
            ));
        }
        relays.push(row[1].clone());
    }
    Ok(PrivateRelayList { relays })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, ReplacementDecision, compare_replacement};

    fn author() -> SecretKey {
        SecretKey::from_byte_array([0x37; 32]).expect("secret")
    }

    #[test]
    fn a_draft_wrap_round_trips_and_a_blank_content_deletes_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/37.md"
        ))
        .unwrap();
        assert!(text.contains("31234"));
        assert!(text.contains("10013"));
        assert!(text.contains("kind:1234"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "37.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "37.md")
        );

        let author = author();
        let created_at = 1_700_000_000;
        let draft = UnsignedDraft {
            kind: 30_023,
            content: "A paragraph of the article.\n\nAnother paragraph.".into(),
            tags: vec![Tag::new(vec!["d".into(), "lorem-ipsum".into()])],
            created_at: Some(created_at),
        };
        let wrap = seal_draft(
            &author,
            created_at,
            "article-1",
            &draft,
            Some(created_at + RECOMMENDED_DRAFT_TTL_SECONDS),
            [7_u8; 32],
        )
        .unwrap();
        wrap.validate_structure().unwrap();
        assert_eq!(wrap.class(), EventClass::Addressable);
        assert!(wrap.tags.iter().any(|tag| tag.name() == Some("expiration")));
        match open_draft_wrap(&wrap, &author).unwrap() {
            DraftWrap::Stored {
                identifier,
                draft: opened,
            } => {
                assert_eq!(identifier, "article-1");
                assert_eq!(opened, draft);
            }
            DraftWrap::Deleted { .. } => panic!("the wrap still holds the draft"),
        }
        let stranger = SecretKey::from_byte_array([0x38; 32]).unwrap();
        assert!(open_draft_wrap(&wrap, &stranger).is_err());

        let revised = UnsignedDraft {
            content: "Revised.".into(),
            ..draft.clone()
        };
        let newer = seal_draft(
            &author,
            created_at + 10,
            "article-1",
            &revised,
            None,
            [8_u8; 32],
        )
        .unwrap();
        assert_eq!(
            compare_replacement(&wrap, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let deleted = sign(
            &author,
            created_at + 20,
            WRAP_KIND,
            vec![
                Tag::new(vec!["d".into(), "article-1".into()]),
                Tag::new(vec!["k".into(), "30023".into()]),
            ],
            String::new(),
        );
        deleted.validate_structure().unwrap();
        assert_eq!(
            open_draft_wrap(&deleted, &author).unwrap(),
            DraftWrap::Deleted {
                identifier: "article-1".into(),
                draft_kind: 30_023,
            }
        );
        assert_eq!(
            compare_replacement(&newer, &deleted).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let missing_kind = sign(
            &author,
            created_at,
            WRAP_KIND,
            vec![Tag::new(vec!["d".into(), "article-1".into()])],
            String::new(),
        );
        assert!(missing_kind.validate_structure().is_err());
        let plaintext = sign(
            &author,
            created_at,
            WRAP_KIND,
            vec![
                Tag::new(vec!["d".into(), "article-1".into()]),
                Tag::new(vec!["k".into(), "30023".into()]),
            ],
            "not ciphertext".into(),
        );
        assert!(plaintext.validate_structure().is_err());

        let checkpoint =
            seal_checkpoint(&author, created_at + 5, "article-1", &draft, [9_u8; 32]).unwrap();
        checkpoint.validate_structure().unwrap();
        assert_eq!(checkpoint.class(), EventClass::Regular);
        let checkpoint = open_checkpoint(&checkpoint, &author).unwrap();
        assert_eq!(checkpoint.draft, draft);
        assert_eq!(checkpoint.parent.kind, WRAP_KIND);
        assert_eq!(checkpoint.parent.identifier, "article-1");

        let relays = seal_private_relays(
            &author,
            created_at,
            &["wss://myrelay.example".into(), "wss://other.example".into()],
            [10_u8; 32],
        )
        .unwrap();
        relays.validate_structure().unwrap();
        assert_eq!(relays.class(), EventClass::Replaceable);
        assert!(relays.tags.is_empty());
        assert_eq!(
            open_private_relays(&relays, &author).unwrap().relays,
            vec![
                "wss://myrelay.example".to_owned(),
                "wss://other.example".to_owned()
            ]
        );
        let leaked = sign(
            &author,
            created_at,
            RELAY_LIST_KIND,
            vec![Tag::new(vec![
                "relay".into(),
                "wss://public.example".into(),
            ])],
            relays.content.clone(),
        );
        assert!(leaked.validate_structure().is_err());
    }
}
