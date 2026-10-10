//! Round trips, refusals, and the NIP-VAULT test vectors in
//! `fixtures/nips/vault/vectors.json`. Set `OA_VAULT_REGENERATE=1` to
//! rewrite the vectors after a deliberate format change.

use serde_json::{Value, json};

use crate::index::{Add, Index, Kind, Route};
use crate::keys::{Dek, Vmk};
use crate::object::{self, New, Tier, Wrap};
use crate::recovery::{self, Code};
use crate::slot::{self, Draft, Method, Slot};
use crate::{Error, b64, hex, unb64, unhex};

const VAULT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OBJECT: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const OTHER: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const SLOT_ID: &str = "4444444444444444444444444444444444444444444444444444444444444444";

fn vmk() -> Vmk {
    Vmk::from_bytes([7u8; 32])
}

fn plain(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

fn new_object(object: &str) -> New<'_> {
    New {
        object,
        vault: VAULT,
        tier: Tier::User,
        media: None,
        created_at: 1_760_000_000,
    }
}

fn sealed(object: &str, text: &[u8]) -> object::Sealed {
    object::seal_with(
        &new_object(object),
        text,
        Vec::new(),
        Dek::from_bytes([9u8; 32]),
        [5u8; 7],
        1024,
    )
    .unwrap()
}

#[test]
fn an_object_round_trips_across_chunks_and_when_empty() {
    for len in [0, 1, 1023, 1024, 1025, 4000] {
        let text = plain(len);
        let made = sealed(OBJECT, &text);
        assert_eq!(
            made.header.core.content.chunks as usize,
            len.div_ceil(1024).max(1)
        );
        let opened = object::open_object(&made.bytes, &Dek::from_bytes([9u8; 32])).unwrap();
        assert_eq!(opened.as_slice(), text.as_slice());
    }
}

#[test]
fn a_truncated_extended_or_reordered_stream_is_refused() {
    let made = sealed(OBJECT, &plain(3000));
    let dek = || Dek::from_bytes([9u8; 32]);
    let (_, start) = object::parse(&made.bytes).unwrap();
    let chunk = 1024 + 16;
    // Truncated: drop the final chunk.
    let cut = &made.bytes[..start + 2 * chunk];
    assert!(matches!(
        object::open_object(cut, &dek()),
        Err(Error::Decrypt(_))
    ));
    // Truncated mid-chunk.
    let cut = &made.bytes[..made.bytes.len() - 5];
    assert!(object::open_object(cut, &dek()).is_err());
    // Extended: a copy of the first chunk after the last.
    let mut long = made.bytes.clone();
    long.extend_from_slice(&made.bytes[start..start + chunk]);
    assert!(object::open_object(&long, &dek()).is_err());
    // Reordered: swap the first two chunks.
    let mut swapped = made.bytes.clone();
    let (a, b) = (start, start + chunk);
    let first = made.bytes[a..a + chunk].to_vec();
    swapped.copy_within(b..b + chunk, a);
    swapped[b..b + chunk].copy_from_slice(&first);
    assert!(object::open_object(&swapped, &dek()).is_err());
    // A changed byte.
    let mut flipped = made.bytes.clone();
    *flipped.last_mut().unwrap() ^= 1;
    assert!(object::open_object(&flipped, &dek()).is_err());
}

#[test]
fn a_wrap_moved_to_another_object_does_not_open() {
    let vmk = vmk();
    let one = sealed(OBJECT, b"one");
    let two = sealed(OTHER, b"two");
    let wrap = object::wrap_user(&vmk, &one.header.core_digest().unwrap(), &one.dek).unwrap();
    assert!(object::unwrap_user(&vmk, &one.header.core_digest().unwrap(), &wrap).is_ok());
    assert!(matches!(
        object::unwrap_user(&vmk, &two.header.core_digest().unwrap(), &wrap),
        Err(Error::Decrypt(_))
    ));
    // Another vault's key doesn't open it either.
    assert!(
        object::unwrap_user(
            &Vmk::from_bytes([8u8; 32]),
            &one.header.core_digest().unwrap(),
            &wrap
        )
        .is_err()
    );
}

