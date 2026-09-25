//! Truthful checks: every signal the checks leave, measured against the
//! verifier on retained graded trials.
//!
//! A graded Coder One trial keeps its composition record, each check's
//! report, each support report, and the executors' streams beside the
//! verifier's reward. That makes every retained trial a free label: what
//! the checks said about the candidate the verifier graded, and whether
//! the verifier passed it. This module builds that label set, splits it by
//! task into a calibration half and a held-out half so no task is in
//! both, and measures each signal's discrimination with Wilson intervals:
//!
//! - **Fail precision**: of the trials the signal calls failed, how many
//!   the verifier failed.
//! - **Failure recall**: of the trials the verifier failed, how many the
//!   signal calls failed.
//! - **Pass rate on "pass"**: of the trials the signal calls passed, how
//!   many the verifier passed.
//!
//! A signal reads only the records the episode wrote about its final
//! candidate, the one the verifier graded: never the verifier's tests.
//! [`super::verdict`] fits the combined verdict on the calibration half
//! and states its precision; the held-out half measures it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use super::truth_micro::LocalScore;

/// The schema of one labeled trial.
pub const ROW_SCHEMA: &str = "openagents.coder-one.check-truth-row.v1";

/// The schema of the measurement summary.
pub const SUMMARY_SCHEMA: &str = "openagents.coder-one.check-truth.v1";

/// Which half of the label set a trial is in, by its task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Split {
    /// The verdict is fitted here.
    Calibration,
    /// The verdict is measured here and never fitted on.
    HeldOut,
}

impl Split {
    /// The split's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Split::Calibration => "calibration",
            Split::HeldOut => "held-out",
        }
    }
}

/// The split a task falls in: the parity of the last hex digit of the
/// digest of its name. Fixed before any signal was measured, so the split
/// can't be tuned to the result.
#[must_use]
pub fn split_of(task: &str) -> Split {
    let task = canonical_task(task);
    let digest = atif::digest(&json!({ "task": task }));
    let last = digest
        .chars()
        .last()
        .and_then(|c| c.to_digit(16))
        .unwrap_or(0);
    if last.is_multiple_of(2) {
        Split::Calibration
    } else {
        Split::HeldOut
    }
}

/// Bare and namespaced Terminal-Bench names must share one partition.
#[must_use]
pub fn canonical_task(task: &str) -> String {
    if task.contains('/') {
        task.to_string()
    } else {
        format!("terminal-bench/{task}")
    }
}

/// Scenario verdicts of one kind on the final candidate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub passed: usize,
    pub failed: usize,
    /// Inconclusive, unavailable, or anything else.
    pub other: usize,
}

/// `verify.support`'s states on the final candidate.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SupportTally {
    pub supported: usize,
    pub contradicted: usize,
    pub unresolved: usize,
    /// The mean `supports` probability over the judged requirements.
    pub mean_supports: Option<f64>,
    /// The largest `contradicts` probability.
    pub max_contradicts: Option<f64>,
}

/// One graded trial and what its records say about its final candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    pub job: String,
    pub trial: String,
    pub task: String,
    /// The arm, such as `coder-one-tunable-v9`.
    pub arm: String,
    /// The first executor's agent and model.
    pub agent: Option<String>,
    pub model: Option<String>,
    /// The verifier's reward: the label.
    pub reward: f64,
    pub split: Split,
    /// Scenario verdicts on the final candidate, by scenario kind.
    pub scenarios: BTreeMap<String, Tally>,
    /// Requirement states on the final candidate.
    pub requirements: BTreeMap<String, usize>,
    /// Support states on the final candidate, when support ran.
    pub support: Option<SupportTally>,
    /// The self-report the episode recorded on the final candidate:
    /// `failed` or `inconclusive`, when the policy ran it.
    pub self_report: Option<String>,
    /// The self-report on the first candidate.
    pub self_report_first: Option<String>,
    /// The self-report detector rerun on the final report, for every trial
    /// whatever its policy: the kinds of admission it found.
    pub admissions: Vec<String>,
    /// Whether the first check failed a scenario.
    pub first_flagged: bool,
    /// Whether the episode ran a repair, a second executor, or persistence.
    pub repaired: bool,
    pub second_fired: bool,
    pub persisted: bool,
    /// Whether the first executor ended other than answered.
    pub executor_unfinished: bool,
    /// The final report's length.
    pub report_chars: usize,
    /// Jev's answers to [`super::verdict`]'s report questions, by question
    /// ID, when they were asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_answers: Option<BTreeMap<String, f64>>,
    /// The self-written score of the selected Microluna candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_score: Option<LocalScore>,
    /// Retained file and session that supplied the report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_unavailable: Option<String>,
    /// Read-only review sessions proven to observe the same candidate files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_sessions: Vec<u64>,
}

impl Row {
    /// Whether the verifier failed the trial.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.reward < 1.0
    }
}

/// A trial loaded from disk, with the texts the report questions read.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub row: Row,
    pub instruction: Option<String>,
    pub report: Option<String>,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The final report of the session that produced the kept candidate: the
