//! `control.route`: pick each task's policy from Jev's task features, and
//! score the pick by leave-one-task-out regret on the outcome matrix.
//!
//! The features come from Coder One's `task.profile` component, exported
//! with recorded Jev answers to `bench/terminal-bench/profiles/task-features.json`:
//! five Nouls and a difficulty Score per task, and a Choice of executor
//! whose state carried each executor's measured behavior on the other
//! tasks.
//!
//! The fitted router is a decision stump: one feature, one threshold, and a
//! policy on each side, or a single policy when no split helps. In each
//! fold the held-out task is left out of everything the fit chooses: the
//! portfolio (the candidate policies that are best on some training task),
//! the feature, the threshold, and both sides' policies. Its regret on the
//! held-out task is its objective minus the best candidate's there. The
//! fitted router is compared with fixed Luna, fixed Opus, a hand-written
//! rule, and the Jev Choice, and every router that reads features pays for
//! the profile request, whatever it picks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::coder_matrix::{Matrix, Source};

/// The schema of `gym coder router --json`.
pub const SCHEMA: &str = "openagents.gym.coder-router.v1";

/// The features a router may split on, in the order the battery asks.
pub const FEATURES: &[&str] = &[
    "builds_code",
    "installs_packages",
    "parses_data",
    "recovers_git",
    "concurrency",
    "difficulty",
];

/// The executor profiles the Jev Choice names, and the arms that measured
/// them.
pub const PROFILES: &[(&str, &str)] = &[
    ("luna-direct", "codex-gpt-6-luna"),
    ("luna-briefed", "coder-one-jevprobe2-luna"),
    ("opus-lean", "coder-one-jevprobe2-opus-lean-low-5m"),
];

/// Fixed Luna: the cheapest single arm.
pub const FIXED_LUNA: &str = "coder-one-jevprobe3-luna";
/// Fixed Opus: the best Opus arm.
pub const FIXED_OPUS: &str = "coder-one-jevprobe2-opus-lean-low-5m";

/// The hand-written rule, in words.
pub const HAND_RULE: &str = "Opus lean when concurrency is at least 0.5 or difficulty is at least 0.75; otherwise briefed Luna";

/// The checked-in feature export and task pool.
#[must_use]
pub fn default_paths() -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/profiles");
    (root.join("task-features.json"), root.join("task-pool.json"))
}

/// One task's profile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Features {
    pub task: String,
    pub values: BTreeMap<String, Option<f64>>,
    /// The profile the Jev Choice named.
    pub choice: Option<String>,
    /// What the profile request cost when asked live.
    pub cost: Option<f64>,
}

impl Features {
    fn value(&self, feature: &str) -> f64 {
        // An unknown answer reads as below every threshold.
        self.values
            .get(feature)
            .copied()
            .flatten()
            .unwrap_or(f64::NEG_INFINITY)
    }
}

/// Reads a `task.profile` export.
///
/// # Errors
///
/// Returns a message when the file doesn't read or isn't an export of
/// `task.profile`.
pub fn load_features(path: &Path) -> Result<BTreeMap<String, Features>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if value["component"].as_str() != Some("task.profile") {
        return Err(format!("{} is not a task.profile export", path.display()));
    }
    let mut out = BTreeMap::new();
    for fixture in value["fixtures"].as_array().into_iter().flatten() {
        let Some(task) = fixture["task"].as_str() else {
            continue;
        };
        let output = &fixture["output"];
        let mut values: BTreeMap<String, Option<f64>> = output["features"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_f64()))
            .collect();
        values.insert("difficulty".to_owned(), output["difficulty"].as_f64());
        out.insert(
            task.to_owned(),
            Features {
                task: task.to_owned(),
                values,
                choice: output["executor"].as_str().map(str::to_owned),
                cost: fixture["recorded_live_cost_usd"].as_f64(),
            },
        );
    }
    Ok(out)
}

/// A routing rule.
#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    Constant(String),
    Stump {
        feature: String,
        threshold: f64,
        above: String,
        below: String,
    },
}

