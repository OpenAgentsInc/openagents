//! NIP-27 text note references.
//!
//! A `nostr:` URI inside content carries a NIP-19 code — `npub`,
//! `nprofile`, `note`, `nevent`, or `naddr` — a reader decodes for
//! preview or linking. Mention tags (`p`, `q`) are optional and stay
//! the author's choice. NIP-27 is a draft and binds reader display
//! only, so nothing changes at admission and it is not added to the
//! NIP-11 list.

use crate::nip19;

use super::{DomainError, Event};

/// A decoded `nostr:` reference inside event content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextReference {
    /// An `npub` or `nprofile` mention of an author.
    Profile { pubkey: String, relays: Vec<String> },
    /// A `note` or `nevent` mention of an event.
    Note {
        id: String,
        relays: Vec<String>,
        author: Option<String>,
        kind: Option<u16>,
    },
    /// An `naddr` mention of an addressable event.
    Address {
        address: String,
        relays: Vec<String>,
        author: String,
        kind: u16,
    },
}

/// Decode one `nostr:` entity: `npub1…`, `nprofile1…`, `note1…`,
/// `nevent1…`, or `naddr1…`.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for a bad bech32 code, an
/// unsupported prefix, or malformed TLV data.
pub fn decode_reference(entity: &str) -> Result<TextReference, DomainError> {
    let (hrp, bytes) =
        nip19::decode(entity).map_err(|_| invalid("a nostr: reference is a NIP-19 code"))?;
    match hrp.as_str() {
        "npub" => {
            let pubkey: [u8; 32] = bytes
                .try_into()
                .map_err(|_| invalid("an npub carries a pubkey"))?;
            Ok(TextReference::Profile {
                pubkey: super::hex::encode_lower_hex(&pubkey),
                relays: Vec::new(),
            })
        }
        "nprofile" => {
            let tlv = parse_tlv(&bytes)?;
            let Some(pubkey) = tlv.special_id() else {
                return Err(invalid("an nprofile carries a pubkey"));
            };
            Ok(TextReference::Profile {
                pubkey,
                relays: tlv.relays,
            })
        }
        "note" => {
            let id: [u8; 32] = bytes
                .try_into()
                .map_err(|_| invalid("a note carries an event id"))?;
            Ok(TextReference::Note {
                id: super::hex::encode_lower_hex(&id),
                relays: Vec::new(),
                author: None,
                kind: None,
            })
        }
        "nevent" => {
            let tlv = parse_tlv(&bytes)?;
            let Some(id) = tlv.special_id() else {
                return Err(invalid("a nevent carries an event id"));
            };
            Ok(TextReference::Note {
                id,
                relays: tlv.relays,
                author: tlv.author,
                kind: tlv.kind,
            })
        }
        "naddr" => {
            let tlv = parse_tlv(&bytes)?;
            let (Some(identifier), Some(author), Some(kind)) =
                (tlv.special_text(), tlv.author, tlv.kind)
            else {
                return Err(invalid("an naddr carries an identifier, author, and kind"));
            };
            Ok(TextReference::Address {
                address: format!("{kind}:{author}:{identifier}"),
                relays: tlv.relays,
                author,
                kind,
            })
        }
        _ => Err(invalid("a nostr: reference is not a readable entity")),
    }
}

/// Every decodable `nostr:` reference in an event's content. A token
/// that fails to decode is skipped — it stays text.
#[must_use]
pub fn text_references(event: &Event) -> Vec<TextReference> {
    let mut references = Vec::new();
    for token in event.content.split_whitespace() {
        let Some(entity) = token.strip_prefix("nostr:") else {
            continue;
        };
        // Strip trailing punctuation that cannot be part of bech32.
        let entity = entity.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        if let Ok(reference) = decode_reference(entity) {
            references.push(reference);
        }
    }
    references
}

/// The NIP-19 TLV records a `nprofile`, `nevent`, or `naddr` carries.
struct Tlv {
    /// Record `0`: the pubkey, event id, or identifier — entity-specific.
    special: Vec<u8>,
    /// Record `1`: relay URL hints.
    relays: Vec<String>,
    /// Record `2`: the author's pubkey.
    author: Option<String>,
    /// Record `3`: the kind, big-endian `u32`.
    kind: Option<u16>,
}

