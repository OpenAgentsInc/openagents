//! Labeled fixtures for the EVALS board and the agents' notes: NIP-EXT
//! releases and extension eval result publications signed with the keys a
//! test passes in. Tests, the relay integration test, and the `gym_peer`
//! example use them against a relay on this machine; the app never shows
//! them as real results.

use nostr::contracts::digest_bytes;
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::eval_ext;
use serde_json::{Value, json};

fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|v| (*v).to_owned()).collect())
}

fn artifact(label: &str, media: &str, schema: Option<&str>) -> Value {
    let mut value = json!({
        "digest": digest_bytes(label.as_bytes()),
        "size": label.len(),
        "media_type": media,
    });
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}

/// A signed NIP-EXT release of `slug` at `version`, by `root`.
#[must_use]
pub fn release(root: &RelaySigner, slug: &str, version: &str, at: u64) -> Event {
    let body = json!({
        "v": 1,
        "requires": [],
        "type": "release",
        "package": format!("{}:{slug}", root.pubkey()),
        "version": version,
        "manifest": artifact(&format!("{slug}@{version} manifest"), "application/json", None),
    });
    root.sign(
        at,
        nostr::kinds::EXT_RELEASE,
        vec![tag(&["t", "oa:ext:release:v1"])],
        body.to_string(),
    )
}

/// What one fixture result says.
#[derive(Clone, Debug)]
pub struct Spec<'a> {
    /// The test set's release.
    pub suite: &'a Event,
    /// The tool's release; its slug is the tool's package and component.
    pub tool: &'a Event,
    /// Cases passed with the tool.
    pub with: u64,
    /// Cases passed without it; `None` runs the subject arm alone.
    pub without: Option<u64>,
    /// Cases in the test set, 1 to 8.
    pub total: u64,
    /// The subject arm's lock label: two results with the same lock can
    /// check each other.
    pub lock: &'a str,
    /// The result this one checks.
    pub checks: Option<&'a str>,
}

impl Spec<'_> {
    fn verdict(&self) -> &'static str {
        match self.without {
            Some(without) if self.with > without => "pass",
            Some(without) if self.with < without => "fail",
            _ => "inconclusive",
        }
    }
}

fn slug_of(release: &Event) -> String {
    let body: Value = serde_json::from_str(&release.content).unwrap_or_default();
    body["package"]
        .as_str()
        .and_then(|p| p.split_once(':'))
        .map(|(_, slug)| slug.to_owned())
        .unwrap_or_default()
}

/// A signed result publication for `spec`, evaluated and signed by
/// `trainer`.
///
/// # Panics
///
/// When `spec` makes an invalid report (for example, more passes than
/// cases), so a fixture mistake fails loudly.
#[must_use]
pub fn result(trainer: &RelaySigner, spec: &Spec<'_>, at: u64) -> Event {
    let arm = |definition: Value, lock: &str| {
        json!({
            "definition": definition,
            "lock": artifact(lock, "application/json", None),
            "configuration": artifact("configuration", "application/json", None),
        })
    };
    let tool_slug = slug_of(spec.tool);
    let subject = json!({
        "id": format!("{}:{tool_slug}/{tool_slug}", spec.tool.pubkey),
        "artifact": artifact(&format!("{tool_slug} definition"), "application/json", None),
        "event": {"id": spec.tool.id, "pubkey": spec.tool.pubkey, "kind": spec.tool.kind},
    });
    let baseline = json!({
        "id": format!("{}:coder-defaults/coder", spec.suite.pubkey),
        "artifact": artifact("coder defaults", "application/json", None),
    });
    let mut suite = artifact(
        &format!("suite {}", spec.suite.id),
        "application/json",
        Some(eval_ext::SUITE_SCHEMA),
    );
    suite["event"] =
        json!({"id": spec.suite.id, "pubkey": spec.suite.pubkey, "kind": spec.suite.kind});
    let cases: Vec<Value> = (1..=spec.total)
        .map(|n| json!({"id": format!("case-{n}"), "kind": "should-fire"}))
        .collect();
    let coverage = |completed: u64| {
        json!({"planned": spec.total, "attempted": spec.total, "completed": completed,
               "refused": 0, "failed": spec.total - completed, "cancelled": 0,
               "unknown": 0, "excluded": 0})
    };
    let report = json!({
        "v": nostr::kb::REPORT_SCHEMA,
        "requires": [],
        "suite": suite,
        "partition": artifact("partition", "application/json", None),
        "subject": arm(subject, spec.lock),
        "baseline": if spec.without.is_some() { arm(baseline, "coder-defaults-lock") } else { Value::Null },
        "evaluator": trainer.pubkey(),
        "started_at": at.saturating_sub(600),
        "ended_at": at.saturating_sub(60),
        "runs": artifact("runs", "application/json", None),
        "coverage": {
            "subject": coverage(spec.with),
            "baseline": spec.without.map_or(Value::Null, coverage),
        },
        "measurements": [{"arm": "subject", "metric": "cases_passed", "value": spec.with,
                          "denominator": spec.total, "unknown_count": 0,
                          "uncertainty": null, "evidence": []}],
        "verdict": spec.verdict(),
        "limitations": artifact("limitations", "text/plain", None),
        "meta": {"ext_eval": {
            "v": eval_ext::PROFILE_SCHEMA,
            "gate": digest_bytes(b"ext-eval-v1 fixture gate"),
            "cases": cases,
            "headline": {"subject_passed": spec.with, "baseline_passed": spec.without, "total": spec.total},
            "requester": null,
        }},
    });
    let parts =
        eval_ext::publication(&report.to_string(), spec.checks).expect("a valid fixture report");
    trainer.sign(at, parts.kind, parts.tags, parts.content)
}