impl Rule {
    #[must_use]
    pub fn pick(&self, features: &Features) -> &str {
        match self {
            Rule::Constant(key) => key,
            Rule::Stump {
                feature,
                threshold,
                above,
                below,
            } => {
                if features.value(feature) >= *threshold {
                    above
                } else {
                    below
                }
            }
        }
    }

    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Rule::Constant(key) => format!("always {key}"),
            Rule::Stump {
                feature,
                threshold,
                above,
                below,
            } => format!("{above} when {feature} is at least {threshold:.3}; otherwise {below}"),
        }
    }
}

fn objective(matrix: &Matrix, task: &str, key: &str) -> f64 {
    matrix
        .cell(task, key)
        .and_then(|cell| cell.objective)
        .unwrap_or(f64::INFINITY)
}

/// The best candidate on `task` by objective, ties to the first key.
fn best(matrix: &Matrix, candidates: &[String], task: &str) -> Option<String> {
    candidates
        .iter()
        .filter(|key| objective(matrix, task, key).is_finite())
        .min_by(|a, b| objective(matrix, task, a).total_cmp(&objective(matrix, task, b)))
        .cloned()
}

/// Fits a stump on `train`: the portfolio is the candidates best on some
/// training task, and the rule minimizes the summed training objective,
/// preferring a constant on ties.
#[must_use]
pub fn fit(
    matrix: &Matrix,
    candidates: &[String],
    train: &[String],
    features: &BTreeMap<String, Features>,
) -> Rule {
    let portfolio: Vec<String> = candidates
        .iter()
        .filter(|key| {
            train
                .iter()
                .any(|task| best(matrix, candidates, task).as_deref() == Some(key.as_str()))
        })
        .cloned()
        .collect();
    let total = |rule: &Rule| -> f64 {
        train
            .iter()
            .map(|task| {
                let empty = Features::default();
                let f = features.get(task).unwrap_or(&empty);
                objective(matrix, task, rule.pick(f))
            })
            .sum()
    };
    let mut best_rule = Rule::Constant(portfolio.first().cloned().unwrap_or_default());
    let mut best_total = total(&best_rule);
    for key in &portfolio {
        let rule = Rule::Constant(key.clone());
        let t = total(&rule);
        if t < best_total - 1e-12 {
            best_rule = rule;
            best_total = t;
        }
    }
    for feature in FEATURES {
        let mut values: Vec<f64> = train
            .iter()
            .filter_map(|task| features.get(task))
            .map(|f| f.value(feature))
            .filter(|v| v.is_finite())
            .collect();
        values.sort_by(f64::total_cmp);
        values.dedup();
        let thresholds: Vec<f64> = values
            .windows(2)
            .map(|pair| ((pair[0] + pair[1]) / 2.0 * 1000.0).round() / 1000.0)
            .collect();
        for threshold in thresholds {
            for above in &portfolio {
                for below in &portfolio {
                    if above == below {
                        continue;
                    }
                    let rule = Rule::Stump {
                        feature: (*feature).to_owned(),
                        threshold,
                        above: above.clone(),
                        below: below.clone(),
                    };
                    let t = total(&rule);
                    if t < best_total - 1e-12 {
                        best_rule = rule;
                        best_total = t;
                    }
                }
            }
        }
    }
    best_rule
}

/// One router's pick on one task.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub router: String,
    pub key: Option<String>,
    /// The cell's objective plus the profile's cost for a router that reads
    /// features.
    pub objective: Option<f64>,
    pub regret: Option<f64>,
    pub passes: usize,
    pub trials: usize,
    pub cost: Option<f64>,
    pub seconds: Option<f64>,
}

/// One held-out task's fold.
#[derive(Clone, Debug, PartialEq)]
pub struct Fold {
    pub task: String,
    pub features: Option<Features>,
    pub oracle: Option<String>,
    pub oracle_objective: Option<f64>,
    /// The rule fitted without this task.
    pub rule: Rule,
    pub picks: Vec<Pick>,
}

/// The evaluation over every fold.
#[derive(Clone, Debug, Default)]
pub struct Evaluation {
    pub candidates: Vec<String>,
    pub folds: Vec<Fold>,
    pub routers: Vec<String>,
    pub held_out: Vec<String>,
    pub errors: Vec<String>,
}

