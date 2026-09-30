//! The chat router as a capability claim (#9959).
//!
//! The essay (`docs/essays/2026-09-29-test-time-capabilities.md`) says
//! every capability, our own router included, is a claim: an evidence
//! record on a claim key, judged by a written policy. Here the record is
//! an `openagents.eval-report.v1` under NIP-EVAL's extension profile
//! (`nips/openagents/NIP-EVAL.md`, *Reports*) whose subject is the router
//! as a decision service: its `configuration` pins the question set as
//! [`crate::router::set_id`] and the bank as [`crate::router::Bank::id`],
//! its suite is the labeled route set as the Gym holds it
//! (`crates/gym/suites/chat-router-v4.json`), its partition is the locked
//! (held-out) rows, and its policy is the `router-v1` gate
//! (`crates/gym/gates/router-v1.json`). The measurements are the ones
//! NIP-EVAL asks of a "better semantic decisions" claim: per-route
//! precision and recall, canned and dispatch precision, the abstention
//! rate, ECE and Brier per question raw and calibrated, and the
//! operating-point table (precision, coverage, and abstention at each
//! threshold) the risk–coverage curve lies on.
//!
//! [`Claim::record`] builds the report and every artifact it references,
//! all from ids, labels, probabilities, tiers, and timings: no message
//! text reaches a record. [`Record::write`] puts them in a directory as
//! `report.json` and the files beside it; the live eval
//! (`crates/coder/tests/router_eval.rs`, `ROUTER_EVAL_PUBLISH=1`) writes
//! `target/router-eval/<date>/`. The bytes pass
//! `nostr::eval_ext::parse_report`, so they can be published as a `3189`
//! once the suite has a release to cite; today they are committed as a
//! measurement (`docs/coder/measurements/2026-09-29-chat-router-claims.md`).
//!
//! The first record has no baseline (the previous router version was not
//! kept runnable), so its verdict is `inconclusive` as NIP-EVAL requires
//! of a report that can claim no change; the gate's floors are still
//! judged and written into the limitations.

use std::collections::BTreeMap;
use std::path::Path;

use ext_eval::artifact::{ArtifactRef, JSON, json_bytes};
use ext_eval::report::{
    LIMITATIONS_SCHEMA, PARTITION_SCHEMA, PROFILE_SCHEMA, REPORT_SCHEMA, SUITE_SCHEMA, wire_gate,
};
use gym::gate::{Gate, RouterComparison, RouterScores};
use serde_json::{Value, json};

use crate::router::calibration::Calibration;
use crate::router::{Bank, SET, set_digest, set_id};
use crate::router_eval::{Reading, Report, Row, Set, observations, operating_points, partition_of};

/// The gate a router record is judged by.
pub const GATE: &str = "router-v1";

/// The task distribution the suite claims to sample: first messages to
/// the OpenAgents chat, as the design's route catalog frames them.
pub const DISTRIBUTION: &str = "chat-router/first-messages";

/// The schema of the runs artifact.
pub const RUNS_SCHEMA: &str = "openagents.chat-router.runs.v1";
/// The schema of the readings artifact the runs cite.
pub const READINGS_SCHEMA: &str = "openagents.chat-router.readings.v1";
/// The schema of the subject's definition artifact.
pub const DEFINITION_SCHEMA: &str = "openagents.chat-router.decision-service.v1";
/// The schema of the subject's lock artifact.
pub const LOCK_SCHEMA: &str = "openagents.chat-router.lock.v1";
/// The schema of the subject's configuration artifact.
pub const CONFIGURATION_SCHEMA: &str = "openagents.chat-router.configuration.v1";
/// The schema of the suite's workload, labels, metrics, and environment
/// artifacts.
pub const WORKLOAD_SCHEMA: &str = "openagents.chat-router.workload.v1";
pub const LABELS_SCHEMA: &str = "openagents.chat-router.labels.v1";
pub const METRICS_SCHEMA: &str = "openagents.chat-router.metrics.v1";
pub const ENVIRONMENT_SCHEMA: &str = "openagents.chat-router.environment.v1";
/// The Gym suite file's schema, which the suite's `cases` artifact is.
pub const GYM_SUITE_SCHEMA: &str = "openagents.gym.suite.v1";

