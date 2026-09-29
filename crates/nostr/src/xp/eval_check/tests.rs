//! `eval-check` tests, mirroring the `reproduce` ones: the checker isn't
//! the evaluator or the suite author, the check comes after the result and
//! inside the season, a disputed check earns the same credit as a
//! confirming one while an inconclusive one earns nothing, each role is
//! paid once per suite version, and a key holding two roles collapses to
//! the larger. Keys are throwaway and derived from labels.

use serde_json::{Value, json};

use super::*;
use crate::eval_ext::tests::{
    AT, Spec, code, pubkey, published, request, requester, schema_check, sign_at, signer,
    subject_release, suite_release,
};
use crate::xp::{self, bind_quest, parse_award, parse_revocation, revocation};

pub(crate) fn season() -> Value {
    json!({"id": "s1", "opens_at": AT - 10_000, "closes_at": AT + 10_000})
}

fn quest_spec(award: Value) -> Value {
    json!({
        "id": "check-project-map",
        "version": 1,
        "season": season(),
        "title": "Check Project map's result",
        "objective": "Rerun the Project map test set and confirm the published result.",
        "acceptance": {
            "rule": "eval-check",
            "suite": suite_release(),
            "subject": subject_release(),
            "max_awards": 500,
        },
        "reference": null,
        "award": award,
    })
}

fn standard() -> Value {
    json!({"checker": 50, "evaluator": 25, "suite-author": 25})
}

fn quest_event(award: Value) -> Event {
    sign_at(
        &signer("referee"),
        AT - 9_000,
        xp::quest(&quest_spec(award)).unwrap(),
    )
}

struct World {
    quest: Event,
    result: Event,
    check: Event,
}

fn world() -> World {
    let result = published("alice", &Spec::by("alice"), None, AT);
    let check = published("bob", &Spec::by("bob"), Some(&result.id), AT + 10);
    World {
        quest: quest_event(standard()),
        result,
        check,
    }
}

fn parsed(quest: &Event) -> Quest {
    parse_quest(quest).unwrap()
}

#[test]
fn an_eval_check_quest_round_trips_and_matches_its_schema() {
    let w = world();
    let quest = parsed(&w.quest);
    assert_eq!(quest.acceptance.rule, EVAL_CHECK);
    let accepted = quest.acceptance.eval.as_ref().unwrap();
    assert_eq!(
        accepted.suite.as_ref().unwrap().pubkey,
        pubkey("suite-author")
    );
    assert_eq!(accepted.max_awards, Some(500));
    assert_eq!(quest.award_limit(), Some(500));
    assert_eq!(quest.total(), 100);
    assert!(
        w.quest
            .tag_values("t")
            .any(|t| t == "oa:xp:rule:eval-check")
    );
    schema_check(
        "xp-quest.v1.json",
        &serde_json::from_str(&w.quest.content).unwrap(),
    );
    // Other rules' fields and roles, and per-awardee, are refused.
    let mut spec = quest_spec(standard());
    spec["acceptance"]["task"] = json!("x");
    assert_eq!(code(xp::quest(&spec)), RefusalCode::UnsupportedFeature);
    let spec = quest_spec(json!({"claimant": 25, "reproducer": 25}));
    assert_eq!(code(xp::quest(&spec)), RefusalCode::UnsupportedFeature);
    let mut spec = quest_spec(standard());
    spec["completions"] = json!("per-awardee");
    spec["max_awards"] = json!(5);
    assert_eq!(code(xp::quest(&spec)), RefusalCode::UnsupportedFeature);
    let mut spec = quest_spec(standard());
    spec["acceptance"]["max_awards"] = json!(0);
    assert_eq!(code(xp::quest(&spec)), RefusalCode::Malformed);
}

