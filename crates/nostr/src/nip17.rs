//! NIP-17 private direct messages.
//!
//! A rumor is an unsigned event. The author seals it as kind `13` with
//! NIP-44, then gift-wraps that seal as kind `1059` under a one-time key.
//! The wrap carries one `p` tag. The reader decrypts both layers and
//! refuses the rumor when its pubkey differs from the seal.
//!
//! The relay does not decrypt a wrap. It requires the one `p` tag and
//! serves the event only to that authenticated reader. Kind `10050` is the
//! replaceable list of relays where that reader accepts messages.
//!
//! The caller supplies the one-time wrapper key so a test can repeat. File
//! bytes are not downloaded or decrypted. Kind `21059` belongs to NIP-59.

use std::collections::BTreeSet;
use std::str::FromStr;

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use crate::domain::{DomainError, Event, Tag};
use crate::nip44;

const SEAL_KIND: u16 = 13;
const CHAT_KIND: u16 = 14;
const FILE_KIND: u16 = 15;
const GIFT_WRAP_KIND: u16 = 1_059;
const INBOX_KIND: u16 = 10_050;
/// A wrap or seal timestamp may sit at most this far before the rumor.
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

/// A kind `14` chat message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub content: String,
    pub receivers: Vec<String>,
    pub reply_to: Option<String>,
    pub subject: Option<String>,
}

/// A kind `15` file message. The bytes behind `url` stay remote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMessage {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub url: String,
    pub receivers: Vec<String>,
    pub file_type: String,
    pub decryption_key: String,
    pub decryption_nonce: String,
    pub encrypted_sha256: String,
    pub plain_sha256: String,
    pub size: Option<u64>,
    pub dimensions: Option<(u64, u64)>,
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

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
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
    /// Build a rumor and set its id from the NIP-01 preimage.
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

fn tag_value<'a>(tags: &'a [Tag], name: &str) -> Result<Option<&'a str>, DomainError> {
    let mut values = tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(invalid("a private message repeats a single-value tag"));
    }
    tag.value()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| invalid("a private message tag is empty"))
}

fn receivers(tags: &[Tag]) -> Result<Vec<String>, DomainError> {
    let mut found = Vec::new();
    for tag in tags.iter().filter(|tag| tag.name() == Some("p")) {
        let slice = tag.as_slice();
        if slice.len() < 2 || xonly(&slice[1]).is_err() {
            return Err(invalid("a chat receiver must be a pubkey"));
        }
        if let Some(relay) = slice.get(2)
            && !relay.is_empty()
            && !is_relay(relay)
        {
            return Err(invalid("a chat relay must be ws:// or wss://"));
        }
        found.push(slice[1].clone());
    }
    if found.is_empty() {
        return Err(invalid("a private message names at least one receiver"));
    }
    Ok(found)
}

fn reply_to(tags: &[Tag]) -> Result<Option<String>, DomainError> {
    let mut replies = tags.iter().filter(|tag| tag.name() == Some("e"));
    let Some(tag) = replies.next() else {
        return Ok(None);
    };
    if replies.next().is_some() {
        return Err(invalid("a private message has one reply parent"));
    }
    let Some(id) = tag.value() else {
        return Err(invalid("a reply parent must be an event id"));
    };
    if !is_lower_hex(id, 32) {
        return Err(invalid("a reply parent must be an event id"));
    }
    if let Some(relay) = tag.as_slice().get(2)
        && !relay.is_empty()
        && !is_relay(relay)
    {
        return Err(invalid("a reply relay must be ws:// or wss://"));
    }
    Ok(Some(id.to_owned()))
}

/// The room is the author plus every `p` tag. Adding or removing a member
/// produces a different set.
pub fn chat_room(author: &str, receivers: &[String]) -> BTreeSet<String> {
    let mut room = BTreeSet::from([author.to_owned()]);
    room.extend(receivers.iter().cloned());
    room
}

/// Kind `14`. Content stays plain text, including an empty string.
pub fn chat_message(rumor: &Rumor) -> Result<ChatMessage, DomainError> {
    if rumor.kind != CHAT_KIND {
        return Err(invalid("a chat message has kind 14"));
    }
    Ok(ChatMessage {
        id: rumor.id.clone(),
        pubkey: rumor.pubkey.clone(),
        created_at: rumor.created_at,
        content: rumor.content.clone(),
        receivers: receivers(&rumor.tags)?,
        reply_to: reply_to(&rumor.tags)?,
        subject: tag_value(&rumor.tags, "subject")?.map(str::to_owned),
    })
}

