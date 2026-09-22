//! NIP-EE E2EE group messaging over MLS.
//!
//! The Nostr envelope is what this module proves. A kind `443`
//! KeyPackage advertises the MLS version, ciphersuite, and extensions a
//! client supports. A kind `10051` list names the relays KeyPackages go
//! to. A kind `444` Welcome is an unsigned rumor carrying the
//! KeyPackage event id and the group relays. A kind `445` group message
//! is signed by a fresh ephemeral key and names the 32-byte Nostr group
//! id in `h`; its content is a NIP-44 payload keyed by the group's
//! `exporter_secret`, and `exporter_conversation_key` derives that key
//! the way the pinned text says: the secret is the sender key and its
//! own public key is the receiver.
//!
//! MLS group state itself is out of scope — this crate does not parse
//! an `MLSMessage` and a relay cannot decrypt kind `445`. Competing
//! `Commit` events resolve by the lowest `created_at`, then the lowest
//! event id, which `commit_order` implements. NIP-EE is marked
//! unrecommended — superseded by the Marmot protocol — so these kinds
//! stay off the NIP-11 list.

use secp256k1::{Keypair, Secp256k1, SecretKey};

use crate::nip44;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, Rumor};

const KEY_PACKAGE_KIND: u16 = 443;
const WELCOME_KIND: u16 = 444;
const GROUP_MESSAGE_KIND: u16 = 445;
const KEY_PACKAGE_RELAYS_KIND: u16 = 10_051;
const MLS_VERSION: &str = "1.0";

/// A kind `443` MLS KeyPackage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPackage {
    /// The ciphersuite id, as `0x`-prefixed hex.
    pub ciphersuite: String,
    /// The MLS extension ids the client supports.
    pub extensions: Vec<String>,
    /// The client tag's name, when the event carries one.
    pub client: Option<String>,
    /// The relays the client will publish this KeyPackage to.
    pub relays: Vec<String>,
}

/// A kind `10051` KeyPackage relays list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPackageRelays {
    /// The relay URIs the author's KeyPackages publish to.
    pub relays: Vec<String>,
}

/// A kind `444` Welcome rumor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Welcome {
    /// The KeyPackage event id used to add the member.
    pub key_package: String,
    /// The relays the new member queries for group events.
    pub relays: Vec<String>,
}

/// Read a kind `443` KeyPackage event.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing or
/// non-`1.0` `mls_protocol_version`, a missing or malformed
/// `ciphersuite` or `extensions` id, or content that is not the
/// hex-encoded `KeyPackageBundle`.
pub fn open_key_package(event: &Event) -> Result<KeyPackage, DomainError> {
    if event.kind != KEY_PACKAGE_KIND {
        return Err(invalid("a KeyPackage event is kind 443"));
    }
    let versions: Vec<&str> = event.tag_values("mls_protocol_version").collect();
    if versions != [MLS_VERSION] {
        return Err(invalid("kind 443 requires one mls_protocol_version of 1.0"));
    }
    let ciphersuites: Vec<&str> = event.tag_values("ciphersuite").collect();
    let [ciphersuite] = ciphersuites.as_slice() else {
        return Err(invalid("kind 443 requires one ciphersuite tag"));
    };
    if !mls_id(ciphersuite) {
        return Err(invalid("a ciphersuite id is 0x-prefixed hex"));
    }
    let mut extensions = Vec::new();
    for tag in event.tag_values("extensions") {
        for id in tag.split(',') {
            let id = id.trim();
            if !mls_id(id) {
                return Err(invalid("an extensions id is 0x-prefixed hex"));
            }
            extensions.push(id.to_string());
        }
    }
    if event.content.is_empty()
        || event.content.len() % 2 == 1
        || !event.content.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid(
            "kind 443 content is the hex-encoded KeyPackageBundle",
        ));
    }
    let client = event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("client"))
        .and_then(|tag| tag.value().map(str::to_string));
    let relays = event.tag_values("relays").flat_map(relay_list).collect();
    Ok(KeyPackage {
        ciphersuite: (*ciphersuite).to_string(),
        extensions,
        client,
        relays,
    })
}

/// Read a kind `10051` KeyPackage relays list.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind or a list with
/// no valid `relay` tag.
pub fn open_key_package_relays(event: &Event) -> Result<KeyPackageRelays, DomainError> {
    if event.kind != KEY_PACKAGE_RELAYS_KIND {
        return Err(invalid("a KeyPackage relays list is kind 10051"));
    }
    let relays: Vec<String> = event.tag_values("relay").flat_map(relay_list).collect();
    if relays.is_empty() {
        return Err(invalid("kind 10051 requires at least one relay URI"));
    }
    Ok(KeyPackageRelays { relays })
}

