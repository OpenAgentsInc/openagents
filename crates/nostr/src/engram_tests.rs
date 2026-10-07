use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;
use crate::domain::hex::{decode_lower_hex, encode_lower_hex};

// TEST KEYS from NIP-AE's reference vectors. Never use them in production.
const SECKEY_A: &str = "0000000000000000000000000000000000000000000000000000000000000001";
const SECKEY_O: &str = "0000000000000000000000000000000000000000000000000000000000000002";
const PUBKEY_A: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const PUBKEY_O: &str = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
const K_C: &str = "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d";
const D_CORE: &str = "bdc233238ffe52e272b44cc233c8f33a2bc510b08be04495b225964283be4a90";
const D_EXAMPLE: &str = "72d4f9629106451505d7d341ea85bb3ebad4f654fcfd2aad100d5a35f8a85cba";
const D_NOTES: &str = "31651571a312780cfdc1f0b706b682ac9f3f51a053e8dca76fe57710bae5a4d4";

const BODY_1: &str = r#"{"slug":"mem/example","value":"hello, agent memory"}"#;
const BODY_2: &str = r#"{"slug":"mem/notes/2026-05-12","value":"meeting note: [[mem/example]]"}"#;
const BODY_3: &str = r#"{"slug":"mem/example","value":null}"#;
const BODY_4: &str =
    r#"{"slug":"core","profile":"test agent. see [[mem/example]] and [[mem/notes/2026-05-12]]."}"#;

struct Vector {
    body: &'static str,
    created_at: u64,
    nonce_last: u8,
    d: &'static str,
    content: &'static str,
    content_len: usize,
    content_sha256: &'static str,
    id: &'static str,
    sig: &'static str,
}

const VECTORS: [Vector; 4] = [
    Vector {
        body: BODY_1,
        created_at: 1_700_000_000,
        nonce_last: 1,
        d: D_EXAMPLE,
        content: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABedgcxyfmpph68LBjCWZsTI5lb0Cbg8dIPVYVe/WVj/l4Yd8HGgzC8awyBi9bn9ClRdtd2IPsmont0jN/cajVSQhahTOwuNNwoJtZIg35aSsUzeCq4tQfd8E+fLoKomdPxjs=",
        content_len: 176,
        content_sha256: "ff680a293019af12709972ae68b6ee79a47f354381a94ca4074d8e0fe3c8bb50",
        id: "f4a594177b7aeea4fe99a09efbf74ae85f0126244f322135682c405888a38689",
        sig: "0a4582f0bc5995b9a010afda5984f568055988ebbe4552b4e0ec6d11aeb2b303af940f3d84726a7edd1763badb284eb3aa8457664ceba85a90d6252ed4b494cb",
    },
    Vector {
        body: BODY_2,
        created_at: 1_700_000_001,
        nonce_last: 2,
        d: D_NOTES,
        content: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACG/JBPvdZxDwAxOG7bY3AW2q1slZqBjQC3NxfPVtfcR+TGjp2GKtjyXyqNwG08GK+00I1u1vUZ4cCjcun9A7ra92rleKKJ5w57pqgFspbv1vClUJY5487A/5phVDHkw6DhRCSMDpEMw5Tapj3Wm1ponAVr5PciPOrTxltEfTVdSKaPA==",
        content_len: 220,
        content_sha256: "ba7b026809363134c4f8de6cfbd82417b838e265281ff7e0005dc193bf1b32c8",
        id: "1a43298ea1fa9b73462a85b9f16f5f6bd2a7ab18b0b02424e5ec3f3b8a48e030",
        sig: "dc9da456db1c89f070edc5f994786f270fc00e8ff19f33d5b0f6cea49421cd727fcd79bb288f3e3dbd5af9ca1ba67f9bd11b02a47c1e6c37cfd32665c17e4a24",
    },
    Vector {
        body: BODY_3,
        created_at: 1_700_000_002,
        nonce_last: 3,
        d: D_EXAMPLE,
        content: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADuau8i0Wu4+ULnp2qTfd+O23jJAapMRrKGGwabNVOlT9hSF8FViBHIS6f86/7xK4qGOin4IH8Wr/3cvHDcQGQd3IXQJr8LHgJkaYpQPdBO1bgqiFu8K3L/CLb1PgG1X7RQ8E=",
        content_len: 176,
        content_sha256: "0c9f72125f6460e68cb4b7ee42298afc8969840f83a156d90aa98a5f461fea44",
        id: "c8604bef05295856a67a88ec895e07b5b47a2febc23c82934734096a7b123b63",
        sig: "c8d53859cf08b3a9a20a5b01c61d12fa2f082f462adb635420f05dc6f9bb662a174e729023854bf53e5e35fae8f6f4c9d604e8979a070e298cd77cfb7e6b6468",
    },
    Vector {
        body: BODY_4,
        created_at: 1_700_000_003,
        nonce_last: 4,
        d: D_CORE,
        content: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEEeZHAFjhc8DAcKaVSSB7IoKG3nr+dX3LXlU7UIdOKayhIVPXvl4WuFmBSVxLO6yEV5vnLvzbo7rU0uPRYyAJLPNnifVTCw2EQZH70zOwTc/mVvaATHKzqcFo5VHrbpKNTzeNnz1Vds2yg2DXmdxaoWQA4YfnlLwZDOpyu9JP1uB1Yw==",
        content_len: 220,
        content_sha256: "070f0f3e2e2bdc016b3ae06e8754e7814ffd4e98f0d5a70d75d1e8eab0d0e474",
        id: "980419c4d231266471242456c832d0c2eb1e6974468dc795f3ae327484129058",
        sig: "ce113fff1205eadb38928b224a90247be1a00b0c3f8ab583d4a5f7274ddba51ebb5eb9d627d44664a78d2e870e61835cf61446cc812ecea139e8b7d41b8e238f",
    },
];

