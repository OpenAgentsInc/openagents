//! `control.effort`: the reasoning effort a long task runs at, chosen per
//! task.
//!
//! One Jev request asks six Nouls over the task text and its early
//! evidence, the files the workspace starts with. Each Noul is phrased so
//! that yes means more reasoning is likelier to change the outcome. A
//! weighted mean of the answers at or above a threshold raises the effort
//! (`xhigh`); below it the task keeps the base effort (`medium`). A task
//! whose request fails gets the raised effort, the choice that costs money
//! rather than a pass.
//!
//! The weights and the threshold are fitted on the unused task pool
//! (`bench/terminal-bench/profiles/task-pool.json`), never on the scored
//! Terminal-Bench 4.0 tasks. A pool task's label comes from the public
//! Terminal-Bench 2.1 leaderboard: the task is effort-sensitive when a
//! stronger configuration of the same agent passes it at least 0.4 more
//! often than a weaker one. [`fit`] weights each Noul by how far its
//! development-split AUC is above chance and picks the threshold with the
//! best Youden index there; the held-out split only reports.
//!
//! `coder-one effort features` asks the battery over the pool and writes
//! `bench/terminal-bench/profiles/effort-features.json`; `coder-one effort
//! fit` reads that file and prints the fitted rule a manifest copies.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use jev::{Noul, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::record::Implementation;

/// The component's ID.
pub const COMPONENT: &str = "control.effort";

/// The only rule this build runs.
pub const RULE: &str = "sensitivity-v1";

/// The schema of the exported pool features.
pub const SCHEMA: &str = "openagents.coder-one.effort-features.v1";

/// Where the pool features live, from the repository root.
pub const FEATURES_FILE: &str = "bench/terminal-bench/profiles/effort-features.json";

/// The most task characters the request carries.
const TASK_CHARS: usize = 8_000;

/// The most workspace paths the request carries.
pub const WORKSPACE_FILES: usize = 60;

/// The Nouls, in order: ID and question. Yes means more reasoning is
/// likelier to change the outcome.
pub const NOULS: &[(&str, &str)] = &[
    (
        "domain_knowledge",
        "Does the task in `task` depend on specialized scientific, engineering, financial, or legal knowledge beyond general programming to get the result right?",
    ),
    (
        "hidden_exactness",
        "Is a checker of the task in `task` likely to test exact values, formats, or behaviors that the task states only in part, so that a plausible-looking result can still fail?",
    ),
    (
        "long_reasoning",
        "Does the task in `task` require working out a solution through a long chain of dependent reasoning, such as a derivation, an algorithm design, a proof, or a diagnosis, rather than following known steps?",
    ),
    (
        "faithful_reproduction",
        "Does the task in `task` ask for existing behavior, a reference, or a specification to be reproduced or ported exactly, in another language, tool, or format?",
    ),
    (
        "unverifiable",
        "Would an agent doing the task in `task` lack a direct way to confirm its own result, such as given tests to run or a stated expected output to compare against?",
    ),
    (
        "close_reading",
        "Do the starting files in `workspace.files` include inputs that the task in `task` needs read closely, such as data sets, source code to port or fix, or reference outputs?",
    ),
];

/// The battery's request over a task and its starting files.
#[must_use]
pub fn request(task: &str, workspace: &[String]) -> (Value, Questions) {
    let mut questions = Questions::new();
    for (id, question) in NOULS {
        questions = questions.with(*id, Noul::new(*question));
    }
    let files: Vec<&String> = workspace.iter().take(WORKSPACE_FILES).collect();
    let state = json!({
        "task": crate::judge::clip(task, TASK_CHARS),
        "workspace": { "files": files, "listed": files.len(), "total": workspace.len() },
    });
    (state, questions)
}

