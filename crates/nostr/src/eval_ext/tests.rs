//! Extension evaluation profile tests: case manifests, suites and their
//! packages, reports, result publications, and check linkage. Events are
//! signed with throwaway keys derived from a label; no key is stored.
//! The helpers are shared with the `eval-check` and `eval-adopt` tests.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;
use crate::domain::RelaySigner;

pub(crate) const AT: u64 = 1_790_000_000;

pub(crate) fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

pub(crate) fn pubkey(label: &str) -> String {
    signer(label).pubkey().to_string()
}

pub(crate) fn sign_at(signer: &RelaySigner, at: u64, parts: Unsigned) -> Event {
    signer.sign(at, parts.kind, parts.tags, parts.content)
}

/// A 64-hex ID from a label.
pub(crate) fn id(label: &str) -> String {
    Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn art(bytes: &[u8], media: &str, schema: Option<&str>) -> Value {
    let mut value = json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len(),
        "media_type": media,
    });
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}

pub(crate) fn code<T: std::fmt::Debug>(result: Result<T, ContractError>) -> RefusalCode {
    result.expect_err("refused").code
}

pub(crate) const PROMPT: &[u8] =
    b"+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-fire\"\n+++\n\nMap this repository.\n";
pub(crate) const GRADER: &[u8] =
    b"+++\ntype = \"decision\"\nquestion = \"Did it map the repository?\"\nthreshold = 0.7\n+++\n\nA map.\n";
pub(crate) const QUIET: &[u8] =
    b"+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-not-fire\"\n+++\n\nSay hello.\n";

pub(crate) fn case(id: &str, kind: &str, prompt: &[u8]) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "runs": 3,
        "prompt": art(prompt, "text/markdown", Some(CASE_SCHEMA)),
        "config": null,
        "graders": [{"name": "criteria", "artifact": art(GRADER, "text/markdown", Some(CASE_SCHEMA))}],
        "fixtures": [{"path": "src/main.rs", "artifact": art(b"fn main() {}\n", "text/plain", None)}],
    })
}

pub(crate) fn cases_bytes() -> Vec<u8> {
    case_manifest(&[
        case("map-repo", "should-fire", PROMPT),
        case("say-hello", "should-not-fire", QUIET),
    ])
    .expect("a valid case manifest")
}

pub(crate) fn gate() -> Value {
    json!({
        "id": format!("{}:gym/ext-eval-v1", pubkey("operator")),
        "artifact": art(b"{\"gate\":\"ext-eval-v1\"}", "application/json", None),
    })
}

pub(crate) fn suite_bytes() -> Vec<u8> {
    let small = |label: &str| art(label.as_bytes(), "application/json", None);
    jcs(&json!({
        "v": SUITE_SCHEMA,
        "requires": [],
        "id": format!("{}:project-map-tests/suite", pubkey("suite-author")),
        "purpose": "operation",
        "workload": small("workload"),
        "cases": art(&cases_bytes(), "application/json", Some(CASE_SCHEMA)),
        "partition": small("partition"),
        "labels": small("labels"),
        "metrics": small("metrics"),
        "acceptance": gate(),
        "environment": small("environment"),
    }))
    .unwrap()
}

/// The suite's release as the report names it.
pub(crate) fn suite_release() -> Value {
    json!({"id": id("suite-release"), "pubkey": pubkey("suite-author"), "kind": 3184})
}

pub(crate) fn subject_release() -> Value {
    json!({"id": id("subject-release"), "pubkey": pubkey("ext-author"), "kind": 3184})
}

pub(crate) fn subject_definition() -> Value {
    json!({
        "id": format!("{}:project-map/map", pubkey("ext-author")),
        "artifact": art(b"project map program", "application/json", None),
        "event": subject_release(),
    })
}

pub(crate) fn counts(completed: u64) -> Value {
    json!({"planned": 2, "attempted": 6, "completed": completed, "refused": 0,
           "failed": 6 - completed, "cancelled": 0, "unknown": 0, "excluded": 0})
}

