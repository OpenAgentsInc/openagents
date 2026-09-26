//! Historical screening of retained Microcoder runs.
//!
//! Screening does not establish a causal effect or authorize admission. Intake
//! retains incomplete and corrupt attempts, unknown costs, exact entry pins,
//! and comparison identities. A prospective declaration remains unverified until
//! a study runner verifies its frozen protocol and complete assignment ledger.
//!
//! Reports preserve observed differences without presenting them as promotion
//! evidence. Operator review remains a separate admission path.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::{Entry, digest};

/// Historical screening threshold, retained for interpreting earlier reports.
/// It is not an admission rule.
pub const MIN_FAVORING: usize = 2;

/// Historical exposure threshold retained for interpreting earlier reports.
/// It does not authorize automatic demotion.
pub const DEMOTE_MIN_RUNS: usize = 5;

/// The admission boundary stated by reports and the CLI.
pub const RULE: &str = "Historical screening never authorizes admission or automatic demotion. \
A prospective comparison requires independently verified pre-run protocol and assignments, \
complete intake, exact entry and configuration identities, and uncertainty supporting its \
declared acceptance rule. The historical reader does not verify prospective studies.";

mod intake;
pub use intake::{Cost, EntryPin, Identity, IntakeStatus, Run, StudyKind, read_run, scan};

/// `~/.openagents/microcoder/runs`.
#[must_use]
pub fn default_runs() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/microcoder/runs"))
}

/// `~/.openagents/knowledge/evidence`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/knowledge/evidence"))
}

/// The task a run ID names: the ID without its trailing `-<digits>`. A
/// bare task name is returned as it is.
#[must_use]
pub fn task_of(run: &str) -> String {
    match run.rsplit_once('-') {
        Some((task, stamp)) if !stamp.is_empty() && stamp.chars().all(|c| c.is_ascii_digit()) => {
            task.to_string()
        }
        _ => run.to_string(),
    }
}

/// One arm of a paired task.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Arm {
    /// Runs with a known reward.
    pub runs: usize,
    pub passes: usize,
    /// Known component costs across all attempts, including unknown outcomes.
    pub known_lower_bound_usd: f64,
    /// Fully known costs of attempts whose outcomes are also known.
    pub complete_cost_usd: f64,
    pub cost_known_runs: usize,
    pub cost_unknown_runs: usize,
    pub cost_overflow: bool,
    /// Runs whose grading failed.
    pub unknown: usize,
}

impl Arm {
    fn add(&mut self, run: &Run) {
        self.known_lower_bound_usd =
            (self.known_lower_bound_usd + run.cost.known_lower_bound_usd).min(f64::MAX);
        if run.reward.is_none() {
            self.unknown += 1;
            return;
        }
        self.runs += 1;
        if let Some(usd) = run.cost.total_usd {
            self.complete_cost_usd += usd;
            self.cost_known_runs += 1;
            if !self.complete_cost_usd.is_finite() {
                self.complete_cost_usd = f64::MAX;
                self.cost_overflow = true;
            }
        } else {
            self.cost_unknown_runs += 1;
        }
        if run.passed() {
            self.passes += 1;
        }
    }

    /// Passes over runs.
    #[must_use]
    pub fn rate(&self) -> f64 {
        if self.runs == 0 {
            0.0
        } else {
            self.passes as f64 / self.runs as f64
        }
    }

    /// Dollars per run.
    #[must_use]
    pub fn usd_per_run(&self) -> Option<f64> {
        (self.runs > 0 && self.unknown == 0 && self.cost_unknown_runs == 0 && !self.cost_overflow)
            .then(|| self.complete_cost_usd / self.runs as f64)
    }