#[test]
fn a_header_must_be_canonical_and_known() {
    let made = sealed(OBJECT, b"x");
    let (_, start) = object::parse(&made.bytes).unwrap();
    let head = std::str::from_utf8(&made.bytes[12..start]).unwrap();
    let spaced = head.replacen(':', ": ", 1);
    let mut bytes = b"OAVAULT1".to_vec();
    bytes.extend_from_slice(&(spaced.len() as u32).to_be_bytes());
    bytes.extend_from_slice(spaced.as_bytes());
    bytes.extend_from_slice(&made.bytes[start..]);
    assert!(object::parse(&bytes).is_err());
    assert!(object::parse(b"NOTVAULT\0\0\0\0").is_err());
    // An operator wrap on a user object is refused.
    let wrong = object::seal_with(
        &new_object(OBJECT),
        b"x",
        vec![Wrap::Operator {
            kms_key: "k".into(),
            dek: "AA==".into(),
        }],
        Dek::from_bytes([1; 32]),
        [0; 7],
        1024,
    );
    assert!(wrong.is_err());
}

fn draft(method: Method, params: serde_json::Map<String, Value>) -> Draft {
    Draft {
        vault: VAULT.into(),
        slot: SLOT_ID.into(),
        method,
        params,
        label: "Test device".into(),
        created_at: 1_760_000_000,
    }
}

#[test]
fn every_slot_method_opens_with_its_secret_only() {
    let vmk = vmk();
    let secret = [3u8; 32];
    let cases = [
        draft(
            Method::PasskeyPrf,
            slot::passkey_params("openagents.com", b"cred-id", &[4u8; 32]),
        ),
        draft(
            Method::Device,
            slot::device_params("macos", "openagents.vault.device"),
        ),
        draft(Method::Nostr, slot::nostr_params(&"ab".repeat(32), "Ag==")),
        draft(
            Method::Recovery,
            recovery::params_with(16, &[6u8; 16]).unwrap(),
        ),
        draft(Method::Pairing, slot::pairing_params(1_760_000_600)),
    ];
    for draft in cases {
        let made = Slot::seal_with(draft, &secret, &vmk, [2u8; 12]).unwrap();
        assert!(made.open(&secret).unwrap().same(&vmk));
        assert!(matches!(made.open(&[4u8; 32]), Err(Error::Decrypt(_))));
        // The label and parameters are bound: a renamed slot doesn't open.
        let mut renamed = made.clone();
        renamed.label = "Someone else".into();
        assert!(renamed.open(&secret).is_err());
        let json = serde_json::to_string(&made).unwrap();
        let back: Slot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, made);
    }
}

#[test]
fn slot_parameters_must_match_the_method() {
    let vmk = vmk();
    assert!(
        Slot::seal(
            draft(Method::Device, slot::passkey_params("a", b"b", &[0; 32])),
            &[1; 32],
            &vmk
        )
        .is_err()
    );
    assert!(
        Slot::seal(
            draft(
                Method::Recovery,
                recovery::params_with(10, &[0; 16]).unwrap()
            ),
            &[1; 32],
            &vmk
        )
        .is_err()
    );
    // A pairing link lives at most 15 minutes.
    assert!(
        Slot::seal(
            draft(Method::Pairing, slot::pairing_params(1_760_000_000 + 3600)),
            &[1; 32],
            &vmk
        )
        .is_err()
    );
    assert!(
        Slot::seal(
            draft(Method::Device, slot::device_params("macos", "k")),
            &[1; 16],
            &vmk
        )
        .is_err()
    );
}

#[test]
fn a_vault_needs_a_recovery_slot_and_a_second_one() {
    let vmk = vmk();
    let passkey = Slot::seal(
        draft(
            Method::PasskeyPrf,
            slot::passkey_params("a", b"b", &[0; 32]),
        ),
        &[1; 32],
        &vmk,
    )
    .unwrap();
    let code = Slot::seal(
        draft(
            Method::Recovery,
            recovery::params_with(16, &[0; 16]).unwrap(),
        ),
        &[1; 32],
        &vmk,
    )
    .unwrap();
    let pairing = Slot::seal(
        draft(Method::Pairing, slot::pairing_params(1_760_000_100)),
        &[1; 32],
        &vmk,
    )
    .unwrap();
    assert!(!slot::enough(std::slice::from_ref(&passkey)));
    assert!(!slot::enough(&[passkey.clone(), pairing.clone()]));
    assert!(!slot::enough(&[code.clone(), pairing]));
    assert!(slot::enough(&[passkey, code]));
}

