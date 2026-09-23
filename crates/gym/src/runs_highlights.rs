//! Highlights: candidate claims worth telling other people, each with the
//! evidence a stranger can check it against.
//!
//! The learning order ranks runs by what a person improving the agents
//! should read. This module answers a different question: which findings
//! are worth sharing, such as an arm that passed a task for a fraction of
//! what another spent, a task this host passes that the leaderboard's top
//! rows fail, or a failure mode several agents share.
//!
//! Code computes every claim from the records with fixed rules. No model
//! writes a number:
//!
//! ```text
//! cost            the same task, both arms passed, one arm's mean cost at
//!                 least FACTOR times the other's
//! time            the same, for the agent's own working time
//! leaderboard     a task an arm here passed that the leaderboard's rows
//!                 ranked TOP_RANK or better pass in at most
//!                 TOP_PASS_AT_MOST of their trials
//! shared-failure  a low-hanging-fruit or misbehavior judgment Jev gave
//!                 runs of at least SHARED_MIN_AGENTS different agents
//! surprise        a run Jev judged `surprise` at SURPRISE_AT or above,
//!                 the SURPRISE_MOST strongest
//! ```
//!
//! Each claim carries its run IDs, its numbers, its sample size, and
//! caveats generated from the data: one run on an arm, costs from list
//! prices applied by hand, a subscription run with no reported cost, arms
//! from different batches, a run a person marked bad. A claim that rests on
//! one run is labeled `n=1` and never phrased as a benchmark result.
//!
//! Nothing here posts anywhere. `gym runs highlights` prints the claims,
//! the Runs pane's Highlights view lists them, and `coder-one ask --scope
//! highlights` drafts short text from chosen claims under a number check.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::runs::{Agent, Catalog, Outcome, Run, Sources, duration, money};
use crate::runs_learning::{self as learning, Answer, Category, JUDGMENTS, REASON_AT};
use crate::runs_marks::{Marks, Verdict};
use crate::terminal_bench_reference::Reference;

/// The schema of `gym runs highlights --json`.
pub const SCHEMA: &str = "openagents.gym.runs-highlights.v1";

/// How many times more one arm must spend, in money or time, for a cost or
/// time claim.
pub const FACTOR: f64 = 2.0;

/// The leaderboard rows a leaderboard claim compares against: those ranked
/// this or better.
pub const TOP_RANK: u64 = 5;

/// The most the top rows may pass a task, as a share of their trials, for
/// a leaderboard claim.
pub const TOP_PASS_AT_MOST: f64 = 0.2;

/// The `surprise` probability a surprise claim needs.
pub const SURPRISE_AT: f64 = 0.7;

/// The most surprise claims: the runs Jev judged most surprising.
pub const SURPRISE_MOST: usize = 8;

/// The fewest agents a shared failure spans.
pub const SHARED_MIN_AGENTS: usize = 2;

/// The fewest runs a shared failure spans.
pub const SHARED_MIN_RUNS: usize = 3;

/// The most runs a claim cites per arm.
pub const CITED_PER_ARM: usize = 6;

/// Which rule found a claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    Cost,
    Time,
    Leaderboard,
    SharedFailure,
    Surprise,
}

impl Rule {
    /// Every rule, in the order `--rule` lists them.
    pub const ALL: [Rule; 5] = [
        Rule::Cost,
        Rule::Time,
        Rule::Leaderboard,
        Rule::SharedFailure,
        Rule::Surprise,
    ];

    /// The rule's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Rule::Cost => "cost",
            Rule::Time => "time",
            Rule::Leaderboard => "leaderboard",
            Rule::SharedFailure => "shared-failure",
            Rule::Surprise => "surprise",
        }
    }

    /// Parses a rule's word.
    ///
    /// # Errors
    ///
    /// Returns a message naming the rules.
    pub fn parse(text: &str) -> Result<Self, String> {
        Rule::ALL
            .into_iter()
            .find(|rule| rule.word() == text.trim())
            .ok_or_else(|| {
                format!(
                    "unknown rule {text}; the rules are {}",
                    Rule::ALL.map(Rule::word).join(", ")
                )
            })
    }
}

/// One number a claim rests on: what it is, its value, and the text the
/// claim writes it as.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    pub label: String,
    pub value: f64,
    pub text: String,
}

impl Number {
    fn new(label: &str, value: f64, text: String) -> Self {
        Number {
            label: label.to_owned(),
            value,
            text,
        }
    }

    fn count(label: &str, value: usize) -> Self {
        Number::new(label, value as f64, value.to_string())
    }
}

/// One candidate claim.
#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    /// A stable name: the rule and a digest of what the claim compares.
    pub key: String,
    pub rule: Rule,
    /// The claim in one sentence, written by code from [`Highlight::numbers`].
    pub claim: String,
    pub task: Option<String>,
    /// The runs it rests on, as `job/trial`.
    pub runs: Vec<String>,
    pub numbers: Vec<Number>,
    /// The sample size: the fewest runs any side of the claim rests on.
    pub sample: usize,
    /// Whether some side of the claim rests on one run.
    pub n1: bool,
    pub caveats: Vec<String>,
    /// How strong the finding is, from 0 to 1, before the sample and marks
    /// weigh it.
    pub strength: f64,
    /// What the claims are ranked by: the strength, halved for a claim that
    /// rests on one run and halved again when a person marked a cited run
    /// bad.
    pub score: f64,
}