    /// Wilson interval for the recorded pass fraction; not causal uncertainty.
    #[must_use]
    pub fn rate_interval(&self) -> Option<(f64, f64)> {
        if self.runs == 0 {
            return None;
        }
        let n = self.runs as f64;
        let p = self.rate();
        let z2 = 1.96_f64.powi(2);
        let denominator = 1.0 + z2 / n;
        let center = (p + z2 / (2.0 * n)) / denominator;
        let half = 1.96 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denominator;
        Some(((center - half).max(0.0), (center + half).min(1.0)))
    }
}

/// How one paired task came out for the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Favors,
    Opposes,
    Even,
    Inconclusive,
}

/// One task and model with runs both with and without the entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pair {
    pub task: String,
    pub model: String,
    /// None means historical task/model grouping, not configuration equivalence.
    pub comparison_digest: Option<String>,
    pub with: Arm,
    pub without: Arm,
    pub side: Side,
}

/// What the rule concludes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The rule admits the entry.
    Pass,
    /// Paired tasks oppose the entry and none favors it.
    Fail,
    /// Not enough evidence either way.
    Inconclusive,
}

/// One entry's paired evidence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Measured {
    pub id: String,
    pub version: u32,
    pub entry_digest: String,
    pub intake_records: usize,
    pub intake_faults: usize,
    pub unknown_membership: usize,
    pub changed_entry_runs: usize,
    pub unpinned_entry_runs: usize,
    pub noncomparable_runs: usize,
    pub prospective_unverified_runs: usize,
    /// This reader does not verify a pre-run study seal or assignment ledger.
    pub promotion_eligible: bool,
    /// Tasks the entry was written from, which never count.
    pub excluded_tasks: Vec<String>,
    /// Runs with the entry on tasks it wasn't written from.
    pub runs_with: usize,
    /// Runs on excluded tasks, with and without the entry.
    pub excluded_runs: (usize, usize),
    pub pairs: Vec<Pair>,
    pub favoring: usize,
    pub opposing: usize,
    pub verdict: Verdict,
}

fn side(with: &Arm, without: &Arm) -> Side {
    if with.runs == 0 || without.runs == 0 || with.unknown > 0 || without.unknown > 0 {
        return Side::Inconclusive;
    }
    let (a, b) = (with.rate(), without.rate());
    if a > b {
        return Side::Favors;
    }
    if a < b {
        return Side::Opposes;
    }
    if with.passes == 0 {
        return Side::Even;
    }
    match (with.usd_per_run(), without.usd_per_run()) {
        (Some(a), Some(b)) if a < 0.9 * b => Side::Favors,
        (Some(a), Some(b)) if a > 1.1 * b => Side::Opposes,
        (Some(_), Some(_)) => Side::Even,
        _ => Side::Inconclusive,
    }
}

