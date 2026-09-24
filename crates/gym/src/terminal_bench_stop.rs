//! The early-stopping rule for a targeted Terminal-Bench experiment.
//!
//! A targeted experiment runs every planned attempt unless something stops
//! it, and the finished trials often settle the question long before the
//! last one. This rule reads the attempts graded so far and the attempts
//! still open, and says which arms need no more trials and whether the
//! experiment is over. It is code only: no model call decides a stop.
//!
//! The rule compares every candidate arm against the baseline, the first
//! arm that isn't a control (`nop` or `oracle`). A control arm never enters
//! a comparison and never stops by this rule. An attempt pair is one task
//! and one attempt number in both arms, as in the report's exact McNemar
//! test ([`crate::terminal_bench_experiment`]).
//!
//! A candidate arm stops when one of these holds, checked in this order:
//!
//! 1. **Dominated.** Even if every open attempt of the arm passes, it ends
//!    with fewer passes than another arm has now, and its mean cost per
//!    graded attempt isn't lower than that arm's.
//! 2. **Decided.** The exact McNemar test against the baseline is below the
//!    significance level, and stays below it with the same winner even if
//!    every open pair goes the other way.
//! 3. **Undecidable.** Even if every open pair went one way, the exact
//!    McNemar test couldn't get below the significance level. A design too
//!    small to get below it even if every planned pair went one way, such
//!    as one task with three attempts, is exploratory: this test never
//!    stops it.
//! 4. **Below the acceptance bar.** Even if every open attempt passes, the
//!    arm's pass rate stays under the bar the experiment set. Without a
//!    bar, this never fires.
//!
//! The experiment ends when every candidate arm has stopped. A stopped arm
//! stays stopped: its open attempts become attempts that never run. The
//! baseline stops only when the experiment ends. The rule makes no
//! correction for comparing several arms, the same as the report.
//!
//! `tbench experiment run --stop-early` applies the same rule after every
//! graded trial (`bench/terminal-bench/tbench/stop_rule.py`), and both
//! implementations are tested against one set of cases,
//! `bench/terminal-bench/tests/fixtures/stop-rule/cases.json`.
//! [`replay`] runs a finished experiment's trials back through the rule in
//! the order they finished and says where it would have stopped.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::terminal_bench_experiment::binomial_two_sided;

/// The schema of the rule's JSON.
pub const SCHEMA: &str = "openagents.gym.experiment-stop.v1";

/// Arms that are controls, not treatments: they never enter a comparison.
pub const CONTROLS: [&str; 2] = ["nop", "oracle"];

/// The two-sided significance level, unless the experiment sets another.
pub const DEFAULT_ALPHA: f64 = 0.05;

/// Whether `arm` is a control arm.
#[must_use]
pub fn is_control(arm: &str) -> bool {
    CONTROLS.contains(&arm)
}

/// One attempt, as the rule sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Graded, and the verifier passed it.
    Pass,
    /// Graded, and the verifier failed it.
    Fail,
    /// Not graded yet: pending, running, or set aside to run again.
    Open,
    /// Never graded: refused, given up on, skipped, finished without a
    /// reward, or stopped.
    Dead,
}

impl Cell {
    /// The cell's word in the shared cases file.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Cell::Pass => "pass",
            Cell::Fail => "fail",
            Cell::Open => "open",
            Cell::Dead => "dead",
        }
    }

    /// Reads a cell's word.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "pass" => Some(Cell::Pass),
            "fail" => Some(Cell::Fail),
            "open" => Some(Cell::Open),
            "dead" => Some(Cell::Dead),
            _ => None,
        }
    }

    /// A trial's cell from its status row: its state and its reward.
    #[must_use]
    pub fn of(state: &str, reward: Option<f64>) -> Self {
        match (state, reward) {
            ("finished", Some(reward)) if reward >= 1.0 => Cell::Pass,
            ("finished", Some(_)) => Cell::Fail,
            ("pending" | "running", _) => Cell::Open,
            _ => Cell::Dead,
        }
    }
}

/// Where an arm stands under the rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmState {
    /// The arm every candidate is compared against.
    Baseline,
    /// A `nop` or `oracle` arm, outside the rule.
    Control,
    /// A candidate that still needs trials.
    Active,
    Dominated,
    Decided,
    Undecidable,
    BelowBar,
}