/// Read a kind `444` Welcome rumor.
///
/// The rumor type carries no signature, which is the pinned rule: a
/// Welcome that leaked must not be publishable.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing or
/// malformed `e` tag naming the KeyPackage event, or no `relays` tag.
pub fn open_welcome(rumor: &Rumor) -> Result<Welcome, DomainError> {
    if rumor.kind != WELCOME_KIND {
        return Err(invalid("a Welcome rumor is kind 444"));
    }
    let ids: Vec<&str> = rumor
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("e"))
        .filter_map(|tag| tag.value())
        .collect();
    let [key_package] = ids.as_slice() else {
        return Err(invalid("kind 444 requires one e tag"));
    };
    if decode_lower_hex::<32>(key_package, "e").is_err() {
        return Err(invalid("the e tag names a KeyPackage event id"));
    }
    let relays: Vec<String> = rumor
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("relays"))
        .filter_map(|tag| tag.value())
        .flat_map(relay_list)
        .collect();
    if relays.is_empty() {
        return Err(invalid("kind 444 requires a relays tag"));
    }
    Ok(Welcome {
        key_package: (*key_package).to_string(),
        relays,
    })
}

/// Check a kind `445` group message event.
///
/// The `h` tag is the 32-byte Nostr group id, the content is a NIP-44
/// payload under the group's exporter key, and the signer is a fresh
/// ephemeral key — a rule the sender keeps, since a relay cannot tell
/// one ephemeral key from another.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing or
/// malformed `h` tag, or content that is not a NIP-44 payload.
pub fn open_group_message(event: &Event) -> Result<(), DomainError> {
    if event.kind != GROUP_MESSAGE_KIND {
        return Err(invalid("a group message event is kind 445"));
    }
    let groups: Vec<&str> = event.tag_values("h").collect();
    let [group] = groups.as_slice() else {
        return Err(invalid("kind 445 requires one h tag"));
    };
    if decode_lower_hex::<32>(group, "h").is_err() {
        return Err(invalid("the h tag is the 32-byte Nostr group id"));
    }
    nip44::payload_shape(&event.content)
        .map_err(|_| invalid("kind 445 content is a NIP-44 payload under the exporter key"))?;
    Ok(())
}

/// Whether an unsigned application rumor is safe inside the group: it
/// must not carry an `h` tag or any other group identifier, so a leak
/// cannot bind it to the group.
#[must_use]
pub fn inner_event_hides_the_group(rumor: &Rumor) -> bool {
    !rumor.tags.iter().any(|tag| tag.name() == Some("h"))
}

/// The NIP-44 conversation key a group message uses: the MLS
/// `exporter_secret` as the sender secret, with that same secret's
/// public key as the receiver — the pinned text's construction for a
/// group-shared key.
///
/// # Errors
///
/// Returns `DomainError::InvalidHex` when the secret is not 32 hex
/// bytes, or `DomainError::InvalidEvent` when it is not a valid secret.
pub fn exporter_conversation_key(exporter_secret: &str) -> Result<[u8; 32], DomainError> {
    let bytes = decode_lower_hex::<32>(exporter_secret, "exporter_secret")?;
    let secret = SecretKey::from_byte_array(bytes)
        .map_err(|_| invalid("the exporter secret is not a valid key"))?;
    let secp = Secp256k1::new();
    let public = Keypair::from_secret_key(&secp, &secret)
        .x_only_public_key()
        .0;
    Ok(nip44::conversation_key(&secret, &public))
}

