//! NIP-XP fixtures: every event is built and signed here with throwaway
//! keys derived from a label, so no key is stored anywhere.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;
use crate::contracts::{digest_bytes, prepare_closure, validate_instance};
use crate::domain::RelaySigner;

const AT: u64 = 1_790_000_000;

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

fn sign_at(signer: &RelaySigner, at: u64, parts: Unsigned) -> Event {
    signer.sign(at, parts.kind, parts.tags, parts.content)
}

fn sign(signer: &RelaySigner, parts: Unsigned) -> Event {
    sign_at(signer, AT, parts)
}

const DOCUMENT: &str = "---\nid: git.reflog-recovery\nversion: 1\nkind: method\n---\n\nBody.\n";

fn spec() -> Value {
    json!({
        "id": "tb4.fix-git.beat-reference",
        "version": 1,
        "season": {"id": "2026-q4", "opens_at": AT - 1_000, "closes_at": AT + 1_000_000},
        "title": "Beat the reference's cheapest winning run on fix-git",
        "objective": "Publish an entry that makes a paired run pass fix-git for less than the reference.",
        "acceptance": {"rule": "kb-transfer", "task": "fix-git", "min_pass_rate": 1.0, "max_usd_per_run": 0.2},
        "reference": {"label": "Fable 5.1 low, cheapest winning run", "usd": 0.2, "seconds": 300, "source": null},
        "award": {"author": 6, "runner": 4},
    })
}

fn signed_quest(referee: &RelaySigner) -> Event {
    sign(referee, quest(&spec()).unwrap())
}

fn signed_entry(author: &RelaySigner) -> Event {
    sign(
        author,
        kb::entry("git.reflog-recovery", 1, "method", &[], DOCUMENT).unwrap(),
    )
}

/// A report on `entry` by `runner`, pairing on `task` with `passes` of two
/// runs with the entry, at `usd` in total.
fn report(runner: &str, entry: &Event, task: &str, passes: u64, usd: f64, verdict: &str) -> String {
    json!({
        "v": "openagents.eval-report.v1",
        "requires": [],
        "evaluator": runner,
        "subject": {"definition": {
            "id": kb::qualified_id(&entry.pubkey, "git.reflog-recovery"),
            "artifact": kb::document_artifact(DOCUMENT),
            "event": {"id": entry.id, "pubkey": entry.pubkey, "kind": kb::ENTRY_KIND},
        }},
        "verdict": verdict,
        "meta": {"kb": {"pairs": [
            {"task": "other-task", "model": "m",
             "with": {"runs": 1, "passes": 1, "usd": 0.1, "unknown": 0},
             "without": {"runs": 1, "passes": 0, "usd": 0.3, "unknown": 0}, "side": "favors"},
            {"task": task, "model": "m",
             "with": {"runs": 2, "passes": passes, "usd": usd, "unknown": 0},
             "without": {"runs": 2, "passes": 0, "usd": 0.6, "unknown": 0}, "side": "favors"},
        ]}},
    })
    .to_string()
}

fn signed_evidence(runner: &RelaySigner, entry: &Event, text: &str) -> Event {
    sign(
        runner,
        kb::evidence(text, std::slice::from_ref(&entry.id)).unwrap(),
    )
}

struct World {
    referee: RelaySigner,
    author: RelaySigner,
    runner: RelaySigner,
    quest: Event,
    entry: Event,
    evidence: Event,
}