fn secret(hex: &str) -> SecretKey {
    SecretKey::from_byte_array(decode_lower_hex::<32>(hex, "secret").unwrap()).unwrap()
}

fn public(hex: &str) -> XOnlyPublicKey {
    XOnlyPublicKey::from_byte_array(decode_lower_hex::<32>(hex, "pubkey").unwrap()).unwrap()
}

fn agent_pair() -> Pair {
    Pair::for_agent(&secret(SECKEY_A), &public(PUBKEY_O))
}

fn owner_pair() -> Pair {
    Pair::for_owner(&secret(SECKEY_O), &public(PUBKEY_A))
}

fn nonce(last: u8) -> [u8; 32] {
    let mut nonce = [0_u8; 32];
    nonce[31] = last;
    nonce
}

fn params(created_at: u64, nonce_last: u8) -> EventParams {
    EventParams {
        created_at,
        nonce: nonce(nonce_last),
        aux: [0; 32],
        alt: false,
    }
}

fn slug(text: &str) -> Slug {
    Slug::parse(text).unwrap()
}

fn write(body: &Body, created_at: u64, nonce_last: u8) -> Event {
    build_event(
        &secret(SECKEY_A),
        &public(PUBKEY_O),
        body,
        &params(created_at, nonce_last),
    )
    .unwrap()
}

/// Encrypts arbitrary plaintext and signs arbitrary tags with the agent key,
/// for events `build_event` would never produce.
fn raw(tags: Vec<Tag>, plaintext: &str, created_at: u64) -> Event {
    let content = nip44::encrypt(plaintext, agent_pair().conversation_key(), nonce(9)).unwrap();
    sign_event(&secret(SECKEY_A), created_at, tags, content, &[0; 32]).unwrap()
}

fn tags(d: &str) -> Vec<Tag> {
    vec![
        Tag::new(vec!["d".into(), d.into()]),
        Tag::new(vec!["p".into(), PUBKEY_O.into()]),
    ]
}

/// Re-signs `event` after a test edits it, so only the edit fails a rule.
fn resign(mut event: Event) -> Event {
    let keypair = Keypair::from_secret_key(&Secp256k1::signing_only(), &secret(SECKEY_A));
    let id = event.computed_id_bytes().unwrap();
    event.id = encode_lower_hex(&id);
    event.sig = Secp256k1::signing_only()
        .sign_schnorr_with_aux_rand(&id, &keypair, &[0; 32])
        .to_string();
    event
}

#[test]
fn bip340_vector_zero_signs_with_zero_aux() {
    let keypair = Keypair::from_secret_key(
        &Secp256k1::signing_only(),
        &secret("0000000000000000000000000000000000000000000000000000000000000003"),
    );
    let sig = Secp256k1::signing_only().sign_schnorr_with_aux_rand(&[0; 32], &keypair, &[0; 32]);
    assert!(sig.to_string().starts_with("e907831f80"));
}

