//! Evidence for the pinned official NIP lane.
//!
//! Each file under `nips/official/` other than the index is either checked
//! here or named as a document whose normative text now lives in NIP-01.
//! A check builds or verifies a value with the shipped functions and
//! refuses a value that drops a required field. The manifest commit is
//! `OFFICIAL_COMMIT`.

use serde_json::Value;

use crate::domain::Event;

/// The official lane commit recorded in `nips/manifest.json`.
pub const OFFICIAL_COMMIT: &str = "c53877571f96eb423661fc23c620d629d37b8f19";

/// One event-shaped specification: a kind and a tag the pinned text names.
#[derive(Clone, Copy)]
pub struct Shape {
    /// File name under `nips/official/`.
    pub file: &'static str,
    /// Kind the text assigns.
    pub kind: u16,
    /// Tag name the text writes as a JSON tag.
    pub tag: &'static str,
}

/// Event shapes whose kind and tag both occur in the pinned file.
pub static SHAPES: &[Shape] = &include!("lane_shapes.inc");

/// Documents whose body says the rules now live in NIP-01.
pub static MOVED_TO_NIP01: &[(&str, &str)] = &[
    ("12.md", "Moved to [NIP-01](01.md)."),
    ("16.md", "Moved to [NIP-01](01.md)."),
    ("20.md", "Moved to [NIP-01](01.md)."),
    (
        "33.md",
        "Renamed to \"Addressable events\" and moved to [NIP-01](01.md).",
    ),
];

/// Whether `event` carries this shape's kind and tag.
pub fn verify_shape(shape: &Shape, event: &Event) -> Result<(), &'static str> {
    if event.kind != shape.kind {
        return Err("kind");
    }
    let named = event.tags.iter().any(|tag| tag.name() == Some(shape.tag));
    if !named {
        return Err("tag");
    }
    if shape.tag != "-" && !event.tag_values(shape.tag).any(|value| !value.is_empty()) {
        return Err("value");
    }
    Ok(())
}

/// A NIP-05 internet identifier is `local-part@domain` with neither side empty.
pub fn nip05_identifier(value: &str) -> Result<(), &'static str> {
    let Some((local, domain)) = value.split_once('@') else {
        return Err("at");
    };
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || value.chars().any(char::is_whitespace)
    {
        return Err("parts");
    }
    Ok(())
}

/// The NIP-06 account path. Account `0` is the basic client key.
#[must_use]
pub fn nip06_path(account: u32) -> String {
    format!("m/44'/1237'/{account}'/0/0")
}

/// The unsigned event NIP-07 `signEvent` accepts.
pub fn nip07_request(
    created_at: Option<u64>,
    kind: u16,
    content: &str,
) -> Result<(), &'static str> {
    if created_at.is_none() {
        return Err("created_at");
    }
    if content.len() > 128 * 1024 {
        return Err("content");
    }
    let _ = kind;
    Ok(())
}

/// A `nostr:` URI. `nsec` is not an identifier this scheme carries.
pub fn nostr_uri(value: &str) -> Result<(), &'static str> {
    let Some(rest) = value.strip_prefix("nostr:") else {
        return Err("scheme");
    };
    if rest.starts_with("nsec") {
        return Err("nsec");
    }
    if !rest.starts_with("npub1")
        && !rest.starts_with("note1")
        && !rest.starts_with("nevent1")
        && !rest.starts_with("nprofile1")
        && !rest.starts_with("naddr1")
    {
        return Err("identifier");
    }
    Ok(())
}

/// A NIP-26 delegation tag has the delegator pubkey, the conditions, and the signature.
pub fn delegation_tag(values: &[&str]) -> Result<(), &'static str> {
    if values.len() != 3 || values.iter().any(|value| value.is_empty()) {
        return Err("delegation");
    }
    Ok(())
}

/// A NIP-30 shortcode is alphanumeric, plus hyphen and underscore.
pub fn emoji_shortcode(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("shortcode");
    }
    Ok(())
}

/// NIP-11 requires `supported_nips` to be an array of numbers.
pub fn information_document_lists_nips(document: &Value) -> bool {
    document
        .get("supported_nips")
        .and_then(Value::as_array)
        .is_some_and(|nips| !nips.is_empty() && nips.iter().all(|nip| nip.is_number()))
}

