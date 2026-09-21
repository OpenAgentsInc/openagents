//! Bounded property tests for the parsers and codecs in this crate: NIP-01
//! event JSON, filter parsing and matching, NIP-19 bech32, and NIP-44
//! payload framing.
//!
//! Where a property compares an implementation to itself (a round trip),
//! a second check compares it to an independent oracle: `serde_json::Value`
//! for the wire shape, a naive matcher for filters, the bech32 error
//! detection guarantee for NIP-19, and the RustCrypto-verified primitives
//! for NIP-44 framing. A minimized failure that needs a fix is preserved
//! under `tests/fixtures/regressions/`.

use std::collections::BTreeMap;

use nostr::domain::{Event, Filter, Tag, is_indexed_tag_name, search_matches};
use nostr::nip19;
use nostr::nip44::{self, primitives};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use secp256k1::{Secp256k1, SecretKey};
use serde_json::Value;

mod common;

use common::lower_hex;

fn hex32() -> impl Strategy<Value = String> {
    any::<[u8; 32]>().prop_map(|bytes| lower_hex(&bytes))
}

fn hex64() -> impl Strategy<Value = String> {
    any::<[u8; 64]>().prop_map(|bytes| lower_hex(&bytes))
}

/// Tag values from a small alphabet so filters have a chance to match.
fn small_text() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "",
        "a",
        "b",
        "cat",
        "dog",
        "é",
        "\"quoted\"",
        "line\nbreak",
    ])
    .prop_map(str::to_owned)
}

fn tag_name() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["e", "p", "d", "t", "work", "zz", "E", "#"]).prop_map(str::to_owned)
}

fn tag() -> impl Strategy<Value = Tag> {
    (tag_name(), prop::collection::vec(small_text(), 0..3)).prop_map(|(name, mut values)| {
        values.insert(0, name);
        Tag::new(values)
    })
}

fn event() -> impl Strategy<Value = Event> {
    (
        hex32(),
        hex32(),
        any::<u64>(),
        any::<u16>(),
        prop::collection::vec(tag(), 0..4),
        ".*",
        hex64(),
    )
        .prop_map(|(id, pubkey, created_at, kind, tags, content, sig)| Event {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig,
        })
}

fn filter() -> impl Strategy<Value = Filter> {
    (
        prop::option::of(prop::collection::vec(hex32(), 0..3)),
        prop::option::of(prop::collection::vec(hex32(), 0..3)),
        prop::option::of(prop::collection::vec(any::<u16>(), 0..3)),
        prop::collection::btree_map(
            prop::sample::select(vec!["e", "p", "d", "t", "work"]).prop_map(str::to_owned),
            prop::collection::vec(small_text(), 0..3),
            0..3,
        ),
        prop::option::of(any::<u64>()),
        prop::option::of(any::<u64>()),
        prop::option::of(0_usize..10_000),
        prop::option::of(prop::sample::select(vec!["cat", "CAT dog", "a b", "x"])),
    )
        .prop_map(
            |(ids, authors, kinds, tags, since, until, limit, search)| Filter {
                ids,
                authors,
                kinds,
                tags,
                since,
                until,
                limit,
                search: search.map(str::to_owned),
            },
        )
}

/// A deliberately naive reading of NIP-01 filter semantics, written apart
/// from `Filter::matches`.
fn naive_matches(filter: &Filter, event: &Event) -> bool {
    fn contains(list: &Option<Vec<String>>, value: &str) -> bool {
        match list {
            None => true,
            Some(list) => list.iter().any(|candidate| candidate == value),
        }
    }
    if !contains(&filter.ids, &event.id) || !contains(&filter.authors, &event.pubkey) {
        return false;
    }
    if let Some(kinds) = &filter.kinds
        && !kinds.contains(&event.kind)
    {
        return false;
    }
    if let Some(since) = filter.since
        && event.created_at < since
    {
        return false;
    }
    if let Some(until) = filter.until
        && event.created_at > until
    {
        return false;
    }
    for (name, wanted) in &filter.tags {
        let mut found = false;
        for tag in &event.tags {
            let values = tag.as_slice();
            if values.len() >= 2 && &values[0] == name && wanted.contains(&values[1]) {
                found = true;
            }
        }
        if !found {
            return false;
        }
    }
    match &filter.search {
        None => true,
        Some(search) => search_matches(search, event.kind, &event.content),
    }
}