impl ArmState {
    /// The state's word, as the ledger and the JSON carry it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            ArmState::Baseline => "baseline",
            ArmState::Control => "control",
            ArmState::Active => "active",
            ArmState::Dominated => "dominated",
            ArmState::Decided => "decided",
            ArmState::Undecidable => "undecidable",
            ArmState::BelowBar => "below_bar",
        }
    }

    /// Reads a state's word.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        [
            ArmState::Baseline,
            ArmState::Control,
            ArmState::Active,
            ArmState::Dominated,
            ArmState::Decided,
            ArmState::Undecidable,
            ArmState::BelowBar,
        ]
        .into_iter()
        .find(|state| state.word() == word)
    }

    /// Whether the arm needs no more trials.
    #[must_use]
    pub fn stopped(self) -> bool {
        matches!(
            self,
            ArmState::Dominated | ArmState::Decided | ArmState::Undecidable | ArmState::BelowBar
        )
    }
}

/// The rule's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    /// The two-sided significance level of the exact McNemar test.
    pub alpha: f64,
    /// The pass rate a candidate must be able to reach, when one is set.
    pub accept_pass_rate: Option<f64>,
}

impl Default for Rule {
    fn default() -> Self {
        Rule {
            alpha: DEFAULT_ALPHA,
            accept_pass_rate: None,
        }
    }
}

/// What the rule reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Input {
    pub arms: Vec<String>,
    pub tasks: Vec<String>,
    pub attempts: u64,
    /// Each attempt's cell by arm, task, and attempt number; a missing one
    /// is open.
    pub cells: BTreeMap<(String, String, u64), Cell>,
    /// Each arm's mean cost per graded attempt, when known.
    pub mean_cost: BTreeMap<String, f64>,
    /// Arms an earlier evaluation stopped, with the state and why.
    pub stopped: BTreeMap<String, (ArmState, String)>,
}

impl Input {
    /// One attempt's cell. An open attempt of a stopped arm never runs.
    #[must_use]
    pub fn cell(&self, arm: &str, task: &str, attempt: u64) -> Cell {
        let cell = self
            .cells
            .get(&(arm.to_owned(), task.to_owned(), attempt))
            .copied()
            .unwrap_or(Cell::Open);
        if cell == Cell::Open && self.stopped.contains_key(arm) {
            Cell::Dead
        } else {
            cell
        }
    }

    fn slots(&self) -> impl Iterator<Item = (&String, u64)> + '_ {
        self.tasks
            .iter()
            .flat_map(|task| (1..=self.attempts).map(move |attempt| (task, attempt)))
    }

    /// `(passes, graded, open)` for one arm.
    #[must_use]
    pub fn counts(&self, arm: &str) -> (usize, usize, usize) {
        let (mut passes, mut graded, mut open) = (0, 0, 0);
        for (task, attempt) in self.slots() {
            match self.cell(arm, task, attempt) {
                Cell::Pass => {
                    passes += 1;
                    graded += 1;
                }
                Cell::Fail => graded += 1,
                Cell::Open => open += 1,
                Cell::Dead => {}
            }
        }
        (passes, graded, open)
    }

    /// Reads the shared cases file's input shape.
    ///
    /// # Errors
    /// When a cell's word isn't known.
    pub fn from_json(value: &Value) -> Result<(Self, Rule), String> {
        let strings = |key: &str| -> Vec<String> {
            value[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        };
        let mut input = Input {
            arms: strings("arms"),
            tasks: strings("tasks"),
            attempts: value["attempts"].as_u64().unwrap_or(0),
            ..Input::default()
        };
        for row in value["cells"].as_array().into_iter().flatten() {
            let word = row["cell"].as_str().unwrap_or_default();
            let cell = Cell::parse(word).ok_or_else(|| format!("unknown cell {word}"))?;
            input.cells.insert(
                (
                    row["arm"].as_str().unwrap_or_default().to_owned(),
                    row["task"].as_str().unwrap_or_default().to_owned(),
                    row["attempt"].as_u64().unwrap_or(0),
                ),
                cell,
            );
        }
        for (arm, cost) in value["mean_cost"].as_object().into_iter().flatten() {
            if let Some(cost) = cost.as_f64() {
                input.mean_cost.insert(arm.clone(), cost);
            }
        }
        for (arm, state) in value["stopped"].as_object().into_iter().flatten() {
            let word = state.as_str().unwrap_or_default();
            let state = ArmState::parse(word).ok_or_else(|| format!("unknown state {word}"))?;
            input
                .stopped
                .insert(arm.clone(), (state, "stopped earlier".to_owned()));
        }
        let rule = Rule {
            alpha: value["alpha"].as_f64().unwrap_or(DEFAULT_ALPHA),
            accept_pass_rate: value["accept_pass_rate"].as_f64(),
        };
        Ok((input, rule))
    }
}