/// Build a kind `14` rumor.
pub fn chat_rumor(
    author: &str,
    created_at: u64,
    content: &str,
    tags: Vec<Tag>,
) -> Result<Rumor, DomainError> {
    let rumor = Rumor::new(author, created_at, CHAT_KIND, tags, content.to_owned())?;
    chat_message(&rumor)?;
    Ok(rumor)
}

fn required_tag<'a>(tags: &'a [Tag], name: &str) -> Result<&'a str, DomainError> {
    tag_value(tags, name)?.ok_or_else(|| invalid("a file message is missing a required tag"))
}

fn http_url(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && !value.chars().any(char::is_whitespace)
}

fn dimensions(value: &str) -> Result<(u64, u64), DomainError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(invalid("file dimensions are widthxheight"));
    };
    let width: u64 = width
        .parse()
        .map_err(|_| invalid("file dimensions are widthxheight"))?;
    let height: u64 = height
        .parse()
        .map_err(|_| invalid("file dimensions are widthxheight"))?;
    if width == 0 || height == 0 {
        return Err(invalid("file dimensions are widthxheight"));
    }
    Ok((width, height))
}

/// Kind `15`. The algorithm must be `aes-gcm`.
pub fn file_message(rumor: &Rumor) -> Result<FileMessage, DomainError> {
    if rumor.kind != FILE_KIND {
        return Err(invalid("a file message has kind 15"));
    }
    if !http_url(&rumor.content) {
        return Err(invalid("a file message content is an http or https URL"));
    }
    let file_type = required_tag(&rumor.tags, "file-type")?;
    let Some((left, right)) = file_type.split_once('/') else {
        return Err(invalid("a file type is a media type"));
    };
    if left.is_empty() || right.is_empty() {
        return Err(invalid("a file type is a media type"));
    }
    if required_tag(&rumor.tags, "encryption-algorithm")? != "aes-gcm" {
        return Err(invalid("a file message uses aes-gcm"));
    }
    let encrypted_sha256 = required_tag(&rumor.tags, "x")?;
    let plain_sha256 = required_tag(&rumor.tags, "ox")?;
    if !is_lower_hex(encrypted_sha256, 32) || !is_lower_hex(plain_sha256, 32) {
        return Err(invalid("a file hash must be 32 lowercase hex bytes"));
    }
    let size = match tag_value(&rumor.tags, "size")? {
        None => None,
        Some(value) => Some(
            value
                .parse()
                .map_err(|_| invalid("a file size must be a non-negative integer"))?,
        ),
    };
    let dimensions = match tag_value(&rumor.tags, "dim")? {
        None => None,
        Some(value) => Some(dimensions(value)?),
    };
    Ok(FileMessage {
        id: rumor.id.clone(),
        pubkey: rumor.pubkey.clone(),
        created_at: rumor.created_at,
        url: rumor.content.clone(),
        receivers: receivers(&rumor.tags)?,
        file_type: file_type.to_owned(),
        decryption_key: required_tag(&rumor.tags, "decryption-key")?.to_owned(),
        decryption_nonce: required_tag(&rumor.tags, "decryption-nonce")?.to_owned(),
        encrypted_sha256: encrypted_sha256.to_owned(),
        plain_sha256: plain_sha256.to_owned(),
        size,
        dimensions,
    })
}

/// Move a timestamp earlier by at most two days.
pub fn hidden_timestamp(rumor_time: u64, seconds_earlier: u64) -> Result<u64, DomainError> {
    if seconds_earlier > RANDOMIZE_WINDOW_SECONDS {
        return Err(invalid(
            "a gift wrap timestamp may move at most two days earlier",
        ));
    }
    rumor_time
        .checked_sub(seconds_earlier)
        .ok_or_else(|| invalid("a gift wrap timestamp underflows"))
}

