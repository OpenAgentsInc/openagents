//! `gym coder routing`: Coder's harness router measured against the best
//! possible choice and against the fixed rule (#11461).
//!
//! Coder's router v0 (`crates/coder-new/src/router.rs`) picks the harness a
//! chat's work runs on: Claude Code or Coder's own loop. This report prices
//! its picks two ways.
//!
//! **Terminal-Bench.** Each development task's instruction was put to the
//! router as a chat's first message; the judgments and the frozen policy's
//! picks are checked in at
//! `bench/terminal-bench/profiles/route-v1-tasks.json`. Each harness stands
//! for the measured arm that ran it on these tasks (the file's
//! `harness_arms`). Per task the report shows the router's pick, the fixed
//! rule's pick (the first admitted harness, Claude Code), and the oracle:
//! the chat harness with the lowest objective on that task, chosen after
//! seeing the outcomes. Neither the router's questions nor its policy were
//! tuned on these tasks, so they are held out for it.
//!
//! **Recorded real turns.** Coder writes a routing record for every turn
//! (`~/.openagents/coder/routing/*.jsonl`): the choice with System One's
//! probabilities, then the outcome, whether the turn's checks passed, and
//! whether the person re-asked it. Turns are grouped by the class System One
//! judged (kind and size). A turn succeeds when it ended with a reply, its
//! checks did not fail, and it was not redone. Per class and harness the
//! report shows turns, success, mean cost and time, and the objective; the
//! class's oracle is the harness with the lowest objective there. These are
//! observed turns, not paired trials: a harness only shows in a class it was
//! sent to.
//!
//! The objective is the outcome matrix's: J = mean cost + usd_per_second ×
//! mean seconds + fail_usd × (1 − success rate), the offline accounting in
//! `docs/optimization/coder-components.md`.
//!
//! **Promotion gate.** A router policy version becomes the default only when
//! its mean regret on held-out tasks is below the fixed rule's, and the
//! recorded real turns, where there are enough of them, do not show it
//! worse.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::coder_matrix::{Matrix, Params, Source};

/// The schema of `gym coder routing --json`.
pub const SCHEMA: &str = "openagents.gym.coder-routing.v1";

/// The harness the fixed rule picks when Claude Code is admitted.
pub const FIXED_RULE: &str = "claude-code";

/// The harnesses a chat turn can run on.
pub const CHAT_HARNESSES: &[&str] = &["claude-code", "coder-loop"];

/// The ended turns a harness needs in a class before its objective counts
/// toward the gate.
pub const MIN_TURNS: usize = 20;

/// The checked-in router picks on the Terminal-Bench tasks.
#[must_use]
pub fn default_picks() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/profiles/route-v1-tasks.json")
}

/// Where Coder writes routing records on this computer.
#[must_use]
pub fn default_records() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CODER_ROUTING_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".openagents/coder/routing"))
}

/// The router's picks on the Terminal-Bench tasks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picks {
    pub set: String,
    pub policy: String,
    /// The measured arm each harness stands for.
    pub arms: BTreeMap<String, String>,
    /// Task, the harness the router picked, and the policy step.
    pub rows: Vec<(String, String, String)>,
}

/// Reads the picks file.
///
/// # Errors
/// The file doesn't read or isn't a route-v1 task set.
pub fn load_picks(path: &Path) -> Result<Picks, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if value["schema"].as_str() != Some("openagents.coder.route-v1-tasks.v1") {
        return Err(format!("{} is not a route-v1 task set", path.display()));
    }
    let arms = value["harness_arms"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(harness, arm)| Some((harness.clone(), arm.as_str()?.to_owned())))
        .collect();
    let rows = value["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            Some((
                row["task"].as_str()?.to_owned(),
                row["pick"]["harness"].as_str()?.to_owned(),
                row["pick"]["step"].as_str()?.to_owned(),
            ))
        })
        .collect();
    Ok(Picks {
        set: value["set"].as_str().unwrap_or_default().to_owned(),
        policy: value["policy"].as_str().unwrap_or_default().to_owned(),
        arms,
        rows,
    })
}

/// One Terminal-Bench task.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRow {
    pub task: String,
    pub router: String,
    pub step: String,
    pub router_objective: Option<f64>,
    pub fixed_objective: Option<f64>,
    /// The chat harness with the lowest objective on the task.
    pub oracle: Option<String>,
    pub oracle_objective: Option<f64>,
    /// Each chat harness's passes, trials, cost, time and objective.
    pub harnesses: BTreeMap<String, HarnessCell>,
}