#[test]
fn a_confirming_check_pays_each_role_once_in_its_own_award() {
    let w = world();
    let quest = parsed(&w.quest);
    let completion = check_eval_check(&quest, &w.result, &w.check, &[]).unwrap();
    let paid: Vec<(&str, String)> = completion
        .payees
        .iter()
        .map(|p| (p.role, p.pubkey.clone()))
        .collect();
    assert_eq!(
        paid,
        [
            ("checker", pubkey("bob")),
            ("evaluator", pubkey("alice")),
            ("suite-author", pubkey("suite-author")),
        ]
    );
    let awards = eval_check_awards(&w.quest, &w.result, &w.check, &[], AT + 20).unwrap();
    assert_eq!(awards.len(), 3);
    for parts in awards {
        let event = sign_at(&signer("referee"), AT + 20, parts);
        let award = parse_award(&event).unwrap();
        assert_eq!(award.rule, EVAL_CHECK);
        assert_eq!(award.awardees.len(), 1);
        assert_eq!(
            award.key,
            key(&quest, &award.awardees[0].role, &award.awardees[0].pubkey).unwrap()
        );
        bind_quest(&award, &w.quest).unwrap();
        bind_eval_check(&award, &quest, &w.result, &w.check, &[]).unwrap();
        schema_check(
            "xp-award.v1.json",
            &serde_json::from_str(&event.content).unwrap(),
        );
        // A revocation of it names its rule-derived key.
        let revoked = sign_at(
            &signer("referee"),
            AT + 30,
            revocation(&event, "the check was rerun on a patched runner").unwrap(),
        );
        assert_eq!(parse_revocation(&revoked).unwrap().key, award.key);
        // Bound to another check, it's refused.
        let other = published("dave", &Spec::by("dave"), Some(&w.result.id), AT + 11);
        assert_eq!(
            code(bind_eval_check(&award, &quest, &w.result, &other, &[])),
            RefusalCode::IdentityMismatch
        );
    }
}

#[test]
fn the_checker_is_neither_the_evaluator_nor_the_suite_author() {
    let w = world();
    let quest = parsed(&w.quest);
    let own = published("alice", &Spec::by("alice"), Some(&w.result.id), AT + 10);
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &own, &[])),
        RefusalCode::NotAdmitted
    );
    let author = published(
        "suite-author",
        &Spec::by("suite-author"),
        Some(&w.result.id),
        AT + 10,
    );
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &author, &[])),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn a_check_comes_after_the_result_and_inside_the_season() {
    let w = world();
    let quest = parsed(&w.quest);
    let early = published("bob", &Spec::by("bob"), Some(&w.result.id), AT - 5);
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &early, &[])),
        RefusalCode::NotAdmitted
    );
    let late = published("bob", &Spec::by("bob"), Some(&w.result.id), AT + 20_000);
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &late, &[])),
        RefusalCode::NotAdmitted
    );
    let before = published("alice", &Spec::by("alice"), None, AT - 20_000);
    let check = published("bob", &Spec::by("bob"), Some(&before.id), AT);
    assert_eq!(
        code(check_eval_check(&quest, &before, &check, &[])),
        RefusalCode::NotAdmitted
    );
    // An award accepted outside the season, or before the check, is refused.
    assert_eq!(
        code(eval_check_awards(
            &w.quest,
            &w.result,
            &w.check,
            &[],
            AT + 20_000
        )),
        RefusalCode::Stale
    );
    assert_eq!(
        code(eval_check_awards(
            &w.quest,
            &w.result,
            &w.check,
            &[],
            AT + 5
        )),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_disputed_check_earns_credit_and_an_inconclusive_one_earns_nothing() {
    let w = world();
    let quest = parsed(&w.quest);
    // Credit is for verification work, not agreement: Bob's dispute pays
    // the same three roles as his confirmation would, and confirms nothing.
    let mut fails = Spec::by("bob");
    fails.verdict = "fail";
    let dispute = published("bob", &fails, Some(&w.result.id), AT + 10);
    let completion = check_eval_check(&quest, &w.result, &dispute, &[]).unwrap();
    let roles: Vec<&str> = completion.payees.iter().map(|p| p.role).collect();
    assert_eq!(roles, ["checker", "evaluator", "suite-author"]);
    let (checker, _, linkage) = credited_check(&completion.result, &completion.check, &[]).unwrap();
    assert_eq!(checker, pubkey("bob"));
    assert_eq!(linkage, Linkage::Dispute);
    assert_eq!(
        code(confirmed_check(&completion.result, &completion.check, &[])),
        RefusalCode::NotAdmitted
    );
    assert_eq!(
        eval_check_awards(&w.quest, &w.result, &dispute, &[], AT + 100)
            .unwrap()
            .len(),
        3
    );
    // An inconclusive result or check has no verdict to verify.
    let mut unsure = Spec::by("alice");
    unsure.verdict = "inconclusive";
    let result = published("alice", &unsure, None, AT);
    unsure.evaluator = pubkey("bob");
    let check = published("bob", &unsure, Some(&result.id), AT + 10);
    assert_eq!(
        code(check_eval_check(&quest, &result, &check, &[])),
        RefusalCode::NotAdmitted
    );
    let mut unsure_check = Spec::by("bob");
    unsure_check.verdict = "inconclusive";
    let check = published("bob", &unsure_check, Some(&w.result.id), AT + 10);
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &check, &[])),
        RefusalCode::NotAdmitted
    );
    // Another subject lock isn't a check of the result.
    let mut locked = Spec::by("bob");
    locked.lock = "lock-b";
    let other = published("bob", &locked, Some(&w.result.id), AT + 10);
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &other, &[])),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn one_award_per_role_per_suite_version() {
    let w = world();
    let quest = parsed(&w.quest);
    let second = published("dave", &Spec::by("dave"), Some(&w.result.id), AT + 12);
    let first = check_eval_check(&quest, &w.result, &w.check, &[]).unwrap();
    let again = check_eval_check(&quest, &w.result, &second, &[]).unwrap();
    let keys = |c: &CheckCompletion| -> Vec<String> {
        c.payees
            .iter()
            .map(|p| key(&quest, p.role, &p.pubkey).unwrap())
            .collect()
    };
    let (first, again) = (keys(&first), keys(&again));
    // Two checkers, two checker keys; the evaluator and the suite author
    // have one key each, however many checks confirm the result.
    assert_ne!(first[0], again[0]);
    assert_eq!(first[1..], again[1..]);
    assert!(first[1].starts_with(&format!(
            "{KEY_PREFIX}s1:{}:evaluator:",
            quest
                .acceptance
                .eval
                .as_ref()
                .unwrap()
                .suite
                .as_ref()
                .unwrap()
                .id
        )));
}

