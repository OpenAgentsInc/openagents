//! Admission by measurement: which entries help, measured from the runs
//! Microcoder has already recorded.
//!
//! Every run's `summary.json` lists the entries its prompts showed. For one
//! entry, a run is *with* the entry when the entry is in that list, and
//! *without* it otherwise: a run with the knowledge base off, a run from
//! before the base existed, or a run where retrieval didn't keep it. Runs
//! are paired by task and model. A task the entry was written from (its
//! `provenance.written_from`) never counts.
//!
//! A paired task *favors* the entry when the runs with it pass more often,
//! or pass as often (and at least once) at under 90% of the cost per run.
//! It *opposes* the entry when they pass less often, or as often at over
//! 110% of the cost. The admission rule is [`RULE`].
//!
//! [`report`] records the result in the NIP-EVAL report shape
//! (`nips/openagents/NIP-EVAL.md`), with the entry as the subject and the
//! runs without it as the baseline.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::{Entry, digest};

/// Paired tasks that must favor an entry, with none opposing, for
/// `kb admit --evidence`.
pub const MIN_FAVORING: usize = 2;

/// Runs with an entry, out of sample, before `kb review` can call it shown
/// often.
pub const DEMOTE_MIN_RUNS: usize = 5;

/// The admission rule, as reports and the CLI state it.
pub const RULE: &str = "Admit an entry when at least 2 paired tasks favor it and none opposes it. \
A paired task is one task and model with runs both with and without the entry, on a task the \
entry wasn't written from. It favors the entry when the runs with it pass more often, or pass \
as often (at least once) at under 90% of the cost per run; it opposes the entry when they pass \
less often, or as often at over 110% of the cost.";

/// One recorded run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Run {
    /// The run directory's name, such as `some-task-1790393791`.
    pub name: String,
    pub task: String,
    pub model: String,
    /// The task's own verifier reward; `None` when grading failed.
    pub reward: Option<f64>,
    /// Model, Jev, and embedding dollars.
    pub usd: f64,
    /// Entry IDs the prompts listed or showed in full.
    pub used: Vec<String>,
    /// Unix seconds the run started, from the directory's name.
    pub started: u64,
}

impl Run {
    /// Whether the verifier gave the full reward.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.reward.is_some_and(|r| r >= 1.0)
    }
}

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

/// Reads one run's `summary.json`.
///
/// # Errors
///
/// No summary, or one without a task.
pub fn read_run(dir: &Path) -> Result<Run, String> {
    let path = dir.join("summary.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let task = value["task"]
        .as_str()
        .ok_or(format!("{} names no task", path.display()))?
        .to_string();
    let outcome = &value["outcome"];
    let usd = ["model_usd", "jev_usd", "embedding_usd"]
        .iter()
        .filter_map(|k| outcome[k].as_f64())
        .sum();
    let used = outcome["knowledge"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|u| u["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let started = name
        .rsplit_once('-')
        .and_then(|(_, stamp)| stamp.parse().ok())
        .unwrap_or(0);
    Ok(Run {
        name,
        task,
        model: value["model"].as_str().unwrap_or("unknown").to_string(),
        reward: value["reward"].as_f64(),
        usd,
        used,
        started,
    })
}

/// Every run under `dir` with a summary, in name order. Runs still going,
/// which have no summary yet, are skipped.
#[must_use]
pub fn scan(dir: &Path) -> Vec<Run> {
    let Ok(listing) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = listing
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("summary.json").is_file())
        .collect();
    dirs.sort();
    dirs.iter().filter_map(|d| read_run(d).ok()).collect()
}

/// One arm of a paired task.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Arm {
    /// Runs with a known reward.
    pub runs: usize,
    pub passes: usize,
    /// Dollars, summed over those runs.
    pub usd: f64,
    /// Runs whose grading failed.
    pub unknown: usize,
}

impl Arm {
    fn add(&mut self, run: &Run) {
        if run.reward.is_none() {
            self.unknown += 1;
            return;
        }
        self.runs += 1;
        self.usd += run.usd;
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
    pub fn usd_per_run(&self) -> f64 {
        if self.runs == 0 {
            0.0
        } else {
            self.usd / self.runs as f64
        }
    }
}

/// How one paired task came out for the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Favors,
    Opposes,
    Even,
}

/// One task and model with runs both with and without the entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pair {
    pub task: String,
    pub model: String,
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
    if with.runs == 0 || without.runs == 0 {
        return Side::Even;
    }
    let (a, b) = (with.rate(), without.rate());
    if a > b {
        Side::Favors
    } else if a < b {
        Side::Opposes
    } else if with.passes == 0 {
        Side::Even
    } else if with.usd_per_run() < 0.9 * without.usd_per_run() {
        Side::Favors
    } else if with.usd_per_run() > 1.1 * without.usd_per_run() {
        Side::Opposes
    } else {
        Side::Even
    }
}