/// last session's, or the first's when a second executor ran and its
/// candidate was set aside.
fn final_report(episode: &Path, composition: &Value) -> super::truth_micro::Selected {
    let mut records: Vec<(usize, PathBuf, bool)> = children(&episode.join("artifacts"))
        .into_iter()
        .filter_map(|p| {
            let n = name(&p);
            let (index, micro) = if let Some(index) = n
                .strip_prefix("delegate-")
                .and_then(|s| s.strip_suffix(".stream.jsonl"))
            {
                (index.parse::<usize>().ok()?, false)
            } else {
                (
                    n.strip_prefix("microluna-")?
                        .strip_suffix(".json")?
                        .parse::<usize>()
                        .ok()?,
                    true,
                )
            };
            Some((index, p, micro))
        })
        .collect();
    records.sort_by_key(|r| r.0);
    let chosen = if composition["second"]["kept"] == "first" {
        records.first()
    } else {
        records.last()
    };
    let Some((_, path, micro)) = chosen else {
        return super::truth_micro::Selected {
            unavailable: Some("No executor report is retained".to_string()),
            ..Default::default()
        };
    };
    let source = format!("artifacts/{}", name(path));
    if *micro {
        return read_json(path).map_or_else(
            || super::truth_micro::Selected {
                unavailable: Some("The Microluna record is unreadable".to_string()),
                ..Default::default()
            },
            |record| super::truth_micro::select(&record, &source),
        );
    }
    let report = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| super::replay::final_report(&s));
    super::truth_micro::Selected {
        unavailable: report
            .is_none()
            .then(|| "The executor stream has no final report".to_string()),
        report,
        source: Some(source),
        ..Default::default()
    }
}

/// Reads one trial: `<jobs>/<job>/<trial>` (the episode under
/// `agent/episode`) or a retained `<traces>/<job>/<trial>.episode`.
/// `None` when it isn't a graded Coder One trial with a composition
/// record: no reward, an agent exception, or a usage-limited session.
#[must_use]
pub fn load(job: &str, dir: &Path) -> Option<Loaded> {
    let retained = name(dir).ends_with(".episode");
    let (episode, result, config) = if retained {
        (
            dir.to_path_buf(),
            read_json(&dir.join("harbor-result.json"))?,
            None,
        )
    } else {
        (
            dir.join("agent/episode"),
            read_json(&dir.join("result.json"))?,
            read_json(&dir.join("config.json")),
        )
    };
    let composition = read_json(&episode.join("artifacts/composition.json"))?;
    if !result["exception_info"].is_null() || !composition["usage_limited"].is_null() {
        return None;
    }
    let reward = result
        .pointer("/verifier_result/rewards/reward")
        .and_then(Value::as_f64)?;
    let trial = name(dir).trim_end_matches(".episode").to_string();
    let task = result["task_name"].as_str().map_or_else(
        || trial.split("__").next().unwrap_or_default().to_string(),
        str::to_string,
    );
    let task = canonical_task(&task);
    if !reward.is_finite() || !(0.0..=1.0).contains(&reward) {
        return None;
    }
    let arm = job.split("--").nth(1).unwrap_or_default().to_string();

    let candidate = composition["final_checks"]["candidate"].as_str();
    let checks: Vec<&Value> = composition["checks"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let entry = checks
        .iter()
        .rev()
        .find(|c| candidate.is_some() && c["summary"]["candidate"].as_str() == candidate)
        .or_else(|| candidate.is_none().then(|| checks.last()).flatten())
        .copied();
    let mut scenarios: BTreeMap<String, Tally> = BTreeMap::new();
    let mut requirements: BTreeMap<String, usize> = BTreeMap::new();
    if let Some(report) = entry
        .and_then(|e| e["file"].as_str())
        .filter(|f| {
            Path::new(f)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        })
        .and_then(|f| read_json(&episode.join(f)))
    {
        let kinds: BTreeMap<&str, &str> = report["scenarios"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| Some((s["id"].as_str()?, s["kind"].as_str()?)))
            .collect();
        for verdict in report["verdicts"].as_array().into_iter().flatten() {
            let id = verdict["scenario"].as_str().unwrap_or_default();
            let kind = kinds.get(id).copied().unwrap_or(id);
            let tally = scenarios.entry(kind.to_string()).or_default();
            match verdict["verdict"].as_str() {
                Some("passed") => tally.passed += 1,
                Some("failed") => tally.failed += 1,
                _ => tally.other += 1,
            }
        }
        for covered in report["coverage"].as_array().into_iter().flatten() {
            if let Some(state) = covered["state"].as_str() {
                *requirements.entry(state.to_string()).or_default() += 1;
            }
        }
    }

    let support = children(&episode.join("verification"))
        .into_iter()
        .filter(|p| {
            let n = name(p);
            n.starts_with("support") && n.ends_with(".json")
        })
        .filter_map(|p| read_json(&p))
        .rfind(|s| candidate.is_some() && s["candidate"].as_str() == candidate)
        .map(|s| {
            let states: Vec<&Value> = s["states"].as_array().into_iter().flatten().collect();
            let count = |word: &str| states.iter().filter(|x| x["state"] == word).count();
            let supports: Vec<f64> = states
                .iter()
                .filter_map(|x| x["judgment"]["supports"].as_f64())
                .collect();
            let contradicts = states
                .iter()
                .filter_map(|x| x["judgment"]["contradicts"].as_f64())
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
            SupportTally {
                supported: count("supported"),
                contradicted: count("contradicted"),
                unresolved: count("unresolved"),
                mean_supports: (!supports.is_empty())
                    .then(|| supports.iter().sum::<f64>() / supports.len() as f64),
                max_contradicts: contradicts,
            }
        });

    let self_report = entry.and_then(|e| e["self_report"]["verdict"].as_str().map(str::to_string));
    let self_report_first = checks
        .first()
        .and_then(|e| e["self_report"]["verdict"].as_str().map(str::to_string));
    let first_flagged = checks
        .first()
        .and_then(|e| e["summary"]["verdicts"]["failed"].as_u64())
        .is_some_and(|n| n > 0);
    let selected = final_report(&episode, &composition);
    let report = selected.report;
    let admissions: Vec<String> = report
        .as_deref()
        .map(super::selfreport::admissions)
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.signal)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let primary = composition["branches"]
        .as_array()
        .and_then(|b| b.iter().find(|x| x["role"] == "primary"));
    let instruction = config
        .as_ref()
        .and_then(|c| c["task"]["path"].as_str())
        .and_then(|p| std::fs::read_to_string(Path::new(p).join("instruction.md")).ok())
        .or_else(|| {
            read_json(&episode.join("artifacts/state.json"))
                .and_then(|s| s["issue"]["body"].as_str().map(str::to_string))
        })
        .map(|t| super::labeled::public_instruction(&t));
    Some(Loaded {
        row: Row {
            schema: ROW_SCHEMA.to_string(),
            job: job.to_string(),
            trial,
            split: split_of(&task),
            task,
            arm,
            agent: primary.and_then(|p| p["tier"]["agent"].as_str().map(str::to_string)),
            model: primary.and_then(|p| p["tier"]["model"].as_str().map(str::to_string)),
            reward,
            scenarios,
            requirements,
            support,
            self_report,
            self_report_first,
            admissions,
            first_flagged,
            repaired: composition["repair"]["ran"] == true,
            second_fired: composition["second"]["fired"]
                .as_array()
                .is_some_and(|f| !f.is_empty()),
            persisted: !composition["persist"].is_null(),
            executor_unfinished: primary.is_some_and(|p| p["status"] != "answered"),
            report_chars: report.as_ref().map_or(0, |r| r.chars().count()),
            report_answers: None,
            local_score: selected.score,
            report_source: selected.source,
            report_unavailable: selected.unavailable,
            review_sessions: selected.reviews,
        },
        instruction,
        report,
    })
}

