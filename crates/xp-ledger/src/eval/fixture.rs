//! Signed extension evaluation fixtures for tests: suite and tool
//! releases, results, checks, and hosted requests, signed with throwaway
//! keys derived from labels. Nothing here is stored or published.

use nostr::contracts::digest_bytes;
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::eval_ext;
use nostr::{ext, kb, kinds};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// A throwaway signer derived from `label`.
///
/// # Panics
///
/// Never: every SHA-256 digest but zero is a valid key.
#[must_use]
pub fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

/// The public key of [`signer`]`(label)`.
#[must_use]
pub fn pubkey(label: &str) -> String {
    signer(label).pubkey().to_string()
}

fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|v| (*v).to_owned()).collect())
}

fn art(bytes: &[u8], media: &str, schema: Option<&str>) -> Value {
    let mut value =
        json!({"digest": digest_bytes(bytes), "size": bytes.len(), "media_type": media});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}

/// A NIP-EXT release of `<by>:<slug>` at `at`. Its manifest is left to
/// the caller; the release only pins its digest.
#[must_use]
pub fn release(by: &RelaySigner, slug: &str, at: u64) -> Event {
    let body = json!({
        "v": 1, "requires": [], "type": "release",
        "package": format!("{}:{slug}", by.pubkey()),
        "version": "1",
        "manifest": art(format!("{slug} manifest").as_bytes(), "application/json", None),
    });
    by.sign(
        at,
        ext::RELEASE_KIND,
        vec![tag(&["t", "oa:ext:release:v1"])],
        body.to_string(),
    )
}

fn pointer(event: &Event) -> Value {
    json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind})
}

/// What one result or check ran and found.
#[derive(Clone)]
pub struct Run<'a> {
    /// The suite's release.
    pub suite: &'a Event,
    /// The tool's release.
    pub subject: &'a Event,
    /// The subject arm's lock; a check matches its result's.
    pub lock: &'a str,
    /// `pass` (Better), `fail` (Worse), or `inconclusive`.
    pub verdict: &'a str,
    /// A hosted run's signed request.
    pub request: Option<&'a Event>,
}

impl<'a> Run<'a> {
    /// A Better run of `subject` on `suite`.
    #[must_use]
    pub fn better(suite: &'a Event, subject: &'a Event) -> Self {
        Run {
            suite,
            subject,
            lock: "lock-a",
            verdict: "pass",
            request: None,
        }
    }
}

/// The subject DefinitionRef a run names.
#[must_use]
pub fn definition(subject: &Event) -> Value {
    json!({
        "id": format!("{}:tool/main", subject.pubkey),
        "artifact": art(format!("tool {}", subject.id).as_bytes(), "application/json", None),
        "event": pointer(subject),
    })
}

/// The report `evaluator` signs for `run`.
#[must_use]
pub fn report(evaluator: &str, run: &Run<'_>) -> String {
    let mut suite = art(
        format!("suite {}", run.suite.id).as_bytes(),
        "application/json",
        Some(eval_ext::SUITE_SCHEMA),
    );
    suite["event"] = pointer(run.suite);
    let arm = |definition: Value, lock: &str| {
        json!({
            "definition": definition,
            "lock": art(lock.as_bytes(), "application/json", None),
            "configuration": art(b"config", "application/json", None),
        })
    };
    let counts = |completed: u64| {
        json!({"planned": 2, "attempted": 6, "completed": completed, "refused": 0,
               "failed": 6 - completed, "cancelled": 0, "unknown": 0, "excluded": 0})
    };
    json!({
        "v": kb::REPORT_SCHEMA,
        "requires": [],
        "suite": suite,
        "partition": art(b"partition", "application/json", None),
        "subject": arm(definition(run.subject), run.lock),
        "baseline": arm(json!({
            "id": format!("{}:coder-defaults/coder", run.suite.pubkey),
            "artifact": art(b"coder", "application/json", None),
        }), "lock-base"),
        "evaluator": evaluator,
        "started_at": 1_000,
        "ended_at": 2_000,
        "runs": art(b"runs", "application/json", None),
        "coverage": {"subject": counts(5), "baseline": counts(4)},
        "measurements": [
            {"arm": "subject", "metric": "cases_passed", "value": 2, "denominator": 2,
             "unknown_count": 0, "uncertainty": null, "evidence": []},
        ],
        "verdict": run.verdict,
        "limitations": art(b"limitations", "text/plain", None),
        "meta": {"ext_eval": {
            "v": eval_ext::PROFILE_SCHEMA,
            "gate": digest_bytes(b"{\"gate\":\"ext-eval-v1\"}"),
            "cases": [{"id": "map-repo", "kind": "should-fire"}, {"id": "say-hello", "kind": "should-not-fire"}],
            "headline": {"subject_passed": 2, "baseline_passed": 1, "total": 2},
            "requester": run.request.map(pointer),
        }},
    })
    .to_string()
}

/// A result publication `by` signs at `at`; with `checks`, a check of
/// that result.
///
/// # Panics
///
/// When the fixture isn't a valid publication, which a test would catch.
#[must_use]
pub fn published(by: &RelaySigner, run: &Run<'_>, checks: Option<&str>, at: u64) -> Event {
    let parts = eval_ext::publication(&report(by.pubkey(), run), checks).expect("a valid result");
    by.sign(at, parts.kind, parts.tags, parts.content)
}

/// A signed NIP-CJ execution request from `trainer` to the hosted
/// runner `runner`.
#[must_use]
pub fn request(trainer: &RelaySigner, runner: &str, at: u64) -> Event {
    trainer.sign(
        at,
        kinds::CJ_EXECUTION_REQUEST,
        vec![tag(&["p", runner])],
        "ciphertext".into(),
    )
}
