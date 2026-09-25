//! `coder-one study run decision-fit`: fit Jev decision settings on
//! recorded answers and labeled outcomes, with [`super::reanchor`].
//!
//! Three sources of examples:
//!
//! - `--component control.stall`: the stall detector's checkpoints, with the
//!   Jev answers `component replay control.stall` recorded and the hindsight
//!   labels of the stall-detection experiment (issue #9627). The settings
//!   are the two thresholds of the cascade: `stall.progress`, the
//!   probability of progress below which a suspect checkpoint stalls, and
//!   `stall.repeating`, the probability of repetition at or above which it
//!   does.
//! - `--component checks.verdict`: the truthful-check rows in
//!   `fixtures/truth/rows.jsonl`, with the recorded report answers and the
//!   verifier's reward (issue #9584). The setting is
//!   `checks.verdict.admission`, the corroboration threshold on
//!   `admits_unmet`.
//! - `--questions FILE --examples FILE`: any question set, with one JSON
//!   line per recorded answer: `id`, `group`, optional `task`, `partition`
//!   (`fit` or `evaluation`), `question`, `answer` as Jev returns it, and
//!   `label`, the decision that was right. Each question's own decision is
//!   fitted, and the set is written back with `decision` blocks.
//!
//! A component's examples carry an answer only where the setting decides
//! something: a checkpoint the code does not suspect, or a verdict the
//! logistic score already passes, reads no threshold.
//!
//! The fit reads the fit partition only. The evaluation partition is scored
//! after the settings are written down, before and after, with a bootstrap
//! over whole tasks. Nothing here asks Jev, and nothing changes a policy:
//! the settings are written as a proposal in the `decision` format of
//! issue #9660.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use jev::{Decision, Threshold};
use serde_json::{Value, json};

use super::reanchor::{
    self, Answer, Comparison, Example, Fit, Kind, Metric, Settings, Spec, yes_no,
};
use super::{write_json, write_lines};

/// The study's name on the command line.
pub const STUDY: &str = "decision-fit";

/// The schema of a fitted-settings proposal.
pub const PROPOSAL_SCHEMA: &str = "openagents.coder-one.decision-fit.v1";

/// The default bootstrap: resamples and seed.
pub const RESAMPLES: usize = 10_000;
pub const SEED: u64 = 9659;

/// The stall component's two settings.
pub const STALL_PROGRESS: &str = "stall.progress";
pub const STALL_REPEATING: &str = "stall.repeating";

/// The command's usage.
pub const USAGE: &str =
    "usage: coder-one study run decision-fit --component control.stall|checks.verdict
                            [--data PATH] [--metric M] [--positive LABEL]
                            [--resamples N] [--seed N] [--out DIR] [--retain [DIR]] [--json]
       coder-one study run decision-fit --questions FILE --examples FILE --metric M
                            [--positive LABEL] [--resamples N] [--seed N] [--out DIR] [--json]

Fits Jev decision settings (Noul thresholds, Score cuts, Choice weights) on
recorded answers and labeled outcomes, with no Jev call. A candidate replaces
the current value only when it scores strictly better on the fit partition and
passes a 5-fold held-out check; the evaluation partition is scored after, with
a task bootstrap. M is accuracy, f1, or recall-at-precision:FLOOR; LABEL is the
label a binary metric counts as positive (true by default). The fitted
settings are written as a proposal in the decision-block format; no policy
changes. --data defaults to the retained records of each component.";

/// Where examples come from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A component with a built-in rule.
    Component { name: String, data: Option<PathBuf> },
    /// A question set and one recorded answer per line.
    Questions {
        questions: PathBuf,
        examples: PathBuf,
    },
}

/// A study's options.
#[derive(Debug, Clone)]
pub struct Options {
    pub source: Source,
    pub metric: Option<String>,
    pub positive: String,
    pub resamples: usize,
    pub seed: u64,
    pub out: PathBuf,
    pub retain: Option<PathBuf>,
}

/// What one component contributes: its examples, settings, rule, and
/// default metric.
struct Loaded<C> {
    component: String,
    about: Value,
    specs: Vec<Spec>,
    start: Settings,
    fit: Vec<Example<C>>,
    evaluation: Vec<Example<C>>,
    default_metric: &'static str,
    /// The question-set file to write back with fitted blocks, if any.
    questions: Option<(PathBuf, Value)>,
}

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

/// A path as a record shows it: relative to the checkout when it's inside
/// it, so a study's digest doesn't depend on where the checkout is.
fn shown(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match (path.canonicalize(), root.canonicalize()) {
        (Ok(path), Ok(root)) => path
            .strip_prefix(&root)
            .map_or_else(|_| path.display().to_string(), |p| p.display().to_string()),
        _ => path.display().to_string(),
    }
}

