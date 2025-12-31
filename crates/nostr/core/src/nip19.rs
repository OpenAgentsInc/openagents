//! NIP-19: bech32-encoded entities
//!
//! This module implements the bech32 encoding for Nostr entities:
//! - Simple entities: npub, nsec, note (just 32 bytes)
//! - TLV entities: nprofile, nevent, naddr (with metadata)
//!
//! Note: npub and nsec are also defined in nip06.rs. This module provides the full
//! NIP-19 implementation including the other entity types.

use thiserror::Error;

/// Errors that can occur during NIP-19 operations.
#[derive(Debug, Error)]
pub enum Nip19Error {
    #[error("bech32 encoding error: {0}")]
    Bech32Encode(String),

    #[error("bech32 decoding error: {0}")]
    Bech32Decode(String),

    #[error("invalid entity format: {0}")]
    InvalidFormat(String),

    #[error("invalid hrp: expected one of {expected:?}, got {got}")]
    InvalidHrp { expected: Vec<String>, got: String },

    #[error("invalid length: expected {expected}, got {got}")]
    InvalidLength { expected: usize, got: usize },

    #[error("TLV encoding error: {0}")]
    TlvEncode(String),

    #[error("TLV decoding error: {0}")]
    TlvDecode(String),

    #[error("missing required TLV field: {0}")]
    MissingTlvField(String),

    #[error("hex decode error: {0}")]
    HexDecode(String),
}

/// A Nostr profile with optional relay hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilePointer {
    /// The 32-byte public key
    pub pubkey: [u8; 32],
    /// Optional relay hints
    pub relays: Vec<String>,
}

/// A Nostr event pointer with optional relay hints and metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPointer {
    /// The 32-byte event ID
    pub id: [u8; 32],
    /// Optional relay hints
    pub relays: Vec<String>,
    /// Optional author public key
    pub author: Option<[u8; 32]>,
    /// Optional event kind
    pub kind: Option<u32>,
}

/// A Nostr addressable event coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressPointer {
    /// The "d" tag identifier
    pub identifier: String,
    /// The author's public key
    pub pubkey: [u8; 32],
    /// The event kind
    pub kind: u32,
    /// Optional relay hints
    pub relays: Vec<String>,
}

/// Decoded NIP-19 entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nip19Entity {
    /// Public key (npub)
    Pubkey([u8; 32]),
    /// Private key (nsec)
    Secret([u8; 32]),
    /// Note/event ID (note)
    Note([u8; 32]),
    /// Profile with relays (nprofile)
    Profile(ProfilePointer),
    /// Event with metadata (nevent)
    Event(EventPointer),
    /// Addressable event (naddr)
    Address(AddressPointer),
}

// TLV type constants
const TLV_TYPE_SPECIAL: u8 = 0;
const TLV_TYPE_RELAY: u8 = 1;
const TLV_TYPE_AUTHOR: u8 = 2;
const TLV_TYPE_KIND: u8 = 3;

/// Encode a public key as npub.
pub fn encode_npub(pubkey: &[u8; 32]) -> Result<String, Nip19Error> {
    encode_bech32("npub", pubkey)
}

/// Encode a private key as nsec.
pub fn encode_nsec(secret: &[u8; 32]) -> Result<String, Nip19Error> {
    encode_bech32("nsec", secret)
}

/// Encode an event ID as note.
pub fn encode_note(event_id: &[u8; 32]) -> Result<String, Nip19Error> {
    encode_bech32("note", event_id)
}

/// Encode a profile pointer as nprofile.
pub fn encode_nprofile(profile: &ProfilePointer) -> Result<String, Nip19Error> {
    let mut tlv_data = Vec::new();

    // TLV type 0: pubkey (32 bytes)
    encode_tlv_field(&mut tlv_data, TLV_TYPE_SPECIAL, &profile.pubkey);

    // TLV type 1: relays (can be multiple)
    for relay in &profile.relays {
        encode_tlv_field(&mut tlv_data, TLV_TYPE_RELAY, relay.as_bytes());
    }

    encode_bech32_bytes("nprofile", &tlv_data)
}

/// Encode an event pointer as nevent.
pub fn encode_nevent(event: &EventPointer) -> Result<String, Nip19Error> {
    let mut tlv_data = Vec::new();

    // TLV type 0: event id (32 bytes)
    encode_tlv_field(&mut tlv_data, TLV_TYPE_SPECIAL, &event.id);

    // TLV type 1: relays (can be multiple)
    for relay in &event.relays {
        encode_tlv_field(&mut tlv_data, TLV_TYPE_RELAY, relay.as_bytes());
    }

    // TLV type 2: author (optional, 32 bytes)
    if let Some(author) = &event.author {
        encode_tlv_field(&mut tlv_data, TLV_TYPE_AUTHOR, author);
    }

    // TLV type 3: kind (optional, 4 bytes big-endian)
    if let Some(kind) = event.kind {
        let kind_bytes = kind.to_be_bytes();
        encode_tlv_field(&mut tlv_data, TLV_TYPE_KIND, &kind_bytes);
    }

    encode_bech32_bytes("nevent", &tlv_data)
}