/// Each Noul's probability out of an answers object; `None` when unknown.
#[must_use]
pub fn read(answers: Option<&Value>) -> BTreeMap<String, Option<f64>> {
    NOULS
        .iter()
        .map(|(id, _)| {
            (
                (*id).to_string(),
                answers
                    .and_then(|a| a.get(*id))
                    .and_then(|a| a.get("noul"))
                    .and_then(Value::as_f64),
            )
        })
        .collect()
}

/// The battery's parameters, digested into its implementation.
#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        COMPONENT,
        "Jev effort-sensitivity battery",
        &json!({ "nouls": NOULS, "task_chars": TASK_CHARS, "workspace_files": WORKSPACE_FILES }),
    )
}

/// The files a workspace starts with, relative to `root`, sorted: at most
/// `limit`, at most three directories deep, without hidden entries or
/// dependency and build directories.
#[must_use]
pub fn workspace_files(root: &Path, limit: usize) -> Vec<String> {
    const SKIP: &[&str] = &["node_modules", "target", "__pycache__", "venv", "dist"];
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || SKIP.contains(&name.as_str()) {
                continue;
            }
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() && depth < 3 => stack.push((path, depth + 1)),
                // A huge tree stops the walk rather than the episode.
                Ok(kind) if kind.is_file() && out.len() < 20_000 => {
                    if let Ok(relative) = path.strip_prefix(root) {
                        out.push(relative.to_string_lossy().into_owned());
                    }
                }
                _ => {}
            }
        }
    }
    out.sort();
    out.truncate(limit);
    out
}

// ---------------------------------------------------------------------------
// The policy and the decision.
// ---------------------------------------------------------------------------

/// `control.effort`: the effort a long task runs at, from the battery.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffortPolicy {
    /// The rule's name; only `sensitivity-v1` in this build.
    pub rule: String,
    /// The effort a task below the threshold runs at.
    pub base: String,
    /// The effort a task at or above the threshold runs at, and any task
    /// whose features are unknown.
    pub raised: String,
    /// Each Noul's weight in the mean; a Noul left out weighs nothing.
    pub weights: BTreeMap<String, f64>,
    /// The weighted mean at or above which the effort is raised.
    pub at: f64,
    /// Where the weights and threshold came from, for the reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<Value>,
}

fn effort_word(effort: &str) -> bool {
    !effort.is_empty() && effort.chars().all(|c| c.is_ascii_lowercase())
}

impl EffortPolicy {
    /// Refuses a policy this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.rule != RULE {
            problems.push(format!(
                "control.effort.rule {} is unknown; this build runs {RULE}",
                self.rule
            ));
        }
        for (field, effort) in [("base", &self.base), ("raised", &self.raised)] {
            if !effort_word(effort) {
                problems.push(format!("control.effort.{field} must be one lowercase word"));
            }
        }
        for (id, weight) in &self.weights {
            if !NOULS.iter().any(|(known, _)| known == id) {
                problems.push(format!(
                    "control.effort.weights names {id}, which the battery doesn't ask"
                ));
            }
            if !(weight.is_finite() && *weight >= 0.0) {
                problems.push(format!("control.effort.weights.{id} must be zero or more"));
            }
        }
        if self.weights.values().sum::<f64>() <= 0.0 {
            problems.push("control.effort.weights must weigh at least one Noul".to_string());
        }
        if !(0.0..=1.0).contains(&self.at) {
            problems.push("control.effort.at must be from 0 to 1".to_string());
        }
        problems
    }

    /// The weighted mean of the known features; `None` when no weighted
    /// feature is known.
    #[must_use]
    pub fn score(&self, features: &BTreeMap<String, Option<f64>>) -> Option<f64> {
        let (sum, weight) = self
            .weights
            .iter()
            .filter(|(_, w)| **w > 0.0)
            .filter_map(|(id, w)| features.get(id).copied().flatten().map(|p| (p * w, *w)))
            .fold((0.0, 0.0), |(s, t), (p, w)| (s + p, t + w));
        (weight > 0.0).then(|| round3(sum / weight))
    }

    /// The effort for a task with these features, and why.
    #[must_use]
    pub fn decide(&self, features: &BTreeMap<String, Option<f64>>) -> Decided {
        match self.score(features) {
            None => Decided {
                effort: self.raised.clone(),
                score: None,
                reason: format!(
                    "no weighted feature is known, so the task runs at {}",
                    self.raised
                ),
            },
            Some(score) if score >= self.at => Decided {
                effort: self.raised.clone(),
                score: Some(score),
                reason: format!("sensitivity {score:.3} is at or above {:.3}", self.at),
            },
            Some(score) => Decided {
                effort: self.base.clone(),
                score: Some(score),
                reason: format!("sensitivity {score:.3} is below {:.3}", self.at),
            },
        }
    }
}

