//! NIP-59 gift wrap.
//!
//! A rumor is an unsigned event of any kind. The author seals it as kind
//! `13` with NIP-44 and no `p` tag. An ephemeral key wraps that seal as kind
//! `1059` or kind `21059` with one `p` tag. Kind `21059` is ephemeral, so
//! the relay does not store it. Kind `1059` is stored and served only to
//! the authenticated reader named by the tag.
//!
//! The relay does not decrypt. A kind `5` from that reader removes stored
//! wraps addressed to them. Proof of work is not required.

use std::str::FromStr;

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use super::{DomainError, Event, Tag};
use crate::nip44;

const SEAL_KIND: u16 = 13;
const WRAP_KIND: u16 = 1_059;
const EPHEMERAL_WRAP_KIND: u16 = 21_059;
/// A seal or wrap timestamp may sit at most this far before the rumor.
pub const RANDOMIZE_WINDOW_SECONDS: u64 = 2 * 24 * 60 * 60;

/// An unsigned rumor. It has an id and no signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rumor {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Tag>,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RumorWire {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: String,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_lower_hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn xonly(hex: &str) -> Result<XOnlyPublicKey, DomainError> {
    if !is_lower_hex(hex, 32) {
        return Err(invalid("a pubkey must be 32 lowercase hex bytes"));
    }
    XOnlyPublicKey::from_str(hex).map_err(|_| invalid("a pubkey is not a valid x-only key"))
}