#[test]
fn a_recovery_code_parses_loosely_and_checks_its_checksum() {
    let code = Code::from_entropy(&[0x5a; 32]).unwrap();
    let words = code.words().to_owned();
    assert_eq!(words.split(' ').count(), 24);
    let typed = format!("  {}  ", words.to_uppercase().replace(' ', "   "));
    let parsed = Code::parse(&typed).unwrap();
    assert_eq!(parsed.words(), words);
    let mut wrong: Vec<&str> = words.split(' ').collect();
    wrong.swap(0, 1);
    if wrong[0] != wrong[1] {
        assert!(Code::parse(&wrong.join(" ")).is_err());
    }
    assert!(Code::parse("abandon abandon").is_err());
    let secret = parsed.secret(16, &[1; 16]).unwrap();
    assert_eq!(secret.as_ref(), code.secret(16, &[1; 16]).unwrap().as_ref());
    assert!(code.secret(12, &[1; 16]).is_err());
}

fn add(name: &str) -> Add<'_> {
    Add {
        kind: Kind::File,
        name,
        media: Some("text/plain"),
        project: None,
        about: Vec::new(),
        route: None,
        created_at: 1_760_000_000,
    }
}

#[test]
fn the_index_opens_files_and_removal_shreds_them() {
    let vmk = vmk();
    let first = Index::new(VAULT).unwrap();
    let (bytes, second) = first
        .add(&vmk, add("statement.txt"), b"balance 100")
        .unwrap();
    assert_eq!(second.epoch, 2);
    let entry = second.entries[0].clone();
    assert_eq!(
        second
            .open_object(&vmk, &entry.object, &bytes)
            .unwrap()
            .as_slice(),
        b"balance 100"
    );
    // The stored object carries no wrap and no file name or media type.
    let (header, _) = object::parse(&bytes).unwrap();
    assert!(header.wraps.is_empty());
    assert!(header.core.media.is_none());
    assert!(!String::from_utf8_lossy(&bytes).contains("statement"));
    // Sealed and reopened, the index is the same; another key can't open it.
    let blob = second.seal(&vmk).unwrap();
    assert_eq!(Index::open(&vmk, VAULT, &blob).unwrap(), second);
    assert!(Index::open(&Vmk::from_bytes([8; 32]), VAULT, &blob).is_err());
    assert!(Index::open(&vmk, OTHER, &blob).is_err());
    // Removal: the next epoch has no key for the file. With the old epoch
    // deleted, the same key and the kept ciphertext open nothing.
    let third = second.remove(&entry.object).unwrap();
    assert_eq!(third.epoch, 3);
    let third_blob = third.seal(&vmk).unwrap();
    let reopened = Index::open(&vmk, VAULT, &third_blob).unwrap();
    assert!(reopened.open_object(&vmk, &entry.object, &bytes).is_err());
    // The data key isn't anywhere in the new index's plaintext.
    let plain = crate::jcs::to_string(&reopened).unwrap();
    if let Wrap::User { dek, .. } = &entry.wraps[0] {
        assert!(!plain.contains(dek.as_str()));
    }
}

#[test]
fn the_index_refuses_a_substituted_object_and_rewraps_on_rotation() {
    let vmk = vmk();
    let (one, index) = Index::new(VAULT)
        .unwrap()
        .add(&vmk, add("a"), b"aaa")
        .unwrap();
    let (two, index) = index.add(&vmk, add("b"), b"bbb").unwrap();
    let a = index.entries[0].object.clone();
    let b = index.entries[1].object.clone();
    // The service hands back b's bytes for a: refused.
    assert!(index.open_object(&vmk, &a, &two).is_err());
    let new = Vmk::from_bytes([11; 32]);
    let rotated = index.rewrap(&vmk, &new).unwrap();
    assert_eq!(
        rotated.open_object(&new, &a, &one).unwrap().as_slice(),
        b"aaa"
    );
    assert_eq!(
        rotated.open_object(&new, &b, &two).unwrap().as_slice(),
        b"bbb"
    );
    assert!(rotated.open_object(&vmk, &a, &one).is_err());
}

