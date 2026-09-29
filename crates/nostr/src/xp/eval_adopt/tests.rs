//! `eval-adopt` tests: a `coder-defaults` release that depends on the
//! extension and cites an admitting decision, whose reports include a
//! confirmed result, pays the extension's author, the suite's author, and
//! the confirmed result's evaluator, once each per subject release.

use serde_json::{Value, json};

use super::*;
use crate::contracts::digest_bytes;
use crate::eval_ext::Cites;
use crate::eval_ext::tests::{
    AT, Spec, art, code, id, pubkey, published, published_citing, schema_check,
    second_suite_release, sign_at, signer, subject_definition, subject_release,
};
use crate::xp::eval_check::tests::season;
use crate::xp::{self, bind_quest, parse_award};

fn defaults() -> String {
    format!("{}:coder-defaults", pubkey("operator"))
}

fn quest_event(award: Value) -> Event {
    let spec = json!({
        "id": "adopt-project-map",
        "version": 1,
        "season": season(),
        "title": "Coder adopts Project map",
        "objective": "Get a tool into Coder's defaults with a confirmed result.",
        "acceptance": {"rule": "eval-adopt", "defaults": defaults(), "subject": subject_release()},
        "reference": null,
        "award": award,
    });
    sign_at(&signer("referee"), AT - 9_000, xp::quest(&spec).unwrap())
}

fn standard() -> Value {
    json!({"extension-author": 200, "suite-author": 100, "evaluator": 50})
}

/// Carol's result on a second suite the validator wrote, externally
/// validating `result`.
fn validation_of(result: &Event) -> Event {
    let mut spec = Spec::by("carol");
    spec.suite = Some((second_suite_release("validator"), "validation-suite"));
    published_citing("carol", &spec, Some(Cites::Validates(&result.id)), AT + 20)
}

fn admission(
    result: &Event,
    validation: Option<&Event>,
    decision: &str,
    expires_at: u64,
) -> Vec<u8> {
    let report: Value = serde_json::from_str(&result.content).unwrap();
    let validation: Vec<Value> = validation
        .map(|v| {
            let content: Value = serde_json::from_str(&v.content).unwrap();
            content["report"].clone()
        })
        .into_iter()
        .collect();
    json!({
        "v": crate::eval_ext::ADMISSION_SCHEMA,
        "requires": [],
        "subject": subject_definition(),
        "reports": [report["report"]],
        "validation": validation,
        "stakes": {"severity": "low", "authority": "read", "reversibility": "reversible"},
        "policy": {
            "id": format!("{}:coder-defaults/policy", pubkey("operator")),
            "artifact": art(b"policy", "application/json", None),
        },
        "scope": art(b"everyone", "application/json", None),
        "decision": decision,
        "issuer": pubkey("operator"),
        "expires_at": expires_at,
    })
    .to_string()
    .into_bytes()
}

fn manifest(admission: &[u8], depends: bool) -> Vec<u8> {
    json!({
        "v": "openagents.package.v1",
        "requires": [],
        "package": defaults(),
        "version": "2026.10.1",
        "license": "CC0-1.0",
        "provenance": {
            "source": "local",
            "receipts": [art(admission, "application/json", Some(crate::eval_ext::ADMISSION_SCHEMA))],
            "unknowns": [],
        },
        "components": [],
        "files": [],
        "dependencies": if depends { json!([id("subject-release")]) } else { json!([]) },
    })
    .to_string()
    .into_bytes()
}

fn release(by: &str, manifest: &[u8], at: u64) -> Event {
    let body = json!({
        "v": 1, "requires": [], "type": "release",
        "package": format!("{}:coder-defaults", pubkey(by)),
        "version": "2026.10.1",
        "manifest": {"digest": digest_bytes(manifest), "size": manifest.len(), "media_type": "application/json"},
    });
    signer(by).sign(
        at,
        crate::kinds::EXT_RELEASE,
        vec![crate::kb::tag(&["t", "oa:ext:release:v1"])],
        body.to_string(),
    )
}

struct World {
    quest: Event,
    release: Event,
    manifest: Vec<u8>,
    admission: Vec<u8>,
    results: Vec<Event>,
    checks: Vec<Event>,
}