/// The effort chosen for one task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decided {
    pub effort: String,
    /// The weighted mean; `None` when no weighted feature was known.
    pub score: Option<f64>,
    pub reason: String,
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

// ---------------------------------------------------------------------------
// Labels from the Terminal-Bench 2.1 leaderboard.
// ---------------------------------------------------------------------------

/// The label rule, as the features file states it.
pub const LABEL_RULE: &str = "A pool task is effort-sensitive when a stronger configuration of the same agent passes it at least 0.4 more often than a weaker one on the Terminal-Bench 2.1 leaderboard: Codex on GPT-6 Astra at xhigh and max against low and medium (effort alone), or Claude Code on Fable 5 at xhigh and Opus 4.8 at high against Sonnet 5 at high and Opus 4.7 at max (more reasoning capability). The gain is the larger of the two differences in pass rate.";

/// The least gain that labels a task sensitive.
pub const LABEL_GAIN: f64 = 0.4;

/// The leaderboard rows each side of the label compares, as agent, model,
/// and effort.
const ASTRA_WEAK: &[(&str, &str, &str)] = &[
    ("Codex", "GPT-6 Astra", "low"),
    ("Codex", "GPT-6 Astra", "medium"),
];
const ASTRA_STRONG: &[(&str, &str, &str)] = &[
    ("Codex", "GPT-6 Astra", "xhigh"),
    ("Codex", "GPT-6 Astra", "max"),
];
const CLAUDE_WEAK: &[(&str, &str, &str)] = &[
    ("Claude Code", "Sonnet 5", "high"),
    ("Claude Code", "Opus 4.7", "max"),
];
const CLAUDE_STRONG: &[(&str, &str, &str)] = &[
    ("Claude Code", "Fable 5", "xhigh"),
    ("Claude Code", "Opus 4.8", "high"),
];

/// A task's label and the pass rates behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub sensitive: bool,
    pub gain: f64,
    pub astra: [f64; 2],
    pub claude_code: [f64; 2],
}

/// The pass rate of `rows` on `task` in a reference document.
fn rate(reference: &Value, rows: &[(&str, &str, &str)], task: &str) -> Result<f64, String> {
    let entries = reference["entries"].as_array().ok_or("no entries")?;
    let (mut passed, mut trials) = (0.0, 0.0);
    for (agent, model, effort) in rows {
        let entry = entries
            .iter()
            .find(|e| {
                e["agent"] == *agent && e["model"] == *model && e["reasoning_effort"] == *effort
            })
            .ok_or_else(|| format!("the reference has no {agent} / {model} ({effort}) row"))?;
        let cell = &entry["tasks"][task];
        passed += cell["successes"].as_f64().unwrap_or(0.0);
        trials += cell["trials"].as_f64().unwrap_or(0.0);
    }
    if trials == 0.0 {
        return Err(format!("the reference has no trials of {task}"));
    }
    Ok(passed / trials)
}

