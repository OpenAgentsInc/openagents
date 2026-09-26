//! NIP-KB fixtures: every event is built and signed here with throwaway
//! keys derived from a label, so no key is stored anywhere.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;
use crate::contracts::{prepare_closure, validate_instance};
use crate::domain::RelaySigner;

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

fn sign(signer: &RelaySigner, parts: Unsigned) -> Event {
    signer.sign(1_790_000_000, parts.kind, parts.tags, parts.content)
}

const DOCUMENT: &str = "---\nid: stats.mmd\nversion: 1\nkind: method\n---\n\n## Details\n\nBody.\n";

fn topics() -> Vec<String> {
    vec!["statistics".to_string(), "kernel".to_string()]
}

fn signed_entry(author: &RelaySigner) -> Event {
    sign(
        author,
        entry("stats.mmd", 1, "method", &topics(), DOCUMENT).unwrap(),
    )
}

fn body(event: &Event) -> Value {
    serde_json::from_str(&event.content).unwrap()
}

fn schema_check(file: &[u8], instance: &Value) {
    let digest = digest_bytes(file);
    let mut documents = BTreeMap::new();
    documents.insert(digest.clone(), file.to_vec());
    let closure = prepare_closure(&documents).expect("schema");
    validate_instance(&closure, &digest, instance).expect("instance matches its schema");
}

#[test]
fn an_entry_round_trips_and_matches_its_schema() {
    let author = signer("author");
    let event = signed_entry(&author);
    let parsed = parse_entry(&event).unwrap();
    assert_eq!(parsed.id, "stats.mmd");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.kind, "method");
    assert_eq!(parsed.document, DOCUMENT);
    assert_eq!(parsed.topics, topics());
    assert_eq!(parsed.digest.len(), 64);
    schema_check(
        include_bytes!("../../../../nips/openagents/schemas/kb-entry.v1.json"),
        &body(&event),
    );
}