/// One arm under the rule.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmVerdict {
    pub arm: String,
    pub state: ArmState,
    /// Why the arm stopped, or where it stands.
    pub reason: String,
    pub passes: usize,
    pub graded: usize,
    pub open: usize,
    pub mean_cost: Option<f64>,
}

/// A candidate against the baseline, paired by task and attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub arm: String,
    pub baseline: String,
    /// Pairs graded in both arms.
    pub pairs: usize,
    pub arm_only: usize,
    pub baseline_only: usize,
    /// The exact McNemar p-value of the graded pairs now.
    pub p_now: f64,
    /// The p-value if every open pair went to the arm.
    pub p_arm_best: f64,
    /// The p-value if every open pair went to the baseline.
    pub p_baseline_best: f64,
    /// `open`, `separated`, `cannot separate`, or `exploratory` when even
    /// every planned pair going one way couldn't separate the arms.
    pub state: &'static str,
    /// When separated, the arm that wins.
    pub winner: Option<String>,
}

/// What the rule says.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub rule: Rule,
    pub baseline: Option<String>,
    pub arms: Vec<ArmVerdict>,
    pub comparisons: Vec<Comparison>,
    /// Whether the experiment needs no more trials.
    pub ended: bool,
    /// When ended: `decided`, `dominated`, `undecidable`, or `below_bar`.
    pub verdict: Option<&'static str>,
    pub reason: String,
}

impl Verdict {
    /// The arms that stopped, with why.
    pub fn stopped(&self) -> impl Iterator<Item = &ArmVerdict> {
        self.arms.iter().filter(|arm| arm.state.stopped())
    }

    /// The verdict as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "alpha": self.rule.alpha,
            "accept_pass_rate": self.rule.accept_pass_rate,
            "baseline": self.baseline,
            "ended": self.ended,
            "verdict": self.verdict,
            "reason": self.reason,
            "arms": self.arms.iter().map(|arm| json!({
                "arm": arm.arm,
                "state": arm.state.word(),
                "reason": arm.reason,
                "passes": arm.passes,
                "graded": arm.graded,
                "open": arm.open,
                "mean_cost_usd": arm.mean_cost,
            })).collect::<Vec<_>>(),
            "comparisons": self.comparisons.iter().map(|c| json!({
                "arm": c.arm,
                "baseline": c.baseline,
                "pairs": c.pairs,
                "arm_only": c.arm_only,
                "baseline_only": c.baseline_only,
                "mcnemar_exact_p": c.p_now,
                "p_if_open_pairs_go_to_arm": c.p_arm_best,
                "p_if_open_pairs_go_to_baseline": c.p_baseline_best,
                "state": c.state,
                "winner": c.winner,
            })).collect::<Vec<_>>(),
        })
    }

    /// The verdict as terminal text.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Stopping rule (alpha {}{}): {}",
            self.rule.alpha,
            self.rule
                .accept_pass_rate
                .map(|bar| format!(", acceptance bar {:.0}%", 100.0 * bar))
                .unwrap_or_default(),
            if self.ended {
                format!(
                    "ENDED, {}: {}",
                    self.verdict.unwrap_or("ended").replace('_', " "),
                    self.reason
                )
            } else {
                format!("continue: {}", self.reason)
            }
        )];
        for arm in &self.arms {
            lines.push(format!(
                "  {:<32} {:<11} {}",
                arm.arm,
                arm.state.word().replace('_', " "),
                arm.reason
            ));
        }
        for c in &self.comparisons {
            lines.push(format!(
                "  {} vs {}: {} pairs, only {} {}, only {} {}; McNemar p = {} now, {} at best for {}, {} at best for {}: {}",
                c.arm,
                c.baseline,
                c.pairs,
                c.arm,
                c.arm_only,
                c.baseline,
                c.baseline_only,
                p_text(c.p_now),
                p_text(c.p_arm_best),
                c.arm,
                p_text(c.p_baseline_best),
                c.baseline,
                c.state
            ));
        }
        lines
    }
}

fn p_text(p: f64) -> String {
    if p >= 0.9995 {
        "1".to_owned()
    } else if p < 0.001 {
        "< 0.001".to_owned()
    } else {
        format!("{p:.3}")
    }
}

fn money(value: Option<f64>) -> String {
    value.map_or_else(|| "unknown".to_owned(), |usd| format!("${usd:.2}"))
}