/// Every graded Coder One trial under the jobs directory, then the
/// retained traces a job didn't already give, each trial once.
#[must_use]
pub fn scan(jobs: Option<&Path>, traces: Option<&Path>) -> Vec<Loaded> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    if let Some(jobs) = jobs {
        for job in children(jobs).into_iter().filter(|p| p.is_dir()) {
            let job_name = name(&job);
            for trial in children(&job)
                .into_iter()
                .filter(|p| p.is_dir() && name(p).contains("__"))
            {
                if let Some(loaded) = load(&job_name, &trial)
                    && seen.insert((job_name.clone(), loaded.row.trial.clone()))
                {
                    out.push(loaded);
                }
            }
        }
    }
    if let Some(traces) = traces {
        for job in children(traces).into_iter().filter(|p| p.is_dir()) {
            let job_name = name(&job);
            for trial in children(&job)
                .into_iter()
                .filter(|p| p.is_dir() && name(p).ends_with(".episode"))
            {
                let id = name(&trial).trim_end_matches(".episode").to_string();
                if seen.contains(&(job_name.clone(), id)) {
                    continue;
                }
                if let Some(loaded) = load(&job_name, &trial)
                    && seen.insert((job_name.clone(), loaded.row.trial.clone()))
                {
                    out.push(loaded);
                }
            }
        }
    }
    out
}

/// What a signal says about one trial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Says {
    Fail,
    Pass,
}

/// What a signal says about a row, or nothing.
pub type Reading = Box<dyn Fn(&Row) -> Option<Says>>;

/// One signal: a name, the family it belongs to, and what it says.
pub struct Signal {
    pub id: String,
    pub family: &'static str,
    pub meaning: String,
    pub says: Reading,
}

impl Signal {
    fn new(
        id: impl Into<String>,
        family: &'static str,
        meaning: impl Into<String>,
        says: impl Fn(&Row) -> Option<Says> + 'static,
    ) -> Self {
        Signal {
            id: id.into(),
            family,
            meaning: meaning.into(),
            says: Box::new(says),
        }
    }
}

fn fail_if(condition: bool) -> Option<Says> {
    condition.then_some(Says::Fail)
}

/// Today's checks: failed when a scenario failed on the final candidate,
/// passed when every scenario passed, silent when any was inconclusive.
#[must_use]
pub fn todays_checks(row: &Row) -> Option<Says> {
    let failed: usize = row.scenarios.values().map(|t| t.failed).sum();
    let other: usize = row.scenarios.values().map(|t| t.other).sum();
    let passed: usize = row.scenarios.values().map(|t| t.passed).sum();
    if failed > 0 {
        Some(Says::Fail)
    } else if other == 0 && passed > 0 {
        Some(Says::Pass)
    } else {
        None
    }
}