/// Measures `entry` against `runs`.
#[must_use]
pub fn measure(entry: &Entry, runs: &[Run]) -> Measured {
    let excluded: BTreeSet<String> = entry
        .written_from
        .iter()
        .filter(|w| w.as_str() != "reference")
        .flat_map(|w| [w.clone(), task_of(w)])
        .collect();
    let mut arms: BTreeMap<(String, String, Option<String>), (Arm, Arm)> = BTreeMap::new();
    let mut unknown_membership = 0;
    let mut changed_entry_runs = 0;
    let mut unpinned_entry_runs = 0;
    let mut runs_with = 0;
    let mut excluded_runs = (0, 0);
    for run in runs {
        if !run.knowledge_complete {
            unknown_membership += 1;
            continue;
        }
        let with = run.used.contains(&entry.id);
        if excluded.contains(&run.task) {
            if with {
                excluded_runs.0 += 1;
            } else {
                excluded_runs.1 += 1;
            }
            continue;
        }
        if with {
            let pins: Vec<_> = run
                .knowledge
                .iter()
                .filter(|pin| pin.id == entry.id)
                .collect();
            if pins.iter().any(|pin| {
                pin.digest
                    .as_ref()
                    .is_some_and(|digest| digest != &entry.digest)
                    || pin.version.is_some_and(|version| version != entry.version)
            }) {
                changed_entry_runs += 1;
                continue;
            }
            if pins.iter().any(|pin| pin.digest.is_none()) {
                unpinned_entry_runs += 1;
            }
        }
        let arm = arms
            .entry((
                run.task.clone(),
                run.model.clone(),
                run.identity.comparison_digest.clone(),
            ))
            .or_default();
        if with {
            runs_with += 1;
            arm.0.add(run);
        } else {
            arm.1.add(run);
        }
    }
    let pairs: Vec<Pair> = arms
        .into_iter()
        .filter(|(_, (with, without))| {
            with.runs + with.unknown > 0 && without.runs + without.unknown > 0
        })
        .map(|((task, model, comparison_digest), (with, without))| Pair {
            side: side(&with, &without),
            task,
            model,
            comparison_digest,
            with,
            without,
        })
        .collect();
    let favoring = pairs.iter().filter(|p| p.side == Side::Favors).count();
    let opposing = pairs.iter().filter(|p| p.side == Side::Opposes).count();
    // Observed associations are retained, but neither promotion nor rejection
    // follows from a historical cohort that was not assigned prospectively.
    let verdict = Verdict::Inconclusive;
    Measured {
        id: entry.id.clone(),
        version: entry.version,
        entry_digest: entry.digest.clone(),
        intake_records: runs.len(),
        intake_faults: runs
            .iter()
            .filter(|run| run.intake != IntakeStatus::Complete)
            .count(),
        unknown_membership,
        changed_entry_runs,
        unpinned_entry_runs,
        noncomparable_runs: runs
            .iter()
            .filter(|run| run.identity.comparison_digest.is_none())
            .count(),
        prospective_unverified_runs: runs
            .iter()
            .filter(|run| run.identity.study_kind == StudyKind::ProspectiveUnverified)
            .count(),
        promotion_eligible: false,
        excluded_tasks: excluded.into_iter().collect(),
        runs_with,
        excluded_runs,
        pairs,
        favoring,
        opposing,
        verdict,
    }
}

/// Who measured: the report's evaluator and the namespace its qualified
/// IDs use. Locally both are `local`; a published report uses the
/// evaluator's public key for both.
#[derive(Clone, Debug)]
pub struct Evaluator {
    pub id: String,
    pub namespace: String,
}

impl Evaluator {
    /// The local evaluator, `local:microcoder`.
    #[must_use]
    pub fn local() -> Self {
        Evaluator {
            id: "local:microcoder".to_string(),
            namespace: "local".to_string(),
        }
    }
}

/// The artifacts a report references, by digest: each one's exact bytes.
pub type Artifacts = BTreeMap<String, Vec<u8>>;

/// Stores `value` as an artifact and returns its ArtifactRef.
fn put(artifacts: &mut Artifacts, schema: &str, value: &Value) -> Value {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let reference = json!({
        "digest": digest(&bytes),
        "size": bytes.len(),
        "media_type": "application/json",
        "schema": schema,
    });
    artifacts.insert(digest(&bytes), bytes);
    reference
}

/// The NIP-EVAL report for `measured`, the entry file `document`, and the
/// artifacts it references. `event` is the entry's `3190` EventRef, when
/// it's published. The entry is the evaluator's own: its qualified ID uses
/// the evaluator's namespace.
#[must_use]
pub fn report(
    measured: &Measured,
    document: &str,
    runs: &[Run],
    evaluator: &Evaluator,
    event: Option<Value>,
) -> (Value, Artifacts) {
    report_about(
        measured,
        document,
        &evaluator.namespace,
        runs,
        evaluator,
        event,
    )
}