/// Seal a rumor to one reader. The rumor pubkey must be the author's.
pub fn seal(
    rumor: &Rumor,
    author: &SecretKey,
    reader: &XOnlyPublicKey,
    created_at: u64,
    nonce: [u8; 32],
    expiration: Option<u64>,
) -> Result<Event, DomainError> {
    if rumor.pubkey != pubkey_hex(author) {
        return Err(invalid("the rumor pubkey must match the seal author"));
    }
    let mut tags = Vec::new();
    if let Some(expiration) = expiration {
        tags.push(Tag::new(vec!["expiration".into(), expiration.to_string()]));
    }
    let payload = nip44::encrypt(
        &rumor.to_json()?,
        &nip44::conversation_key(author, reader),
        nonce,
    )
    .map_err(|reason| invalid(&reason))?;
    Ok(sign(author, created_at, SEAL_KIND, tags, payload))
}

/// Gift-wrap a seal to one reader under the one-time key.
pub fn gift_wrap(
    seal_event: &Event,
    wrapper: &SecretKey,
    reader: &XOnlyPublicKey,
    created_at: u64,
    nonce: [u8; 32],
    expiration: Option<u64>,
) -> Result<Event, DomainError> {
    if seal_event.kind != SEAL_KIND {
        return Err(invalid("a gift wrap carries a kind 13 seal"));
    }
    seal_event
        .validate_crypto()
        .map_err(|_| invalid("a gift wrap carries a signed seal"))?;
    let mut tags = vec![Tag::new(vec!["p".into(), reader.to_string()])];
    if let Some(expiration) = expiration {
        tags.push(Tag::new(vec!["expiration".into(), expiration.to_string()]));
    }
    let payload = serde_json::to_string(seal_event)
        .map_err(|error| DomainError::Serialization(error.to_string()))?;
    let content = nip44::encrypt(&payload, &nip44::conversation_key(wrapper, reader), nonce)
        .map_err(|reason| invalid(&reason))?;
    Ok(sign(wrapper, created_at, GIFT_WRAP_KIND, tags, content))
}

fn seal_expiration(event: &Event) -> Result<(), DomainError> {
    if event.tags.is_empty() {
        return Ok(());
    }
    if event.tags.len() == 1 && event.tags[0].name() == Some("expiration") {
        let Some(value) = event.tags[0].value() else {
            return Err(invalid("a seal expiration must be a timestamp"));
        };
        if value.parse::<u64>().is_err() {
            return Err(invalid("a seal expiration must be a timestamp"));
        }
        return Ok(());
    }
    Err(invalid("a seal has no tags, or one expiration tag"))
}

/// Decrypt a gift wrap addressed to `reader` and return the rumor.
///
/// The wrap's `p` tag must be this reader. The rumor pubkey must equal the
/// seal pubkey.
pub fn open_direct_message(wrap: &Event, reader: &SecretKey) -> Result<Rumor, DomainError> {
    validate_gift_wrap(wrap)?;
    let reader_hex = pubkey_hex(reader);
    if wrap.gift_wrap_recipient() != Some(reader_hex.as_str()) {
        return Err(invalid("the gift wrap is addressed to someone else"));
    }
    let wrap_key = xonly(&wrap.pubkey)?;
    let seal_json = nip44::decrypt(&wrap.content, &nip44::conversation_key(reader, &wrap_key))
        .map_err(|reason| invalid(&reason))?;
    let seal_event: Event = serde_json::from_str(&seal_json)
        .map_err(|_| invalid("a gift wrap must contain a seal event"))?;
    if seal_event.kind != SEAL_KIND {
        return Err(invalid("a gift wrap must contain a kind 13 seal"));
    }
    seal_event
        .validate_crypto()
        .map_err(|_| invalid("a seal signature does not verify"))?;
    seal_expiration(&seal_event)?;
    let author = xonly(&seal_event.pubkey)?;
    let rumor_json = nip44::decrypt(
        &seal_event.content,
        &nip44::conversation_key(reader, &author),
    )
    .map_err(|reason| invalid(&reason))?;
    let rumor = Rumor::from_json(&rumor_json)?;
    if rumor.pubkey != seal_event.pubkey {
        return Err(invalid("the rumor pubkey must match the seal author"));
    }
    Ok(rumor)
}

/// Kind `1059` admission: exactly one lowercase pubkey `p` tag.
pub fn validate_gift_wrap(event: &Event) -> Result<(), DomainError> {
    if event.kind != GIFT_WRAP_KIND {
        return Err(invalid("a gift wrap has kind 1059"));
    }
    let recipients = event.tag_values("p").collect::<Vec<_>>();
    if recipients.len() != 1 {
        return Err(invalid("gift wraps require exactly one p-tagged recipient"));
    }
    if !is_lower_hex(recipients[0], 32) {
        return Err(DomainError::InvalidHex {
            field: "gift wrap recipient",
            expected_bytes: 32,
        });
    }
    Ok(())
}

