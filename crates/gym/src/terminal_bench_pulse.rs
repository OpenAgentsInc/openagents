//! A targeted experiment's pulse: how it stands while it runs.
//!
//! `gym experiment pulse ID` reads an experiment's status file, every
//! finished trial's verifier reward and harness attempt record, and Coder
//! One's `composition.json` where the trial ran Coder One. It makes no
//! model call, and reports:
//!
//! - each arm's passes over graded attempts with a 95% Wilson interval, its
//!   mean and total cost, its mean time per graded attempt, and its mean
//!   setup before the agent's first command (environment and agent setup);
//! - how well each of Coder One's signals separates the verifier's passes
//!   from its failures: the final checks' verdicts, Jev's support answers,
//!   and the effort score, with counts, intervals, a Fisher exact test, and
//!   for the score the area under its ROC curve;
//! - how often escalation, the second executor, repair, and persistence
//!   fired, and how the trials they fired on ended, with persistence's
//!   rounds and stop reasons;
//! - the early-stopping verdict ([`crate::terminal_bench_stop`]) and the
//!   stops the scheduler recorded in the experiment's ledger;
//! - notable trials: checks that passed a failure or failed a pass, near
//!   misses, rescues, and tasks where the arms split.
//!
//! `--jev` and `--live` add Jev's judgments
//! ([`crate::terminal_bench_pulse_jev`]); everything in this module is
//! code.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::coder_matrix::wilson;
use crate::terminal_bench_experiment::{self as experiment, Report};
use crate::terminal_bench_stop::{self as stop, ArmState, Cell, Input, Recorded, Rule};

/// The schema of `gym experiment pulse --json`.
pub const SCHEMA: &str = "openagents.gym.experiment-pulse.v1";

/// The ledger schema `tbench experiment` writes.
pub const LEDGER_SCHEMA: &str = "openagents.tbench.experiment-ledger.v1";

/// A failed trial whose verifier passed at least this share of its tests
/// is a near miss.
pub const NEAR_MISS_SHARE: f64 = 0.8;

/// The most notable trials the text lists.
pub const NOTABLE_LIMIT: usize = 12;

/// One scheduled trial with what its files say.
#[derive(Clone, Debug, PartialEq)]
pub struct TrialFacts {
    pub job: String,
    pub arm: String,
    pub task: String,
    pub attempt: u64,
    pub state: String,
    pub reward: Option<f64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// The trial's whole cost. Partial Claude quota is not a substitute
    /// for an unpriced mixed-executor trial.
    pub cost_usd: Option<f64>,
    pub quota_usd: f64,
    pub duration_ms: Option<u64>,
    /// Setup before the agent's first command: Harbor's environment_setup
    /// and agent_setup phases, from the attempt record.
    pub setup_ms: Option<u64>,
    /// The trial directory under the job, when it exists.
    pub trial_dir: Option<PathBuf>,
    /// Coder One's composition record, when the trial ran Coder One.
    pub composition: Option<Value>,
    /// The verifier's `(passed, total)` tests.
    pub tests: Option<(u64, u64)>,
    pub failing_tests: Vec<String>,
}

impl TrialFacts {
    #[must_use]
    pub fn graded(&self) -> bool {
        self.state == "finished" && self.reward.is_some()
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        self.graded() && self.reward.is_some_and(|reward| reward >= 1.0)
    }

    /// `job/trial`, the id `gym runs` uses.
    #[must_use]
    pub fn run_id(&self) -> Option<String> {
        let trial = self.trial_dir.as_ref()?.file_name()?.to_string_lossy();
        Some(format!("{}/{trial}", self.job))
    }

    fn near_miss(&self) -> bool {
        !self.passed()
            && self.graded()
            && self.tests.is_some_and(|(passed, total)| {
                total > 0 && passed < total && passed as f64 >= NEAR_MISS_SHARE * total as f64
            })
    }
}

/// A stop the scheduler recorded in the ledger.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerStop {
    pub at: String,
    pub arm: Option<String>,
    pub state: String,
    pub reason: String,
    pub skipped: usize,
}

/// One row of a signal's table: the trials whose signal read `label`.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub label: String,
    pub passes: usize,
    pub fails: usize,
    /// A note about the row, such as how many ran no scenario.
    pub note: Option<String>,
}

impl Split {
    fn new(label: &str) -> Self {
        Split {
            label: label.to_owned(),
            passes: 0,
            fails: 0,
            note: None,
        }
    }

    fn add(&mut self, passed: bool) {
        if passed {
            self.passes += 1;
        } else {
            self.fails += 1;
        }
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.passes + self.fails
    }

    fn to_json(&self) -> Value {
        let interval = wilson(self.passes, self.total());
        json!({
            "label": self.label,
            "passes": self.passes,
            "fails": self.fails,
            "pass_rate": (self.total() > 0).then(|| self.passes as f64 / self.total() as f64),
            "wilson_95": [interval.0, interval.1],
            "note": self.note,
        })
    }

    fn line(&self) -> String {
        let (low, high) = wilson(self.passes, self.total());
        format!(
            "    {:<44} {:>3} pass {:>3} fail  {:>5}  95% {}{}",
            self.label,
            self.passes,
            self.fails,
            rate(self.passes, self.total()),
            if self.total() == 0 {
                "—".to_owned()
            } else {
                format!("{:.0}–{:.0}%", 100.0 * low, 100.0 * high)
            },
            self.note
                .as_deref()
                .map(|note| format!("  ({note})"))
                .unwrap_or_default()
        )
    }
}

/// How one signal lines up with the verifier.
#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    pub name: &'static str,
    pub rows: Vec<Split>,
    /// The Fisher exact test of the first row against every other row:
    /// `(first row's label, p)`.
    pub fisher: Option<(String, f64)>,
    /// For a score: the area under its ROC curve for a pass, with the
    /// passes and fails it rests on.
    pub auc: Option<(f64, usize, usize)>,
}

impl Signal {
    fn with_fisher(name: &'static str, rows: Vec<Split>) -> Self {
        let rows: Vec<Split> = rows.into_iter().filter(|row| row.total() > 0).collect();
        let fisher = rows.first().filter(|_| rows.len() > 1).map(|first| {
            let rest_pass: usize = rows[1..].iter().map(|row| row.passes).sum();
            let rest_fail: usize = rows[1..].iter().map(|row| row.fails).sum();
            (
                first.label.clone(),
                fisher_two_sided(first.passes, first.fails, rest_pass, rest_fail),
            )
        });
        Signal {
            name,
            rows,
            fisher,
            auc: None,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "signal": self.name,
            "rows": self.rows.iter().map(Split::to_json).collect::<Vec<_>>(),
            "fisher_exact": self.fisher.as_ref().map(|(label, p)| json!({"row": label, "against": "every other row", "p": p})),
            "auc": self.auc.map(|(auc, passes, fails)| json!({"auc": auc, "passes": passes, "fails": fails})),
        })
    }

    fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!("  {} compared with the verifier", self.name)];
        if self.rows.is_empty() {
            lines.push("    no graded trial recorded it".to_owned());
            return lines;
        }
        lines.extend(self.rows.iter().map(Split::line));
        if let Some((label, p)) = &self.fisher {
            lines.push(format!(
                "    \"{label}\" compared with the other rows: Fisher exact p = {}{}",
                p_text(*p),
                if *p >= 0.05 {
                    ", no clear difference"
                } else {
                    ""
                }
            ));
        }
        if let Some((auc, passes, fails)) = self.auc {
            lines.push(format!(
                "    AUC of the score for a pass: {auc:.2} over {passes} passes and {fails} fails (0.5 is chance)"
            ));
        }
        lines
    }
}

/// How often one component fired and how the trials ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub name: &'static str,
    /// Graded trials whose composition configured the component.
    pub configured: usize,
    /// Graded trials where it ran.
    pub fired: usize,
    pub rows: Vec<Split>,
    pub notes: Vec<String>,
}

impl Component {
    fn to_json(&self) -> Value {
        json!({
            "component": self.name,
            "configured": self.configured,
            "fired": self.fired,
            "fire_rate": (self.configured > 0).then(|| self.fired as f64 / self.configured as f64),
            "rows": self.rows.iter().map(Split::to_json).collect::<Vec<_>>(),
            "notes": self.notes,
        })
    }

    fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "  {}: configured on {}, ran on {} ({})",
            self.name,
            self.configured,
            self.fired,
            rate(self.fired, self.configured)
        )];
        lines.extend(
            self.rows
                .iter()
                .filter(|row| row.total() > 0)
                .map(Split::line),
        );
        lines.extend(self.notes.iter().map(|note| format!("    {note}")));
        lines
    }
}

/// One arm's standing.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmPulse {
    pub arm: String,
    pub scheduled: usize,
    pub graded: usize,
    pub passes: usize,
    pub running: usize,
    pub pending: usize,
    pub stopped: usize,
    pub interval: (f64, f64),
    pub mean_cost: Option<f64>,
    pub total_cost: f64,
    pub unpriced: usize,
    pub mean_ms: Option<u64>,
    /// Mean setup before the agent's first command, over the graded
    /// trials whose attempt record has it.
    pub mean_setup_ms: Option<u64>,
    pub quota_usd: f64,
}

/// A trial worth a look, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Notable {
    pub job: String,
    pub why: String,
}

