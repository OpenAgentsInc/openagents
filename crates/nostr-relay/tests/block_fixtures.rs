//! Fixture-backed coverage for every server-relevant pinned Block NIP.

use nostr_relay::domain::{
    AGENT_ENGRAM_KIND, AGENT_PERSONA_KIND, DM_HIDE_KIND, EVENT_REMINDER_KIND, Event, EventClass,
    Filter, PROJECT_KIND, PUSH_LEASE_KIND, READ_STATE_KIND, RELAY_ONLY_BLOCK_KINDS,
    TEAM_CATALOG_KIND, Tag, dm_visibility_channel, parse_identity_archive_request,
    validate_block_ingest, workspace_icon,
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::Value;

#[test]
fn nipae_engram_server_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipae/server.json")).unwrap();
    let d = fixture["valid"]["d"].as_str().unwrap();
    let event = signed_event(
        4,
        200,
        AGENT_ENGRAM_KIND,
        vec![
            Tag::new(vec!["d".into(), d.into()]),
            Tag::new(vec!["p".into(), pubkey(3)]),
        ],
        &fake_nip44_v2(),
    );
    assert!(validate_block_ingest(&event, 200).is_ok());
    let mut duplicate = event.clone();
    duplicate.tags.push(duplicate.tags[1].clone());
    resign(&mut duplicate, 4);
    assert!(validate_block_ingest(&duplicate, 200).is_err());
    let plaintext = signed_event(4, 200, AGENT_ENGRAM_KIND, event.tags, "plaintext");
    assert!(validate_block_ingest(&plaintext, 200).is_err());
}

#[test]
fn nipap_persona_and_team_server_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipap/server.json")).unwrap();
    for case in fixture["persona_slugs"].as_array().unwrap() {
        let event = signed_event(
            3,
            200,
            AGENT_PERSONA_KIND,
            vec![Tag::new(vec!["d".into(), case[0].as_str().unwrap().into()])],
            "{}",
        );
        assert_eq!(
            validate_block_ingest(&event, 200).is_ok(),
            case[1].as_bool().unwrap()
        );
    }
    for case in fixture["team_ids"].as_array().unwrap() {
        let event = signed_event(
            3,
            200,
            TEAM_CATALOG_KIND,
            vec![Tag::new(vec!["d".into(), case[0].as_str().unwrap().into()])],
            "{}",
        );
        assert_eq!(
            validate_block_ingest(&event, 200).is_ok(),
            case[1].as_bool().unwrap()
        );
    }
    let bad_shared = signed_event(
        3,
        200,
        AGENT_PERSONA_KIND,
        vec![
            Tag::new(vec!["d".into(), "agent".into()]),
            Tag::new(vec!["shared".into(), "false".into()]),
        ],
        "{}",
    );
    assert!(validate_block_ingest(&bad_shared, 200).is_err());
}

#[test]
fn niper_reminder_server_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/niper/server.json")).unwrap();
    for case in fixture["not_before"].as_array().unwrap() {
        let validation_now = case[0].as_str().unwrap().parse::<u64>().unwrap_or(200);
        let event = signed_event(
            3,
            200,
            EVENT_REMINDER_KIND,
            vec![
                Tag::new(vec!["d".into(), "0123456789abcdef0123456789abcdef".into()]),
                Tag::new(vec!["not_before".into(), case[0].as_str().unwrap().into()]),
            ],
            &fake_nip44_v2(),
        );
        assert_eq!(
            validate_block_ingest(&event, validation_now).is_ok(),
            case[1].as_bool().unwrap()
        );
    }
    let too_far = signed_event(
        3,
        200,
        EVENT_REMINDER_KIND,
        vec![
            Tag::new(vec!["d".into(), "0123456789abcdef0123456789abcdef".into()]),
            Tag::new(vec!["not_before".into(), "31536001".into()]),
        ],
        &fake_nip44_v2(),
    );
    assert_eq!(
        validate_block_ingest(&too_far, 0).unwrap_err(),
        "not_before too far in future"
    );
    let ordered = signed_event(
        3,
        200,
        EVENT_REMINDER_KIND,
        vec![
            Tag::new(vec!["d".into(), "0123456789abcdef0123456789abcdef".into()]),
            Tag::new(vec!["not_before".into(), "300".into()]),
            Tag::new(vec!["expiration".into(), "300".into()]),
        ],
        &fake_nip44_v2(),
    );
    assert!(validate_block_ingest(&ordered, 200).is_err());
}

#[test]
fn nipmp_project_server_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipmp/server.json")).unwrap();
    let members = fixture["valid_members"].as_array().unwrap();
    let event = signed_event(
        3,
        200,
        PROJECT_KIND,
        vec![
            Tag::new(vec!["d".into(), "platform".into()]),
            Tag::new(vec!["a".into(), members[0].as_str().unwrap().into()]),
            Tag::new(vec![
                "a".into(),
                members[1].as_str().unwrap().into(),
                "wss://relay".into(),
            ]),
        ],
        "ignored",
    );
    assert!(validate_block_ingest(&event, 200).is_ok());
    let mut duplicate = event.clone();
    duplicate.tags.push(duplicate.tags[1].clone());
    resign(&mut duplicate, 3);
    assert!(
        validate_block_ingest(&duplicate, 200)
            .unwrap_err()
            .contains("member-duplicate")
    );
    assert_eq!(fixture["reject_rules"].as_array().unwrap().len(), 8);
}