/// A harness's measured cell on one task.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HarnessCell {
    pub arm: String,
    pub passes: usize,
    pub trials: usize,
    pub cost: Option<f64>,
    pub seconds: Option<f64>,
    pub objective: Option<f64>,
}

impl TaskRow {
    #[must_use]
    pub fn router_regret(&self) -> Option<f64> {
        self.router_objective
            .zip(self.oracle_objective)
            .map(|(j, best)| j - best)
    }

    #[must_use]
    pub fn fixed_regret(&self) -> Option<f64> {
        self.fixed_objective
            .zip(self.oracle_objective)
            .map(|(j, best)| j - best)
    }
}

/// The router, the fixed rule, and the oracle on each task the picks name.
#[must_use]
pub fn tasks(matrix: &Matrix, picks: &Picks) -> Vec<TaskRow> {
    picks
        .rows
        .iter()
        .map(|(task, router, step)| {
            let harnesses: BTreeMap<String, HarnessCell> = CHAT_HARNESSES
                .iter()
                .filter_map(|harness| {
                    let arm = picks.arms.get(*harness)?;
                    let cell = matrix.cell(task, arm);
                    Some((
                        (*harness).to_owned(),
                        HarnessCell {
                            arm: arm.clone(),
                            passes: cell.map_or(0, |c| c.passes),
                            trials: cell.map_or(0, |c| c.trials),
                            cost: cell.and_then(|c| c.mean_cost),
                            seconds: cell.and_then(|c| c.mean_seconds),
                            objective: cell.and_then(|c| c.objective),
                        },
                    ))
                })
                .collect();
            let objective = |harness: &str| harnesses.get(harness).and_then(|h| h.objective);
            let oracle = harnesses
                .iter()
                .filter_map(|(name, cell)| Some((name, cell.objective?)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            TaskRow {
                task: task.clone(),
                router: router.clone(),
                step: step.clone(),
                router_objective: objective(router),
                fixed_objective: objective(FIXED_RULE),
                oracle: oracle.map(|(name, _)| name.clone()),
                oracle_objective: oracle.map(|(_, j)| j),
                harnesses,
            }
        })
        .collect()
}

fn mean(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let values: Option<Vec<f64>> = values.collect();
    values
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().sum::<f64>() / v.len() as f64)
}

/// One recorded turn: its choice and, when it ended, its outcome.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turn {
    pub id: String,
    pub chat: Option<String>,
    pub harness: String,
    pub rule: String,
    pub policy: Option<String>,
    /// System One's judged class, `kind/size`, or `unjudged`.
    pub class: String,
    pub ended: Option<String>,
    pub seconds: Option<f64>,
    pub cost: Option<f64>,
    pub checks_passed: Option<bool>,
    pub redone: bool,
}

impl Turn {
    /// Ended with a reply, its checks did not fail, and it was not redone.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.ended.as_deref() == Some("done") && self.checks_passed != Some(false) && !self.redone
    }
}