/// Every signal the records carry, the scenario kinds the rows hold
/// included, then the report questions' answers at 0.5.
#[must_use]
pub fn catalog(rows: &[Row]) -> Vec<Signal> {
    let mut signals = vec![
        Signal::new(
            "checks.final",
            "checks",
            "today's checks on the final candidate: a failed scenario says fail, all passed says pass",
            todays_checks,
        ),
        Signal::new(
            "checks.first-flagged",
            "checks",
            "the first check failed a scenario",
            |r| fail_if(r.first_flagged),
        ),
        Signal::new(
            "checks.scenario-failed",
            "checks",
            "a scenario other than the self-report failed on the final candidate",
            |r| {
                fail_if(
                    r.scenarios
                        .iter()
                        .any(|(k, t)| k != "generic.self-report" && t.failed > 0),
                )
            },
        ),
    ];
    let kinds: BTreeSet<&String> = rows.iter().flat_map(|r| r.scenarios.keys()).collect();
    for kind in kinds {
        let k = kind.clone();
        signals.push(Signal::new(
            format!("scenario.{kind}"),
            "scenario",
            format!("`{kind}` failed says fail; every `{kind}` passed says pass"),
            move |r| {
                let t = r.scenarios.get(&k)?;
                if t.failed > 0 {
                    Some(Says::Fail)
                } else if t.passed > 0 && t.other == 0 {
                    Some(Says::Pass)
                } else {
                    None
                }
            },
        ));
    }
    signals.extend([
        Signal::new(
            "requirements.contradicted",
            "requirements",
            "a scenario contradicted a requirement",
            |r| fail_if(r.requirements.get("contradicted").is_some_and(|n| *n > 0)),
        ),
        Signal::new(
            "requirements.unverifiable",
            "requirements",
            "a requirement is unverifiable",
            |r| fail_if(r.requirements.get("unverifiable").is_some_and(|n| *n > 0)),
        ),
        Signal::new(
            "requirements.observed-none-contradicted",
            "requirements",
            "a requirement was observed and none contradicted says pass",
            |r| {
                (r.requirements.get("observed").is_some_and(|n| *n > 0)
                    && r.requirements.get("contradicted").is_none_or(|n| *n == 0))
                .then_some(Says::Pass)
            },
        ),
        Signal::new(
            "support.contradicted",
            "support",
            "Jev support read a requirement as contradicted",
            |r| fail_if(r.support.as_ref()?.contradicted > 0),
        ),
        Signal::new(
            "support.supported",
            "support",
            "Jev support read a requirement as supported and none as contradicted",
            |r| {
                let s = r.support.as_ref()?;
                (s.supported > 0 && s.contradicted == 0).then_some(Says::Pass)
            },
        ),
        Signal::new(
            "support.unresolved",
            "support",
            "Jev support left every judged requirement unresolved",
            |r| {
                let s = r.support.as_ref()?;
                fail_if(s.unresolved > 0 && s.supported == 0 && s.contradicted == 0)
            },
        ),
        Signal::new(
            "self-report.recorded",
            "self-report",
            "the episode's self-report on the final candidate failed",
            |r| fail_if(r.self_report.as_deref() == Some("failed")),
        ),
        Signal::new(
            "self-report.first",
            "self-report",
            "the episode's self-report on the first candidate failed",
            |r| fail_if(r.self_report_first.as_deref() == Some("failed")),
        ),
        Signal::new(
            "self-report.detector",
            "self-report",
            "the self-report detector, rerun on the final report of every trial, found an admission",
            |r| fail_if(!r.admissions.is_empty()),
        ),
        Signal::new(
            "control.repaired",
            "control",
            "the episode ran a repair",
            |r| fail_if(r.repaired),
        ),
        Signal::new(
            "control.second-fired",
            "control",
            "a second-executor trigger fired",
            |r| fail_if(r.second_fired),
        ),
    ]);
    for (id, meaning, direction) in super::verdict::REPORT_SIGNALS {
        let key = (*id).to_string();
        let fail_when_true = *direction == Says::Fail;
        signals.push(Signal::new(
            format!("report.{id}"),
            "report",
            format!("Jev on the final report, at 0.5: {meaning}"),
            move |r| {
                let p = *r.report_answers.as_ref()?.get(&key)?;
                let yes = p >= 0.5;
                Some(if yes == fail_when_true {
                    Says::Fail
                } else {
                    Says::Pass
                })
            },
        ));
    }
    let params = super::verdict::fitted();
    signals.push(Signal::new(
        "verdict.combined",
        "verdict",
        "the calibrated combined verdict",
        move |r| super::verdict::judge(&super::verdict::Evidence::of_row(r), &params).says(),
    ));
    let params = super::verdict::fitted();
    signals.push(Signal::new(
        "verdict.corroborated",
        "verdict",
        "the combined failure score corroborated by an admission (experimental)",
        move |r| super::verdict::corroborated(&super::verdict::Evidence::of_row(r), &params).says(),
    ));
    signals.push(Signal::new(
        "microluna.local-score",
        "microluna",
        "the selected candidate's self-written evaluator; a green score is not official acceptance",
        |r| {
            let s = r.local_score.as_ref()?;
            (s.total > 0 && s.passed <= s.total).then_some(if s.passed == s.total {
                Says::Pass
            } else {
                Says::Fail
            })
        },
    ));
    signals
}