#[test]
fn jcs_sorts_keys_and_escapes_like_javascript() {
    let value = json!({"b": 1, "a": [true, null, "x\u{1}\"\n"], "é": "ü"});
    assert_eq!(
        crate::jcs::to_string(&value).unwrap(),
        "{\"a\":[true,null,\"x\\u0001\\\"\\n\"],\"b\":1,\"é\":\"ü\"}"
    );
    assert!(crate::jcs::to_string(&json!({"f": 1.5})).is_err());
}

/// The vectors: every value is derived from fixed inputs, so any client can
/// reproduce them.
fn vectors() -> Value {
    let vmk_bytes = [0x42u8; 32];
    let vmk = Vmk::from_bytes(vmk_bytes);
    let text = plain(2500);
    let made = object::seal_with(
        &new_object(OBJECT),
        &text,
        Vec::new(),
        Dek::from_bytes([0x24; 32]),
        [1, 2, 3, 4, 5, 6, 7],
        1024,
    )
    .unwrap();
    let digest = made.header.core_digest().unwrap();
    let wrap = object::wrap_user_with(&vmk, &digest, &made.dek, [0x0c; 12]).unwrap();
    let (_, start) = object::parse(&made.bytes).unwrap();
    let chunk = 1024 + 16;
    let truncated = b64(&made.bytes[..start + 2 * chunk]);
    let mut extended = made.bytes.clone();
    extended.extend_from_slice(&made.bytes[start..start + chunk]);
    let mut reordered = made.bytes.clone();
    let first = made.bytes[start..start + chunk].to_vec();
    reordered.copy_within(start + chunk..start + 2 * chunk, start);
    reordered[start + chunk..start + 2 * chunk].copy_from_slice(&first);
    let other = object::seal_with(
        &new_object(OTHER),
        b"other",
        Vec::new(),
        Dek::from_bytes([0x25; 32]),
        [7; 7],
        1024,
    )
    .unwrap();

    let code = Code::from_entropy(&[0x5a; 32]).unwrap();
    let recovery_secret = code.secret(16, &[0x33; 16]).unwrap();
    let methods: Vec<(Method, serde_json::Map<String, Value>, Vec<u8>)> = vec![
        (
            Method::PasskeyPrf,
            slot::passkey_params("openagents.com", b"credential-1", &[0x51; 32]),
            vec![0x61; 32],
        ),
        (
            Method::Device,
            slot::device_params("ios", "com.openagents.vault.device"),
            vec![0x62; 32],
        ),
        (
            Method::Nostr,
            slot::nostr_params(&"ab".repeat(32), "AnNlYWxlZA=="),
            vec![0x63; 32],
        ),
        (
            Method::Recovery,
            recovery::params_with(16, &[0x33; 16]).unwrap(),
            recovery_secret.to_vec(),
        ),
        (
            Method::Pairing,
            slot::pairing_params(1_760_000_600),
            vec![0x64; 32],
        ),
    ];
    let slots: Vec<Value> = methods
        .into_iter()
        .map(|(method, params, secret)| {
            let made = Slot::seal_with(
                Draft {
                    vault: VAULT.into(),
                    slot: SLOT_ID.into(),
                    method,
                    params,
                    label: "Vector".into(),
                    created_at: 1_760_000_000,
                },
                &secret,
                &vmk,
                [0x0d; 12],
            )
            .unwrap();
            json!({ "method_secret": hex(&secret), "slot": made })
        })
        .collect();

    let mut index = Index::new(VAULT).unwrap();
    index.epoch = 2;
    index.entries.push(crate::index::Entry {
        object: OBJECT.into(),
        kind: Kind::File,
        name: "statement.pdf".into(),
        media: Some("application/pdf".into()),
        size: text.len() as u64,
        project: None,
        about: Vec::new(),
        route: None,
        core_digest: hex(&digest),
        wraps: vec![wrap.clone()],
        created_at: 1_760_000_000,
    });
    let index_blob = index.seal_with(&vmk, [0x0e; 12]).unwrap();
    let mut destroyed = index.next().unwrap();
    destroyed.entries.clear();
    let destroyed_blob = destroyed.seal_with(&vmk, [0x0f; 12]).unwrap();
    let _ = Route::Device;

    json!({
        "v": "openagents.vault-vectors.v1",
        "note": "Generated by crates/oa-vault (cargo test -p oa-vault). Binary values are standard base64; keys and digests are lowercase hex.",
        "keys": {
            "vmk": hex(&vmk_bytes),
            "k_user_wrap": hex(vmk.user_wrap_key().as_ref()),
            "s_user": hex(vmk.user_share().as_ref()),
            "user_key_id": vmk.user_key_id(),
            "index_key_epoch_1": hex(vmk.index_key(1).as_ref()),
        },
        "object": {
            "vault": VAULT,
            "object": OBJECT,
            "dek": hex(&[0x24; 32]),
            "nonce_prefix": b64(&[1, 2, 3, 4, 5, 6, 7]),
            "chunk_bytes": 1024,
            "created_at": 1_760_000_000,
            "plaintext": b64(&text),
            "core_digest": hex(&digest),
            "bytes": b64(&made.bytes),
            "user_wrap": wrap,
            "refuse": {
                "truncated": truncated,
                "extended": b64(&extended),
                "reordered": b64(&reordered),
                "wrap_moved_to": b64(&other.bytes),
            },
        },
        "slots": slots,
        "recovery": {
            "entropy": hex(&[0x5a; 32]),
            "words": code.words(),
            "log_n": 16,
            "salt": b64(&[0x33; 16]),
            "secret": hex(recovery_secret.as_ref()),
        },
        "index": {
            "plaintext": index,
            "sealed": b64(&index_blob),
            "after_delete": b64(&destroyed_blob),
        },
    })
}