/// How a report under test differs from the default.
#[derive(Clone)]
pub(crate) struct Spec {
    pub evaluator: String,
    pub verdict: &'static str,
    pub lock: &'static str,
    pub requester: Option<Value>,
    pub baseline: bool,
}

impl Spec {
    pub(crate) fn by(label: &str) -> Self {
        Spec {
            evaluator: pubkey(label),
            verdict: "pass",
            lock: "lock-a",
            requester: None,
            baseline: true,
        }
    }
}

pub(crate) fn report_value(spec: &Spec) -> Value {
    let mut suite = art(&suite_bytes(), "application/json", Some(SUITE_SCHEMA));
    suite["event"] = suite_release();
    let arm = |definition: Value, lock: &str| {
        json!({
            "definition": definition,
            "lock": art(lock.as_bytes(), "application/json", None),
            "configuration": art(b"config", "application/json", None),
        })
    };
    let baseline_def = json!({
        "id": format!("{}:coder-defaults/coder", pubkey("operator")),
        "artifact": art(b"coder", "application/json", None),
    });
    json!({
        "v": kb::REPORT_SCHEMA,
        "requires": [],
        "suite": suite,
        "partition": art(b"partition", "application/json", None),
        "subject": arm(subject_definition(), spec.lock),
        "baseline": if spec.baseline { arm(baseline_def, "lock-base") } else { Value::Null },
        "evaluator": spec.evaluator,
        "started_at": AT - 600,
        "ended_at": AT - 60,
        "runs": art(b"runs", "application/json", None),
        "coverage": {"subject": counts(5), "baseline": if spec.baseline { counts(4) } else { Value::Null }},
        "measurements": [
            {"arm": "subject", "metric": "cases_passed", "value": 2, "denominator": 2,
             "unknown_count": 0, "uncertainty": null, "evidence": []},
        ],
        "verdict": spec.verdict,
        "limitations": art(b"limitations", "text/plain", None),
        "meta": {"ext_eval": {
            "v": PROFILE_SCHEMA,
            "gate": digest_bytes(b"{\"gate\":\"ext-eval-v1\"}"),
            "cases": [{"id": "map-repo", "kind": "should-fire"}, {"id": "say-hello", "kind": "should-not-fire"}],
            "headline": {"subject_passed": 2, "baseline_passed": if spec.baseline { json!(1) } else { Value::Null }, "total": 2},
            "requester": spec.requester,
        }},
    })
}

pub(crate) fn report(spec: &Spec) -> String {
    report_value(spec).to_string()
}

/// A signed result publication for `spec`, signed by `label` at `at`.
pub(crate) fn published(label: &str, spec: &Spec, checks: Option<&str>, at: u64) -> Event {
    sign_at(
        &signer(label),
        at,
        publication(&report(spec), checks).expect("a valid publication"),
    )
}

/// A signed NIP-CJ execution request from `trainer` to `runner`.
pub(crate) fn request(trainer: &str, runner: &str, at: u64) -> Event {
    signer(trainer).sign(
        at,
        crate::kinds::CJ_EXECUTION_REQUEST,
        vec![tag(&["p", &pubkey(runner)])],
        "ciphertext".into(),
    )
}

pub(crate) fn requester(request: &Event) -> Value {
    json!({"id": request.id, "pubkey": request.pubkey, "kind": request.kind})
}

/// Re-signs `event` with `parts` changed by `edit`.
fn resign(
    label: &str,
    event: &Event,
    edit: impl FnOnce(&mut Vec<crate::domain::Tag>, &mut Value),
) -> Event {
    let mut tags = event.tags.clone();
    let mut content: Value = serde_json::from_str(&event.content).unwrap();
    edit(&mut tags, &mut content);
    signer(label).sign(event.created_at, event.kind, tags, content.to_string())
}