/// A proportion with its 95% Wilson interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rate {
    pub k: usize,
    pub n: usize,
    pub p: Option<f64>,
    pub low: Option<f64>,
    pub high: Option<f64>,
}

impl Rate {
    /// `k` of `n` with its Wilson interval at z = 1.96.
    #[must_use]
    pub fn of(k: usize, n: usize) -> Rate {
        if n == 0 {
            return Rate {
                k,
                n,
                p: None,
                low: None,
                high: None,
            };
        }
        let z = 1.96_f64;
        let nf = n as f64;
        let p = k as f64 / nf;
        let d = 1.0 + z * z / nf;
        let c = (p + z * z / (2.0 * nf)) / d;
        let h = z * (p * (1.0 - p) / nf + z * z / (4.0 * nf * nf)).sqrt() / d;
        Rate {
            k,
            n,
            p: Some(p),
            low: Some((c - h).max(0.0)),
            high: Some((c + h).min(1.0)),
        }
    }

    /// `k/n p% (low–high%)`, or `k/n` when `n` is zero.
    #[must_use]
    pub fn text(&self) -> String {
        match (self.p, self.low, self.high) {
            (Some(p), Some(l), Some(h)) => format!(
                "{}/{} {:.0}% ({:.0}–{:.0}%)",
                self.k,
                self.n,
                p * 100.0,
                l * 100.0,
                h * 100.0
            ),
            _ => format!("{}/{}", self.k, self.n),
        }
    }
}

/// One signal's discrimination on one set of trials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measure {
    pub signal: String,
    pub family: String,
    pub meaning: String,
    pub trials: usize,
    pub failures: usize,
    /// Of the trials the signal calls failed, the verifier failed `k`.
    pub fail_precision: Rate,
    /// Of the verifier's failures, the signal called `k` failed.
    pub failure_recall: Rate,
    /// Of the trials the signal calls passed, the verifier passed `k`.
    pub pass_when_pass: Rate,
    /// Of the trials the signal doesn't call failed, the verifier passed
    /// `k`: what silence is worth.
    pub pass_when_not_fail: Rate,
    /// The same fail precision and recall on the tasks where the verifier
    /// both passed and failed a trial: whether the signal tells two
    /// attempts at one task apart, which best-of-N and persistence need.
    pub within_fail_precision: Rate,
    pub within_failure_recall: Rate,
    /// Whether the signal separates on this set: fail precision's lower
    /// bound above the set's failure rate, or pass-when-pass's lower bound
    /// above its pass rate.
    pub separates: bool,
}

/// Measures `signal` on `rows`.
#[must_use]
pub fn measure(signal: &Signal, rows: &[&Row]) -> Measure {
    let failures = rows.iter().filter(|r| r.failed()).count();
    let said: Vec<(Option<Says>, bool)> = rows
        .iter()
        .map(|r| ((signal.says)(r), r.failed()))
        .collect();
    let count =
        |f: &dyn Fn(Option<Says>, bool) -> bool| said.iter().filter(|(s, y)| f(*s, *y)).count();
    let says_fail = count(&|s, _| s == Some(Says::Fail));
    let fail_right = count(&|s, y| s == Some(Says::Fail) && y);
    let says_pass = count(&|s, _| s == Some(Says::Pass));
    let pass_right = count(&|s, y| s == Some(Says::Pass) && !y);
    let not_fail = count(&|s, _| s != Some(Says::Fail));
    let not_fail_right = count(&|s, y| s != Some(Says::Fail) && !y);

    let mut by_task: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in rows {
        let e = by_task.entry(r.task.as_str()).or_default();
        if r.failed() {
            e.0 += 1;
        } else {
            e.1 += 1;
        }
    }
    let mixed: Vec<&&Row> = rows
        .iter()
        .filter(|r| {
            by_task
                .get(r.task.as_str())
                .is_some_and(|(f, p)| *f > 0 && *p > 0)
        })
        .collect();
    let within_failures = mixed.iter().filter(|r| r.failed()).count();
    let within_says = mixed
        .iter()
        .filter(|r| (signal.says)(r) == Some(Says::Fail))
        .count();
    let within_right = mixed
        .iter()
        .filter(|r| r.failed() && (signal.says)(r) == Some(Says::Fail))
        .count();

    let fail_precision = Rate::of(fail_right, says_fail);
    let pass_when_pass = Rate::of(pass_right, says_pass);
    let base_fail = failures as f64 / rows.len().max(1) as f64;
    let separates = fail_precision.low.is_some_and(|l| l > base_fail)
        || pass_when_pass.low.is_some_and(|l| l > 1.0 - base_fail);
    Measure {
        signal: signal.id.clone(),
        family: signal.family.to_string(),
        meaning: signal.meaning.clone(),
        trials: rows.len(),
        failures,
        fail_precision,
        failure_recall: Rate::of(fail_right, failures),
        pass_when_pass,
        pass_when_not_fail: Rate::of(not_fail_right, not_fail),
        within_fail_precision: Rate::of(within_right, within_says),
        within_failure_recall: Rate::of(within_right, within_failures),
        separates,
    }
}