impl Highlight {
    /// The claim as `--json` carries it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "key": self.key,
            "rule": self.rule.word(),
            "claim": self.claim,
            "task": self.task,
            "runs": self.runs,
            "numbers": self.numbers.iter().map(|n| json!({"label": n.label, "value": n.value, "text": n.text})).collect::<Vec<_>>(),
            "sample": self.sample,
            "n1": self.n1,
            "caveats": self.caveats,
            "strength": round3(self.strength),
            "score": round3(self.score),
        })
    }

    /// `n=1` or `n=4`.
    #[must_use]
    pub fn sample_label(&self) -> String {
        if self.n1 {
            "n=1: rests on one run, an anecdote rather than a benchmark result".to_owned()
        } else {
            format!("n={}", self.sample)
        }
    }
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// What the rules read.
pub struct Inputs<'a> {
    pub runs: &'a [Run],
    /// Jev's answer per run, by `job/trial`.
    pub answers: &'a HashMap<String, &'a Answer>,
    pub reference: Option<&'a Reference>,
    pub marks: &'a Marks,
}

/// Whether a run's agent is one a claim may be about: not the reference
/// solution, the do-nothing control, or an unknown agent.
fn eligible(run: &Run) -> bool {
    !matches!(run.agent, Agent::Reference | Agent::Control | Agent::Other)
        && matches!(run.outcome, Outcome::Passed | Outcome::Failed)
}

fn key(rule: Rule, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rule.word().as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("{}-{hex}", rule.word())
}

/// Computes every claim, strongest first.
#[must_use]
pub fn compute(inputs: &Inputs<'_>) -> Vec<Highlight> {
    let mut claims = Vec::new();
    claims.extend(arm_differences(inputs, Rule::Cost));
    claims.extend(arm_differences(inputs, Rule::Time));
    claims.extend(leaderboard(inputs));
    claims.extend(shared_failures(inputs));
    claims.extend(surprises(inputs));
    for claim in &mut claims {
        let marked_bad = claim
            .runs
            .iter()
            .filter(|run| inputs.marks.verdict(run) == Some(Verdict::Bad))
            .count();
        let mut score = claim.strength;
        if claim.n1 {
            score *= 0.5;
        }
        if marked_bad > 0 && claim.rule != Rule::SharedFailure {
            score *= 0.5;
        }
        claim.score = score;
    }
    claims.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(b.sample.cmp(&a.sample))
            .then(b.strength.total_cmp(&a.strength))
            .then(a.rule.cmp(&b.rule))
            .then(a.key.cmp(&b.key))
    });
    claims
}

/// One arm's runs of one task.
struct Arm<'a> {
    label: String,
    agent: Agent,
    graded: Vec<&'a Run>,
    passed: Vec<&'a Run>,
    /// The passing runs with the metric, and the metric's value.
    measured: Vec<(&'a Run, f64)>,
}

impl Arm<'_> {
    fn mean(&self) -> Option<f64> {
        (!self.measured.is_empty())
            .then(|| self.measured.iter().map(|(_, v)| v).sum::<f64>() / self.measured.len() as f64)
    }
}

/// Graded runs of eligible agents by task, then by arm.
fn by_task_and_arm<'a>(runs: &'a [Run]) -> BTreeMap<String, BTreeMap<String, Vec<&'a Run>>> {
    let mut tasks: BTreeMap<String, BTreeMap<String, Vec<&'a Run>>> = BTreeMap::new();
    for run in runs.iter().filter(|run| eligible(run)) {
        tasks
            .entry(run.task.clone())
            .or_default()
            .entry(run.agent_label())
            .or_default()
            .push(run);
    }
    tasks
}

fn cite<'a>(runs: impl Iterator<Item = &'a Run>) -> Vec<String> {
    runs.take(CITED_PER_ARM).map(Run::id).collect()
}

/// The cost and time rules: per task, the arm with the lowest mean among
/// the passing runs, against each other agent's best-sampled arm, when the
/// other spent at least [`FACTOR`] times as much.
fn arm_differences(inputs: &Inputs<'_>, rule: Rule) -> Vec<Highlight> {
    let metric = |run: &Run| -> Option<f64> {
        match rule {
            Rule::Cost => run.cost_usd.filter(|cost| *cost > 0.0),
            _ => run.agent_ms.filter(|ms| *ms > 0).map(|ms| ms as f64),
        }
    };
    let show = |value: f64| match rule {
        Rule::Cost => money(value),
        _ => duration(value.round() as u64),
    };
    let mut claims = Vec::new();
    for (task, arms) in by_task_and_arm(inputs.runs) {
        let arms: Vec<Arm> = arms
            .into_iter()
            .map(|(label, graded)| {
                let passed: Vec<&Run> = graded
                    .iter()
                    .copied()
                    .filter(|run| run.outcome == Outcome::Passed)
                    .collect();
                let measured = passed
                    .iter()
                    .filter_map(|run| Some((*run, metric(run)?)))
                    .collect();
                Arm {
                    label,
                    agent: graded[0].agent,
                    graded,
                    passed,
                    measured,
                }
            })
            .filter(|arm| !arm.measured.is_empty())
            .collect();
        if arms.len() < 2 {
            continue;
        }
        let low = arms
            .iter()
            .min_by(|a, b| {
                a.mean()
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.mean().unwrap_or(f64::MAX))
                    .then(b.measured.len().cmp(&a.measured.len()))
            })
            .expect("two arms");
        let agents: BTreeSet<Agent> = arms.iter().map(|arm| arm.agent).collect();
        for agent in agents.into_iter().filter(|agent| *agent != low.agent) {
            // The other agent's arm with the most measured runs says the
            // most about it; a tie goes to the higher mean.
            let Some(high) = arms.iter().filter(|arm| arm.agent == agent).max_by(|a, b| {
                a.measured
                    .len()
                    .cmp(&b.measured.len())
                    .then(a.mean().unwrap_or(0.0).total_cmp(&b.mean().unwrap_or(0.0)))
            }) else {
                continue;
            };
            let (Some(low_mean), Some(high_mean)) = (low.mean(), high.mean()) else {
                continue;
            };
            let ratio = high_mean / low_mean;
            if ratio < FACTOR {
                continue;
            }
            claims.push(arm_claim(
                inputs, rule, &task, low, high, low_mean, high_mean, &show,
            ));
        }
    }
    claims
}