/// A pool task's label under [`LABEL_RULE`].
///
/// # Errors
///
/// Returns a message when a compared row or the task's trials are missing.
pub fn label(reference: &Value, task: &str) -> Result<Label, String> {
    let astra = [
        round3(rate(reference, ASTRA_WEAK, task)?),
        round3(rate(reference, ASTRA_STRONG, task)?),
    ];
    let claude_code = [
        round3(rate(reference, CLAUDE_WEAK, task)?),
        round3(rate(reference, CLAUDE_STRONG, task)?),
    ];
    let gain = round3((astra[1] - astra[0]).max(claude_code[1] - claude_code[0]));
    Ok(Label {
        sensitive: gain >= LABEL_GAIN,
        gain,
        astra,
        claude_code,
    })
}

// ---------------------------------------------------------------------------
// The fit.
// ---------------------------------------------------------------------------

/// One pool task in the features file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoolTask {
    pub task: String,
    /// `development`, `development-current`, or `held-out`.
    pub split: String,
    pub features: BTreeMap<String, Option<f64>>,
    pub label: Label,
    #[serde(default)]
    pub workspace_files: usize,
    #[serde(default)]
    pub jev_usd: Option<f64>,
}

impl PoolTask {
    fn fits(&self) -> bool {
        self.split.starts_with("development")
    }
}

/// How a rule's picks meet the labels on one split.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Confusion {
    pub tasks: usize,
    pub sensitive: usize,
    /// Sensitive tasks raised.
    pub caught: usize,
    /// Other tasks raised.
    pub raised_other: usize,
}

impl Confusion {
    fn of(policy: &EffortPolicy, tasks: &[&PoolTask]) -> Self {
        let mut confusion = Confusion::default();
        for task in tasks {
            let raised = policy.decide(&task.features).effort == policy.raised;
            confusion.tasks += 1;
            confusion.sensitive += usize::from(task.label.sensitive);
            confusion.caught += usize::from(raised && task.label.sensitive);
            confusion.raised_other += usize::from(raised && !task.label.sensitive);
        }
        confusion
    }

    /// Caught over sensitive, less raised others over other tasks.
    #[must_use]
    pub fn youden(&self) -> f64 {
        let others = self.tasks - self.sensitive;
        let tpr = if self.sensitive == 0 {
            0.0
        } else {
            self.caught as f64 / self.sensitive as f64
        };
        let fpr = if others == 0 {
            0.0
        } else {
            self.raised_other as f64 / others as f64
        };
        tpr - fpr
    }
}

/// The fitted rule and how it reads on each split.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fitted {
    pub weights: BTreeMap<String, f64>,
    pub at: f64,
    /// Each Noul's AUC against the labels on the development split.
    pub auc: BTreeMap<String, f64>,
    pub development: Confusion,
    pub held_out: Confusion,
}

/// The probability that a sensitive task's value exceeds another's, ties
/// counting half.
fn auc(pairs: &[(f64, bool)]) -> f64 {
    let positives: Vec<f64> = pairs.iter().filter(|p| p.1).map(|p| p.0).collect();
    let negatives: Vec<f64> = pairs.iter().filter(|p| !p.1).map(|p| p.0).collect();
    if positives.is_empty() || negatives.is_empty() {
        return 0.5;
    }
    let mut wins = 0.0;
    for p in &positives {
        for n in &negatives {
            wins += if p > n {
                1.0
            } else if (p - n).abs() < f64::EPSILON {
                0.5
            } else {
                0.0
            };
        }
    }
    wins / (positives.len() * negatives.len()) as f64
}