#[test]
fn reference_keys_conversation_key_and_d_tags_reproduce() {
    assert_eq!(x_only(&secret(SECKEY_A)).to_string(), PUBKEY_A);
    assert_eq!(x_only(&secret(SECKEY_O)).to_string(), PUBKEY_O);
    assert_eq!(encode_lower_hex(agent_pair().conversation_key()), K_C);
    assert_eq!(agent_pair(), owner_pair());
    assert_eq!(agent_pair().d_tag(&Slug::core()), D_CORE);
    assert_eq!(agent_pair().d_tag(&slug("mem/example")), D_EXAMPLE);
    assert_eq!(agent_pair().d_tag(&slug("mem/notes/2026-05-12")), D_NOTES);
}

#[test]
fn reference_events_reproduce_byte_for_byte() {
    for vector in &VECTORS {
        let body = Body::parse(vector.body).unwrap();
        assert_eq!(body.to_json(), vector.body);
        let event = write(&body, vector.created_at, vector.nonce_last);
        assert_eq!(event.pubkey, PUBKEY_A);
        assert_eq!(event.kind, ENGRAM_KIND);
        assert_eq!(event.tags, tags(vector.d));
        assert_eq!(event.content, vector.content);
        assert_eq!(event.content.len(), vector.content_len);
        assert_eq!(
            encode_lower_hex(&Sha256::digest(event.content.as_bytes())),
            vector.content_sha256
        );
        assert_eq!(event.id, vector.id);
        assert_eq!(event.sig, vector.sig);
    }
}

#[test]
fn reference_events_validate_from_both_sides() {
    for vector in &VECTORS {
        let event = write(
            &Body::parse(vector.body).unwrap(),
            vector.created_at,
            vector.nonce_last,
        );
        let from_agent = validate_and_decrypt(&event, &agent_pair()).unwrap();
        let from_owner = validate_and_decrypt(&event, &owner_pair()).unwrap();
        assert_eq!(from_agent, from_owner);
        assert_eq!(from_agent.d, vector.d);
        assert_eq!(from_agent.body.to_json(), vector.body);
    }
}

#[test]
fn reference_events_list_and_link() {
    let engrams = VECTORS
        .iter()
        .map(|vector| {
            let event = write(
                &Body::parse(vector.body).unwrap(),
                vector.created_at,
                vector.nonce_last,
            );
            validate_and_decrypt(&event, &owner_pair()).unwrap()
        })
        .collect::<Vec<_>>();
    // Event 3 tombstones `mem/example`, and `core` is omitted.
    assert_eq!(
        list_heads(&engrams),
        vec![HeadEntry {
            slug: slug("mem/notes/2026-05-12"),
            event_id: VECTORS[1].id.to_owned(),
            created_at: 1_700_000_001,
        }]
    );
    assert_eq!(
        engrams[3].body.links(),
        vec![slug("mem/example"), slug("mem/notes/2026-05-12")]
    );
    assert_eq!(engrams[1].body.links(), vec![slug("mem/example")]);
    assert!(engrams[2].body.links().is_empty());
}