/// The discordant pair counts `(arm_only, baseline_only)` when every open
/// cell of the arm takes `arm_open` and of the baseline `base_open`.
fn discordant(
    input: &Input,
    arm: &str,
    baseline: &str,
    fill: Option<(bool, bool)>,
) -> (usize, usize, usize) {
    let (mut arm_only, mut baseline_only, mut pairs) = (0, 0, 0);
    for (task, attempt) in input.slots() {
        let a = input.cell(arm, task, attempt);
        let b = input.cell(baseline, task, attempt);
        if a == Cell::Dead || b == Cell::Dead {
            continue;
        }
        let resolve = |cell: Cell, open: Option<bool>| match cell {
            Cell::Pass => Some(true),
            Cell::Fail => Some(false),
            _ => open,
        };
        let (Some(a), Some(b)) = (resolve(a, fill.map(|f| f.0)), resolve(b, fill.map(|f| f.1)))
        else {
            continue;
        };
        pairs += 1;
        match (a, b) {
            (true, false) => arm_only += 1,
            (false, true) => baseline_only += 1,
            _ => {}
        }
    }
    (arm_only, baseline_only, pairs)
}

fn compare(input: &Input, arm: &str, baseline: &str, alpha: f64) -> Comparison {
    let (arm_only, baseline_only, pairs) = discordant(input, arm, baseline, None);
    // Every open pair to the arm, then every open pair to the baseline.
    let (a1, b1, _) = discordant(input, arm, baseline, Some((true, false)));
    let (a2, b2, _) = discordant(input, arm, baseline, Some((false, true)));
    let p_arm_best = if a1 > b1 {
        binomial_two_sided(b1, a1 + b1)
    } else {
        1.0
    };
    let p_baseline_best = if b2 > a2 {
        binomial_two_sided(a2, a2 + b2)
    } else {
        1.0
    };
    // The arm wins even when every open pair goes to the baseline, or the
    // baseline wins even when every open pair goes to the arm.
    let arm_locked = a2 > b2 && binomial_two_sided(b2, a2 + b2) < alpha;
    let baseline_locked = b1 > a1 && binomial_two_sided(a1, a1 + b1) < alpha;
    let slots = input.tasks.len() * usize::try_from(input.attempts).unwrap_or(usize::MAX);
    let (state, winner) = if binomial_two_sided(0, slots) >= alpha {
        // Even every planned pair going one way couldn't separate the arms:
        // an exploratory design, which this test never stops.
        ("exploratory", None)
    } else if arm_locked {
        ("separated", Some(arm.to_owned()))
    } else if baseline_locked {
        ("separated", Some(baseline.to_owned()))
    } else if p_arm_best >= alpha && p_baseline_best >= alpha {
        ("cannot separate", None)
    } else {
        ("open", None)
    };
    Comparison {
        arm: arm.to_owned(),
        baseline: baseline.to_owned(),
        pairs,
        arm_only,
        baseline_only,
        p_now: binomial_two_sided(arm_only, arm_only + baseline_only),
        p_arm_best,
        p_baseline_best,
        state,
        winner,
    }
}