fn threshold(value: f64) -> Decision {
    Decision {
        threshold: Some(Threshold::at(value)),
        ..Decision::default()
    }
}

fn read_lines(path: &Path) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line)
                .map_err(|error| format!("{} line {}: {error}", path.display(), index + 1))
        })
        .collect()
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

// ---------------------------------------------------------------------------
// control.stall
// ---------------------------------------------------------------------------

/// A stall checkpoint's code features and Jev answers.
#[derive(Debug, Clone)]
pub struct StallContext {
    pub features: crate::stall::Features,
    pub answers: crate::stall::Answers,
}

/// The stall call `settings` make at a checkpoint: the detector's cascade,
/// with `stall.progress` read as the probability at or above which the
/// session progressed.
#[must_use]
pub fn stall_rule(settings: &Settings, example: &Example<StallContext>) -> String {
    let at = |name: &str, default: f64| {
        settings
            .get(name)
            .and_then(|decision| decision.threshold)
            .map_or(default, Threshold::value)
    };
    let params = crate::stall::Params {
        progress_below: at(STALL_PROGRESS, crate::stall::FROZEN.progress_below),
        repeating_at: at(STALL_REPEATING, crate::stall::FROZEN.repeating_at),
    };
    let verdict = crate::stall::decide(&example.context.features, &example.context.answers, params);
    yes_no(verdict.stalled)
}

/// The retained stall-detection experiment.
#[must_use]
pub fn stall_data() -> PathBuf {
    repo("bench/terminal-bench/experiments/2026-09-25-stall-detection")
}

fn load_stall(dir: &Path) -> Result<Loaded<StallContext>, String> {
    let rows = read_lines(&dir.join("records/rows.jsonl"))?;
    let mut labels: BTreeMap<(String, u64, u64, String), Value> = BTreeMap::new();
    for file in ["labels-calibration.jsonl", "labels-evaluation.jsonl"] {
        for label in read_lines(&dir.join("records").join(file))? {
            let key = (
                label["trial"].as_str().unwrap_or_default().to_string(),
                label["session"].as_u64().unwrap_or_default(),
                label["turn"].as_u64().unwrap_or_default(),
                label["at"].as_str().unwrap_or_default().to_string(),
            );
            labels.insert(key, label);
        }
    }
    let mut fit = Vec::new();
    let mut evaluation = Vec::new();
    let mut unlabeled = 0usize;
    for row in rows {
        let trial = row["trial"].as_str().unwrap_or_default().to_string();
        let key = (
            trial.clone(),
            row["session"].as_u64().unwrap_or_default(),
            row["turn"].as_u64().unwrap_or_default(),
            row["at"].as_str().unwrap_or_default().to_string(),
        );
        let Some(label) = labels.get(&key) else {
            unlabeled += 1;
            continue;
        };
        if label.pointer("/hindsight/labeled") != Some(&Value::Bool(true)) {
            unlabeled += 1;
            continue;
        }
        let stall = label.pointer("/hindsight/stall") == Some(&Value::Bool(true));
        let features: crate::stall::Features = serde_json::from_value(row["features"].clone())
            .map_err(|error| format!("{trial}: features do not read: {error}"))?;
        let answers: crate::stall::Answers = serde_json::from_value(row["answers"].clone())
            .map_err(|error| format!("{trial}: answers do not read: {error}"))?;
        // Only a suspect checkpoint reads Jev's answers.
        let mut read = BTreeMap::new();
        if features.suspect() {
            if let Some(p) = answers.progress {
                read.insert(STALL_PROGRESS.to_string(), Answer::Noul(p));
            }
            if let Some(p) = answers.repeating {
                read.insert(STALL_REPEATING.to_string(), Answer::Noul(p));
            }
        }
        let example = Example {
            id: format!("{}#{}@{}:{}", key.0, key.1, key.2, key.3),
            group: trial,
            task: row["task"].as_str().unwrap_or_default().to_string(),
            answers: read,
            label: yes_no(stall),
            context: StallContext { features, answers },
        };
        match row["partition"].as_str() {
            Some("calibration") => fit.push(example),
            Some("evaluation") => evaluation.push(example),
            _ => {}
        }
    }
    let split = read_json(&dir.join("split.json"))?;
    Ok(Loaded {
        component: crate::stall::STALL_COMPONENT.to_string(),
        about: json!({
            "records": shown(&dir.join("records/rows.jsonl")),
            "labels": ["records/labels-calibration.jsonl", "records/labels-evaluation.jsonl"],
            "answers": "the Jev answers each row carries, from jev-recorded.json through `component replay control.stall` in recorded mode (records/replay.txt: 450 rows, 0 recorded misses)",
            "split": split,
            "split_file": shown(&dir.join("split.json")),
            "fold_unit": "trial",
            "interval_unit": "task",
            "label": "hindsight.stall: no measurable progress later in the attempt (protocol.md)",
            "unlabeled_skipped": unlabeled,
            "rule": "crate::stall::decide: a suspect checkpoint stalls when progress < stall.progress or repeating >= stall.repeating",
        }),
        specs: vec![
            Spec {
                name: STALL_PROGRESS.to_string(),
                kind: Kind::Threshold,
            },
            Spec {
                name: STALL_REPEATING.to_string(),
                kind: Kind::Threshold,
            },
        ],
        start: [
            (
                STALL_PROGRESS.to_string(),
                threshold(crate::stall::FROZEN.progress_below),
            ),
            (
                STALL_REPEATING.to_string(),
                threshold(crate::stall::FROZEN.repeating_at),
            ),
        ]
        .into(),
        fit,
        evaluation,
        default_metric: "recall-at-precision:0.8",
        questions: None,
    })
}