#[test]
fn nippl_push_lease_public_handler_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nippl/server.json")).unwrap();
    let event = signed_event(
        3,
        200,
        PUSH_LEASE_KIND,
        vec![
            Tag::new(vec!["d".into(), "installation".into()]),
            Tag::new(vec!["expiration".into(), "1000".into()]),
            Tag::new(vec!["exec".into(), "2026-08".into()]),
            Tag::new(vec!["alt".into(), "Push lease".into()]),
        ],
        &fake_nip44_v2(),
    );
    assert!(validate_block_ingest(&event, 200).is_ok());
    assert_eq!(
        fixture["executor_posture"],
        "fail-closed-when-not-advertised"
    );
    let mut unexpected = event;
    unexpected.tags.push(Tag::new(vec!["p".into(), pubkey(3)]));
    resign(&mut unexpected, 3);
    assert!(validate_block_ingest(&unexpected, 200).is_err());
}

#[test]
fn nipia_request_and_relay_only_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipia/server.json")).unwrap();
    let event = signed_event(
        3,
        200,
        9_035,
        vec![
            Tag::new(vec!["-".into()]),
            Tag::new(vec!["p".into(), pubkey(4)]),
        ],
        "retired",
    );
    let request = parse_identity_archive_request(&event, 200).unwrap();
    assert!(request.archive);
    assert_eq!(request.target, pubkey(4));
    for kind in fixture["relay_only"].as_array().unwrap() {
        assert!(RELAY_ONLY_BLOCK_KINDS.contains(&(kind.as_u64().unwrap() as u16)));
    }
}

#[test]
fn nipdv_visibility_command_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipdv/server.json")).unwrap();
    let event = signed_event(
        3,
        200,
        DM_HIDE_KIND,
        vec![Tag::new(vec!["h".into(), "dm-channel".into()])],
        "",
    );
    assert_eq!(dm_visibility_channel(&event).unwrap(), "dm-channel");
    assert!(RELAY_ONLY_BLOCK_KINDS.contains(&(fixture["snapshot_kind"].as_u64().unwrap() as u16)));
}

#[test]
fn nipwp_workspace_icon_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipwp/server.json")).unwrap();
    for icon in fixture["valid"].as_array().unwrap() {
        let event = signed_event(
            3,
            200,
            9_033,
            vec![Tag::new(vec!["icon".into(), icon.as_str().unwrap().into()])],
            "",
        );
        assert!(workspace_icon(&event).is_ok());
    }
    for icon in fixture["invalid"].as_array().unwrap() {
        let event = signed_event(
            3,
            200,
            9_033,
            vec![Tag::new(vec!["icon".into(), icon.as_str().unwrap().into()])],
            "",
        );
        assert!(workspace_icon(&event).is_err());
    }
}

#[test]
fn nipcw_websocket_degradation_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipcw/server.json")).unwrap();
    let filter: Filter = serde_json::from_value(serde_json::json!({
        "kinds": [9], "#h": ["channel"], "limit": 50,
        "top_level": true, "include_summaries": true, "include_aux": true,
        "until": 200, "before_id": "0".repeat(64)
    }))
    .unwrap();
    let serialized = serde_json::to_value(filter).unwrap();
    for field in fixture["discarded_extension_fields"].as_array().unwrap() {
        assert!(serialized.get(field.as_str().unwrap()).is_none());
    }
    assert!(RELAY_ONLY_BLOCK_KINDS.contains(&39_005));
    assert!(RELAY_ONLY_BLOCK_KINDS.contains(&39_006));
}

#[test]
fn niprs_standard_addressable_server_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/niprs/server.json")).unwrap();
    let event = signed_event(
        3,
        200,
        READ_STATE_KIND,
        vec![
            Tag::new(vec!["d".into(), fixture["d"].as_str().unwrap().into()]),
            Tag::new(vec!["t".into(), fixture["t"].as_str().unwrap().into()]),
        ],
        &fake_nip44_v2(),
    );
    assert_eq!(event.class(), EventClass::Addressable);
    assert_eq!(event.distinct_parameter(), fixture["d"].as_str());
    assert!(validate_block_ingest(&event, 200).is_ok());
}

#[test]
fn nipgs_has_no_relay_handler_fixture_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipgs/server.json")).unwrap();
    assert!(fixture["nostr_event_kinds"].as_array().unwrap().is_empty());
    assert!(fixture["relay_handlers"].as_array().unwrap().is_empty());
    assert!(
        include_str!("../../../nips/block/NIP-GS.md")
            .contains("does not define any Nostr event kinds")
    );
}

fn signed_event(
    secret_byte: u8,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: &str,
) -> Event {
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: pubkey(secret_byte),
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    resign(&mut event, secret_byte);
    event
}

fn resign(event: &mut Event, secret_byte: u8) {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let digest = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp.sign_schnorr_no_aux_rand(&digest, &keypair).to_string();
}

fn pubkey(secret_byte: u8) -> String {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    Keypair::from_secret_key(&secp, &secret)
        .x_only_public_key()
        .0
        .to_string()
}

fn fake_nip44_v2() -> String {
    let mut bytes = [0_u8; 99];
    bytes[0] = 0x02;
    base64(&bytes)
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(TABLE[usize::from(first >> 2)]));
        output.push(char::from(
            TABLE[usize::from((first & 3) << 4 | second >> 4)],
        ));
        output.push(if chunk.len() > 1 {
            char::from(TABLE[usize::from((second & 15) << 2 | third >> 6)])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(TABLE[usize::from(third & 63)])
        } else {
            '='
        });
    }
    output
}