impl Tlv {
    /// The special record as a 32-byte hex id, for nprofile/nevent.
    fn special_id(&self) -> Option<String> {
        <[u8; 32]>::try_from(self.special.as_slice())
            .ok()
            .map(|bytes| super::hex::encode_lower_hex(&bytes))
    }

    /// The special record as a UTF-8 identifier, for naddr.
    fn special_text(&self) -> Option<String> {
        String::from_utf8(self.special.clone()).ok()
    }
}

/// Parse NIP-19 TLV bytes: `0` special, `1` relay, `2` author, `3` kind.
fn parse_tlv(bytes: &[u8]) -> Result<Tlv, DomainError> {
    let mut tlv = Tlv {
        special: Vec::new(),
        relays: Vec::new(),
        author: None,
        kind: None,
    };
    let mut rest = bytes;
    while rest.len() >= 2 {
        let (record, length) = (rest[0], usize::from(rest[1]));
        rest = &rest[2..];
        if rest.len() < length {
            return Err(invalid("a TLV record overruns the code"));
        }
        let value = &rest[..length];
        match (record, value.len()) {
            (0, _) => tlv.special = value.to_vec(),
            (1, _) => tlv.relays.push(
                String::from_utf8(value.to_vec())
                    .map_err(|_| invalid("a TLV relay hint is UTF-8"))?,
            ),
            (2, 32) => tlv.author = Some(super::hex::encode_lower_hex(value)),
            (3, 4) => {
                tlv.kind =
                    u16::try_from(u32::from_be_bytes(value.try_into().unwrap_or_default())).ok()
            }
            _ => {}
        }
        rest = &rest[length..];
    }
    if !rest.is_empty() {
        return Err(invalid("a TLV record overruns the code"));
    }
    Ok(tlv)
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, 1, Vec::<Tag>::new(), content.to_string())
    }

    fn hex32(byte: u8) -> String {
        super::super::hex::encode_lower_hex(&[byte; 32])
    }

    #[test]
    fn nostr_references_decode_to_their_entity() {
        let pubkey = [0xab_u8; 32];
        let npub = nip19::encode_npub(&pubkey);
        // nevent TLV: id + kind (kept short for the 90-char bound).
        let mut tlv = vec![0x00, 32];
        tlv.extend_from_slice(&[0xcd; 32]);
        tlv.extend_from_slice(&[0x03, 0x04, 0, 0, 0, 1]);
        let nevent = nip19::encode("nevent", &tlv).unwrap();
        // naddr TLV: identifier + author + kind 30001.
        let mut atlv = vec![0x00, 3];
        atlv.extend_from_slice(b"doc");
        atlv.extend_from_slice(&[0x02, 32]);
        atlv.extend_from_slice(&pubkey);
        atlv.extend_from_slice(&[0x03, 0x04, 0, 0, 0x75, 0x31]);
        let naddr = nip19::encode("naddr", &atlv).unwrap();

        let event = sign(&format!(
            "hi nostr:{npub} see nostr:{nevent} and nostr:{naddr}."
        ));
        let refs = text_references(&event);
        assert_eq!(refs.len(), 3);
        match &refs[0] {
            TextReference::Profile { pubkey: p, .. } => assert_eq!(p, &hex32(0xab)),
            other => panic!("expected profile, got {other:?}"),
        }
        match &refs[1] {
            TextReference::Note {
                id,
                relays,
                author,
                kind,
            } => {
                assert_eq!(id, &"cd".repeat(32));
                assert!(relays.is_empty());
                assert_eq!(*author, None);
                assert_eq!(*kind, Some(1));
            }
            other => panic!("expected note, got {other:?}"),
        }
        match &refs[2] {
            TextReference::Address { address, kind, .. } => {
                assert_eq!(address, &format!("30001:{}:doc", hex32(0xab)));
                assert_eq!(*kind, 30_001);
            }
            other => panic!("expected address, got {other:?}"),
        }

        // Garbage after nostr: is skipped, not an error.
        assert!(text_references(&sign("nostr:notreal1xxx")).is_empty());
        assert!(text_references(&sign("no refs")).is_empty());
    }
}