// ---------------------------------------------------------------------------
// checks.verdict
// ---------------------------------------------------------------------------

/// The evidence of one truthful-check row.
pub type VerdictContext = crate::checks::verdict::Evidence;

/// The corroborated verdict's fail call under `settings`.
#[must_use]
pub fn verdict_rule(settings: &Settings, example: &Example<VerdictContext>) -> String {
    let admission = settings
        .get(crate::decision::VERDICT_ADMISSION.name)
        .and_then(|decision| decision.threshold)
        .unwrap_or(crate::decision::VERDICT_ADMISSION.default);
    let verdict = crate::checks::verdict::corroborated_at(
        &example.context,
        &crate::checks::verdict::fitted(),
        admission,
    );
    yes_no(verdict.call == "fail")
}

/// The retained truthful-check rows.
#[must_use]
pub fn verdict_data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/truth/rows.jsonl")
}

fn load_verdict(path: &Path) -> Result<Loaded<VerdictContext>, String> {
    use crate::checks::verdict::{Evidence, fitted, judge};
    let rows = crate::checks::truth::read_rows(path)?;
    let name = crate::decision::VERDICT_ADMISSION.name.to_string();
    let mut fit = Vec::new();
    let mut evaluation = Vec::new();
    for row in &rows {
        let evidence = Evidence::of_row(row);
        let mut read = BTreeMap::new();
        // Only a failure call without a detected admission reads the
        // threshold.
        if !evidence.admitted
            && judge(&evidence, &fitted()).call == "fail"
            && let Some(p) = evidence
                .report_answers
                .as_ref()
                .and_then(|answers| answers.get("admits_unmet"))
        {
            read.insert(name.clone(), Answer::Noul(*p));
        }
        let task = row.task.clone();
        let example = Example {
            id: row.trial.clone(),
            group: task.clone(),
            task,
            answers: read,
            label: yes_no(row.failed()),
            context: evidence,
        };
        match row.split {
            crate::checks::truth::Split::Calibration => fit.push(example),
            crate::checks::truth::Split::HeldOut => evaluation.push(example),
        }
    }
    Ok(Loaded {
        component: "checks.verdict".to_string(),
        about: json!({
            "records": shown(path),
            "answers": "each row's recorded report answers (report_answers), asked once and kept with the row",
            "split": "each row's own split, calibration or held-out, fixed by the original truthful-checks study and grouped by task",
            "fold_unit": "task",
            "interval_unit": "task",
            "label": "the verifier failed the trial (reward below 1)",
            "rule": "crate::checks::verdict::corroborated_at with the fitted logistic parameters: a failure call stands when the detector found an admission or admits_unmet reaches the threshold",
        }),
        specs: vec![Spec {
            name: name.clone(),
            kind: Kind::Threshold,
        }],
        start: [(
            name,
            Decision {
                threshold: Some(crate::decision::VERDICT_ADMISSION.default),
                ..Decision::default()
            },
        )]
        .into(),
        fit,
        evaluation,
        default_metric: "recall-at-precision:0.9",
        questions: None,
    })
}

// ---------------------------------------------------------------------------
// A question set and recorded answers
// ---------------------------------------------------------------------------