/// The label set's size and spread.
#[must_use]
pub fn spread(rows: &[Row]) -> Value {
    let mut tasks: BTreeMap<&str, (usize, usize, Split)> = BTreeMap::new();
    for r in rows {
        let e = tasks.entry(r.task.as_str()).or_insert((0, 0, r.split));
        if r.failed() {
            e.1 += 1;
        } else {
            e.0 += 1;
        }
    }
    let half = |split: Split| {
        let in_split: Vec<&Row> = rows.iter().filter(|r| r.split == split).collect();
        json!({
            "trials": in_split.len(),
            "passes": in_split.iter().filter(|r| !r.failed()).count(),
            "failures": in_split.iter().filter(|r| r.failed()).count(),
            "tasks": tasks.values().filter(|t| t.2 == split).count(),
            "mixed_tasks": tasks.values().filter(|t| t.2 == split && t.0 > 0 && t.1 > 0).count(),
        })
    };
    let mut arms: BTreeMap<&str, usize> = BTreeMap::new();
    let mut models: BTreeMap<&str, usize> = BTreeMap::new();
    for r in rows {
        *arms.entry(r.arm.as_str()).or_default() += 1;
        *models
            .entry(r.model.as_deref().unwrap_or("unknown"))
            .or_default() += 1;
    }
    json!({
        "trials": rows.len(),
        "passes": rows.iter().filter(|r| !r.failed()).count(),
        "failures": rows.iter().filter(|r| r.failed()).count(),
        "tasks": tasks.len(),
        "calibration": half(Split::Calibration),
        "held_out": half(Split::HeldOut),
        "arms": arms,
        "models": models,
        "per_task": tasks.iter().map(|(t, (p, f, s))| json!({
            "task": t, "passes": p, "failures": f, "split": s.word(),
        })).collect::<Vec<_>>(),
    })
}

/// Every signal measured on all trials, the calibration half, and the
/// held-out half; the verdict's fitted parameters and its held-out
/// numbers against today's checks.
#[must_use]
pub fn summary(rows: &[Row]) -> Value {
    let signals = catalog(rows);
    let all: Vec<&Row> = rows.iter().collect();
    let calibration: Vec<&Row> = rows
        .iter()
        .filter(|r| r.split == Split::Calibration)
        .collect();
    let held_out: Vec<&Row> = rows.iter().filter(|r| r.split == Split::HeldOut).collect();
    let set =
        |rows: &[&Row]| -> Vec<Measure> { signals.iter().map(|s| measure(s, rows)).collect() };
    let answered = rows.iter().filter(|r| r.report_answers.is_some()).count();
    let fitted = super::verdict::fitted();
    let refit = super::verdict::fit(&calibration);
    let pick =
        |id: &str, set: &[&Row]| signals.iter().find(|s| s.id == id).map(|s| measure(s, set));
    json!({
        "schema": SUMMARY_SCHEMA,
        "predictions": rows.iter().map(|r| json!({
            "job": r.job, "trial": r.trial, "task": canonical_task(&r.task),
            "split": r.split, "reward": r.reward,
            "calls": signals.iter().filter(|s| matches!(s.id.as_str(),
                "checks.final" | "microluna.local-score" | "verdict.combined" | "verdict.corroborated"))
                .map(|s| (s.id.clone(), (s.says)(r))).collect::<BTreeMap<_, _>>(),
        })).collect::<Vec<_>>(),
        "label_set": spread(rows),
        "report_answers": answered,
        "evidence_coverage": {
            "reports": rows.iter().filter(|r| r.report_chars > 0).count(),
            "missing_reports": rows.iter().filter(|r| r.report_chars == 0).count(),
            "microluna_scores": rows.iter().filter(|r| r.local_score.is_some()).count(),
            "same_candidate_reviews": rows.iter().filter(|r| !r.review_sessions.is_empty()).count(),
        },
        "validation_note": "Historical task split; repeatedly studied tasks and reused comparison results are not untouched validation. Wilson intervals treat trials as independent; use task-cluster intervals for repeated tasks.",
        "verdict": super::verdict::describe(&fitted),
        "refit": refit,
        "refit_matches": refit.as_ref().is_some_and(|r| {
            r.fail_support == fitted.fail_support
                && r.pass_support == fitted.pass_support
                && (r.fail_at - fitted.fail_at).abs() < 1e-9
                && (r.pass_at - fitted.pass_at).abs() < 1e-9
        }),
        "held_out": {
            "verdict": pick("verdict.combined", &held_out),
            "corroborated": pick("verdict.corroborated", &held_out),
            "todays_checks": pick("checks.final", &held_out),
        },
        "signals": {
            "all": set(&all),
            "calibration": set(&calibration),
            "held_out": set(&held_out),
        },
    })
}

/// A rate as a short cell.
fn cell(rate: &Rate) -> String {
    match rate.p {
        Some(p) => format!(
            "{:>3}/{:<3} {:>3.0}% {:>3.0}–{:<3.0}",
            rate.k,
            rate.n,
            p * 100.0,
            rate.low.unwrap_or(0.0) * 100.0,
            rate.high.unwrap_or(0.0) * 100.0
        ),
        None => format!("{:>3}/{:<3} {:>13}", rate.k, rate.n, "—"),
    }
}