#[test]
fn a_key_holding_two_roles_is_paid_once_in_the_larger() {
    // The suite's author published the result: evaluator and suite author
    // are one key. On a tie the earlier role (evaluator) wins.
    let result = published("suite-author", &Spec::by("suite-author"), None, AT);
    let check = published("bob", &Spec::by("bob"), Some(&result.id), AT + 10);
    let quest = parsed(&quest_event(standard()));
    let roles: Vec<&str> = check_eval_check(&quest, &result, &check, &[])
        .unwrap()
        .payees
        .iter()
        .map(|p| p.role)
        .collect();
    assert_eq!(roles, ["checker", "evaluator"]);
    let quest = parsed(&quest_event(
        json!({"checker": 50, "evaluator": 20, "suite-author": 30}),
    ));
    let roles: Vec<&str> = check_eval_check(&quest, &result, &check, &[])
        .unwrap()
        .payees
        .iter()
        .map(|p| p.role)
        .collect();
    assert_eq!(roles, ["checker", "suite-author"]);
}

#[test]
fn a_hosted_result_credits_the_trainer_who_asked() {
    let asked = request("carol", "runner", AT - 700);
    let rerun = request("dave", "runner", AT + 5);
    let mut spec = Spec::by("runner");
    spec.requester = Some(requester(&asked));
    let result = published("runner", &spec, None, AT);
    spec.requester = Some(requester(&rerun));
    let check = published("runner", &spec, Some(&result.id), AT + 10);
    let quest = parsed(&quest_event(standard()));
    let requests = [asked, rerun];
    let payees = check_eval_check(&quest, &result, &check, &requests)
        .unwrap()
        .payees;
    assert_eq!(payees[0].pubkey, pubkey("dave"));
    assert_eq!(payees[1].pubkey, pubkey("carol"));
    assert_eq!(
        code(check_eval_check(&quest, &result, &check, &requests[..1])),
        RefusalCode::ContentUnavailable
    );
}

#[test]
fn a_result_on_another_suite_or_subject_isnt_the_quests() {
    let w = world();
    let mut spec = quest_spec(standard());
    spec["acceptance"]["suite"]["id"] = json!(crate::eval_ext::tests::id("another suite"));
    let quest = parse_quest(&sign_at(
        &signer("referee"),
        AT - 9_000,
        xp::quest(&spec).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        code(check_eval_check(&quest, &w.result, &w.check, &[])),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn an_eval_award_credits_exactly_one_role_under_its_own_key() {
    let w = world();
    let parts = eval_check_awards(&w.quest, &w.result, &w.check, &[], AT + 20)
        .unwrap()
        .remove(0);
    let mut content: Value = serde_json::from_str(&parts.content).unwrap();
    content["awardees"][0]["pubkey"] = json!(pubkey("mallory"));
    let forged =
        signer("referee").sign(AT + 20, parts.kind, parts.tags.clone(), content.to_string());
    assert_eq!(code(parse_award(&forged)), RefusalCode::IdentityMismatch);
    let mut content: Value = serde_json::from_str(&parts.content).unwrap();
    let extra = content["awardees"][0].clone();
    content["awardees"].as_array_mut().unwrap().push(extra);
    let two = signer("referee").sign(AT + 20, parts.kind, parts.tags, content.to_string());
    assert_eq!(code(parse_award(&two)), RefusalCode::Malformed);
}