/// The question a setting name refers to: `questions.<id>`, or
/// `per_finding` for a set that asks one question per finding.
fn question_mut<'a>(set: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    if name == "per_finding" {
        return set.get_mut("per_finding");
    }
    set.get_mut("questions")?.get_mut(name)
}

/// The question set with each fitted setting's `decision` block joined
/// beside its wording, every block checked against its question.
///
/// # Errors
///
/// Returns a sentence when a setting names no question in the set, or its
/// block doesn't fit the question.
pub fn with_blocks(set: &Value, settings: &Settings) -> Result<Value, String> {
    let mut set = set.clone();
    for (name, decision) in settings {
        let question = question_mut(&mut set, name)
            .ok_or_else(|| format!("the question set has no question {name}"))?;
        if let Some(object) = question.as_object_mut() {
            object.remove(jev::decision::DECISION_KEY);
        }
        decision
            .validate(question)
            .map_err(|error| format!("{name}: {error}"))?;
        jev::decision::join(question, decision);
    }
    Ok(set)
}

fn load_questions(questions: &Path, examples: &Path) -> Result<Loaded<()>, String> {
    let set = read_json(questions)?;
    let lines = read_lines(examples)?;
    let mut fit = Vec::new();
    let mut evaluation = Vec::new();
    let mut specs: BTreeMap<String, Spec> = BTreeMap::new();
    let mut start = Settings::new();
    for (index, line) in lines.iter().enumerate() {
        let at = || format!("{} line {}", examples.display(), index + 1);
        let name = line["question"]
            .as_str()
            .ok_or_else(|| format!("{}: no question", at()))?
            .to_string();
        let answer =
            Answer::from_json(&line["answer"]).map_err(|error| format!("{}: {error}", at()))?;
        if !specs.contains_key(&name) {
            let mut question = question_mut(&mut set.clone(), &name)
                .cloned()
                .ok_or_else(|| format!("{}: the set has no question {name}", at()))?;
            let current = jev::decision::split(&mut question)
                .map_err(|error| format!("{name}: {error}"))?
                .unwrap_or_default();
            start.insert(name.clone(), current);
            let kind = match question.get("type").and_then(Value::as_str) {
                Some("noul") => Kind::Threshold,
                Some("score") => Kind::Cuts { levels: Vec::new() },
                Some("choice") => Kind::Weights {
                    options: Vec::new(),
                },
                other => {
                    return Err(format!(
                        "{name}: a question's type is noul, score, or choice, not {}",
                        other.unwrap_or("missing")
                    ));
                }
            };
            specs.insert(
                name.clone(),
                Spec {
                    name: name.clone(),
                    kind,
                },
            );
        }
        // Levels and options come from the answers.
        if let Some(spec) = specs.get_mut(&name) {
            match (&mut spec.kind, &answer) {
                (Kind::Cuts { levels }, Answer::Score(score)) => {
                    let named = if score.legend.is_empty() {
                        score.probabilities.keys().copied().collect::<Vec<_>>()
                    } else {
                        score.legend.keys().copied().collect()
                    };
                    for level in named {
                        if !levels.contains(&level) {
                            levels.push(level);
                        }
                    }
                    levels.sort_unstable();
                }
                (Kind::Weights { options }, Answer::Choice(choice)) => {
                    for option in choice.probabilities.keys() {
                        if !options.contains(option) {
                            options.push(option.clone());
                        }
                    }
                }
                (Kind::Threshold, Answer::Noul(_)) => {}
                _ => {
                    return Err(format!(
                        "{}: the answer's type does not match question {name}",
                        at()
                    ));
                }
            }
        }
        let label = match &line["label"] {
            Value::String(text) => text.clone(),
            Value::Bool(yes) => yes_no(*yes),
            Value::Number(number) => number.to_string(),
            _ => return Err(format!("{}: no label", at())),
        };
        let group = line["group"]
            .as_str()
            .ok_or_else(|| format!("{}: no group", at()))?
            .to_string();
        let example = Example {
            id: line["id"]
                .as_str()
                .map_or_else(|| format!("line-{}", index + 1), str::to_string),
            task: line["task"].as_str().unwrap_or(&group).to_string(),
            group,
            answers: [(name, answer)].into(),
            label,
            context: (),
        };
        match line["partition"].as_str() {
            Some("fit") => fit.push(example),
            Some("evaluation") => evaluation.push(example),
            other => {
                return Err(format!(
                    "{}: partition is fit or evaluation, not {}",
                    at(),
                    other.unwrap_or("missing")
                ));
            }
        }
    }
    Ok(Loaded {
        component: set["id"].as_str().unwrap_or("questions").to_string(),
        about: json!({
            "questions": shown(questions),
            "records": shown(examples),
            "answers": "one recorded answer per line",
            "fold_unit": "group",
            "interval_unit": "task, or group when a line names none",
            "label": "the decision that was right, per line",
            "rule": "each question's own decision: yes at the threshold, the level under the cuts, the weighted pick",
        }),
        specs: specs.into_values().collect(),
        start,
        fit,
        evaluation,
        default_metric: "accuracy",
        questions: Some((questions.to_path_buf(), set)),
    })
}