#[test]
fn slug_grammar() {
    for valid in [
        "core",
        "mem/a",
        "mem/0",
        "mem/a_b-c/d",
        "mem/notes/2026-05-12",
        &format!("mem/{}", "a".repeat(64)),
    ] {
        assert!(Slug::parse(valid).is_ok(), "{valid}");
    }
    let long = format!("mem/{}", vec!["a".repeat(63); 4].join("/"));
    assert_eq!(long.len(), 259);
    let at_limit = &long[..255];
    assert!(Slug::parse(at_limit).is_ok());
    for invalid in [
        "",
        "Core",
        "core/x",
        "mem",
        "mem/",
        "mem//a",
        "mem/a/",
        "mem/_a",
        "mem/-a",
        "mem/A",
        "mem/a b",
        "mem/a.b",
        "mem/é",
        "memo/a",
        "/mem/a",
        &format!("mem/{}", "a".repeat(65)),
        &long,
    ] {
        assert!(Slug::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn memory_bodies_refuse_the_core_slug_and_reserved_extras() {
    assert!(Body::memory(Slug::core(), "x").is_err());
    assert!(Body::tombstone(Slug::core()).is_err());
    assert_eq!(
        Body::core("p").with_extra("profile", json!(1)),
        Err(EngramError::ReservedField("profile".into()))
    );
    let body = Body::memory(slug("mem/entry/1"), "text")
        .unwrap()
        .with_extra("schema", json!("openagents.memory-entry.v1"))
        .unwrap()
        .with_extra("v", json!(1))
        .unwrap();
    assert_eq!(
        body.to_json(),
        r#"{"slug":"mem/entry/1","value":"text","schema":"openagents.memory-entry.v1","v":1}"#
    );
    assert_eq!(Body::parse(&body.to_json()).unwrap(), body);
}

#[test]
fn bodies_keep_non_ascii_text_unescaped() {
    let body = Body::memory(slug("mem/a"), "café \u{1f600} \"q\"\n").unwrap();
    assert_eq!(
        body.to_json(),
        "{\"slug\":\"mem/a\",\"value\":\"café \u{1f600} \\\"q\\\"\\n\"}"
    );
    assert_eq!(Body::parse(&body.to_json()).unwrap(), body);
}

#[test]
fn strict_parse_rejects_duplicates_anywhere_and_ignores_unknown_fields() {
    assert_eq!(
        Body::parse(r#"{"slug":"mem/a","value":"x","slug":"mem/a"}"#),
        Err(EngramError::DuplicateMember)
    );
    assert_eq!(
        Body::parse(r#"{"slug":"mem/a","value":"x","meta":{"k":1,"k":2}}"#),
        Err(EngramError::DuplicateMember)
    );
    assert_eq!(
        Body::parse(r#"{"slug":"mem/a","value":"x","list":[{"k":1},{"k":2,"k":3}]}"#),
        Err(EngramError::DuplicateMember)
    );
    let body =
        Body::parse(r#"{"slug":"mem/a","value":"x","meta":{"k":[1,2.5,null,true]}}"#).unwrap();
    assert_eq!(body.extra()["meta"], json!({"k": [1, 2.5, null, true]}));
    assert!(matches!(
        Body::parse("[1]"),
        Err(EngramError::NotJsonObject(_))
    ));
    assert!(matches!(
        Body::parse(r#"{"slug":"mem/a","value":"x"} trailing"#),
        Err(EngramError::NotJsonObject(_))
    ));
}

#[test]
fn shapes_match_the_slug_type() {
    for (text, expected) in [
        (r#"{"value":"x"}"#, "slug"),
        (r#"{"slug":1,"value":"x"}"#, "slug"),
        (r#"{"slug":"mem/A","value":"x"}"#, "slug"),
        (r#"{"slug":"mem/a"}"#, "shape"),
        (r#"{"slug":"mem/a","value":1}"#, "shape"),
        (r#"{"slug":"mem/a","profile":"x"}"#, "shape"),
        (r#"{"slug":"core"}"#, "shape"),
        (r#"{"slug":"core","profile":null}"#, "shape"),
        (r#"{"slug":"core","value":"x"}"#, "shape"),
    ] {
        let error = Body::parse(text).unwrap_err();
        let kind = match error {
            EngramError::BodySlug(_) => "slug",
            EngramError::Shape(_) => "shape",
            other => panic!("{text}: {other}"),
        };
        assert_eq!(kind, expected, "{text}");
    }
    // A core body may carry `value` as an unknown field.
    assert!(Body::parse(r#"{"slug":"core","profile":"p","value":"x"}"#).is_ok());
}

#[test]
fn rule_one_checks_kind_author_and_tags() {
    let event = write(&Body::parse(BODY_1).unwrap(), 10, 1);
    let pair = agent_pair();

    let mut wrong_kind = event.clone();
    wrong_kind.kind = 30_078;
    assert_eq!(
        validate_and_decrypt(&resign(wrong_kind), &pair),
        Err(EngramError::WrongKind(30_078))
    );

    let other = SecretKey::from_byte_array([7; 32]).unwrap();
    let foreign = build_event(
        &other,
        &public(PUBKEY_O),
        &Body::parse(BODY_1).unwrap(),
        &params(10, 1),
    )
    .unwrap();
    assert_eq!(
        validate_and_decrypt(&foreign, &pair),
        Err(EngramError::WrongAuthor)
    );

    let mut two_d = event.clone();
    two_d
        .tags
        .push(Tag::new(vec!["d".into(), D_EXAMPLE.into()]));
    assert_eq!(
        validate_and_decrypt(&resign(two_d), &pair),
        Err(EngramError::DTagCount)
    );
    let mut no_d = event.clone();
    no_d.tags.remove(0);
    assert_eq!(
        validate_and_decrypt(&resign(no_d), &pair),
        Err(EngramError::DTagCount)
    );
    let mut bare_d = event.clone();
    bare_d.tags[0] = Tag::new(vec!["d".into()]);
    assert_eq!(
        validate_and_decrypt(&resign(bare_d), &pair),
        Err(EngramError::DTagCount)
    );

    let mut two_p = event.clone();
    two_p.tags.push(Tag::new(vec!["p".into(), PUBKEY_O.into()]));
    assert_eq!(
        validate_and_decrypt(&resign(two_p), &pair),
        Err(EngramError::PTagCount)
    );
    let mut wrong_p = event.clone();
    wrong_p.tags[1] = Tag::new(vec!["p".into(), PUBKEY_A.into()]);
    assert_eq!(
        validate_and_decrypt(&resign(wrong_p), &pair),
        Err(EngramError::WrongOwner)
    );

    // Extra tags beyond `d`, `p`, and `alt` have no effect.
    let mut extra = event;
    extra.tags.push(Tag::new(vec!["t".into(), "x".into()]));
    assert!(validate_and_decrypt(&resign(extra), &pair).is_ok());
}

#[test]
fn rule_two_checks_the_signature_before_decrypting() {
    let event = write(&Body::parse(BODY_1).unwrap(), 10, 1);
    let mut bad_sig = event.clone();
    bad_sig.sig = format!("{}00", &bad_sig.sig[..126]);
    assert_eq!(
        validate_and_decrypt(&bad_sig, &agent_pair()),
        Err(EngramError::Signature)
    );
    // Content that would not decrypt still reports the signature first.
    let mut tampered = event;
    tampered.content = "garbage".into();
    assert_eq!(
        validate_and_decrypt(&tampered, &agent_pair()),
        Err(EngramError::Signature)
    );
}

#[test]
fn rule_three_checks_decryption_and_strict_json() {
    let mut undecryptable = write(&Body::parse(BODY_1).unwrap(), 10, 1);
    let other_key = nip44::conversation_key(
        &SecretKey::from_byte_array([7; 32]).unwrap(),
        &public(PUBKEY_O),
    );
    undecryptable.content = nip44::encrypt(BODY_1, &other_key, nonce(1)).unwrap();
    assert!(matches!(
        validate_and_decrypt(&resign(undecryptable), &agent_pair()),
        Err(EngramError::Decrypt(_))
    ));

    let not_json = raw(tags(D_EXAMPLE), "not json", 10);
    assert!(matches!(
        validate_and_decrypt(&not_json, &agent_pair()),
        Err(EngramError::NotJsonObject(_))
    ));
    let duplicate = raw(
        tags(D_EXAMPLE),
        r#"{"slug":"mem/example","value":"a","value":"b"}"#,
        10,
    );
    assert_eq!(
        validate_and_decrypt(&duplicate, &owner_pair()),
        Err(EngramError::DuplicateMember)
    );
}

#[test]
fn rule_four_rederives_the_d_tag() {
    let mismatch = raw(tags(D_NOTES), BODY_1, 10);
    assert_eq!(
        validate_and_decrypt(&mismatch, &agent_pair()),
        Err(EngramError::DTagMismatch)
    );
    let bad_slug = raw(tags(D_EXAMPLE), r#"{"slug":"mem/Example","value":"x"}"#, 10);
    assert!(matches!(
        validate_and_decrypt(&bad_slug, &agent_pair()),
        Err(EngramError::BodySlug(_))
    ));
}

#[test]
fn rule_five_checks_the_shape() {
    let wrong = raw(tags(D_CORE), r#"{"slug":"core","value":"x"}"#, 10);
    assert!(matches!(
        validate_and_decrypt(&wrong, &agent_pair()),
        Err(EngramError::Shape(_))
    ));
}

#[test]
fn build_adds_alt_and_refuses_oversized_bodies() {
    let mut with_alt = params(10, 1);
    with_alt.alt = true;
    let event = build_event(
        &secret(SECKEY_A),
        &public(PUBKEY_O),
        &Body::parse(BODY_1).unwrap(),
        &with_alt,
    )
    .unwrap();
    assert_eq!(event.tags[2], Tag::new(vec!["alt".into(), ALT_TEXT.into()]));
    assert!(validate_and_decrypt(&event, &owner_pair()).is_ok());

    let big = Body::memory(slug("mem/big"), "x".repeat(MAX_BODY_BYTES)).unwrap();
    assert!(matches!(
        build_event(&secret(SECKEY_A), &public(PUBKEY_O), &big, &params(10, 1)),
        Err(EngramError::BodyTooLarge(_))
    ));
    let overhead = r#"{"slug":"mem/big","value":""}"#.len();
    let fits = Body::memory(slug("mem/big"), "x".repeat(MAX_BODY_BYTES - overhead)).unwrap();
    let event = build_event(&secret(SECKEY_A), &public(PUBKEY_O), &fits, &params(10, 1)).unwrap();
    assert_eq!(
        validate_and_decrypt(&event, &owner_pair()).unwrap().body,
        fits
    );
}

#[test]
fn heads_take_the_latest_and_break_ties_by_lowest_id() {
    let pair = agent_pair();
    let first = validate_and_decrypt(&write(&Body::parse(BODY_1).unwrap(), 10, 1), &pair).unwrap();
    let later = validate_and_decrypt(
        &write(&Body::memory(slug("mem/example"), "v2").unwrap(), 11, 2),
        &pair,
    )
    .unwrap();
    assert_eq!(select_head([&first, &later]).unwrap(), &later);
    assert_eq!(select_head([&later, &first]).unwrap(), &later);
    assert_eq!(select_head(std::iter::empty()), None);

    let tie_a = validate_and_decrypt(
        &write(&Body::memory(slug("mem/example"), "a").unwrap(), 11, 3),
        &pair,
    )
    .unwrap();
    let low = if tie_a.id < later.id { &tie_a } else { &later };
    assert_eq!(select_head([&tie_a, &later]).unwrap(), low);
    assert_eq!(select_head([&later, &tie_a]).unwrap(), low);

    let listed = list_heads(&[later.clone(), first.clone(), tie_a.clone()]);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].event_id, low.id);
}

#[test]
fn tombstones_hide_a_slug_until_a_later_write() {
    let pair = owner_pair();
    let written =
        validate_and_decrypt(&write(&Body::parse(BODY_1).unwrap(), 10, 1), &pair).unwrap();
    let tombstone =
        validate_and_decrypt(&write(&Body::parse(BODY_3).unwrap(), 11, 2), &pair).unwrap();
    assert!(tombstone.is_tombstone());
    assert!(list_heads(&[written.clone(), tombstone.clone()]).is_empty());

    let revived = validate_and_decrypt(
        &write(&Body::memory(slug("mem/example"), "back").unwrap(), 12, 3),
        &pair,
    )
    .unwrap();
    let listed = list_heads(&[tombstone, revived.clone(), written]);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].event_id, revived.id);
    assert_eq!(listed[0].slug, slug("mem/example"));
}

#[test]
fn monotonic_writes_and_clock_poison() {
    assert_eq!(monotonic_created_at(100, None), Ok(100));
    assert_eq!(monotonic_created_at(100, Some(50)), Ok(100));
    assert_eq!(monotonic_created_at(100, Some(100)), Ok(101));
    assert_eq!(monotonic_created_at(100, Some(120)), Ok(121));
    assert_eq!(
        monotonic_created_at(100, Some(99 + CLOCK_POISON_SECS)),
        Ok(100 + CLOCK_POISON_SECS)
    );
    assert_eq!(
        monotonic_created_at(100, Some(100 + CLOCK_POISON_SECS)),
        Err(EngramError::ClockPoisoned {
            head: 100 + CLOCK_POISON_SECS,
            now: 100
        })
    );
    assert_eq!(monotonic_created_at_within(100, Some(104), 5), Ok(105));
    assert!(monotonic_created_at_within(100, Some(105), 5).is_err());
    assert!(monotonic_created_at(u64::MAX, Some(u64::MAX)).is_err());
}

#[test]
fn wiki_links_are_literal_bracketed_slugs() {
    assert_eq!(
        wiki_links("see [[mem/a]], [[core]], [[mem/a]] and [[mem/b/c]]"),
        vec![slug("mem/a"), Slug::core(), slug("mem/b/c")]
    );
    assert!(wiki_links("mem/a [mem/b] [[Mem/c]] [[mem/d] ]] [[mem/e").is_empty());
    assert_eq!(wiki_links("[[[mem/a]]]"), vec![slug("mem/a")]);
    assert_eq!(wiki_links("[[ [[mem/a]]"), vec![slug("mem/a")]);
    assert_eq!(wiki_links("é[[mem/é]] [[mem/x]]"), vec![slug("mem/x")]);
}

#[test]
fn pair_debug_redacts_the_conversation_key() {
    let text = format!("{:?}", agent_pair());
    assert!(text.contains("<redacted>"));
    assert!(!text.contains(K_C));
}
