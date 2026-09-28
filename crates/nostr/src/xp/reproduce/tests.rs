//! `reproduce` fixtures: a claim, a reproduction, and awards, signed with
//! throwaway keys derived from a label. No key is stored anywhere.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;
use crate::contracts::{prepare_closure, validate_instance};
use crate::domain::RelaySigner;
use crate::xp::{self, parse_award};

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

/// A Microcoder `summary.json` for `build-pmars` with `reward`; `nonce`
/// makes two runs' files differ.
fn summary(reward: f64, nonce: u64, model: &str) -> Vec<u8> {
    json!({
        "task": "build-pmars",
        "model": model,
        "effort": "medium",
        "outcome": {"ending": {"reason": "finished"}, "steps": 8, "seconds": 85.8 + nonce as f64,
                    "usd": 0.0065, "known_usd": 0.0065},
        "reward": reward,
        "image": "alexgshaw/build-pmars:20251031",
        "kb": "off",
        "container": format!("microcoder-build-pmars-{nonce}"),
    })
    .to_string()
    .into_bytes()
}

fn recipe() -> Value {
    recipe_from_summary(&summary(1.0, 0, "gpt-6-luna"), "terminal-bench", "2.1").unwrap()
}

fn claim(claimant: &RelaySigner, at: u64) -> Event {
    let record = record_from_summary(&summary(1.0, 0, "gpt-6-luna")).unwrap();
    sign_at(
        claimant,
        at,
        run_evidence(
            claimant.pubkey(),
            claimant.pubkey(),
            &recipe(),
            &record,
            &[],
        )
        .unwrap(),
    )
}

fn reproduction(reproducer: &RelaySigner, claim: &Event, bytes: &[u8], at: u64) -> Event {
    let record = record_from_summary(bytes).unwrap();
    sign_at(
        reproducer,
        at,
        run_evidence(
            reproducer.pubkey(),
            &claim.pubkey,
            &recipe(),
            &record,
            std::slice::from_ref(&claim.id),
        )
        .unwrap(),
    )
}

fn quest_spec(claim: &Event) -> Value {
    json!({
        "id": "tb21.build-pmars.reproduce",
        "version": 1,
        "season": {"id": "tb21-tutorial-s1", "opens_at": AT - 1_000, "closes_at": AT + 1_000_000},
        "title": "Reproduce Microcoder's pass on build-pmars",
        "objective": "Rerun the published pass from its recipe and pass the task's tests.",
        "acceptance": {
            "rule": "reproduce",
            "task": "build-pmars",
            "recipe": recipe_digest(&recipe()).unwrap(),
            "claim": {"id": claim.id, "pubkey": claim.pubkey, "kind": kb::EVIDENCE_KIND},
        },
        "reference": null,
        "award": {"claimant": 0, "reproducer": 50},
    })
}

struct World {
    referee: RelaySigner,
    reproducer: RelaySigner,
    quest: Event,
    claim: Event,
    reproduction: Event,
}

fn world() -> World {
    let referee = signer("referee");
    let claimant = signer("claimant");
    let reproducer = signer("reproducer");
    let claim = claim(&claimant, AT - 5_000);
    let quest = sign_at(&referee, AT, xp::quest(&quest_spec(&claim)).unwrap());
    let reproduction = reproduction(&reproducer, &claim, &summary(1.0, 1, "gpt-6-luna"), AT + 10);
    World {
        referee,
        reproducer,
        quest,
        claim,
        reproduction,
    }
}

fn code(result: Result<impl std::fmt::Debug, ContractError>) -> RefusalCode {
    result.unwrap_err().code
}

fn schema_check(file: &[u8], instance: &Value) {
    let digest = digest_bytes(file);
    let mut documents = BTreeMap::new();
    documents.insert(digest.clone(), file.to_vec());
    let closure = prepare_closure(&documents).expect("schema");
    validate_instance(&closure, &digest, instance).expect("instance matches its schema");
}

#[test]
fn a_reproduce_quest_round_trips_and_matches_its_schema() {
    let w = world();
    let parsed = parse_quest(&w.quest).unwrap();
    assert_eq!(parsed.acceptance.rule, REPRODUCE);
    assert_eq!(parsed.acceptance.claim.as_ref().unwrap().id, w.claim.id);
    assert_eq!(
        parsed.acceptance.recipe.as_deref(),
        Some(recipe_digest(&recipe()).unwrap().as_str())
    );
    assert_eq!(parsed.total(), 50);
    assert!(w.quest.tag_values("t").any(|t| t == "oa:xp:rule:reproduce"));
    let body: Value = serde_json::from_str(&w.quest.content).unwrap();
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-quest.v1.json"),
        &body,
    );
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-recipe.v1.json"),
        &recipe(),
    );
}