/// Fits the weights and threshold on the development split of `pool`.
/// A Noul weighs its development AUC above 0.5, so a Noul that doesn't
/// separate the labels weighs nothing; with none that does, every Noul
/// weighs the same. The threshold is the score with the best Youden index
/// on the development split, the higher one on a tie, since a lower
/// threshold raises more tasks and costs more.
#[must_use]
pub fn fit(pool: &[PoolTask], base: &str, raised: &str) -> Fitted {
    let development: Vec<&PoolTask> = pool.iter().filter(|t| t.fits()).collect();
    let held_out: Vec<&PoolTask> = pool.iter().filter(|t| t.split == "held-out").collect();
    let mut aucs = BTreeMap::new();
    for (id, _) in NOULS {
        let pairs: Vec<(f64, bool)> = development
            .iter()
            .filter_map(|t| {
                t.features
                    .get(*id)
                    .copied()
                    .flatten()
                    .map(|p| (p, t.label.sensitive))
            })
            .collect();
        aucs.insert((*id).to_string(), round3(auc(&pairs)));
    }
    let mut weights: BTreeMap<String, f64> = aucs
        .iter()
        .map(|(id, a)| (id.clone(), round3((a - 0.5).max(0.0))))
        .filter(|(_, w)| *w > 0.0)
        .collect();
    if weights.is_empty() {
        weights = NOULS
            .iter()
            .map(|(id, _)| ((*id).to_string(), 1.0))
            .collect();
    }
    let mut policy = EffortPolicy {
        rule: RULE.to_string(),
        base: base.to_string(),
        raised: raised.to_string(),
        weights,
        at: 1.0,
        fit: None,
    };
    let mut candidates: Vec<f64> = development
        .iter()
        .filter_map(|t| policy.score(&t.features))
        .collect();
    candidates.sort_by(f64::total_cmp);
    candidates.dedup();
    let mut best: Option<(f64, f64)> = None;
    for at in candidates.into_iter().rev() {
        policy.at = at;
        let j = Confusion::of(&policy, &development).youden();
        if best.is_none_or(|(_, top)| j > top + 1e-9) {
            best = Some((at, j));
        }
    }
    policy.at = best.map_or(0.5, |(at, _)| at);
    Fitted {
        development: Confusion::of(&policy, &development),
        held_out: Confusion::of(&policy, &held_out),
        weights: policy.weights,
        at: policy.at,
        auc: aucs,
    }
}

/// The pool tasks in a features file.
///
/// # Errors
///
/// Returns a message when the file doesn't read or carries another schema.
pub fn load(path: &Path) -> Result<(Value, Vec<PoolTask>), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    if document["schema"] != SCHEMA {
        return Err(format!("{} is not {SCHEMA}", path.display()));
    }
    let tasks = serde_json::from_value(document["tasks"].clone())
        .map_err(|error| format!("{} has unreadable tasks: {error}", path.display()))?;
    Ok((document, tasks))
}

// ---------------------------------------------------------------------------
// The command.
// ---------------------------------------------------------------------------

/// `coder-one effort`'s usage.
pub const USAGE: &str = "usage:
  coder-one effort features --pool FILE --checkout DIR --reference FILE --out FILE
      asks the effort battery live over every development and held-out task
      in the pool, labels each from the reference, and writes the features
  coder-one effort fit [--features FILE] [--base medium] [--raised xhigh] [--json]
      fits the weights and threshold on the development split and reports
      both splits";