impl World {
    fn adoption(&self) -> Adoption<'_> {
        Adoption {
            release: &self.release,
            manifest: &self.manifest,
            admission: &self.admission,
            results: &self.results,
            checks: &self.checks,
            requests: &[],
        }
    }
}

fn world_with(evaluator: &str, decision: &str, depends: bool) -> World {
    let result = published(evaluator, &Spec::by(evaluator), None, AT);
    let check = published("bob", &Spec::by("bob"), Some(&result.id), AT + 10);
    let validation = validation_of(&result);
    let admission = admission(&result, Some(&validation), decision, AT + 5_000);
    let manifest = manifest(&admission, depends);
    World {
        quest: quest_event(standard()),
        release: release("operator", &manifest, AT + 100),
        manifest,
        admission,
        results: vec![result, validation],
        checks: vec![check],
    }
}

fn world() -> World {
    world_with("alice", "admit", true)
}

#[test]
fn an_eval_adopt_quest_round_trips_and_matches_its_schema() {
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    assert_eq!(quest.acceptance.rule, EVAL_ADOPT);
    let accepted = quest.acceptance.eval.as_ref().unwrap();
    assert_eq!(accepted.defaults.as_deref(), Some(defaults().as_str()));
    assert_eq!(quest.award_limit(), None);
    schema_check(
        "xp-quest.v1.json",
        &serde_json::from_str(&w.quest.content).unwrap(),
    );
}

#[test]
fn an_adoption_pays_the_authors_and_the_confirmed_evaluator() {
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    let completion = check_eval_adopt(&quest, &w.adoption()).unwrap();
    let paid: Vec<(&str, String)> = completion
        .payees
        .iter()
        .map(|p| (p.role, p.pubkey.clone()))
        .collect();
    assert_eq!(
        paid,
        [
            ("extension-author", pubkey("ext-author")),
            ("suite-author", pubkey("suite-author")),
            ("evaluator", pubkey("alice")),
        ]
    );
    let awards = eval_adopt_awards(&w.quest, &w.adoption(), AT + 200).unwrap();
    assert_eq!(awards.len(), 3);
    for parts in awards {
        let event = sign_at(&signer("referee"), AT + 200, parts);
        let award = parse_award(&event).unwrap();
        assert_eq!(award.rule, EVAL_ADOPT);
        assert!(
            award
                .key
                .starts_with(&format!("{KEY_PREFIX}{}:", id("subject-release")))
        );
        bind_quest(&award, &w.quest).unwrap();
        bind_eval_adopt(&award, &quest, &w.adoption()).unwrap();
        schema_check(
            "xp-award.v1.json",
            &serde_json::from_str(&event.content).unwrap(),
        );
    }
}

