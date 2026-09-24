//! The outcome matrix: task by policy, from retained and local attempts.
//!
//! Each cell holds one task's graded trials of one policy: the passes over
//! the trials with a 95% Wilson interval, the mean cost, the mean agent
//! time, and a runtime objective that prices time and failure in dollars.
//! A cell is keyed by the policy manifest digest when its attempts recorded
//! one and by the arm name otherwise, so two arm names that ran one
//! configuration pool, and one arm whose configuration changed splits.
//!
//! Each task's Pareto frontier marks the cells no other cell beats on pass
//! rate, cost, and time at once. The oracle rows pick, per task, the
//! cheapest or the fastest cell that passed every one of at least
//! `min_trials` trials with known cost and time: an upper bound on routing
//! over the cells, chosen after seeing the outcomes, not a policy anyone can
//! run.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::terminal_bench::{Attempt, Records};
use crate::terminal_bench_reference::{self as reference, Reference};

/// The schema of `gym coder matrix --json`.
pub const SCHEMA: &str = "openagents.gym.coder-matrix.v1";

/// The two-sided 95% normal quantile.
const Z: f64 = 1.959_963_984_540_054;

/// How the runtime objective prices a cell, and what an oracle needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Dollars per second of agent time.
    pub usd_per_second: f64,
    /// Dollars charged per unit of failure probability: the cost of
    /// finishing a failed task another way.
    pub fail_usd: f64,
    /// The trials an oracle pick needs, every one passed.
    pub min_trials: usize,
}

impl Default for Params {
    /// One hour of agent time at $0.36, and a failure at $1.00: a failed
    /// task needs a second, stronger attempt, which costs about one
    /// standalone Opus run. Accounting choices, frozen here so every
    /// router is scored alike; change them with flags, not in code.
    fn default() -> Self {
        Self {
            usd_per_second: 0.0001,
            fail_usd: 1.0,
            min_trials: 3,
        }
    }
}

impl Params {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "usd_per_second": self.usd_per_second,
            "fail_usd": self.fail_usd,
            "min_trials": self.min_trials,
            "objective": "J = mean cost + usd_per_second × mean agent seconds + fail_usd × (1 − pass fraction)",
        })
    }
}

/// The 95% Wilson score interval for `passes` of `trials`.
#[must_use]
pub fn wilson(passes: usize, trials: usize) -> (f64, f64) {
    if trials == 0 {
        return (0.0, 1.0);
    }
    let n = trials as f64;
    let p = passes as f64 / n;
    let z2 = Z * Z;
    let centre = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let half = Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// One task's trials of one policy.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub task: String,
    /// `policy <digest>` or the arm name.
    pub key: String,
    /// The arm names of the cell's attempts.
    pub arms: Vec<String>,
    pub trials: usize,
    pub passes: usize,
    pub interval: (f64, f64),
    /// The mean over the trials, or `None` when any trial's is unknown.
    pub mean_cost: Option<f64>,
    pub mean_seconds: Option<f64>,
    pub objective: Option<f64>,
    pub frontier: bool,
}

impl Cell {
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.trials == 0 {
            0.0
        } else {
            self.passes as f64 / self.trials as f64
        }
    }

    /// A short label: the arm, or the arms that ran one policy.
    #[must_use]
    pub fn label(&self) -> String {
        if self.key.starts_with("policy ") {
            format!(
                "{} ({})",
                self.key.get(..19).unwrap_or(&self.key),
                self.arms.join(", ")
            )
        } else {
            self.key.clone()
        }
    }

    /// Whether an oracle may pick the cell.
    #[must_use]
    pub fn reliable(&self, params: &Params) -> bool {
        self.trials >= params.min_trials
            && self.passes == self.trials
            && self.mean_cost.is_some()
            && self.mean_seconds.is_some()
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "task": self.task,
            "key": self.key,
            "arms": self.arms,
            "trials": self.trials,
            "passes": self.passes,
            "wilson_95": [round(self.interval.0), round(self.interval.1)],
            "mean_cost_usd": self.mean_cost,
            "mean_agent_seconds": self.mean_seconds,
            "objective_usd": self.objective,
            "frontier": self.frontier,
        })
    }
}

fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// The key a cell groups an attempt under.
#[must_use]
pub fn key(attempt: &Attempt) -> String {
    attempt.policy.as_ref().map_or_else(
        || attempt.arm.clone(),
        |policy| format!("policy {}", policy.digest),
    )
}

/// Whether an attempt is a graded trial of an agent.
#[must_use]
pub fn graded(attempt: &Attempt) -> bool {
    !attempt.is_control() && attempt.reward.is_some()
}

/// The matrix.
#[derive(Clone, Debug, Default)]
pub struct Matrix {
    pub params: Params,
    pub tasks: Vec<String>,
    pub cells: Vec<Cell>,
}

/// One oracle's or one fixed policy's pick on every task.
#[derive(Clone, Debug, PartialEq)]
pub struct Portfolio {
    pub name: String,
    /// Per task: the cell picked, or `None` when nothing qualified.
    pub picks: Vec<(String, Option<Cell>)>,
}

impl Portfolio {
    #[must_use]
    pub fn passes(&self) -> (usize, usize) {
        self.picks
            .iter()
            .filter_map(|(_, cell)| cell.as_ref())
            .fold((0, 0), |(k, n), cell| (k + cell.passes, n + cell.trials))
    }

    /// Summed mean cost over the tasks, when every pick's is known.
    #[must_use]
    pub fn cost(&self) -> Option<f64> {
        self.picks
            .iter()
            .map(|(_, cell)| cell.as_ref().and_then(|cell| cell.mean_cost))
            .sum()
    }

    #[must_use]
    pub fn seconds(&self) -> Option<f64> {
        self.picks
            .iter()
            .map(|(_, cell)| cell.as_ref().and_then(|cell| cell.mean_seconds))
            .sum()
    }

    #[must_use]
    pub fn objective(&self) -> Option<f64> {
        self.picks
            .iter()
            .map(|(_, cell)| cell.as_ref().and_then(|cell| cell.objective))
            .sum()
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        let (passes, trials) = self.passes();
        json!({
            "name": self.name,
            "passes": passes,
            "trials": trials,
            "summed_mean_cost_usd": self.cost(),
            "summed_mean_agent_seconds": self.seconds(),
            "summed_objective_usd": self.objective(),
            "picks": self.picks.iter().map(|(task, cell)| json!({
                "task": task,
                "key": cell.as_ref().map(|c| c.key.clone()),
                "arms": cell.as_ref().map(|c| c.arms.clone()),
                "passes": cell.as_ref().map(|c| c.passes),
                "trials": cell.as_ref().map(|c| c.trials),
                "mean_cost_usd": cell.as_ref().and_then(|c| c.mean_cost),
                "mean_agent_seconds": cell.as_ref().and_then(|c| c.mean_seconds),
            })).collect::<Vec<_>>(),
        })
    }
}