/// [`report`] for an entry in the namespace `author`, which may be another
/// author's: the entry's qualified ID is `<author>:kb/<slug>`, and every
/// other ID stays in the evaluator's namespace.
#[must_use]
pub fn report_about(
    measured: &Measured,
    document: &str,
    author: &str,
    runs: &[Run],
    evaluator: &Evaluator,
    event: Option<Value>,
) -> (Value, Artifacts) {
    let mut artifacts = Artifacts::new();
    let ns = &evaluator.namespace;
    let mut definition = json!({
        "id": nostr::kb::qualified_id(author, &measured.id),
        "artifact": nostr::kb::document_artifact(document),
    });
    if let Some(event) = event {
        definition["event"] = event;
    }
    let lock = put(
        &mut artifacts,
        "openagents.lock.v1",
        &json!({
            "v": "openagents.lock.v1", "requires": [], "root": definition,
            "entries": [{"id": definition["id"], "definition": definition, "dependencies": []}],
        }),
    );
    let arm = |artifacts: &mut Artifacts, name: &str| {
        json!({
            "definition": definition,
            "lock": lock,
            "configuration": put(artifacts, "openagents.kb-arm.v1", &json!({
                "v": "openagents.kb-arm.v1", "requires": [], "arm": name,
            })),
        })
    };
    let subject = arm(&mut artifacts, "entry shown");
    let baseline = arm(&mut artifacts, "entry not shown");
    let paired: BTreeSet<(&str, &str, Option<&str>)> = measured
        .pairs
        .iter()
        .map(|p| {
            (
                p.task.as_str(),
                p.model.as_str(),
                p.comparison_digest.as_deref(),
            )
        })
        .collect();
    let in_pair = |run: &&Run| {
        paired.contains(&(
            run.task.as_str(),
            run.model.as_str(),
            run.identity.comparison_digest.as_deref(),
        ))
    };
    let mut entries = Vec::new();
    for run in runs {
        let mut retained = serde_json::to_value(run).unwrap_or(Value::Null);
        if let Some(bytes) = &run.summary_bytes {
            let key = digest(bytes);
            artifacts.insert(key.clone(), bytes.clone());
            retained["summary_artifact"] = json!({
                "digest": key, "size": bytes.len(), "media_type": "application/octet-stream",
            });
        }
        retained["partition"] = json!(if measured.excluded_tasks.contains(&run.task) {
            "source_excluded"
        } else if !run.knowledge_complete {
            "unassigned"
        } else if run.knowledge.iter().any(|pin| pin.id == measured.id
            && (pin
                .digest
                .as_ref()
                .is_some_and(|digest| digest != &measured.entry_digest)
                || pin
                    .version
                    .is_some_and(|version| version != measured.version)))
        {
            "different_entry_version"
        } else if in_pair(&run) {
            "paired_screening"
        } else {
            "unpaired"
        });
        retained["arm"] = if run.knowledge_complete {
            json!(if run.used.contains(&measured.id) {
                "subject"
            } else {
                "baseline"
            })
        } else {
            Value::Null
        };
        entries.push(retained);
    }
    let partition = put(
        &mut artifacts,
        "openagents.eval-partition.v1",
        &json!({
            "development": [],
            "held_out": [],
            "observational": measured.pairs.iter().map(|p| &p.task).collect::<BTreeSet<_>>(),
            "excluded": measured.excluded_tasks,
            "note": "A task absent from written_from is not automatically a held-out task.",
        }),
    );
    let mut metric = |id: &str, unit: &str, direction: &str, operation: &str| {
        let artifact = put(
            &mut artifacts,
            "openagents.kb-aggregation.v1",
            &json!({"operation": operation}),
        );
        json!({"id": id, "unit": unit, "direction": direction,
               "population": "historical screening groups; unknown outcomes and costs retained separately",
               "aggregation": {"id": format!("{ns}:kb/{}", id.replace('_', "-")), "artifact": artifact},
               "missing": "report_separately"})
    };
    let metrics = json!([
        metric("pass_rate", "fraction", "higher", "passes divided by runs"),
        metric(
            "usd_per_run",
            "usd",
            "lower",
            "complete component costs divided by runs; null if any outcome or cost is unknown"
        ),
        metric(
            "known_lower_bound_usd",
            "usd",
            "lower",
            "sum of known components including attempts with unknown outcomes; not comparable total cost"
        ),
        metric(
            "favoring_tasks",
            "tasks",
            "higher",
            "paired tasks that favor the entry"
        ),
        metric(
            "opposing_tasks",
            "tasks",
            "lower",
            "paired tasks that oppose the entry"
        ),
    ]);
    let suite = json!({
        "v": "openagents.eval-suite.v1", "requires": [],
        "id": format!("{ns}:kb/microcoder-runs"),
        "purpose": "context",
        "workload": put(&mut artifacts, "openagents.kb-workload.v1", &json!({
            "description": "Microcoder runs on Terminal-Bench 4 tasks, as recorded; observed, not sampled.",
            "synthetic": false,
        })),
        "cases": put(&mut artifacts, "openagents.kb-cases.v1",
            &json!(measured.pairs.iter().map(|p| &p.task).collect::<BTreeSet<_>>())),
        "partition": partition,
        "labels": put(&mut artifacts, "openagents.kb-labels.v1", &json!({
            "source": "each task's own verifier; a reward of 1 is a pass",
        })),
        "metrics": put(&mut artifacts, "openagents.kb-metrics.v1", &metrics),
        "acceptance": {
            "id": format!("{ns}:kb/paired-rule"),
            "artifact": put(&mut artifacts, "openagents.kb-rule.v1", &json!({
                "rule": RULE, "min_favoring": MIN_FAVORING,
            })),
        },
        "environment": put(&mut artifacts, "openagents.kb-environment.v1", &json!({
            "runner": "microcoder",
            "models": measured.pairs.iter().map(|p| &p.model).collect::<BTreeSet<_>>(),
        })),
    });
    let totals = |pick: fn(&Pair) -> &Arm| {
        measured.pairs.iter().fold(Arm::default(), |mut sum, p| {
            let arm = pick(p);
            sum.runs += arm.runs;
            sum.passes += arm.passes;
            sum.known_lower_bound_usd += arm.known_lower_bound_usd;
            sum.complete_cost_usd += arm.complete_cost_usd;
            sum.cost_known_runs += arm.cost_known_runs;
            sum.cost_unknown_runs += arm.cost_unknown_runs;
            sum.unknown += arm.unknown;
            sum
        })
    };
    let (with, without) = (totals(|p| &p.with), totals(|p| &p.without));
    let coverage = |arm: &Arm, excluded: usize| {
        json!({"planned": arm.runs + arm.unknown, "attempted": arm.runs + arm.unknown,
               "completed": arm.runs, "refused": 0, "failed": 0, "cancelled": 0,
               "unknown": arm.unknown, "excluded": excluded})
    };
    let measurement =
        |arm: &str, metric: &str, value: Option<f64>, denominator: usize, unknown: usize| {
            json!({"arm": arm, "metric": metric, "value": value, "denominator": denominator,
               "unknown_count": unknown, "uncertainty": null, "evidence": []})
        };
    let pairs = measured.pairs.len();
    let limitations = put(
        &mut artifacts,
        "openagents.kb-limitations.v1",
        &json!([
            "Runs are observed, not assigned: a run is with the entry when retrieval kept it, so the two arms can differ in more than the entry.",
            "Entries shown in the same runs share the credit; this doesn't isolate one entry's effect.",
            "Wilson intervals describe recorded pass fractions, not a causal entry effect or correction for selection bias.",
            "No historical cohort or self-declared prospective metadata authorizes admission or automatic demotion.",
            "Missing entry or configuration pins do not establish equivalence; changed entry versions are excluded from this version's groups.",
            "The intake artifact retains every attempted read, including unassigned and corrupt records; grouped metrics do not erase that denominator.",
        ]),
    );
    let started = runs
        .iter()
        .filter(in_pair)
        .map(|r| r.started)
        .min()
        .unwrap_or(0);
    let ended = runs
        .iter()
        .filter(in_pair)
        .map(|r| r.started)
        .max()
        .unwrap_or(0);
    let pair_records: Vec<Value> = measured
        .pairs
        .iter()
        .map(|pair| {
            let mut value = json!(pair);
            // NIP-XP v1 reads this total. Preserve its name without turning a
            // lower bound or incomplete arm into a comparable cost.
            value["with"]["usd"] =
                json!(pair.with.usd_per_run().map(|_| pair.with.complete_cost_usd));
            value["without"]["usd"] = json!(
                pair.without
                    .usd_per_run()
                    .map(|_| pair.without.complete_cost_usd)
            );
            value
        })
        .collect();
    let report = json!({
        "v": "openagents.eval-report.v1",
        "requires": [],
        "suite": put(&mut artifacts, "openagents.eval-suite.v1", &suite),
        "partition": partition,
        "subject": subject,
        "baseline": baseline,
        "evaluator": evaluator.id,
        "started_at": started,
        "ended_at": ended,
        "runs": put(&mut artifacts, "openagents.kb-intake.v2", &json!(entries)),
        "coverage": {
            "subject": coverage(&with, measured.excluded_runs.0),
            "baseline": coverage(&without, measured.excluded_runs.1),
        },
        "measurements": [
            measurement("subject", "pass_rate", (with.runs > 0).then(|| with.rate()), with.runs, with.unknown),
            measurement("baseline", "pass_rate", (without.runs > 0).then(|| without.rate()), without.runs, without.unknown),
            measurement("subject", "usd_per_run", with.usd_per_run(), with.runs, with.cost_unknown_runs + with.unknown),
            measurement("subject", "known_lower_bound_usd", Some(with.known_lower_bound_usd), with.runs + with.unknown, with.cost_unknown_runs + with.unknown),
            measurement("baseline", "usd_per_run", without.usd_per_run(), without.runs, without.cost_unknown_runs + without.unknown),
            measurement("baseline", "known_lower_bound_usd", Some(without.known_lower_bound_usd), without.runs + without.unknown, without.cost_unknown_runs + without.unknown),
            measurement("comparison", "favoring_tasks", Some(measured.favoring as f64), pairs, 0),
            measurement("comparison", "opposing_tasks", Some(measured.opposing as f64), pairs, 0),
        ],
        "verdict": measured.verdict,
        "limitations": limitations,
        "meta": {"kb": {
            "entry": measured.id, "version": measured.version, "entry_digest": measured.entry_digest,
            "evidence_revision": 2, "kind": "historical_screening", "promotion_eligible": false,
            "denominators": {
                "intake_records": measured.intake_records,
                "intake_faults": measured.intake_faults,
                "unknown_membership": measured.unknown_membership,
                "changed_entry_runs": measured.changed_entry_runs,
                "unpinned_entry_runs": measured.unpinned_entry_runs,
                "noncomparable_runs": measured.noncomparable_runs,
                "prospective_unverified_runs": measured.prospective_unverified_runs,
            },
            "pass_rate_uncertainty": {
                "method": "Wilson 95% marginal intervals; descriptive only",
                "subject": with.rate_interval(), "baseline": without.rate_interval(),
            },
            "runs_with": measured.runs_with, "pairs": pair_records, "rule": RULE,
        }},
    });
    (report, artifacts)
}