#[test]
fn a_reproduce_quest_refuses_kb_fields_and_kb_roles() {
    let w = world();
    let mut spec = quest_spec(&w.claim);
    spec["acceptance"]["min_pass_rate"] = json!(1.0);
    assert_eq!(code(xp::quest(&spec)), RefusalCode::UnsupportedFeature);
    let mut spec = quest_spec(&w.claim);
    spec["award"] = json!({"author": 6, "runner": 4});
    assert_eq!(code(xp::quest(&spec)), RefusalCode::UnsupportedFeature);
    let mut spec = quest_spec(&w.claim);
    spec["acceptance"]["recipe"] = json!("not-hex");
    assert_eq!(code(xp::quest(&spec)), RefusalCode::Malformed);
    let mut spec = quest_spec(&w.claim);
    spec["acceptance"]["claim"]["kind"] = json!(3190);
    assert_eq!(code(xp::quest(&spec)), RefusalCode::IdentityMismatch);
}

#[test]
fn run_evidence_round_trips_and_isnt_knowledge_evidence() {
    let w = world();
    let claim = parse_run_evidence(&w.claim).unwrap();
    assert_eq!(claim.verdict, "pass");
    assert_eq!(claim.owner, w.claim.pubkey);
    assert_eq!(claim.recipe.task, "build-pmars");
    assert_eq!(claim.recipe.agent, "microcoder");
    assert!(claim.cites.is_empty());
    assert_eq!(claim.record.ending.as_deref(), Some("finished"));
    let reproduced = parse_run_evidence(&w.reproduction).unwrap();
    assert_eq!(reproduced.cites, vec![w.claim.id.clone()]);
    assert_eq!(reproduced.subject_id, claim.subject_id);
    // NIP-KB readers refuse run evidence, and run readers refuse the
    // knowledge profile's marker set.
    assert!(kb::parse_evidence(&w.claim).is_err());
    // A failed run is evidence with a `fail` verdict.
    let failed = reproduction(
        &w.reproducer,
        &w.claim,
        &summary(0.0, 2, "gpt-6-luna"),
        AT + 10,
    );
    assert_eq!(parse_run_evidence(&failed).unwrap().verdict, "fail");
}