/// Encode an address pointer as naddr.
pub fn encode_naddr(addr: &AddressPointer) -> Result<String, Nip19Error> {
    let mut tlv_data = Vec::new();

    // TLV type 0: identifier (d tag)
    encode_tlv_field(&mut tlv_data, TLV_TYPE_SPECIAL, addr.identifier.as_bytes());

    // TLV type 1: relays (can be multiple)
    for relay in &addr.relays {
        encode_tlv_field(&mut tlv_data, TLV_TYPE_RELAY, relay.as_bytes());
    }

    // TLV type 2: author (required, 32 bytes)
    encode_tlv_field(&mut tlv_data, TLV_TYPE_AUTHOR, &addr.pubkey);

    // TLV type 3: kind (required, 4 bytes big-endian)
    let kind_bytes = addr.kind.to_be_bytes();
    encode_tlv_field(&mut tlv_data, TLV_TYPE_KIND, &kind_bytes);

    encode_bech32_bytes("naddr", &tlv_data)
}

/// Decode a NIP-19 entity from a bech32 string.
pub fn decode(s: &str) -> Result<Nip19Entity, Nip19Error> {
    let (hrp, data) = bech32::decode(s).map_err(|e| Nip19Error::Bech32Decode(e.to_string()))?;

    let hrp_str = hrp.to_string();
    let bytes: Vec<u8> = data;

    match hrp_str.as_str() {
        "npub" => {
            let pubkey = parse_32_bytes(&bytes)?;
            Ok(Nip19Entity::Pubkey(pubkey))
        }
        "nsec" => {
            let secret = parse_32_bytes(&bytes)?;
            Ok(Nip19Entity::Secret(secret))
        }
        "note" => {
            let note_id = parse_32_bytes(&bytes)?;
            Ok(Nip19Entity::Note(note_id))
        }
        "nprofile" => {
            let profile = decode_nprofile(&bytes)?;
            Ok(Nip19Entity::Profile(profile))
        }
        "nevent" => {
            let event = decode_nevent(&bytes)?;
            Ok(Nip19Entity::Event(event))
        }
        "naddr" => {
            let addr = decode_naddr(&bytes)?;
            Ok(Nip19Entity::Address(addr))
        }
        other => Err(Nip19Error::InvalidHrp {
            expected: vec![
                "npub".to_string(),
                "nsec".to_string(),
                "note".to_string(),
                "nprofile".to_string(),
                "nevent".to_string(),
                "naddr".to_string(),
            ],
            got: other.to_string(),
        }),
    }
}

/// Encode bytes as bech32 with the given HRP (for 32-byte entities).
fn encode_bech32(hrp: &str, data: &[u8; 32]) -> Result<String, Nip19Error> {
    encode_bech32_bytes(hrp, data)
}

/// Encode bytes as bech32 with the given HRP (for any length).
fn encode_bech32_bytes(hrp: &str, data: &[u8]) -> Result<String, Nip19Error> {
    use bech32::{Bech32, Hrp};

    let hrp = Hrp::parse(hrp).map_err(|e| Nip19Error::Bech32Encode(e.to_string()))?;

    bech32::encode::<Bech32>(hrp, data).map_err(|e| Nip19Error::Bech32Encode(e.to_string()))
}

/// Encode a single TLV field.
fn encode_tlv_field(buf: &mut Vec<u8>, tlv_type: u8, value: &[u8]) {
    buf.push(tlv_type);
    buf.push(value.len() as u8);
    buf.extend_from_slice(value);
}

/// Parse TLV data into fields.
fn parse_tlv(data: &[u8]) -> Result<Vec<(u8, Vec<u8>)>, Nip19Error> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if i + 2 > data.len() {
            return Err(Nip19Error::TlvDecode(
                "insufficient data for TLV header".to_string(),
            ));
        }

        let tlv_type = data[i];
        let length = data[i + 1] as usize;
        i += 2;

        if i + length > data.len() {
            return Err(Nip19Error::TlvDecode(format!(
                "insufficient data for TLV value: expected {} bytes",
                length
            )));
        }

        let value = data[i..i + length].to_vec();
        i += length;

        result.push((tlv_type, value));
    }

    Ok(result)
}