/// A line's own decision under `settings`.
#[must_use]
pub fn single_rule(settings: &Settings, example: &Example<()>) -> String {
    example
        .answers
        .iter()
        .next()
        .and_then(|(name, answer)| {
            answer.decide(settings.get(name).unwrap_or(&Decision::default()))
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// A finished study.
#[derive(Debug, Clone)]
pub struct Finished {
    pub dir: PathBuf,
    pub result: Value,
}

fn describe(decision: &Decision) -> Value {
    serde_json::to_value(decision).unwrap_or(Value::Null)
}

fn settings_json(settings: &Settings) -> Value {
    json!(
        settings
            .iter()
            .map(|(name, decision)| (name.clone(), describe(decision)))
            .collect::<BTreeMap<_, _>>()
    )
}

fn comparison_json(comparison: &Comparison) -> Value {
    serde_json::to_value(comparison).unwrap_or(Value::Null)
}

#[allow(clippy::too_many_lines)]
fn run_loaded<C>(
    loaded: Loaded<C>,
    rule: reanchor::Rule<'_, C>,
    options: &Options,
) -> Result<Finished, String> {
    let started = Instant::now();
    if loaded.fit.is_empty() {
        return Err(format!(
            "{}: no example in the fit partition",
            loaded.component
        ));
    }
    let metric_name = options
        .metric
        .clone()
        .unwrap_or_else(|| loaded.default_metric.to_string());
    let metric = Metric::parse(&metric_name, &options.positive)?;
    let plan = json!({
        "schema": super::STUDY_SCHEMA,
        "nip_opt": "openagents.optimization-study.v1",
        "study_kind": STUDY,
        "component": loaded.component,
        "algorithm": {
            "name": "decision-settings fit on recorded answers",
            "credit": "reimplemented from DSPy 3.4.0's ReAnchor (stanfordnlp/dspy#10475, MIT); no source copied",
            "candidates": format!("threshold and cut: midpoints between neighboring observed values, 0 and 1 (or the neighboring cuts) as the outer neighbors; weight: flip points on a log scale from 1/{0} to {0}; at most {1} per coordinate, evenly spaced by rank", reanchor::WEIGHT_SPAN, reanchor::CAP),
            "selection": "highest training score, then the widest gap, then the lowest value",
            "acceptance": format!("strictly better on the fit partition and on a {}-fold held-out check that reruns the selection without each fold and pools the held-out predictions; otherwise the current value stays", reanchor::FOLDS),
            "fold_rule": format!("groups ordered by SHA-256 of \"{}\" and the name, dealt round-robin", reanchor::FOLD_SALT),
            "passes": 1,
        },
        "metric": metric,
        "settings": loaded.specs,
        "start": settings_json(&loaded.start),
        "data": loaded.about,
        "partitions": {
            "fit": loaded.fit.len(),
            "evaluation": loaded.evaluation.len(),
            "evaluation_read": "after the fitted settings are written",
        },
        "interval": { "resamples": options.resamples, "seed": options.seed, "unit": "task" },
        "bounds": { "spend_usd": 0.0, "jev_calls": 0, "note": "recorded answers only; no Jev, Luna, or Terminal-Bench call" },
    });
    let digest = atif::digest(&plan);
    let id = format!(
        "decision-fit-{}-{}",
        loaded.component.replace(['.', '/'], "-"),
        &digest[..12]
    );
    let dir = options.out.join(&id);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    let mut plan = plan;
    plan["study"] = json!(id);
    plan["digest"] = json!(digest);
    write_json(&dir.join("study.json"), &plan)?;

    // Fit on the fit partition only, and write the settings down.
    let fit_refs: Vec<&Example<C>> = loaded.fit.iter().collect();
    let fitted: Fit = reanchor::fit(&loaded.specs, &loaded.start, &fit_refs, &metric, rule);
    write_lines(&dir.join("steps.jsonl"), &fitted.steps)?;
    let changed = fitted.changed();
    let proposal = json!({
        "schema": PROPOSAL_SCHEMA,
        "study": id,
        "component": loaded.component,
        "status": "proposal: not in effect in any policy",
        "decision": settings_json(&changed),
        "all": settings_json(&fitted.after),
        "policy_jev_decision": (!changed.is_empty()).then(|| atif::digest(&json!(changed))),
        "note": "`decision` holds the settings the fit moved, by name, in the decision-block format; `policy_jev_decision` is the digest a manifest would record if they were the only settings off their defaults",
    });
    write_json(&dir.join("decisions.json"), &proposal)?;
    let mut question_file = None;
    if let Some((path, set)) = &loaded.questions {
        let written = with_blocks(set, &changed)?;
        let name = path.file_name().map_or_else(
            || "questions.json".to_string(),
            |n| n.to_string_lossy().to_string(),
        );
        let target = dir.join(&name);
        write_json(&target, &written)?;
        question_file = Some(shown(&target));
    }

    // Then read the evaluation partition.
    let evaluation_refs: Vec<&Example<C>> = loaded.evaluation.iter().collect();
    let evaluation = (!evaluation_refs.is_empty()).then(|| {
        reanchor::compare(
            &metric,
            rule,
            &fitted.before,
            &fitted.after,
            &evaluation_refs,
            options.resamples,
            options.seed,
        )
    });
    let training = reanchor::compare(
        &metric,
        rule,
        &fitted.before,
        &fitted.after,
        &fit_refs,
        options.resamples,
        options.seed,
    );
    let statement = if changed.is_empty() {
        format!(
            "No setting moved: no candidate beat the current value on training and on the held-out check, so {} keeps its settings.",
            loaded.component
        )
    } else {
        let moved: Vec<String> = changed
            .iter()
            .map(|(name, decision)| format!("{name} {}", describe(decision)))
            .collect();
        match &evaluation {
            Some(evaluation) => format!(
                "Moved {}. On the evaluation partition, {} went from {:.4} to {:.4} ({:+.4}, 95% interval {:+.4} to {:+.4}).",
                moved.join(", "),
                metric.name(),
                evaluation.before.score,
                evaluation.after.score,
                evaluation.difference,
                evaluation.difference_interval[0],
                evaluation.difference_interval[1],
            ),
            None => format!(
                "Moved {}; there is no evaluation partition.",
                moved.join(", ")
            ),
        }
    };
    let result = json!({
        "schema": super::RESULT_SCHEMA,
        "study": id,
        "study_kind": STUDY,
        "component": loaded.component,
        "status": "finished",
        "metric": metric.name(),
        "positive": metric.positive(),
        "before": settings_json(&fitted.before),
        "after": settings_json(&fitted.after),
        "changed": settings_json(&changed),
        "steps": fitted.steps,
        "training": {
            "examples": fitted.train_examples,
            "groups": fitted.groups,
            "before": fitted.train_before,
            "after": fitted.train_after,
            "comparison": comparison_json(&training),
        },
        "evaluation": evaluation.as_ref().map(comparison_json),
        "question_set": question_file,
        "candidates": [],
        "confirmation": { "statement": statement },
        "spend": { "calls": 0, "spend_usd": 0.0, "wall_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX), "complete": true },
    });
    write_json(&dir.join("result.json"), &result)?;
    if let Some(retain) = &options.retain {
        let target = retain.join(&id);
        std::fs::create_dir_all(&target)
            .map_err(|error| format!("cannot create {}: {error}", target.display()))?;
        for entry in std::fs::read_dir(&dir)
            .map_err(|error| format!("cannot read {}: {error}", dir.display()))?
            .flatten()
        {
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name()
            {
                std::fs::copy(&path, target.join(name))
                    .map_err(|error| format!("cannot copy {}: {error}", path.display()))?;
            }
        }
    }
    Ok(Finished { dir, result })
}

/// Runs the study.
///
/// # Errors
///
/// Returns a sentence when the examples don't read or the records can't be
/// written. A study that moves nothing is a finished study.
pub fn run(options: &Options) -> Result<Finished, String> {
    match &options.source {
        Source::Component { name, data } => match name.as_str() {
            "control.stall" => {
                let loaded = load_stall(&data.clone().unwrap_or_else(stall_data))?;
                run_loaded(loaded, &stall_rule, options)
            }
            "checks.verdict" => {
                let loaded = load_verdict(&data.clone().unwrap_or_else(verdict_data))?;
                run_loaded(loaded, &verdict_rule, options)
            }
            other => Err(format!(
                "decision-fit has no rule for {other}: use control.stall, checks.verdict, or --questions with --examples"
            )),
        },
        Source::Questions {
            questions,
            examples,
        } => run_loaded(load_questions(questions, examples)?, &single_rule, options),
    }
}

/// `coder-one study run decision-fit …`: returns the exit code.
///
/// # Errors
///
/// Returns a sentence for a malformed command or a study that can't run.
pub fn command(args: &[String]) -> Result<i32, String> {
    let mut component = None;
    let mut data = None;
    let mut questions = None;
    let mut examples = None;
    let mut metric = None;
    let mut positive = "true".to_string();
    let mut resamples = RESAMPLES;
    let mut seed = SEED;
    let mut out = None;
    let mut retain = None;
    let mut json_output = false;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--component" => component = Some(value("--component")?),
            "--data" => data = Some(PathBuf::from(value("--data")?)),
            "--questions" => questions = Some(PathBuf::from(value("--questions")?)),
            "--examples" => examples = Some(PathBuf::from(value("--examples")?)),
            "--metric" => metric = Some(value("--metric")?),
            "--positive" => positive = value("--positive")?,
            "--resamples" => {
                let text = value("--resamples")?;
                resamples = text
                    .parse()
                    .map_err(|_| format!("--resamples takes a whole number, not {text}"))?;
            }
            "--seed" => {
                let text = value("--seed")?;
                seed = text
                    .parse()
                    .map_err(|_| format!("--seed takes a whole number, not {text}"))?;
            }
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--json" => json_output = true,
            "--retain" => {
                retain = Some(match iter.peek() {
                    Some(next) if !next.starts_with("--") => {
                        PathBuf::from(iter.next().cloned().unwrap_or_default())
                    }
                    _ => super::retained_dir(),
                });
            }
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    let source = match (component, questions, examples) {
        (Some(name), None, None) => Source::Component { name, data },
        (None, Some(questions), Some(examples)) => Source::Questions {
            questions,
            examples,
        },
        _ => {
            return Err(format!(
                "name either --component, or --questions with --examples\n{USAGE}"
            ));
        }
    };
    let out = out
        .or_else(super::default_dir)
        .ok_or("no --out and no HOME to record under")?;
    let options = Options {
        source,
        metric,
        positive,
        resamples,
        seed,
        out,
        retain,
    };
    let finished = run(&options)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&finished.result).map_err(|error| error.to_string())?
        );
    } else {
        print_result(&finished.result);
        println!("recorded under {}", finished.dir.display());
    }
    Ok(0)
}

fn print_result(result: &Value) {
    let number = |v: &Value| v.as_f64().map_or("—".to_string(), |x| format!("{x:.4}"));
    let pair = |v: &Value| format!("{} to {}", number(&v[0]), number(&v[1]));
    println!(
        "study {} · {} · metric {}",
        result["study"].as_str().unwrap_or("?"),
        result["component"].as_str().unwrap_or("?"),
        result["metric"].as_str().unwrap_or("?"),
    );
    for step in result["steps"].as_array().into_iter().flatten() {
        println!(
            "  {} {}: {} candidates; training {} -> {}; held-out {}; {}",
            step["setting"].as_str().unwrap_or("?"),
            step["coordinate"].as_str().unwrap_or("?"),
            step["candidates"],
            number(&step["train_before"]),
            number(&step["train_after"]),
            step.get("held_out").filter(|h| !h.is_null()).map_or(
                "not run".to_string(),
                |h| format!("{} -> {}", number(&h["before"]), number(&h["after"]))
            ),
            step["why"].as_str().unwrap_or(""),
        );
    }
    println!("before {}", result["before"]);
    println!("after  {}", result["after"]);
    let training = &result["training"]["comparison"];
    println!(
        "training ({} examples): {} -> {}",
        training["examples"],
        number(&training["before"]["score"]),
        number(&training["after"]["score"]),
    );
    let evaluation = &result["evaluation"];
    if !evaluation.is_null() {
        println!(
            "evaluation ({} examples, {} tasks): {} ({}) -> {} ({}); difference {} ({})",
            evaluation["examples"],
            evaluation["tasks"],
            number(&evaluation["before"]["score"]),
            pair(&evaluation["before"]["interval"]),
            number(&evaluation["after"]["score"]),
            pair(&evaluation["after"]["interval"]),
            number(&evaluation["difference"]),
            pair(&evaluation["difference_interval"]),
        );
    }
    if let Some(statement) = result
        .pointer("/confirmation/statement")
        .and_then(Value::as_str)
    {
        println!("{statement}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stall_rule_at_the_frozen_settings_is_the_detectors_call() {
        let loaded = load_stall(&stall_data()).expect("the retained stall records");
        assert_eq!(loaded.fit.len(), 327);
        assert_eq!(loaded.evaluation.len(), 102);
        // At the frozen thresholds the rule reproduces selection.json: 88
        // calls, 80 right, on calibration.
        let calls: Vec<&Example<StallContext>> = loaded
            .fit
            .iter()
            .filter(|e| stall_rule(&loaded.start, e) == "true")
            .collect();
        assert_eq!(calls.len(), 88);
        assert_eq!(calls.iter().filter(|e| e.label == "true").count(), 80);
    }

    #[test]
    fn the_verdict_rule_at_the_setting_in_effect_is_the_corroborated_verdict() {
        let loaded = load_verdict(&verdict_data()).expect("the truth rows");
        assert_eq!(loaded.fit.len() + loaded.evaluation.len(), 317);
        for example in loaded.fit.iter().chain(&loaded.evaluation) {
            let expected = crate::checks::verdict::corroborated(
                &example.context,
                &crate::checks::verdict::fitted(),
            );
            assert_eq!(
                verdict_rule(&loaded.start, example),
                yes_no(expected.call == "fail")
            );
        }
    }

    #[test]
    fn a_question_set_study_writes_its_fitted_blocks_and_moves_nothing_live() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let questions = dir.path().join("set.json");
        std::fs::write(
            &questions,
            serde_json::to_string(&json!({
                "v": 1,
                "id": "openagents.test.v1",
                "questions": { "refund": { "type": "noul", "instructions": "Money back?" } },
            }))
            .unwrap(),
        )
        .unwrap();
        let mut lines = String::new();
        for i in 0..300 {
            let p = (f64::from(i) + 0.5) / 300.0;
            let partition = if i % 3 == 0 { "evaluation" } else { "fit" };
            lines.push_str(
                &json!({
                    "id": format!("e{i}"),
                    "group": format!("g{}", i % 12),
                    "partition": partition,
                    "question": "refund",
                    "answer": { "type": "noul", "noul": p },
                    "label": p >= 0.7,
                })
                .to_string(),
            );
            lines.push('\n');
        }
        let examples = dir.path().join("examples.jsonl");
        std::fs::write(&examples, lines).unwrap();
        let before = std::fs::read(&questions).unwrap();
        let options = Options {
            source: Source::Questions {
                questions: questions.clone(),
                examples,
            },
            metric: Some("accuracy".to_string()),
            positive: "true".to_string(),
            resamples: 200,
            seed: 1,
            out: dir.path().join("out"),
            retain: None,
        };
        let finished = run(&options).expect("the study runs");
        // The input set is untouched; the fitted copy carries the block.
        assert_eq!(std::fs::read(&questions).unwrap(), before);
        let written = read_json(&finished.dir.join("set.json")).unwrap();
        let t = written["questions"]["refund"]["decision"]["threshold"]
            .as_f64()
            .expect("a fitted threshold");
        assert!((t - 0.7).abs() < 0.01, "{t}");
        let mut question = written["questions"]["refund"].clone();
        assert!(jev::decision::split(&mut question).unwrap().is_some());
        let proposal = read_json(&finished.dir.join("decisions.json")).unwrap();
        assert_eq!(
            proposal["decision"]["refund"]["threshold"].as_f64(),
            Some(t)
        );
        assert!(proposal["policy_jev_decision"].is_string());
        let evaluation = &finished.result["evaluation"];
        assert!(evaluation["difference"].as_f64().unwrap() > 0.1);
        // Rerunning writes the same result.
        let again = run(&options).expect("the study reruns");
        assert_eq!(again.result["after"], finished.result["after"]);
        assert_eq!(again.dir, finished.dir);
    }

    #[test]
    fn a_block_for_a_missing_question_is_refused() {
        let set = json!({ "questions": { "a": { "type": "noul" } } });
        let settings: Settings = [("b".to_string(), threshold(0.6))].into();
        assert!(
            with_blocks(&set, &settings)
                .unwrap_err()
                .contains("no question b")
        );
        let wrong: Settings = [(
            "a".to_string(),
            Decision {
                cuts: Some(jev::Cuts::new(vec![0.5]).unwrap()),
                ..Decision::default()
            },
        )]
        .into();
        assert!(with_blocks(&set, &wrong).unwrap_err().contains("Score"));
        let per_finding = json!({ "per_finding": { "type": "noul" } });
        let fitted: Settings = [("per_finding".to_string(), threshold(0.65))].into();
        assert_eq!(
            with_blocks(&per_finding, &fitted).unwrap()["per_finding"]["decision"]["threshold"],
            json!(0.65)
        );
    }
}