/// Applies the rule.
#[must_use]
pub fn evaluate(input: &Input, rule: Rule) -> Verdict {
    let real: Vec<&String> = input.arms.iter().filter(|arm| !is_control(arm)).collect();
    let baseline = (real.len() >= 2).then(|| real[0].clone());
    let candidates: Vec<&String> = real
        .iter()
        .copied()
        .filter(|arm| Some(*arm) != baseline.as_ref())
        .collect();
    let counts: BTreeMap<&String, (usize, usize, usize)> = input
        .arms
        .iter()
        .map(|arm| (arm, input.counts(arm)))
        .collect();
    let mut comparisons = Vec::new();
    let mut arms = Vec::new();
    for arm in &input.arms {
        let (passes, graded, open) = counts[arm];
        let mean_cost = input.mean_cost.get(arm).copied();
        let verdict = |state: ArmState, reason: String| ArmVerdict {
            arm: arm.clone(),
            state,
            reason,
            passes,
            graded,
            open,
            mean_cost,
        };
        if is_control(arm) {
            arms.push(verdict(
                ArmState::Control,
                "a control arm: outside the rule".to_owned(),
            ));
            continue;
        }
        if Some(arm) == baseline.as_ref() {
            arms.push(verdict(
                ArmState::Baseline,
                format!("{passes} of {graded} graded, {open} open"),
            ));
            continue;
        }
        let comparison = baseline
            .as_ref()
            .map(|baseline| compare(input, arm, baseline, rule.alpha));
        if let Some((state, reason)) = input.stopped.get(arm) {
            arms.push(verdict(*state, reason.clone()));
            comparisons.extend(comparison);
            continue;
        }
        let best = passes + open;
        let dominator = real
            .iter()
            .copied()
            .filter(|other| *other != arm)
            .find(|other| {
                let (other_passes, _, _) = counts[other];
                // Not cheaper: its mean cost is at least the other arm's.
                let not_cheaper = match (mean_cost, input.mean_cost.get(*other)) {
                    (Some(mine), Some(theirs)) => mine >= *theirs,
                    _ => false,
                };
                best < other_passes && not_cheaper
            });
        let (state, reason) = if let Some(other) = dominator {
            (
                ArmState::Dominated,
                format!(
                    "at most {best} passes even if every open attempt passes, fewer than {other}'s {}, at {} per graded attempt against {}",
                    counts[other].0,
                    money(mean_cost),
                    money(input.mean_cost.get(other).copied())
                ),
            )
        } else if let Some(c) = comparison.as_ref().filter(|c| c.state == "separated") {
            (
                ArmState::Decided,
                format!(
                    "{} wins: only {} passed {} pairs, only {} passed {}; exact McNemar stays below {} whichever way the open pairs go",
                    c.winner.as_deref().unwrap_or_default(),
                    c.arm,
                    c.arm_only,
                    c.baseline,
                    c.baseline_only,
                    rule.alpha
                ),
            )
        } else if let Some(c) = comparison.as_ref().filter(|c| c.state == "cannot separate") {
            (
                ArmState::Undecidable,
                format!(
                    "even if every open pair went one way, exact McNemar against {} gets no lower than {:.3}, not below {}",
                    c.baseline,
                    c.p_arm_best.min(c.p_baseline_best),
                    rule.alpha
                ),
            )
        } else if let Some(bar) = rule
            .accept_pass_rate
            .filter(|bar| graded + open > 0 && (best as f64) < bar * (graded + open) as f64)
        {
            (
                ArmState::BelowBar,
                format!(
                    "at most {best} of {} ({:.0}%) even if every open attempt passes, under the {:.0}% acceptance bar",
                    graded + open,
                    100.0 * best as f64 / (graded + open) as f64,
                    100.0 * bar
                ),
            )
        } else {
            (
                ArmState::Active,
                format!("{passes} of {graded} graded, {open} open"),
            )
        };
        arms.push(verdict(state, reason));
        comparisons.extend(comparison);
    }
    let candidate_states: Vec<ArmState> = arms
        .iter()
        .filter(|verdict| candidates.contains(&&verdict.arm))
        .map(|verdict| verdict.state)
        .collect();
    let ended = !candidate_states.is_empty() && candidate_states.iter().all(|s| s.stopped());
    let verdict = ended.then(|| {
        [
            (ArmState::Decided, "decided"),
            (ArmState::Dominated, "dominated"),
            (ArmState::Undecidable, "undecidable"),
            (ArmState::BelowBar, "below_bar"),
        ]
        .into_iter()
        .find(|(state, _)| candidate_states.contains(state))
        .map_or("ended", |(_, word)| word)
    });
    let reason = if candidates.is_empty() {
        "no candidate arm: every arm is a control".to_owned()
    } else if ended {
        arms.iter()
            .filter(|verdict| candidates.contains(&&verdict.arm))
            .map(|verdict| format!("{} {}", verdict.arm, verdict.state.word().replace('_', " ")))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        let active: Vec<&str> = arms
            .iter()
            .filter(|verdict| verdict.state == ArmState::Active)
            .map(|verdict| verdict.arm.as_str())
            .collect();
        format!("{} still open", active.join(", "))
    };
    Verdict {
        rule,
        baseline,
        arms,
        comparisons,
        ended,
        verdict,
        reason,
    }
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

/// One trial of a recorded experiment, for a replay.
#[derive(Clone, Debug, PartialEq)]
pub struct Recorded {
    pub job: String,
    pub arm: String,
    pub task: String,
    pub attempt: u64,
    pub state: String,
    pub reward: Option<f64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// The trial's whole cost: the harness's attempt record, or the Claude
    /// quota it drew when no record priced it.
    pub cost_usd: Option<f64>,
}

impl Recorded {
    fn graded(&self) -> bool {
        self.state == "finished" && self.reward.is_some()
    }
}

/// A stop the replay found.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayStop {
    /// The graded trials the rule had read when it stopped, counting the
    /// one that triggered it.
    pub after_graded: usize,
    pub at: String,
    /// The trial whose grade triggered the stop.
    pub trigger: String,
    /// The arm stopped, or `None` when the experiment ended.
    pub arm: Option<String>,
    pub state: String,
    pub reason: String,
}