/// The whole pulse.
#[derive(Clone, Debug)]
pub struct Pulse {
    pub report: Report,
    pub dir: Option<PathBuf>,
    pub trials: Vec<TrialFacts>,
    pub arms: Vec<ArmPulse>,
    pub signals: Vec<Signal>,
    pub components: Vec<Component>,
    pub notable: Vec<Notable>,
    pub rule: Rule,
    /// Whether the scheduler applies the rule, when the status says.
    pub stop_early: Option<bool>,
    pub verdict: stop::Verdict,
    pub ledger_stops: Vec<LedgerStop>,
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    out.sort();
    out
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// A job's trial directory: the newest one with a result, or any.
fn trial_dir(job: &Path) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = children(job)
        .into_iter()
        .filter(|path| path.is_dir() && path.join("config.json").is_file())
        .collect();
    dirs.iter()
        .rev()
        .find(|dir| dir.join("result.json").is_file())
        .or_else(|| dirs.last())
        .cloned()
}

/// Coder One's composition record in a trial directory.
fn composition(trial: &Path) -> Option<Value> {
    [
        "agent/episode/artifacts/composition.json",
        "artifacts/composition.json",
        "agent/composition.json",
    ]
    .into_iter()
    .find_map(|relative| read_json(&trial.join(relative)))
    .filter(|record| {
        crate::coder_composition::SCHEMAS
            .iter()
            .any(|schema| record["schema"] == *schema)
    })
}

/// The verifier's passed and total tests, and the failing tests' names.
fn verifier(trial: &Path) -> (Option<(u64, u64)>, Vec<String>) {
    let Some(ctrf) = read_json(&trial.join("verifier/ctrf.json")) else {
        return (None, Vec::new());
    };
    let summary = &ctrf["results"]["summary"];
    let tests = summary["tests"]
        .as_u64()
        .map(|total| (summary["passed"].as_u64().unwrap_or(0), total));
    let failing = ctrf["results"]["tests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|test| test["status"] == "failed")
        .filter_map(|test| test["name"].as_str().map(str::to_owned))
        .collect();
    (tests, failing)
}

/// An attempt record's setup before the agent: its `timing.setup_ms`, or
/// the environment and agent setup phases added when an older record has
/// only those.
fn setup_ms(record: &Value) -> Option<u64> {
    let field = |key: &str| {
        record
            .pointer(&format!("/timing/{key}"))
            .and_then(Value::as_u64)
    };
    field("setup_ms").or_else(|| Some(field("environment_setup_ms")? + field("agent_setup_ms")?))
}

impl TrialFacts {
    fn read(row: &Value, trial: &experiment::Trial, jobs: Option<&Path>) -> Self {
        let text = |key: &str| row[key].as_str().map(str::to_owned);
        let mut facts = TrialFacts {
            job: trial.job.clone(),
            arm: trial.arm.clone(),
            task: trial.task.clone(),
            attempt: trial.attempt,
            state: trial.state.clone(),
            reward: trial.reward,
            started_at: text("started_at"),
            finished_at: text("finished_at"),
            cost_usd: None,
            quota_usd: trial.quota_usd,
            duration_ms: None,
            setup_ms: None,
            trial_dir: None,
            composition: None,
            tests: None,
            failing_tests: Vec::new(),
        };
        if let (Some(started), Some(finished)) = (&facts.started_at, &facts.finished_at) {
            facts.duration_ms = crate::terminal_bench::timestamp_ms(finished)
                .zip(crate::terminal_bench::timestamp_ms(started))
                .and_then(|(end, start)| u64::try_from(end - start).ok());
        }
        let job_dir = jobs.map(|jobs| jobs.join(&trial.job));
        if let Some(job_dir) = job_dir.filter(|dir| dir.is_dir()) {
            facts.trial_dir = trial_dir(&job_dir);
            if let Some(dir) = &facts.trial_dir {
                let name = dir.file_name().unwrap_or_default().to_string_lossy();
                let record_path = job_dir.join("tbench/attempts").join(format!("{name}.json"));
                let record = read_json(&record_path);
                if let Some(record) = &record {
                    facts.cost_usd = record
                        .pointer("/cost/amount_usd")
                        .and_then(Value::as_f64)
                        .filter(|usd| usd.is_finite() && *usd >= 0.0);
                    if let Some(ms) = record.pointer("/timing/total_ms").and_then(Value::as_u64) {
                        facts.duration_ms = Some(ms);
                    }
                    facts.setup_ms = setup_ms(record);
                    // A trial the scheduler adopted after a restart has no
                    // times in the status file; the attempt record has them.
                    let timing = |key: &str| {
                        record
                            .pointer(&format!("/timing/{key}"))
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    };
                    if facts.started_at.is_none() {
                        facts.started_at = timing("started_at");
                    }
                    if facts.finished_at.is_none() && facts.state == "finished" {
                        facts.finished_at = timing("finished_at");
                    }
                }
                if !record_path.exists() {
                    facts.cost_usd = read_json(&dir.join("result.json"))
                        .and_then(|result| result.pointer("/agent_result/cost_usd")?.as_f64())
                        .filter(|usd| usd.is_finite() && *usd >= 0.0);
                }
                facts.composition = composition(dir);
                (facts.tests, facts.failing_tests) = verifier(dir);
            }
        }
        facts
    }
}

fn rate(part: usize, whole: usize) -> String {
    if whole == 0 {
        "—".to_owned()
    } else {
        format!("{:.0}%", 100.0 * part as f64 / whole as f64)
    }
}

fn p_text(p: f64) -> String {
    if p >= 0.995 {
        "1".to_owned()
    } else if p < 0.001 {
        "< 0.001".to_owned()
    } else {
        format!("{p:.3}")
    }
}

fn duration_text(ms: Option<u64>) -> String {
    ms.map_or_else(
        || "—".to_owned(),
        |ms| {
            let seconds = ms / 1000;
            if seconds >= 3600 {
                format!("{}h{:02}m", seconds / 3600, (seconds % 3600) / 60)
            } else {
                format!("{}m{:02}s", seconds / 60, seconds % 60)
            }
        },
    )
}

fn ln_choose(n: usize, k: usize) -> f64 {
    let ln_factorial = |n: usize| (1..=n).map(|i| (i as f64).ln()).sum::<f64>();
    ln_factorial(n) - ln_factorial(k) - ln_factorial(n - k)
}

/// The two-sided Fisher exact test of a 2×2 table: `a` and `b` in the
/// first row, `c` and `d` in the second.
#[must_use]
pub fn fisher_two_sided(a: usize, b: usize, c: usize, d: usize) -> f64 {
    let n = a + b + c + d;
    let row = a + b;
    let column = a + c;
    if n == 0 || row == 0 || row == n || column == 0 || column == n {
        return 1.0;
    }
    let p = |x: usize| {
        (ln_choose(row, x) + ln_choose(n - row, column - x) - ln_choose(n, column)).exp()
    };
    let observed = p(a);
    let low = column.saturating_sub(n - row);
    let high = row.min(column);
    (low..=high)
        .map(p)
        .filter(|q| *q <= observed * (1.0 + 1e-7))
        .sum::<f64>()
        .min(1.0)
}

/// The area under the ROC curve of `scores` for a pass: the chance a
/// passing trial scores above a failing one, ties counted half.
#[must_use]
pub fn auc(scores: &[(f64, bool)]) -> Option<f64> {
    let passes: Vec<f64> = scores.iter().filter(|s| s.1).map(|s| s.0).collect();
    let fails: Vec<f64> = scores.iter().filter(|s| !s.1).map(|s| s.0).collect();
    if passes.is_empty() || fails.is_empty() {
        return None;
    }
    let mut wins = 0.0;
    for p in &passes {
        for f in &fails {
            wins += match p.total_cmp(f) {
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Equal => 0.5,
                std::cmp::Ordering::Less => 0.0,
            };
        }
    }
    Some(wins / (passes.len() * fails.len()) as f64)
}

/// `text` with every number replaced by `N`, so stop reasons that differ
/// only in their numbers group together.
#[must_use]
pub fn normalize(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].is_ascii_digit() {
            // A period or comma inside a number, such as `2.91` or
            // `20,000`, is part of it; one that ends a sentence isn't.
            while index < chars.len()
                && (chars[index].is_ascii_digit()
                    || (matches!(chars[index], '.' | ',')
                        && chars.get(index + 1).is_some_and(char::is_ascii_digit)))
            {
                index += 1;
            }
            out.push('N');
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

/// The final checks' verdict: `all passed`, `a check failed`, or
/// `inconclusive`, and whether no scenario ran.
fn checks_word(record: &Value) -> (&'static str, bool) {
    let verdicts = &record["final_checks"]["verdicts"];
    let count = |key: &str| verdicts[key].as_u64().unwrap_or(0);
    let total: u64 = verdicts
        .as_object()
        .map(|map| map.values().filter_map(Value::as_u64).sum())
        .unwrap_or(0);
    if count("failed") > 0 {
        ("a check failed", false)
    } else if total > 0 && count("passed") == total {
        ("all passed", false)
    } else {
        ("inconclusive", total == 0)
    }
}

fn support_word(record: &Value) -> Option<&'static str> {
    let support = &record["support"];
    if !support.is_object() {
        return None;
    }
    let n = |key: &str| support[key].as_u64().unwrap_or(0);
    Some(if n("contradicted") > 0 {
        "a requirement contradicted"
    } else if n("judged") == 0 {
        "nothing judged"
    } else if n("supported") == n("judged") {
        "every judged requirement supported"
    } else {
        "unresolved requirements, none contradicted"
    })
}

fn split_rows(labels: &[&str]) -> Vec<Split> {
    labels.iter().map(|label| Split::new(label)).collect()
}

fn row<'a>(rows: &'a mut Vec<Split>, label: &str) -> &'a mut Split {
    if let Some(index) = rows.iter().position(|row| row.label == label) {
        &mut rows[index]
    } else {
        rows.push(Split::new(label));
        rows.last_mut().expect("just pushed")
    }
}

