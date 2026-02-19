//! NIP-21: `nostr:` URI scheme
//!
//! This module implements the `nostr:` URI scheme for Nostr entities.
//! The URI scheme is simply `nostr:` followed by a NIP-19 entity (npub, note, nprofile, etc.).
//!
//! Examples:
//! - `nostr:npub1...`
//! - `nostr:note1...`
//! - `nostr:nprofile1...`
//! - `nostr:nevent1...`
//! - `nostr:naddr1...`
//!
//! Note: `nsec` entities SHOULD NOT be shared via URIs for security reasons.

use crate::nip19::{Nip19Entity, Nip19Error};
use thiserror::Error;

/// The nostr URI scheme prefix
pub const NOSTR_URI_SCHEME: &str = "nostr:";

/// Errors that can occur during NIP-21 operations.
#[derive(Debug, Error)]
pub enum Nip21Error {
    #[error("invalid URI scheme: expected 'nostr:', got '{0}'")]
    InvalidScheme(String),

    #[error("NIP-19 error: {0}")]
    Nip19(#[from] Nip19Error),

    #[error("nsec entities should not be shared via nostr: URIs")]
    NsecNotAllowed,
}

/// Convert a NIP-19 entity to a `nostr:` URI.
///
/// # Errors
///
/// Returns an error if the entity is a secret key (nsec), as these should not be shared.
pub fn to_nostr_uri(entity: &Nip19Entity) -> Result<String, Nip21Error> {
    // Reject nsec for security reasons
    if matches!(entity, Nip19Entity::Secret(_)) {
        return Err(Nip21Error::NsecNotAllowed);
    }

    let nip19_string = match entity {
        Nip19Entity::Pubkey(pubkey) => crate::nip19::encode_npub(pubkey)?,
        Nip19Entity::Secret(_) => unreachable!(), // Already checked above
        Nip19Entity::Note(note_id) => crate::nip19::encode_note(note_id)?,
        Nip19Entity::Profile(profile) => crate::nip19::encode_nprofile(profile)?,
        Nip19Entity::Event(event) => crate::nip19::encode_nevent(event)?,
        Nip19Entity::Address(addr) => crate::nip19::encode_naddr(addr)?,
    };

    Ok(format!("{}{}", NOSTR_URI_SCHEME, nip19_string))
}

/// Parse a `nostr:` URI into a NIP-19 entity.
///
/// Accepts both:
/// - Full URI: `nostr:npub1...`
/// - Just the entity: `npub1...` (for convenience)
pub fn from_nostr_uri(uri: &str) -> Result<Nip19Entity, Nip21Error> {
    let entity_str = if let Some(stripped) = uri.strip_prefix(NOSTR_URI_SCHEME) {
        stripped
    } else {
        // Also accept bare NIP-19 entities for convenience
        uri
    };

    let entity = crate::nip19::decode(entity_str)?;

    // Warn if nsec was decoded (shouldn't be shared via URIs)
    if matches!(entity, Nip19Entity::Secret(_)) {
        return Err(Nip21Error::NsecNotAllowed);
    }

    Ok(entity)
}

/// Check if a string is a `nostr:` URI.
pub fn is_nostr_uri(s: &str) -> bool {
    s.starts_with(NOSTR_URI_SCHEME)
}

/// Extract the NIP-19 entity part from a `nostr:` URI.
///
/// Returns the part after `nostr:`, or the original string if it doesn't have the prefix.
pub fn strip_nostr_prefix(uri: &str) -> &str {
    uri.strip_prefix(NOSTR_URI_SCHEME).unwrap_or(uri)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip19::{AddressPointer, EventPointer, ProfilePointer};

    #[test]
    fn test_npub_to_nostr_uri() {
        let pubkey_hex = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
        let pubkey_bytes = hex::decode(pubkey_hex).unwrap();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&pubkey_bytes);

        let entity = Nip19Entity::Pubkey(pubkey);
        let uri = to_nostr_uri(&entity).unwrap();

        assert!(uri.starts_with("nostr:npub"));

        let decoded = from_nostr_uri(&uri).unwrap();
        assert_eq!(decoded, entity);
    }

    #[test]
    fn test_note_to_nostr_uri() {
        let note_id = [42u8; 32];

        let entity = Nip19Entity::Note(note_id);
        let uri = to_nostr_uri(&entity).unwrap();

        assert!(uri.starts_with("nostr:note"));

        let decoded = from_nostr_uri(&uri).unwrap();
        assert_eq!(decoded, entity);
    }