fn argmax(probabilities: &[Value], question: &str) -> Option<String> {
    probabilities
        .iter()
        .filter(|p| p["question"].as_str() == Some(question))
        .filter_map(|p| Some((p["answer"].as_str()?, p["p"].as_f64()?)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(answer, _)| answer.to_owned())
}

/// The records' primary turns, with their outcomes and redos. Lines with no
/// chat were written by test runs before records were limited to real
/// chats; they are counted, not read.
#[must_use]
pub fn read_turns(dir: &Path) -> (Vec<Turn>, usize) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    let mut turns: Vec<Turn> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut skipped = 0;
    let mut later = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            match value["type"].as_str() {
                Some("chose") => {
                    if value["kind"].as_str().unwrap_or("primary") != "primary" {
                        continue;
                    }
                    let Some(chat) = value["chat"].as_str() else {
                        skipped += 1;
                        continue;
                    };
                    let probabilities = value["inputs"]["probabilities"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let class = match (
                        argmax(&probabilities, "route-v1.kind"),
                        argmax(&probabilities, "route-v1.size"),
                    ) {
                        (Some(kind), Some(size)) => format!("{kind}/{size}"),
                        _ => "unjudged".to_owned(),
                    };
                    let id = value["id"].as_str().unwrap_or_default().to_owned();
                    index.insert(id.clone(), turns.len());
                    turns.push(Turn {
                        id,
                        chat: Some(chat.to_owned()),
                        harness: value["harness"].as_str().unwrap_or_default().to_owned(),
                        rule: value["rule"].as_str().unwrap_or_default().to_owned(),
                        policy: value["inputs"]["router"]["policy"]
                            .as_str()
                            .map(str::to_owned),
                        class,
                        ..Turn::default()
                    });
                }
                Some("ended" | "redone") => later.push(value),
                _ => {}
            }
        }
    }
    for value in later {
        let Some(&at) = value["id"].as_str().and_then(|id| index.get(id)) else {
            continue;
        };
        let turn = &mut turns[at];
        if value["type"] == "redone" {
            turn.redone = true;
            continue;
        }
        turn.ended = value["ended"].as_str().map(str::to_owned);
        turn.seconds = value["elapsed_ms"].as_f64().map(|ms| ms / 1000.0);
        turn.cost = value["cost_usd"].as_f64();
        turn.checks_passed = value["checks_passed"].as_bool();
        turn.redone |= value["redone"].as_bool() == Some(true);
    }
    (turns, skipped)
}

/// A group of ended turns, priced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub turns: usize,
    pub succeeded: usize,
    pub checked: usize,
    pub checks_passed: usize,
    pub redone: usize,
    pub cost: Option<f64>,
    pub seconds: Option<f64>,
    pub objective: Option<f64>,
}

fn group<'a>(turns: impl Iterator<Item = &'a Turn>, params: &Params) -> Group {
    let ended: Vec<&Turn> = turns.filter(|turn| turn.ended.is_some()).collect();
    let n = ended.len();
    let succeeded = ended.iter().filter(|turn| turn.succeeded()).count();
    let cost = mean(ended.iter().map(|turn| Some(turn.cost.unwrap_or(0.0))));
    let seconds = mean(ended.iter().map(|turn| turn.seconds));
    Group {
        turns: n,
        succeeded,
        checked: ended.iter().filter(|t| t.checks_passed.is_some()).count(),
        checks_passed: ended
            .iter()
            .filter(|t| t.checks_passed == Some(true))
            .count(),
        redone: ended.iter().filter(|t| t.redone).count(),
        objective: cost.zip(seconds).map(|(cost, seconds)| {
            cost + params.usd_per_second * seconds
                + params.fail_usd * (1.0 - succeeded as f64 / n as f64)
        }),
        cost,
        seconds,
    }
}

/// One judged class of recorded turns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassRow {
    pub class: String,
    pub harnesses: BTreeMap<String, Group>,
    /// The turns the router picked, whatever harness.
    pub router: Group,
    pub oracle: Option<String>,
}

impl ClassRow {
    fn oracle_objective(&self) -> Option<f64> {
        self.oracle
            .as_ref()
            .and_then(|h| self.harnesses.get(h))
            .and_then(|g| g.objective)
    }

    #[must_use]
    pub fn router_regret(&self) -> Option<f64> {
        self.router
            .objective
            .zip(self.oracle_objective())
            .map(|(j, best)| j - best)
    }

    #[must_use]
    pub fn fixed_regret(&self) -> Option<f64> {
        self.harnesses
            .get(FIXED_RULE)
            .and_then(|g| g.objective)
            .zip(self.oracle_objective())
            .map(|(j, best)| j - best)
    }
}

/// Recorded turns by judged class.
#[must_use]
pub fn classes(turns: &[Turn], params: &Params) -> Vec<ClassRow> {
    let names: BTreeSet<&str> = turns.iter().map(|turn| turn.class.as_str()).collect();
    names
        .into_iter()
        .map(|class| {
            let of = |turn: &&Turn| turn.class == class;
            let harnesses: BTreeMap<String, Group> = turns
                .iter()
                .filter(of)
                .map(|turn| turn.harness.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|harness| {
                    (
                        harness.to_owned(),
                        group(
                            turns.iter().filter(of).filter(|t| t.harness == harness),
                            params,
                        ),
                    )
                })
                .filter(|(_, group)| group.turns > 0)
                .collect();
            let oracle = harnesses
                .iter()
                .filter(|(_, g)| g.succeeded > 0)
                .filter_map(|(name, g)| Some((name, g.objective?)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(name, _)| name.clone());
            ClassRow {
                class: class.to_owned(),
                router: group(
                    turns.iter().filter(of).filter(|t| t.rule == "router"),
                    params,
                ),
                harnesses,
                oracle,
            }
        })
        .collect()
}

/// The promotion gate's verdict for one policy version.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    pub policy: String,
    pub held_out_router: Option<f64>,
    pub held_out_fixed: Option<f64>,
    /// Classes with enough router and fixed-rule turns to compare.
    pub real_classes: usize,
    pub real_router: Option<f64>,
    pub real_fixed: Option<f64>,
    pub promote: bool,
    pub reason: String,
}