/// Per-kind discrimination on the candidate the experiment graded. Missing
/// reports are not passing observations, and each trial counts once per kind.
fn per_check_signal(composed: &[&TrialFacts]) -> Signal {
    let mut kinds: BTreeMap<String, Vec<Split>> = BTreeMap::new();
    let failures = composed.iter().filter(|t| !t.passed()).count();
    for trial in composed {
        let Some(record) = &trial.composition else {
            continue;
        };
        let Some(candidate) = record["final_checks"]["candidate"].as_str() else {
            continue;
        };
        let Some(entry) = record["checks"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|c| c["summary"]["candidate"].as_str() == Some(candidate))
        else {
            continue;
        };
        let Some(file) = entry["file"].as_str().filter(|f| {
            Path::new(f)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        }) else {
            continue;
        };
        let Some(dir) = &trial.trial_dir else {
            continue;
        };
        let Some(report) = read_json(&dir.join("agent/episode").join(file)) else {
            continue;
        };
        if report["candidate"]["digest"].as_str() != Some(candidate) {
            continue;
        }
        let names: BTreeMap<&str, &str> = report["scenarios"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| Some((s["id"].as_str()?, s["kind"].as_str()?)))
            .collect();
        let mut verdicts: BTreeMap<&str, u8> = BTreeMap::new();
        for v in report["verdicts"].as_array().into_iter().flatten() {
            let Some(kind) = v["scenario"].as_str().and_then(|id| names.get(id)).copied() else {
                continue;
            };
            let rank = match v["verdict"].as_str() {
                Some("passed") => 0,
                Some("failed") => 2,
                _ => 1,
            };
            verdicts
                .entry(kind)
                .and_modify(|r| *r = (*r).max(rank))
                .or_insert(rank);
        }
        for (kind, rank) in verdicts {
            kinds
                .entry(kind.to_string())
                .or_insert_with(|| split_rows(&["passed", "unknown", "failed"]))[usize::from(rank)]
            .add(trial.passed());
        }
    }
    let mut rows = Vec::new();
    for (kind, mut states) in kinds {
        let failed = &states[2];
        let precision = rate_text(failed.fails, failed.total());
        let recall = rate_text(failed.fails, failures);
        states[2].note = Some(format!(
            "fail precision {precision}; recall across all composed failures {recall}; descriptive trial-level intervals"
        ));
        for mut state in states {
            state.label = format!("{kind}: {}", state.label);
            if state.total() > 0 {
                rows.push(state);
            }
        }
    }
    Signal {
        name: "Per-check verdicts",
        rows,
        fisher: None,
        auc: None,
    }
}

fn rate_text(k: usize, n: usize) -> String {
    if n == 0 {
        return format!("{k}/{n} unknown");
    }
    let (low, high) = wilson(k, n);
    format!(
        "{k}/{n} {:.0}% ({:.0}–{:.0}%)",
        100.0 * k as f64 / n as f64,
        low * 100.0,
        high * 100.0
    )
}

fn signals(composed: &[&TrialFacts]) -> Vec<Signal> {
    let mut checks = split_rows(&["all passed", "inconclusive", "a check failed"]);
    let mut none_ran = (0, 0);
    let mut support = split_rows(&[
        "every judged requirement supported",
        "unresolved requirements, none contradicted",
        "nothing judged",
        "a requirement contradicted",
    ]);
    let mut effort_rows: Vec<Split> = Vec::new();
    let mut scores = Vec::new();
    for trial in composed {
        let record = trial.composition.as_ref().expect("composed");
        let passed = trial.passed();
        let (word, no_scenario) = checks_word(record);
        row(&mut checks, word).add(passed);
        if no_scenario {
            if passed {
                none_ran.0 += 1;
            } else {
                none_ran.1 += 1;
            }
        }
        if let Some(word) = support_word(record) {
            row(&mut support, word).add(passed);
        }
        let effort = &record["effort"];
        if let Some(score) = effort["score"].as_f64() {
            scores.push((score, passed));
            let at = effort["at"].as_f64();
            let label = format!(
                "ran {}{}",
                effort["effort"].as_str().unwrap_or("?"),
                at.map(|at| if score < at {
                    format!(", score below {at}")
                } else {
                    format!(", score at or above {at}")
                })
                .unwrap_or_default()
            );
            row(&mut effort_rows, &label).add(passed);
        }
    }
    if none_ran.0 + none_ran.1 > 0 {
        let note = format!(
            "{} ran no scenario: {} pass, {} fail",
            none_ran.0 + none_ran.1,
            none_ran.0,
            none_ran.1
        );
        row(&mut checks, "inconclusive").note = Some(note);
    }
    effort_rows.sort_by(|a, b| a.label.cmp(&b.label));
    let mut effort = Signal::with_fisher("Effort score", effort_rows);
    effort.fisher = None;
    let passes = scores.iter().filter(|s| s.1).count();
    effort.auc = auc(&scores).map(|value| (value, passes, scores.len() - passes));
    vec![
        Signal::with_fisher("Final checks", checks),
        Signal::with_fisher("Jev support", support),
        effort,
        per_check_signal(composed),
    ]
}

