//! Runs every vector in the official NIP-44 v2 set against this crate.
//!
//! `fixtures/README.md` records the source, commit, checksum, and license of
//! `fixtures/nip44/nip44.vectors.json`, and the one place the vectors and
//! the pinned specification disagree.

mod common;

use common::{fixture_json, hex_array, lower_hex};
use nostr::nip44::{self, MAX_CLIENT_PLAINTEXT_BYTES, primitives};
use secp256k1::{Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn vectors() -> Value {
    fixture_json("nip44/nip44.vectors.json")["v2"].clone()
}

fn field<'a>(vector: &'a Value, name: &str) -> &'a str {
    vector[name]
        .as_str()
        .unwrap_or_else(|| panic!("vector lacks string field {name}: {vector}"))
}

fn x_only_from_secret(secret: &SecretKey) -> XOnlyPublicKey {
    secret.public_key(&Secp256k1::new()).x_only_public_key().0
}

#[test]
fn vector_file_matches_the_checksum_the_pinned_spec_publishes() {
    let bytes = std::fs::read(common::fixture_path("nip44/nip44.vectors.json")).unwrap();
    assert_eq!(
        lower_hex(&Sha256::digest(&bytes)),
        "269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040"
    );
}

#[test]
fn valid_conversation_keys() {
    let cases = vectors()["valid"]["get_conversation_key"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 35);
    for case in cases {
        let secret = SecretKey::from_byte_array(hex_array::<32>(field(case, "sec1"))).unwrap();
        let peer = XOnlyPublicKey::from_byte_array(hex_array::<32>(field(case, "pub2"))).unwrap();
        assert_eq!(
            lower_hex(&nip44::conversation_key(&secret, &peer)),
            field(case, "conversation_key"),
            "{case}"
        );
    }
}

#[test]
fn invalid_conversation_key_inputs_are_refused_by_key_parsing() {
    // `conversation_key` takes typed keys, so an out-of-range scalar or an
    // x coordinate that is not on the curve is refused before ECDH runs.
    let cases = vectors()["invalid"]["get_conversation_key"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let secret = SecretKey::from_byte_array(hex_array::<32>(field(case, "sec1")));
        let peer = XOnlyPublicKey::from_byte_array(hex_array::<32>(field(case, "pub2")));
        assert!(
            secret.is_err() || peer.is_err(),
            "expected refusal: {}",
            field(case, "note")
        );
    }
}