/// Runs `coder-one effort`.
///
/// # Errors
///
/// Returns a message on bad arguments, a missing file, or no Jev key.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    match args.first().map(String::as_str) {
        Some("features") => {
            let (Some(pool), Some(checkout), Some(reference), Some(out)) = (
                flag("--pool"),
                flag("--checkout"),
                flag("--reference"),
                flag("--out"),
            ) else {
                return Err(USAGE.to_string());
            };
            features(
                Path::new(&pool),
                Path::new(&checkout),
                Path::new(&reference),
                Path::new(&out),
            )
            .await
            .map(|()| 0)
        }
        Some("fit") => {
            let path = flag("--features").unwrap_or_else(|| FEATURES_FILE.to_string());
            let (_, pool) = load(Path::new(&path))?;
            let fitted = fit(
                &pool,
                &flag("--base").unwrap_or_else(|| "medium".to_string()),
                &flag("--raised").unwrap_or_else(|| "xhigh".to_string()),
            );
            if args.iter().any(|a| a == "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&fitted).unwrap_or_default()
                );
            } else {
                for line in fit_lines(&fitted) {
                    println!("{line}");
                }
            }
            Ok(0)
        }
        Some("help") => {
            println!("{USAGE}");
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

/// The fit as text.
#[must_use]
pub fn fit_lines(fitted: &Fitted) -> Vec<String> {
    let mut lines = vec!["Noul AUC on the development split, and its weight:".to_string()];
    for (id, a) in &fitted.auc {
        lines.push(format!(
            "  {id:<22} AUC {a:.3}  weight {:.3}",
            fitted.weights.get(id).copied().unwrap_or(0.0)
        ));
    }
    lines.push(format!("Threshold: {:.3}", fitted.at));
    for (split, c) in [
        ("development", &fitted.development),
        ("held-out", &fitted.held_out),
    ] {
        lines.push(format!(
            "  {split:<12} {} tasks, {} sensitive: raised {} of {} sensitive and {} of {} others (Youden {:.3})",
            c.tasks,
            c.sensitive,
            c.caught,
            c.sensitive,
            c.raised_other,
            c.tasks - c.sensitive,
            c.youden()
        ));
    }
    lines
}

/// Asks the battery over the pool and writes the features file.
async fn features(
    pool: &Path,
    checkout: &Path,
    reference: &Path,
    out: &Path,
) -> Result<(), String> {
    let load_json = |path: &Path| -> Result<Value, String> {
        serde_json::from_str(
            &std::fs::read_to_string(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("{} is not JSON: {error}", path.display()))
    };
    let pool_doc = load_json(pool)?;
    let reference_doc = load_json(reference)?;
    let client = crate::component::cli::live_client()?;
    let mode = crate::component::jev::JevMode::Live(client);
    let recorder = crate::record::Recorder::default();
    let mut tasks = Vec::new();
    let mut spent = 0.0;
    for task in pool_doc["tasks"].as_array().into_iter().flatten() {
        let split = task["split"].as_str().unwrap_or_default();
        if !(split.starts_with("development") || split == "held-out") {
            continue;
        }
        let id = task["id"].as_str().unwrap_or_default();
        let dir = checkout.join(task["path"].as_str().unwrap_or_default());
        let instruction = std::fs::read_to_string(dir.join("instruction.md"))
            .map_err(|error| format!("cannot read {id}'s instruction: {error}"))?;
        let workspace: Vec<String> = workspace_files(&dir.join("environment"), 400)
            .into_iter()
            .filter(|p| !p.starts_with("Dockerfile") && !p.starts_with("docker-compose"))
            .collect();
        let (state, questions) = request(&instruction, &workspace);
        let asked = crate::component::jev::ask(
            &mode,
            &recorder,
            crate::component::jev::Ask {
                component: COMPONENT,
                name: "jev_effort",
                id: format!("jev-effort-{id}"),
                state,
                questions,
                parent: None,
                deadline: None,
            },
        )
        .await;
        if let Some(error) = &asked.error {
            return Err(format!("Jev failed on {id}: {error}"));
        }
        let usd = asked.input_tokens.map(|tokens| {
            tokens as f64 * crate::component::jev::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        spent += usd.unwrap_or(0.0);
        let label = label(&reference_doc, id)?;
        println!(
            "{id:<36} {split:<20} gain {:+.2}{}",
            label.gain,
            if label.sensitive { " sensitive" } else { "" }
        );
        tasks.push(PoolTask {
            task: id.to_string(),
            split: split.to_string(),
            features: read(asked.answers.as_ref()),
            label,
            workspace_files: workspace.len(),
            jev_usd: usd.map(|u| (u * 1e9).round() / 1e9),
        });
    }
    let document = json!({
        "schema": SCHEMA,
        "component": COMPONENT,
        "implementation": implementation(),
        "pool": { "path": "bench/terminal-bench/profiles/task-pool.json", "held_out_sha256": pool_doc["held_out_sha256"] },
        "label": {
            "rule": LABEL_RULE,
            "gain": LABEL_GAIN,
            "reference": reference_doc["leaderboard"],
            "fetched_at": reference_doc["fetched_at"],
        },
        "workspace": "The files under the task's environment directory, without the Dockerfile or compose file: what the container starts with.",
        "jev": { "requests": tasks.len(), "usd": (spent * 1e6).round() / 1e6 },
        "tasks": tasks,
    });
    std::fs::write(
        out,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document).unwrap_or_default()
        ),
    )
    .map_err(|error| format!("cannot write {}: {error}", out.display()))?;
    println!(
        "wrote {} ({} tasks, Jev ${spent:.6})",
        out.display(),
        tasks.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> EffortPolicy {
        EffortPolicy {
            rule: RULE.to_string(),
            base: "medium".to_string(),
            raised: "xhigh".to_string(),
            weights: [
                ("domain_knowledge".to_string(), 0.2),
                ("unverifiable".to_string(), 0.1),
            ]
            .into_iter()
            .collect(),
            at: 0.5,
            fit: None,
        }
    }

    fn features(pairs: &[(&str, f64)]) -> BTreeMap<String, Option<f64>> {
        let mut map: BTreeMap<String, Option<f64>> = NOULS
            .iter()
            .map(|(id, _)| ((*id).to_string(), None))
            .collect();
        for (id, p) in pairs {
            map.insert((*id).to_string(), Some(*p));
        }
        map
    }

    #[test]
    fn the_battery_asks_six_nouls_over_the_task_and_its_files() {
        let files: Vec<String> = (0..100).map(|i| format!("data/{i}.csv")).collect();
        let (state, questions) = request("Port the macro.", &files);
        assert_eq!(questions.len(), 6);
        assert_eq!(state["workspace"]["listed"], 60);
        assert_eq!(state["workspace"]["total"], 100);
        assert_eq!(state["task"], "Port the macro.");
    }

    #[test]
    fn the_weighted_mean_raises_the_effort_at_the_threshold() {
        let policy = policy();
        let high = policy.decide(&features(&[
            ("domain_knowledge", 0.9),
            ("unverifiable", 0.3),
        ]));
        assert_eq!(high.effort, "xhigh");
        assert_eq!(high.score, Some(0.7));
        let low = policy.decide(&features(&[
            ("domain_knowledge", 0.2),
            ("unverifiable", 0.9),
        ]));
        assert_eq!(low.effort, "medium");
        assert!(low.reason.contains("below 0.500"), "{}", low.reason);
        // An unweighted feature doesn't move the score.
        let same = policy.decide(&features(&[
            ("domain_knowledge", 0.9),
            ("unverifiable", 0.3),
            ("close_reading", 0.0),
        ]));
        assert_eq!(same.score, Some(0.7));
    }

    #[test]
    fn unknown_features_raise_the_effort() {
        let decided = policy().decide(&features(&[]));
        assert_eq!(decided.effort, "xhigh");
        assert_eq!(decided.score, None);
    }

    #[test]
    fn a_policy_names_only_battery_nouls_and_lowercase_efforts() {
        assert!(policy().validate().is_empty());
        let mut bad = policy();
        bad.rule = "other".to_string();
        bad.base = "Medium".to_string();
        bad.weights.insert("difficulty".to_string(), 1.0);
        bad.at = 1.5;
        let problems = bad.validate().join("\n");
        for needle in [
            "rule other",
            "base must be",
            "names difficulty",
            "at must be",
        ] {
            assert!(problems.contains(needle), "{problems}");
        }
    }

    fn pool_task(task: &str, split: &str, p: f64, sensitive: bool) -> PoolTask {
        PoolTask {
            task: task.to_string(),
            split: split.to_string(),
            features: features(&[("domain_knowledge", p), ("unverifiable", 0.5)]),
            label: Label {
                sensitive,
                gain: if sensitive { 0.6 } else { 0.0 },
                astra: [1.0, 1.0],
                claude_code: [0.4, 1.0],
            },
            workspace_files: 0,
            jev_usd: None,
        }
    }

    #[test]
    fn the_fit_weighs_separating_nouls_and_picks_the_best_threshold() {
        let pool = vec![
            pool_task("a", "development", 0.9, true),
            pool_task("b", "development", 0.8, true),
            pool_task("c", "development", 0.3, false),
            pool_task("d", "development-current", 0.2, false),
            pool_task("e", "development", 0.85, false),
            pool_task("h", "held-out", 0.95, true),
            pool_task("i", "held-out", 0.1, false),
        ];
        let fitted = fit(&pool, "medium", "xhigh");
        // Only domain_knowledge separates the labels; the constant
        // unverifiable answer ties everywhere.
        assert_eq!(
            fitted.weights.keys().collect::<Vec<_>>(),
            ["domain_knowledge"]
        );
        assert_eq!(fitted.at, 0.8);
        assert_eq!(fitted.development.caught, 2);
        assert_eq!(fitted.development.raised_other, 1);
        assert_eq!(fitted.held_out.caught, 1);
        assert_eq!(fitted.held_out.raised_other, 0);
    }

    #[test]
    fn a_label_compares_stronger_and_weaker_rows_of_one_agent() {
        let entry = |agent: &str, model: &str, effort: &str, s: u64| json!({ "agent": agent, "model": model, "reasoning_effort": effort, "tasks": { "t": { "successes": s, "trials": 5 } } });
        let reference = json!({ "entries": [
            entry("Codex", "GPT-6 Astra", "low", 5),
            entry("Codex", "GPT-6 Astra", "medium", 5),
            entry("Codex", "GPT-6 Astra", "xhigh", 5),
            entry("Codex", "GPT-6 Astra", "max", 5),
            entry("Claude Code", "Sonnet 5", "high", 1),
            entry("Claude Code", "Opus 4.7", "max", 1),
            entry("Claude Code", "Fable 5", "xhigh", 4),
            entry("Claude Code", "Opus 4.8", "high", 4),
        ]});
        let label = label(&reference, "t").unwrap();
        assert!(label.sensitive);
        assert_eq!(label.gain, 0.6);
        assert!(super::label(&reference, "missing").is_err());
    }

    #[test]
    fn the_workspace_lists_files_without_hidden_or_dependency_directories() {
        let dir = tempfile::tempdir().unwrap();
        for path in [
            "b.txt",
            "src/a.rs",
            ".git/HEAD",
            "node_modules/x.js",
            "a/b/c/d/deep.txt",
        ] {
            let full = dir.path().join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, "x").unwrap();
        }
        assert_eq!(workspace_files(dir.path(), 10), ["b.txt", "src/a.rs"]);
        assert_eq!(workspace_files(dir.path(), 1), ["b.txt"]);
    }

    /// The checked-in manifest must carry what the checked-in pool
    /// features fit today.
    #[test]
    fn the_v9_effort_rule_is_the_fit_of_the_pool_features() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok((_, pool)) = load(&root.join(FEATURES_FILE)) else {
            panic!("{FEATURES_FILE} doesn't load");
        };
        let fitted = fit(&pool, "medium", "xhigh");
        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("policies/tunable-v9.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let policy: EffortPolicy =
            serde_json::from_value(manifest["policy"]["control"]["effort"].clone()).unwrap();
        assert_eq!(
            (policy.weights, policy.at),
            (fitted.weights, fitted.at),
            "tunable-v9's control.effort is stale: run `coder-one effort fit`"
        );
    }
}