impl Matrix {
    /// Builds the matrix from every graded attempt, restricted to `tasks`
    /// when given.
    #[must_use]
    pub fn from_records(
        records: &Records,
        params: Params,
        tasks: Option<&BTreeSet<String>>,
    ) -> Self {
        let mut grouped: BTreeMap<(String, String), Vec<&Attempt>> = BTreeMap::new();
        for attempt in records.attempts.iter().filter(|a| graded(a)) {
            let task = short_task(&attempt.task);
            if tasks.is_some_and(|tasks| !tasks.contains(&task)) {
                continue;
            }
            grouped
                .entry((task, key(attempt)))
                .or_default()
                .push(attempt);
        }
        let mut cells: Vec<Cell> = grouped
            .into_iter()
            .map(|((task, key), attempts)| {
                let trials = attempts.len();
                let passes = attempts
                    .iter()
                    .filter(|a| a.reward.is_some_and(|r| r >= 1.0))
                    .count();
                let mean = |values: Vec<Option<f64>>| {
                    let n = values.len() as f64;
                    values
                        .into_iter()
                        .sum::<Option<f64>>()
                        .map(|total| total / n)
                };
                let mean_cost = mean(attempts.iter().map(|a| a.cost_usd).collect());
                let mean_seconds = mean(
                    attempts
                        .iter()
                        .map(|a| a.phases_ms[2].map(|ms| ms as f64 / 1000.0))
                        .collect(),
                );
                let mut arms: Vec<String> = attempts.iter().map(|a| a.arm.clone()).collect();
                arms.sort();
                arms.dedup();
                let rate = passes as f64 / trials as f64;
                Cell {
                    task,
                    key,
                    arms,
                    trials,
                    passes,
                    interval: wilson(passes, trials),
                    mean_cost,
                    mean_seconds,
                    objective: mean_cost.zip(mean_seconds).map(|(cost, seconds)| {
                        cost + params.usd_per_second * seconds + params.fail_usd * (1.0 - rate)
                    }),
                    frontier: false,
                }
            })
            .collect();
        // Each task's frontier: not beaten on pass rate, cost, and time at
        // once. A cell with an unknown cost or time is never on it.
        let snapshot = cells.clone();
        for cell in &mut cells {
            let (Some(cost), Some(seconds)) = (cell.mean_cost, cell.mean_seconds) else {
                continue;
            };
            let rate = cell.pass_rate();
            cell.frontier = !snapshot.iter().any(|other| {
                other.task == cell.task
                    && other.key != cell.key
                    && other
                        .mean_cost
                        .zip(other.mean_seconds)
                        .is_some_and(|(c, s)| {
                            let r = other.pass_rate();
                            r >= rate
                                && c <= cost
                                && s <= seconds
                                && (r > rate || c < cost || s < seconds)
                        })
            });
        }
        let tasks: Vec<String> = cells
            .iter()
            .map(|cell| cell.task.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            params,
            tasks,
            cells,
        }
    }

    /// The cells of one task.
    pub fn task(&self, task: &str) -> impl Iterator<Item = &Cell> {
        self.cells.iter().filter(move |cell| cell.task == task)
    }

    /// The cell of `key` on `task`, when it ran.
    #[must_use]
    pub fn cell(&self, task: &str, key: &str) -> Option<&Cell> {
        self.cells
            .iter()
            .find(|cell| cell.task == task && cell.key == key)
    }

    /// The cheapest oracle: per task, the reliable cell with the lowest
    /// mean cost.
    #[must_use]
    pub fn cheapest(&self) -> Portfolio {
        self.oracle("oracle: cheapest reliable cell", |cell| cell.mean_cost)
    }

    /// The fastest oracle: per task, the reliable cell with the lowest
    /// mean agent time.
    #[must_use]
    pub fn fastest(&self) -> Portfolio {
        self.oracle("oracle: fastest reliable cell", |cell| cell.mean_seconds)
    }

    /// The objective oracle: per task, the cell with the lowest runtime
    /// objective, reliable or not.
    #[must_use]
    pub fn best_objective(&self) -> Portfolio {
        Portfolio {
            name: "oracle: lowest objective".to_owned(),
            picks: self
                .tasks
                .iter()
                .map(|task| {
                    (
                        task.clone(),
                        self.task(task)
                            .filter(|cell| cell.objective.is_some())
                            .min_by(|a, b| {
                                a.objective
                                    .unwrap_or(f64::MAX)
                                    .total_cmp(&b.objective.unwrap_or(f64::MAX))
                            })
                            .cloned(),
                    )
                })
                .collect(),
        }
    }

    fn oracle(&self, name: &str, measure: impl Fn(&Cell) -> Option<f64>) -> Portfolio {
        Portfolio {
            name: name.to_owned(),
            picks: self
                .tasks
                .iter()
                .map(|task| {
                    (
                        task.clone(),
                        self.task(task)
                            .filter(|cell| cell.reliable(&self.params))
                            .min_by(|a, b| {
                                measure(a)
                                    .unwrap_or(f64::MAX)
                                    .total_cmp(&measure(b).unwrap_or(f64::MAX))
                                    .then(a.key.cmp(&b.key))
                            })
                            .cloned(),
                    )
                })
                .collect(),
        }
    }

