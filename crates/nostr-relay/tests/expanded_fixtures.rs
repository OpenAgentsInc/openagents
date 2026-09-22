//! Fixture-backed domain coverage for the protocol expansion milestone.

use nostr_relay::domain::{
    Event, Filter, GroupAction, Tag, parse_http_authorization, parse_http_authorization_hash,
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct RoutingFixture {
    #[serde(default)]
    nip44_payload: Option<String>,
    cases: Vec<RoutingCase>,
}

#[derive(Deserialize)]
struct RoutingCase {
    name: String,
    kind: u16,
    tags: Vec<Tag>,
    #[serde(default)]
    nip44: bool,
    valid: bool,
}

#[test]
fn nip17_gift_wrap_and_inbox_list_fixture_corpus() {
    let fixture: RoutingFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip17/routing.json")).unwrap();
    for case in fixture.cases {
        let content = if case.nip44 {
            fixture.nip44_payload.as_deref().unwrap()
        } else {
            ""
        };
        let mut event = example_event(case.kind, case.tags, content);
        assert_eq!(
            event.validate_structure().is_ok(),
            case.valid,
            "case: {}",
            case.name
        );
        if case.kind == 1_059 && case.valid {
            assert_eq!(
                event.gift_wrap_recipient(),
                Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            );
        }
        event.id.clear();
    }
}

#[test]
fn nip65_relay_list_fixture_corpus() {
    let fixture: RoutingFixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nip65/relay-list.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let event = example_event(case.kind, case.tags, "");
        assert_eq!(
            event.validate_structure().is_ok(),
            case.valid,
            "case: {}",
            case.name
        );
    }
}

#[test]
fn nip94_file_metadata_fixture_corpus() {
    let fixture: RoutingFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip94/metadata.json")).unwrap();
    for case in fixture.cases {
        let event = example_event(case.kind, case.tags, "");
        assert_eq!(
            event.validate_structure().is_ok(),
            case.valid,
            "case: {}",
            case.name
        );
    }
}

#[test]
fn nipb7_server_list_fixture_corpus() {
    let fixture: RoutingFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipb7/servers.json")).unwrap();
    for case in fixture.cases {
        let event = example_event(case.kind, case.tags, "");
        assert_eq!(
            event.validate_structure().is_ok(),
            case.valid,
            "case: {}",
            case.name
        );
    }
}

#[derive(Deserialize)]
struct GroupFixture {
    cases: Vec<GroupCase>,
}

#[derive(Deserialize)]
struct GroupCase {
    name: String,
    kind: u16,
    tags: Vec<Tag>,
    action: String,
}

#[test]
fn nip29_group_action_fixture_corpus() {
    let fixture: GroupFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip29/groups.json")).unwrap();
    for case in fixture.cases {
        let event = example_event(case.kind, case.tags, "");
        event.validate_structure().unwrap();
        let action = GroupAction::from_event(&event).unwrap().unwrap();
        let actual = match action {
            GroupAction::PutUser { .. } => "put-user",
            GroupAction::RemoveUser { .. } => "remove-user",
            GroupAction::EditMetadata(_) => "edit-metadata",
            GroupAction::CreateInvite { .. } => "create-invite",
            GroupAction::UpdatePins { .. } => "update-pins",
            GroupAction::Join { .. } => "join",
            GroupAction::Leave => "leave",
            GroupAction::DeleteEvent { .. }
            | GroupAction::CreateGroup
            | GroupAction::DeleteGroup => "other",
        };
        assert_eq!(actual, case.action, "case: {}", case.name);
    }
}

#[derive(Deserialize)]
struct SearchFixture {
    content: String,
    cases: Vec<SearchCase>,
}

#[derive(Deserialize)]
struct SearchCase {
    search: String,
    matches: bool,
    valid: bool,
}

#[test]
fn nip50_search_filter_fixture_corpus() {
    let fixture: SearchFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip50/search.json")).unwrap();
    let event = example_event(1, Vec::new(), &fixture.content);
    for case in fixture.cases {
        let filter = Filter {
            search: Some(case.search),
            ..Filter::default()
        };
        assert_eq!(filter.validate().is_ok(), case.valid);
        assert_eq!(filter.matches(&event), case.matches);
    }
}

#[derive(Deserialize)]
struct ProtectedFixture {
    cases: Vec<ProtectedCase>,
}

#[derive(Deserialize)]
struct ProtectedCase {
    name: String,
    tags: Vec<Tag>,
    protected: bool,
}

#[test]
fn nip70_protected_tag_fixture_corpus() {
    let fixture: ProtectedFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip70/protected.json")).unwrap();
    for case in fixture.cases {
        let event = example_event(1, case.tags, "");
        assert_eq!(event.is_protected(), case.protected, "case: {}", case.name);
    }

    let protected = example_event(1, vec![Tag::new(vec!["-".into()])], "");
    let repost = example_event(6, Vec::new(), &serde_json::to_string(&protected).unwrap());
    assert!(repost.embeds_protected_repost());
    let generic_repost = example_event(16, Vec::new(), &serde_json::to_string(&protected).unwrap());
    assert!(generic_repost.embeds_protected_repost());
}

#[test]
fn nip98_http_auth_fixture_contract() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip98/http-auth.json")).unwrap();
    assert_eq!(fixture["kind"], 27_235);
    let payload = br#"{"method":"listallowedkinds","params":[]}"#;
    let payload_hash = hex(&Sha256::digest(payload));
    let now = 1_000;
    let event = signed_event(
        42,
        now,
        27_235,
        vec![
            Tag::new(vec!["u".into(), "https://relay.example/manage".into()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec!["payload".into(), payload_hash]),
        ],
        "",
    );
    let header = format!("Nostr {}", base64(&serde_json::to_vec(&event).unwrap()));
    let auth = parse_http_authorization(
        &header,
        "POST",
        "https://relay.example/manage",
        payload,
        now,
    )
    .unwrap();
    assert_eq!(auth.pubkey, event.pubkey);
    assert!(
        parse_http_authorization(&header, "GET", "https://relay.example/manage", payload, now,)
            .is_err()
    );

    let delete = signed_event(
        42,
        now,
        27_235,
        vec![
            Tag::new(vec![
                "u".into(),
                "https://relay.example/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
            ]),
            Tag::new(vec!["method".into(), "DELETE".into()]),
        ],
        "",
    );
    let delete_header = format!("Nostr {}", base64(&serde_json::to_vec(&delete).unwrap()));
    parse_http_authorization_hash(
        &delete_header,
        "DELETE",
        "https://relay.example/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        now,
    )
    .unwrap();
}

fn example_event(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
    Event {
        id: "0".repeat(64),
        pubkey: "a".repeat(64),
        created_at: 1,
        kind,
        tags,
        content: content.to_owned(),
        sig: "0".repeat(128),
    }
}

fn signed_event(
    secret_byte: u8,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: &str,
) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    let id = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
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
            TABLE[usize::from((first & 0x03) << 4 | second >> 4)],
        ));
        if chunk.len() > 1 {
            output.push(char::from(
                TABLE[usize::from((second & 0x0f) << 2 | third >> 6)],
            ));
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(char::from(TABLE[usize::from(third & 0x3f)]));
        } else {
            output.push('=');
        }
    }
    output
}