#[must_use]
pub fn gate(policy: &str, tasks: &[TaskRow], classes: &[ClassRow]) -> Gate {
    let held_out_router = mean(tasks.iter().map(TaskRow::router_regret));
    let held_out_fixed = mean(tasks.iter().map(TaskRow::fixed_regret));
    let enough: Vec<&ClassRow> = classes
        .iter()
        .filter(|class| {
            class.router.turns >= MIN_TURNS
                && class
                    .harnesses
                    .get(FIXED_RULE)
                    .is_some_and(|g| g.turns >= MIN_TURNS)
        })
        .collect();
    let real_router = mean(enough.iter().map(|c| c.router_regret()));
    let real_fixed = mean(enough.iter().map(|c| c.fixed_regret()));
    let (promote, reason) = match (held_out_router, held_out_fixed) {
        (Some(router), Some(fixed)) if router < fixed - 1e-9 => {
            match (real_router, real_fixed) {
                (Some(r), Some(f)) if r > f + 1e-9 => (
                    false,
                    "held-out regret is lower than the fixed rule's, but recorded turns show the router worse"
                        .to_owned(),
                ),
                _ => (
                    true,
                    "held-out regret is lower than the fixed rule's".to_owned(),
                ),
            }
        }
        (Some(router), Some(fixed)) if (router - fixed).abs() <= 1e-9 => (
            false,
            "held-out regret equals the fixed rule's: the router picks what the fixed rule picks on every task"
                .to_owned(),
        ),
        (Some(_), Some(_)) => (
            false,
            "held-out regret is higher than the fixed rule's".to_owned(),
        ),
        _ => (false, "no held-out task has both picks measured".to_owned()),
    };
    Gate {
        policy: policy.to_owned(),
        held_out_router,
        held_out_fixed,
        real_classes: enough.len(),
        real_router,
        real_fixed,
        promote,
        reason,
    }
}

/// The whole report.
#[derive(Clone, Debug)]
pub struct Report {
    pub params: Params,
    pub picks: Picks,
    pub tasks: Vec<TaskRow>,
    pub records: Option<PathBuf>,
    pub turns: usize,
    pub skipped: usize,
    pub classes: Vec<ClassRow>,
    pub gate: Gate,
}

/// Builds the report from a matrix, the picks, and a records folder.
#[must_use]
pub fn report(matrix: &Matrix, picks: Picks, records: Option<&Path>) -> Report {
    let tasks = tasks(matrix, &picks);
    let (turns, skipped) = records.map(read_turns).unwrap_or_default();
    let classes = classes(&turns, &matrix.params);
    let gate = gate(&picks.policy, &tasks, &classes);
    Report {
        params: matrix.params,
        picks,
        tasks,
        records: records.map(Path::to_path_buf),
        turns: turns.len(),
        skipped,
        classes,
        gate,
    }
}

fn money(value: Option<f64>) -> String {
    value.map_or("—".to_owned(), |v| format!("${v:.4}"))
}

fn secs(value: Option<f64>) -> String {
    value.map_or("—".to_owned(), |v| format!("{v:.1} s"))
}

fn group_json(group: &Group) -> Value {
    json!({
        "turns": group.turns, "succeeded": group.succeeded, "checked": group.checked,
        "checks_passed": group.checks_passed, "redone": group.redone,
        "mean_cost_usd": group.cost, "mean_seconds": group.seconds, "objective_usd": group.objective,
    })
}