    /// One policy on every task.
    #[must_use]
    pub fn fixed(&self, key: &str) -> Portfolio {
        Portfolio {
            name: format!("fixed: {key}"),
            picks: self
                .tasks
                .iter()
                .map(|task| (task.clone(), self.cell(task, key).cloned()))
                .collect(),
        }
    }

    /// Keys that ran every task.
    #[must_use]
    pub fn complete_keys(&self) -> Vec<String> {
        let mut keys: BTreeMap<&str, usize> = BTreeMap::new();
        for cell in &self.cells {
            *keys.entry(cell.key.as_str()).or_default() += 1;
        }
        keys.into_iter()
            .filter(|(_, n)| *n == self.tasks.len())
            .map(|(key, _)| key.to_owned())
            .collect()
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "params": self.params.to_json(),
            "tasks": self.tasks,
            "cells": self.cells.iter().map(Cell::to_json).collect::<Vec<_>>(),
            "oracles": [self.cheapest().to_json(), self.fastest().to_json(), self.best_objective().to_json()],
            "complete_policies": self.complete_keys().iter().map(|key| self.fixed(key).to_json()).collect::<Vec<_>>(),
        })
    }

    /// The matrix as text lines: each task's cells, frontier first, then
    /// the oracle rows and every policy that ran all tasks.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.lines_with(None)
    }

    /// The matrix with a leaderboard's rows under each task: every task the
    /// reference covers is listed, including the ones no arm ran yet, and
    /// each shows the best-ranked `rows` rows of the reference.
    #[must_use]
    pub fn lines_with(&self, board: Option<(&Reference, usize)>) -> Vec<String> {
        let mut tasks: Vec<String> = self.tasks.clone();
        if let Some((board, _)) = board {
            tasks.extend(board.tasks());
            tasks.sort();
            tasks.dedup();
        }
        let mut lines = vec![
            format!(
                "Outcome matrix · {} tasks · {} cells · J = cost + ${}/s × agent time + ${} × (1 − pass)",
                self.tasks.len(),
                self.cells.len(),
                self.params.usd_per_second,
                self.params.fail_usd
            ),
            format!(
                "  {:<46} {:>6} {:>13} {:>10} {:>9} {:>9}",
                "task / policy", "pass", "Wilson 95%", "mean $", "mean s", "J $"
            ),
        ];
        for task in &tasks {
            lines.push(task.clone());
            let mut cells: Vec<&Cell> = self.task(task).collect();
            if cells.is_empty() {
                lines.push("    no graded attempts yet".to_owned());
            }
            cells.sort_by(|a, b| {
                b.frontier.cmp(&a.frontier).then(
                    a.objective
                        .unwrap_or(f64::MAX)
                        .total_cmp(&b.objective.unwrap_or(f64::MAX)),
                )
            });
            for cell in cells {
                lines.push(format!(
                    "  {} {:<44} {:>6} {:>13} {:>10} {:>9} {:>9}",
                    if cell.frontier { "◆" } else { " " },
                    clip(&cell.label(), 44),
                    format!("{}/{}", cell.passes, cell.trials),
                    format!("[{:.2}, {:.2}]", cell.interval.0, cell.interval.1),
                    cell.mean_cost
                        .map_or("unknown".to_owned(), |c| format!("${c:.4}")),
                    cell.mean_seconds
                        .map_or("—".to_owned(), |s| format!("{s:.1}")),
                    cell.objective.map_or("—".to_owned(), |j| format!("{j:.4}"))
                ));
            }
            if let Some((board, rows)) = board {
                for (entry, result) in board.task(task).into_iter().take(rows) {
                    lines.push(format!(
                        "  ▷ {:<44} {:>6} {:>13} {:>10} {:>9}",
                        clip(
                            &format!("leaderboard #{} {}", entry.rank.unwrap_or(0), entry.label()),
                            44
                        ),
                        format!("{}/{}", result.successes, result.trials),
                        {
                            let (low, high) =
                                wilson(result.successes as usize, result.trials as usize);
                            format!("[{low:.2}, {high:.2}]")
                        },
                        result.cost_usd.map_or("unknown".to_owned(), |c| format!(
                            "${:.4}",
                            c / result.trials.max(1) as f64
                        )),
                        result
                            .mean_agent_sec
                            .map_or("—".to_owned(), |s| format!("{s:.1}"))
                    ));
                }
            }
        }
        lines.push(String::new());
        lines.push(format!(
            "Portfolios over the {} tasks (sums of per-task means; oracles need {} of {} passes, known cost and time)",
            self.tasks.len(),
            self.params.min_trials,
            self.params.min_trials
        ));
        let mut portfolios = vec![self.cheapest(), self.fastest(), self.best_objective()];
        portfolios.extend(self.complete_keys().iter().map(|key| self.fixed(key)));
        for portfolio in &portfolios {
            lines.push(portfolio_line(portfolio));
        }
        lines.push("  ◆ on the task's Pareto frontier: no other cell has a pass rate as high, a cost as low, and a time as short.".to_owned());
        if let Some((board, rows)) = board {
            lines.push(format!(
                "  ▷ the public leaderboard's best {rows} rows per task ({}, fetched {}); mean $ is its cost per trial, from Harbor Hub job aggregates.",
                board.dataset_ref, board.fetched_at
            ));
            for entry in &board.entries {
                lines.push(format!("    {}", entry.summary_line()));
            }
        }
        lines
    }
}