/// The `Commit` event the group applies out of `a` and `b`: the lower
/// `created_at`, then the lower event id when the times match.
#[must_use]
pub fn commit_wins<'a>(a: &'a Event, b: &'a Event) -> &'a Event {
    if (b.created_at, &b.id) < (a.created_at, &a.id) {
        b
    } else {
        a
    }
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

fn mls_id(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("0x") else {
        return false;
    };
    !hex.is_empty() && hex.len() <= 8 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn relay_list(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|uri| valid_relay_uri(uri))
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter()
}

fn valid_relay_uri(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    fn key_package() -> Event {
        sign(
            KEY_PACKAGE_KIND,
            vec![
                Tag::new(vec!["mls_protocol_version".into(), "1.0".into()]),
                Tag::new(vec!["ciphersuite".into(), "0x0001".into()]),
                Tag::new(vec!["extensions".into(), "0x0001, 0x0002".into()]),
                Tag::new(vec!["client".into(), "gossip".into(), "ab".repeat(32)]),
                Tag::new(vec!["relays".into(), "wss://relay.example".into()]),
                Tag::new(vec!["-".into()]),
            ],
            &"aa".repeat(64),
        )
    }

    #[test]
    fn a_key_package_a_welcome_and_a_group_message_follow_the_pinned_envelopes() {
        let package = open_key_package(&key_package()).unwrap();
        assert_eq!(package.ciphersuite, "0x0001");
        assert_eq!(package.extensions, ["0x0001", "0x0002"]);
        assert_eq!(package.client.as_deref(), Some("gossip"));
        assert_eq!(package.relays, ["wss://relay.example"]);

        let relays = sign(
            KEY_PACKAGE_RELAYS_KIND,
            vec![
                Tag::new(vec!["relay".into(), "wss://inbox.example".into()]),
                Tag::new(vec!["relay".into(), "wss://other.example".into()]),
            ],
            "",
        );
        let list = open_key_package_relays(&relays).unwrap();
        assert_eq!(list.relays.len(), 2);
        assert_eq!(
            EventClass::from_kind(KEY_PACKAGE_RELAYS_KIND),
            EventClass::Replaceable
        );

        let package_id = "cd".repeat(32);
        let welcome = Rumor {
            id: String::new(),
            pubkey: "ab".repeat(32),
            created_at: 1_700_000_000,
            kind: WELCOME_KIND,
            tags: vec![
                Tag::new(vec!["e".into(), package_id.clone()]),
                Tag::new(vec!["relays".into(), "wss://groups.example".into()]),
            ],
            content: "serialized-welcome".into(),
        };
        let welcome = open_welcome(&welcome).unwrap();
        assert_eq!(welcome.key_package, package_id);

        let group = "ef".repeat(32);
        let conversation = exporter_conversation_key(&"77".repeat(32)).unwrap();
        let message = sign(
            GROUP_MESSAGE_KIND,
            vec![Tag::new(vec!["h".into(), group])],
            &nip44::encrypt("an MLSMessage", &conversation, [9; 32]).unwrap(),
        );
        open_group_message(&message).unwrap();
        // The group key encrypts and decrypts under itself: the exporter
        // secret is sender and receiver at once.
        let payload = nip44::encrypt("the pinned MLSMessage", &conversation, [1; 32]).unwrap();
        assert_eq!(
            nip44::decrypt(&payload, &conversation).unwrap(),
            "the pinned MLSMessage"
        );

        let inner = Rumor {
            id: String::new(),
            pubkey: "ab".repeat(32),
            created_at: 1_700_000_000,
            kind: 9,
            tags: vec![Tag::new(vec!["e".into(), "cd".repeat(32)])],
            content: "chat".into(),
        };
        assert!(inner_event_hides_the_group(&inner));
        let leaking = Rumor {
            tags: vec![Tag::new(vec!["h".into(), "ef".repeat(32)])],
            ..inner
        };
        assert!(!inner_event_hides_the_group(&leaking));
    }

    #[test]
    fn the_earliest_commit_wins_then_the_lowest_id() {
        let h = vec![Tag::new(vec!["h".into(), "ef".repeat(32)])];
        let mut early = sign(GROUP_MESSAGE_KIND, h.clone(), "");
        let mut late = sign(GROUP_MESSAGE_KIND, h.clone(), "");
        early.created_at = 1_700_000_000;
        late.created_at = 1_700_000_100;
        assert!(std::ptr::eq(commit_wins(&early, &late), &early));
        assert!(std::ptr::eq(commit_wins(&late, &early), &early));

        // Same created_at: the lower id applies. The contents differ so
        // the two Commits carry different ids.
        let one = sign(GROUP_MESSAGE_KIND, h.clone(), "commit one");
        let two = sign(GROUP_MESSAGE_KIND, h, "commit two");
        let (low, high) = if one.id < two.id {
            (one, two)
        } else {
            (two, one)
        };
        assert!(std::ptr::eq(commit_wins(&low, &high), &low));
        assert!(std::ptr::eq(commit_wins(&high, &low), &low));
    }

    #[test]
    fn malformed_envelopes_are_refused() {
        let no_version = sign(
            KEY_PACKAGE_KIND,
            vec![Tag::new(vec!["ciphersuite".into(), "0x0001".into()])],
            &"aa".repeat(64),
        );
        assert!(open_key_package(&no_version).is_err());
        let bad_cipher = sign(
            KEY_PACKAGE_KIND,
            vec![
                Tag::new(vec!["mls_protocol_version".into(), "1.0".into()]),
                Tag::new(vec!["ciphersuite".into(), "1".into()]),
            ],
            &"aa".repeat(64),
        );
        assert!(open_key_package(&bad_cipher).is_err());
        assert!(open_key_package(&sign(1, Vec::new(), "note")).is_err());
        assert!(open_key_package_relays(&sign(KEY_PACKAGE_RELAYS_KIND, Vec::new(), "")).is_err());
        assert!(
            open_welcome(&Rumor {
                id: String::new(),
                pubkey: "ab".repeat(32),
                created_at: 0,
                kind: WELCOME_KIND,
                tags: Vec::new(),
                content: String::new(),
            })
            .is_err()
        );
        let no_group = sign(GROUP_MESSAGE_KIND, Vec::new(), "");
        assert!(open_group_message(&no_group).is_err());
        let plaintext = sign(
            GROUP_MESSAGE_KIND,
            vec![Tag::new(vec!["h".into(), "ef".repeat(32)])],
            "not encrypted",
        );
        assert!(open_group_message(&plaintext).is_err());
        assert!(exporter_conversation_key("not hex").is_err());
    }
}