impl Report {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "params": self.params.to_json(),
            "terminal_bench": {
                "set": self.picks.set,
                "policy": self.picks.policy,
                "harness_arms": self.picks.arms,
                "fixed_rule": FIXED_RULE,
                "tasks": self.tasks.iter().map(|row| json!({
                    "task": row.task,
                    "router": row.router,
                    "step": row.step,
                    "oracle": row.oracle,
                    "oracle_objective_usd": row.oracle_objective,
                    "router_objective_usd": row.router_objective,
                    "router_regret_usd": row.router_regret(),
                    "fixed_objective_usd": row.fixed_objective,
                    "fixed_regret_usd": row.fixed_regret(),
                    "harnesses": row.harnesses.iter().map(|(name, cell)| (name.clone(), json!({
                        "arm": cell.arm, "passes": cell.passes, "trials": cell.trials,
                        "mean_cost_usd": cell.cost, "mean_agent_seconds": cell.seconds,
                        "objective_usd": cell.objective,
                    }))).collect::<serde_json::Map<_, _>>(),
                })).collect::<Vec<_>>(),
            },
            "recorded_turns": {
                "dir": self.records,
                "turns": self.turns,
                "skipped_without_chat": self.skipped,
                "classes": self.classes.iter().map(|class| json!({
                    "class": class.class,
                    "oracle": class.oracle,
                    "router": group_json(&class.router),
                    "router_regret_usd": class.router_regret(),
                    "fixed_regret_usd": class.fixed_regret(),
                    "harnesses": class.harnesses.iter().map(|(name, g)| (name.clone(), group_json(g))).collect::<serde_json::Map<_, _>>(),
                })).collect::<Vec<_>>(),
            },
            "gate": {
                "policy": self.gate.policy,
                "rule": format!("a policy becomes the default only when its mean held-out regret is below the fixed rule's and recorded turns with at least {MIN_TURNS} turns per side do not show it worse"),
                "held_out_router_regret_usd": self.gate.held_out_router,
                "held_out_fixed_regret_usd": self.gate.held_out_fixed,
                "real_classes_compared": self.gate.real_classes,
                "real_router_regret_usd": self.gate.real_router,
                "real_fixed_regret_usd": self.gate.real_fixed,
                "promote": self.gate.promote,
                "reason": self.gate.reason,
            },
        })
    }

    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "Coder routing · {} on {} · fixed rule: {FIXED_RULE} first",
                self.picks.policy, self.picks.set
            ),
            format!(
                "  J = cost + {} × seconds + {} × (1 − success); harnesses stand for: {}",
                self.params.usd_per_second,
                self.params.fail_usd,
                self.picks
                    .arms
                    .iter()
                    .filter(|(h, _)| CHAT_HARNESSES.contains(&h.as_str()))
                    .map(|(h, a)| format!("{h} = {a}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            String::new(),
            "Terminal-Bench development tasks (held out for the router)".to_owned(),
            format!(
                "  {:<24} {:<12} {:<12} {:>10} {:>10} {:>10} {:>10}",
                "task", "router", "oracle", "J router", "J fixed", "regret r", "regret f"
            ),
        ];
        let j = |v: Option<f64>| v.map_or("—".to_owned(), |v| format!("{v:.4}"));
        for row in &self.tasks {
            lines.push(format!(
                "  {:<24} {:<12} {:<12} {:>10} {:>10} {:>10} {:>10}",
                row.task,
                row.router,
                row.oracle.as_deref().unwrap_or("—"),
                j(row.router_objective),
                j(row.fixed_objective),
                j(row.router_regret()),
                j(row.fixed_regret()),
            ));
            for (name, cell) in &row.harnesses {
                lines.push(format!(
                    "      {name:<12} {} of {} passed · {} · {} · J {}",
                    cell.passes,
                    cell.trials,
                    money(cell.cost),
                    secs(cell.seconds),
                    j(cell.objective)
                ));
            }
        }
        lines.push(String::new());
        lines.push(format!(
            "Recorded turns · {} from {}{}",
            self.turns,
            self.records
                .as_ref()
                .map_or("no folder".to_owned(), |dir| dir.display().to_string()),
            if self.skipped > 0 {
                format!(" · {} test lines without a chat left out", self.skipped)
            } else {
                String::new()
            }
        ));
        for class in &self.classes {
            lines.push(format!(
                "  {} · oracle {} · router regret {} · fixed-rule regret {}",
                class.class,
                class.oracle.as_deref().unwrap_or("—"),
                j(class.router_regret()),
                j(class.fixed_regret()),
            ));
            let mut rows: Vec<(String, &Group)> = class
                .harnesses
                .iter()
                .map(|(name, g)| (name.clone(), g))
                .collect();
            rows.push(("(router)".to_owned(), &class.router));
            for (name, g) in rows {
                if g.turns == 0 {
                    continue;
                }
                lines.push(format!(
                    "      {name:<12} {} turns · {} succeeded · checks {} of {} · {} redone · {} · {} · J {}",
                    g.turns,
                    g.succeeded,
                    g.checks_passed,
                    g.checked,
                    g.redone,
                    money(g.cost),
                    secs(g.seconds),
                    j(g.objective)
                ));
            }
        }
        lines.push(String::new());
        lines.push(format!(
            "Promotion gate · {}: {} ({}). Held-out mean regret: router {}, fixed rule {}. Recorded classes with {MIN_TURNS}+ turns on both sides: {}.",
            self.gate.policy,
            if self.gate.promote { "promote" } else { "hold" },
            self.gate.reason,
            j(self.gate.held_out_router),
            j(self.gate.held_out_fixed),
            self.gate.real_classes,
        ));
        lines
    }
}