/// One portfolio as a text row.
#[must_use]
pub fn portfolio_line(portfolio: &Portfolio) -> String {
    let (passes, trials) = portfolio.passes();
    let missing = portfolio
        .picks
        .iter()
        .filter(|(_, cell)| cell.is_none())
        .count();
    format!(
        "  {:<48} {:>7} {:>11} {:>10} {:>10}{}",
        clip(&portfolio.name, 48),
        format!("{passes}/{trials}"),
        portfolio
            .cost()
            .map_or("unknown".to_owned(), |c| format!("${c:.4}")),
        portfolio
            .seconds()
            .map_or("—".to_owned(), |s| format!("{s:.1} s")),
        portfolio
            .objective()
            .map_or("—".to_owned(), |j| format!("J {j:.3}")),
        if missing == 0 {
            String::new()
        } else {
            format!("  ({missing} tasks without a pick)")
        }
    )
}

fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Where attempts come from, which tasks count, and how cells are priced:
/// the flags `gym coder matrix` and `gym coder router` share.
pub struct Source {
    pub jobs: Option<std::path::PathBuf>,
    pub traces: Option<std::path::PathBuf>,
    /// `None` counts every task.
    pub tasks: Option<BTreeSet<String>>,
    /// Only attempts of this job profile. Without one, every profile but
    /// `tb4` counts: the Terminal-Bench 4.0 suite shares task names with
    /// the panel's pin at another commit, so its attempts never pool.
    pub profile: Option<String>,
    /// Leaderboard rows per task in the text view of a `tb4` matrix.
    pub reference_rows: usize,
    pub params: Params,
    pub json: bool,
    /// Arguments the shared parser didn't take.
    pub rest: Vec<String>,
}

/// The shared flags' help.
pub const SOURCE_HELP: &str = "\
  --tasks development|all|A,B  the tasks that count (default: the eight
                           development tasks, or every task with --profile)
  --profile ID             only attempts of this job profile; tb4 adds the
                           Terminal-Bench 4.0 leaderboard's rows per task
  --reference-rows N       leaderboard rows per task in text (default 5)
  --usd-per-second X       the objective's price of agent time (default 0.0001)
  --fail-usd X             the objective's price of a failure (default 1.0)
  --min-trials N           trials an oracle pick needs, all passed (default 3)
  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --no-jobs, --no-traces   leave out that source
  --json                   print versioned JSON instead of text";

