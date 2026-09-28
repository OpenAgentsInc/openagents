//! Gym results publication fixtures: every event is signed here with a
//! throwaway key derived from a label, so no key is stored anywhere.

use sha2::{Digest, Sha256};

use super::*;
use crate::domain::{RelaySigner, Tag};

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

const DIGEST: &str = "bf451beee1b755cf6e655920ac6f3124ae42f62c86e97bbe64677bcd5d8425aa";
const COMMIT: &str = "6bfc94876de82ec6a8c1a684dc650cb76131c20a";

fn boards() -> Vec<String> {
    vec![
        "tb4-fable-delegate-repro-9776".to_string(),
        "tb21-oos-microcoder-9683".to_string(),
    ]
}

fn sign(parts: Unsigned) -> Event {
    signer("publisher").sign(1_790_000_000, parts.kind, parts.tags, parts.content)
}

fn signed() -> Event {
    sign(publication(DIGEST, COMMIT, &boards()).unwrap())
}

#[test]
fn a_publication_round_trips() {
    let event = signed();
    assert_eq!(event.kind, PUBLICATION_KIND);
    let parsed = parse_publication(&event).unwrap();
    assert_eq!(parsed.digest, DIGEST);
    assert_eq!(parsed.commit, COMMIT);
    assert_eq!(parsed.boards, boards());
    assert_eq!(parsed.publisher, signer("publisher").pubkey());
    assert_eq!(parsed.id, event.id);
}

#[test]
fn a_changed_body_or_signature_refuses() {
    let mut event = signed();
    event.content = event.content.replace(COMMIT, &"0".repeat(40));
    assert_eq!(
        parse_publication(&event).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    let mut event = signed();
    event.pubkey = signer("someone else").pubkey().to_string();
    assert!(parse_publication(&event).is_err());
}

#[test]
fn tags_must_agree_with_the_body() {
    // A signed event whose x tag names another digest.
    let mut parts = publication(DIGEST, COMMIT, &boards()).unwrap();
    parts.tags[1] = Tag(vec!["x".into(), "a".repeat(64)]);
    assert_eq!(parse_publication(&sign(parts)).unwrap_err().detail, "x tag");
    // No marker, or a second one.
    let mut parts = publication(DIGEST, COMMIT, &boards()).unwrap();
    parts.tags.remove(0);
    assert!(parse_publication(&sign(parts)).is_err());
    let mut parts = publication(DIGEST, COMMIT, &boards()).unwrap();
    parts.tags.push(Tag(vec!["t".into(), "oa:eval:v1".into()]));
    assert!(parse_publication(&sign(parts)).is_err());
    // Another kind.
    let mut parts = publication(DIGEST, COMMIT, &boards()).unwrap();
    parts.kind = 3_189;
    assert!(parse_publication(&sign(parts)).is_err());
}

#[test]
fn a_body_outside_the_closed_shape_refuses() {
    let parts = publication(DIGEST, COMMIT, &boards()).unwrap();
    let mut body: Value = serde_json::from_str(&parts.content).unwrap();
    body["extra"] = Value::from(1);
    let event = sign(Unsigned {
        content: body.to_string(),
        ..parts.clone()
    });
    assert_eq!(
        parse_publication(&event).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    let mut body: Value = serde_json::from_str(&parts.content).unwrap();
    body["v"] = Value::from("openagents.gym-results-publication.v2");
    let event = sign(Unsigned {
        content: body.to_string(),
        ..parts.clone()
    });
    assert_eq!(
        parse_publication(&event).unwrap_err().code,
        RefusalCode::UnsupportedVersion
    );
    let mut body: Value = serde_json::from_str(&parts.content).unwrap();
    body["requires"] = serde_json::json!(["something"]);
    let event = sign(Unsigned {
        content: body.to_string(),
        ..parts
    });
    assert!(parse_publication(&event).is_err());
}

#[test]
fn bad_identities_refuse_to_build() {
    assert!(publication("ABC", COMMIT, &boards()).is_err());
    assert!(publication(&DIGEST.to_uppercase(), COMMIT, &boards()).is_err());
    assert!(publication(DIGEST, "6bfc948", &boards()).is_err());
    assert!(publication(DIGEST, COMMIT, &[]).is_err());
    let twice = vec!["a".to_string(), "a".to_string()];
    assert!(publication(DIGEST, COMMIT, &twice).is_err());
    assert!(publication(DIGEST, COMMIT, &["Has Space".to_string()]).is_err());
    let many: Vec<String> = (0..=MAX_BOARDS).map(|i| format!("b{i}")).collect();
    assert_eq!(
        publication(DIGEST, COMMIT, &many).unwrap_err().code,
        RefusalCode::LimitExceeded
    );
}