/// Parse exactly 32 bytes from a slice.
fn parse_32_bytes(data: &[u8]) -> Result<[u8; 32], Nip19Error> {
    if data.len() != 32 {
        return Err(Nip19Error::InvalidLength {
            expected: 32,
            got: data.len(),
        });
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(data);
    Ok(result)
}

/// Decode nprofile TLV data.
fn decode_nprofile(data: &[u8]) -> Result<ProfilePointer, Nip19Error> {
    let fields = parse_tlv(data)?;

    let mut pubkey: Option<[u8; 32]> = None;
    let mut relays = Vec::new();

    for (tlv_type, value) in fields {
        match tlv_type {
            TLV_TYPE_SPECIAL => {
                pubkey = Some(parse_32_bytes(&value)?);
            }
            TLV_TYPE_RELAY => {
                let relay = String::from_utf8(value)
                    .map_err(|e| Nip19Error::TlvDecode(format!("invalid UTF-8 in relay: {}", e)))?;
                relays.push(relay);
            }
            // Ignore unknown TLV types per spec
            _ => {}
        }
    }

    let pubkey = pubkey.ok_or_else(|| Nip19Error::MissingTlvField("pubkey".to_string()))?;

    Ok(ProfilePointer { pubkey, relays })
}

/// Decode nevent TLV data.
fn decode_nevent(data: &[u8]) -> Result<EventPointer, Nip19Error> {
    let fields = parse_tlv(data)?;

    let mut id: Option<[u8; 32]> = None;
    let mut relays = Vec::new();
    let mut author: Option<[u8; 32]> = None;
    let mut kind: Option<u32> = None;

    for (tlv_type, value) in fields {
        match tlv_type {
            TLV_TYPE_SPECIAL => {
                id = Some(parse_32_bytes(&value)?);
            }
            TLV_TYPE_RELAY => {
                let relay = String::from_utf8(value)
                    .map_err(|e| Nip19Error::TlvDecode(format!("invalid UTF-8 in relay: {}", e)))?;
                relays.push(relay);
            }
            TLV_TYPE_AUTHOR => {
                author = Some(parse_32_bytes(&value)?);
            }
            TLV_TYPE_KIND => {
                if value.len() != 4 {
                    return Err(Nip19Error::InvalidLength {
                        expected: 4,
                        got: value.len(),
                    });
                }
                let mut kind_bytes = [0u8; 4];
                kind_bytes.copy_from_slice(&value);
                kind = Some(u32::from_be_bytes(kind_bytes));
            }
            // Ignore unknown TLV types per spec
            _ => {}
        }
    }

    let id = id.ok_or_else(|| Nip19Error::MissingTlvField("event id".to_string()))?;

    Ok(EventPointer {
        id,
        relays,
        author,
        kind,
    })
}

/// Decode naddr TLV data.
fn decode_naddr(data: &[u8]) -> Result<AddressPointer, Nip19Error> {
    let fields = parse_tlv(data)?;

    let mut identifier: Option<String> = None;
    let mut pubkey: Option<[u8; 32]> = None;
    let mut kind: Option<u32> = None;
    let mut relays = Vec::new();

    for (tlv_type, value) in fields {
        match tlv_type {
            TLV_TYPE_SPECIAL => {
                let id = String::from_utf8(value).map_err(|e| {
                    Nip19Error::TlvDecode(format!("invalid UTF-8 in identifier: {}", e))
                })?;
                identifier = Some(id);
            }
            TLV_TYPE_RELAY => {
                let relay = String::from_utf8(value)
                    .map_err(|e| Nip19Error::TlvDecode(format!("invalid UTF-8 in relay: {}", e)))?;
                relays.push(relay);
            }
            TLV_TYPE_AUTHOR => {
                pubkey = Some(parse_32_bytes(&value)?);
            }
            TLV_TYPE_KIND => {
                if value.len() != 4 {
                    return Err(Nip19Error::InvalidLength {
                        expected: 4,
                        got: value.len(),
                    });
                }
                let mut kind_bytes = [0u8; 4];
                kind_bytes.copy_from_slice(&value);
                kind = Some(u32::from_be_bytes(kind_bytes));
            }
            // Ignore unknown TLV types per spec
            _ => {}
        }
    }

    let identifier =
        identifier.ok_or_else(|| Nip19Error::MissingTlvField("identifier".to_string()))?;
    let pubkey = pubkey.ok_or_else(|| Nip19Error::MissingTlvField("pubkey".to_string()))?;
    let kind = kind.ok_or_else(|| Nip19Error::MissingTlvField("kind".to_string()))?;

    Ok(AddressPointer {
        identifier,
        pubkey,
        kind,
        relays,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_npub() {
        let pubkey_hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        let expected_npub = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";

        let pubkey_bytes = hex::decode(pubkey_hex).unwrap();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&pubkey_bytes);

        let npub = encode_npub(&pubkey).unwrap();
        assert_eq!(npub, expected_npub);

        let decoded = decode(&npub).unwrap();
        assert_eq!(decoded, Nip19Entity::Pubkey(pubkey));
    }

    #[test]
    fn test_encode_decode_nsec() {
        let secret_hex = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa";
        let expected_nsec = "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";

        let secret_bytes = hex::decode(secret_hex).unwrap();
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&secret_bytes);

        let nsec = encode_nsec(&secret).unwrap();
        assert_eq!(nsec, expected_nsec);

        let decoded = decode(&nsec).unwrap();
        assert_eq!(decoded, Nip19Entity::Secret(secret));
    }

    #[test]
    fn test_encode_decode_note() {
        let note_hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";

        let note_bytes = hex::decode(note_hex).unwrap();
        let mut note_id = [0u8; 32];
        note_id.copy_from_slice(&note_bytes);

        let note = encode_note(&note_id).unwrap();

        let decoded = decode(&note).unwrap();
        assert_eq!(decoded, Nip19Entity::Note(note_id));
    }

    #[test]
    fn test_encode_decode_nprofile() {
        let pubkey_hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        let pubkey_bytes = hex::decode(pubkey_hex).unwrap();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&pubkey_bytes);

        let profile = ProfilePointer {
            pubkey,
            relays: vec![
                "wss://r.x.com".to_string(),
                "wss://djbas.sadkb.com".to_string(),
            ],
        };

        let nprofile = encode_nprofile(&profile).unwrap();

        // The expected value from the NIP-19 spec
        let expected_nprofile = "nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p";
        assert_eq!(nprofile, expected_nprofile);

        let decoded = decode(&nprofile).unwrap();
        match decoded {
            Nip19Entity::Profile(p) => {
                assert_eq!(p.pubkey, pubkey);
                assert_eq!(p.relays, profile.relays);
            }
            _ => panic!("expected Profile"),
        }
    }

    #[test]
    fn test_encode_decode_nevent() {
        let id_hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        let id_bytes = hex::decode(id_hex).unwrap();
        let mut id = [0u8; 32];
        id.copy_from_slice(&id_bytes);

        let event = EventPointer {
            id,
            relays: vec!["wss://relay.example.com".to_string()],
            author: None,
            kind: Some(1),
        };

        let nevent = encode_nevent(&event).unwrap();

        let decoded = decode(&nevent).unwrap();
        match decoded {
            Nip19Entity::Event(e) => {
                assert_eq!(e.id, id);
                assert_eq!(e.relays, event.relays);
                assert_eq!(e.author, None);
                assert_eq!(e.kind, Some(1));
            }
            _ => panic!("expected Event"),
        }
    }

    #[test]
    fn test_encode_decode_naddr() {
        let pubkey_hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        let pubkey_bytes = hex::decode(pubkey_hex).unwrap();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&pubkey_bytes);

        let addr = AddressPointer {
            identifier: "test-article".to_string(),
            pubkey,
            kind: 30023,
            relays: vec!["wss://relay.example.com".to_string()],
        };

        let naddr = encode_naddr(&addr).unwrap();

        let decoded = decode(&naddr).unwrap();
        match decoded {
            Nip19Entity::Address(a) => {
                assert_eq!(a.identifier, addr.identifier);
                assert_eq!(a.pubkey, addr.pubkey);
                assert_eq!(a.kind, addr.kind);
                assert_eq!(a.relays, addr.relays);
            }
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn test_naddr_empty_identifier() {
        // For normal replaceable events, use empty string as identifier
        let pubkey = [1u8; 32];

        let addr = AddressPointer {
            identifier: "".to_string(),
            pubkey,
            kind: 10000,
            relays: vec![],
        };

        let naddr = encode_naddr(&addr).unwrap();

        let decoded = decode(&naddr).unwrap();
        match decoded {
            Nip19Entity::Address(a) => {
                assert_eq!(a.identifier, "");
                assert_eq!(a.pubkey, pubkey);
                assert_eq!(a.kind, 10000);
            }
            _ => panic!("expected Address"),
        }
    }

    #[test]
    fn test_decode_invalid_hrp() {
        // Create a valid bech32 string with "invalid" HRP
        // This uses a valid checksum so bech32 decode succeeds, but our code rejects the HRP
        use bech32::{Bech32, Hrp};
        let hrp = Hrp::parse("invalid").unwrap();
        let data = vec![0u8; 32]; // 32 bytes of zeros
        let invalid_bech32 = bech32::encode::<Bech32>(hrp, &data).unwrap();

        let result = decode(&invalid_bech32);
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip19Error::InvalidHrp { expected, got } => {
                assert_eq!(got, "invalid");
                assert!(expected.contains(&"npub".to_string()));
            }
            other => panic!("expected InvalidHrp error, got: {:?}", other),
        }
    }

    #[test]
    fn test_decode_invalid_length() {
        // npub with wrong length
        let result = decode("npub1test");
        assert!(result.is_err());
    }
}