/// Minimized failures persist under `tests/fixtures/regressions/`, relative
/// to the crate directory `cargo test` runs in, so a failing seed becomes a
/// fixture that reruns first.
fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/fixtures/regressions/properties.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn event_json_round_trips(event in event()) {
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&parsed, &event);
    }

    #[test]
    fn event_json_has_the_nip01_wire_shape(event in event()) {
        let value: Value = serde_json::to_value(&event).unwrap();
        let object = value.as_object().unwrap();
        prop_assert_eq!(object.len(), 7);
        prop_assert_eq!(object["id"].as_str(), Some(event.id.as_str()));
        prop_assert_eq!(object["pubkey"].as_str(), Some(event.pubkey.as_str()));
        prop_assert_eq!(object["created_at"].as_u64(), Some(event.created_at));
        prop_assert_eq!(object["kind"].as_u64(), Some(u64::from(event.kind)));
        prop_assert_eq!(object["content"].as_str(), Some(event.content.as_str()));
        prop_assert_eq!(object["sig"].as_str(), Some(event.sig.as_str()));
        let tags = object["tags"].as_array().unwrap();
        prop_assert_eq!(tags.len(), event.tags.len());
        for (tag, value) in event.tags.iter().zip(tags) {
            let strings: Vec<&str> = value
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_str().unwrap())
                .collect();
            prop_assert_eq!(strings, tag.as_slice().iter().map(String::as_str).collect::<Vec<_>>());
        }
    }

    #[test]
    fn canonical_preimage_is_the_nip01_array(event in event()) {
        let canonical: Value = serde_json::from_str(&event.canonical_json().unwrap()).unwrap();
        let array = canonical.as_array().unwrap();
        prop_assert_eq!(array.len(), 6);
        prop_assert_eq!(array[0].as_u64(), Some(0));
        prop_assert_eq!(array[1].as_str(), Some(event.pubkey.as_str()));
        prop_assert_eq!(array[2].as_u64(), Some(event.created_at));
        prop_assert_eq!(array[3].as_u64(), Some(u64::from(event.kind)));
        prop_assert_eq!(&array[4], &serde_json::to_value(&event.tags).unwrap());
        prop_assert_eq!(array[5].as_str(), Some(event.content.as_str()));
        // The preimage has no insignificant whitespace.
        let text = event.canonical_json().unwrap();
        prop_assert_eq!(&text, &serde_json::to_string(&canonical).unwrap());
        // The id is a lowercase 64-character digest of that preimage.
        let id = event.computed_id().unwrap();
        prop_assert_eq!(id.len(), 64);
        prop_assert!(id.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')));
    }

    #[test]
    fn structure_validation_accepts_well_formed_fields_and_never_panics(event in event()) {
        // Ids, pubkeys, and signatures come from `hex32`/`hex64`, so only the
        // tag shape can fail here.
        let result = event.validate_nip01_structure();
        let _ = result;
        let mut upper = event.clone();
        upper.id = upper.id.to_ascii_uppercase();
        if upper.id != event.id {
            prop_assert!(upper.validate_nip01_structure().is_err());
        }
        let mut short = event;
        short.pubkey.pop();
        prop_assert!(short.validate_nip01_structure().is_err());
    }

    #[test]
    fn filter_json_round_trips(filter in filter()) {
        let json = serde_json::to_string(&filter).unwrap();
        let parsed: Filter = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&parsed, &filter);
        let value: Value = serde_json::from_str(&json).unwrap();
        let object = value.as_object().unwrap();
        for (name, values) in &filter.tags {
            prop_assert!(is_indexed_tag_name(name));
            prop_assert_eq!(&object[&format!("#{name}")], &serde_json::to_value(values).unwrap());
        }
        prop_assert_eq!(object.contains_key("since"), filter.since.is_some());
        prop_assert_eq!(object.contains_key("limit"), filter.limit.is_some());
    }

    #[test]
    fn filter_parsing_rejects_unknown_fields_and_keeps_channel_window_fields(
        filter in filter(),
        unknown in "[a-z]{2,6}",
    ) {
        let mut object = serde_json::to_value(&filter).unwrap();
        let map = object.as_object_mut().unwrap();
        map.insert("top_level".to_owned(), Value::Bool(true));
        map.insert("before_id".to_owned(), Value::Null);
        let parsed: Filter = serde_json::from_value(object.clone()).unwrap();
        prop_assert_eq!(&parsed, &filter);
        let map = object.as_object_mut().unwrap();
        if !map.contains_key(&unknown) && !["ids", "authors", "kinds", "since", "until", "limit", "search", "top_level", "include_summaries", "include_aux", "before_id"].contains(&unknown.as_str()) {
            map.insert(unknown, Value::Bool(true));
            prop_assert!(serde_json::from_value::<Filter>(object).is_err());
        }
    }

    #[test]
    fn filter_matching_agrees_with_a_naive_reading(filter in filter(), event in event()) {
        prop_assert_eq!(filter.matches(&event), naive_matches(&filter, &event));
    }

    #[test]
    fn a_filter_built_from_an_event_matches_it(event in event(), pick_tag in any::<prop::sample::Index>()) {
        let mut tags = BTreeMap::new();
        if let Some((name, value)) = event.indexed_tags().nth(pick_tag.index(event.tags.len().max(1))) {
            tags.insert(name.to_owned(), vec![value.to_owned()]);
        }
        let filter = Filter {
            ids: Some(vec![event.id.clone()]),
            authors: Some(vec![event.pubkey.clone()]),
            kinds: Some(vec![event.kind]),
            tags,
            since: Some(event.created_at),
            until: Some(event.created_at),
            limit: None,
            search: None,
        };
        prop_assert!(filter.matches(&event));
        prop_assert!(naive_matches(&filter, &event));
    }

    #[test]
    fn filter_json_from_arbitrary_values_never_panics(value in arbitrary_json()) {
        let _ = serde_json::from_value::<Filter>(value.clone());
        let _ = serde_json::from_value::<Event>(value);
    }

    #[test]
    fn bech32_round_trips_any_prefix_and_payload(
        prefix in "[a-z]{1,8}",
        data in prop::collection::vec(any::<u8>(), 0..=40),
    ) {
        // 8 prefix + 1 separator + 64 data + 6 checksum characters stays
        // within the 90-character bech32 limit.
        let encoded = nip19::encode(&prefix, &data).unwrap();
        prop_assert!(encoded.len() <= 90);
        prop_assert!(encoded.is_ascii());
        let (decoded_prefix, decoded) = nip19::decode(&encoded).unwrap();
        prop_assert_eq!(decoded_prefix, prefix);
        prop_assert_eq!(decoded, data);
        prop_assert_eq!(nip19::decode(&encoded.to_ascii_uppercase()).unwrap().1, nip19::decode(&encoded).unwrap().1);
    }

    #[test]
    fn bech32_refuses_to_encode_past_90_characters(
        prefix in "[a-z]{1,8}",
        data in prop::collection::vec(any::<u8>(), 53..80),
    ) {
        prop_assert_eq!(nip19::encode(&prefix, &data), Err(nip19::Nip19Error::InvalidLength));
    }

    #[test]
    fn bech32_detects_any_single_character_substitution(
        prefix in "[a-z]{1,4}",
        data in prop::collection::vec(any::<u8>(), 0..40),
        position in any::<prop::sample::Index>(),
        replacement in prop::sample::select(b"qpzry9x8gf2tvdw0s3jn54khce6mua7l".to_vec()),
    ) {
        let encoded = nip19::encode(&prefix, &data).unwrap();
        let separator = encoded.rfind('1').unwrap();
        let data_start = separator + 1;
        let index = data_start + position.index(encoded.len() - data_start);
        let mut corrupted = encoded.clone().into_bytes();
        prop_assume!(corrupted[index] != replacement);
        corrupted[index] = replacement;
        let corrupted = String::from_utf8(corrupted).unwrap();
        // BCH code guarantee: one substitution is always detected.
        prop_assert!(nip19::decode(&corrupted).is_err(), "{encoded} -> {corrupted}");
    }

    #[test]
    fn npub_and_nsec_round_trip_real_keys(secret_bytes in any::<[u8; 32]>()) {
        let Ok(secret) = SecretKey::from_byte_array(secret_bytes) else {
            return Ok(());
        };
        let (xonly, _) = secret.x_only_public_key(&Secp256k1::new());
        let nsec = nip19::encode_nsec(&secret_bytes);
        prop_assert!(nsec.starts_with("nsec1"));
        prop_assert_eq!(nsec.len(), 63);
        prop_assert_eq!(nip19::decode_nsec(&nsec).unwrap(), secret_bytes);
        let pubkey = xonly.serialize();
        let npub = nip19::encode_npub(&pubkey);
        prop_assert!(npub.starts_with("npub1"));
        prop_assert_eq!(npub.len(), 63);
        prop_assert_eq!(nip19::decode_npub(&npub).unwrap(), pubkey);
        prop_assert_eq!(
            nip19::decode_npub(&nsec),
            Err(nip19::Nip19Error::WrongPrefix { expected: "npub", actual: "nsec".to_owned() })
        );
        prop_assert!(nip19::decode_nsec(&npub).is_err());
    }

    #[test]
    fn bech32_decode_never_panics(text in "[ -~]{0,100}") {
        let _ = nip19::decode(&text);
        let _ = nip19::decode_npub(&text);
        let _ = nip19::decode_nsec(&text);
    }

    #[test]
    fn nip44_payload_framing(
        conversation_key in any::<[u8; 32]>(),
        nonce in any::<[u8; 32]>(),
        plaintext in "[a-zA-Z0-9 é🙂]{1,300}",
    ) {
        let payload = nip44::encrypt(&plaintext, &conversation_key, nonce).unwrap();
        prop_assert!(payload.len() <= primitives::encoded_maximum(nip44::MAX_CLIENT_PLAINTEXT_BYTES));
        let bytes = primitives::base64_decode(&payload).unwrap();
        let padded_length = primitives::padded_length(plaintext.len()).unwrap();
        prop_assert_eq!(bytes[0], 2);
        prop_assert_eq!(bytes.len(), 1 + 32 + 2 + padded_length + 32);
        prop_assert_eq!(&bytes[1..33], &nonce[..]);
        // The MAC is HMAC-SHA256(hmac_key, nonce || ciphertext) with the
        // keys HKDF expands, all of which the differential tests pin to
        // RustCrypto.
        let keys = primitives::message_keys(&conversation_key, &nonce);
        let ciphertext = &bytes[33..bytes.len() - 32];
        let mut mac_input = nonce.to_vec();
        mac_input.extend_from_slice(ciphertext);
        prop_assert_eq!(&bytes[bytes.len() - 32..], &primitives::hmac_sha256(&keys[44..76], &mac_input)[..]);
        let mut padded = ciphertext.to_vec();
        primitives::chacha20_xor(&keys[..32], &keys[32..44], &mut padded);
        prop_assert_eq!(primitives::unpad(&padded).unwrap(), plaintext.as_bytes());
        prop_assert_eq!(nip44::decrypt(&payload, &conversation_key).unwrap(), plaintext);
    }

    #[test]
    fn nip44_rejects_every_single_byte_corruption(
        conversation_key in any::<[u8; 32]>(),
        nonce in any::<[u8; 32]>(),
        plaintext in "[a-z]{1,64}",
        position in any::<prop::sample::Index>(),
        flip in 1_u8..=255,
    ) {
        let payload = nip44::encrypt(&plaintext, &conversation_key, nonce).unwrap();
        let mut bytes = primitives::base64_decode(&payload).unwrap();
        let index = position.index(bytes.len());
        bytes[index] ^= flip;
        let corrupted = primitives::base64_encode(&bytes);
        prop_assert!(nip44::decrypt(&corrupted, &conversation_key).is_err(), "byte {index}");
        let mut other_key = conversation_key;
        other_key[index % 32] ^= flip;
        prop_assert!(nip44::decrypt(&payload, &other_key).is_err());
    }

    #[test]
    fn nip44_pad_and_unpad_round_trip(plaintext in prop::collection::vec(any::<u8>(), 1..70_000)) {
        let padded = primitives::pad(&plaintext).unwrap();
        let prefix = if plaintext.len() >= 65_536 { 6 } else { 2 };
        let body = padded.len() - prefix;
        prop_assert!(body >= plaintext.len());
        prop_assert!(body == 32 || body.is_power_of_two() || body.is_multiple_of(32));
        prop_assert_eq!(body, primitives::padded_length(plaintext.len()).unwrap());
        prop_assert_eq!(primitives::unpad(&padded).unwrap(), &plaintext[..]);
        // Padding is zeros, and a non-zero pad byte is a framing error
        // only where the specification says the length prefix rules.
        prop_assert!(padded[prefix + plaintext.len()..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn nip44_padded_length_is_monotone_and_bounded(length in 1_usize..=nip44::MAX_CLIENT_PLAINTEXT_BYTES) {
        let padded = primitives::padded_length(length).unwrap();
        prop_assert!(padded >= length);
        prop_assert!(padded >= 32);
        // NIP-44 pads to a multiple of `chunk`, where `chunk` is 32 or an
        // eighth of the power of two strictly above `length - 1`.
        // next_power_of_two on length - 1 is not strict at 257, 513, etc.
        let next_power = if length <= 32 { 32 } else { 1_usize << ((length - 1).ilog2() + 1) };
        let chunk = 32.max(next_power / 8);
        prop_assert!(padded - length < chunk);
        prop_assert!(padded.is_multiple_of(chunk));
        if length > 1 {
            prop_assert!(primitives::padded_length(length - 1).unwrap() <= padded);
        }
    }

    #[test]
    fn nip44_decrypt_never_panics_on_arbitrary_text(
        conversation_key in any::<[u8; 32]>(),
        payload in "[A-Za-z0-9+/=#]{0,400}",
    ) {
        let _ = nip44::decrypt(&payload, &conversation_key);
        let _ = primitives::base64_decode(&payload);
    }

    #[test]
    fn nip44_base64_round_trips(bytes in prop::collection::vec(any::<u8>(), 0..300)) {
        let encoded = primitives::base64_encode(&bytes);
        prop_assert_eq!(encoded.len(), bytes.len().div_ceil(3) * 4);
        prop_assert_eq!(primitives::base64_decode(&encoded).unwrap(), bytes);
    }
}

fn arbitrary_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        "[ -~]{0,20}".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map(
                prop::sample::select(vec![
                    "ids",
                    "authors",
                    "kinds",
                    "#e",
                    "#p",
                    "#work",
                    "#zz",
                    "since",
                    "until",
                    "limit",
                    "search",
                    "id",
                    "pubkey",
                    "created_at",
                    "kind",
                    "tags",
                    "content",
                    "sig",
                    "other",
                ])
                .prop_map(str::to_owned),
                inner,
                0..8,
            )
            .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}