/// The router names, in report order.
pub const ROUTERS: &[&str] = &[
    "fitted stump",
    "jev choice",
    "hand rule",
    "fixed luna",
    "fixed opus",
];

/// Leave-one-task-out evaluation of every router on `matrix`.
#[must_use]
pub fn evaluate(matrix: &Matrix, features: &BTreeMap<String, Features>) -> Evaluation {
    let candidates = matrix.complete_keys();
    let tasks = matrix.tasks.clone();
    let profile_key = |name: &str| {
        PROFILES
            .iter()
            .find(|(profile, _)| *profile == name)
            .map(|(_, key)| (*key).to_owned())
    };
    let folds = tasks
        .iter()
        .map(|task| {
            let train: Vec<String> = tasks.iter().filter(|t| *t != task).cloned().collect();
            let rule = fit(matrix, &candidates, &train, features);
            let f = features.get(task).cloned();
            let oracle = best(matrix, &candidates, task);
            let oracle_objective = oracle.as_ref().map(|key| objective(matrix, task, key));
            let hand = f.as_ref().map(|f| {
                if f.value("concurrency") >= 0.5 || f.value("difficulty") >= 0.75 {
                    FIXED_OPUS.to_owned()
                } else {
                    "coder-one-jevprobe2-luna".to_owned()
                }
            });
            let choice = f
                .as_ref()
                .and_then(|f| f.choice.as_deref())
                .and_then(profile_key);
            let fitted = f.as_ref().map(|f| rule.pick(f).to_owned());
            let profile_cost = f.as_ref().and_then(|f| f.cost).unwrap_or(0.0);
            let entries: Vec<(&str, Option<String>, bool)> = vec![
                ("fitted stump", fitted, true),
                ("jev choice", choice, true),
                ("hand rule", hand, true),
                ("fixed luna", Some(FIXED_LUNA.to_owned()), false),
                ("fixed opus", Some(FIXED_OPUS.to_owned()), false),
            ];
            let picks = entries
                .into_iter()
                .map(|(router, key, reads)| {
                    let cell = key.as_ref().and_then(|key| matrix.cell(task, key));
                    let objective = cell
                        .and_then(|cell| cell.objective)
                        .map(|j| j + if reads { profile_cost } else { 0.0 });
                    Pick {
                        router: router.to_owned(),
                        regret: objective.zip(oracle_objective).map(|(j, best)| j - best),
                        objective,
                        passes: cell.map_or(0, |cell| cell.passes),
                        trials: cell.map_or(0, |cell| cell.trials),
                        cost: cell
                            .and_then(|cell| cell.mean_cost)
                            .map(|c| c + if reads { profile_cost } else { 0.0 }),
                        seconds: cell.and_then(|cell| cell.mean_seconds),
                        key,
                    }
                })
                .collect();
            Fold {
                task: task.clone(),
                features: f,
                oracle,
                oracle_objective,
                rule,
                picks,
            }
        })
        .collect();
    Evaluation {
        candidates,
        folds,
        routers: ROUTERS.iter().map(|r| (*r).to_owned()).collect(),
        held_out: Vec::new(),
        errors: Vec::new(),
    }
}

/// One router's totals over the folds.
#[derive(Clone, Debug, PartialEq)]
pub struct Total {
    pub router: String,
    pub folds: usize,
    pub mean_regret: Option<f64>,
    pub passes: usize,
    pub trials: usize,
    pub cost: Option<f64>,
    pub seconds: Option<f64>,
    pub objective: Option<f64>,
}