fn components(composed: &[&TrialFacts]) -> Vec<Component> {
    let mut escalation = Component {
        name: "Escalation (verify.second)",
        configured: 0,
        fired: 0,
        rows: split_rows(&["ran, kept the second candidate", "ran, kept the first"]),
        notes: Vec::new(),
    };
    let mut repair = Component {
        name: "Repair",
        configured: 0,
        fired: 0,
        rows: split_rows(&["ran, changed the candidate", "ran, changed nothing"]),
        notes: Vec::new(),
    };
    let mut persist = Component {
        name: "Persistence (control.persist)",
        configured: 0,
        fired: 0,
        rows: Vec::new(),
        notes: Vec::new(),
    };
    let mut handoff = Component {
        name: "Handoff escalation (route escalated)",
        configured: 0,
        fired: 0,
        rows: split_rows(&["escalated"]),
        notes: Vec::new(),
    };
    let mut rounds: BTreeMap<usize, usize> = BTreeMap::new();
    let mut persist_cost = 0.0;
    let mut second_cost = 0.0;
    for trial in composed {
        let record = trial.composition.as_ref().expect("composed");
        let passed = trial.passed();
        let second = &record["second"];
        if second.is_object() {
            escalation.configured += 1;
            if let Some(why) = second["skipped"].as_str() {
                row(
                    &mut escalation.rows,
                    &format!("skipped: {}", normalize(why)),
                )
                .add(passed);
            } else {
                escalation.fired += 1;
                second_cost += second["cost_usd"].as_f64().unwrap_or(0.0);
                let label = if second["kept"] == "second" {
                    "ran, kept the second candidate"
                } else {
                    "ran, kept the first"
                };
                row(&mut escalation.rows, label).add(passed);
            }
        }
        let repair_record = &record["repair"];
        if repair_record.is_object() {
            repair.configured += 1;
            if repair_record["ran"] == true {
                repair.fired += 1;
                let label = if repair_record["changed"] == true {
                    "ran, changed the candidate"
                } else {
                    "ran, changed nothing"
                };
                row(&mut repair.rows, label).add(passed);
            } else {
                let why = repair_record["skipped"].as_str().unwrap_or("not triggered");
                row(&mut repair.rows, &format!("skipped: {}", normalize(why))).add(passed);
            }
        }
        if record.get("escalated").is_some() {
            handoff.configured += 1;
            if record["escalated"] == true {
                handoff.fired += 1;
                row(&mut handoff.rows, "escalated").add(passed);
            }
        }
        let persist_record = &record["persist"];
        if persist_record.is_object() {
            persist.configured += 1;
            let n = persist_record["rounds"].as_array().map_or(0, Vec::len);
            *rounds.entry(n).or_default() += 1;
            if n > 0 {
                persist.fired += 1;
            }
            persist_cost += persist_record
                .pointer("/totals/cost_usd")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let why = persist_record["stopped"]
                .as_str()
                .map_or_else(|| "no stop recorded".to_owned(), normalize);
            row(&mut persist.rows, &format!("stopped: {why}")).add(passed);
        }
    }
    let ran = escalation.rows[0].total() + escalation.rows[1].total();
    let ran_passed = escalation.rows[0].passes + escalation.rows[1].passes;
    if escalation.configured > 0 {
        escalation.notes.push(format!(
            "ran {ran} times with {ran_passed} passes; second executors cost ${second_cost:.2}"
        ));
        for row in escalation
            .rows
            .iter()
            .filter(|r| r.label.starts_with("skipped"))
        {
            escalation.notes.push(format!(
                "skipped on {} trials ({}), and {} of those failed anyway",
                row.total(),
                row.label.trim_start_matches("skipped: "),
                row.fails
            ));
        }
    }
    if persist.configured > 0 {
        persist.notes.push(format!(
            "rounds per trial: {}; rounds cost ${persist_cost:.2}",
            rounds
                .iter()
                .map(|(n, count)| format!("{n} round{} × {count}", if *n == 1 { "" } else { "s" }))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut out = vec![escalation, repair, persist];
    if handoff.configured > 0 {
        out.push(handoff);
    }
    out
}

fn notable(trials: &[TrialFacts], report: &Report) -> Vec<Notable> {
    let mut out = Vec::new();
    for trial in trials.iter().filter(|t| t.graded()) {
        let Some(record) = &trial.composition else {
            if trial.near_miss() {
                out.push(Notable {
                    job: trial.job.clone(),
                    why: near_miss_text(trial),
                });
            }
            continue;
        };
        let (checks, _) = checks_word(record);
        if checks == "all passed" && !trial.passed() {
            out.push(Notable {
                job: trial.job.clone(),
                why: format!(
                    "every final check passed and the verifier failed it{}",
                    trial
                        .tests
                        .map(|(p, t)| format!(" ({p} of {t} verifier tests passed)"))
                        .unwrap_or_default()
                ),
            });
        } else if checks == "a check failed" && trial.passed() {
            out.push(Notable {
                job: trial.job.clone(),
                why: "a final check failed and the verifier passed it".to_owned(),
            });
        }
        if record["second"]["kept"] == "second" && record["second"]["skipped"].is_null() {
            out.push(Notable {
                job: trial.job.clone(),
                why: format!(
                    "escalation kept the second candidate, and the verifier {}",
                    if trial.passed() {
                        "passed it"
                    } else {
                        "failed it"
                    }
                ),
            });
        }
        if trial.near_miss() {
            out.push(Notable {
                job: trial.job.clone(),
                why: near_miss_text(trial),
            });
        }
    }
    // Tasks where one arm passes at least two attempts and another none of
    // at least two.
    for task in &report.tasks {
        let cells = report.task_cells(task);
        for (i, (passes, graded)) in cells.iter().enumerate() {
            for (j, (other_passes, other_graded)) in cells.iter().enumerate() {
                if i != j && *passes >= 2 && *other_passes == 0 && *other_graded >= 2 {
                    out.push(Notable {
                        job: task.clone(),
                        why: format!(
                            "{} passed {passes} of {graded}, {} passed 0 of {other_graded}",
                            report.arms[i], report.arms[j]
                        ),
                    });
                }
            }
        }
    }
    if let Some(costliest) = trials
        .iter()
        .filter(|t| t.graded())
        .filter_map(|t| t.cost_usd.map(|usd| (t, usd)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
    {
        out.push(Notable {
            job: costliest.0.job.clone(),
            why: format!(
                "the costliest graded trial: ${:.2}, {}",
                costliest.1,
                if costliest.0.passed() {
                    "passed"
                } else {
                    "failed"
                }
            ),
        });
    }
    // One line per trial, its reasons joined, in the order first found.
    let mut merged: Vec<Notable> = Vec::new();
    for item in out {
        match merged.iter_mut().find(|m| m.job == item.job) {
            Some(found) => {
                found.why.push_str("; ");
                found.why.push_str(&item.why);
            }
            None => merged.push(item),
        }
    }
    merged
}

fn near_miss_text(trial: &TrialFacts) -> String {
    let (passed, total) = trial.tests.unwrap_or((0, 0));
    format!(
        "a near miss: {passed} of {total} verifier tests passed{}",
        if trial.failing_tests.is_empty() {
            String::new()
        } else {
            format!(
                "; failing {}",
                trial
                    .failing_tests
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    )
}

/// The stops `tbench experiment run --stop-early` recorded.
#[must_use]
pub fn ledger_stops(dir: &Path) -> Vec<LedgerStop> {
    let Ok(text) = std::fs::read_to_string(dir.join("ledger.jsonl")) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|record| record["event"] == "stop")
        .map(|record| LedgerStop {
            at: record["at"].as_str().unwrap_or_default().to_owned(),
            arm: record["arm"].as_str().map(str::to_owned),
            state: record["state"].as_str().unwrap_or_default().to_owned(),
            reason: record["reason"].as_str().unwrap_or_default().to_owned(),
            skipped: record["skipped"].as_array().map_or(0, Vec::len),
        })
        .collect()
}

/// The rule's input from the trials as they stand now.
#[must_use]
pub fn rule_input(report: &Report, trials: &[TrialFacts], stops: &[LedgerStop]) -> Input {
    let mut input = Input {
        arms: report.arms.clone(),
        tasks: report.tasks.clone(),
        attempts: report.attempts,
        ..Input::default()
    };
    let mut costs: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
    for trial in trials {
        let mut cell = Cell::of(&trial.state, trial.reward);
        if cell == Cell::Open && trial.state == "skipped" {
            cell = Cell::Dead;
        }
        input
            .cells
            .insert((trial.arm.clone(), trial.task.clone(), trial.attempt), cell);
        if trial.graded()
            && let Some(usd) = trial.cost_usd
        {
            let entry = costs.entry(&trial.arm).or_default();
            entry.0 += usd;
            entry.1 += 1;
        }
    }
    input.mean_cost = costs
        .into_iter()
        .filter(|(arm, (_, n))| {
            *n == trials
                .iter()
                .filter(|t| t.arm == **arm && t.graded())
                .count()
        })
        .map(|(arm, (sum, n))| (arm.to_owned(), sum / n as f64))
        .collect();
    for stop in stops {
        if let Some(arm) = &stop.arm
            && let Some(state) = ArmState::parse(&stop.state).filter(|s| s.stopped())
        {
            input
                .stopped
                .insert(arm.clone(), (state, stop.reason.clone()));
        }
    }
    input
}

impl Pulse {
    /// Reads an experiment's status file and every trial's files.
    ///
    /// # Errors
    /// When the status file can't be read or isn't an experiment's.
    pub fn load(status: &Path, jobs: Option<&Path>, rule: Option<Rule>) -> Result<Self, String> {
        let body = std::fs::read_to_string(status)
            .map_err(|error| format!("can't read {}: {error}", status.display()))?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|error| format!("{} isn't JSON: {error}", status.display()))?;
        let mut report = Report::from_status(&value)?;
        report.source = Some(status.to_path_buf());
        let rows = value["trials"].as_array().cloned().unwrap_or_default();
        let trials: Vec<TrialFacts> = report
            .trials
            .iter()
            .zip(&rows)
            .map(|(trial, row)| TrialFacts::read(row, trial, jobs))
            .collect();
        let dir = status.parent().map(Path::to_path_buf);
        let ledger_stops = dir.as_deref().map(ledger_stops).unwrap_or_default();
        let settings = &value["stop_early"];
        let rule = rule.unwrap_or(Rule {
            alpha: settings["alpha"].as_f64().unwrap_or(stop::DEFAULT_ALPHA),
            accept_pass_rate: settings["accept_pass_rate"].as_f64(),
        });
        let verdict = stop::evaluate(&rule_input(&report, &trials, &ledger_stops), rule);
        let composed: Vec<&TrialFacts> = trials
            .iter()
            .filter(|t| t.graded() && t.composition.is_some())
            .collect();
        let arms = report
            .summaries
            .iter()
            .map(|summary| {
                let mine: Vec<&TrialFacts> =
                    trials.iter().filter(|t| t.arm == summary.arm).collect();
                let graded: Vec<&&TrialFacts> = mine.iter().filter(|t| t.graded()).collect();
                let priced: Vec<f64> = graded.iter().filter_map(|t| t.cost_usd).collect();
                // A float sum of nothing is -0.0; start from 0.0.
                let total = priced.iter().fold(0.0, |sum, usd| sum + usd);
                let times: Vec<u64> = graded.iter().filter_map(|t| t.duration_ms).collect();
                let setups: Vec<u64> = graded.iter().filter_map(|t| t.setup_ms).collect();
                ArmPulse {
                    arm: summary.arm.clone(),
                    scheduled: summary.scheduled,
                    graded: summary.graded,
                    passes: summary.passes,
                    running: mine.iter().filter(|t| t.state == "running").count(),
                    pending: mine.iter().filter(|t| t.state == "pending").count(),
                    stopped: mine.iter().filter(|t| t.state == "skipped").count(),
                    interval: summary.interval,
                    mean_cost: (!priced.is_empty() && priced.len() == graded.len())
                        .then(|| total / priced.len() as f64),
                    total_cost: total,
                    unpriced: graded.len() - priced.len(),
                    mean_ms: (!times.is_empty())
                        .then(|| times.iter().sum::<u64>() / times.len() as u64),
                    mean_setup_ms: (!setups.is_empty())
                        .then(|| setups.iter().sum::<u64>() / setups.len() as u64),
                    quota_usd: summary.quota_usd,
                }
            })
            .collect();
        Ok(Pulse {
            signals: signals(&composed),
            components: components(&composed),
            notable: notable(&trials, &report),
            stop_early: settings["enabled"].as_bool(),
            report,
            dir,
            trials,
            arms,
            rule,
            verdict,
            ledger_stops,
        })
    }

    /// Applies another rule and evaluates it again.
    pub fn set_rule(&mut self, rule: Rule) {
        self.rule = rule;
        self.verdict = stop::evaluate(
            &rule_input(&self.report, &self.trials, &self.ledger_stops),
            rule,
        );
    }

    /// Graded trials with a composition record.
    #[must_use]
    pub fn composed(&self) -> usize {
        self.trials
            .iter()
            .filter(|t| t.graded() && t.composition.is_some())
            .count()
    }

    /// Replays the recorded trials through the rule.
    #[must_use]
    pub fn replay(&self) -> stop::Replay {
        let recorded: Vec<Recorded> = self
            .trials
            .iter()
            .map(|t| Recorded {
                job: t.job.clone(),
                arm: t.arm.clone(),
                task: t.task.clone(),
                attempt: t.attempt,
                state: t.state.clone(),
                reward: t.reward,
                started_at: t.started_at.clone(),
                finished_at: t.finished_at.clone(),
                cost_usd: t.cost_usd,
            })
            .collect();
        stop::replay(
            &self.report.arms,
            &self.report.tasks,
            self.report.attempts,
            &recorded,
            self.rule,
        )
    }

    fn short_arms(&self) -> Vec<String> {
        short_names(&self.report.arms)
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "experiment": self.report.id,
            "profile": self.report.profile,
            "state": self.report.state,
            "updated_at": self.report.updated_at,
            "source": self.report.source,
            "arms": self.arms.iter().map(|a| json!({
                "arm": a.arm,
                "scheduled": a.scheduled,
                "graded": a.graded,
                "passes": a.passes,
                "running": a.running,
                "pending": a.pending,
                "stopped": a.stopped,
                "pass_rate": (a.graded > 0).then(|| a.passes as f64 / a.graded as f64),
                "wilson_95": [a.interval.0, a.interval.1],
                "mean_cost_usd": a.mean_cost,
                "total_cost_usd": (a.unpriced == 0).then_some(a.total_cost),
                "priced_total_cost_usd": a.total_cost,
                "unpriced": a.unpriced,
                "mean_ms": a.mean_ms,
                "mean_setup_ms": a.mean_setup_ms,
                "claude_quota_usd": a.quota_usd,
            })).collect::<Vec<_>>(),
            "tasks_by_arm": self.report.tasks.iter().map(|task| json!({
                "task": task,
                "cells": self.report.task_cells(task).iter().zip(&self.report.arms).map(|((passes, graded), arm)| json!({"arm": arm, "passes": passes, "graded": graded})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "composed_graded": self.composed(),
            "signals": self.signals.iter().map(Signal::to_json).collect::<Vec<_>>(),
            "components": self.components.iter().map(Component::to_json).collect::<Vec<_>>(),
            "stop_early": self.stop_early,
            "stopping": self.verdict.to_json(),
            "ledger_stops": self.ledger_stops.iter().map(|s| json!({
                "at": s.at, "arm": s.arm, "state": s.state, "reason": s.reason, "skipped": s.skipped,
            })).collect::<Vec<_>>(),
            "notable": self.notable.iter().map(|n| json!({"job": n.job, "why": n.why})).collect::<Vec<_>>(),
            "parallel": self.trials.iter().filter_map(|t| {
                let branches = t.composition.as_ref()?["branches"].as_array()?.clone();
                let parallel: Vec<Value> = branches.iter().filter(|b| b["parallel"].is_object()).map(|b| b["parallel"].clone()).collect();
                (!parallel.is_empty()).then(|| json!({ "job": t.job, "parallel": parallel }))
            }).collect::<Vec<_>>(),
            "model_calls": 0,
        })
    }

    /// Where the experiment stands: its arms, its tasks, the stopping
    /// verdict, and the ledger's stops.
    #[must_use]
    pub fn standing_lines(&self) -> Vec<String> {
        let r = &self.report;
        let graded = self.trials.iter().filter(|t| t.graded()).count();
        let running = self.trials.iter().filter(|t| t.state == "running").count();
        let pending = self.trials.iter().filter(|t| t.state == "pending").count();
        let stopped = self.trials.iter().filter(|t| t.state == "skipped").count();
        let mut lines = vec![
            format!(
                "Pulse of {} ({}): {} arms · {} tasks × {} attempts · {}{}",
                r.id,
                r.profile,
                r.arms.len(),
                r.tasks.len(),
                r.attempts,
                r.state,
                r.updated_at
                    .as_deref()
                    .map(|at| format!(" · updated {at}"))
                    .unwrap_or_default()
            ),
            format!(
                "  {graded} graded of {} scheduled · {running} running · {pending} pending · {stopped} skipped or stopped · {} lost and rerun · no model calls",
                self.trials.len(),
                r.losses().values().sum::<usize>()
            ),
            String::new(),
            format!(
                "  {:<34} {:>7} {:>5}  {:<9} {:>9} {:>10} {:>9} {:>10} {:>9}",
                "arm",
                "passes",
                "rate",
                "95% range",
                "mean cost",
                "total cost",
                "mean time",
                "mean setup",
                "Claude $"
            ),
        ];
        for a in &self.arms {
            lines.push(format!(
                "  {:<34} {:>3}/{:<3} {:>5}  {:<9} {:>9} {:>10} {:>9} {:>10} {:>9}{}",
                a.arm,
                a.passes,
                a.graded,
                rate(a.passes, a.graded),
                if a.graded == 0 {
                    "—".to_owned()
                } else {
                    format!("{:.0}–{:.0}%", 100.0 * a.interval.0, 100.0 * a.interval.1)
                },
                a.mean_cost
                    .map_or_else(|| "—".to_owned(), |usd| format!("${usd:.2}")),
                if a.unpriced == 0 {
                    format!("${:.2}", a.total_cost)
                } else {
                    format!(">=${:.2}", a.total_cost)
                },
                duration_text(a.mean_ms),
                duration_text(a.mean_setup_ms),
                format!("${:.2}", a.quota_usd),
                if a.running + a.pending + a.stopped > 0 {
                    format!(
                        "  ({} running, {} pending, {} stopped)",
                        a.running, a.pending, a.stopped
                    )
                } else {
                    String::new()
                }
            ));
            if a.unpriced > 0 {
                lines.push(format!(
                    "    {} of {} graded trials have unknown total cost; the subtotal includes only fully priced trials",
                    a.unpriced, a.graded
                ));
            }
        }
        let short = self.short_arms();
        lines.push(String::new());
        lines.push(format!(
            "  {:<32} {}",
            "task",
            short
                .iter()
                .map(|name| format!("{name:>12}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        for task in &r.tasks {
            lines.push(format!(
                "  {:<32} {}",
                task,
                r.task_cells(task)
                    .iter()
                    .map(|(passes, graded)| format!("{:>12}", format!("{passes}/{graded}")))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        lines.push(String::new());
        lines.extend(self.verdict.lines());
        if self.stop_early == Some(false) {
            lines.push(
                "  The scheduler runs with --no-stop-early; this verdict is advisory.".to_owned(),
            );
        }
        for stop in &self.ledger_stops {
            lines.push(format!(
                "  recorded stop at {}: {} {}, {} trials skipped: {}",
                stop.at,
                stop.arm.as_deref().unwrap_or("experiment"),
                stop.state.replace('_', " "),
                stop.skipped,
                stop.reason
            ));
        }
        lines
    }

    /// The pulse as terminal text: the standing, then component health and
    /// notable trials.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = self.standing_lines();
        lines.push(String::new());
        lines.extend(health_lines(
            "Component health",
            self.composed(),
            &self.signals,
            &self.components,
            &self.notable,
        ));
        let parallel = parallel_lines(&self.trials);
        if !parallel.is_empty() {
            lines.push(String::new());
            lines.extend(parallel);
        }
        lines
    }
}

/// One line per trial whose Microluna suite loop recorded a timeline: the
/// wall time against the summed session time, the suite's share of the
/// critical path, and what running sessions at once saved.
#[must_use]
pub fn parallel_lines(trials: &[TrialFacts]) -> Vec<String> {
    let mut lines = Vec::new();
    for trial in trials {
        let Some(composition) = &trial.composition else {
            continue;
        };
        for branch in composition["branches"].as_array().into_iter().flatten() {
            let parallel = &branch["parallel"];
            if !parallel.is_object() {
                continue;
            }
            let secs = |key: &str| {
                parallel[key].as_u64().map_or_else(
                    || "?".to_owned(),
                    |ms| format!("{:.0}s", ms as f64 / 1000.0),
                )
            };
            lines.push(format!(
                "  {}: {} sessions, wall {}, session time {}, concurrency {}, peak {}; suite {} ({} on the critical path); saved {}; {} merges, {} conflicts; cached {}",
                trial.job,
                parallel["sessions"].as_u64().unwrap_or(0),
                secs("wall_ms"),
                secs("session_ms"),
                parallel["concurrency"]
                    .as_f64()
                    .map_or("?".to_owned(), |c| format!("{c:.2}×")),
                parallel["peak"].as_u64().unwrap_or(0),
                secs("suite_ms"),
                secs("suite_on_critical_path_ms"),
                secs("saved_ms"),
                parallel["merges"].as_u64().unwrap_or(0),
                parallel["conflicts"].as_u64().unwrap_or(0),
                parallel["cached_share"]
                    .as_f64()
                    .map_or("?".to_owned(), |c| format!("{:.0}%", c * 100.0)),
            ));
        }
    }
    if !lines.is_empty() {
        lines.insert(0, "Parallel sessions".to_owned());
    }
    lines
}

/// Component health and notable trials as terminal text.
#[must_use]
pub fn health_lines(
    title: &str,
    composed: usize,
    signals: &[Signal],
    components: &[Component],
    notable: &[Notable],
) -> Vec<String> {
    let mut lines = vec![format!(
        "{title}: {composed} graded trials ran Coder One with a composition record"
    )];
    if composed > 0 {
        for signal in signals {
            lines.extend(signal.lines());
        }
        for component in components.iter().filter(|c| c.configured > 0) {
            lines.extend(component.lines());
        }
    }
    lines.push(String::new());
    lines.push("Notable trials".to_owned());
    if notable.is_empty() {
        lines.push("  none yet".to_owned());
    }
    for item in notable.iter().take(NOTABLE_LIMIT) {
        lines.push(format!("  {}: {}", item.job, item.why));
    }
    if notable.len() > NOTABLE_LIMIT {
        lines.push(format!(
            "  and {} more in --json",
            notable.len() - NOTABLE_LIMIT
        ));
    }
    lines
}

/// Several experiments' component health, over all their composed trials.
#[derive(Clone, Debug)]
pub struct Combined {
    pub experiments: Vec<String>,
    pub composed: usize,
    pub signals: Vec<Signal>,
    pub components: Vec<Component>,
    pub notable: Vec<Notable>,
}

impl Combined {
    /// Pools the pulses' composed trials.
    #[must_use]
    pub fn of(pulses: &[Pulse]) -> Self {
        let composed: Vec<&TrialFacts> = pulses
            .iter()
            .flat_map(|pulse| pulse.trials.iter())
            .filter(|t| t.graded() && t.composition.is_some())
            .collect();
        Combined {
            experiments: pulses.iter().map(|p| p.report.id.clone()).collect(),
            composed: composed.len(),
            signals: signals(&composed),
            components: components(&composed),
            notable: pulses
                .iter()
                .flat_map(|pulse| pulse.notable.iter().cloned())
                .collect(),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "experiments": self.experiments,
            "composed_graded": self.composed,
            "signals": self.signals.iter().map(Signal::to_json).collect::<Vec<_>>(),
            "components": self.components.iter().map(Component::to_json).collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        health_lines(
            &format!(
                "Combined component health of {}",
                self.experiments.join(", ")
            ),
            self.composed,
            &self.signals,
            &self.components,
            &self.notable,
        )
    }
}

/// Arm names with the prefix they all share cut, such as `v3` for
/// `coder-one-tunable-v3`.
#[must_use]
pub fn short_names(arms: &[String]) -> Vec<String> {
    let split: Vec<Vec<&str>> = arms.iter().map(|arm| arm.split('-').collect()).collect();
    let shared = (0..)
        .take_while(|&i| {
            split.len() > 1
                && split.iter().all(|parts| parts.len() > i + 1)
                && split.iter().all(|parts| parts[i] == split[0][i])
        })
        .count();
    split
        .iter()
        .map(|parts| {
            let name = parts[shared..].join("-");
            let name = ["coder-one-tunable-", "coder-one-", "claude-code-"]
                .into_iter()
                .find_map(|prefix| name.strip_prefix(prefix))
                .filter(|_| name.chars().count() > SHORT)
                .unwrap_or(&name);
            name.chars().take(SHORT).collect()
        })
        .collect()
}

/// The widest short arm name.
const SHORT: usize = 12;

/// One experiment in the experiments directory.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: String,
    pub state: String,
    pub updated_at: String,
    pub graded: u64,
    pub trials: u64,
    pub status: PathBuf,
}

/// Every experiment with a status file, the most recently updated first.
#[must_use]
pub fn list(dir: &Path) -> Vec<Listed> {
    let mut out: Vec<Listed> = children(dir)
        .into_iter()
        .filter_map(|path| {
            let status = path.join("status.json");
            let value = read_json(&status)?;
            if value["schema"] != experiment::STATUS_SCHEMA {
                return None;
            }
            Some(Listed {
                id: value["experiment"].as_str().unwrap_or_default().to_owned(),
                state: value["state"].as_str().unwrap_or("unknown").to_owned(),
                updated_at: value["updated_at"].as_str().unwrap_or_default().to_owned(),
                graded: value["graded"].as_u64().unwrap_or(0),
                trials: value["trials"].as_array().map_or(0, |t| t.len() as u64),
                status,
            })
        })
        .collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
    out
}

/// Where `tbench` keeps trial jobs: `TBENCH_STATE_DIR` or
/// `~/.openagents/terminal-bench`, then `jobs/`.
#[must_use]
pub fn default_jobs_dir() -> PathBuf {
    experiment::default_dir()
        .parent()
        .map_or_else(|| PathBuf::from("jobs"), |state| state.join("jobs"))
}

const HELP: &str = "gym experiment pulse ID|PATH... [--json] [--jev] [--live]
gym experiment replay ID|PATH... [--json]
gym experiment list

With several experiments, pulse prints each one's standing and then the
component health of all their trials pooled.

pulse    How a targeted experiment stands, from its status file, every
         finished trial's verifier reward and attempt record, and Coder
         One's composition records, with no model call: each arm's passes
         with 95% Wilson intervals, cost, and time; how the final checks,
         Jev's support answers, and the effort score line up with the
         verifier; how often escalation, repair, and persistence fired and
         how those trials ended; the early-stopping verdict; and notable
         trials.
         --jev   also ask Jev about newly finished trials: `gym runs rank`'s
                 18 judgments, grouped by arm, and three experiment
                 questions (did escalation change the candidate, did the
                 effort level plausibly matter, was the failure a near
                 miss). Answers are kept, so a trial is asked once. The cost
                 is reported.
         --live  also ask Jev, advisory only, whether each running trial is
                 looping, stalled on transport, or done but still spending.
                 Nothing is stopped.
replay   Run the recorded trials back through the early-stopping rule in
         the order they finished: where it would have stopped each arm and
         the experiment, and the trials and dollars that saves.
list     Every experiment with a status file, most recently updated first.

  --experiments-dir PATH   default ~/.openagents/terminal-bench/experiments
  --jobs-dir PATH          default ~/.openagents/terminal-bench/jobs
  --alpha P                the rule's significance level (default: the
                           experiment's, or 0.05)
  --accept-pass-rate R     the acceptance bar, a pass rate from 0 to 1
  --recorded FILE          answer Jev questions from a recorded file
  --pulse-dir PATH         where experiment answers are kept
                           (default ~/.openagents/gym/pulse)
  --learning-dir PATH      where `gym runs rank` answers are kept
                           (default ~/.openagents/gym/learning)
  --no-reference           leave the TB4 leaderboard out of Jev's state
  --json                   print versioned JSON";

/// `gym experiment`.
///
/// # Errors
/// Returns a message for a bad argument, an unreadable status file, or a
/// failed write.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        writeln!(out, "{HELP}").map_err(|e| e.to_string())?;
        return Ok(0);
    };
    let mut dir = experiment::default_dir();
    let mut jobs = default_jobs_dir();
    let mut queries: Vec<String> = Vec::new();
    let (mut json_out, mut jev, mut live, mut reference) = (false, false, false, true);
    let mut alpha = None;
    let mut bar = None;
    let mut recorded: Option<PathBuf> = None;
    let mut pulse_dir = crate::terminal_bench_pulse_jev::default_dir();
    let mut learning_dir = crate::runs_learning::default_dir();
    let mut index = 0;
    let value = |index: usize| {
        rest.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("{} needs a value", rest[index]))
    };
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_out = true,
            "--jev" => jev = true,
            "--live" => live = true,
            "--no-reference" => reference = false,
            "--experiments-dir" => {
                dir = PathBuf::from(value(index)?);
                index += 1;
            }
            "--jobs-dir" => {
                jobs = PathBuf::from(value(index)?);
                index += 1;
            }
            "--recorded" => {
                recorded = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--pulse-dir" => {
                pulse_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--learning-dir" => {
                learning_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--alpha" => {
                alpha = Some(
                    value(index)?
                        .parse::<f64>()
                        .ok()
                        .filter(|a| *a > 0.0 && *a < 1.0)
                        .ok_or("--alpha needs a number between 0 and 1")?,
                );
                index += 1;
            }
            "--accept-pass-rate" => {
                bar = Some(
                    value(index)?
                        .parse::<f64>()
                        .ok()
                        .filter(|r| (0.0..=1.0).contains(r))
                        .ok_or("--accept-pass-rate needs a number from 0 to 1")?,
                );
                index += 1;
            }
            "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown option {flag}\n\n{HELP}"));
            }
            other => queries.push(other.to_owned()),
        }
        index += 1;
    }
    let write = |out: &mut dyn std::io::Write, text: &str| -> Result<(), String> {
        writeln!(out, "{text}").map_err(|e| e.to_string())
    };
    match verb.as_str() {
        "list" => {
            let listed = list(&dir);
            if json_out {
                let value = json!({
                    "schema": "openagents.gym.experiment-list.v1",
                    "experiments": listed.iter().map(|l| json!({
                        "experiment": l.id, "state": l.state, "updated_at": l.updated_at,
                        "graded": l.graded, "trials": l.trials, "status": l.status,
                    })).collect::<Vec<_>>(),
                });
                write(
                    out,
                    &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                )?;
            } else {
                for l in listed {
                    write(
                        out,
                        &format!(
                            "{:<28} {:<9} {:>3}/{:<3} graded  updated {}",
                            l.id, l.state, l.graded, l.trials, l.updated_at
                        ),
                    )?;
                }
            }
            Ok(0)
        }
        "pulse" | "replay" => {
            if queries.is_empty() {
                return Err(format!("{verb} needs an experiment id or path"));
            }
            let mut pulses = Vec::new();
            for query in &queries {
                let mut pulse =
                    Pulse::load(&experiment::status_path(query, &dir), Some(&jobs), None)?;
                if alpha.is_some() || bar.is_some() {
                    pulse.set_rule(Rule {
                        alpha: alpha.unwrap_or(pulse.rule.alpha),
                        accept_pass_rate: bar.or(pulse.rule.accept_pass_rate),
                    });
                }
                pulses.push(pulse);
            }
            if verb == "replay" {
                let mut values = Vec::new();
                for (index, pulse) in pulses.iter().enumerate() {
                    let replay = pulse.replay();
                    if json_out {
                        let mut value = replay.to_json();
                        value["experiment"] = json!(pulse.report.id);
                        values.push(value);
                    } else {
                        if index > 0 {
                            write(out, "")?;
                        }
                        write(out, &format!("Experiment {}", pulse.report.id))?;
                        for line in replay.lines() {
                            write(out, &line)?;
                        }
                    }
                }
                if json_out {
                    let value = if values.len() == 1 {
                        values.remove(0)
                    } else {
                        Value::Array(values)
                    };
                    write(
                        out,
                        &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                    )?;
                }
                return Ok(0);
            }
            let judge = if !(jev || live) {
                None
            } else if let Some(path) = &recorded {
                Some(crate::runs_learning::Judge::Recorded(
                    crate::runs_learning::Recorded::load(path)?,
                ))
            } else {
                Some(crate::runs_learning::Judge::from_environment())
            };
            let mut values = Vec::new();
            let mut text = Vec::new();
            let several = pulses.len() > 1;
            for pulse in &pulses {
                let learning = match (&judge, jev) {
                    (Some(judge), true) => Some(crate::terminal_bench_pulse_jev::learn_with(
                        pulse,
                        judge,
                        crate::runs::Sources {
                            jobs: Some(jobs.clone()),
                            ..crate::runs::Sources::standard()
                        },
                        reference,
                        pulse_dir.clone(),
                        learning_dir.clone(),
                    )?),
                    _ => None,
                };
                let advice = match (&judge, live) {
                    (Some(judge), true) => {
                        Some(crate::terminal_bench_pulse_jev::advise(pulse, judge)?)
                    }
                    _ => None,
                };
                let mut value = pulse.to_json();
                value["model_calls"] = json!(
                    learning.as_ref().map_or(0, |l| l.asked())
                        + advice.as_ref().map_or(0, |a| a.asked)
                );
                if !text.is_empty() {
                    text.push(String::new());
                }
                if several {
                    text.extend(pulse.standing_lines());
                } else {
                    text.extend(pulse.lines());
                }
                if let Some(learning) = &learning {
                    value["jev"] = learning.to_json();
                    text.push(String::new());
                    text.extend(learning.lines());
                }
                if let Some(advice) = &advice {
                    value["live"] = advice.to_json();
                    text.push(String::new());
                    text.extend(advice.lines());
                }
                values.push(value);
            }
            let combined = several.then(|| Combined::of(&pulses));
            if let Some(combined) = &combined {
                text.push(String::new());
                text.extend(combined.lines());
            }
            if json_out {
                let value = match combined {
                    Some(combined) => json!({
                        "schema": SCHEMA,
                        "experiments": values,
                        "combined": combined.to_json(),
                    }),
                    None => values.remove(0),
                };
                write(
                    out,
                    &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                )?;
            } else {
                for line in text {
                    write(out, &line)?;
                }
            }
            Ok(0)
        }
        "--help" | "-h" | "help" => {
            write(out, HELP)?;
            Ok(0)
        }
        other => Err(format!("unknown command {other}\n\n{HELP}")),
    }
}

/// A small experiment on disk for tests: two arms, two tasks, two attempts.
#[cfg(test)]
pub(crate) mod fixture {
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};

    fn write(path: &Path, value: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    /// One trial's files, and its status row.
    #[allow(clippy::too_many_arguments)]
    fn trial(
        jobs: &Path,
        arm: &str,
        task: &str,
        attempt: u64,
        state: &str,
        reward: Option<f64>,
        cost: f64,
        composition: Option<Value>,
        tests: Option<(u64, u64)>,
        minute: u32,
    ) -> Value {
        let job = format!("tb4--{arm}--{task}--x-r{attempt}");
        let name = format!("{task}__{arm}{attempt}");
        let dir = jobs.join(&job).join(&name);
        write(&dir.join("config.json"), &json!({}));
        if state == "finished" {
            write(
                &dir.join("result.json"),
                &json!({"verifier_result": {"rewards": {"reward": reward}}}),
            );
            write(
                &jobs
                    .join(&job)
                    .join("tbench/attempts")
                    .join(format!("{name}.json")),
                &json!({"cost": {"amount_usd": cost}, "timing": {"total_ms": 60_000}}),
            );
        }
        if let Some(record) = composition {
            write(
                &dir.join("agent/episode/artifacts/composition.json"),
                &record,
            );
        }
        if let Some((passed, total)) = tests {
            let failing: Vec<Value> = (passed..total)
                .map(|i| json!({"name": format!("test_{i}"), "status": "failed"}))
                .collect();
            write(
                &dir.join("verifier/ctrf.json"),
                &json!({"results": {"summary": {"tests": total, "passed": passed}, "tests": failing}}),
            );
        }
        json!({
            "arm": arm, "task": task, "attempt": attempt, "job": job, "state": state,
            "reward": reward,
            "started_at": format!("2026-09-24T01:{minute:02}:00+00:00"),
            "finished_at": (state == "finished").then(|| format!("2026-09-24T01:{minute:02}:30+00:00")),
            "claude_usage": {"usd": cost, "sessions": 1, "unpriced": 0},
            "losses": [],
        })
    }

    fn composition(checks: Value, effort: (f64, &str), second: Value) -> Value {
        json!({
            "schema": "openagents.coder-one.composition.v3",
            "effort": {"score": effort.0, "at": 0.4, "effort": effort.1},
            "final_checks": {"verdicts": checks},
            "support": {"judged": 2, "supported": 2, "contradicted": 0, "unresolved": 0},
            "second": second,
            "repair": {"ran": false, "skipped": "no check contradicted a requirement"},
            "persist": {"rounds": [{"round": 1}], "stopped": "round 1 changed nothing", "totals": {"cost_usd": 0.5}},
            "escalated": false,
        })
    }

    /// Writes the experiment; returns the experiments and jobs directories.
    pub(crate) fn write_experiment(root: &Path) -> (PathBuf, PathBuf) {
        let jobs = root.join("jobs");
        let experiments = root.join("experiments");
        let skipped = json!({"skipped": "no check failed and the executor reported no failure"});
        let ran = json!({"kept": "second", "fired": ["check"], "cost_usd": 0.7});
        let trials = vec![
            trial(
                &jobs,
                "base",
                "t1",
                1,
                "finished",
                Some(1.0),
                1.0,
                None,
                Some((5, 5)),
                1,
            ),
            trial(
                &jobs,
                "cand",
                "t1",
                1,
                "finished",
                Some(1.0),
                2.0,
                Some(composition(json!({"passed": 3}), (0.8, "xhigh"), skipped)),
                Some((5, 5)),
                2,
            ),
            trial(
                &jobs,
                "base",
                "t1",
                2,
                "finished",
                Some(0.0),
                1.0,
                None,
                Some((2, 10)),
                3,
            ),
            trial(
                &jobs,
                "cand",
                "t1",
                2,
                "finished",
                Some(0.0),
                2.0,
                Some(composition(json!({"passed": 2}), (0.2, "medium"), ran)),
                Some((9, 10)),
                4,
            ),
            trial(
                &jobs,
                "base",
                "t2",
                1,
                "finished",
                Some(1.0),
                1.0,
                None,
                None,
                5,
            ),
            trial(&jobs, "cand", "t2", 1, "running", None, 0.0, None, None, 6),
            trial(&jobs, "base", "t2", 2, "pending", None, 0.0, None, None, 7),
            trial(&jobs, "cand", "t2", 2, "pending", None, 0.0, None, None, 8),
        ];
        write(
            &experiments.join("x/status.json"),
            &json!({
                "schema": crate::terminal_bench_experiment::STATUS_SCHEMA,
                "experiment": "x", "profile": "tb4", "state": "running",
                "updated_at": "2026-09-24T01:10:00+00:00",
                "arms": [{"id": "base"}, {"id": "cand"}],
                "tasks": ["t1", "t2"], "attempts": 2, "graded": 5,
                "quota": {"budget_usd": null, "used": {"usd": 8.0}},
                "stop_early": {"enabled": true, "alpha": 0.05, "accept_pass_rate": null},
                "trials": trials,
            }),
        );
        std::fs::write(
            experiments.join("x/ledger.jsonl"),
            "{\"schema\": \"openagents.tbench.experiment-ledger.v1\", \"event\": \"loss\", \"job\": \"j\"}\n",
        )
        .unwrap();
        (experiments, jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_before_the_agent_comes_from_the_attempt_record() {
        let record = serde_json::json!({"timing": {"setup_ms": 9_000, "environment_setup_ms": 1}});
        assert_eq!(setup_ms(&record), Some(9_000));
        // An older record has only the two phases; setup is their sum.
        let older =
            serde_json::json!({"timing": {"environment_setup_ms": 6_400, "agent_setup_ms": 1_500}});
        assert_eq!(setup_ms(&older), Some(7_900));
        let partial = serde_json::json!({"timing": {"environment_setup_ms": 6_400}});
        assert_eq!(setup_ms(&partial), None);
    }

    #[test]
    fn an_unknown_total_never_becomes_a_partial_quota_or_arm_mean() {
        let temp = tempfile::tempdir().unwrap();
        let (experiments, jobs) = fixture::write_experiment(temp.path());
        let job = jobs.join("tb4--cand--t1--x-r1");
        std::fs::write(
            job.join("tbench/attempts/t1__cand1.json"),
            r#"{"cost":{"amount_usd":null,"lower_bound_usd":2.0,"unknown_calls":1}}"#,
        )
        .unwrap();
        // Harbor and Claude report executor-only costs. Neither completes
        // an explicitly unknown whole-trial record.
        std::fs::write(
            job.join("t1__cand1/result.json"),
            r#"{"agent_result":{"cost_usd":2.0}}"#,
        )
        .unwrap();
        let pulse = Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        let trial = pulse
            .trials
            .iter()
            .find(|t| t.job == "tb4--cand--t1--x-r1")
            .unwrap();
        assert!(trial.quota_usd > 0.0);
        assert_eq!(trial.cost_usd, None);
        let arm = &pulse.arms[1];
        assert_eq!(arm.unpriced, 1);
        assert_eq!(arm.mean_cost, None);
        assert_eq!(arm.total_cost, 2.0);
        let input = rule_input(&pulse.report, &pulse.trials, &[]);
        assert!(!input.mean_cost.contains_key("cand"));
        assert_eq!(input.mean_cost["base"], 1.0);
        assert!(pulse.to_json()["arms"][1]["total_cost_usd"].is_null());
        assert_eq!(pulse.to_json()["arms"][1]["priced_total_cost_usd"], 2.0);
        assert!(
            pulse
                .lines()
                .join("\n")
                .contains("1 of 2 graded trials have unknown total cost")
        );
        std::fs::write(job.join("tbench/attempts/t1__cand1.json"), "{incomplete").unwrap();
        let incomplete =
            Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        assert_eq!(incomplete.arms[1].mean_cost, None);
        // With no attempt record, a native Harbor total remains readable.
        std::fs::remove_file(job.join("tbench/attempts/t1__cand1.json")).unwrap();
        let native = Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        assert_eq!(native.arms[1].mean_cost, Some(2.0));
    }

    #[test]
    fn per_check_discrimination_counts_candidates_once_and_rejects_other_versions() {
        let temp = tempfile::tempdir().unwrap();
        let (experiments, jobs) = fixture::write_experiment(temp.path());
        let pulse = Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        let mut trial = pulse
            .trials
            .iter()
            .find(|t| t.graded() && t.composition.is_some() && !t.passed())
            .unwrap()
            .clone();
        let record = trial.composition.as_mut().unwrap();
        record["final_checks"]["candidate"] = json!("selected");
        record["checks"] =
            json!([{"summary":{"candidate":"selected"}, "file":"verification/checks.json"}]);
        let report_path = trial
            .trial_dir
            .as_ref()
            .unwrap()
            .join("agent/episode/verification/checks.json");
        std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        let mut report = json!({"candidate":{"digest":"selected"}, "scenarios":[
            {"id":"a", "kind":"behavior.example"}, {"id":"b", "kind":"behavior.example"}],
            "verdicts":[{"scenario":"a","verdict":"passed"},{"scenario":"b","verdict":"failed"}]});
        std::fs::write(&report_path, report.to_string()).unwrap();
        let measured = per_check_signal(&[&trial]);
        assert_eq!(measured.rows.len(), 1);
        assert_eq!(measured.rows[0].fails, 1);
        assert!(
            measured.rows[0]
                .note
                .as_ref()
                .unwrap()
                .contains("fail precision 1/1")
        );
        report["candidate"]["digest"] = json!("discarded");
        std::fs::write(&report_path, report.to_string()).unwrap();
        assert!(per_check_signal(&[&trial]).rows.is_empty());
    }

    #[test]
    fn a_pulse_reads_arms_signals_components_and_notables() {
        let temp = tempfile::tempdir().unwrap();
        let (experiments, jobs) = fixture::write_experiment(temp.path());
        let pulse = Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        let base = &pulse.arms[0];
        assert_eq!((base.passes, base.graded, base.pending), (2, 3, 1));
        assert_eq!(base.mean_cost, Some(1.0));
        let cand = &pulse.arms[1];
        assert_eq!((cand.passes, cand.graded, cand.running), (1, 2, 1));
        assert_eq!(cand.total_cost, 4.0);
        assert_eq!(pulse.composed(), 2);
        let checks = &pulse.signals[0];
        assert_eq!(checks.name, "Final checks");
        assert_eq!(
            (
                checks.rows[0].label.as_str(),
                checks.rows[0].passes,
                checks.rows[0].fails
            ),
            ("all passed", 1, 1)
        );
        let effort = &pulse.signals[2];
        assert_eq!(effort.auc, Some((1.0, 1, 1)));
        let escalation = &pulse.components[0];
        assert_eq!((escalation.configured, escalation.fired), (2, 1));
        assert!(
            escalation
                .notes
                .iter()
                .any(|n| n == "ran 1 times with 0 passes; second executors cost $0.70")
        );
        let persist = &pulse.components[2];
        assert_eq!(persist.rows[0].label, "stopped: round N changed nothing");
        let miss = pulse
            .notable
            .iter()
            .find(|n| n.job == "tb4--cand--t1--x-r2")
            .unwrap();
        assert!(
            miss.why
                .contains("every final check passed and the verifier failed it")
        );
        assert!(
            miss.why
                .contains("a near miss: 9 of 10 verifier tests passed; failing test_9")
        );
        assert!(miss.why.contains("escalation kept the second candidate"));
        // Two tasks × two attempts can't separate: exploratory, never stopped.
        assert!(!pulse.verdict.ended);
        assert_eq!(pulse.verdict.comparisons[0].state, "exploratory");
        assert_eq!(pulse.stop_early, Some(true));
        let value = pulse.to_json();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["model_calls"], 0);
        assert_eq!(value["arms"][1]["graded"], 2);
        let text = pulse.lines().join("\n");
        assert!(text.contains("Pulse of x (tb4): 2 arms"));
        assert!(text.contains("Escalation (verify.second): configured on 2, ran on 1 (50%)"));
    }

    #[test]
    fn the_command_prints_pulse_replay_and_list() {
        let temp = tempfile::tempdir().unwrap();
        let (experiments, jobs) = fixture::write_experiment(temp.path());
        let run = |args: &[&str]| {
            let mut all: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
            all.extend([
                "--experiments-dir".to_owned(),
                experiments.to_string_lossy().into_owned(),
                "--jobs-dir".to_owned(),
                jobs.to_string_lossy().into_owned(),
            ]);
            let mut out = Vec::new();
            let code = command(&all, &mut out).unwrap();
            (code, String::from_utf8(out).unwrap())
        };
        let (code, text) = run(&["pulse", "x"]);
        assert_eq!(code, 0);
        assert!(text.contains("Component health: 2 graded trials"));
        let (_, json_text) = run(&["pulse", "x", "--json"]);
        let value: Value = serde_json::from_str(&json_text).unwrap();
        assert_eq!(value["experiment"], "x");
        let (_, both) = run(&["pulse", "x", "x"]);
        assert!(both.contains("Combined component health of x, x: 4 graded trials"));
        let (_, replay) = run(&["replay", "x", "--accept-pass-rate", "0.9"]);
        // With a 90% bar, the candidate's first failure leaves it at most 3
        // of 4: it stops and the experiment ends. The baseline's attempt
        // that started later is saved, and three never finished anyway.
        assert!(replay.contains("stop cand below bar"), "{replay}");
        assert!(
            replay.contains("saved: tb4--base--t2--x-r1 ($1.00)"),
            "{replay}"
        );
        assert!(replay.contains("3 more that never finished"), "{replay}");
        let (_, listed) = run(&["list"]);
        assert!(listed.starts_with("x "));
        assert!(command(&["pulse".to_owned()], &mut Vec::new()).is_err());
    }

    #[test]
    fn fisher_matches_known_tables() {
        // A classic tea-tasting table: [[3, 1], [1, 3]] has p = 0.486.
        assert!((fisher_two_sided(3, 1, 1, 3) - 0.485_714).abs() < 1e-5);
        assert!((fisher_two_sided(19, 19, 19, 21) - 1.0).abs() < 1e-9);
        assert!(fisher_two_sided(10, 0, 0, 10) < 0.001);
        assert!((fisher_two_sided(0, 0, 3, 4) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn auc_counts_ties_half() {
        assert_eq!(auc(&[(0.9, true), (0.1, false)]), Some(1.0));
        assert_eq!(auc(&[(0.5, true), (0.5, false)]), Some(0.5));
        assert_eq!(auc(&[(0.2, true), (0.8, false), (0.6, true)]), Some(0.0));
        assert_eq!(auc(&[(0.2, true)]), None);
    }

    #[test]
    fn stop_reasons_group_by_their_words() {
        assert_eq!(
            normalize("round 2 changed nothing"),
            "round N changed nothing"
        );
        assert_eq!(
            normalize(
                "the next round would cost about $2.91, over the $0.49 left of the $3.82 cap"
            ),
            "the next round would cost about $N, over the $N left of the $N cap"
        );
        assert_eq!(
            normalize("reached the cap of 2 rounds"),
            "reached the cap of N rounds"
        );
        assert_eq!(normalize("ended at 12."), "ended at N.");
        assert_eq!(
            normalize("over 256 MiB or 20,000 files"),
            "over N MiB or N files"
        );
        assert_eq!(normalize("round 3"), "round N");
    }

    #[test]
    fn short_names_cut_the_shared_prefix() {
        let arms = [
            "coder-one-tunable-v3".to_owned(),
            "coder-one-tunable-v9-escalate".to_owned(),
        ];
        assert_eq!(short_names(&arms), vec!["v3", "v9-escalate"]);
        assert_eq!(
            short_names(&["coder".to_owned(), "nop".to_owned()]),
            vec!["coder", "nop"]
        );
    }
}