#[allow(clippy::too_many_arguments)]
fn arm_claim(
    inputs: &Inputs<'_>,
    rule: Rule,
    task: &str,
    low: &Arm<'_>,
    high: &Arm<'_>,
    low_mean: f64,
    high_mean: f64,
    show: &dyn Fn(f64) -> String,
) -> Highlight {
    let ratio = high_mean / low_mean;
    let share = (low_mean / high_mean * 100.0).round();
    let (low_text, high_text) = (show(low_mean), show(high_mean));
    let numbers = vec![
        Number::new("low_mean", low_mean, low_text.clone()),
        Number::new("high_mean", high_mean, high_text.clone()),
        Number::new("share_percent", share, format!("{share:.0}%")),
        Number::new("ratio", ratio, format!("{ratio:.1}")),
        Number::count("low_runs", low.measured.len()),
        Number::count("high_runs", high.measured.len()),
        Number::count("low_passed", low.passed.len()),
        Number::count("low_graded", low.graded.len()),
        Number::count("high_passed", high.passed.len()),
        Number::count("high_graded", high.graded.len()),
    ];
    let claim = match rule {
        Rule::Cost => format!(
            "{} passed {task} for {share:.0}% of what {} spent: {low_text} against {high_text} a passing run on average, {ratio:.1} times as much for the second, over {} and {} runs.",
            low.label,
            high.label,
            low.measured.len(),
            high.measured.len()
        ),
        _ => format!(
            "{} passed {task} in {share:.0}% of {}'s agent time: {low_text} against {high_text} a passing run on average, {ratio:.1} times as long for the second, over {} and {} runs.",
            low.label,
            high.label,
            low.measured.len(),
            high.measured.len()
        ),
    };
    let mut caveats = Vec::new();
    for arm in [low, high] {
        if arm.measured.len() == 1 {
            caveats.push(format!(
                "{} has one passing run with {} here: an anecdote, not a benchmark result.",
                arm.label,
                if rule == Rule::Cost {
                    "a cost"
                } else {
                    "an agent time"
                }
            ));
        }
        let missing = arm.passed.len() - arm.measured.len();
        if missing > 0 {
            caveats.push(match rule {
                Rule::Cost => format!(
                    "{missing} of {}'s {} passing runs report no cost (a subscription run), so the mean leaves them out.",
                    arm.label,
                    arm.passed.len()
                ),
                _ => format!(
                    "{missing} of {}'s {} passing runs record no agent time, so the mean leaves them out.",
                    arm.label,
                    arm.passed.len()
                ),
            });
        }
        if rule == Rule::Cost {
            if arm.measured.iter().any(|(run, _)| run.cost_estimated) {
                caveats.push(format!(
                    "{}'s cost comes from list prices applied by hand to its token counts, not a bill.",
                    arm.label
                ));
            }
            let ledgers: Vec<Ledger> = arm
                .measured
                .iter()
                .map(|(run, _)| Ledger::of(run))
                .collect();
            if ledgers.iter().any(|ledger| ledger.estimated) {
                caveats.push(format!(
                    "{}'s cost includes an executor priced by hand from its token counts (Codex reports no cost), not a bill.",
                    arm.label
                ));
            }
            if arm.agent == Agent::ClaudeCode || ledgers.iter().any(|ledger| ledger.cli_list_price)
            {
                caveats.push(format!(
                    "{}'s cost is the Claude Code CLI's own list-price figure; these runs used a subscription, so it isn't a bill.",
                    arm.label
                ));
            }
        }
    }
    let rate = |arm: &Arm<'_>| arm.passed.len() as f64 / arm.graded.len().max(1) as f64;
    if rate(low) < rate(high) {
        caveats.push(format!(
            "{} passed {} of its {} graded runs of the task and {} passed {} of {}: the {} arm fails more often.",
            low.label,
            low.passed.len(),
            low.graded.len(),
            high.label,
            high.passed.len(),
            high.graded.len(),
            if rule == Rule::Cost {
                "cheaper"
            } else {
                "faster"
            }
        ));
    }
    let batches = |arm: &Arm<'_>| -> BTreeSet<String> {
        arm.measured
            .iter()
            .map(|(run, _)| run.batch.clone())
            .collect()
    };
    let (low_batches, high_batches) = (batches(low), batches(high));
    if low_batches.is_disjoint(&high_batches) {
        caveats.push(format!(
            "The arms ran in different batches ({} and {}), not side by side.",
            low_batches.into_iter().collect::<Vec<_>>().join(", "),
            high_batches.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let runs: Vec<String> = cite(low.measured.iter().map(|(run, _)| *run))
        .into_iter()
        .chain(cite(high.measured.iter().map(|(run, _)| *run)))
        .collect();
    caveats.extend(mark_caveats(inputs.marks, &runs));
    let sample = low.measured.len().min(high.measured.len());
    Highlight {
        key: key(rule, &[task, &low.label, &high.label]),
        rule,
        claim,
        task: Some(task.to_owned()),
        runs,
        numbers,
        sample,
        n1: sample == 1,
        caveats,
        // A difference of 32 times is as strong as one gets.
        strength: (ratio.log2() / 5.0).clamp(0.0, 1.0),
        score: 0.0,
    }
}

/// What Coder One's cost ledger says about where its cost came from.
#[derive(Default)]
struct Ledger {
    /// A delegated executor's cost is a price estimate from token counts.
    estimated: bool,
    /// A delegated Claude Code's cost is the CLI's list-price figure.
    cli_list_price: bool,
}

impl Ledger {
    fn of(run: &Run) -> Self {
        let Some(episode) = &run.files.episode else {
            return Ledger::default();
        };
        let Some(usage) = crate::runs::read_json(&episode.join("evaluation/usage.json"))
            .or_else(|| crate::runs::read_json(&episode.join("usage.json")))
        else {
            return Ledger::default();
        };
        let delegate = usage
            .pointer("/components/delegate")
            .filter(|delegate| delegate["cost_usd"].as_f64().is_some_and(|c| c > 0.0));
        let provenance = delegate
            .and_then(|delegate| delegate["cost_provenance"].as_str())
            .unwrap_or_default();
        Ledger {
            estimated: provenance == "price_estimate",
            cli_list_price: provenance.starts_with("cli_list_price"),
        }
    }
}

/// A caveat for each cited run a person marked bad.
fn mark_caveats(marks: &Marks, runs: &[String]) -> Vec<String> {
    runs.iter()
        .filter(|run| marks.verdict(run) == Some(Verdict::Bad))
        .map(|run| format!("A person marked {run} bad."))
        .collect()
}

/// The leaderboard rule.
fn leaderboard(inputs: &Inputs<'_>) -> Vec<Highlight> {
    let Some(reference) = inputs.reference else {
        return Vec::new();
    };
    let top_rows = reference
        .entries
        .iter()
        .filter(|entry| entry.rank.is_some_and(|rank| rank <= TOP_RANK))
        .count();
    let mut claims = Vec::new();
    for (task, arms) in by_task_and_arm(inputs.runs) {
        let rows = reference.task(&task);
        let top: Vec<_> = rows
            .iter()
            .filter(|(entry, _)| entry.rank.is_some_and(|rank| rank <= TOP_RANK))
            .collect();
        let successes: u64 = top.iter().map(|(_, result)| result.successes).sum();
        let trials: u64 = top.iter().map(|(_, result)| result.trials).sum();
        if trials == 0 {
            continue;
        }
        let rate = successes as f64 / trials as f64;
        if rate > TOP_PASS_AT_MOST {
            continue;
        }
        let passing: Vec<(&String, &Vec<&Run>, usize)> = arms
            .iter()
            .map(|(label, runs)| {
                (
                    label,
                    runs,
                    runs.iter()
                        .filter(|run| run.outcome == Outcome::Passed)
                        .count(),
                )
            })
            .filter(|(_, _, passes)| *passes > 0)
            .collect();
        let Some((best, best_runs, best_passes)) = passing.iter().max_by(|a, b| {
            a.2.cmp(&b.2)
                .then((a.2 * b.1.len()).cmp(&(b.2 * a.1.len())))
                .then(b.0.cmp(a.0))
        }) else {
            continue;
        };
        let rows_ever = rows.iter().filter(|(_, r)| r.successes > 0).count();
        let percent = (rate * 100.0).round();
        let mut numbers = vec![
            Number::count("passes_here", *best_passes),
            Number::count("graded_here", best_runs.len()),
            Number::count("top_rows", top.len()),
            Number::new("top_successes", successes as f64, successes.to_string()),
            Number::new("top_trials", trials as f64, trials.to_string()),
            Number::new("top_percent", percent, format!("{percent:.0}%")),
            Number::count("rows_that_ever_pass", rows_ever),
            Number::count("rows", rows.len()),
        ];
        let mut claim = format!(
            "{best} passed {task} in {best_passes} of {} graded runs here; the leaderboard's top {} rows (rank {TOP_RANK} or better) passed it {successes} of {trials} trials ({percent:.0}%), and {rows_ever} of its {} rows ever pass it.",
            best_runs.len(),
            top.len(),
            rows.len(),
        );
        if passing.len() > 1 {
            numbers.push(Number::count("arms_passing_here", passing.len()));
            claim.push_str(&format!(" {} arms here passed it.", passing.len()));
        }
        let mut runs: Vec<String> = Vec::new();
        for (_, arm_runs, _) in &passing {
            runs.extend(cite(
                arm_runs
                    .iter()
                    .copied()
                    .filter(|run| run.outcome == Outcome::Passed),
            ));
        }
        let mut caveats = Vec::new();
        if *best_passes == 1 {
            caveats.push(format!(
                "{best} passed it in one run here: an anecdote, not a benchmark result."
            ));
        }
        if best_runs.len() > *best_passes {
            caveats.push(format!(
                "{best} failed {} of its {} graded runs of it here.",
                best_runs.len() - best_passes,
                best_runs.len()
            ));
        }
        caveats.push(format!(
            "The leaderboard's rows ran on their own infrastructure and limits (dataset {}, fetched {}); this host's runs did not, and its top {top_rows} rows are the comparison, not every row.",
            if reference.dataset_ref.is_empty() {
                "unknown"
            } else {
                &reference.dataset_ref
            },
            reference.fetched_at.get(..10).unwrap_or(&reference.fetched_at),
        ));
        caveats.extend(mark_caveats(inputs.marks, &runs));
        claims.push(Highlight {
            key: key(Rule::Leaderboard, &[&task, best]),
            rule: Rule::Leaderboard,
            claim,
            task: Some(task.clone()),
            runs,
            numbers,
            sample: *best_passes,
            n1: *best_passes == 1,
            caveats,
            strength: 1.0 - rate,
            score: 0.0,
        });
    }
    claims
}

/// The shared-failure rule.
fn shared_failures(inputs: &Inputs<'_>) -> Vec<Highlight> {
    let judged: Vec<(&Run, &Answer)> = inputs
        .runs
        .iter()
        .filter(|run| eligible(run))
        .filter_map(|run| Some((run, *inputs.answers.get(&run.id())?)))
        .collect();
    let mut judged_by_agent: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (run, _) in &judged {
        *judged_by_agent.entry(run.agent.name()).or_default() += 1;
    }
    let mut claims = Vec::new();
    for judgment in JUDGMENTS
        .iter()
        .filter(|j| matches!(j.category, Category::Fruit | Category::Misbehavior))
    {
        let mut members: Vec<(&Run, f64)> = judged
            .iter()
            .filter_map(|(run, answer)| {
                let p = *answer.nouls.get(judgment.id)?;
                (p >= REASON_AT).then_some((*run, p))
            })
            .collect();
        members.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.id().cmp(&b.0.id())));
        let mut by_agent: BTreeMap<&'static str, Vec<(&Run, f64)>> = BTreeMap::new();
        for (run, p) in &members {
            by_agent
                .entry(run.agent.name())
                .or_default()
                .push((run, *p));
        }
        if by_agent.len() < SHARED_MIN_AGENTS || members.len() < SHARED_MIN_RUNS {
            continue;
        }
        let mean = members.iter().map(|(_, p)| p).sum::<f64>() / members.len() as f64;
        let mut numbers = vec![
            Number::count("runs", members.len()),
            Number::count("agents", by_agent.len()),
            Number::new("mean_probability", mean, format!("{mean:.2}")),
        ];
        let mut parts = Vec::new();
        for (agent, runs) in &by_agent {
            let of = judged_by_agent.get(agent).copied().unwrap_or(0);
            numbers.push(Number::count(&format!("{agent} runs"), runs.len()));
            numbers.push(Number::count(&format!("{agent} judged"), of));
            parts.push(format!("{agent} {} of {of} judged runs", runs.len()));
        }
        let claim = format!(
            "Jev found {} ({}) in {} runs by {} agents: {}; mean probability {mean:.2}.",
            judgment.tag,
            judgment.id,
            members.len(),
            by_agent.len(),
            parts.join(", ")
        );
        // The strongest two per agent, so every agent is cited.
        let runs: Vec<String> = by_agent
            .values()
            .flat_map(|runs| runs.iter().take(2).map(|(run, _)| run.id()))
            .collect();
        let mut caveats = vec![format!(
            "This is Jev's judgment ({}, {} or above), not a person's.",
            learning::QUESTION_SET,
            REASON_AT
        )];
        let smallest = by_agent.values().map(Vec::len).min().unwrap_or(0);
        for (agent, runs) in &by_agent {
            if runs.len() == 1 {
                caveats.push(format!(
                    "{agent} has one such run: an anecdote, not a pattern."
                ));
            }
        }
        let member_ids: Vec<String> = members.iter().map(|(run, _)| run.id()).collect();
        let tagged = member_ids
            .iter()
            .filter(|run| {
                inputs.marks.of_run(run).iter().any(|mark| {
                    mark.verdict == Verdict::Bad && mark.tags.iter().any(|t| t == judgment.id)
                })
            })
            .count();
        let cleared = member_ids
            .iter()
            .filter(|run| inputs.marks.verdict(run) == Some(Verdict::Clear))
            .count();
        if tagged + cleared == 0 {
            caveats.push(format!(
                "No person has marked these runs, so how often Jev is right about {} is unmeasured.",
                judgment.id
            ));
        } else {
            numbers.push(Number::count("tagged_by_a_person", tagged));
            numbers.push(Number::count("cleared_by_a_person", cleared));
            caveats.push(format!(
                "A person tagged {tagged} of these runs {} and cleared {cleared}.",
                judgment.id
            ));
        }
        let agents = by_agent.len() as f64;
        claims.push(Highlight {
            key: key(Rule::SharedFailure, &[judgment.id]),
            rule: Rule::SharedFailure,
            claim,
            task: None,
            runs,
            numbers,
            sample: smallest,
            n1: smallest == 1,
            caveats,
            strength: mean * (agents / 3.0).min(1.0),
            score: 0.0,
        });
    }
    claims
}