fn pubkey_hex(secret: &SecretKey) -> String {
    let secp = Secp256k1::signing_only();
    Keypair::from_secret_key(&secp, secret)
        .x_only_public_key()
        .0
        .to_string()
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

impl Rumor {
    /// Build a rumor and set its id from the NIP-01 preimage. There is no signature.
    ///
    /// # Errors
    ///
    /// Returns a sentence when `pubkey` is not 32 lowercase hex bytes on the curve.
    pub fn new(
        pubkey: &str,
        created_at: u64,
        kind: u16,
        tags: Vec<Tag>,
        content: String,
    ) -> Result<Self, DomainError> {
        xonly(pubkey)?;
        let draft = Event {
            id: "0".repeat(64),
            pubkey: pubkey.to_owned(),
            created_at,
            kind,
            tags,
            content,
            sig: "0".repeat(128),
        };
        let id = draft.computed_id()?;
        Ok(Self {
            id,
            pubkey: draft.pubkey,
            created_at,
            kind,
            tags: draft.tags,
            content: draft.content,
        })
    }

    fn to_json(&self) -> Result<String, DomainError> {
        serde_json::to_string(&RumorWire {
            id: self.id.clone(),
            pubkey: self.pubkey.clone(),
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags.clone(),
            content: self.content.clone(),
        })
        .map_err(|error| DomainError::Serialization(error.to_string()))
    }

    fn from_json(text: &str) -> Result<Self, DomainError> {
        let wire: RumorWire = serde_json::from_str(text)
            .map_err(|_| invalid("a rumor must be an unsigned event object"))?;
        let rumor = Self::new(
            &wire.pubkey,
            wire.created_at,
            wire.kind,
            wire.tags,
            wire.content,
        )?;
        if rumor.id != wire.id {
            return Err(invalid("a rumor id does not match its content"));
        }
        Ok(rumor)
    }
}

/// Move a timestamp earlier by at most two days. The result stays in the past
/// relative to `rumor_time`.
///
/// # Errors
///
/// Returns a sentence when the offset is larger than two days or underflows.
pub fn randomized_timestamp(rumor_time: u64, seconds_earlier: u64) -> Result<u64, DomainError> {
    if seconds_earlier > RANDOMIZE_WINDOW_SECONDS {
        return Err(invalid(
            "a gift wrap timestamp may move at most two days earlier",
        ));
    }
    rumor_time
        .checked_sub(seconds_earlier)
        .ok_or_else(|| invalid("a gift wrap timestamp underflows"))
}

fn seal_tags(expiration: Option<u64>) -> Vec<Tag> {
    match expiration {
        Some(expiration) => vec![Tag::new(vec!["expiration".into(), expiration.to_string()])],
        None => Vec::new(),
    }
}

/// Seal a rumor to `recipient`. The rumor pubkey must be the author's.
///
/// # Errors
///
/// Returns a sentence when the author does not match or encryption fails.
pub fn seal_rumor(
    rumor: &Rumor,
    author: &SecretKey,
    recipient: &XOnlyPublicKey,
    created_at: u64,
    nonce: [u8; 32],
    expiration: Option<u64>,
) -> Result<Event, DomainError> {
    if rumor.pubkey != pubkey_hex(author) {
        return Err(invalid("the rumor pubkey must match the seal author"));
    }
    let payload = nip44::encrypt(
        &rumor.to_json()?,
        &nip44::conversation_key(author, recipient),
        nonce,
    )
    .map_err(|reason| invalid(&reason))?;
    Ok(sign(
        author,
        created_at,
        SEAL_KIND,
        seal_tags(expiration),
        payload,
    ))
}

/// Wrap a signed seal as kind `1059` or kind `21059` under `wrapper`.
///
/// # Errors
///
/// Returns a sentence when the kind, the seal, or the ciphertext is refused.
pub fn wrap_seal(
    seal_event: &Event,
    wrapper: &SecretKey,
    recipient: &XOnlyPublicKey,
    created_at: u64,
    nonce: [u8; 32],
    kind: u16,
) -> Result<Event, DomainError> {
    if !matches!(kind, WRAP_KIND | EPHEMERAL_WRAP_KIND) {
        return Err(invalid("a gift wrap has kind 1059 or 21059"));
    }
    if seal_event.kind != SEAL_KIND {
        return Err(invalid("a gift wrap carries a kind 13 seal"));
    }
    seal_event
        .validate_crypto()
        .map_err(|_| invalid("a gift wrap carries a signed seal"))?;
    let payload = serde_json::to_string(seal_event)
        .map_err(|error| DomainError::Serialization(error.to_string()))?;
    let content = nip44::encrypt(
        &payload,
        &nip44::conversation_key(wrapper, recipient),
        nonce,
    )
    .map_err(|reason| invalid(&reason))?;
    Ok(sign(
        wrapper,
        created_at,
        kind,
        vec![Tag::new(vec!["p".into(), recipient.to_string()])],
        content,
    ))
}

/// Kind `13` admission. Tags are empty, or one expiration. Content is NIP-44.
///
/// # Errors
///
/// Returns a sentence when the kind, tags, or ciphertext framing is refused.
pub fn validate_seal(event: &Event) -> Result<(), DomainError> {
    if event.kind != SEAL_KIND {
        return Err(invalid("a seal has kind 13"));
    }
    match event.tags.as_slice() {
        [] => {}
        [tag]
            if tag.name() == Some("expiration")
                && tag
                    .value()
                    .is_some_and(|value| value.parse::<u64>().is_ok()) => {}
        _ => {
            return Err(invalid("a seal has no tags, or one expiration tag"));
        }
    }
    nip44::payload_shape(&event.content)
        .map_err(|_| invalid("seal content must be NIP-44 ciphertext"))?;
    Ok(())
}

/// Kind `1059` or `21059` admission. One `p` tag and NIP-44 framing.
///
/// # Errors
///
/// Returns a sentence when the kind, recipient, or ciphertext framing is refused.
pub fn validate_wrap(event: &Event) -> Result<(), DomainError> {
    if !matches!(event.kind, WRAP_KIND | EPHEMERAL_WRAP_KIND) {
        return Err(invalid("a gift wrap has kind 1059 or 21059"));
    }
    let recipients = event.tag_values("p").collect::<Vec<_>>();
    if recipients.len() != 1 || !is_lower_hex(recipients[0], 32) {
        return Err(DomainError::InvalidHex {
            field: "gift wrap recipient",
            expected_bytes: 32,
        });
    }
    nip44::payload_shape(&event.content)
        .map_err(|_| invalid("gift wrap content must be NIP-44 ciphertext"))?;
    Ok(())
}

/// True when `event` is a wrap addressed only to `requester`.
///
/// A kind `5` or vanish request from that reader removes the wrap even
/// though the wrap is signed by a one-time key.
pub fn recipient_removed_wrap(event: &Event, requester: &str) -> bool {
    if !matches!(event.kind, WRAP_KIND | EPHEMERAL_WRAP_KIND) {
        return false;
    }
    let recipients = event.tag_values("p").collect::<Vec<_>>();
    recipients.len() == 1 && recipients[0] == requester
}

/// Decrypt a wrap addressed to `reader` and return the rumor.
///
/// # Errors
///
/// Returns a sentence when the wrap, seal, or rumor does not match.
pub fn open_wrap(event: &Event, reader: &SecretKey) -> Result<Rumor, DomainError> {
    validate_wrap(event)?;
    let reader_hex = pubkey_hex(reader);
    if event.tag_values("p").next() != Some(reader_hex.as_str()) {
        return Err(invalid("the gift wrap is addressed to someone else"));
    }
    let wrap_key = xonly(&event.pubkey)?;
    let seal_json = nip44::decrypt(&event.content, &nip44::conversation_key(reader, &wrap_key))
        .map_err(|_| invalid("a gift wrap cannot be decrypted"))?;
    let seal_event: Event = serde_json::from_str(&seal_json)
        .map_err(|_| invalid("a gift wrap must contain a seal event"))?;
    validate_seal(&seal_event)?;
    seal_event
        .validate_crypto()
        .map_err(|_| invalid("a seal signature does not verify"))?;
    let author = xonly(&seal_event.pubkey)?;
    let rumor_json = nip44::decrypt(
        &seal_event.content,
        &nip44::conversation_key(reader, &author),
    )
    .map_err(|_| invalid("a seal cannot be decrypted"))?;
    let rumor = Rumor::from_json(&rumor_json)?;
    if rumor.pubkey != seal_event.pubkey {
        return Err(invalid("the rumor pubkey must match the seal author"));
    }
    Ok(rumor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DeletionRequest, EventClass, RelaySigner, Tag};
    use crate::nip17;

    fn secret_hex(hex: &str) -> SecretKey {
        SecretKey::from_str(hex).expect("pinned secret")
    }

    #[test]
    fn a_rumor_is_sealed_and_the_ephemeral_wrap_is_not_stored() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/59.md"
        ))
        .unwrap();
        assert!(text.contains("Are you going to the party tonight?"));
        assert!(text.contains("kind:1059"));
        assert!(text.contains("kind:21059"));
        assert!(text.contains("0beebd062ec8735f4243466049d7747ef5d6594ee838de147f8aab842b15e273"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "59.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "59.md")
        );
        assert_eq!(RANDOMIZE_WINDOW_SECONDS, nip17::RANDOMIZE_WINDOW_SECONDS);

        let author = secret_hex("0beebd062ec8735f4243466049d7747ef5d6594ee838de147f8aab842b15e273");
        let recipient =
            secret_hex("e108399bd8424357a710b606ae0c13166d853d327e47a6e5e038197346bdbf45");
        let wrapper =
            secret_hex("4f02eac59266002db5801adc5270700ca69d5b8f761d8732fab2fbf233c90cbd");
        let author_hex = pubkey_hex(&author);
        let recipient_hex = pubkey_hex(&recipient);
        assert_eq!(
            author_hex,
            "611df01bfcf85c26ae65453b772d8f1dfd25c264621c0277e1fc1518686faef9"
        );
        assert_eq!(
            recipient_hex,
            "166bf3765ebd1fc55decfe395beff2ea3b2a4e0a8946e7eb578512b555737c99"
        );
        assert_eq!(
            pubkey_hex(&wrapper),
            "18b1a75918f1f2c90c23da616bce317d36e348bcf5f7ba55e75949319210c87c"
        );

        let rumor_time = 1_691_518_405;
        let rumor = Rumor::new(
            &author_hex,
            rumor_time,
            1,
            Vec::new(),
            "Are you going to the party tonight?".into(),
        )
        .unwrap();
        assert_eq!(
            rumor.id,
            "9dd003c6d3b73b74a85a9ab099469ce251653a7af76f523671ab828acd2a0ef9"
        );
        let seal_time = randomized_timestamp(rumor_time, 1_000).unwrap();
        assert!(randomized_timestamp(rumor_time, RANDOMIZE_WINDOW_SECONDS + 1).is_err());
        let sealed = seal_rumor(
            &rumor,
            &author,
            &xonly(&recipient_hex).unwrap(),
            seal_time,
            [7_u8; 32],
            None,
        )
        .unwrap();
        sealed.validate_structure().unwrap();
        assert!(sealed.tags.is_empty());

        let stored = wrap_seal(
            &sealed,
            &wrapper,
            &xonly(&recipient_hex).unwrap(),
            randomized_timestamp(rumor_time, 2_000).unwrap(),
            [8_u8; 32],
            WRAP_KIND,
        )
        .unwrap();
        stored.validate_structure().unwrap();
        assert_eq!(stored.class(), EventClass::Regular);
        assert_eq!(open_wrap(&stored, &recipient).unwrap(), rumor);
        assert!(open_wrap(&stored, &author).is_err());

        let live = wrap_seal(
            &sealed,
            &wrapper,
            &xonly(&recipient_hex).unwrap(),
            randomized_timestamp(rumor_time, 3_000).unwrap(),
            [9_u8; 32],
            EPHEMERAL_WRAP_KIND,
        )
        .unwrap();
        live.validate_structure().unwrap();
        assert_eq!(live.class(), EventClass::Ephemeral);
        assert_eq!(live.gift_wrap_recipient(), Some(recipient_hex.as_str()));
        assert_eq!(open_wrap(&live, &recipient).unwrap().content, rumor.content);

        let mut extra = live.clone();
        extra
            .tags
            .push(Tag::new(vec!["p".into(), author_hex.clone()]));
        assert!(validate_wrap(&extra).is_err());
        let tagged_seal = sign(
            &author,
            seal_time,
            SEAL_KIND,
            vec![Tag::new(vec!["p".into(), recipient_hex.clone()])],
            sealed.content.clone(),
        );
        assert!(validate_seal(&tagged_seal).is_err());

        let request = RelaySigner::from_secret_hex(
            "e108399bd8424357a710b606ae0c13166d853d327e47a6e5e038197346bdbf45",
        )
        .unwrap()
        .sign(rumor_time, 5, Vec::new(), String::new());
        let request = DeletionRequest::from_event(&request).unwrap();
        assert!(request.deletes(&stored));
        assert!(request.deletes(&live));
        let stranger = RelaySigner::from_secret_hex(&"59".repeat(32))
            .unwrap()
            .sign(rumor_time, 5, Vec::new(), String::new());
        assert!(
            !DeletionRequest::from_event(&stranger)
                .unwrap()
                .deletes(&stored)
        );
    }
}