#[test]
fn a_case_manifest_round_trips_and_refuses_bad_entries() {
    let bytes = cases_bytes();
    let parsed = parse_case_manifest(&bytes).unwrap();
    assert_eq!(parsed.cases.len(), 2);
    assert_eq!(parsed.cases[0].id, "map-repo");
    assert_eq!(parsed.cases[1].kind, CaseKind::ShouldNotFire);
    assert_eq!(parsed.should_not_fire(), 1);
    assert_eq!(parsed.cases[0].files().count(), 3);
    assert_eq!(parsed.cases[0].graders[0].name, "criteria");

    // Out of order, duplicated, reserved, oversize, unknown kind.
    let good = || case("map-repo", "should-fire", PROMPT);
    assert_eq!(
        code(case_manifest(&[case("z", "should-fire", PROMPT), good()])),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(case_manifest(&[good(), good()])),
        RefusalCode::Conflict
    );
    assert_eq!(
        code(case_manifest(&[case("results", "should-fire", PROMPT)])),
        RefusalCode::Malformed
    );
    let mut big = good();
    big["prompt"]["size"] = json!(MAX_CASE_FILE_BYTES + 1);
    assert_eq!(code(case_manifest(&[big])), RefusalCode::LimitExceeded);
    let mut unknown = good();
    unknown["kind"] = json!("maybe-fire");
    assert_eq!(
        code(case_manifest(&[unknown])),
        RefusalCode::UnsupportedFeature
    );
    let mut runs = good();
    runs["runs"] = json!(11);
    assert_eq!(code(case_manifest(&[runs])), RefusalCode::Malformed);
    let mut no_graders = good();
    no_graders["graders"] = json!([]);
    assert_eq!(code(case_manifest(&[no_graders])), RefusalCode::Malformed);
    let mut escape = good();
    escape["fixtures"][0]["path"] = json!("../etc/passwd");
    assert_eq!(code(case_manifest(&[escape])), RefusalCode::Malformed);
    let mut wrong_schema = good();
    wrong_schema["prompt"]["schema"] = json!("openagents.other.v1");
    assert_eq!(
        code(case_manifest(&[wrong_schema])),
        RefusalCode::IdentityMismatch
    );
    assert_eq!(code(case_manifest(&[])), RefusalCode::Malformed);
}

#[test]
fn a_suite_parses_and_refuses_another_purpose_or_gate() {
    let suite = parse_suite(&suite_bytes()).unwrap();
    assert_eq!(suite.acceptance.id.rsplit('/').next(), Some(GATE));
    check_artifact_bytes(&suite.cases, &cases_bytes()).unwrap();

    let mut value: Value = serde_json::from_slice(&suite_bytes()).unwrap();
    value["purpose"] = json!("agent");
    assert_eq!(
        code(parse_suite(value.to_string().as_bytes())),
        RefusalCode::UnsupportedFeature
    );
    let mut value: Value = serde_json::from_slice(&suite_bytes()).unwrap();
    value["acceptance"]["id"] = json!(format!("{}:gym/other-gate", pubkey("operator")));
    assert_eq!(
        code(parse_suite(value.to_string().as_bytes())),
        RefusalCode::IdentityMismatch
    );
}

pub(crate) fn suite_manifest(extra_component: bool) -> Value {
    let cases = cases_bytes();
    let suite = suite_bytes();
    let file = |path: &str, bytes: &[u8], media: &str| json!({"path": path, "digest": digest_bytes(bytes), "size": bytes.len(), "media_type": media});
    let mut components = vec![json!({
        "slug": "suite",
        "kind": COMPONENT_KIND,
        "definition": art(&suite, "application/json", Some(SUITE_SCHEMA)),
    })];
    if extra_component {
        components.push(json!({
            "slug": "other",
            "kind": COMPONENT_KIND,
            "definition": art(&suite, "application/json", Some(SUITE_SCHEMA)),
        }));
    }
    json!({
        "v": "openagents.package.v1",
        "requires": [],
        "package": format!("{}:project-map-tests", pubkey("suite-author")),
        "version": "1.0.0",
        "license": "CC0-1.0",
        "provenance": {"source": "local", "receipts": [], "unknowns": []},
        "components": components,
        "files": [
            file("suite.json", &suite, "application/json"),
            file("evals/cases.json", &cases, "application/json"),
            file("evals/map-repo/prompt.md", PROMPT, "text/markdown"),
            file("evals/map-repo/graders/criteria.md", GRADER, "text/markdown"),
            file("evals/map-repo/fixtures/src/main.rs", b"fn main() {}\n", "text/plain"),
            file("evals/say-hello/prompt.md", QUIET, "text/markdown"),
        ],
        "dependencies": [],
    })
}