fn world() -> World {
    let referee = signer("referee");
    let author = signer("author");
    let runner = signer("runner");
    let quest = signed_quest(&referee);
    let entry = signed_entry(&author);
    let text = report(runner.pubkey(), &entry, "fix-git", 2, 0.3, "pass");
    let evidence = signed_evidence(&runner, &entry, &text);
    World {
        referee,
        author,
        runner,
        quest,
        entry,
        evidence,
    }
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

fn code(result: Result<impl std::fmt::Debug, ContractError>) -> RefusalCode {
    result.unwrap_err().code
}

#[test]
fn a_quest_round_trips_and_matches_its_schema() {
    let w = world();
    let parsed = parse_quest(&w.quest).unwrap();
    assert_eq!(parsed.address, "tb4.fix-git.beat-reference@1");
    assert_eq!(parsed.total(), 10);
    assert_eq!(parsed.completions, "first");
    assert_eq!(parsed.acceptance.max_usd_per_run, Some(0.2));
    assert_eq!(parsed.reference.unwrap().seconds, Some(300));
    schema_check(
        include_bytes!("../../../../nips/openagents/schemas/xp-quest.v1.json"),
        &body(&w.quest),
    );
}

#[test]
fn bad_quests_are_refused_before_signing() {
    let with = |pointer: &str, value: Value| {
        let mut spec = spec();
        *spec.pointer_mut(pointer).unwrap() = value;
        quest(&spec)
    };
    assert_eq!(code(with("/id", json!("Bad Id"))), RefusalCode::Malformed);
    assert_eq!(code(with("/version", json!(0))), RefusalCode::Malformed);
    assert_eq!(
        code(with("/acceptance/rule", json!("per-commit"))),
        RefusalCode::UnsupportedFeature
    );
    assert_eq!(
        code(with("/acceptance/min_pass_rate", json!(1.5))),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(with("/award", json!({"author": 0, "runner": 0}))),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(with("/award", json!({"author": 900, "runner": 900}))),
        RefusalCode::LimitExceeded
    );
    assert_eq!(
        code(with(
            "/award",
            json!({"author": 1, "runner": 1, "reviewer": 1})
        )),
        RefusalCode::UnsupportedFeature
    );
    assert_eq!(
        code(with("/season/closes_at", json!(0))),
        RefusalCode::Malformed
    );
    let mut spec = spec();
    spec["completions"] = json!("every");
    assert_eq!(code(quest(&spec)), RefusalCode::UnsupportedFeature);
    let mut spec = self::spec();
    spec["v"] = json!(1);
    assert_eq!(code(quest(&spec)), RefusalCode::Malformed);
}

#[test]
fn a_quest_whose_tags_disagree_is_refused() {
    let referee = signer("referee");
    let mut parts = quest(&spec()).unwrap();
    parts.tags[0] = tag(&["d", "tb4.fix-git.beat-reference@2"]);
    assert_eq!(
        code(parse_quest(&sign(&referee, parts))),
        RefusalCode::IdentityMismatch
    );
    let mut parts = quest(&spec()).unwrap();
    parts.tags[2] = tag(&["t", "oa:xp:season:2027-q1"]);
    assert_eq!(
        code(parse_quest(&sign(&referee, parts))),
        RefusalCode::IdentityMismatch
    );
    let mut tampered = signed_quest(&referee);
    tampered.content = tampered.content.replace("\"author\":6", "\"author\":600");
    assert_eq!(code(parse_quest(&tampered)), RefusalCode::IdentityMismatch);
}

#[test]
fn an_award_round_trips_binds_and_matches_its_schema() {
    let w = world();
    let parts = award(&w.quest, &w.entry, &w.evidence, &[], AT + 10).unwrap();
    let event = sign_at(&w.referee, AT + 10, parts);
    let parsed = parse_award(&event).unwrap();
    assert_eq!(parsed.total(), 10);
    assert_eq!(parsed.role("author").unwrap().pubkey, w.author.pubkey());
    assert_eq!(parsed.role("runner").unwrap().pubkey, w.runner.pubkey());
    assert_eq!(
        parsed.key,
        coordinate(w.referee.pubkey(), "tb4.fix-git.beat-reference@1")
    );
    let quest = bind_quest(&parsed, &w.quest).unwrap();
    bind_evidence(&parsed, &quest, &w.entry, &w.evidence, &[]).unwrap();
    schema_check(
        include_bytes!("../../../../nips/openagents/schemas/xp-award.v1.json"),
        &body(&event),
    );
}

#[test]
fn the_rule_refuses_completions_that_do_not_qualify() {
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    let check = |evidence: &Event, excluded: &[String]| {
        check_transfer(&quest, &w.entry, evidence, excluded).map(|_| ())
    };
    // Written from the quest's task: not out of sample.
    assert_eq!(
        code(check(&w.evidence, &["fix-git".to_string()])),
        RefusalCode::NotAdmitted
    );
    // The author runs their own evidence.
    let text = report(w.author.pubkey(), &w.entry, "fix-git", 2, 0.3, "pass");
    assert_eq!(
        code(check(&signed_evidence(&w.author, &w.entry, &text), &[])),
        RefusalCode::NotAdmitted
    );
    // One of two runs failed: under the quest's pass rate.
    let text = report(w.runner.pubkey(), &w.entry, "fix-git", 1, 0.3, "pass");
    assert_eq!(
        code(check(&signed_evidence(&w.runner, &w.entry, &text), &[])),
        RefusalCode::NotAdmitted
    );
    // $0.25 per run is not under $0.20.
    let text = report(w.runner.pubkey(), &w.entry, "fix-git", 2, 0.5, "pass");
    assert_eq!(
        code(check(&signed_evidence(&w.runner, &w.entry, &text), &[])),
        RefusalCode::NotAdmitted
    );
    // A report that doesn't pass, or doesn't pair on the task.
    let text = report(
        w.runner.pubkey(),
        &w.entry,
        "fix-git",
        2,
        0.3,
        "inconclusive",
    );
    assert_eq!(
        code(check(&signed_evidence(&w.runner, &w.entry, &text), &[])),
        RefusalCode::NotAdmitted
    );
    let text = report(w.runner.pubkey(), &w.entry, "another", 2, 0.3, "pass");
    assert_eq!(
        code(check(&signed_evidence(&w.runner, &w.entry, &text), &[])),
        RefusalCode::NotAdmitted
    );
    // Evidence from before the season opened.
    let text = report(w.runner.pubkey(), &w.entry, "fix-git", 2, 0.3, "pass");
    let early = sign_at(
        &w.runner,
        AT - 5_000,
        kb::evidence(&text, std::slice::from_ref(&w.entry.id)).unwrap(),
    );
    assert_eq!(code(check(&early, &[])), RefusalCode::NotAdmitted);
    // Evidence about another entry.
    let other = sign(
        &w.author,
        kb::entry(
            "git.reflog-recovery",
            2,
            "method",
            &[],
            &DOCUMENT.replace("version: 1", "version: 2"),
        )
        .unwrap(),
    );
    assert_eq!(
        code(check_transfer(&quest, &other, &w.evidence, &[]).map(|_| ())),
        RefusalCode::IdentityMismatch
    );
    // A report that cites the entry event but measured other bytes, or
    // names the entry in the runner's namespace instead of its author's.
    let text = report(w.runner.pubkey(), &w.entry, "fix-git", 2, 0.3, "pass");
    let mut value: Value = serde_json::from_str(&text).unwrap();
    value["subject"]["definition"]["artifact"] =
        kb::document_artifact(&DOCUMENT.replace("Body.", "Other body."));
    assert_eq!(
        code(check(
            &signed_evidence(&w.runner, &w.entry, &value.to_string()),
            &[]
        )),
        RefusalCode::IdentityMismatch
    );
    let mut value: Value = serde_json::from_str(&text).unwrap();
    value["subject"]["definition"]["id"] =
        json!(kb::qualified_id(w.runner.pubkey(), "git.reflog-recovery"));
    assert_eq!(
        code(check(
            &signed_evidence(&w.runner, &w.entry, &value.to_string()),
            &[]
        )),
        RefusalCode::IdentityMismatch
    );
    // The builder refuses what the rule refuses.
    assert!(
        award(
            &w.quest,
            &w.entry,
            &w.evidence,
            &["fix-git".to_string()],
            AT
        )
        .is_err()
    );
    // And a time outside the season.
    assert_eq!(
        code(award(&w.quest, &w.entry, &w.evidence, &[], AT + 2_000_000)),
        RefusalCode::Stale
    );
}

#[test]
fn a_forged_or_inflated_award_is_refused() {
    let w = world();
    let good = award(&w.quest, &w.entry, &w.evidence, &[], AT).unwrap();

    // Signed by someone other than the quest's referee.
    let forged = sign(&signer("mallory"), good.clone());
    assert_eq!(code(parse_award(&forged)), RefusalCode::IdentityMismatch);

    // More XP than the quest's table: parses alone, refused when bound.
    let mut inflated = good.clone();
    inflated.content = inflated.content.replace("\"xp\":6", "\"xp\":60");
    let inflated = parse_award(&sign(&w.referee, inflated)).unwrap();
    assert_eq!(code(bind_quest(&inflated, &w.quest)), RefusalCode::Conflict);

    // The runner swapped for the author: self-evidence.
    let mut value: Value = serde_json::from_str(&good.content).unwrap();
    value["awardees"][1]["pubkey"] = json!(w.author.pubkey());
    value["evidence"][0]["pubkey"] = json!(w.author.pubkey());
    let mut swapped = good.clone();
    swapped.content = value.to_string();
    swapped.tags.retain(|t| t.name() != Some("p"));
    swapped.tags.push(tag(&["p", w.author.pubkey()]));
    assert_eq!(
        code(parse_award(&sign(&w.referee, swapped))),
        RefusalCode::NotAdmitted
    );

    // A key other than the quest version's coordinate.
    let mut rekeyed = good.clone();
    rekeyed.content = rekeyed.content.replace("\"key\":\"", "\"key\":\"x");
    assert_eq!(
        code(parse_award(&sign(&w.referee, rekeyed))),
        RefusalCode::IdentityMismatch
    );

    // Tags that don't name the body's events.
    let mut untagged = good.clone();
    untagged.tags.retain(|t| t.name() != Some("e"));
    assert_eq!(
        code(parse_award(&sign(&w.referee, untagged))),
        RefusalCode::IdentityMismatch
    );

    // A second quest version, same referee: the award doesn't bind to it.
    let mut spec = spec();
    spec["version"] = json!(2);
    let second = sign(&w.referee, quest(&spec).unwrap());
    let parsed = parse_award(&sign(&w.referee, good.clone())).unwrap();
    assert_eq!(
        code(bind_quest(&parsed, &second)),
        RefusalCode::IdentityMismatch
    );

    // Bound to evidence it doesn't name.
    let text = report(w.runner.pubkey(), &w.entry, "fix-git", 2, 0.1, "pass");
    let other = signed_evidence(&w.runner, &w.entry, &text);
    let quest = parse_quest(&w.quest).unwrap();
    assert_eq!(
        code(bind_evidence(&parsed, &quest, &w.entry, &other, &[])),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn only_the_referee_revokes_an_award() {
    let w = world();
    let event = sign(
        &w.referee,
        award(&w.quest, &w.entry, &w.evidence, &[], AT).unwrap(),
    );
    let revoked = sign(
        &w.referee,
        revocation(&event, "the runs were mislabeled").unwrap(),
    );
    let parsed = parse_revocation(&revoked).unwrap();
    assert_eq!(parsed.award.id, event.id);
    assert_eq!(parsed.key, parse_award(&event).unwrap().key);
    schema_check(
        include_bytes!("../../../../nips/openagents/schemas/xp-revocation.v1.json"),
        &body(&revoked),
    );
    let foreign = sign(&w.author, revocation(&event, "I disagree").unwrap());
    assert_eq!(
        code(parse_revocation(&foreign)),
        RefusalCode::IdentityMismatch
    );
    assert_eq!(code(revocation(&event, " ")), RefusalCode::Malformed);
    assert_eq!(
        code(revocation(&event, &"x".repeat(MAX_REASON_CHARS + 1))),
        RefusalCode::LimitExceeded
    );
}

#[test]
fn an_achievement_label_points_at_its_award() {
    let w = world();
    let event = sign(
        &w.referee,
        award(&w.quest, &w.entry, &w.evidence, &[], AT).unwrap(),
    );
    let label = sign(&w.referee, achievement(&event, "beat-reference").unwrap());
    let parsed = parse_achievement(&label).unwrap();
    assert_eq!(parsed.award, event.id);
    assert_eq!(parsed.value, "beat-reference");
    assert_eq!(parsed.awardees.len(), 2);
    assert!(achievement(&event, "Beat Reference").is_err());
    let mut other = achievement(&event, "beat-reference").unwrap();
    other.tags[0] = tag(&["L", "openagents.voyager"]);
    assert_eq!(
        code(parse_achievement(&sign(&w.referee, other))),
        RefusalCode::IdentityMismatch
    );
}