#[test]
fn tampered_run_evidence_is_refused() {
    let w = world();
    // A changed signature byte.
    let mut forged = w.reproduction.clone();
    forged.content = forged.content.replace("finished", "finishes");
    assert_eq!(
        code(parse_run_evidence(&forged)),
        RefusalCode::IdentityMismatch
    );
    // A record that didn't follow its recipe can't be built.
    let other = record_from_summary(&summary(1.0, 3, "gpt-6-sol")).unwrap();
    assert_eq!(
        code(run_evidence(
            w.reproducer.pubkey(),
            &w.claim.pubkey,
            &recipe(),
            &other,
            &[]
        )),
        RefusalCode::NotAdmitted
    );
    // A run without a reward was never graded.
    let ungraded = json!({"task": "build-pmars", "model": "m", "effort": "medium",
        "image": "i", "kb": "off", "reward": null})
    .to_string();
    assert_eq!(
        code(record_from_summary(ungraded.as_bytes())),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn a_reproduction_is_accepted_awarded_and_bound() {
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    check_reproduce(&quest, &w.claim, &w.reproduction).unwrap();
    let parts = reproduce_award(&w.quest, &w.claim, &w.reproduction, AT + 20).unwrap();
    let signed = sign_at(&w.referee, AT + 20, parts);
    let award = parse_award(&signed).unwrap();
    assert_eq!(award.rule, REPRODUCE);
    assert!(award.entry.is_none());
    assert_eq!(
        award.role("reproducer").unwrap().pubkey,
        w.reproducer.pubkey()
    );
    assert_eq!(award.role("reproducer").unwrap().xp, 50);
    assert_eq!(award.role("claimant").unwrap().xp, 0);
    let bound = xp::bind_quest(&award, &w.quest).unwrap();
    bind_reproduction(&award, &bound, &w.claim, &w.reproduction).unwrap();
    let body: Value = serde_json::from_str(&signed.content).unwrap();
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-award.v1.json"),
        &body,
    );
    // The award names the claim, then the reproduction; swapped, it binds
    // nothing.
    assert_eq!(
        code(bind_reproduction(&award, &bound, &w.reproduction, &w.claim)),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn the_claimant_cant_reproduce_their_own_attempt() {
    let w = world();
    let claimant = signer("claimant");
    let own = reproduction(&claimant, &w.claim, &summary(1.0, 4, "gpt-6-luna"), AT + 10);
    let quest = parse_quest(&w.quest).unwrap();
    assert_eq!(
        code(check_reproduce(&quest, &w.claim, &own)),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn a_failed_copied_late_or_uncited_reproduction_is_refused() {
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    let failed = reproduction(
        &w.reproducer,
        &w.claim,
        &summary(0.0, 5, "gpt-6-luna"),
        AT + 10,
    );
    assert_eq!(
        code(check_reproduce(&quest, &w.claim, &failed)),
        RefusalCode::NotAdmitted
    );
    // The claimant's own record, republished under another key.
    let copied = reproduction(
        &w.reproducer,
        &w.claim,
        &summary(1.0, 0, "gpt-6-luna"),
        AT + 10,
    );
    assert_eq!(
        code(check_reproduce(&quest, &w.claim, &copied)),
        RefusalCode::NotAdmitted
    );
    let late = reproduction(
        &w.reproducer,
        &w.claim,
        &summary(1.0, 6, "gpt-6-luna"),
        AT + 2_000_000,
    );
    assert_eq!(
        code(check_reproduce(&quest, &w.claim, &late)),
        RefusalCode::NotAdmitted
    );
    let record = record_from_summary(&summary(1.0, 7, "gpt-6-luna")).unwrap();
    let uncited = sign_at(
        &w.reproducer,
        AT + 10,
        run_evidence(
            w.reproducer.pubkey(),
            &w.claim.pubkey,
            &recipe(),
            &record,
            &[],
        )
        .unwrap(),
    );
    assert_eq!(
        code(check_reproduce(&quest, &w.claim, &uncited)),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_claim_the_quest_doesnt_pin_or_that_failed_is_refused() {
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    let other = claim(&signer("claimant"), AT - 4_000);
    assert_eq!(
        code(check_reproduce(&quest, &other, &w.reproduction)),
        RefusalCode::IdentityMismatch
    );
    // A quest that pins a failed attempt can't be completed.
    let claimant = signer("claimant");
    let record = record_from_summary(&summary(0.0, 8, "gpt-6-luna")).unwrap();
    let failed_claim = sign_at(
        &claimant,
        AT - 5_000,
        run_evidence(
            claimant.pubkey(),
            claimant.pubkey(),
            &recipe(),
            &record,
            &[],
        )
        .unwrap(),
    );
    let failed_quest = sign_at(
        &w.referee,
        AT,
        xp::quest(&quest_spec(&failed_claim)).unwrap(),
    );
    let parsed = parse_quest(&failed_quest).unwrap();
    let repro = reproduction(
        &w.reproducer,
        &failed_claim,
        &summary(1.0, 9, "gpt-6-luna"),
        AT + 10,
    );
    assert_eq!(
        code(check_reproduce(&parsed, &failed_claim, &repro)),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn a_reproduce_award_with_wrong_roles_or_xp_is_refused() {
    let w = world();
    let parts = reproduce_award(&w.quest, &w.claim, &w.reproduction, AT + 20).unwrap();
    let signed = sign_at(&w.referee, AT + 20, parts.clone());
    // XP that differs from the quest's table.
    let mut body: Value = serde_json::from_str(&signed.content).unwrap();
    body["awardees"][1]["xp"] = json!(60);
    let inflated = sign_at(
        &w.referee,
        AT + 20,
        Unsigned {
            content: body.to_string(),
            ..parts.clone()
        },
    );
    let award = parse_award(&inflated).unwrap();
    assert_eq!(
        code(xp::bind_quest(&award, &w.quest)),
        RefusalCode::Conflict
    );
    // Swapped roles name the wrong signers.
    let mut body: Value = serde_json::from_str(&signed.content).unwrap();
    body["awardees"][0]["pubkey"] = json!(w.reproducer.pubkey());
    body["awardees"][1]["pubkey"] = json!(w.claim.pubkey);
    let swapped = sign_at(
        &w.referee,
        AT + 20,
        Unsigned {
            content: body.to_string(),
            ..parts.clone()
        },
    );
    assert_eq!(code(parse_award(&swapped)), RefusalCode::IdentityMismatch);
    // A reproduce award can't bind to a kb-transfer rule's builder.
    assert!(xp::award(&w.quest, &w.claim, &w.reproduction, &[], AT + 20).is_err());
}