/// The surprise rule.
fn surprises(inputs: &Inputs<'_>) -> Vec<Highlight> {
    let mut surprising: Vec<(&Run, f64)> = inputs
        .runs
        .iter()
        .filter(|run| eligible(run))
        .filter_map(|run| {
            let p = *inputs.answers.get(&run.id())?.nouls.get("surprise")?;
            (p >= SURPRISE_AT).then_some((run, p))
        })
        .collect();
    surprising.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.id().cmp(&b.0.id())));
    surprising.truncate(SURPRISE_MOST);
    let mut claims = Vec::new();
    for (run, p) in surprising {
        let others: Vec<&Run> = inputs
            .runs
            .iter()
            .filter(|other| other.task == run.task && other.id() != run.id() && eligible(other))
            .collect();
        let others_passed = others
            .iter()
            .filter(|other| other.outcome == Outcome::Passed)
            .count();
        let mut numbers = vec![
            Number::new("surprise", p, format!("{p:.2}")),
            Number::count("other_runs_here", others.len()),
            Number::count("other_runs_passed_here", others_passed),
        ];
        let mut claim = format!(
            "{} {} {}, an outcome Jev judged surprising ({p:.2}); this host's other runs of it passed {others_passed} of {}",
            run.agent_label(),
            run.outcome.word(),
            run.task,
            others.len()
        );
        let mut caveats = vec![
            "One run: an anecdote, not a benchmark result.".to_owned(),
            format!(
                "Surprise is Jev's judgment ({}), not a person's.",
                learning::QUESTION_SET
            ),
        ];
        let rows = inputs
            .reference
            .map(|reference| reference.task(&run.task))
            .unwrap_or_default();
        let trials: u64 = rows.iter().map(|(_, r)| r.trials).sum();
        if trials > 0 {
            let successes: u64 = rows.iter().map(|(_, r)| r.successes).sum();
            let percent = (successes as f64 / trials as f64 * 100.0).round();
            numbers.push(Number::new(
                "leaderboard_successes",
                successes as f64,
                successes.to_string(),
            ));
            numbers.push(Number::new(
                "leaderboard_trials",
                trials as f64,
                trials.to_string(),
            ));
            numbers.push(Number::new(
                "leaderboard_percent",
                percent,
                format!("{percent:.0}%"),
            ));
            claim.push_str(&format!(
                ", and the leaderboard's rows passed it {successes} of {trials} trials ({percent:.0}%)."
            ));
        } else {
            claim.push('.');
            caveats.push("The leaderboard has no row for this task.".to_owned());
        }
        let runs = vec![run.id()];
        caveats.extend(mark_caveats(inputs.marks, &runs));
        claims.push(Highlight {
            key: key(Rule::Surprise, &[&run.id()]),
            rule: Rule::Surprise,
            claim,
            task: Some(run.task.clone()),
            runs,
            numbers,
            sample: 1,
            n1: true,
            caveats,
            strength: p,
            score: 0.0,
        });
    }
    claims
}