#[test]
fn valid_message_keys() {
    let set = vectors()["valid"]["get_message_keys"].clone();
    let conversation_key = hex_array::<32>(field(&set, "conversation_key"));
    let keys = set["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 32);
    for case in keys {
        let nonce = hex_array::<32>(field(case, "nonce"));
        let derived = primitives::message_keys(&conversation_key, &nonce);
        assert_eq!(
            lower_hex(&derived[..32]),
            field(case, "chacha_key"),
            "{case}"
        );
        assert_eq!(
            lower_hex(&derived[32..44]),
            field(case, "chacha_nonce"),
            "{case}"
        );
        assert_eq!(lower_hex(&derived[44..]), field(case, "hmac_key"), "{case}");
    }
}

#[test]
fn valid_padded_lengths() {
    let cases = vectors()["valid"]["calc_padded_len"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 24);
    for case in cases {
        let length = usize::try_from(case[0].as_u64().unwrap()).unwrap();
        let expected = usize::try_from(case[1].as_u64().unwrap()).unwrap();
        assert_eq!(primitives::padded_length(length), Ok(expected), "{case}");
        let padded = primitives::pad(&vec![0x61; length]).unwrap();
        let prefix = if length >= 65_536 { 6 } else { 2 };
        assert_eq!(padded.len(), prefix + expected, "{case}");
    }
}

#[test]
fn valid_encrypt_decrypt() {
    let cases = vectors()["valid"]["encrypt_decrypt"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let secret_one = SecretKey::from_byte_array(hex_array::<32>(field(case, "sec1"))).unwrap();
        let secret_two = SecretKey::from_byte_array(hex_array::<32>(field(case, "sec2"))).unwrap();
        let key = nip44::conversation_key(&secret_one, &x_only_from_secret(&secret_two));
        assert_eq!(lower_hex(&key), field(case, "conversation_key"), "{case}");
        assert_eq!(
            key,
            nip44::conversation_key(&secret_two, &x_only_from_secret(&secret_one)),
            "conversation key must not depend on the role: {case}"
        );
        let nonce = hex_array::<32>(field(case, "nonce"));
        let plaintext = field(case, "plaintext");
        let payload = nip44::encrypt(plaintext, &key, nonce).unwrap();
        assert_eq!(payload, field(case, "payload"), "{case}");
        assert_eq!(nip44::decrypt(&payload, &key).unwrap(), plaintext, "{case}");
    }
}

#[test]
fn valid_encrypt_decrypt_long_messages() {
    let cases = vectors()["valid"]["encrypt_decrypt_long_msg"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let key = hex_array::<32>(field(case, "conversation_key"));
        let nonce = hex_array::<32>(field(case, "nonce"));
        let repeat = usize::try_from(case["repeat"].as_u64().unwrap()).unwrap();
        let plaintext = field(case, "pattern").repeat(repeat);
        assert_eq!(
            lower_hex(&Sha256::digest(plaintext.as_bytes())),
            field(case, "plaintext_sha256")
        );
        let payload = nip44::encrypt(&plaintext, &key, nonce).unwrap();
        assert_eq!(
            lower_hex(&Sha256::digest(payload.as_bytes())),
            field(case, "payload_sha256"),
            "{case}"
        );
        assert_eq!(nip44::decrypt(&payload, &key).unwrap(), plaintext);
    }
}

#[test]
fn invalid_plaintext_lengths() {
    let cases = vectors()["invalid"]["encrypt_msg_lengths"].clone();
    let lengths: Vec<usize> = cases
        .as_array()
        .unwrap()
        .iter()
        .map(|value| usize::try_from(value.as_u64().unwrap()).unwrap())
        .collect();
    assert_eq!(lengths, [0, 65_536, 100_000, 10_000_000]);
    let key = [0x11; 32];
    let nonce = [0x22; 32];
    for length in lengths {
        let plaintext = "a".repeat(length);
        let result = nip44::encrypt(&plaintext, &key, nonce);
        if length == 0 || length > MAX_CLIENT_PLAINTEXT_BYTES {
            assert!(result.is_err(), "length {length} must be refused");
        } else {
            // The vectors predate the pinned spec's extended length prefix
            // (nostr-protocol/nips 733a047). Under the pinned text these
            // lengths are valid, and the crate accepts them up to its
            // client bound.
            let payload = result.unwrap_or_else(|error| panic!("length {length}: {error}"));
            assert_eq!(nip44::decrypt(&payload, &key).unwrap(), plaintext);
        }
    }
}

#[test]
fn invalid_payloads_are_refused_with_the_named_cause() {
    let cases = vectors()["invalid"]["decrypt"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let key = hex_array::<32>(field(case, "conversation_key"));
        let note = field(case, "note");
        let error = nip44::decrypt(field(case, "payload"), &key)
            .expect_err(&format!("payload must be refused: {note}"));
        let expected_fragments: &[&str] = match note {
            "unknown encryption version" | "unknown encryption version 0" => &["unsupported"],
            "invalid base64" => &["base64"],
            "invalid MAC" => &["MAC"],
            // A forged length prefix fails either the plaintext bound or
            // the padded-length check, both after the MAC passed.
            "invalid padding" => &["padding", "length"],
            _ if note.starts_with("invalid payload length") => &["size", "short"],
            other => panic!("unexpected vector note {other:?}"),
        };
        assert!(
            expected_fragments
                .iter()
                .any(|fragment| error.contains(fragment)),
            "{note}: error {error:?} should mention one of {expected_fragments:?}"
        );
    }
}

#[test]
fn plaintext_length_boundaries() {
    let key = [0x33; 32];
    let nonce = [0x44; 32];
    assert!(nip44::encrypt("", &key, nonce).is_err());
    assert!(primitives::pad(b"").is_err());
    assert_eq!(
        primitives::padded_length(0),
        Err(String::from(
            "NIP-44 plaintext length is outside the client bound"
        ))
    );

    let one = "a";
    let payload = nip44::encrypt(one, &key, nonce).unwrap();
    assert_eq!(
        payload.len(),
        132,
        "the smallest payload is exactly 132 chars"
    );
    assert_eq!(nip44::decrypt(&payload, &key).unwrap(), one);

    let padded = primitives::pad(&[0x61; 65_535]).unwrap();
    assert_eq!(&padded[..2], &[0xff, 0xff], "65535 uses the 2-byte prefix");
    assert_eq!(padded.len(), 2 + 65_536);
    assert_eq!(primitives::unpad(&padded).unwrap().len(), 65_535);

    let padded = primitives::pad(&[0x61; 65_536]).unwrap();
    assert_eq!(
        &padded[..6],
        &[0, 0, 0, 1, 0, 0],
        "65536 switches to the 6-byte extended prefix"
    );
    assert_eq!(primitives::unpad(&padded).unwrap().len(), 65_536);

    let mut noncanonical = primitives::pad(&[0x61; 65_536]).unwrap();
    noncanonical[2..6].copy_from_slice(&65_535_u32.to_be_bytes());
    assert!(
        primitives::unpad(&noncanonical).is_err(),
        "an extended prefix below 65536 is noncanonical"
    );

    assert!(nip44::encrypt(&"a".repeat(MAX_CLIENT_PLAINTEXT_BYTES), &key, nonce).is_ok());
    assert!(nip44::encrypt(&"a".repeat(MAX_CLIENT_PLAINTEXT_BYTES + 1), &key, nonce).is_err());
    assert!(primitives::padded_length(MAX_CLIENT_PLAINTEXT_BYTES + 1).is_err());
}

#[test]
fn payload_size_boundaries_before_any_key_material_is_used() {
    let key = [0x55; 32];
    assert_eq!(
        nip44::decrypt("#anything", &key),
        Err("NIP-44 payload encoding is unsupported".to_owned())
    );
    let short = "A".repeat(128);
    assert_eq!(
        nip44::decrypt(&short, &key),
        Err("NIP-44 payload size is outside the client bound".to_owned())
    );
    let mut bytes = vec![2_u8];
    bytes.extend_from_slice(&[0; 98]);
    let ninety_nine = primitives::base64_encode(&bytes);
    assert_eq!(ninety_nine.len(), 132);
    assert_eq!(
        nip44::decrypt(&ninety_nine, &key),
        Err("NIP-44 payload MAC is invalid".to_owned()),
        "99 decoded bytes reach the MAC check"
    );
    let mut bytes = vec![2_u8];
    bytes.extend_from_slice(&[0; 97]);
    let ninety_eight = primitives::base64_encode(&bytes);
    assert_eq!(ninety_eight.len(), 132);
    assert_eq!(
        nip44::decrypt(&ninety_eight, &key),
        Err("NIP-44 decoded payload is too short".to_owned())
    );
}