#[test]
fn a_suite_package_holds_one_eval_suite_and_lists_every_case_file() {
    let package = check_suite_package(&suite_manifest(false), &suite_bytes(), &cases_bytes())
        .expect("a valid suite package");
    assert_eq!(package.slug, "suite");
    assert_eq!(package.cases.cases.len(), 2);

    assert_eq!(
        code(check_suite_package(
            &suite_manifest(true),
            &suite_bytes(),
            &cases_bytes()
        )),
        RefusalCode::Malformed
    );
    let mut unlisted = suite_manifest(false);
    unlisted["files"].as_array_mut().unwrap().pop();
    assert_eq!(
        code(check_suite_package(
            &unlisted,
            &suite_bytes(),
            &cases_bytes()
        )),
        RefusalCode::ContentUnavailable
    );
    assert_eq!(
        code(check_suite_package(
            &suite_manifest(false),
            b"{}",
            &cases_bytes()
        )),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_result_publication_round_trips() {
    let event = published("alice", &Spec::by("alice"), None, AT);
    let parsed = parse_publication(&event).unwrap();
    assert_eq!(parsed.evaluator, pubkey("alice"));
    assert_eq!(parsed.trainer(), pubkey("alice"));
    assert_eq!(parsed.verdict(), Verdict::Pass);
    assert_eq!(parsed.suite_release.id, id("suite-release"));
    assert_eq!(parsed.suite_author(), pubkey("suite-author"));
    assert_eq!(
        parsed.subject_release.as_ref().map(|r| r.id.clone()),
        Some(id("subject-release"))
    );
    assert_eq!(parsed.checks, None);
    assert_eq!(parsed.report.profile.headline.baseline_passed, Some(1));
    assert_eq!(parsed.report.profile.cases[1].1, CaseKind::ShouldNotFire);
    let markers: Vec<&str> = event.tag_values("t").collect();
    assert_eq!(markers, [kb::EVAL_MARKER, PROFILE_MARKER]);
    // NIP-KB's and NIP-XP's readers don't mistake it for their evidence.
    assert!(kb::parse_evidence(&event).is_err());
    assert!(crate::xp::parse_run_evidence(&event).is_err());
}

#[test]
fn a_hosted_result_names_its_requester() {
    let asked = request("carol", "runner", AT - 700);
    let mut spec = Spec::by("runner");
    spec.requester = Some(requester(&asked));
    let event = published("runner", &spec, None, AT);
    let parsed = parse_publication(&event).unwrap();
    assert_eq!(parsed.trainer(), pubkey("carol"));
    assert_eq!(event.tag_values("p").collect::<Vec<_>>(), [pubkey("carol")]);
    check_request(&parsed, &asked).unwrap();
    assert_eq!(
        verified_trainer(&parsed, std::slice::from_ref(&asked)).unwrap(),
        pubkey("carol")
    );
    assert_eq!(
        code(verified_trainer(&parsed, &[])),
        RefusalCode::ContentUnavailable
    );
    // A request sent to another worker isn't this result's.
    let elsewhere = request("carol", "someone-else", AT - 700);
    let mut spec = Spec::by("runner");
    spec.requester = Some(requester(&elsewhere));
    let other = parse_publication(&published("runner", &spec, None, AT)).unwrap();
    assert_eq!(
        code(check_request(&other, &elsewhere)),
        RefusalCode::IdentityMismatch
    );
    // Dropping the p tag breaks the binding.
    let stripped = resign("runner", &event, |tags, _| {
        tags.retain(|t| t.name() != Some("p"))
    });
    assert_eq!(
        code(parse_publication(&stripped)),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_publication_refuses_a_wrong_marker() {
    let event = published("alice", &Spec::by("alice"), None, AT);
    let wrong = resign("alice", &event, |tags, _| {
        for t in tags.iter_mut() {
            if t.0[1] == PROFILE_MARKER {
                t.0[1] = "oa:xp:run:v1".into();
            }
        }
    });
    assert_eq!(
        code(parse_publication(&wrong)),
        RefusalCode::IdentityMismatch
    );
    let missing = resign("alice", &event, |tags, _| {
        tags.retain(|t| t.value() != Some(PROFILE_MARKER));
    });
    assert_eq!(
        code(parse_publication(&missing)),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_publication_refuses_a_digest_mismatch() {
    let event = published("alice", &Spec::by("alice"), None, AT);
    let tampered = resign("alice", &event, |_, content| {
        let text = content["meta"]["ext_eval_report"]
            .as_str()
            .unwrap()
            .to_string();
        content["meta"]["ext_eval_report"] = json!(text.replace("\"pass\"", "\"fail\""));
    });
    assert_eq!(
        code(parse_publication(&tampered)),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_publication_refuses_meta_over_64_kib() {
    let mut value = report_value(&Spec::by("alice"));
    value["measurements"][0]["evidence"] = json!(["x".repeat(MAX_REPORT_BYTES)]);
    let text = value.to_string();
    assert_eq!(code(publication(&text, None)), RefusalCode::LimitExceeded);
    // A hand-built event carrying the oversize bytes is refused as well.
    let event = published("alice", &Spec::by("alice"), None, AT);
    let oversize = resign("alice", &event, |tags, content| {
        let digest = digest_bytes(text.as_bytes());
        content["report"]["digest"] = json!(digest);
        content["report"]["size"] = json!(text.len());
        content["meta"]["ext_eval_report"] = json!(text);
        for t in tags.iter_mut() {
            if t.name() == Some("x") {
                t.0[1] = digest.trim_start_matches("sha256:").to_string();
            }
        }
    });
    assert_eq!(
        code(parse_publication(&oversize)),
        RefusalCode::LimitExceeded
    );
}

#[test]
fn a_publication_refuses_a_missing_suite_e_tag() {
    let event = published("alice", &Spec::by("alice"), None, AT);
    let missing = resign("alice", &event, |tags, _| {
        tags.retain(|t| t.0.get(3).map(String::as_str) != Some("suite"));
    });
    assert_eq!(code(parse_publication(&missing)), RefusalCode::Malformed);
    let other = resign("alice", &event, |tags, _| {
        for t in tags.iter_mut() {
            if t.0.get(3).map(String::as_str) == Some("suite") {
                t.0[1] = id("another suite");
            }
        }
    });
    assert_eq!(
        code(parse_publication(&other)),
        RefusalCode::IdentityMismatch
    );
    let unmarked = resign("alice", &event, |tags, _| {
        tags.push(tag(&["e", &id("loose")]));
    });
    assert_eq!(
        code(parse_publication(&unmarked)),
        RefusalCode::UnsupportedFeature
    );
    // A report whose suite has no release can't be published.
    let mut value = report_value(&Spec::by("alice"));
    value["suite"].as_object_mut().unwrap().remove("event");
    assert_eq!(
        code(publication(&value.to_string(), None)),
        RefusalCode::Malformed
    );
}

#[test]
fn a_publication_refuses_a_signer_who_isnt_the_evaluator() {
    let parts = publication(&report(&Spec::by("alice")), None).unwrap();
    let event = sign_at(&signer("mallory"), AT, parts);
    assert_eq!(
        code(parse_publication(&event)),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_report_without_a_baseline_is_inconclusive() {
    let mut spec = Spec::by("alice");
    spec.baseline = false;
    assert_eq!(
        code(parse_report(report(&spec).as_bytes())),
        RefusalCode::IdentityMismatch
    );
    spec.verdict = "inconclusive";
    let parsed = parse_report(report(&spec).as_bytes()).unwrap();
    assert!(parsed.baseline.is_none());
    assert_eq!(parsed.profile.headline.baseline_passed, None);
    // Coverage that doesn't sum is refused.
    let mut value = report_value(&Spec::by("alice"));
    value["coverage"]["subject"]["failed"] = json!(9);
    assert_eq!(
        code(parse_report(value.to_string().as_bytes())),
        RefusalCode::IdentityMismatch
    );
    // A headline over the case count is refused.
    let mut value = report_value(&Spec::by("alice"));
    value["meta"]["ext_eval"]["headline"]["subject_passed"] = json!(3);
    assert_eq!(
        code(parse_report(value.to_string().as_bytes())),
        RefusalCode::IdentityMismatch
    );
}

#[test]
fn a_check_confirms_or_disputes() {
    let original = published("alice", &Spec::by("alice"), None, AT);
    let check = published("bob", &Spec::by("bob"), Some(&original.id), AT + 10);
    assert_eq!(confirms(&original, &check).unwrap(), Linkage::Confirm);
    assert_eq!(
        parse_publication(&check).unwrap().checks.as_deref(),
        Some(original.id.as_str())
    );
    let mut fails = Spec::by("bob");
    fails.verdict = "fail";
    let dispute = published("bob", &fails, Some(&original.id), AT + 10);
    assert_eq!(confirms(&original, &dispute).unwrap(), Linkage::Dispute);
}

#[test]
fn a_check_with_another_subject_lock_is_not_a_check() {
    let original = published("alice", &Spec::by("alice"), None, AT);
    let mut spec = Spec::by("bob");
    spec.lock = "lock-b";
    let other = published("bob", &spec, Some(&original.id), AT + 10);
    assert_eq!(confirms(&original, &other).unwrap(), Linkage::NotACheck);
    // Nor is a result that doesn't cite the original.
    let uncited = published("bob", &Spec::by("bob"), None, AT + 10);
    assert_eq!(confirms(&original, &uncited).unwrap(), Linkage::NotACheck);
}

#[test]
fn a_self_check_is_not_a_check() {
    let original = published("alice", &Spec::by("alice"), None, AT);
    let again = published("alice", &Spec::by("alice"), Some(&original.id), AT + 10);
    assert_eq!(confirms(&original, &again).unwrap(), Linkage::NotACheck);
    // Two hosted runs by one runner are checks when different trainers
    // asked, and not when the same one did.
    let first = request("carol", "runner", AT - 700);
    let second = request("dave", "runner", AT - 5);
    let third = request("carol", "runner", AT - 4);
    let mut spec = Spec::by("runner");
    spec.requester = Some(requester(&first));
    let hosted = published("runner", &spec, None, AT);
    spec.requester = Some(requester(&second));
    let by_dave = published("runner", &spec, Some(&hosted.id), AT + 10);
    assert_eq!(confirms(&hosted, &by_dave).unwrap(), Linkage::Confirm);
    spec.requester = Some(requester(&third));
    let by_carol = published("runner", &spec, Some(&hosted.id), AT + 10);
    assert_eq!(confirms(&hosted, &by_carol).unwrap(), Linkage::NotACheck);
}

#[test]
fn hosted_sizes_stay_inside_the_runners_bounds() {
    check_hosted_size(8, 3, 2).unwrap();
    assert_eq!(code(check_hosted_size(9, 3, 2)), RefusalCode::LimitExceeded);
    assert_eq!(code(check_hosted_size(8, 4, 2)), RefusalCode::LimitExceeded);
    assert_eq!(code(check_hosted_size(0, 3, 2)), RefusalCode::Malformed);
}

#[test]
fn the_profile_record_round_trips_and_matches_its_schema() {
    let value = report_value(&Spec::by("alice"))["meta"]["ext_eval"].clone();
    let parsed = parse_profile(&value).unwrap();
    assert_eq!(profile_value(&parsed), value);
    schema_check("ext-eval.v1.json", &value);
    schema_check(
        "eval-case.v1.json",
        &serde_json::from_slice(&cases_bytes()).unwrap(),
    );
}

pub(crate) fn schema_check(name: &str, instance: &Value) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../nips/openagents/schemas")
        .join(name);
    let file = std::fs::read(&path).expect("schema");
    let digest = digest_bytes(&file);
    let mut documents = std::collections::BTreeMap::new();
    documents.insert(digest.clone(), file);
    let closure = crate::contracts::prepare_closure(&documents).expect("schema");
    crate::contracts::validate_instance(&closure, &digest, instance)
        .unwrap_or_else(|e| panic!("{name} refuses the instance: {e:?}"));
}