#[test]
fn a_head_and_a_withdrawal_bind_to_their_entry() {
    let author = signer("author");
    let event = signed_entry(&author);
    let head_event = sign(&author, head(&event).unwrap());
    let parsed = parse_head(&head_event).unwrap();
    bind(&parsed.entry, &parsed.id, parsed.version, &event).unwrap();
    schema_check(
        include_bytes!("../../../../nips/openagents/schemas/kb-head.v1.json"),
        &body(&head_event),
    );
    let gone = sign(&author, withdrawal(&event, "wrong formula").unwrap());
    let parsed = parse_withdrawal(&gone).unwrap();
    assert_eq!(parsed.reason, "wrong formula");
    bind(&parsed.entry, &parsed.id, parsed.version, &event).unwrap();
    schema_check(
        include_bytes!("../../../../nips/openagents/schemas/kb-withdrawal.v1.json"),
        &body(&gone),
    );
    // A pointer to a different version doesn't bind.
    assert_eq!(
        bind(&parsed.entry, &parsed.id, 2, &event).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_tampered_entry_is_refused() {
    let author = signer("author");
    let mut event = signed_entry(&author);
    event.content = event.content.replace("Body.", "Other.");
    assert_eq!(
        parse_entry(&event).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    // Re-signed by the author with the old digest tag: the tag disagrees.
    let mut parts = entry("stats.mmd", 1, "method", &topics(), DOCUMENT).unwrap();
    parts.content = parts.content.replace("Body.", "Other.");
    let error = parse_entry(&sign(&author, parts)).unwrap_err();
    assert_eq!(error.code, RefusalCode::IdentityMismatch);
}

#[test]
fn a_body_that_disagrees_with_its_tags_is_refused() {
    let author = signer("author");
    let mut parts = entry("stats.mmd", 1, "method", &topics(), DOCUMENT).unwrap();
    parts.content = parts.content.replace("\"method\"", "\"slip\"");
    assert_eq!(
        parse_entry(&sign(&author, parts)).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    let mut parts = entry("stats.mmd", 1, "method", &topics(), DOCUMENT).unwrap();
    parts.content = parts
        .content
        .replace("\"type\":\"entry\"", "\"type\":\"head\"");
    assert!(parse_entry(&sign(&author, parts)).is_err());
    let mut parts = entry("stats.mmd", 1, "method", &topics(), DOCUMENT).unwrap();
    let mut extended: Value = serde_json::from_str(&parts.content).unwrap();
    extended["run"] = json!("rm -rf /");
    parts.content = extended.to_string();
    assert_eq!(
        parse_entry(&sign(&author, parts)).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
}

#[test]
fn bad_identities_and_topics_are_refused_before_signing() {
    assert!(entry("Stats", 1, "method", &[], DOCUMENT).is_err());
    assert!(entry("stats.mmd", 0, "method", &[], DOCUMENT).is_err());
    assert!(entry("stats.mmd", 1, "fact", &[], DOCUMENT).is_err());
    assert!(entry("stats.mmd", 1, "method", &["Kernel".to_string()], DOCUMENT).is_err());
    assert!(entry("stats.mmd", 1, "method", &["oa:kb:x".to_string()], DOCUMENT).is_err());
    assert_eq!(
        qualified_id(&"a".repeat(64), "stats.mmd"),
        format!("{}:kb/stats_mmd", "a".repeat(64))
    );
}

#[test]
fn only_the_author_heads_or_withdraws_an_entry() {
    let author = signer("author");
    let other = signer("other");
    let event = signed_entry(&author);
    let foreign = sign(&other, head(&event).unwrap());
    assert_eq!(
        parse_head(&foreign).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    let foreign = sign(&other, withdrawal(&event, "no").unwrap());
    assert_eq!(
        parse_withdrawal(&foreign).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn two_documents_for_one_version_are_equivocation() {
    let author = signer("author");
    let one = parse_entry(&signed_entry(&author)).unwrap();
    let other = parse_entry(&sign(
        &author,
        entry(
            "stats.mmd",
            1,
            "method",
            &[],
            &DOCUMENT.replace("Body", "Else"),
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        equivocation(&one, &other).unwrap_err().code,
        RefusalCode::Conflict
    );
    equivocation(&one, &one.clone()).unwrap();
}

fn report(evaluator: &str, entry_event: &Event) -> String {
    json!({
        "v": "openagents.eval-report.v1",
        "requires": [],
        "evaluator": evaluator,
        "subject": {
            "definition": {
                "id": qualified_id(&entry_event.pubkey, "stats.mmd"),
                "artifact": document_artifact(DOCUMENT),
                "event": {"id": entry_event.id, "pubkey": entry_event.pubkey, "kind": ENTRY_KIND},
            },
        },
        "verdict": "inconclusive",
    })
    .to_string()
}

#[test]
fn evidence_round_trips_and_checks_its_report() {
    let author = signer("author");
    let evaluator = signer("evaluator");
    let event = signed_entry(&author);
    let text = report(evaluator.pubkey(), &event);
    let published = sign(
        &evaluator,
        evidence(&text, std::slice::from_ref(&event.id)).unwrap(),
    );
    let parsed = parse_evidence(&published).unwrap();
    assert_eq!(parsed.report_bytes, text);
    assert_eq!(parsed.entries, std::slice::from_ref(&event.id));
    assert_eq!(parsed.subject.event.unwrap().id, event.id);

    // The inline report no longer matches its digest.
    let mut parts = evidence(&text, std::slice::from_ref(&event.id)).unwrap();
    parts.content = parts.content.replace("inconclusive", "pass");
    assert_eq!(
        parse_evidence(&sign(&evaluator, parts)).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    // Signed by someone other than the report's evaluator.
    let forged = sign(
        &author,
        evidence(&text, std::slice::from_ref(&event.id)).unwrap(),
    );
    assert_eq!(
        parse_evidence(&forged).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    // No `e` tag for the subject's entry.
    let unbound = sign(&evaluator, evidence(&text, &["0".repeat(64)]).unwrap());
    assert_eq!(
        parse_evidence(&unbound).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
}