/// `<dir>/<id>.v<version>.json`, where an entry's report is kept.
#[must_use]
pub fn report_path(dir: &Path, id: &str, version: u32) -> PathBuf {
    dir.join(format!("{id}.v{version}.json"))
}

/// Writes `report` to `path` and its artifacts to `<dir>/artifacts/`,
/// each named by its digest. Returns the report's bytes.
///
/// # Errors
///
/// When a file can't be written.
pub fn write(path: &Path, report: &Value, artifacts: &Artifacts) -> Result<String, String> {
    let dir = path.parent().ok_or("the report path has no directory")?;
    let store = dir.join("artifacts");
    std::fs::create_dir_all(&store).map_err(|e| format!("can't make {}: {e}", store.display()))?;
    for (key, bytes) in artifacts {
        let name = format!("{}.json", key.trim_start_matches("sha256:"));
        std::fs::write(store.join(name), bytes).map_err(|e| format!("can't write: {e}"))?;
    }
    let text = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    match std::fs::read(path) {
        Ok(previous) => {
            let archive = dir.join("history");
            std::fs::create_dir_all(&archive)
                .map_err(|e| format!("can't create report history: {e}"))?;
            let name = format!("{}.json", digest(&previous).trim_start_matches("sha256:"));
            std::fs::write(archive.join(name), previous)
                .map_err(|e| format!("can't retain previous report: {e}"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("can't read previous report for retention: {error}")),
    }
    std::fs::write(path, &text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
    Ok(text)
}

/// The recorded verdict in the report at `path`, and the report's digest.
///
/// # Errors
///
/// When there's no report or it has no verdict.
pub fn recorded(path: &Path) -> Result<(Verdict, String), String> {
    let text = std::fs::read_to_string(path).map_err(|_| {
        format!(
            "no evidence recorded at {}; run kb evidence first",
            path.display()
        )
    })?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let verdict = match value["verdict"].as_str() {
        Some("pass") => Verdict::Pass,
        Some("fail") => Verdict::Fail,
        Some("inconclusive") => Verdict::Inconclusive,
        _ => return Err(format!("{} has no verdict", path.display())),
    };
    Ok((verdict, digest(text.as_bytes())))
}

/// `n` and `noun`, plural unless `n` is 1: `2 runs`, `2 entries`.
#[must_use]
pub fn count(n: usize, noun: &str) -> String {
    match noun.strip_suffix('y') {
        _ if n == 1 => format!("1 {noun}"),
        Some(stem) => format!("{n} {stem}ies"),
        None => format!("{n} {noun}s"),
    }
}

/// `3 paired tasks: 1 for it, 0 against it`.
#[must_use]
pub fn tally(measured: &Measured) -> String {
    format!(
        "{}: {} for it, {} against it",
        count(measured.pairs.len(), "paired task"),
        measured.favoring,
        measured.opposing
    )
}

/// The one-line summary an entry's evidence list keeps.
#[must_use]
pub fn line(measured: &Measured, report_digest: &str) -> String {
    format!(
        "measured {}: {} ({}) {report_digest}",
        crate::today(),
        tally(measured),
        match measured.verdict {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::Inconclusive => "inconclusive",
        }
    )
}

/// What `kb review` proposes for one entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Proposal {
    pub id: String,
    /// `demote` an admitted entry to candidate, or `admit` a candidate.
    pub action: &'static str,
    pub reason: String,
}

/// Historical screening can inform an operator, but cannot automatically
/// admit or demote an entry. Prospective admission needs a verified study.
#[must_use]
pub fn review(_entries: &[Entry], _runs: &[Run]) -> Vec<Proposal> {
    Vec::new()
}

/// Reads a report without allowing its historical verdict to authorize admission.
///
/// # Errors
///
/// A historical report or a self-declared prospective report is not a verified
/// study. The historical reader has no prospective study verifier.
pub fn recorded_for_admission(path: &Path, entry: &Entry) -> Result<(Verdict, String), String> {
    let _ = recorded(path)?;
    Err(format!(
        "evidence for {} v{} cannot authorize admission: a verified prospective study is required; use operator review for historical evidence",
        entry.id, entry.version
    ))
}

#[cfg(test)]
mod tests;