/// The summary as text: the label set, then each signal on the set named
/// by `which` (`all`, `calibration`, or `held_out`).
#[must_use]
pub fn lines(summary: &Value, which: &str) -> Vec<String> {
    let set = &summary["label_set"];
    let mut out = vec![
        format!(
            "Label set: {} graded trials ({} passes, {} failures) on {} tasks",
            set["trials"], set["passes"], set["failures"], set["tasks"]
        ),
        format!(
            "  calibration: {} trials on {} tasks · held-out: {} trials on {} tasks",
            set["calibration"]["trials"],
            set["calibration"]["tasks"],
            set["held_out"]["trials"],
            set["held_out"]["tasks"]
        ),
        String::new(),
        format!(
            "{:<44} {:<23} {:<23} {:<23} {}",
            format!("signal ({which})"),
            "fail precision",
            "failure recall",
            "pass when \"pass\"",
            "separates"
        ),
    ];
    for m in summary["signals"][which].as_array().into_iter().flatten() {
        let Ok(m) = serde_json::from_value::<Measure>(m.clone()) else {
            continue;
        };
        out.push(format!(
            "{:<44} {:<23} {:<23} {:<23} {}",
            m.signal,
            cell(&m.fail_precision),
            cell(&m.failure_recall),
            cell(&m.pass_when_pass),
            if m.separates { "yes" } else { "no" }
        ));
    }
    out.push(String::new());
    out.push("Held-out half, fail precision and failure recall:".to_string());
    for (label, key) in [
        ("combined verdict", "verdict"),
        ("corroborated", "corroborated"),
        ("today's checks", "todays_checks"),
    ] {
        if let Ok(m) = serde_json::from_value::<Measure>(summary["held_out"][key].clone()) {
            out.push(format!(
                "  {label:<17} {} · {} · pass when \"pass\" {}",
                m.fail_precision.text(),
                m.failure_recall.text(),
                m.pass_when_pass.text()
            ));
        }
    }
    let params = &summary["verdict"]["params"];
    out.push(format!(
        "Historical verdict: fail at p ≥ {:.3} (original calibration precision {:.0}%), pass at p ≤ {:.3} ({:.0}%); refit on these rows {}",
        params["fail_at"].as_f64().unwrap_or(f64::NAN),
        params["fail_precision"].as_f64().unwrap_or(0.0) * 100.0,
        params["pass_at"].as_f64().unwrap_or(f64::NAN),
        params["pass_precision"].as_f64().unwrap_or(0.0) * 100.0,
        if summary["refit_matches"] == true {
            "matches"
        } else {
            "differs: the rows aren't the checked-in set"
        }
    ));
    out
}

/// Where `checks truth` writes by default.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/checks-truth"))
}

/// The file of recorded report answers in a truth directory.
pub const RECORDED_FILE: &str = "jev-recorded.json";

/// How many report requests run at once.
const CONCURRENCY: usize = 8;

/// Asks the report questions for every loaded trial that has a task and a
/// final report: answers recorded in `recorded` first, then `live` for the
/// rest when there is a client. New live answers join `recorded`.
pub async fn ask_reports(
    loaded: &mut [Loaded],
    recorded: &mut crate::component::jev::Recorded,
    live: Option<&jev::Client>,
) -> (usize, usize, u64) {
    use crate::component::jev::{Ask, JevMode, RecordedAnswer, ask};
    use futures_util::StreamExt;

    let recorder = crate::record::Recorder::default();
    let replay = JevMode::Recorded(recorded.clone());
    let live_mode = live.map(|c| JevMode::Live(c.clone()));
    let questions = super::verdict::report_questions();
    let jobs: Vec<(usize, Value)> = loaded
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            Some((
                i,
                super::verdict::report_state(l.instruction.as_deref()?, l.report.as_deref()?),
            ))
        })
        .collect();
    let answers: Vec<(usize, crate::component::jev::Asked)> =
        futures_util::stream::iter(jobs.into_iter().map(|(i, state)| {
            let questions = questions.clone();
            let recorder = &recorder;
            let replay = &replay;
            let live_mode = live_mode.as_ref();
            async move {
                let request = |mode| {
                    ask(
                        mode,
                        recorder,
                        Ask {
                            component: "verify.verdict",
                            name: "jev_report_verdict",
                            id: format!("jev-report-verdict-{i}"),
                            state: state.clone(),
                            questions: questions.clone(),
                            parent: None,
                            deadline: None,
                        },
                    )
                };
                let mut asked = request(replay).await;
                if !asked.answered()
                    && let Some(mode) = live_mode
                {
                    asked = request(mode).await;
                }
                (i, asked)
            }
        }))
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;
    let (mut replayed, mut asked_live, mut tokens) = (0, 0, 0);
    for (i, asked) in answers {
        let Some(values) = &asked.answers else {
            continue;
        };
        if asked.how == "live" {
            asked_live += 1;
            tokens += asked.input_tokens.unwrap_or(0);
            recorded.entries.insert(
                asked.key.clone(),
                RecordedAnswer {
                    name: "jev_report_verdict".to_string(),
                    model: crate::credentials::JEV_MODEL.to_string(),
                    answers: values.clone(),
                    input_tokens: asked.input_tokens,
                    output_tokens: asked.output_tokens,
                    milliseconds: asked.milliseconds,
                    source: format!("checks truth: {}", loaded[i].row.trial),
                },
            );
        } else {
            replayed += 1;
        }
        let map: BTreeMap<String, f64> = super::verdict::REPORT_SIGNALS
            .iter()
            .filter_map(|(id, _, _)| Some(((*id).to_string(), asked.noul(id)?)))
            .collect();
        if !map.is_empty() {
            loaded[i].row.report_answers = Some(map);
        }
    }
    (replayed, asked_live, tokens)
}

