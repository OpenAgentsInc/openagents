//! Fixture-backed Block agent identity and turn protocol coverage.

use nostr_relay::domain::{
    AgentObserverDirection, Event, Tag, agent_observer_route, agent_turn_metric_owner,
    verify_agent_auth_attestation, verify_owner_attestation,
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct OaFixture {
    #[allow(dead_code)]
    source: String,
    owner_pubkey: String,
    event: Event,
    invalid_conditions: Vec<String>,
}

#[test]
fn nipoa_owner_attestation_fixture_corpus() {
    let fixture: OaFixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nipoa/attestation.json"
    ))
    .unwrap();
    let verified = verify_owner_attestation(&fixture.event)
        .unwrap()
        .expect("fixture carries an attestation");
    assert_eq!(verified.owner_pubkey, fixture.owner_pubkey);

    for conditions in fixture.invalid_conditions {
        let event = attested_event(3, 4, 200, 1, &conditions);
        assert!(
            verify_owner_attestation(&event).is_err(),
            "condition should be invalid: {conditions}"
        );
    }

    let mut duplicate = attested_event(3, 4, 200, 1, "");
    duplicate.tags.push(duplicate.tags[0].clone());
    resign(&mut duplicate, 4);
    assert!(verify_owner_attestation(&duplicate).is_err());

    let self_pubkey = pubkey(3);
    let self_tag = auth_tag(3, &self_pubkey, "");
    let self_attested = signed_event(3, 200, 1, vec![self_tag], "self");
    assert!(verify_owner_attestation(&self_attested).is_err());
}

#[derive(Deserialize)]
struct AaFixture {
    cases: Vec<AaCase>,
}

#[derive(Deserialize)]
struct AaCase {
    name: String,
    conditions: String,
    created_at: u64,
    auth_tags: usize,
    valid: bool,
}

#[test]
fn nipaa_authentication_credential_fixture_corpus() {
    let fixture: AaFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipaa/auth.json")).unwrap();
    for case in fixture.cases {
        let agent = pubkey(4);
        let tag = auth_tag(3, &agent, &case.conditions);
        let tags = std::iter::repeat_n(tag, case.auth_tags).collect();
        let event = signed_event(4, case.created_at, 22_242, tags, "");
        let accepted = matches!(verify_agent_auth_attestation(&event), Ok(Some(_)));
        assert_eq!(accepted, case.valid, "case: {}", case.name);
    }
}

#[derive(Deserialize)]
struct AoFixture {
    cases: Vec<AoCase>,
}

#[derive(Deserialize)]
struct AoCase {
    name: String,
    sender: String,
    recipient: String,
    agent: String,
    frame: String,
    content: String,
    duplicate_p: bool,
    outcome: String,
}

#[test]
fn nipao_observer_envelope_fixture_corpus() {
    let fixture: AoFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nipao/observer.json")).unwrap();
    for case in fixture.cases {
        let owner = pubkey(3);
        let agent = pubkey(4);
        let resolve = |role: &str| match role {
            "owner" => owner.clone(),
            "agent" => agent.clone(),
            _ => panic!("unknown fixture role"),
        };
        let mut tags = vec![
            Tag::new(vec!["p".into(), resolve(&case.recipient)]),
            Tag::new(vec!["agent".into(), resolve(&case.agent)]),
            Tag::new(vec!["frame".into(), case.frame]),
        ];
        if case.duplicate_p {
            tags.push(tags[0].clone());
        }
        let sender = if case.sender == "owner" { 3 } else { 4 };
        let content = if case.content == "nip44" {
            fake_nip44_v2()
        } else {
            "plaintext".to_owned()
        };
        let event = signed_event(sender, 200, 24_200, tags, &content);
        let outcome = match agent_observer_route(&event) {
            Ok(Some(route)) if route.direction == AgentObserverDirection::Telemetry => "telemetry",
            Ok(Some(route)) if route.direction == AgentObserverDirection::Control => "control",
            Ok(Some(_)) => unreachable!(),
            Ok(None) => "drop",
            Err(_) => "reject",
        };
        assert_eq!(outcome, case.outcome, "case: {}", case.name);
    }
}

#[derive(Deserialize)]
struct AmFixture {
    cases: Vec<AmCase>,
}

#[derive(Deserialize)]
struct AmCase {
    name: String,
    p_tags: usize,
    agent_tags: usize,
    agent_matches: bool,
    has_h: bool,
    content: String,
    valid: bool,
}

#[test]
fn nipam_turn_metric_envelope_fixture_corpus() {
    let fixture: AmFixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nipam/turn-metrics.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let owner = pubkey(3);
        let agent = pubkey(4);
        let mut tags = Vec::new();
        tags.extend(std::iter::repeat_n(
            Tag::new(vec!["p".into(), owner.clone()]),
            case.p_tags,
        ));
        let tagged_agent = if case.agent_matches {
            agent.clone()
        } else {
            pubkey(5)
        };
        tags.extend(std::iter::repeat_n(
            Tag::new(vec!["agent".into(), tagged_agent]),
            case.agent_tags,
        ));
        if case.has_h {
            tags.push(Tag::new(vec!["h".into(), "private-channel".into()]));
        }
        let content = if case.content == "nip44" {
            fake_nip44_v2()
        } else {
            "plaintext".to_owned()
        };
        let event = signed_event(4, 200, 44_200, tags, &content);
        assert_eq!(
            agent_turn_metric_owner(&event).is_ok(),
            case.valid,
            "case: {}",
            case.name
        );
    }
}

fn attested_event(
    owner_secret: u8,
    agent_secret: u8,
    created_at: u64,
    kind: u16,
    conditions: &str,
) -> Event {
    let agent = pubkey(agent_secret);
    signed_event(
        agent_secret,
        created_at,
        kind,
        vec![auth_tag(owner_secret, &agent, conditions)],
        "attested",
    )
}

fn auth_tag(owner_secret: u8, agent_pubkey: &str, conditions: &str) -> Tag {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([owner_secret; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let digest: [u8; 32] =
        Sha256::digest(format!("nostr:agent-auth:{agent_pubkey}:{conditions}").as_bytes()).into();
    let signature = secp.sign_schnorr_no_aux_rand(&digest, &keypair).to_string();
    Tag::new(vec![
        "auth".into(),
        keypair.x_only_public_key().0.to_string(),
        conditions.into(),
        signature,
    ])
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