/// Everything a record is built from. Nothing here is message text.
pub struct Claim<'a> {
    /// The labeled set, for the partition.
    pub set: &'a Set,
    /// The committed Gym suite generated from it, byte for byte.
    pub suite_bytes: &'a [u8],
    /// The gate file, byte for byte.
    pub gate_bytes: &'a [u8],
    /// The held-out rows the report is on.
    pub rows: &'a [&'a Row],
    /// The router's reading of each.
    pub readings: &'a [Reading],
    /// The readings scored against the rows.
    pub report: &'a Report,
    /// The calibration fitted on the calibration partition and scored on
    /// these rows.
    pub calibration: &'a Calibration,
    /// The `router-v1` gate.
    pub gate: &'a Gate,
    pub bank: &'a Bank,
    /// The decision door, as a name (`typesafe:jev-latest`), never a key.
    pub judge: &'a str,
    /// The worker build, `coder@<version>`.
    pub worker: &'a str,
    /// The commit the eval ran at, when known.
    pub commit: Option<&'a str>,
    pub started_at: u64,
    pub ended_at: u64,
}

/// A built record: the report and the files it references, by name.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub report: Value,
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Record {
    /// The report's exact bytes: what a `3189` would carry. Compact JSON
    /// with a trailing newline, since NIP-EVAL bounds a report at 64 KiB
    /// and a held-out split of two hundred rows with its operating points
    /// runs past that pretty-printed.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&self.report).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    /// Writes `report.json` and every file into `dir`.
    ///
    /// # Errors
    ///
    /// The I/O error.
    pub fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("report.json"), self.bytes())?;
        for (name, bytes) in &self.files {
            std::fs::write(dir.join(name), bytes)?;
        }
        Ok(())
    }
}

/// A row's case id: its id with `/` as `-`, the shape NIP-EVAL's case ids
/// take.
#[must_use]
pub fn case_id(row_id: &str) -> String {
    row_id.replace('/', "-")
}

/// A row's case kind: `should-fire` when a correct router acts on its own
/// reading (a prepared answer, a stem, an offer, a refusal, a Gym reply,
/// an interview step, a grounded reply), `should-not-fire` when the model
/// answers alone.
#[must_use]
pub fn case_kind(row: &Row) -> &'static str {
    if row.tier == "model" {
        "should-not-fire"
    } else {
        "should-fire"
    }
}

/// Whether the router passed a row: the route reading is the label, no
/// harmful error was made (a wrong whole answer, an offer on another
/// route's row, a Gym tier on another route's row), a row that expects a
/// prepared answer whole got the right one, and a row the model should
/// answer alone got the model alone.
#[must_use]
pub fn passes(row: &Row, reading: &Reading) -> bool {
    if reading.error.is_some() || reading.route.as_deref() != Some(row.route.as_str()) {
        return false;
    }
    let served = reading.answer.as_deref().unwrap_or_default();
    let canned_right = row.tier == "canned" && row.accepts(served);
    if reading.tier == "canned" && !canned_right {
        return false;
    }
    if reading.dispatch && row.route != "work.dispatch" {
        return false;
    }
    if row.tier == "canned" && !(reading.tier == "canned" && canned_right) {
        return false;
    }
    if row.tier == "model" && reading.tier != "model" {
        return false;
    }
    true
}

fn evaluator() -> String {
    nostr::contracts::digest_bytes(b"openagents coder router_eval")
        .trim_start_matches("sha256:")
        .to_string()
}

fn put(
    files: &mut BTreeMap<String, Vec<u8>>,
    name: &str,
    value: &Value,
    schema: &str,
) -> ArtifactRef {
    let bytes = json_bytes(value);
    let reference = ArtifactRef::of(&bytes, JSON, Some(schema));
    files.insert(name.to_string(), bytes);
    reference
}

fn measurement(
    metric: String,
    value: Option<f64>,
    denominator: usize,
    unknown: usize,
    evidence: &[Value],
) -> Value {
    json!({
        "arm": "subject",
        "metric": metric,
        "value": value.filter(|v| v.is_finite()),
        "denominator": denominator,
        "unknown_count": unknown,
        "uncertainty": null,
        "evidence": evidence,
    })
}

#[allow(clippy::cast_precision_loss)]
fn count(n: usize) -> Option<f64> {
    Some(n as f64)
}