/// `gym coder routing …`.
///
/// # Errors
/// An unknown option, or a picks file that doesn't read.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        writeln!(
            out,
            "gym coder routing [--picks PATH] [--records DIR | --no-records] [--json]\n\nCoder's harness router against the oracle and the fixed rule: on the\nTerminal-Bench development tasks, and on the routing records of real turns\ngrouped by System One's judged class, with the promotion gate's verdict.\n\n  --picks PATH             the router's picks on the tasks (default\n                           bench/terminal-bench/profiles/route-v1-tasks.json)\n  --records DIR            routing records (default CODER_ROUTING_DIR or\n                           ~/.openagents/coder/routing)\n{}",
            crate::coder_matrix::SOURCE_HELP
        )
        .map_err(|error| error.to_string())?;
        return Ok(0);
    }
    let source = Source::parse(args)?;
    let mut picks_path = default_picks();
    let mut records = default_records();
    let mut rest = source.rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--picks" => picks_path = PathBuf::from(rest.next().ok_or("--picks needs a value")?),
            "--records" => {
                records = Some(PathBuf::from(
                    rest.next().ok_or("--records needs a folder")?,
                ));
            }
            "--no-records" => records = None,
            other => return Err(format!("unknown option {other}")),
        }
    }
    let (matrix, _) = source.matrix();
    let report = report(&matrix, load_picks(&picks_path)?, records.as_deref());
    if source.json {
        serde_json::to_writer_pretty(&mut *out, &report.to_json())
            .map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in report.lines() {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_bench::{Attempt, Records};

    fn attempt(task: &str, arm: &str, reward: f64, cost: f64, ms: u64) -> Attempt {
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.task = task.to_owned();
        attempt.arm = arm.to_owned();
        attempt.reward = Some(reward);
        attempt.cost_usd = Some(cost);
        attempt.phases_ms[2] = Some(ms);
        attempt
    }

    fn picks(router: &[(&str, &str)]) -> Picks {
        Picks {
            set: "route-v1".into(),
            policy: "route-policy-v1".into(),
            arms: [("claude-code", "cc"), ("coder-loop", "loop")]
                .into_iter()
                .map(|(h, a)| (h.to_owned(), a.to_owned()))
                .collect(),
            rows: router
                .iter()
                .map(|(task, harness)| ((*task).into(), (*harness).into(), "plan-work".into()))
                .collect(),
        }
    }

    /// Claude Code passes both tasks at a price; the loop passes the easy
    /// one cheaply and fails the hard one.
    fn matrix() -> Matrix {
        let mut attempts = Vec::new();
        for _ in 0..3 {
            attempts.push(attempt("easy", "cc", 1.0, 0.5, 60_000));
            attempts.push(attempt("easy", "loop", 1.0, 0.01, 60_000));
            attempts.push(attempt("hard", "cc", 1.0, 0.5, 60_000));
            attempts.push(attempt("hard", "loop", 0.0, 0.01, 60_000));
        }
        Matrix::from_records(
            &Records {
                attempts,
                ..Records::default()
            },
            Params::default(),
            None,
        )
    }

    #[test]
    fn a_router_that_sends_the_easy_task_to_the_loop_beats_the_fixed_rule() {
        let matrix = matrix();
        let rows = tasks(
            &matrix,
            &picks(&[("easy", "coder-loop"), ("hard", "claude-code")]),
        );
        assert_eq!(rows[0].oracle.as_deref(), Some("coder-loop"));
        assert_eq!(rows[1].oracle.as_deref(), Some("claude-code"));
        assert!(rows.iter().all(|row| row.router_regret() == Some(0.0)));
        assert!(rows[0].fixed_regret().unwrap() > 0.4);
        let verdict = gate("route-policy-v1", &rows, &[]);
        assert!(verdict.promote, "{verdict:?}");
    }

    #[test]
    fn a_router_that_always_picks_the_fixed_rules_harness_holds() {
        let matrix = matrix();
        let rows = tasks(
            &matrix,
            &picks(&[("easy", "claude-code"), ("hard", "claude-code")]),
        );
        let verdict = gate("route-policy-v1", &rows, &[]);
        assert!(!verdict.promote);
        assert!(verdict.reason.contains("equals"), "{}", verdict.reason);
    }

    fn write(dir: &Path, lines: &[Value]) {
        std::fs::create_dir_all(dir).unwrap();
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(dir.join("2026-10-12.jsonl"), text).unwrap();
    }

    fn chose(id: &str, chat: Option<&str>, harness: &str, rule: &str, kind: &str) -> Value {
        json!({"type": "chose", "id": id, "chat": chat, "kind": "primary", "harness": harness, "rule": rule,
            "inputs": {"probabilities": [
                {"question": "route-v1.kind", "answer": kind, "p": 0.9},
                {"question": "route-v1.kind", "answer": "research", "p": 0.1},
                {"question": "route-v1.size", "answer": "small", "p": 0.8},
            ], "router": {"policy": "route-policy-v1"}}})
    }

    fn ended(id: &str, ended: &str, cost: f64, ms: u64, checks: Option<bool>) -> Value {
        json!({"type": "ended", "id": id, "ended": ended, "cost_usd": cost, "elapsed_ms": ms, "checks_passed": checks})
    }

    #[test]
    fn recorded_turns_group_by_class_and_count_checks_and_redos() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            &[
                chose("a", Some("c1"), "coder-loop", "router", "answer"),
                ended("a", "done", 0.001, 4_000, None),
                chose("b", Some("c1"), "coder-loop", "router", "answer"),
                ended("b", "done", 0.001, 4_000, None),
                json!({"type": "redone", "id": "b", "why": "same-message"}),
                chose(
                    "c",
                    Some("c2"),
                    "claude-code",
                    "first-admitted",
                    "repo_change",
                ),
                ended("c", "done", 0.2, 60_000, Some(true)),
                chose("d", Some("c2"), "claude-code", "router", "repo_change"),
                ended("d", "error", 0.0, 1_000, Some(false)),
                chose("t", None, "coder-loop", "fallback", "answer"),
                ended("t", "error", 0.0, 4, None),
            ],
        );
        let (turns, skipped) = read_turns(dir.path());
        assert_eq!((turns.len(), skipped), (4, 1));
        let rows = classes(&turns, &Params::default());
        let answer = rows.iter().find(|row| row.class == "answer/small").unwrap();
        let loop_ = &answer.harnesses["coder-loop"];
        assert_eq!((loop_.turns, loop_.succeeded, loop_.redone), (2, 1, 1));
        let repo = rows
            .iter()
            .find(|row| row.class == "repo_change/small")
            .unwrap();
        let claude = &repo.harnesses["claude-code"];
        assert_eq!((claude.checked, claude.checks_passed), (2, 1));
        assert_eq!(repo.router.turns, 1);
        assert_eq!(repo.oracle.as_deref(), Some("claude-code"));
    }

    /// The checked-in picks read, and every task names a chat harness.
    #[test]
    fn the_checked_in_picks_read() {
        let picks = load_picks(&default_picks()).unwrap();
        assert_eq!(picks.policy, "route-policy-v1");
        assert_eq!(picks.rows.len(), 8);
        assert!(
            picks
                .rows
                .iter()
                .all(|(_, harness, _)| CHAT_HARNESSES.contains(&harness.as_str()))
        );
        for harness in CHAT_HARNESSES {
            assert!(picks.arms.contains_key(*harness));
        }
    }
}