impl Evaluation {
    #[must_use]
    pub fn totals(&self) -> Vec<Total> {
        self.routers
            .iter()
            .map(|router| {
                let picks: Vec<&Pick> = self
                    .folds
                    .iter()
                    .filter_map(|fold| fold.picks.iter().find(|p| &p.router == router))
                    .collect();
                let regrets: Option<Vec<f64>> = picks.iter().map(|p| p.regret).collect();
                Total {
                    router: router.clone(),
                    folds: picks.len(),
                    mean_regret: regrets
                        .filter(|r| !r.is_empty())
                        .map(|r| r.iter().sum::<f64>() / r.len() as f64),
                    passes: picks.iter().map(|p| p.passes).sum(),
                    trials: picks.iter().map(|p| p.trials).sum(),
                    cost: picks.iter().map(|p| p.cost).sum(),
                    seconds: picks.iter().map(|p| p.seconds).sum(),
                    objective: picks.iter().map(|p| p.objective).sum(),
                }
            })
            .collect()
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "method": "leave-one-task-out: the held-out task is excluded from the portfolio, the feature, the threshold, and both sides' policies; routers that read features pay the profile request",
            "candidates": self.candidates,
            "hand_rule": HAND_RULE,
            "profiles": PROFILES.iter().map(|(name, key)| json!({ "name": name, "key": key })).collect::<Vec<_>>(),
            "folds": self.folds.iter().map(|fold| json!({
                "task": fold.task,
                "features": fold.features.as_ref().map(|f| json!({ "values": f.values, "choice": f.choice, "profile_cost_usd": f.cost })),
                "oracle": fold.oracle,
                "oracle_objective_usd": fold.oracle_objective,
                "fitted_rule": fold.rule.describe(),
                "picks": fold.picks.iter().map(|p| json!({
                    "router": p.router, "key": p.key, "objective_usd": p.objective, "regret_usd": p.regret,
                    "passes": p.passes, "trials": p.trials, "mean_cost_usd": p.cost, "mean_agent_seconds": p.seconds,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "totals": self.totals().iter().map(|t| json!({
                "router": t.router, "folds": t.folds, "mean_regret_usd": t.mean_regret,
                "passes": t.passes, "trials": t.trials, "summed_mean_cost_usd": t.cost,
                "summed_mean_agent_seconds": t.seconds, "summed_objective_usd": t.objective,
            })).collect::<Vec<_>>(),
            "held_out": {
                "tasks": self.held_out,
                "confirmed": false,
                "note": "No held-out task has run; confirmation needs trials of the frozen held-out split.",
            },
            "errors": self.errors,
        })
    }

    /// The evaluation as text lines.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let short = |value: Option<f64>| value.map_or("—".to_owned(), |v| format!("{v:.2}"));
        let mut lines = vec![
            format!(
                "Router · leave-one-task-out over {} tasks · {} candidate policies ran every task",
                self.folds.len(),
                self.candidates.len()
            ),
            format!("  hand rule: {HAND_RULE}"),
        ];
        for fold in &self.folds {
            lines.push(String::new());
            let features = fold.features.as_ref().map_or_else(
                || "no features".to_owned(),
                |f| {
                    format!(
                        "builds {} · packages {} · data {} · git {} · concurrency {} · difficulty {} · Jev choice {}",
                        short(f.values.get("builds_code").copied().flatten()),
                        short(f.values.get("installs_packages").copied().flatten()),
                        short(f.values.get("parses_data").copied().flatten()),
                        short(f.values.get("recovers_git").copied().flatten()),
                        short(f.values.get("concurrency").copied().flatten()),
                        short(f.values.get("difficulty").copied().flatten()),
                        f.choice.as_deref().unwrap_or("—")
                    )
                },
            );
            lines.push(format!("{} · {features}", fold.task));
            lines.push(format!("  fitted without it: {}", fold.rule.describe()));
            lines.push(format!(
                "  {:<14} {:<40} J {}",
                "oracle",
                fold.oracle.as_deref().unwrap_or("—"),
                fold.oracle_objective
                    .map_or("—".to_owned(), |j| format!("{j:.4}"))
            ));
            for pick in &fold.picks {
                lines.push(format!(
                    "  {:<14} {:<40} J {}  regret {}  {} of {} passed",
                    pick.router,
                    pick.key.as_deref().unwrap_or("—"),
                    pick.objective.map_or("—".to_owned(), |j| format!("{j:.4}")),
                    pick.regret.map_or("—".to_owned(), |r| format!("{r:.4}")),
                    pick.passes,
                    pick.trials
                ));
            }
        }
        lines.push(String::new());
        lines.push(format!(
            "  {:<14} {:>12} {:>8} {:>10} {:>10} {:>9}",
            "router", "mean regret", "passed", "cost", "time", "J"
        ));
        for total in self.totals() {
            lines.push(format!(
                "  {:<14} {:>12} {:>8} {:>10} {:>10} {:>9}",
                total.router,
                total
                    .mean_regret
                    .map_or("—".to_owned(), |r| format!("${r:.4}")),
                format!("{}/{}", total.passes, total.trials),
                total
                    .cost
                    .map_or("unknown".to_owned(), |c| format!("${c:.4}")),
                total
                    .seconds
                    .map_or("—".to_owned(), |s| format!("{s:.1} s")),
                total
                    .objective
                    .map_or("—".to_owned(), |j| format!("{j:.3}"))
            ));
        }
        lines.push(format!(
            "  Held-out confirmation: not run. {} tasks are frozen in bench/terminal-bench/profiles/task-pool.json.",
            self.held_out.len()
        ));
        for error in &self.errors {
            lines.push(format!("  unavailable: {error}"));
        }
        lines
    }
}

/// Reads the frozen held-out task IDs from the task pool.
///
/// # Errors
///
/// Returns a message when the pool doesn't read.
pub fn held_out(path: &Path) -> Result<Vec<String>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(value["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|task| task["split"].as_str() == Some("held-out"))
        .filter_map(|task| task["id"].as_str().map(str::to_owned))
        .collect())
}

/// The evaluation from the checked-in features and pool.
#[must_use]
pub fn from_checkout(matrix: &Matrix) -> Evaluation {
    let (features_path, pool_path) = default_paths();
    let mut errors = Vec::new();
    let features = load_features(&features_path).unwrap_or_else(|error| {
        errors.push(error);
        BTreeMap::new()
    });
    let mut evaluation = evaluate(matrix, &features);
    evaluation.held_out = held_out(&pool_path).unwrap_or_else(|error| {
        errors.push(error);
        Vec::new()
    });
    evaluation.errors = errors;
    evaluation
}

/// `gym coder router …`.
///
/// # Errors
///
/// Returns a message for an unknown option.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        writeln!(
            out,
            "gym coder router [--features PATH] [--json]\n\nEach development task's Jev features, the router's pick fitted without that\ntask, the oracle's pick, and the regret, for the fitted one-split rule, the Jev\nChoice, the hand-written rule, fixed Luna, and fixed Opus.\n\n  --features PATH          a task.profile export (default\n                           bench/terminal-bench/profiles/task-features.json)\n{}",
            crate::coder_matrix::SOURCE_HELP
        )
        .map_err(|error| error.to_string())?;
        return Ok(0);
    }
    let source = Source::parse(args)?;
    let mut features_path = None;
    let mut rest = source.rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--features" => {
                features_path = Some(PathBuf::from(
                    rest.next().ok_or("--features needs a value")?,
                ));
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    let (matrix, _) = source.matrix();
    let mut evaluation = from_checkout(&matrix);
    if let Some(path) = features_path {
        let features = load_features(&path)?;
        let held_out = evaluation.held_out.clone();
        evaluation = evaluate(&matrix, &features);
        evaluation.held_out = held_out;
    }
    if source.json {
        serde_json::to_writer_pretty(&mut *out, &evaluation.to_json())
            .map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in evaluation.lines() {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coder_matrix::Params;
    use crate::terminal_bench::{Attempt, Records};
    use std::collections::BTreeSet;

    fn attempt(task: &str, arm: &str, reward: f64, cost: f64, ms: u64) -> Attempt {
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.task = task.to_owned();
        attempt.arm = arm.to_owned();
        attempt.reward = Some(reward);
        attempt.cost_usd = Some(cost);
        attempt.phases_ms[2] = Some(ms);
        attempt
    }

    fn features(task: &str, concurrency: f64) -> (String, Features) {
        let mut values: BTreeMap<String, Option<f64>> = FEATURES
            .iter()
            .map(|f| ((*f).to_owned(), Some(0.1)))
            .collect();
        values.insert("concurrency".to_owned(), Some(concurrency));
        (
            task.to_owned(),
            Features {
                task: task.to_owned(),
                values,
                choice: Some("opus-lean".to_owned()),
                cost: Some(0.0001),
            },
        )
    }

    /// Luna passes the calm tasks cheaply and fails the concurrent ones;
    /// Opus passes everything at a higher price.
    fn world() -> (Matrix, BTreeMap<String, Features>) {
        let mut attempts = Vec::new();
        let mut features_map = BTreeMap::new();
        for (task, concurrent) in [
            ("a", false),
            ("b", false),
            ("c", true),
            ("d", true),
            ("e", false),
        ] {
            for _ in 0..3 {
                attempts.push(attempt(
                    task,
                    FIXED_LUNA,
                    if concurrent { 0.0 } else { 1.0 },
                    0.002,
                    30_000,
                ));
                attempts.push(attempt(task, FIXED_OPUS, 1.0, 0.05, 15_000));
            }
            let (name, f) = features(task, if concurrent { 0.9 } else { 0.1 });
            features_map.insert(name, f);
        }
        let records = Records {
            attempts,
            ..Records::default()
        };
        (
            Matrix::from_records(&records, Params::default(), None),
            features_map,
        )
    }

    #[test]
    fn the_stump_learns_the_split_and_routes_each_held_out_task() {
        let (matrix, features) = world();
        let evaluation = evaluate(&matrix, &features);
        let totals = evaluation.totals();
        let fitted = totals.iter().find(|t| t.router == "fitted stump").unwrap();
        let luna = totals.iter().find(|t| t.router == "fixed luna").unwrap();
        let opus = totals.iter().find(|t| t.router == "fixed opus").unwrap();
        // The only regret the stump pays is the profile request.
        assert!(fitted.mean_regret.unwrap() < 0.001, "{fitted:?}");
        assert!(luna.mean_regret.unwrap() > fitted.mean_regret.unwrap());
        assert!(opus.mean_regret.unwrap() > fitted.mean_regret.unwrap());
        assert_eq!((fitted.passes, fitted.trials), (15, 15));
        for fold in &evaluation.folds {
            assert!(
                matches!(fold.rule, Rule::Stump { .. }),
                "{}",
                fold.rule.describe()
            );
        }
    }

    #[test]
    fn the_held_out_task_shapes_nothing_in_its_own_fold() {
        let (matrix, mut features) = world();
        // Only `c` is concurrent in training if `d` is held out; the rule
        // fitted without `d` must not change when `d`'s own outcome or
        // features change.
        let train: Vec<String> = ["a", "b", "c", "e"]
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        let before = fit(&matrix, &matrix.complete_keys(), &train, &features);
        features
            .get_mut("d")
            .unwrap()
            .values
            .insert("concurrency".to_owned(), Some(0.0));
        let after = fit(&matrix, &matrix.complete_keys(), &train, &features);
        assert_eq!(before, after);
    }

    #[test]
    fn the_checked_in_features_route_the_development_tasks() {
        let traces =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces");
        let records = Records::load(None, Some(&traces), None);
        let tasks: BTreeSet<String> = crate::coder_matrix::DEVELOPMENT
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        let matrix = Matrix::from_records(&records, Params::default(), Some(&tasks));
        let evaluation = from_checkout(&matrix);
        assert!(evaluation.errors.is_empty(), "{:?}", evaluation.errors);
        assert_eq!(evaluation.folds.len(), 8);
        assert!(evaluation.folds.iter().all(|fold| fold.features.is_some()));
        assert!(evaluation.candidates.contains(&FIXED_LUNA.to_owned()));
        assert!(evaluation.candidates.contains(&FIXED_OPUS.to_owned()));
        assert!(!evaluation.held_out.is_empty());
        let text = evaluation.lines().join("\n");
        assert!(text.contains("fitted without it"), "{text}");
        assert!(text.contains("Held-out confirmation: not run"), "{text}");
    }
}