/// What a replay found.
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    pub rule: Rule,
    pub stops: Vec<ReplayStop>,
    /// Graded trials in the record.
    pub graded: usize,
    /// Graded trials the rule read: those that would have run.
    pub read: usize,
    /// Trials that ran to the end but would not have started:
    /// `(job, cost)`.
    pub saved: Vec<(String, Option<f64>)>,
    /// Trials that never finished anyway and the rule would also have
    /// skipped.
    pub skipped_anyway: usize,
    /// The verdict at the point the replay stopped reading.
    pub last: Verdict,
}

impl Replay {
    /// The known cost of the trials saved, and how many had no cost.
    #[must_use]
    pub fn saved_usd(&self) -> (f64, usize) {
        let known = self
            .saved
            .iter()
            .filter_map(|(_, usd)| *usd)
            .fold(0.0, |sum, usd| sum + usd);
        let unknown = self.saved.iter().filter(|(_, usd)| usd.is_none()).count();
        (known, unknown)
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        let (usd, unknown) = self.saved_usd();
        json!({
            "schema": "openagents.gym.experiment-stop-replay.v1",
            "alpha": self.rule.alpha,
            "accept_pass_rate": self.rule.accept_pass_rate,
            "graded": self.graded,
            "read": self.read,
            "stops": self.stops.iter().map(|s| json!({
                "after_graded": s.after_graded,
                "at": s.at,
                "trigger": s.trigger,
                "scope": if s.arm.is_some() { "arm" } else { "experiment" },
                "arm": s.arm,
                "state": s.state,
                "reason": s.reason,
            })).collect::<Vec<_>>(),
            "saved_trials": self.saved.len(),
            "saved_usd": usd,
            "saved_unpriced": unknown,
            "saved_jobs": self.saved.iter().map(|(job, usd)| json!({"job": job, "cost_usd": usd})).collect::<Vec<_>>(),
            "skipped_anyway": self.skipped_anyway,
            "last": self.last.to_json(),
        })
    }

    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let (usd, unknown) = self.saved_usd();
        let mut lines = vec![format!(
            "Replay of {} graded trials, in the order they finished (alpha {}{}):",
            self.graded,
            self.rule.alpha,
            self.rule
                .accept_pass_rate
                .map(|bar| format!(", acceptance bar {:.0}%", 100.0 * bar))
                .unwrap_or_default()
        )];
        if self.stops.is_empty() {
            lines.push("  The rule never stopped an arm; every trial would have run.".to_owned());
        }
        for stop in &self.stops {
            lines.push(format!(
                "  after graded trial {} ({}, {}): {} {}: {}",
                stop.after_graded,
                stop.at,
                stop.trigger,
                stop.arm
                    .as_deref()
                    .map_or_else(|| "the experiment".to_owned(), |arm| format!("stop {arm}")),
                stop.state.replace('_', " "),
                stop.reason
            ));
        }
        lines.push(format!(
            "  would not have started: {} trials that ran, ${usd:.2}{}; {} more that never finished would also have been skipped",
            self.saved.len(),
            if unknown > 0 {
                format!(" plus {unknown} unpriced")
            } else {
                String::new()
            },
            self.skipped_anyway
        ));
        if !self.saved.is_empty() {
            lines.push(format!(
                "  saved: {}",
                self.saved
                    .iter()
                    .map(|(job, usd)| format!(
                        "{} ({})",
                        job,
                        usd.map_or_else(|| "unpriced".to_owned(), |usd| format!("${usd:.2}"))
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        lines.push(format!(
            "  the rule read {} of {} graded trials{}",
            self.read,
            self.graded,
            if self.last.ended {
                " before the experiment ended"
            } else {
                "; the experiment never ended early"
            }
        ));
        lines
    }
}

/// Runs a recorded experiment back through the rule: the graded trials in
/// the order they finished, each read only when it would have run. A trial
/// of a stopped arm, or of an ended experiment, that started after the stop
/// would not have run; one already running at the stop finishes and counts.
#[must_use]
pub fn replay(
    arms: &[String],
    tasks: &[String],
    attempts: u64,
    trials: &[Recorded],
    rule: Rule,
) -> Replay {
    // Times as milliseconds: the status file and the attempt records write
    // them in different forms.
    let ms = |text: &Option<String>| {
        text.as_deref()
            .and_then(crate::terminal_bench::timestamp_ms)
    };
    let mut graded: Vec<&Recorded> = trials.iter().filter(|t| t.graded()).collect();
    graded.sort_by(|a, b| {
        ms(&a.finished_at)
            .cmp(&ms(&b.finished_at))
            .then(a.job.cmp(&b.job))
    });
    let mut input = Input {
        arms: arms.to_vec(),
        tasks: tasks.to_vec(),
        attempts,
        ..Input::default()
    };
    // When each arm stopped, and when the experiment ended.
    let mut stopped_at: BTreeMap<String, i64> = BTreeMap::new();
    let mut ended_at: Option<i64> = None;
    let mut stops = Vec::new();
    let mut read = 0;
    let mut costs: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    let mut observed: BTreeMap<String, usize> = BTreeMap::new();
    let mut last = evaluate(&input, rule);
    let would_run =
        |trial: &Recorded, stopped_at: &BTreeMap<String, i64>, ended_at: Option<i64>| {
            let cutoff = [stopped_at.get(&trial.arm).copied(), ended_at]
                .into_iter()
                .flatten()
                .min();
            match (cutoff, ms(&trial.started_at)) {
                (None, _) => true,
                (Some(cutoff), Some(started)) => started <= cutoff,
                (Some(_), None) => false,
            }
        };
    for trial in &graded {
        if ended_at.is_some() {
            break;
        }
        if !would_run(trial, &stopped_at, ended_at) {
            continue;
        }
        read += 1;
        *observed.entry(trial.arm.clone()).or_default() += 1;
        let at = trial.finished_at.clone().unwrap_or_default();
        let at_ms = ms(&trial.finished_at).unwrap_or(i64::MAX);
        input.cells.insert(
            (trial.arm.clone(), trial.task.clone(), trial.attempt),
            Cell::of(&trial.state, trial.reward),
        );
        if let Some(usd) = trial.cost_usd {
            let entry = costs.entry(trial.arm.clone()).or_default();
            entry.0 += usd;
            entry.1 += 1;
        }
        input.mean_cost = costs
            .iter()
            .filter(|(arm, (_, n))| observed.get(*arm) == Some(n))
            .map(|(arm, (sum, n))| (arm.clone(), sum / *n as f64))
            .collect();
        // A trial that ended without a grade before now never counts.
        for other in trials {
            if !other.graded()
                && other.state != "pending"
                && other.state != "running"
                && ms(&other.finished_at).is_some_and(|f| f <= at_ms)
            {
                input.cells.insert(
                    (other.arm.clone(), other.task.clone(), other.attempt),
                    Cell::Dead,
                );
            }
        }
        last = evaluate(&input, rule);
        for arm in last.stopped() {
            if input.stopped.contains_key(&arm.arm) {
                continue;
            }
            input
                .stopped
                .insert(arm.arm.clone(), (arm.state, arm.reason.clone()));
            stopped_at.insert(arm.arm.clone(), at_ms);
            stops.push(ReplayStop {
                after_graded: read,
                at: at.clone(),
                trigger: trial.job.clone(),
                arm: Some(arm.arm.clone()),
                state: arm.state.word().to_owned(),
                reason: arm.reason.clone(),
            });
        }
        if last.ended {
            ended_at = Some(at_ms);
            stops.push(ReplayStop {
                after_graded: read,
                at,
                trigger: trial.job.clone(),
                arm: None,
                state: last.verdict.unwrap_or("ended").to_owned(),
                reason: last.reason.clone(),
            });
        }
    }
    let mut saved = Vec::new();
    let mut skipped_anyway = 0;
    let mut seen = BTreeSet::new();
    for trial in trials {
        if !seen.insert(&trial.job) || would_run(trial, &stopped_at, ended_at) {
            continue;
        }
        // A trial that ran to the end would not have run; one still pending
        // or interrupted never finished anyway.
        if trial.state == "finished" {
            saved.push((trial.job.clone(), trial.cost_usd));
        } else {
            skipped_anyway += 1;
        }
    }
    Replay {
        rule,
        stops,
        graded: graded.len(),
        read,
        saved,
        skipped_anyway,
        last,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &str =
        include_str!("../../../bench/terminal-bench/tests/fixtures/stop-rule/cases.json");

    #[test]
    fn every_shared_case_gives_the_expected_verdict() {
        let cases: Value = serde_json::from_str(CASES).unwrap();
        let cases = cases["cases"].as_array().unwrap();
        assert!(cases.len() >= 6);
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let (input, rule) = Input::from_json(&case["input"]).unwrap();
            let verdict = evaluate(&input, rule);
            let expect = &case["expect"];
            assert_eq!(verdict.ended, expect["ended"], "{name}: ended");
            assert_eq!(
                verdict.verdict.map(Value::from).unwrap_or(Value::Null),
                expect["verdict"],
                "{name}: verdict"
            );
            for (arm, state) in expect["arms"].as_object().unwrap() {
                let found = verdict.arms.iter().find(|a| &a.arm == arm).unwrap();
                assert_eq!(found.state.word(), state, "{name}: {arm}");
            }
        }
    }

    fn recorded(arm: &str, task: &str, attempt: u64, reward: Option<f64>, minute: u32) -> Recorded {
        Recorded {
            job: format!("tb4--{arm}--{task}--x-r{attempt}"),
            arm: arm.to_owned(),
            task: task.to_owned(),
            attempt,
            state: if reward.is_some() {
                "finished"
            } else {
                "pending"
            }
            .to_owned(),
            reward,
            started_at: reward.map(|_| format!("2026-09-24T00:{minute:02}:00+00:00")),
            finished_at: reward.map(|_| format!("2026-09-24T00:{minute:02}:30+00:00")),
            cost_usd: reward.map(|_| 1.0),
        }
    }

    #[test]
    fn a_replay_stops_where_the_rule_first_holds_and_counts_what_it_saves() {
        // Two arms, one task, ten attempts, the baseline's attempt first.
        // The candidate passes every attempt and the baseline fails every
        // one. Once the baseline's ninth attempt fails, eight pairs went to
        // the candidate; the ninth can at worst tie, and the tenth can at
        // worst go to the baseline: 2 × P(X ≤ 1 | 9) = 0.039, below 0.05.
        let arms = vec!["base".to_owned(), "cand".to_owned()];
        let tasks = vec!["t".to_owned()];
        let mut trials = Vec::new();
        let mut minute = 0;
        for attempt in 1..=10 {
            trials.push(recorded("base", "t", attempt, Some(0.0), minute));
            trials.push(recorded("cand", "t", attempt, Some(1.0), minute + 1));
            minute += 2;
        }
        let replay = replay(&arms, &tasks, 10, &trials, Rule::default());
        let first = &replay.stops[0];
        assert_eq!(first.arm.as_deref(), Some("cand"));
        assert_eq!(first.state, "decided");
        assert_eq!(first.after_graded, 17);
        assert!(
            replay
                .stops
                .iter()
                .any(|s| s.arm.is_none() && s.state == "decided" && s.after_graded == 17)
        );
        // The candidate's ninth and both tenth attempts started later.
        assert_eq!(replay.saved.len(), 3);
        assert_eq!(replay.saved_usd(), (3.0, 0));
        assert_eq!(replay.read, 17);
        assert!(
            replay
                .lines()
                .iter()
                .any(|l| l.contains("would not have started: 3 trials"))
        );
    }

    #[test]
    fn replay_does_not_treat_a_priced_subset_as_the_arm_cost() {
        let arms = vec!["base".to_owned(), "cand".to_owned()];
        let tasks = vec!["t".to_owned()];
        let mut trials = vec![
            recorded("base", "t", 1, Some(1.0), 0),
            recorded("cand", "t", 1, Some(1.0), 1),
            recorded("base", "t", 2, Some(0.0), 2),
            recorded("cand", "t", 2, Some(0.0), 3),
        ];
        trials[3].cost_usd = None;
        let result = replay(&arms, &tasks, 12, &trials, Rule::default());
        assert!(result.stops.is_empty());
        assert_eq!(result.last.arms[0].mean_cost, Some(1.0));
        assert_eq!(result.last.arms[1].mean_cost, None);
    }

    #[test]
    fn a_control_arm_never_enters_the_rule() {
        let mut input = Input {
            arms: vec!["coder".to_owned(), "nop".to_owned()],
            tasks: vec!["t".to_owned()],
            attempts: 9,
            ..Input::default()
        };
        for attempt in 1..=9 {
            input
                .cells
                .insert(("coder".into(), "t".into(), attempt), Cell::Pass);
            input
                .cells
                .insert(("nop".into(), "t".into(), attempt), Cell::Fail);
        }
        let verdict = evaluate(&input, Rule::default());
        assert!(!verdict.ended);
        assert!(verdict.comparisons.is_empty());
        assert_eq!(verdict.arms[1].state, ArmState::Control);
        // With a bar, the one real arm is the candidate.
        let bar = evaluate(
            &input,
            Rule {
                accept_pass_rate: Some(0.5),
                ..Rule::default()
            },
        );
        assert!(!bar.ended);
    }

    #[test]
    fn cells_follow_the_status_rows() {
        assert_eq!(Cell::of("finished", Some(1.0)), Cell::Pass);
        assert_eq!(Cell::of("finished", Some(0.5)), Cell::Fail);
        assert_eq!(Cell::of("finished", None), Cell::Dead);
        assert_eq!(Cell::of("running", None), Cell::Open);
        assert_eq!(Cell::of("skipped", None), Cell::Dead);
    }
}