/// Reads rows written by [`write_rows`].
///
/// # Errors
///
/// Returns why the file doesn't read.
pub fn read_rows(path: &Path) -> Result<Vec<Row>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let rows: Vec<Row> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Row>(l).map_err(|e| format!("{}: {e}", path.display())))
        .collect::<Result<_, _>>()?;
    validate_rows(&rows)?;
    Ok(rows)
}

/// Reject duplicate trials, invalid labels, and task aliases in different splits.
///
/// # Errors
/// Returns the first inconsistency; fitting inconsistent rows is not permitted.
pub fn validate_rows(rows: &[Row]) -> Result<(), String> {
    let mut trials = BTreeSet::new();
    let mut tasks = BTreeMap::new();
    for row in rows {
        if row.schema != ROW_SCHEMA || !row.reward.is_finite() || !(0.0..=1.0).contains(&row.reward)
        {
            return Err(format!("Invalid truth row or reward: {}", row.trial));
        }
        if !trials.insert((&row.job, &row.trial)) {
            return Err(format!("Duplicate trial: {}/{}", row.job, row.trial));
        }
        let task = canonical_task(&row.task);
        if tasks
            .insert(task.clone(), row.split)
            .is_some_and(|s| s != row.split)
        {
            return Err(format!("Task occurs in both partitions: {task}"));
        }
        if row.report_answers.as_ref().is_some_and(|a| {
            a.values()
                .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
        }) {
            return Err(format!("Invalid report probability: {}", row.trial));
        }
    }
    Ok(())
}

/// Writes rows as JSON lines, in trial order.
///
/// # Errors
///
/// Returns why the file can't be written.
pub fn write_rows(path: &Path, rows: &[Row]) -> Result<(), String> {
    let mut text = String::new();
    for row in rows {
        text.push_str(&serde_json::to_string(row).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    crate::record::write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> Row {
        serde_json::from_str(
            include_str!("../../fixtures/truth/rows.jsonl")
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn imported_rows_refuse_alias_leakage_and_invalid_labels() {
        let first = sample_row();
        let mut second = first.clone();
        second.trial.push_str("-another");
        second.task = first
            .task
            .strip_prefix("terminal-bench/")
            .unwrap()
            .to_string();
        second.split = if first.split == Split::Calibration {
            Split::HeldOut
        } else {
            Split::Calibration
        };
        assert!(
            validate_rows(&[first.clone(), second])
                .unwrap_err()
                .contains("both partitions")
        );
        assert!(
            validate_rows(&[first.clone(), first.clone()])
                .unwrap_err()
                .contains("Duplicate")
        );
        let mut invalid = first;
        invalid.reward = f64::NAN;
        assert!(validate_rows(&[invalid]).is_err());
    }

    #[test]
    fn retained_microluna_recovers_public_instruction_and_selected_report() {
        let traces =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces");
        let job = "tb4--coder-one-microluna-evidence-v1--embedding-drift-monitor--candidate-evidence-9607-r3";
        let trial = "embedding-drift-monitor__KDeY8Bf.episode";
        let loaded = load(job, &traces.join(job).join(trial)).expect("retained trial");
        assert!(loaded.instruction.as_ref().is_some_and(|s| !s.is_empty()));
        assert!(loaded.report.as_ref().is_some_and(|s| s.contains("13/13")));
        assert!(loaded.row.report_source.unwrap().ends_with("#session-1"));
        assert_eq!(
            loaded.row.local_score,
            Some(LocalScore {
                passed: 13,
                total: 13
            })
        );
        assert_eq!(
            loaded.row.reward, 0.0,
            "a green self-check is not the official label"
        );
    }

    #[test]
    fn wilson_matches_a_known_interval() {
        let r = Rate::of(8, 12);
        assert!((r.p.unwrap() - 0.6667).abs() < 1e-3);
        assert!((r.low.unwrap() - 0.391).abs() < 5e-3);
        assert!((r.high.unwrap() - 0.862).abs() < 5e-3);
        assert_eq!(Rate::of(0, 0).p, None);
    }

    #[test]
    fn a_task_is_in_one_split_only() {
        assert_eq!(split_of("cad-model"), split_of("terminal-bench/cad-model"));
        assert_eq!(split_of("cad-model"), split_of("cad-model"));
        let splits: BTreeSet<Split> = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .iter()
            .map(|t| split_of(t))
            .collect();
        assert_eq!(splits.len(), 2, "eight names land in both halves");
    }
}