#[test]
fn the_nip_vault_vectors_hold() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/nips/vault/vectors.json");
    let made = vectors();
    if std::env::var_os("OA_VAULT_REGENERATE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&made).unwrap() + "\n").unwrap();
    }
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        kept, made,
        "the vectors changed; regenerate with OA_VAULT_REGENERATE=1 only for a deliberate format change"
    );

    // And each vector checks out from its inputs alone.
    let vmk = Vmk::from_bytes(unhex::<32>(kept["keys"]["vmk"].as_str().unwrap()).unwrap());
    let object = &kept["object"];
    let bytes = unb64(object["bytes"].as_str().unwrap()).unwrap();
    let wrap: Wrap = serde_json::from_value(object["user_wrap"].clone()).unwrap();
    let digest = unhex::<32>(object["core_digest"].as_str().unwrap()).unwrap();
    let dek = object::unwrap_user(&vmk, &digest, &wrap).unwrap();
    assert_eq!(
        object::open_object(&bytes, &dek).unwrap().as_slice(),
        unb64(object["plaintext"].as_str().unwrap())
            .unwrap()
            .as_slice()
    );
    for case in ["truncated", "extended", "reordered"] {
        let bad = unb64(object["refuse"][case].as_str().unwrap()).unwrap();
        assert!(
            object::open_object(&bad, &object::unwrap_user(&vmk, &digest, &wrap).unwrap()).is_err(),
            "{case}"
        );
    }
    let moved = unb64(object["refuse"]["wrap_moved_to"].as_str().unwrap()).unwrap();
    let (moved_header, _) = object::parse(&moved).unwrap();
    assert!(object::unwrap_user(&vmk, &moved_header.core_digest().unwrap(), &wrap).is_err());
    for slot in kept["slots"].as_array().unwrap() {
        let parsed: Slot = serde_json::from_value(slot["slot"].clone()).unwrap();
        let secret = crate::unhex::<32>(slot["method_secret"].as_str().unwrap()).unwrap();
        assert!(parsed.open(&secret).unwrap().same(&vmk));
    }
    let index = Index::open(
        &vmk,
        VAULT,
        &unb64(kept["index"]["sealed"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(index.open_object(&vmk, OBJECT, &bytes).unwrap().len(), 2500);
    let after = Index::open(
        &vmk,
        VAULT,
        &unb64(kept["index"]["after_delete"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert!(after.open_object(&vmk, OBJECT, &bytes).is_err());
}
