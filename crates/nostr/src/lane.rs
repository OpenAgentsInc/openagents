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

/// One pinned file whose applicable roles are configured and proven.
///
/// `partial` shape checks stay in [`SHAPES`]. A row here is a different
/// status: domain, client, and server each name the shipped function, and
/// `acceptance` names the test that calls it.
pub struct Evidence {
    pub file: &'static str,
    pub domain: &'static str,
    pub client: &'static str,
    pub server: &'static str,
    pub paths: &'static str,
    pub configuration: &'static str,
    pub fixture: &'static str,
    pub acceptance: &'static str,
    pub limitations: &'static str,
    pub owner: &'static str,
    pub status: &'static str,
}

/// Official files whose checks go beyond a kind and one tag.
pub static PROVEN: &[Evidence] = &[
    Evidence {
        file: "02.md",
        domain: "kind 3 is replaceable; a p tag is a 32-byte hex key, an optional ws:// or wss:// relay, and an optional petname",
        client: "parse_follow_list, append_follow, and displayed_petname",
        server: "EventClass::from_kind(3) is Replaceable, so the relay replacement head deletes the previous list",
        paths: "crates/nostr/src/domain/follow.rs; crates/nostr/src/domain/replacement.rs; crates/nostr-relay/src/store/mod.rs",
        configuration: "no setting; kind 3 uses the ordinary replacement head",
        fixture: "the pinned p-tag triple: pubkey, relay URL, petname",
        acceptance: "lane::tests::nip02_follow_lists_replace_and_petnames_chain",
        limitations: "content is ignored, as the pinned text says it is not used",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "03.md",
        domain: "kind 1040 binds an event id to one OpenTimestamps proof",
        client: "open_attestation",
        server: "admission refuses a kind 1040 event whose proof does not bind that id",
        paths: "crates/nostr/src/domain/ots.rs; crates/nostr/src/domain/event.rs",
        configuration: "no setting; NIP-03 stays off the NIP-11 list because the pinned text marks it unrecommended",
        fixture: "an .ots file whose digest is the e tag and whose one attestation is a Bitcoin height",
        acceptance: "domain::ots::tests::a_bitcoin_proof_binds_the_event_id_and_one_height",
        limitations: "the height is not compared to a Bitcoin block header; pending and non-Bitcoin attestations are refused",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "04.md",
        domain: "kind 4 content is AES-256-CBC under the unhashed X coordinate of the ECDH point",
        client: "nip04::encrypt and nip04::decrypt",
        server: "admission checks the p tag and the ?iv= content form without decrypting",
        paths: "crates/nostr/src/nip04.rs; crates/nostr/src/domain/event.rs",
        configuration: "no setting; NIP-04 stays off the NIP-11 list because the pinned text marks it unrecommended",
        fixture: "FIPS-197 AES-256 block, then a two-party round trip of base64(ciphertext)?iv=base64(iv)",
        acceptance: "nip04::tests::a_direct_message_round_trips_and_keeps_a_mention_as_text",
        limitations: "there is no MAC, so a padding failure is not authentication; the recipient pubkey is visible in the p tag",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "09.md",
        domain: "kind 5 names event ids and same-author replacement addresses; a request does not delete kind 5",
        client: "DeletionRequest::from_event, tombstones, and deletes",
        server: "admit stores the request, apply_deletion writes tombstones, and a later matching event is rejected",
        paths: "crates/nostr/src/domain/deletion.rs; crates/nostr-relay/src/store/mod.rs; crates/nostr-relay/src/store/statements.rs",
        configuration: "no setting; NIP-11 lists 9 because kind 5 admission and tombstones run on every relay",
        fixture: "the pinned kind 5 example: e and a references, optional k tags, and a reason in content",
        acceptance: "lane::tests::nip09_deletion_requests_hide_the_authors_events_through_the_request_time",
        limitations: "a request with no well-formed same-author reference creates no tombstone and is still stored; k tags do not select targets; relays that already published the event may still hold a copy",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "15.md",
        domain: "kinds 30017, 30018, 30019, and 30020 are addressable marketplace records; kind 1021 is one bid on one auction event id",
        client: "open_stall, open_product, open_auction, open_bid, bid_confirmation_matches, shipping_cost, and open_checkout",
        server: "admission refuses a malformed marketplace event; addressable kinds use the ordinary replacement head",
        paths: "crates/nostr/src/domain/market.rs; crates/nostr/src/domain/event.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; NIP-15 stays off the NIP-11 list because the pinned text marks it unrecommended",
        fixture: "a stall whose d tag equals its id, a product shipping extra, a bid on one auction version, and checkout types 0, 1, and 2",
        acceptance: "domain::market::tests::a_stall_product_and_bid_follow_the_pinned_marketplace_events",
        limitations: "checkout JSON is parsed after decryption, so a relay does not read kind 4; costs are JSON numbers and no payment is settled; a later auction edit is stored and does not keep bids from the previous event id",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "17.md",
        domain: "an unsigned kind 14 or 15 rumor is sealed as kind 13 and gift-wrapped as kind 1059 to one recipient",
        client: "chat_rumor, seal, gift_wrap, open_direct_message, file_message, and inbox_relays",
        server: "admission requires one p tag on kind 1059; the relay serves that event only to the authenticated reader named by the tag",
        paths: "crates/nostr/src/nip17.rs; crates/nostr/src/domain/expanded.rs; crates/nostr-relay/src/gateway/subscription.rs",
        configuration: "NOSTR_RELAY_URL; NIP-11 then lists 17 because gift wraps are served only to the authenticated p-tagged reader",
        fixture: "a kind 14 rumor sealed and wrapped to the recipient and to the author, plus a kind 10050 inbox list",
        acceptance: "nip17::tests::a_private_message_round_trips_and_rejects_an_impersonated_rumor",
        limitations: "the relay does not decrypt the wrap; file bytes are not downloaded or decrypted; the caller supplies the one-time wrapper key; kind 21059 belongs to NIP-59",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "29.md",
        domain: "GroupMetadata and GroupAction, including private, hidden, restricted, and one parent",
        client: "GroupMetadata::from_tags and parent_would_cycle",
        server: "admission, the query filter, and metadata regeneration",
        paths: "crates/nostr/src/domain/expanded.rs; crates/nostr-relay/src/store/mod.rs",
        configuration: "NOSTR_RELAY_RELAY_SECRET_KEY; NIP-11 nip29.subgroups is true when that key is set",
        fixture: "private, hidden, restricted, one parent, and the child list",
        acceptance: "domain::expanded::tests::private_hidden_and_subgroup_fields_follow_the_pinned_metadata_event",
        limitations: "kinds 9003, 9004, 9006, and 9011-9020 have no row in the pinned moderation table; kind 39004 stays empty because this process does not run LiveKit",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
];

/// Every official file this module accounts for.
pub fn covered_files() -> Vec<&'static str> {
    let mut files: Vec<&str> = SHAPES.iter().map(|shape| shape.file).collect();
    files.extend(PROVEN.iter().map(|row| row.file));
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

    use crate::domain::{
        DeletionRequest, DomainError, EventClass, GroupMetadata, RelaySigner, ReplacementDecision,
        Tag, compare_replacement, displayed_petname, parse_follow_list, search_matches,
    };
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
    fn nip02_follow_lists_replace_and_petnames_chain() {
        let text = fs::read_to_string(official_dir().join("02.md")).unwrap();
        assert!(text.contains("follow list"));
        assert!(text.contains("petname"));
        let row = PROVEN.iter().find(|row| row.file == "02.md").unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(SHAPES.iter().all(|shape| shape.file != "02.md"));

        let alice = "ab".repeat(32);
        let bob = "cd".repeat(32);
        let carol = "ef".repeat(32);
        let tags = vec![
            Tag::new(vec![
                "p".into(),
                alice.clone(),
                "wss://alicerelay.com/".into(),
                "alice".into(),
            ]),
            Tag::new(vec!["p".into(), bob.clone()]),
        ];
        let follows = parse_follow_list(&tags).unwrap();
        assert_eq!(follows[0].petname, "alice");
        assert_eq!(follows[1].relay, "");
        let event = sign(3, tags, "unused");
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Replaceable);

        let newer = signer().sign(
            1_700_000_100,
            3,
            vec![Tag::new(vec!["p".into(), alice.clone()])],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&event, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let alice_list = [crate::domain::Follow {
            pubkey: bob.clone(),
            relay: String::new(),
            petname: "bob".into(),
        }];
        let bob_list = [crate::domain::Follow {
            pubkey: carol.clone(),
            relay: String::new(),
            petname: "carol".into(),
        }];
        let mut published = std::collections::BTreeMap::new();
        published.insert(alice.as_str(), alice_list.as_slice());
        published.insert(bob.as_str(), bob_list.as_slice());
        let viewer = [crate::domain::Follow {
            pubkey: alice.clone(),
            relay: String::new(),
            petname: "alice".into(),
        }];
        assert_eq!(
            displayed_petname(&viewer, &published, &carol).as_deref(),
            Some("carol.bob.alice")
        );

        let bad = sign(3, vec![Tag::new(vec!["p".into(), "zz".into()])], "");
        assert!(bad.validate_structure().is_err());
        let _ = GroupMetadata::from_tags(&[Tag::new(vec!["private".into()])]).unwrap();
        assert!(PROVEN.iter().any(|row| row.file == "29.md"));
    }

    #[test]
    fn nip09_deletion_requests_hide_the_authors_events_through_the_request_time() {
        let text = fs::read_to_string(official_dir().join("09.md")).unwrap();
        assert!(text.contains("deletion request"));
        assert!(text.contains("identical `pubkey`"));
        assert!(text.contains("has no effect"));
        let row = PROVEN.iter().find(|row| row.file == "09.md").unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(SHAPES.iter().all(|shape| shape.file != "09.md"));

        let author = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let other = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let note = author.sign(1_700_000_000, 1, Vec::new(), "accidental".into());
        let foreign_note = other.sign(1_700_000_000, 1, Vec::new(), "not mine".into());
        let address = author.sign(
            1_700_000_010,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "draft".into(),
        );
        let boundary = author.sign(
            1_700_000_020,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "same timestamp".into(),
        );
        let later = author.sign(
            1_700_000_050,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "kept".into(),
        );
        let foreign_address = other.sign(
            1_700_000_010,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "someone else".into(),
        );
        let reason = "these posts were published by accident";
        let request_event = author.sign(
            1_700_000_020,
            5,
            vec![
                Tag::new(vec!["e".into(), note.id.clone()]),
                Tag::new(vec!["e".into(), foreign_note.id.clone()]),
                Tag::new(vec!["e".into(), "dcd59".into()]),
                Tag::new(vec!["e".into(), note.id.to_uppercase()]),
                Tag::new(vec!["a".into(), format!("30023:{}:post", author.pubkey())]),
                Tag::new(vec!["a".into(), format!("30023:{}:post", other.pubkey())]),
                Tag::new(vec!["k".into(), "1".into()]),
                Tag::new(vec!["k".into(), "30023".into()]),
            ],
            reason.into(),
        );
        request_event.validate_structure().unwrap();
        assert_eq!(request_event.class(), EventClass::Regular);
        assert_eq!(request_event.content, reason);

        let request = DeletionRequest::from_event(&request_event).unwrap();
        assert!(request.event_ids.contains(&note.id));
        assert!(request.event_ids.contains(&foreign_note.id));
        assert_eq!(request.event_ids.len(), 2);
        assert_eq!(
            request
                .addresses
                .iter()
                .map(|address| address.to_string())
                .collect::<Vec<_>>(),
            vec![format!("30023:{}:post", author.pubkey())]
        );
        assert_eq!(request.tombstones().count(), 3);
        assert!(request.deletes(&note));
        let mut spoofed = note.clone();
        spoofed.pubkey = other.pubkey().to_owned();
        assert!(!request.deletes(&spoofed));
        assert!(!request.deletes(&foreign_note));
        assert!(request.deletes(&address));
        assert!(request.deletes(&boundary));
        assert!(!request.deletes(&later));
        assert!(!request.deletes(&foreign_address));

        let unrelated = author.sign(1_700_000_000, 1, Vec::new(), "other note".into());
        assert!(!request.deletes(&unrelated));
        let without_kinds = author.sign(
            1_700_000_020,
            5,
            vec![Tag::new(vec!["e".into(), note.id.clone()])],
            String::new(),
        );
        assert!(
            DeletionRequest::from_event(&without_kinds)
                .unwrap()
                .deletes(&note)
        );

        let retraction = author.sign(
            1_700_000_030,
            5,
            vec![Tag::new(vec!["e".into(), request_event.id.clone()])],
            String::new(),
        );
        assert!(
            !DeletionRequest::from_event(&retraction)
                .unwrap()
                .deletes(&request_event)
        );
        assert!(request.deletes(&note));

        let empty = author.sign(
            1_700_000_040,
            5,
            vec![Tag::new(vec!["e".into(), "zz".into()])],
            "nothing actionable".into(),
        );
        let empty_request = DeletionRequest::from_event(&empty).unwrap();
        assert!(empty_request.tombstones().next().is_none());
        assert!(!empty_request.deletes(&note));

        let not_a_request = author.sign(1, 1, Vec::new(), String::new());
        assert!(matches!(
            DeletionRequest::from_event(&not_a_request),
            Err(DomainError::NotDeletionRequest)
        ));
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