/// The claims as `--json` prints them.
#[must_use]
pub fn highlights_json(claims: &[Highlight], shown: usize, runs: usize) -> Value {
    json!({
        "schema": SCHEMA,
        "runs": runs,
        "total": claims.len(),
        "shown": shown.min(claims.len()),
        "rules": {
            "factor": FACTOR,
            "top_rank": TOP_RANK,
            "top_pass_at_most": TOP_PASS_AT_MOST,
            "surprise_at": SURPRISE_AT,
            "surprise_most": SURPRISE_MOST,
            "shared_min_agents": SHARED_MIN_AGENTS,
            "shared_min_runs": SHARED_MIN_RUNS,
            "reason_at": REASON_AT,
        },
        "by_rule": Rule::ALL.iter().map(|rule| (rule.word().to_owned(), json!(claims.iter().filter(|c| c.rule == *rule).count()))).collect::<serde_json::Map<String, Value>>(),
        "highlights": claims.iter().take(shown).map(Highlight::to_json).collect::<Vec<_>>(),
    })
}

/// The claims as text.
#[must_use]
pub fn highlights_text(claims: &[Highlight], shown: usize, runs: usize) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Highlights: {} candidate claims from {runs} runs, computed by fixed rules. No model wrote these numbers, and nothing here posts anywhere.",
            claims.len()
        ),
        format!(
            "By rule: {}.",
            Rule::ALL
                .iter()
                .map(|rule| format!(
                    "{} {}",
                    rule.word(),
                    claims.iter().filter(|c| c.rule == *rule).count()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
    ];
    for (index, claim) in claims.iter().take(shown).enumerate() {
        lines.push(format!(
            "{:>2}. [{}] {}",
            index + 1,
            claim.rule.word(),
            claim.claim
        ));
        lines.push(format!(
            "    {} · {} · score {:.2}",
            claim.key,
            claim.sample_label(),
            claim.score
        ));
        lines.push(format!("    runs: {}", claim.runs.join(", ")));
        for caveat in &claim.caveats {
            lines.push(format!("    - {caveat}"));
        }
        lines.push(String::new());
    }
    if claims.len() > shown {
        lines.push(format!(
            "{} more; --limit N shows more.",
            claims.len() - shown
        ));
    }
    lines.push(
        "Draft text from chosen claims: coder-one ask --scope highlights --claim KEY".to_owned(),
    );
    lines
}

/// The usage of `gym runs highlights`.
pub const USAGE: &str = "\
gym runs highlights: candidate claims worth sharing, with their evidence.

Usage:
  gym runs highlights [--rule RULE]... [--limit N] [--json]

Computes candidate claims from the runs with fixed rules; no model writes a
number, and nothing posts anywhere. The rules:

  cost            both arms passed the same task, and one spent at least
                  2 times as much on average
  time            the same, for the agent's own working time
  leaderboard     an arm here passed a task that the leaderboard's rows
                  ranked 5 or better pass in at most 20% of their trials
  shared-failure  a low-hanging-fruit or misbehavior judgment Jev gave runs
                  of at least 2 agents, 3 runs in all
  surprise        a run Jev judged `surprise` at 0.70 or above, the 8
                  strongest

Each claim lists its runs, its numbers, its sample size, and caveats from
the data. A claim that rests on one run is labeled n=1. The claims come
strongest first: a claim resting on one run counts half, and one citing a
run a person marked bad counts half again. --rule keeps one rule's claims;
repeat it for several. --limit N shows N claims (default 20).

The source flags --jobs-dir, --traces-dir, --no-jobs, --no-traces,
--no-tasks, --learning-dir, --marks-dir, and --no-reference work as they do
for `gym runs`. `coder-one ask --scope highlights --claim KEY` drafts short
text from chosen claims and refuses a draft whose numbers or citations
don't check.";

/// `gym runs highlights`.
///
/// # Errors
///
/// Returns the usage when the arguments don't parse.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    let mut sources = Sources::standard();
    let mut learning_dir = learning::default_dir();
    let mut marks_dir = crate::runs_marks::default_dir();
    let mut reference = true;
    let mut rules: Vec<Rule> = Vec::new();
    let mut limit = 20usize;
    let mut json_out = false;
    let mut index = 1;
    let value = |index: usize| {
        args.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("{} needs a value\n\n{USAGE}", args[index]))
    };
    while index < args.len() {
        match args[index].as_str() {
            "--rule" => {
                rules.push(Rule::parse(&value(index)?)?);
                index += 1;
            }
            "--limit" => {
                limit = value(index)?
                    .parse()
                    .map_err(|_| format!("--limit needs a number\n\n{USAGE}"))?;
                index += 1;
            }
            "--json" => json_out = true,
            "--learning-dir" => {
                learning_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--marks-dir" => {
                marks_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--jobs-dir" => {
                sources.jobs = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--traces-dir" => {
                sources.traces = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--no-jobs" => sources.jobs = None,
            "--no-traces" => sources.traces = None,
            "--no-tasks" => sources.tasks.clear(),
            "--no-reference" => reference = false,
            "--help" | "-h" => {
                writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    let catalog = Catalog::load(sources);
    let reference = reference.then(Reference::checked).flatten();
    let context = learning::Context::new(&catalog, reference.clone());
    let store = learning::Store::open(learning_dir);
    let answers = learning::answers(&catalog, &store, &context);
    let marks = Marks::open(marks_dir);
    let mut claims = compute(&Inputs {
        runs: &catalog.runs,
        answers: &answers,
        reference: reference.as_ref(),
        marks: &marks,
    });
    if !rules.is_empty() {
        claims.retain(|claim| rules.contains(&claim.rule));
    }
    let write =
        |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    if json_out {
        let value = highlights_json(&claims, limit, catalog.runs.len());
        write(
            out,
            &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
        )?;
        return Ok(0);
    }
    for line in highlights_text(&claims, limit, catalog.runs.len()) {
        write(out, &line)?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::{Files, Tests};
    use crate::runs_marks::NewMark;
    use crate::terminal_bench_reference::{Entry, TaskResult};

    #[allow(clippy::too_many_arguments)]
    fn run(
        job: &str,
        task: &str,
        agent: Agent,
        variant: Option<&str>,
        model: Option<&str>,
        passed: bool,
        cost: Option<f64>,
        agent_ms: Option<u64>,
    ) -> Run {
        Run {
            job: job.to_owned(),
            trial: format!("{task}__1"),
            batch: job.split("--").next().unwrap_or("tb4").to_owned(),
            retained: false,
            files: Files::default(),
            task: task.to_owned(),
            task_path: None,
            ask: None,
            category: None,
            expert_hours: None,
            time_limit_sec: None,
            agent,
            variant: variant.map(str::to_owned),
            model: model.map(str::to_owned),
            started_ms: Some(0),
            ended_ms: Some(1),
            agent_ms,
            active_ms: None,
            outcome: if passed {
                Outcome::Passed
            } else {
                Outcome::Failed
            },
            reward: Some(if passed { 1.0 } else { 0.0 }),
            tests: Some(Tests {
                passed: 1,
                failed: 0,
                total: 1,
            }),
            cost_usd: cost,
            cost_estimated: false,
            notes: Vec::new(),
        }
    }

    fn coder(job: &str, task: &str, passed: bool, cost: Option<f64>, ms: u64) -> Run {
        run(
            job,
            task,
            Agent::CoderOne,
            Some("tunable-v6"),
            None,
            passed,
            cost,
            Some(ms),
        )
    }

    fn claude(job: &str, task: &str, passed: bool, cost: Option<f64>, ms: u64) -> Run {
        run(
            job,
            task,
            Agent::ClaudeCode,
            None,
            Some("claude-opus-5-5"),
            passed,
            cost,
            Some(ms),
        )
    }

    fn answer(run: &Run, yes: &[(&str, f64)]) -> Answer {
        let mut answer = Answer::from_answers(
            &run.id(),
            format!("k-{}", run.id()),
            Value::Null,
            &json!({}),
        );
        for judgment in &JUDGMENTS {
            let p = yes
                .iter()
                .find(|(id, _)| *id == judgment.id)
                .map_or(0.1, |(_, p)| *p);
            answer.nouls.insert(judgment.id.to_owned(), p);
        }
        answer
    }

    fn reference() -> Reference {
        let entry = |rank: u64, successes: u64| Entry {
            rank: Some(rank),
            agent: "Claude Code".to_owned(),
            model: format!("Model {rank}"),
            reasoning_effort: None,
            accuracy: None,
            successes: None,
            trials: None,
            total_cost_usd: None,
            consistent: true,
            tasks: BTreeMap::from([(
                "hard".to_owned(),
                TaskResult {
                    successes,
                    trials: 5,
                    cost_usd: None,
                    mean_agent_sec: None,
                },
            )]),
        };
        Reference {
            leaderboard: "x".to_owned(),
            fetched_at: "2026-09-23T05:18:54+00:00".to_owned(),
            dataset_ref: "v4.0.0".to_owned(),
            entries: vec![entry(1, 0), entry(2, 1), entry(9, 3)],
        }
    }

    fn fixture() -> Vec<Run> {
        vec![
            // cheap: Coder One passes for a fraction of Claude Code's cost.
            coder("tb4--coder-one-a", "cheap", true, Some(0.5), 60_000),
            coder("tb4--coder-one-b", "cheap", true, Some(0.7), 70_000),
            claude("tb4--claude-code-a", "cheap", true, Some(3.0), 90_000),
            claude("tb4--claude-code-b", "cheap", true, Some(3.0), 80_000),
            claude("tb4--claude-code-c", "cheap", true, None, 85_000),
            // even: no large difference.
            coder("tb4--coder-one-c", "even", true, Some(1.0), 60_000),
            claude("tb4--claude-code-d", "even", true, Some(1.5), 60_000),
            // hard: one Coder One pass the top rows rarely manage.
            coder("panel--coder-one-d", "hard", true, Some(2.0), 60_000),
            coder("panel--coder-one-e", "hard", false, Some(2.0), 60_000),
            // A control never counts.
            run(
                "tb4--nop",
                "hard",
                Agent::Control,
                None,
                None,
                true,
                None,
                None,
            ),
        ]
    }

    fn computed(runs: &[Run], answers: &[Answer], marks: &Marks) -> Vec<Highlight> {
        let map: HashMap<String, &Answer> = answers.iter().map(|a| (a.run.clone(), a)).collect();
        let reference = reference();
        compute(&Inputs {
            runs,
            answers: &map,
            reference: Some(&reference),
            marks,
        })
    }

    #[test]
    fn a_large_cost_difference_on_a_task_both_arms_pass_is_a_claim_with_its_caveats() {
        let runs = fixture();
        let claims = computed(&runs, &[], &Marks::open(None));
        let cost: Vec<&Highlight> = claims.iter().filter(|c| c.rule == Rule::Cost).collect();
        assert_eq!(cost.len(), 1, "{cost:#?}");
        let claim = cost[0];
        assert_eq!(claim.task.as_deref(), Some("cheap"));
        assert!(
            claim.claim.starts_with(
                "Coder One · tunable-v6 passed cheap for 20% of what Claude Code · Opus 5.5 spent: $0.60 against $3.00"
            ),
            "{}",
            claim.claim
        );
        assert_eq!(claim.sample, 2);
        assert!(!claim.n1);
        assert_eq!(claim.runs.len(), 4);
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c.contains("1 of Claude Code · Opus 5.5's 3 passing runs report no cost")),
            "{:?}",
            claim.caveats
        );
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c.contains("subscription, so it isn't a bill")),
            "{:?}",
            claim.caveats
        );
        let texts: Vec<&str> = claim.numbers.iter().map(|n| n.text.as_str()).collect();
        for needed in ["$0.60", "$3.00", "20%", "5.0", "2"] {
            assert!(texts.contains(&needed), "{needed} in {texts:?}");
        }
        // `even` differs by 1.5 times, under the factor: no claim.
        assert!(
            claims
                .iter()
                .all(|c| c.task.as_deref() != Some("even") || c.rule != Rule::Cost)
        );
        // Time: 65s against 85s is under the factor.
        assert!(claims.iter().all(|c| c.rule != Rule::Time));
    }

    #[test]
    fn a_task_the_top_rows_fail_is_a_claim_and_one_pass_is_labeled_n1() {
        let runs = fixture();
        let claims = computed(&runs, &[], &Marks::open(None));
        let board: Vec<&Highlight> = claims
            .iter()
            .filter(|c| c.rule == Rule::Leaderboard)
            .collect();
        assert_eq!(board.len(), 1, "{board:#?}");
        let claim = board[0];
        assert!(
            claim.claim.contains(
                "passed hard in 1 of 2 graded runs here; the leaderboard's top 2 rows (rank 5 or better) passed it 1 of 10 trials (10%)"
            ),
            "{}",
            claim.claim
        );
        assert!(claim.n1);
        assert!(claim.sample_label().starts_with("n=1"));
        assert_eq!(claim.runs, vec!["panel--coder-one-d/hard__1"]);
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c.contains("failed 1 of its 2"))
        );
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c.contains("fetched 2026-09-23"))
        );
        // Halved for resting on one run.
        assert!((claim.score - 0.45).abs() < 1e-9, "{}", claim.score);
    }

    #[test]
    fn a_failure_several_agents_share_and_a_surprise_are_claims() {
        let runs = fixture();
        let answers = vec![
            answer(&runs[0], &[("looped", 0.9)]),
            answer(&runs[1], &[("looped", 0.8)]),
            answer(&runs[2], &[("looped", 0.7)]),
            answer(&runs[7], &[("surprise", 0.92)]),
            // A control's judgment never counts.
            answer(&runs[9], &[("looped", 0.99), ("surprise", 0.99)]),
        ];
        let mut marks = Marks::open(None);
        marks
            .mark(NewMark {
                run: runs[2].id(),
                step: None,
                verdict: Verdict::Bad,
                tags: vec!["looped".to_owned()],
                note: None,
                evidence: None,
                author: "test".to_owned(),
                at_ms: 0,
            })
            .unwrap();
        let claims = computed(&runs, &answers, &marks);
        let shared: Vec<&Highlight> = claims
            .iter()
            .filter(|c| c.rule == Rule::SharedFailure)
            .collect();
        assert_eq!(shared.len(), 1, "{shared:#?}");
        let claim = shared[0];
        assert!(
            claim.claim.contains(
                "in 3 runs by 2 agents: Claude Code 1 of 1 judged runs, Coder One 2 of 3 judged runs"
            ),
            "{}",
            claim.claim
        );
        assert!(claim.n1, "Claude Code has one such run");
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c == "A person tagged 1 of these runs looped and cleared 0."),
            "{:?}",
            claim.caveats
        );
        let surprise: Vec<&Highlight> =
            claims.iter().filter(|c| c.rule == Rule::Surprise).collect();
        assert_eq!(surprise.len(), 1);
        assert!(
            surprise[0].claim.contains(
                "passed hard, an outcome Jev judged surprising (0.92); this host's other runs of it passed 0 of 1, and the leaderboard's rows passed it 4 of 15 trials (27%)"
            ),
            "{}",
            surprise[0].claim
        );
        // The same inputs give the same keys and order.
        let again = computed(&runs, &answers, &marks);
        assert_eq!(
            claims.iter().map(|c| &c.key).collect::<Vec<_>>(),
            again.iter().map(|c| &c.key).collect::<Vec<_>>()
        );
        // Every number a claim's sentence writes is one of its numbers or
        // part of a name.
        let json = highlights_json(&claims, 10, runs.len());
        assert_eq!(json["schema"], SCHEMA);
        assert_eq!(json["by_rule"]["surprise"], 1);
        let text = highlights_text(&claims, 10, runs.len()).join("\n");
        assert!(text.contains("n=1: rests on one run"), "{text}");
        assert!(text.contains("nothing here posts anywhere"), "{text}");
    }

    #[test]
    fn the_command_reads_the_fixture_runs() {
        let (dir, sources) = crate::runs::fixture_sources();
        let mut out = Vec::new();
        let args: Vec<String> = [
            "highlights",
            "--jobs-dir",
            &sources.jobs.unwrap().display().to_string(),
            "--traces-dir",
            &sources.traces.unwrap().display().to_string(),
            "--no-tasks",
            "--learning-dir",
            &dir.path().join("learning").display().to_string(),
            "--marks-dir",
            &dir.path().join("marks").display().to_string(),
            "--json",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert_eq!(command(&args, &mut out).unwrap(), 0);
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        assert!(value["runs"].as_u64().unwrap() > 0, "{value}");
        for claim in value["highlights"].as_array().unwrap() {
            assert!(!claim["runs"].as_array().unwrap().is_empty(), "{claim}");
            assert!(claim["sample"].as_u64().unwrap() >= 1, "{claim}");
        }
        assert!(
            command(
                &[
                    "highlights".to_owned(),
                    "--rule".to_owned(),
                    "nope".to_owned()
                ],
                &mut Vec::new()
            )
            .is_err()
        );
    }
}