impl Source {
    /// Parses the shared flags.
    ///
    /// # Errors
    ///
    /// Returns a message for a flag without its value or a bad number.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let repo =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
        let mut source = Self {
            jobs: std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".openagents/terminal-bench/jobs")),
            traces: Some(repo.join("traces")),
            tasks: Some(DEVELOPMENT.iter().map(|t| (*t).to_owned()).collect()),
            profile: None,
            reference_rows: 5,
            params: Params::default(),
            json: false,
            rest: Vec::new(),
        };
        let mut tasks_given = false;
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].as_str();
            let value = || {
                args.get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .cloned()
                    .ok_or_else(|| format!("{argument} needs a value"))
            };
            let number = |text: String| {
                text.parse::<f64>()
                    .map_err(|_| format!("{argument} needs a number"))
            };
            match argument {
                "--json" => source.json = true,
                "--no-jobs" => source.jobs = None,
                "--no-traces" => source.traces = None,
                "--jobs-dir" => {
                    source.jobs = Some(value()?.into());
                    index += 1;
                }
                "--traces-dir" => {
                    source.traces = Some(value()?.into());
                    index += 1;
                }
                "--profile" => {
                    source.profile = Some(value()?);
                    index += 1;
                }
                "--reference-rows" => {
                    source.reference_rows = value()?
                        .parse()
                        .map_err(|_| "--reference-rows needs a whole number".to_owned())?;
                    index += 1;
                }
                "--tasks" => {
                    tasks_given = true;
                    let tasks = value()?;
                    source.tasks = match tasks.as_str() {
                        "all" => None,
                        "development" => {
                            Some(DEVELOPMENT.iter().map(|t| (*t).to_owned()).collect())
                        }
                        list => Some(list.split(',').map(|t| t.trim().to_owned()).collect()),
                    };
                    index += 1;
                }
                "--usd-per-second" => {
                    source.params.usd_per_second = number(value()?)?;
                    index += 1;
                }
                "--fail-usd" => {
                    source.params.fail_usd = number(value()?)?;
                    index += 1;
                }
                "--min-trials" => {
                    source.params.min_trials = value()?
                        .parse()
                        .map_err(|_| "--min-trials needs a whole number".to_owned())?;
                    index += 1;
                }
                _ => source.rest.push(args[index].clone()),
            }
            index += 1;
        }
        if source.profile.is_some() && !tasks_given {
            source.tasks = None;
        }
        Ok(source)
    }

    /// Loads the attempts and builds the matrix.
    #[must_use]
    pub fn matrix(&self) -> (Matrix, Records) {
        let mut records = Records::load(self.jobs.as_deref(), self.traces.as_deref(), None);
        match &self.profile {
            Some(profile) => records.attempts.retain(|a| &a.profile == profile),
            None => records.attempts.retain(|a| a.profile != reference::PROFILE),
        }
        (
            Matrix::from_records(&records, self.params, self.tasks.as_ref()),
            records,
        )
    }
}

