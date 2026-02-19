//! Property-based tests for NIP-19 encoding/decoding
//!
//! These tests use quickcheck to verify that encoding and decoding operations
//! satisfy fundamental properties for all possible inputs.

use crate::nip19::{
    AddressPointer, EventPointer, Nip19Entity, ProfilePointer, decode, encode_naddr, encode_nevent,
    encode_note, encode_nprofile, encode_npub, encode_nsec,
};
use quickcheck::{Arbitrary, Gen};

/// Wrapper for [u8; 32] to implement Arbitrary
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bytes32([u8; 32]);

impl Arbitrary for Bytes32 {
    fn arbitrary(g: &mut Gen) -> Self {
        let mut bytes = [0u8; 32];
        for byte in &mut bytes {
            *byte = u8::arbitrary(g);
        }
        Bytes32(bytes)
    }
}

/// Property: npub encoding/decoding roundtrips correctly
fn prop_npub_roundtrip(pubkey: Bytes32) -> bool {
    let encoded = match encode_npub(&pubkey.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let decoded = match decode(&encoded) {
        Ok(Nip19Entity::Pubkey(pk)) => pk,
        _ => return false,
    };

    decoded == pubkey.0
}

/// Property: nsec encoding/decoding roundtrips correctly
fn prop_nsec_roundtrip(secret: Bytes32) -> bool {
    let encoded = match encode_nsec(&secret.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let decoded = match decode(&encoded) {
        Ok(Nip19Entity::Secret(sk)) => sk,
        _ => return false,
    };

    decoded == secret.0
}

/// Property: note encoding/decoding roundtrips correctly
fn prop_note_roundtrip(note_id: Bytes32) -> bool {
    let encoded = match encode_note(&note_id.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let decoded = match decode(&encoded) {
        Ok(Nip19Entity::Note(id)) => id,
        _ => return false,
    };

    decoded == note_id.0
}

/// Wrapper for ProfilePointer to implement Arbitrary
#[derive(Debug, Clone)]
struct ArbitraryProfile(ProfilePointer);

impl Arbitrary for ArbitraryProfile {
    fn arbitrary(g: &mut Gen) -> Self {
        let pubkey = Bytes32::arbitrary(g).0;
        // Limit relay count to avoid overly large test cases
        let relay_count = usize::arbitrary(g) % 5;
        let relays = (0..relay_count)
            .map(|_| {
                // Generate simple relay URLs
                let suffix = (0..10)
                    .map(|_| char::arbitrary(g).to_string())
                    .collect::<String>();
                format!("wss://relay{}.example.com", suffix)
            })
            .collect();

        ArbitraryProfile(ProfilePointer { pubkey, relays })
    }
}

/// Property: nprofile encoding/decoding roundtrips correctly
fn prop_nprofile_roundtrip(profile: ArbitraryProfile) -> bool {
    let encoded = match encode_nprofile(&profile.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let decoded = match decode(&encoded) {
        Ok(Nip19Entity::Profile(p)) => p,
        _ => return false,
    };

    decoded.pubkey == profile.0.pubkey && decoded.relays == profile.0.relays
}

/// Wrapper for EventPointer to implement Arbitrary
#[derive(Debug, Clone)]
struct ArbitraryEvent(EventPointer);

impl Arbitrary for ArbitraryEvent {
    fn arbitrary(g: &mut Gen) -> Self {
        let id = Bytes32::arbitrary(g).0;
        let relay_count = usize::arbitrary(g) % 3;
        let relays = (0..relay_count)
            .map(|_| {
                let suffix = (0..8)
                    .map(|_| char::arbitrary(g).to_string())
                    .collect::<String>();
                format!("wss://relay{}.example.com", suffix)
            })
            .collect();
        let author = if bool::arbitrary(g) {
            Some(Bytes32::arbitrary(g).0)
        } else {
            None
        };
        let kind = if bool::arbitrary(g) {
            Some(u32::arbitrary(g))
        } else {
            None
        };

        ArbitraryEvent(EventPointer {
            id,
            relays,
            author,
            kind,
        })
    }
}

/// Property: nevent encoding/decoding roundtrips correctly
fn prop_nevent_roundtrip(event: ArbitraryEvent) -> bool {
    let encoded = match encode_nevent(&event.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let decoded = match decode(&encoded) {
        Ok(Nip19Entity::Event(e)) => e,
        _ => return false,
    };

    decoded.id == event.0.id
        && decoded.relays == event.0.relays
        && decoded.author == event.0.author
        && decoded.kind == event.0.kind
}

/// Wrapper for AddressPointer to implement Arbitrary
#[derive(Debug, Clone)]
struct ArbitraryAddress(AddressPointer);

impl Arbitrary for ArbitraryAddress {
    fn arbitrary(g: &mut Gen) -> Self {
        let identifier = {
            let len = usize::arbitrary(g) % 20 + 1;
            (0..len)
                .map(|_| {
                    // Use alphanumeric chars to avoid UTF-8 issues
                    let chars = "abcdefghijklmnopqrstuvwxyz0123456789-_";
                    chars
                        .chars()
                        .nth(usize::arbitrary(g) % chars.len())
                        .unwrap()
                })
                .collect::<String>()
        };
        let pubkey = Bytes32::arbitrary(g).0;
        let kind = u32::arbitrary(g);
        let relay_count = usize::arbitrary(g) % 3;
        let relays = (0..relay_count)
            .map(|_| {
                let suffix = (0..8)
                    .map(|_| char::arbitrary(g).to_string())
                    .collect::<String>();
                format!("wss://relay{}.example.com", suffix)
            })
            .collect();

        ArbitraryAddress(AddressPointer {
            identifier,
            pubkey,
            kind,
            relays,
        })
    }
}

/// Property: naddr encoding/decoding roundtrips correctly
fn prop_naddr_roundtrip(addr: ArbitraryAddress) -> bool {
    let encoded = match encode_naddr(&addr.0) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let decoded = match decode(&encoded) {
        Ok(Nip19Entity::Address(a)) => a,
        _ => return false,
    };

    decoded.identifier == addr.0.identifier
        && decoded.pubkey == addr.0.pubkey
        && decoded.kind == addr.0.kind
        && decoded.relays == addr.0.relays
}

/// Property: encoding the same data twice produces the same result (deterministic)
fn prop_npub_deterministic(pubkey: Bytes32) -> bool {
    let enc1 = encode_npub(&pubkey.0).ok();
    let enc2 = encode_npub(&pubkey.0).ok();
    enc1 == enc2
}

/// Property: decoding invalid bech32 strings should fail gracefully
fn prop_decode_invalid_graceful(invalid_str: String) -> bool {
    // If the string doesn't start with a valid NIP-19 prefix, it should error
    if !invalid_str.starts_with("npub")
        && !invalid_str.starts_with("nsec")
        && !invalid_str.starts_with("note")
        && !invalid_str.starts_with("nprofile")
        && !invalid_str.starts_with("nevent")
        && !invalid_str.starts_with("naddr")
    {
        decode(&invalid_str).is_err()
    } else {
        // If it has a valid prefix but is otherwise invalid, should still error gracefully
        // We just check it doesn't panic
        let _ = decode(&invalid_str);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::QuickCheck;

    #[test]
    fn test_npub_roundtrip() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_npub_roundtrip as fn(Bytes32) -> bool);
    }

    #[test]
    fn test_nsec_roundtrip() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_nsec_roundtrip as fn(Bytes32) -> bool);
    }

    #[test]
    fn test_note_roundtrip() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_note_roundtrip as fn(Bytes32) -> bool);
    }

    #[test]
    fn test_nprofile_roundtrip() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_nprofile_roundtrip as fn(ArbitraryProfile) -> bool);
    }

    #[test]
    fn test_nevent_roundtrip() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_nevent_roundtrip as fn(ArbitraryEvent) -> bool);
    }

    #[test]
    fn test_naddr_roundtrip() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_naddr_roundtrip as fn(ArbitraryAddress) -> bool);
    }

    #[test]
    fn test_npub_deterministic() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_npub_deterministic as fn(Bytes32) -> bool);
    }

    #[test]
    fn test_decode_invalid_graceful() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_decode_invalid_graceful as fn(String) -> bool);
    }
}