impl Claim<'_> {
    /// The gate's reading of the report's numbers: the subject arm alone.
    #[must_use]
    pub fn comparison(&self) -> RouterComparison {
        RouterComparison {
            group: format!("{SET}/{}", self.report.split),
            subject: RouterScores {
                items: self.report.read(),
                canned_precision: self.report.canned.precision(),
                canned_recall: self.report.canned.recall(),
                dispatch_precision: self.report.dispatch.precision(),
                route_accuracy: self.report.route_accuracy,
                ece: Some(self.calibration.route.raw.ece),
            },
            baseline: None,
        }
    }

    /// The report and its artifacts.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn record(&self) -> Record {
        let mut files = BTreeMap::new();
        let evaluator = evaluator();
        let report = self.report;

        // The subject: the router as a decision service.
        let definition = put(
            &mut files,
            "definition.json",
            &json!({
                "v": DEFINITION_SCHEMA,
                "requires": [],
                "id": "chat-router/decide",
                "summary": "The chat router: one System One request (the route, answer, needs_specifics, lane, opener, cli_group, tool, and risk questions) and the policy table that turns its readings into a tier.",
                "questions": ["route", "answer", "needs_specifics", "lane", "opener", "cli_group", "tool", "risk"],
                "policy": "crates/coder/src/router/policy.rs",
                "design": "docs/coder/design/2026-09-28-chat-router.md",
            }),
            DEFINITION_SCHEMA,
        );
        let lock = put(
            &mut files,
            "lock.json",
            &json!({
                "v": LOCK_SCHEMA,
                "requires": [],
                "worker": self.worker,
                "commit": self.commit,
                "judge": self.judge,
                "set": set_id(),
                "bank": self.bank.id(),
            }),
            LOCK_SCHEMA,
        );
        let configuration = put(
            &mut files,
            "configuration.json",
            &json!({
                "v": CONFIGURATION_SCHEMA,
                "requires": [],
                "set": set_id(),
                "set_digest": format!("sha256:{}", set_digest()),
                "bank": self.bank.id(),
                "mode": "router",
                "calibration": "off",
                "thresholds": {
                    "route_confidence": crate::router::policy::ROUTE_CONFIDENCE,
                    "answer_confidence": crate::router::policy::ANSWER_CONFIDENCE,
                    "specifics_ceiling": crate::router::policy::SPECIFICS_CEILING,
                    "stem_confidence": crate::router::policy::STEM_CONFIDENCE,
                    "opener_confidence": crate::router::policy::OPENER_CONFIDENCE,
                    "dispatch_route": crate::router::policy::DISPATCH_ROUTE,
                    "dispatch_lane": crate::router::policy::DISPATCH_LANE,
                    "cli_route": crate::router::policy::CLI_ROUTE,
                    "cli_group": crate::router::policy::CLI_GROUP,
                    "risk_warn": crate::router::policy::RISK_WARN,
                    "risk_refuse": crate::router::policy::RISK_REFUSE,
                    "grounded_route": crate::router::policy::GROUNDED_ROUTE,
                    "clarify_route": crate::router::policy::CLARIFY_ROUTE,
                    "eval_route": crate::router::policy::EVAL_ROUTE,
                    "tool_confidence": crate::router::policy::TOOL_CONFIDENCE,
                },
            }),
            CONFIGURATION_SCHEMA,
        );

        // The suite: the labeled set as the Gym holds it.
        let cases = ArtifactRef::of(self.suite_bytes, JSON, Some(GYM_SUITE_SCHEMA));
        files.insert("cases.json".to_string(), self.suite_bytes.to_vec());
        let gate_file = ArtifactRef::of(self.gate_bytes, JSON, Some(gym::gate::SCHEMA));
        files.insert("gate.json".to_string(), self.gate_bytes.to_vec());
        let ids = |partition: &str| -> Vec<String> {
            self.set
                .rows
                .iter()
                .filter(|row| partition_of(row) == partition)
                .map(|row| case_id(&row.id))
                .collect()
        };
        let partition = put(
            &mut files,
            "partition.json",
            &json!({
                "v": PARTITION_SCHEMA,
                "requires": [],
                "development": ids("development"),
                "calibration": ids("calibration"),
                "held_out": ids("locked"),
                "excluded": [],
                "tuning_access": "thresholds and rubric text were tuned on the development and calibration rows; the calibration map was fitted on the calibration rows; the held-out rows were read once per published run and never tuned on",
            }),
            PARTITION_SCHEMA,
        );
        let mut strata: BTreeMap<String, usize> = BTreeMap::new();
        for row in &self.set.rows {
            *strata.entry(row.route.clone()).or_default() += 1;
        }
        let workload = put(
            &mut files,
            "workload.json",
            &json!({
                "v": WORKLOAD_SCHEMA,
                "requires": [],
                "distribution": DISTRIBUTION,
                "frame": "first messages a person sends the OpenAgents chat, as the route catalog of docs/coder/design/2026-09-28-chat-router.md frames them, plus the owner-reported messages and multi-turn interview replies",
                "inclusion": "every row names a route of the catalog, a tier, a risk, and the prepared answer a correct router serves or none",
                "exclusions": "messages with a secret in them are written as placeholders, never real credentials; no production traffic was mirrored",
                "strata": strata,
                "method": "constructed",
                "synthetic": true,
            }),
            WORKLOAD_SCHEMA,
        );
        let labels = put(
            &mut files,
            "labels.json",
            &json!({
                "v": LABELS_SCHEMA,
                "requires": [],
                "source": "author",
                "rubric": "docs/coder/design/2026-09-28-chat-router.md",
                "procedure": "each row labeled by hand with the route a correct router takes, the prepared answer it serves (or none), other acceptable answers, the tier, the risk, and the command group",
                "uncertainty": "the rows were written and labeled by the router's authors, so a reader weighs them as the authors' own claim about what the router should do",
            }),
            LABELS_SCHEMA,
        );
        let metric = |id: &str, unit: &str, direction: &str, population: &str| {
            json!({
                "id": id,
                "unit": unit,
                "direction": direction,
                "population": population,
                "aggregation": "chat-router.v1/ratio",
                "missing": "report_separately",
            })
        };
        let metrics = put(
            &mut files,
            "metrics.json",
            &json!({
                "v": METRICS_SCHEMA,
                "requires": [],
                "metrics": [
                    metric("route_accuracy", "share", "higher", "rows with a route reading"),
                    metric("route.<route>.precision", "share", "higher", "readings of the route"),
                    metric("route.<route>.recall", "share", "higher", "rows labeled the route"),
                    metric("canned_precision", "share", "higher", "turns served a prepared answer whole"),
                    metric("canned_recall", "share", "higher", "rows expecting a prepared answer whole"),
                    metric("canned_coverage", "share", "descriptive", "rows expecting a prepared answer whole"),
                    metric("dispatch_precision", "share", "higher", "turns given a Coder offer"),
                    metric("dispatch_recall", "share", "higher", "rows asking for work on code"),
                    metric("gym_precision", "share", "higher", "turns given a Gym or interview tier"),
                    metric("refuse_precision", "share", "higher", "turns refused"),
                    metric("secret_recall", "share", "higher", "rows holding or asking for a secret"),
                    metric("abstention_rate", "share", "descriptive", "rows read"),
                    metric("cases_passed", "cases", "higher", "rows read: route right, no harmful error, canned rows served whole, model rows left to the model"),
                    metric("latency_p50_ms", "milliseconds", "lower", "rows read"),
                    metric("latency_p95_ms", "milliseconds", "lower", "rows read"),
                    metric("<question>.ece", "error", "lower", "rows with a reading on the question, raw probabilities"),
                    metric("<question>.brier", "error", "lower", "rows with a reading on the question, raw probabilities"),
                    metric("<question>.ece_calibrated", "error", "lower", "the same rows, through the fitted map"),
                    metric("<question>.brier_calibrated", "error", "lower", "the same rows, through the fitted map"),
                    metric("<question>.precision_at_<t>", "share", "higher", "readings at or above t"),
                    metric("<question>.coverage_at_<t>", "share", "descriptive", "rows with a reading on the question"),
                    metric("<question>.abstention_at_<t>", "share", "descriptive", "rows with a reading on the question"),
                ],
            }),
            METRICS_SCHEMA,
        );
        let host = std::env::var("HOSTNAME")
            .ok()
            .filter(|name| !name.is_empty())
            .map(|name| nostr::contracts::digest_bytes(name.as_bytes()));
        let environment = put(
            &mut files,
            "environment.json",
            &json!({
                "v": ENVIRONMENT_SCHEMA,
                "requires": [],
                "engine": format!("coder router_eval {}", env!("CARGO_PKG_VERSION")),
                "judge": self.judge,
                "host": host,
                "policy": "one row at a time, the router's own request with the tool question over the product corpus's catalog and the CLI route's command groups, as the deployed worker asks it",
            }),
            ENVIRONMENT_SCHEMA,
        );
        let suite = put(
            &mut files,
            "suite.json",
            &json!({
                "v": SUITE_SCHEMA,
                "requires": [],
                "id": format!("{evaluator}:chat-router/{SET}"),
                "purpose": "routing",
                "workload": workload.value(),
                "cases": cases.value(),
                "partition": partition.value(),
                "labels": labels.value(),
                "metrics": metrics.value(),
                "acceptance": {
                    "id": format!("{evaluator}:gym-gates/{GATE}"),
                    "artifact": gate_file.value(),
                },
                "environment": environment.value(),
            }),
            SUITE_SCHEMA,
        );

        // The runs: one per row, with the readings beside them.
        let readings = put(
            &mut files,
            "readings.json",
            &json!({
                "v": READINGS_SCHEMA,
                "requires": [],
                "readings": self.readings,
            }),
            READINGS_SCHEMA,
        );
        let by_id: BTreeMap<&str, &Reading> =
            self.readings.iter().map(|r| (r.id.as_str(), r)).collect();
        let mut passed = 0;
        let runs_entries: Vec<Value> = self
            .rows
            .iter()
            .map(|row| {
                let reading = by_id.get(row.id.as_str());
                let outcome = match reading {
                    Some(reading) if reading.error.is_none() => "completed",
                    Some(_) => "failed",
                    None => "unknown",
                };
                let pass = reading.is_some_and(|reading| passes(row, reading));
                passed += usize::from(pass);
                json!({
                    "arm": "subject",
                    "case": case_id(&row.id),
                    "attempt": 1,
                    "outcome": outcome,
                    "passed": pass,
                    "receipts": [],
                    "artifacts": [readings.value()],
                })
            })
            .collect();
        let runs = put(
            &mut files,
            "runs.json",
            &json!({ "v": RUNS_SCHEMA, "requires": [], "runs": runs_entries }),
            RUNS_SCHEMA,
        );
        let calibration = put(
            &mut files,
            "calibration.json",
            &serde_json::to_value(self.calibration).unwrap_or(Value::Null),
            crate::router::calibration::SCHEMA,
        );

        // The measurements. Evidence names the runs artifact (and the
        // calibration record, for the calibration measures) by digest;
        // the full ArtifactRefs are on `runs` and in the calibration file.
        let evidence = vec![json!(runs.digest)];
        let with_map = vec![json!(runs.digest), json!(calibration.digest)];
        let read = report.read();
        let routed: usize = report.routes.values().map(|c| c.predicted).sum();
        let mut measurements = vec![
            measurement(
                "rows_read".into(),
                count(read),
                report.rows,
                report.errors,
                &evidence,
            ),
            measurement(
                "cases_passed".into(),
                count(passed),
                read,
                report.errors,
                &evidence,
            ),
            measurement(
                "route_accuracy".into(),
                report.route_accuracy,
                routed,
                report.errors,
                &evidence,
            ),
        ];
        for (route, counts) in &report.routes {
            measurements.push(measurement(
                format!("route.{route}.precision"),
                counts.precision(),
                counts.predicted,
                0,
                &evidence,
            ));
            measurements.push(measurement(
                format!("route.{route}.recall"),
                counts.recall(),
                counts.labeled,
                0,
                &evidence,
            ));
        }
        let ratios: [(&str, Option<f64>, usize); 12] = [
            (
                "canned_precision",
                report.canned.precision(),
                report.canned.predicted,
            ),
            (
                "canned_recall",
                report.canned.recall(),
                report.canned.labeled,
            ),
            (
                "canned_coverage",
                (report.canned.labeled > 0).then(|| {
                    #[allow(clippy::cast_precision_loss)]
                    let share = report.canned.predicted as f64 / report.canned.labeled as f64;
                    share
                }),
                report.canned.labeled,
            ),
            (
                "canned_over_stem",
                count(report.canned_over_stem),
                report.canned.predicted,
            ),
            (
                "dispatch_precision",
                report.dispatch.precision(),
                report.dispatch.predicted,
            ),
            (
                "dispatch_recall",
                report.dispatch.recall(),
                report.dispatch.labeled,
            ),
            (
                "gym_precision",
                report.gym.precision(),
                report.gym.predicted,
            ),
            ("gym_recall", report.gym.recall(), report.gym.labeled),
            (
                "refuse_precision",
                report.refuse.precision(),
                report.refuse.predicted,
            ),
            (
                "refuse_recall",
                report.refuse.recall(),
                report.refuse.labeled,
            ),
            (
                "secret_recall",
                report.secret.recall(),
                report.secret.labeled,
            ),
            ("abstention_rate", report.abstention_rate(), read),
        ];
        for (name, value, denominator) in ratios {
            measurements.push(measurement(name.into(), value, denominator, 0, &evidence));
        }
        #[allow(clippy::cast_precision_loss)]
        for (name, value) in [
            ("latency_p50_ms", report.latency_p50),
            ("latency_p95_ms", report.latency_p95),
        ] {
            measurements.push(measurement(
                name.into(),
                value.map(|v| v as f64),
                read,
                report.errors,
                &evidence,
            ));
        }
        let (route_obs, answer_obs) = observations(self.rows, self.readings);
        for (question, fitted, obs) in [
            ("route", &self.calibration.route, &route_obs),
            ("answer", &self.calibration.answer, &answer_obs),
        ] {
            let unavailable = read.saturating_sub(obs.len());
            for (name, value) in [
                ("ece", fitted.raw.ece),
                ("brier", fitted.raw.brier),
                ("nll", fitted.raw.nll),
                ("ece_calibrated", fitted.calibrated.ece),
                ("brier_calibrated", fitted.calibrated.brier),
                ("nll_calibrated", fitted.calibrated.nll),
            ] {
                measurements.push(measurement(
                    format!("{question}.{name}"),
                    Some(value),
                    fitted.held_out_items,
                    unavailable,
                    &with_map,
                ));
            }
            for point in operating_points(obs) {
                let t = format!("{:.2}", point.threshold);
                measurements.push(measurement(
                    format!("{question}.precision_at_{t}"),
                    point.precision(),
                    point.served,
                    unavailable,
                    &evidence,
                ));
                measurements.push(measurement(
                    format!("{question}.coverage_at_{t}"),
                    point.coverage(),
                    point.read,
                    unavailable,
                    &evidence,
                ));
                measurements.push(measurement(
                    format!("{question}.abstention_at_{t}"),
                    point.abstention(),
                    point.read,
                    unavailable,
                    &evidence,
                ));
            }
        }

        // The policy's reading, and the limitations.
        let outcome = self.gate.judge_router(&self.comparison());
        let mut limitations = vec![
            "no baseline arm ran: the previous router version was not kept runnable, so this record claims no change and its verdict is inconclusive; the gate's product floors are judged below".to_string(),
            "the subject's definition is not a published NIP-CAP head; its id's publisher is the evaluator's local provenance id, and identity is `version` (the question set and bank by digest, the judge by name), never `content`: the model behind the judge is not pinned".to_string(),
            "the rows were written and labeled by the router's authors (labels.json); no second author has validated the suite".to_string(),
            "the eval wires the CLI route and the tool question but no knowledge base, so a row expecting a grounded reply reads as the model alone here; grounded rows are counted on their route, not their tier".to_string(),
            "one run of each row; the run-to-run spread of every measure is the gate's pending measurement".to_string(),
            format!(
                "the calibration maps were fitted on the {} calibration rows and scored on the held-out rows; whether the worker serves them is CODER_WORKER_ROUTER_CALIBRATION, off by default (route map: {}; answer map: {})",
                self.calibration.fitted_rows, self.calibration.route.verdict, self.calibration.answer.verdict
            ),
        ];
        limitations.extend(outcome.criteria.iter().map(|c| {
            format!(
                "gate {} {}: {} ({})",
                outcome.gate_id, c.name, c.verdict, c.detail
            )
        }));
        let limitations = put(
            &mut files,
            "limitations.json",
            &json!({
                "v": LIMITATIONS_SCHEMA,
                "requires": [],
                "limitations": limitations,
                "partial": null,
                "gate_outcome": serde_json::to_value(&outcome).unwrap_or(Value::Null),
            }),
            LIMITATIONS_SCHEMA,
        );

        let cases_meta: Vec<Value> = self
            .rows
            .iter()
            .map(|row| json!({ "id": case_id(&row.id), "kind": case_kind(row) }))
            .collect();
        let report_value = json!({
            "v": REPORT_SCHEMA,
            "requires": [],
            "suite": suite.value(),
            "partition": partition.value(),
            "subject": {
                "definition": {
                    "id": format!("{evaluator}:chat-router/decide"),
                    "artifact": definition.value(),
                },
                "lock": lock.value(),
                "configuration": configuration.value(),
            },
            "baseline": null,
            "evaluator": evaluator,
            "started_at": self.started_at,
            "ended_at": self.ended_at,
            "runs": runs.value(),
            "coverage": {
                "subject": {
                    "planned": report.rows,
                    "attempted": report.rows,
                    "completed": read,
                    "refused": 0,
                    "failed": report.errors,
                    "cancelled": 0,
                    "unknown": 0,
                    "excluded": 0,
                },
                "baseline": null,
            },
            "measurements": measurements,
            "verdict": "inconclusive",
            "limitations": limitations.value(),
            "meta": {
                "ext_eval": {
                    "v": PROFILE_SCHEMA,
                    "gate": wire_gate(&self.gate.digest()),
                    "cases": cases_meta,
                    "headline": {
                        "subject_passed": passed,
                        "baseline_passed": null,
                        "total": self.rows.len(),
                    },
                    "requester": null,
                    "reliance": {
                        "runner": null,
                        "host": host,
                        "door": self.judge,
                        "model": null,
                        "agent": self.worker,
                        "selector": self.judge,
                        "graders": format!("coder router_eval {}", env!("CARGO_PKG_VERSION")),
                    },
                    "identity": "version",
                    "distribution": DISTRIBUTION,
                },
            },
        });
        Record {
            report: report_value,
            files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router_eval::Said;

    fn row(id: &str, route: &str, answer: Option<&str>, tier: &str) -> Row {
        Row {
            id: id.to_string(),
            messages: vec![Said {
                role: "user".to_string(),
                text: id.to_string(),
            }],
            route: route.to_string(),
            answer: answer.map(str::to_string),
            also: Vec::new(),
            tier: tier.to_string(),
            risk: "ok".to_string(),
            cli_group: None,
            tool: None,
            tags: Vec::new(),
            split: "held_out".to_string(),
        }
    }

    fn reading(id: &str, route: &str, answer: Option<&str>, tier: &str, dispatch: bool) -> Reading {
        Reading {
            id: id.to_string(),
            route: Some(route.to_string()),
            route_p: 0.9,
            answer: answer.map(str::to_string),
            read_answer: answer.map(str::to_string),
            answer_p: 0.85,
            tier: tier.to_string(),
            dispatch,
            ms: 200,
            ..Reading::default()
        }
    }

    /// A pass is the route right with no harmful error, coverage of a
    /// canned row, and restraint on a model row.
    #[test]
    fn a_case_passes_on_the_route_the_tier_and_no_harm() {
        let canned = row("meta/001", "meta", Some("meta.who"), "canned");
        assert!(passes(
            &canned,
            &reading("meta/001", "meta", Some("meta.who"), "canned", false)
        ));
        assert!(
            !passes(&canned, &reading("meta/001", "meta", None, "model", false)),
            "not covered"
        );
        assert!(
            !passes(
                &canned,
                &reading("meta/001", "meta", Some("meta.model"), "canned", false)
            ),
            "wrong answer"
        );
        assert!(
            !passes(
                &canned,
                &reading("meta/001", "general", Some("meta.who"), "canned", false)
            ),
            "wrong route"
        );
        let model = row("general/001", "general", None, "model");
        assert!(passes(
            &model,
            &reading("general/001", "general", None, "model", false)
        ));
        assert!(
            !passes(
                &model,
                &reading("general/001", "general", Some("meta.who"), "canned", false)
            ),
            "fired"
        );
        assert!(
            !passes(
                &model,
                &reading("general/001", "general", None, "offer", true)
            ),
            "offered Coder"
        );
        let stem = row(
            "dispatch/001",
            "work.dispatch",
            Some("dispatch.stem"),
            "offer",
        );
        assert!(passes(
            &stem,
            &reading(
                "dispatch/001",
                "work.dispatch",
                Some("dispatch.stem"),
                "offer",
                true
            )
        ));
        assert!(
            passes(
                &stem,
                &reading("dispatch/001", "work.dispatch", None, "model", false)
            ),
            "a miss on a stem row is not harm"
        );
        let mut failed = reading("meta/001", "meta", None, "model", false);
        failed.error = Some("timeout".into());
        assert!(!passes(&canned, &failed));
        assert_eq!(case_id("eval.run/012"), "eval.run-012");
        assert_eq!(case_kind(&model), "should-not-fire");
        assert_eq!(case_kind(&stem), "should-fire");
    }

    /// The record's bytes are what NIP-EVAL's parser accepts, every
    /// artifact the report cites is in the record by its digest, and no
    /// file carries a row's message.
    #[test]
    fn the_record_is_a_publishable_report_whose_artifacts_are_beside_it() {
        let rows = vec![
            row("meta/001", "meta", Some("meta.who"), "canned"),
            row("meta/002", "meta", Some("meta.model"), "canned"),
            row("general/001", "general", None, "model"),
            row(
                "dispatch/001",
                "work.dispatch",
                Some("dispatch.stem"),
                "offer",
            ),
            row("refuse/001", "refuse", Some("refuse.harmful"), "refuse"),
        ];
        let refs: Vec<&Row> = rows.iter().collect();
        let mut readings = vec![
            reading("meta/001", "meta", Some("meta.who"), "canned", false),
            reading("meta/002", "meta", Some("meta.who"), "canned", false),
            reading("general/001", "general", None, "model", false),
            reading(
                "dispatch/001",
                "work.dispatch",
                Some("dispatch.stem"),
                "offer",
                true,
            ),
        ];
        readings.push(Reading {
            id: "refuse/001".into(),
            error: Some("timeout".into()),
            ..Reading::default()
        });
        let report = Report::of("chat-router-v2", "held_out", &refs, &readings);
        let set = Set {
            schema: crate::router_eval::SCHEMA.into(),
            set: SET.into(),
            created: "2026-09-29".into(),
            rows: rows.clone(),
        };
        let calibration = Calibration::builtin().expect("the fixture parses");
        let gate = gym::gate::load(GATE).expect("the gate loads");
        let gate_bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../gym/gates/router-v1.json"),
        )
        .expect("the gate file");
        let claim = Claim {
            set: &set,
            suite_bytes: b"{\"schema\":\"openagents.gym.suite.v1\"}\n",
            gate_bytes: &gate_bytes,
            rows: &refs,
            readings: &readings,
            report: &report,
            calibration: &calibration,
            gate: &gate,
            bank: Bank::builtin(),
            judge: "typesafe:jev-latest",
            worker: "coder@test",
            commit: Some("0000000"),
            started_at: 1_790_000_000,
            ended_at: 1_790_000_100,
        };
        let record = claim.record();
        let bytes = record.bytes();
        let parsed = nostr::eval_ext::parse_report(&bytes).expect("NIP-EVAL accepts the report");
        assert_eq!(parsed.verdict, nostr::eval_ext::Verdict::Inconclusive);
        assert!(parsed.baseline.is_none());
        assert_eq!(parsed.profile.headline.total, 5);
        assert_eq!(
            parsed.profile.headline.subject_passed, 3,
            "meta/002 served the wrong answer and refuse/001 failed"
        );
        assert_eq!(
            parsed.profile.identity,
            Some(nostr::eval_ext::IdentityStrength::Version)
        );
        assert_eq!(parsed.profile.distribution.as_deref(), Some(DISTRIBUTION));
        assert_eq!(
            parsed.subject.configuration.schema.as_deref(),
            Some(CONFIGURATION_SCHEMA)
        );
        assert!(parsed.profile.gate.starts_with("sha256:"));
        assert!(gate.has_digest(&format!("gate:{}", &parsed.profile.gate["sha256:".len()..])));

        // Every cited artifact is in the record, byte for byte.
        let mut cited = Vec::new();
        collect_refs(&record.report, &mut cited);
        assert_eq!(
            cited.len(),
            7,
            "suite, partition, definition, lock, configuration, runs, limitations"
        );
        for (digest, size) in cited {
            let found = record.files.values().any(|bytes| {
                nostr::contracts::digest_bytes(bytes) == digest && bytes.len() as u64 == size
            });
            assert!(found, "{digest} is not beside the report");
        }
        let configuration = record.files.get("configuration.json").unwrap();
        let configuration: Value = serde_json::from_slice(configuration).unwrap();
        assert_eq!(configuration["set"], set_id());
        assert_eq!(configuration["bank"], Bank::builtin().id());
        let measurements = record.report["measurements"].as_array().unwrap();
        assert!(measurements.len() < nostr::eval_ext::MAX_MEASUREMENTS);
        // A held-out split of 256 rows (NIP-EVAL's most cases) stays under
        // the report's 64 KiB with these measurements: each row adds its
        // case and one run entry, and the measurements are fixed in number.
        let per_case = 64
            + serde_json::to_vec(&json!({"id": "eval.author-000", "kind": "should-not-fire"}))
                .unwrap()
                .len();
        assert!(
            bytes.len() + (nostr::eval_ext::MAX_CASES - 5) * per_case
                < nostr::eval_ext::MAX_REPORT_BYTES,
            "{} bytes for 5 rows",
            bytes.len()
        );
        let metric = |name: &str| {
            measurements
                .iter()
                .find(|m| m["metric"] == name)
                .unwrap_or_else(|| panic!("{name}"))
                .clone()
        };
        assert_eq!(metric("canned_precision")["value"], 0.5);
        assert_eq!(metric("dispatch_precision")["value"], 1.0);
        assert_eq!(metric("abstention_rate")["value"], 0.25);
        assert_eq!(metric("route.meta.recall")["value"], 1.0);
        assert!(metric("route.ece").is_object());
        assert!(metric("answer.precision_at_0.80").is_object());
        assert_eq!(metric("cases_passed")["unknown_count"], 1);

        // The gate's reading is written into the limitations: five rows are
        // under its items floor, so nothing past it is judged.
        let limitations: Value =
            serde_json::from_slice(record.files.get("limitations.json").unwrap()).unwrap();
        let lines = limitations["limitations"].as_array().unwrap();
        assert!(lines.iter().any(|l| {
            l.as_str()
                .unwrap()
                .contains("scored_rows>=30: unverifiable")
        }));
        assert!(lines.iter().any(|l| {
            l.as_str()
                .unwrap()
                .contains("canned_precision_at_or_above_floor: unverifiable")
        }));
        assert_eq!(limitations["gate_outcome"]["verdict"], "unverifiable");
        assert_eq!(claim.comparison().subject.items, 4);
        assert_eq!(claim.comparison().subject.canned_precision, Some(0.5));

        // Nothing carries a message: the rows' texts are their ids here,
        // and no file names the messages field.
        for (name, bytes) in &record.files {
            assert!(
                !String::from_utf8_lossy(bytes).contains("\"messages\""),
                "{name}"
            );
        }
        let dir = tempfile::tempdir().unwrap();
        record.write(dir.path()).unwrap();
        assert!(dir.path().join("report.json").is_file());
        assert!(dir.path().join("runs.json").is_file());
    }

    fn collect_refs(value: &Value, out: &mut Vec<(String, u64)>) {
        match value {
            Value::Object(map) => {
                if let (Some(digest), Some(size)) = (
                    map.get("digest").and_then(Value::as_str),
                    map.get("size").and_then(Value::as_u64),
                ) {
                    out.push((digest.to_string(), size));
                }
                for inner in map.values() {
                    collect_refs(inner, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
            _ => {}
        }
    }
}