/// `gym coder matrix …`.
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
            "gym coder matrix [--json]\n\nTask by policy: passes over graded trials with a 95% Wilson interval, mean\ncost, mean agent time, and the runtime objective; each task's Pareto frontier;\nthe cheapest, fastest, and lowest-objective oracles; and every policy that ran\nall the tasks. A cell is keyed by the policy manifest digest when its attempts\nrecorded one, and by the arm otherwise.\n\n{SOURCE_HELP}"
        )
        .map_err(|error| error.to_string())?;
        return Ok(0);
    }
    let source = Source::parse(args)?;
    if let Some(other) = source.rest.first() {
        return Err(format!("unknown option {other}"));
    }
    let (matrix, _) = source.matrix();
    let board = (source.profile.as_deref() == Some(reference::PROFILE))
        .then(Reference::checked)
        .flatten();
    // The mini-task patterns are policies too, measured on another task set.
    let patterns = crate::coder_handoff::load(&crate::coder_handoff::default_path())
        .ok()
        .filter(|_| source.profile.is_none());
    if source.json {
        let mut value = matrix.to_json();
        if let Some(map) = value.as_object_mut() {
            map.insert("profile".to_owned(), json!(source.profile));
            if let Some(patterns) = &patterns {
                map.insert("minitask_patterns".to_owned(), patterns.to_json());
            }
            if let Some(board) = &board {
                let mut summary = board.summary_json();
                summary["tasks"] = board
                    .tasks()
                    .iter()
                    .map(|task| (task.clone(), board.task_json(task)))
                    .collect::<serde_json::Map<_, _>>()
                    .into();
                map.insert("reference".to_owned(), summary);
            }
        }
        serde_json::to_writer_pretty(&mut *out, &value).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in matrix.lines_with(board.as_ref().map(|b| (b, source.reference_rows))) {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
        if let Some(patterns) = &patterns {
            writeln!(out).map_err(|error| error.to_string())?;
            for line in patterns.lines() {
                writeln!(out, "{line}").map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(0)
}

/// A task name without its dataset prefix.
#[must_use]
pub fn short_task(task: &str) -> String {
    task.rsplit('/').next().unwrap_or(task).to_owned()
}

/// The eight development tasks every retained arm ran.
pub const DEVELOPMENT: &[&str] = &[
    "build-cython-ext",
    "cancel-async-tasks",
    "fix-code-vulnerability",
    "fix-git",
    "git-leak-recovery",
    "headless-terminal",
    "log-summary-date-ranges",
    "sqlite-db-truncate",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wilson_interval_matches_the_harness() {
        let (low, high) = wilson(3, 3);
        assert!((low - 0.4385).abs() < 1e-3 && (high - 1.0).abs() < 1e-9);
        let (low, high) = wilson(0, 3);
        assert!(low.abs() < 1e-9 && (high - 0.5615).abs() < 1e-3);
    }

    fn attempt(task: &str, arm: &str, reward: f64, cost: f64, ms: u64) -> Attempt {
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.task = task.to_owned();
        attempt.arm = arm.to_owned();
        attempt.reward = Some(reward);
        attempt.cost_usd = Some(cost);
        attempt.phases_ms[2] = Some(ms);
        attempt
    }

    #[test]
    fn oracles_pick_the_cheapest_and_fastest_reliable_cells() {
        let mut attempts = Vec::new();
        for _ in 0..3 {
            attempts.push(attempt("a", "luna", 1.0, 0.01, 60_000));
            attempts.push(attempt("a", "opus", 1.0, 0.20, 20_000));
            attempts.push(attempt("b", "opus", 1.0, 0.30, 30_000));
        }
        attempts.push(attempt("b", "luna", 1.0, 0.01, 50_000));
        attempts.push(attempt("b", "luna", 0.0, 0.01, 50_000));
        attempts.push(attempt("b", "luna", 1.0, 0.01, 50_000));
        let records = Records {
            attempts,
            ..Records::default()
        };
        let matrix = Matrix::from_records(&records, Params::default(), None);
        let cheapest = matrix.cheapest();
        assert_eq!(cheapest.picks[0].1.as_ref().unwrap().key, "luna");
        assert_eq!(cheapest.picks[1].1.as_ref().unwrap().key, "opus");
        assert_eq!(cheapest.passes(), (6, 6));
        assert!((cheapest.cost().unwrap() - 0.31).abs() < 1e-9);
        let fastest = matrix.fastest();
        assert!((fastest.seconds().unwrap() - 50.0).abs() < 1e-9);
        // Luna on b is cheaper but failed once; both stay on b's frontier.
        let frontier: Vec<&str> = matrix
            .task("b")
            .filter(|c| c.frontier)
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(frontier, ["luna", "opus"]);
        assert_eq!(matrix.fixed("luna").passes(), (5, 6));
        assert_eq!(matrix.complete_keys(), ["luna", "opus"]);
    }

    #[test]
    fn tb4_attempts_stay_under_their_profile_with_the_leaderboard_beside() {
        let mut panel = attempt("cad-model", "luna", 1.0, 0.01, 60_000);
        panel.profile = "panel".to_owned();
        let mut tb4 = attempt("cad-model", "luna", 0.0, 0.02, 90_000);
        tb4.profile = "tb4".to_owned();
        let source = |profile: Option<&str>| {
            let mut records = Records {
                attempts: vec![panel.clone(), tb4.clone()],
                ..Records::default()
            };
            match profile {
                Some(p) => records.attempts.retain(|a| a.profile == p),
                None => records.attempts.retain(|a| a.profile != reference::PROFILE),
            }
            Matrix::from_records(&records, Params::default(), None)
        };
        assert_eq!(source(None).cells[0].passes, 1);
        let matrix = source(Some("tb4"));
        assert_eq!((matrix.cells[0].passes, matrix.cells[0].trials), (0, 1));
        let board = Reference::from_json(&json!({
            "schema": crate::terminal_bench_reference::SCHEMA,
            "dataset_ref": "v4.0.0",
            "fetched_at": "t",
            "entries": [{
                "rank": 1, "agent": "Codex", "model": "GPT-6 Astra", "reasoning_effort": "max",
                "metrics": {"successes": 5, "n_trials": 10},
                "tasks": {
                    "cad-model": {"successes": 5, "trials": 5, "cost_usd": 10.0, "mean_agent_sec": 580.0},
                    "bun-sourcemap-leak": {"successes": 0, "trials": 5, "cost_usd": null, "mean_agent_sec": null}
                }
            }]
        }))
        .unwrap();
        let text = matrix.lines_with(Some((&board, 3))).join("\n");
        assert!(
            text.contains("bun-sourcemap-leak\n    no graded attempts yet"),
            "{text}"
        );
        assert!(
            text.contains("▷ leaderboard #1 Codex / GPT-6 Astra (max)"),
            "{text}"
        );
        assert!(text.contains("5/5"), "{text}");
        assert!(text.contains("$2.0000"), "{text}");
        let parsed = Source::parse(&["--profile".to_owned(), "tb4".to_owned()]).unwrap();
        assert!(parsed.tasks.is_none());
        assert_eq!(parsed.profile.as_deref(), Some("tb4"));
    }

    /// The oracle totals in `docs/optimization/coder-components.md`,
    /// recomputed from the retained trials.
    #[test]
    fn the_retained_trials_reproduce_the_documented_oracles() {
        let traces = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/terminal-bench/traces");
        let mut records = Records::load(None, Some(&traces), None);
        // The documented oracles cover the arms retained on 2026-09-22.
        // Later arms legitimately move the oracle, so the reproduction
        // pins the arm set it was documented over.
        const DOCUMENTED: [&str; 10] = [
            "claude-code-opus",
            "codex-gpt-6-luna",
            "coder-one-jevprobe-luna",
            "coder-one-jevprobe2-luna",
            "coder-one-jevprobe3-luna",
            "coder-one-jevprobe-opus-lean-low",
            "coder-one-jevprobe2-opus-lean-low",
            "coder-one-jevprobe2-opus-lean-low-5m",
            "coder-one-jevprobe3-opus-lean-low",
            "coder-one-jevprobe-opus-lean",
        ];
        records
            .attempts
            .retain(|attempt| DOCUMENTED.contains(&attempt.arm.as_str()));
        let tasks: BTreeSet<String> = DEVELOPMENT.iter().map(|t| (*t).to_owned()).collect();
        let matrix = Matrix::from_records(&records, Params::default(), Some(&tasks));
        let cheapest = matrix.cheapest();
        assert_eq!(cheapest.passes(), (24, 24));
        assert!(
            (cheapest.cost().unwrap() - 0.071_615_749).abs() < 1e-6,
            "{:?}",
            cheapest.cost()
        );
        assert!(
            (cheapest.seconds().unwrap() - 430.0735).abs() < 0.05,
            "{:?}",
            cheapest.seconds()
        );
        let fastest = matrix.fastest();
        assert!(
            (fastest.cost().unwrap() - 0.513_684_957).abs() < 1e-6,
            "{:?}",
            fastest.cost()
        );
        assert!(
            (fastest.seconds().unwrap() - 174.52).abs() < 0.05,
            "{:?}",
            fastest.seconds()
        );
        let pick = |task: &str| {
            cheapest
                .picks
                .iter()
                .find(|(t, _)| t == task)
                .and_then(|(_, c)| c.as_ref())
                .map(|c| c.key.clone())
        };
        assert_eq!(
            pick("cancel-async-tasks").as_deref(),
            Some("coder-one-jevprobe2-opus-lean-low-5m")
        );
        assert_eq!(
            pick("sqlite-db-truncate").as_deref(),
            Some("codex-gpt-6-luna")
        );
    }
}