/// Measures `entry` against `runs`.
#[must_use]
pub fn measure(entry: &Entry, runs: &[Run]) -> Measured {
    let excluded: BTreeSet<String> = entry
        .written_from
        .iter()
        .filter(|w| w.as_str() != "reference")
        .map(|w| task_of(w))
        .collect();
    let mut arms: BTreeMap<(String, String), (Arm, Arm)> = BTreeMap::new();
    let mut runs_with = 0;
    let mut excluded_runs = (0, 0);
    for run in runs {
        let with = run.used.contains(&entry.id);
        if excluded.contains(&run.task) {
            if with {
                excluded_runs.0 += 1;
            } else {
                excluded_runs.1 += 1;
            }
            continue;
        }
        let arm = arms
            .entry((run.task.clone(), run.model.clone()))
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
        .filter(|(_, (with, without))| with.runs > 0 && without.runs > 0)
        .map(|((task, model), (with, without))| Pair {
            side: side(&with, &without),
            task,
            model,
            with,
            without,
        })
        .collect();
    let favoring = pairs.iter().filter(|p| p.side == Side::Favors).count();
    let opposing = pairs.iter().filter(|p| p.side == Side::Opposes).count();
    let verdict = if favoring >= MIN_FAVORING && opposing == 0 {
        Verdict::Pass
    } else if opposing > 0 && favoring == 0 {
        Verdict::Fail
    } else {
        Verdict::Inconclusive
    };
    Measured {
        id: entry.id.clone(),
        version: entry.version,
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
    let paired: BTreeSet<(&str, &str)> = measured
        .pairs
        .iter()
        .map(|p| (p.task.as_str(), p.model.as_str()))
        .collect();
    let in_pair = |run: &&Run| paired.contains(&(run.task.as_str(), run.model.as_str()));
    let mut entries = Vec::new();
    let mut attempts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for run in runs.iter().filter(in_pair) {
        let arm = if run.used.contains(&measured.id) {
            "subject"
        } else {
            "baseline"
        };
        let attempt = attempts.entry((arm, run.task.as_str())).or_default();
        *attempt += 1;
        entries.push(json!({
            "arm": arm, "case": run.task, "attempt": *attempt,
            "outcome": if run.reward.is_some() { "completed" } else { "unknown" },
            "receipts": [], "artifacts": [],
            "meta": {"run": run.name, "model": run.model, "reward": run.reward, "usd": run.usd},
        }));
    }
    let partition = put(
        &mut artifacts,
        "openagents.eval-partition.v1",
        &json!({
            "development": [],
            "held_out": measured.pairs.iter().map(|p| &p.task).collect::<BTreeSet<_>>(),
            "excluded": measured.excluded_tasks,
        }),
    );
    let mut metric = |id: &str, unit: &str, direction: &str, operation: &str| {
        let artifact = put(
            &mut artifacts,
            "openagents.kb-aggregation.v1",
            &json!({"operation": operation}),
        );
        json!({"id": id, "unit": unit, "direction": direction,
               "population": "paired tasks' runs with a known reward",
               "aggregation": {"id": format!("{ns}:kb/{}", id.replace('_', "-")), "artifact": artifact},
               "missing": "report_separately"})
    };
    let metrics = json!([
        metric("pass_rate", "fraction", "higher", "passes divided by runs"),
        metric("usd_per_run", "usd", "lower", "dollars divided by runs"),
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
            sum.usd += arm.usd;
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
    let measurement = |arm: &str, metric: &str, value: f64, denominator: usize, unknown: usize| {
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
            "Few runs per task; no confidence interval is claimed.",
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
        "runs": put(&mut artifacts, "openagents.kb-runs.v1", &json!(entries)),
        "coverage": {
            "subject": coverage(&with, measured.excluded_runs.0),
            "baseline": coverage(&without, measured.excluded_runs.1),
        },
        "measurements": [
            measurement("subject", "pass_rate", with.rate(), with.runs, with.unknown),
            measurement("baseline", "pass_rate", without.rate(), without.runs, without.unknown),
            measurement("subject", "usd_per_run", with.usd_per_run(), with.runs, with.unknown),
            measurement("baseline", "usd_per_run", without.usd_per_run(), without.runs, without.unknown),
            measurement("comparison", "favoring_tasks", measured.favoring as f64, pairs, 0),
            measurement("comparison", "opposing_tasks", measured.opposing as f64, pairs, 0),
        ],
        "verdict": measured.verdict,
        "limitations": limitations,
        "meta": {"kb": {
            "entry": measured.id, "version": measured.version,
            "runs_with": measured.runs_with, "pairs": measured.pairs, "rule": RULE,
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

/// Demotes admitted entries shown often that never help, and names
/// candidates the rule would admit.
///
/// An admitted entry is shown often when at least [`DEMOTE_MIN_RUNS`] runs
/// had it, out of sample, and it never helps when it has at least one
/// paired task and none favors it.
#[must_use]
pub fn review(entries: &[Entry], runs: &[Run]) -> Vec<Proposal> {
    let mut out = Vec::new();
    for entry in entries {
        let m = measure(entry, runs);
        match entry.status {
            crate::Status::Admitted
                if m.runs_with >= DEMOTE_MIN_RUNS && !m.pairs.is_empty() && m.favoring == 0 =>
            {
                out.push(Proposal {
                    id: entry.id.clone(),
                    action: "demote",
                    reason: format!("shown in {}; {}", count(m.runs_with, "run"), tally(&m)),
                });
            }
            crate::Status::Candidate if m.verdict == Verdict::Pass => out.push(Proposal {
                id: entry.id.clone(),
                action: "admit",
                reason: tally(&m),
            }),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests;