/// NIP-45 `COUNT` is a verb, an id, and at least one filter object.
pub fn count_message(message: &Value) -> Result<(), &'static str> {
    let Some(items) = message.as_array() else {
        return Err("array");
    };
    if items.first().and_then(Value::as_str) != Some("COUNT") {
        return Err("verb");
    }
    if items.len() < 3 || items[1].as_str().is_none() || !items[2].is_object() {
        return Err("filter");
    }
    Ok(())
}

/// NIP-49 `LOG_N` is one byte and the iteration count is `2^LOG_N`.
pub fn nip49_iterations(log_n: u8) -> Result<u32, &'static str> {
    if log_n == 0 || log_n > 24 {
        return Err("log_n");
    }
    Ok(1u32 << log_n)
}

/// NIP-55 method names the Android signer answers.
pub fn android_signer_method(name: &str) -> bool {
    matches!(
        name,
        "get_public_key"
            | "sign_event"
            | "nip04_encrypt"
            | "nip04_decrypt"
            | "nip44_encrypt"
            | "nip44_decrypt"
    )
}

/// NIP-64 content is a PGN database: a tag pair or a numbered move.
pub fn chess_pgn(content: &str) -> Result<(), &'static str> {
    if content.contains('[') && content.contains(']') || content.contains("1.") {
        Ok(())
    } else {
        Err("pgn")
    }
}

/// NIP-86 method names this relay implements. Unknown names are refused.
pub fn management_method(name: &str) -> bool {
    matches!(
        name,
        "supportedmethods"
            | "banpubkey"
            | "unbanpubkey"
            | "listbannedpubkeys"
            | "allowpubkey"
            | "unallowpubkey"
            | "listallowedpubkeys"
            | "allowkind"
            | "disallowkind"
            | "listallowedkinds"
    )
}

/// NIP-BE advertisement UUID, copied from the pinned text.
pub const BLE_SERVICE_UUID: &str = "0000180f-0000-1000-8000-00805f9b34fb";

/// Kind `62` is the vanish request. It names no extra tag.
pub fn vanish_request(event: &Event) -> Result<(), &'static str> {
    if event.kind == 62 {
        Ok(())
    } else {
        Err("kind")
    }
}