    #[test]
    fn test_nprofile_to_nostr_uri() {
        let pubkey = [1u8; 32];
        let profile = ProfilePointer {
            pubkey,
            relays: vec!["wss://relay.example.com".to_string()],
        };

        let entity = Nip19Entity::Profile(profile.clone());
        let uri = to_nostr_uri(&entity).unwrap();

        assert!(uri.starts_with("nostr:nprofile"));

        let decoded = from_nostr_uri(&uri).unwrap();
        match decoded {
            Nip19Entity::Profile(p) => {
                assert_eq!(p.pubkey, pubkey);
                assert_eq!(p.relays, profile.relays);
            }
            _ => panic!("expected Profile"),
        }
    }

    #[test]
    fn test_nevent_to_nostr_uri() {
        let id = [2u8; 32];
        let event = EventPointer {
            id,
            relays: vec!["wss://relay.example.com".to_string()],
            author: Some([3u8; 32]),
            kind: Some(1),
        };

        let entity = Nip19Entity::Event(event.clone());
        let uri = to_nostr_uri(&entity).unwrap();

        assert!(uri.starts_with("nostr:nevent"));

        let decoded = from_nostr_uri(&uri).unwrap();
        match decoded {
            Nip19Entity::Event(e) => {
                assert_eq!(e.id, id);
                assert_eq!(e.relays, event.relays);
                assert_eq!(e.author, event.author);
                assert_eq!(e.kind, event.kind);
            }
            _ => panic!("expected Event"),
        }
    }

    #[test]
    fn test_naddr_to_nostr_uri() {
        let pubkey = [4u8; 32];
        let addr = AddressPointer {
            identifier: "my-article".to_string(),
            pubkey,
            kind: 30023,
            relays: vec!["wss://relay.example.com".to_string()],
        };

        let entity = Nip19Entity::Address(addr.clone());
        let uri = to_nostr_uri(&entity).unwrap();

        assert!(uri.starts_with("nostr:naddr"));

        let decoded = from_nostr_uri(&uri).unwrap();
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
    fn test_nsec_rejected() {
        let secret = [5u8; 32];
        let entity = Nip19Entity::Secret(secret);

        let result = to_nostr_uri(&entity);
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip21Error::NsecNotAllowed => {}
            _ => panic!("expected NsecNotAllowed error"),
        }
    }

    #[test]
    fn test_from_nostr_uri_without_prefix() {
        // Should also accept bare NIP-19 entities
        let pubkey = [6u8; 32];
        let npub = crate::nip19::encode_npub(&pubkey).unwrap();

        let decoded = from_nostr_uri(&npub).unwrap();
        match decoded {
            Nip19Entity::Pubkey(p) => assert_eq!(p, pubkey),
            _ => panic!("expected Pubkey"),
        }
    }

    #[test]
    fn test_is_nostr_uri() {
        assert!(is_nostr_uri("nostr:npub1test"));
        assert!(is_nostr_uri("nostr:note1test"));
        assert!(!is_nostr_uri("npub1test"));
        assert!(!is_nostr_uri("https://example.com"));
    }

    #[test]
    fn test_strip_nostr_prefix() {
        assert_eq!(strip_nostr_prefix("nostr:npub1test"), "npub1test");
        assert_eq!(strip_nostr_prefix("npub1test"), "npub1test");
    }

    #[test]
    fn test_example_from_nip21_spec() {
        // Example from NIP-21 spec
        let uri = "nostr:npub1sn0wdenkukak0d9dfczzeacvhkrgz92ak56egt7vdgzn8pv2wfqqhrjdv9";
        let decoded = from_nostr_uri(uri).unwrap();

        match decoded {
            Nip19Entity::Pubkey(_) => {}
            _ => panic!("expected Pubkey"),
        }
    }

    #[test]
    fn test_nprofile_example_from_nip21_spec() {
        // Example from NIP-21 spec
        let uri = "nostr:nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p";
        let decoded = from_nostr_uri(uri).unwrap();

        match decoded {
            Nip19Entity::Profile(p) => {
                assert_eq!(
                    hex::encode(p.pubkey),
                    "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
                );
                assert_eq!(p.relays.len(), 2);
                assert_eq!(p.relays[0], "wss://r.x.com");
                assert_eq!(p.relays[1], "wss://djbas.sadkb.com");
            }
            _ => panic!("expected Profile"),
        }
    }

    #[test]
    fn test_roundtrip() {
        let pubkey = [7u8; 32];
        let entity = Nip19Entity::Pubkey(pubkey);

        let uri = to_nostr_uri(&entity).unwrap();
        let decoded = from_nostr_uri(&uri).unwrap();

        assert_eq!(decoded, entity);
    }
}