#[test]
fn an_admission_that_doesnt_admit_earns_nothing() {
    let w = world_with("alice", "reject", true);
    let quest = parse_quest(&w.quest).unwrap();
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn the_release_depends_on_the_extension_and_cites_the_admission() {
    let w = world_with("alice", "admit", false);
    let quest = parse_quest(&w.quest).unwrap();
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
    let mut w = world();
    w.admission = admission(&w.results[0], Some(&w.results[1]), "admit", AT + 6_000);
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::IdentityMismatch
    );
    // Signed by someone other than the package's root.
    let mut w = world();
    w.release = {
        let body = w.release.content.clone();
        signer("mallory").sign(
            w.release.created_at,
            w.release.kind,
            w.release.tags.clone(),
            body,
        )
    };
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::IdentityMismatch
    );
    // Published after the admission expired.
    let mut w = world();
    w.release = release("operator", &w.manifest, AT + 6_000);
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn an_adoption_needs_a_confirmed_result() {
    let mut w = world();
    let quest = parse_quest(&w.quest).unwrap();
    let mut fails = Spec::by("bob");
    fails.verdict = "fail";
    w.checks = vec![published("bob", &fails, Some(&w.results[0].id), AT + 10)];
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
    w.checks.clear();
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn an_adoption_needs_an_externally_validated_result() {
    // An admit that cites no validation doesn't parse as an admission.
    let w = world();
    let quest = parse_quest(&w.quest).unwrap();
    let bare = admission(&w.results[0], None, "admit", AT + 5_000);
    assert_eq!(
        code(crate::eval_ext::parse_admission(&bare)),
        RefusalCode::NotAdmitted
    );
    let mut without = world();
    without.admission = bare.clone();
    without.manifest = manifest(&bare, true);
    without.release = release("operator", &without.manifest, AT + 100);
    assert_eq!(
        code(check_eval_adopt(&quest, &without.adoption())),
        RefusalCode::NotAdmitted
    );
    // A second suite by the tool's own author is provenance, not
    // independence.
    let mut own = Spec::by("carol");
    own.suite = Some((second_suite_release("ext-author"), "own-suite"));
    let own = published_citing(
        "carol",
        &own,
        Some(Cites::Validates(&w.results[0].id)),
        AT + 20,
    );
    let mut w = world();
    w.admission = admission(&w.results[0], Some(&own), "admit", AT + 5_000);
    w.manifest = manifest(&w.admission, true);
    w.release = release("operator", &w.manifest, AT + 100);
    w.results = vec![w.results[0].clone(), own];
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
    // A validation that came out Worse validates nothing.
    let mut worse = Spec::by("carol");
    worse.suite = Some((second_suite_release("validator"), "validation-suite"));
    worse.verdict = "fail";
    let worse = published_citing(
        "carol",
        &worse,
        Some(Cites::Validates(&w.results[0].id)),
        AT + 20,
    );
    let mut w = world();
    w.admission = admission(&w.results[0], Some(&worse), "admit", AT + 5_000);
    w.manifest = manifest(&w.admission, true);
    w.release = release("operator", &w.manifest, AT + 100);
    w.results = vec![w.results[0].clone(), worse];
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
    // A reject needs no validation.
    let reject = admission(&world().results[0], None, "reject", AT + 5_000);
    assert!(crate::eval_ext::parse_admission(&reject).is_ok());
}

#[test]
fn an_unresolved_subject_is_never_adopted() {
    // Reproducibility cannot be stronger than identity, and neither can
    // adoption: a result that records its subject as unresolved is
    // refused however many checks confirmed it.
    let mut spec = Spec::by("alice");
    spec.identity = Some("unresolved");
    let result = published("alice", &spec, None, AT);
    let check = published("bob", &Spec::by("bob"), Some(&result.id), AT + 10);
    let validation = validation_of(&result);
    let decision = admission(&result, Some(&validation), "admit", AT + 5_000);
    let manifest = manifest(&decision, true);
    let w = World {
        quest: quest_event(standard()),
        release: release("operator", &manifest, AT + 100),
        manifest,
        admission: decision,
        results: vec![result, validation],
        checks: vec![check],
    };
    let quest = parse_quest(&w.quest).unwrap();
    assert_eq!(
        code(check_eval_adopt(&quest, &w.adoption())),
        RefusalCode::NotAdmitted
    );
    // A content-addressed subject with the same evidence is adopted.
    let mut spec = Spec::by("alice");
    spec.identity = Some("content");
    let result = published("alice", &spec, None, AT);
    let check = published("bob", &Spec::by("bob"), Some(&result.id), AT + 10);
    let validation = validation_of(&result);
    let decision = admission(&result, Some(&validation), "admit", AT + 5_000);
    let bytes = self::manifest(&decision, true);
    let w = World {
        quest: w.quest.clone(),
        release: release("operator", &bytes, AT + 100),
        manifest: bytes,
        admission: decision,
        results: vec![result, validation],
        checks: vec![check],
    };
    assert!(check_eval_adopt(&quest, &w.adoption()).is_ok());
}

#[test]
fn an_author_who_also_evaluated_is_paid_once_in_the_larger_role() {
    let w = world_with("ext-author", "admit", true);
    let quest = parse_quest(&w.quest).unwrap();
    let roles: Vec<(&str, String)> = check_eval_adopt(&quest, &w.adoption())
        .unwrap()
        .payees
        .iter()
        .map(|p| (p.role, p.pubkey.clone()))
        .collect();
    assert_eq!(
        roles,
        [
            ("extension-author", pubkey("ext-author")),
            ("suite-author", pubkey("suite-author")),
        ]
    );
}