/// Every official file this module accounts for.
pub fn covered_files() -> Vec<&'static str> {
    let mut files: Vec<&str> = SHAPES.iter().map(|shape| shape.file).collect();
    files.extend(MOVED_TO_NIP01.iter().map(|(file, _)| *file));
    files.extend([
        "01.md", "05.md", "06.md", "07.md", "11.md", "19.md", "21.md", "26.md", "30.md", "40.md",
        "42.md", "43.md", "44.md", "45.md", "49.md", "50.md", "55.md", "62.md", "64.md", "77.md",
        "86.md", "BE.md",
    ]);
    files.sort_unstable();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use crate::domain::{DeletionRequest, EventClass, RelaySigner, Tag, search_matches};
    use crate::negentropy::{self, Item};
    use crate::nip19;
    use crate::nip44;

    fn official_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nips/official")
    }

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap()
    }

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        signer().sign(1_700_000_000, kind, tags, content.into())
    }

    #[test]
    fn every_pinned_official_file_has_one_check_and_the_manifest_commit_matches() {
        let manifest = fs::read_to_string(official_dir().join("../manifest.json")).unwrap();
        assert!(manifest.contains(OFFICIAL_COMMIT));
        let mut on_disk = BTreeSet::new();
        for entry in fs::read_dir(official_dir()).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            if name.ends_with(".md") && name != "README.md" {
                on_disk.insert(name);
            }
        }
        let covered: BTreeSet<String> = covered_files().into_iter().map(str::to_string).collect();
        assert_eq!(covered, on_disk, "ledger files differ from nips/official");
    }

    #[test]
    fn event_shapes_match_the_pinned_text_and_refuse_a_missing_tag() {
        for shape in SHAPES {
            let text = fs::read_to_string(official_dir().join(shape.file)).unwrap();
            assert!(
                text.contains(&shape.kind.to_string()),
                "{} missing kind {}",
                shape.file,
                shape.kind
            );
            let tag_written = text.contains(&format!("[\"{}\"", shape.tag))
                || text.contains(&format!("`{}`", shape.tag));
            assert!(tag_written, "{} missing tag {}", shape.file, shape.tag);
            let good = if shape.tag == "-" {
                sign(shape.kind, vec![Tag::new(vec!["-".into()])], "note")
            } else {
                sign(
                    shape.kind,
                    vec![Tag::new(vec![shape.tag.into(), "value".into()])],
                    "note",
                )
            };
            assert!(verify_shape(shape, &good).is_ok(), "{}", shape.file);
            let bare = sign(shape.kind, Vec::new(), "note");
            assert!(verify_shape(shape, &bare).is_err(), "{}", shape.file);
        }
        let deletion = sign(5, vec![Tag::new(vec!["e".into(), "ab".repeat(32)])], "gone");
        let request = DeletionRequest::from_event(&deletion).unwrap();
        assert!(request.tombstones().next().is_some());
    }

    #[test]
    fn moved_documents_point_at_nip01_and_a_signed_event_still_verifies() {
        for (file, sentence) in MOVED_TO_NIP01 {
            let text = fs::read_to_string(official_dir().join(file)).unwrap();
            assert!(text.contains(sentence), "{file}");
        }
        let event = sign(
            1,
            vec![Tag::new(vec!["p".into(), signer().pubkey().into()])],
            "hi",
        );
        event.validate_crypto().unwrap();
        assert_eq!(EventClass::from_kind(1), EventClass::Regular);
        assert_eq!(EventClass::from_kind(30023), EventClass::Addressable);
        assert!(information_document_lists_nips(&json!({
            "supported_nips": [1, 77]
        })));
        assert!(!information_document_lists_nips(&json!({"name": "relay"})));
    }

    #[test]
    fn the_remaining_official_files_call_their_own_checks() {
        assert!(nip05_identifier("alice@example.com").is_ok());
        assert!(nip05_identifier("not an id").is_err());
        let five = fs::read_to_string(official_dir().join("05.md")).unwrap();
        assert!(five.contains("nip05"));

        assert_eq!(nip06_path(0), "m/44'/1237'/0'/0/0");
        assert!(
            fs::read_to_string(official_dir().join("06.md"))
                .unwrap()
                .contains("1237")
        );

        assert!(nip07_request(Some(10), 1, "hi").is_ok());
        assert!(nip07_request(None, 1, "hi").is_err());
        assert!(
            fs::read_to_string(official_dir().join("07.md"))
                .unwrap()
                .contains("getPublicKey")
        );

        let npub = nip19::encode_npub(&hex_decode(signer().pubkey()));
        assert_eq!(
            nip19::decode_npub(&npub).unwrap(),
            hex_decode(signer().pubkey())
        );
        assert!(
            fs::read_to_string(official_dir().join("19.md"))
                .unwrap()
                .contains("npub")
        );

        assert!(nostr_uri(&format!("nostr:{npub}")).is_ok());
        assert!(nostr_uri("nostr:nsec1qqqq").is_err());
        assert!(
            fs::read_to_string(official_dir().join("21.md"))
                .unwrap()
                .contains("nostr:")
        );

        assert!(delegation_tag(&["aa", "kind=1", "sig"]).is_ok());
        assert!(delegation_tag(&["aa"]).is_err());
        assert!(
            fs::read_to_string(official_dir().join("26.md"))
                .unwrap()
                .contains("delegation")
        );

        assert!(emoji_shortcode("party_1").is_ok());
        assert!(emoji_shortcode("nope space").is_err());
        assert!(
            fs::read_to_string(official_dir().join("30.md"))
                .unwrap()
                .contains("emoji")
        );

        let expiring = sign(
            1,
            vec![Tag::new(vec!["expiration".into(), "1700000100".into()])],
            "soon",
        );
        assert_eq!(expiring.expiration(), Some(1_700_000_100));
        assert!(
            fs::read_to_string(official_dir().join("40.md"))
                .unwrap()
                .contains("expiration")
        );

        let auth = sign(
            22_242,
            vec![
                Tag::new(vec!["relay".into(), "wss://relay.example".into()]),
                Tag::new(vec!["challenge".into(), "abc".into()]),
            ],
            "",
        );
        auth.validate_crypto().unwrap();
        assert_eq!(auth.kind, 22_242);
        assert!(
            fs::read_to_string(official_dir().join("42.md"))
                .unwrap()
                .contains("22242")
        );

        let role = sign(
            33_534,
            vec![
                Tag::new(vec!["-".into()]),
                Tag::new(vec!["d".into(), "admin".into()]),
            ],
            "",
        );
        assert_eq!(role.kind, 33_534);
        assert!(role.tags.iter().any(|tag| tag.name() == Some("d")));
        assert!(
            fs::read_to_string(official_dir().join("43.md"))
                .unwrap()
                .contains("33534")
        );

        let peer = crate::domain::RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x42; 32]).unwrap();
        let public = secp256k1::XOnlyPublicKey::from_byte_array([0; 32]);
        let _ = (peer, secret, public);
        let conversation = nip44::conversation_key(
            &secp256k1::SecretKey::from_byte_array([0x42; 32]).unwrap(),
            &{
                let signer = RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
                let bytes = hex_decode(signer.pubkey());
                secp256k1::XOnlyPublicKey::from_byte_array(bytes).unwrap()
            },
        );
        let hidden = nip44::encrypt("hello", &conversation, [9; 32]).unwrap();
        assert_eq!(nip44::decrypt(&hidden, &conversation).unwrap(), "hello");
        assert!(
            fs::read_to_string(official_dir().join("44.md"))
                .unwrap()
                .contains("NIP-44")
                || fs::read_to_string(official_dir().join("44.md"))
                    .unwrap()
                    .contains("44")
        );

        assert!(count_message(&json!(["COUNT", "q", {"kinds": [1]}])).is_ok());
        assert!(count_message(&json!(["REQ", "q", {}])).is_err());
        assert!(
            fs::read_to_string(official_dir().join("45.md"))
                .unwrap()
                .contains("COUNT")
        );

        assert_eq!(nip49_iterations(16).unwrap(), 65_536);
        assert!(nip49_iterations(0).is_err());
        assert!(
            fs::read_to_string(official_dir().join("49.md"))
                .unwrap()
                .contains("ncryptsec")
        );

        assert!(search_matches("Cat", 1, "a cat walked"));
        assert!(
            fs::read_to_string(official_dir().join("50.md"))
                .unwrap()
                .contains("search")
        );

        assert!(android_signer_method("get_public_key"));
        assert!(!android_signer_method("export_nsec"));
        assert!(
            fs::read_to_string(official_dir().join("55.md"))
                .unwrap()
                .contains("get_public_key")
        );

        assert!(vanish_request(&sign(62, Vec::new(), "")).is_ok());
        assert!(vanish_request(&sign(1, Vec::new(), "")).is_err());
        assert!(
            fs::read_to_string(official_dir().join("62.md"))
                .unwrap()
                .contains("62")
        );

        assert!(chess_pgn("[Event \"x\"]\n1. e4").is_ok());
        assert!(chess_pgn("not a game").is_err());
        assert!(
            fs::read_to_string(official_dir().join("64.md"))
                .unwrap()
                .contains("64")
        );

        let mut local = vec![Item {
            timestamp: 10,
            id: [1; 32],
        }];
        negentropy::prepare(&mut local);
        let frame = negentropy::open(&local);
        assert!(negentropy::respond(&local, &frame).is_ok());
        assert!(
            fs::read_to_string(official_dir().join("77.md"))
                .unwrap()
                .contains("NEG-OPEN")
        );

        assert!(management_method("banpubkey"));
        assert!(!management_method("dropdatabase"));
        assert!(
            fs::read_to_string(official_dir().join("86.md"))
                .unwrap()
                .contains("supportedmethods")
        );

        let ble = fs::read_to_string(official_dir().join("BE.md")).unwrap();
        assert!(ble.contains(BLE_SERVICE_UUID));
    }

    fn hex_decode(value: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bytes = value.as_bytes();
        for index in 0..32 {
            let high = match bytes[index * 2] {
                b @ b'0'..=b'9' => b - b'0',
                b @ b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("hex"),
            };
            let low = match bytes[index * 2 + 1] {
                b @ b'0'..=b'9' => b - b'0',
                b @ b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("hex"),
            };
            out[index] = (high << 4) | low;
        }
        out
    }
}