/// Kind `10050` admission: one or more `ws://` or `wss://` relay tags.
pub fn validate_inbox(event: &Event) -> Result<(), DomainError> {
    inbox_relays(event)?;
    Ok(())
}

/// Relay URLs from a kind `10050` event, in tag order.
pub fn inbox_relays(event: &Event) -> Result<Vec<String>, DomainError> {
    if event.kind != INBOX_KIND {
        return Err(invalid("an inbox list has kind 10050"));
    }
    let relays = event.tag_values("relay").collect::<Vec<_>>();
    if relays.is_empty() || relays.iter().any(|relay| !is_relay(relay)) {
        return Err(invalid(
            "kind 10050 requires valid ws:// or wss:// relay tags",
        ));
    }
    Ok(relays.into_iter().map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(byte: u8) -> SecretKey {
        SecretKey::from_byte_array([byte; 32]).expect("secret")
    }

    fn peer(secret: &SecretKey) -> XOnlyPublicKey {
        xonly(&pubkey_hex(secret)).unwrap()
    }

    #[test]
    fn a_private_message_round_trips_and_rejects_an_impersonated_rumor() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/17.md"
        ))
        .unwrap();
        assert!(text.contains("gift wrap"));
        assert!(text.contains("10050"));
        assert!(text.contains("kind 14"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "17.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "17.md")
        );

        let author = secret(0x11);
        let reader = secret(0x22);
        let wrapper = secret(0x33);
        let author_copy = secret(0x44);
        let rumor_time = 1_700_000_000;
        let rumor = chat_rumor(
            &pubkey_hex(&author),
            rumor_time,
            "Hola, que tal?",
            vec![
                Tag::new(vec![
                    "p".into(),
                    pubkey_hex(&reader),
                    "wss://inbox.example".into(),
                ]),
                Tag::new(vec!["subject".into(), "hello".into()]),
            ],
        )
        .unwrap();
        let chat = chat_message(&rumor).unwrap();
        assert_eq!(chat.content, "Hola, que tal?");
        let room = chat_room(&chat.pubkey, &chat.receivers);
        assert_eq!(room.len(), 2);
        let mut wider = chat.receivers.clone();
        wider.push(pubkey_hex(&secret(0x55)));
        assert_ne!(chat_room(&chat.pubkey, &wider), room);

        let seal_time = hidden_timestamp(rumor_time, 1_000).unwrap();
        let wrap_time = hidden_timestamp(rumor_time, 50_000).unwrap();
        assert!(hidden_timestamp(rumor_time, RANDOMIZE_WINDOW_SECONDS + 1).is_err());
        let nonce = [7_u8; 32];
        let sealed = seal(
            &rumor,
            &author,
            &peer(&reader),
            seal_time,
            nonce,
            Some(rumor_time + 86_400),
        )
        .unwrap();
        assert_eq!(sealed.kind, SEAL_KIND);
        assert!(
            sealed
                .tags
                .iter()
                .any(|tag| tag.name() == Some("expiration"))
        );
        let wrapped = gift_wrap(
            &sealed,
            &wrapper,
            &peer(&reader),
            wrap_time,
            [8_u8; 32],
            Some(rumor_time + 90_000),
        )
        .unwrap();
        wrapped.validate_structure().unwrap();
        assert_eq!(wrapped.kind, GIFT_WRAP_KIND);
        let reader_hex = pubkey_hex(&reader);
        assert_eq!(wrapped.gift_wrap_recipient(), Some(reader_hex.as_str()));
        assert_ne!(
            wrapped
                .tags
                .iter()
                .find(|tag| tag.name() == Some("expiration"))
                .and_then(Tag::value),
            sealed
                .tags
                .iter()
                .find(|tag| tag.name() == Some("expiration"))
                .and_then(Tag::value)
        );

        let opened = open_direct_message(&wrapped, &reader).unwrap();
        assert_eq!(opened, rumor);
        assert_eq!(
            chat_message(&opened).unwrap().subject.as_deref(),
            Some("hello")
        );
        assert!(open_direct_message(&wrapped, &author).is_err());

        let self_seal = seal(&rumor, &author, &peer(&author), seal_time, [9_u8; 32], None).unwrap();
        let self_wrap = gift_wrap(
            &self_seal,
            &author_copy,
            &peer(&author),
            wrap_time,
            [10_u8; 32],
            None,
        )
        .unwrap();
        assert_eq!(open_direct_message(&self_wrap, &author).unwrap(), rumor);

        let stranger = chat_rumor(
            &pubkey_hex(&reader),
            rumor_time,
            "not from the seal",
            vec![Tag::new(vec!["p".into(), pubkey_hex(&author)])],
        )
        .unwrap();
        let forged_payload = nip44::encrypt(
            &stranger.to_json().unwrap(),
            &nip44::conversation_key(&author, &peer(&reader)),
            [11_u8; 32],
        )
        .unwrap();
        let forged_seal = sign(&author, seal_time, SEAL_KIND, Vec::new(), forged_payload);
        let forged_wrap = gift_wrap(
            &forged_seal,
            &wrapper,
            &peer(&reader),
            wrap_time,
            [12_u8; 32],
            None,
        )
        .unwrap();
        assert!(open_direct_message(&forged_wrap, &reader).is_err());
        assert!(seal(&stranger, &author, &peer(&reader), seal_time, nonce, None).is_err());

        let bare = sign(
            &wrapper,
            wrap_time,
            GIFT_WRAP_KIND,
            Vec::new(),
            wrapped.content.clone(),
        );
        assert!(bare.validate_structure().is_err());

        let file = Rumor::new(
            &pubkey_hex(&author),
            rumor_time,
            FILE_KIND,
            vec![
                Tag::new(vec!["p".into(), pubkey_hex(&reader)]),
                Tag::new(vec!["file-type".into(), "image/jpeg".into()]),
                Tag::new(vec!["encryption-algorithm".into(), "aes-gcm".into()]),
                Tag::new(vec!["decryption-key".into(), "ab".repeat(16)]),
                Tag::new(vec!["decryption-nonce".into(), "cd".repeat(12)]),
                Tag::new(vec!["x".into(), "11".repeat(32)]),
                Tag::new(vec!["ox".into(), "22".repeat(32)]),
                Tag::new(vec!["size".into(), "42".into()]),
                Tag::new(vec!["dim".into(), "640x480".into()]),
            ],
            "https://cdn.example/file".into(),
        )
        .unwrap();
        let file = file_message(&file).unwrap();
        assert_eq!(file.dimensions, Some((640, 480)));
        assert_eq!(file.size, Some(42));
        let bad_algorithm = Rumor::new(
            &pubkey_hex(&author),
            rumor_time,
            FILE_KIND,
            vec![
                Tag::new(vec!["p".into(), pubkey_hex(&reader)]),
                Tag::new(vec!["file-type".into(), "image/jpeg".into()]),
                Tag::new(vec!["encryption-algorithm".into(), "aes-cbc".into()]),
                Tag::new(vec!["decryption-key".into(), "k".into()]),
                Tag::new(vec!["decryption-nonce".into(), "n".into()]),
                Tag::new(vec!["x".into(), "11".repeat(32)]),
                Tag::new(vec!["ox".into(), "22".repeat(32)]),
            ],
            "https://cdn.example/file".into(),
        )
        .unwrap();
        assert!(file_message(&bad_algorithm).is_err());

        let inbox = sign(
            &reader,
            rumor_time,
            INBOX_KIND,
            vec![
                Tag::new(vec!["relay".into(), "wss://inbox.example".into()]),
                Tag::new(vec!["relay".into(), "wss://other.example".into()]),
            ],
            String::new(),
        );
        inbox.validate_structure().unwrap();
        assert_eq!(inbox.class(), crate::domain::EventClass::Replaceable);
        assert_eq!(
            inbox_relays(&inbox).unwrap(),
            vec![
                "wss://inbox.example".to_owned(),
                "wss://other.example".to_owned()
            ]
        );
        let empty_inbox = sign(&reader, rumor_time, INBOX_KIND, Vec::new(), String::new());
        assert!(empty_inbox.validate_structure().is_err());
        let newer = sign(
            &reader,
            rumor_time + 10,
            INBOX_KIND,
            vec![Tag::new(vec!["relay".into(), "wss://inbox.example".into()])],
            String::new(),
        );
        assert_eq!(
            crate::domain::compare_replacement(&inbox, &newer).unwrap(),
            crate::domain::ReplacementDecision::ReplaceCurrent
        );
    }
}
