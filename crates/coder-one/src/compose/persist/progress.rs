//! What `control.persist` measures between rounds, and what it runs next.
//!
//! v5 let persistence rounds run that moved no test: on Terminal-Bench 4.0
//! one `cargo-flight-dispatch` trial ran seven sessions for about $12.75
//! and took the task from 8 failing tests to 2 without a pass. This module
//! holds the parts of the answer that need no executor, so each is tested
//! in milliseconds:
//!
//! - [`OwnTests`] and [`TestRun`]: the executor keeps its own tests behind
//!   one runner script that prints `PASS <name>` or `FAIL <name>` for each
//!   test, and the host runs it after every round.
//! - [`Delta`]: what a round changed, as tests fixed and broken and check
//!   failures before and after, and whether that is progress.
//! - [`CheapRounds`] and [`after_round`]: later rounds run a cheaper
//!   executor, and a cheap round that makes no progress hands the next
//!   round back to the strong one.
//! - [`SpendCap`]: the rounds together may spend only a share of what the
//!   task's budget has left when they start.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::compose::Standing;
use crate::handoff::Tier;

fn runner() -> String {
    "/tmp/persist-tests/run.sh".to_string()
}
fn test_timeout() -> u64 {
    600
}
fn from_round() -> u32 {
    2
}
fn one() -> u32 {
    1
}
fn half() -> f64 {
    0.5
}

/// The executor's own tests, run by the host after each round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnTests {
    /// The runner the brief asks the executor to keep. The host runs it
    /// with `bash` from the working directory.
    #[serde(default = "runner")]
    pub runner: String,
    /// The longest the host waits for the runner, in seconds.
    #[serde(default = "test_timeout")]
    pub timeout_sec: u64,
}

impl Default for OwnTests {
    fn default() -> Self {
        OwnTests {
            runner: runner(),
            timeout_sec: test_timeout(),
        }
    }
}

/// Later rounds on a cheaper executor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheapRounds {
    /// The cheap executors, taken in turn by each cheap round.
    pub tiers: Vec<Tier>,
    /// The first round that runs cheap. Earlier rounds run the executor
    /// that produced the candidate.
    #[serde(default = "from_round")]
    pub from_round: u32,
    /// How many times a cheap round that made no progress may hand the
    /// next round back to the strong executor.
    #[serde(default = "one")]
    pub max_escalations: u32,
    /// The strong executor. The one that produced the candidate when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strong: Option<Tier>,
}

/// The rounds' spending cap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpendCap {
    /// The task's budget in US dollars when the manifest sets no soft
    /// spend ceiling. The soft ceiling wins when both are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    /// The share of the budget left when the rounds start that they may
    /// spend together.
    #[serde(default = "half")]
    pub share: f64,
}

impl OwnTests {
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.runner.trim().is_empty() {
            problems.push("control.persist.own_tests.runner must name a file".to_string());
        }
        if self.timeout_sec == 0 || self.timeout_sec > 3_600 {
            problems.push("control.persist.own_tests.timeout_sec must be 1 to 3600".to_string());
        }
        problems
    }
}

impl CheapRounds {
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.tiers.is_empty() {
            problems.push("control.persist.cheap.tiers must name an executor".to_string());
        }
        for (i, tier) in self.tiers.iter().enumerate() {
            problems.extend(tier.validate(&format!("control.persist.cheap.tiers[{i}]")));
        }
        if let Some(strong) = &self.strong {
            problems.extend(strong.validate("control.persist.cheap.strong"));
        }
        if self.from_round == 0 {
            problems.push("control.persist.cheap.from_round must be 1 or more".to_string());
        }
        problems
    }
}

impl SpendCap {
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(self.share > 0.0 && self.share <= 1.0) {
            problems.push("control.persist.spend.share must be above 0 and at most 1".to_string());
        }
        if self.budget_usd.is_some_and(|b| b.is_nan() || b <= 0.0) {
            problems.push("control.persist.spend.budget_usd must be above 0".to_string());
        }
        problems
    }

    /// What the rounds may spend together, given the manifest's soft
    /// ceiling and what the episode spent before them. `None` when there
    /// is no budget to take a share of.
    #[must_use]
    pub fn cap(&self, soft_usd: Option<f64>, spent_before: f64) -> Option<f64> {
        let budget = soft_usd.or(self.budget_usd)?;
        Some(((budget - spent_before).max(0.0)) * self.share)
    }
}

/// Why no further round may start under the spending cap, or `None`.
/// `estimate` is what the next round is expected to cost: the last round
/// on the same kind of executor, when one ran.
#[must_use]
pub fn over_cap(cap: Option<f64>, spent: f64, estimate: Option<f64>) -> Option<String> {
    let cap = cap?;
    if spent >= cap {
        return Some(format!(
            "the rounds spent ${spent:.2} of their ${cap:.2} cap"
        ));
    }
    match estimate {
        Some(next) if spent + next > cap => Some(format!(
            "the next round would cost about ${next:.2}, over the ${:.2} left of the ${cap:.2} cap",
            cap - spent
        )),
        _ => None,
    }
}

/// One run of the executor's own tests.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TestRun {
    /// Each test's name and whether it passed.
    pub results: BTreeMap<String, bool>,
    /// The runner's exit code; `None` when it didn't exit by itself.
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub milliseconds: u64,
}

impl TestRun {
    /// Reads a runner's output: one `PASS <name>` or `FAIL <name>` line
    /// per test. Other lines are ignored, and a later line for the same
    /// name replaces an earlier one.
    #[must_use]
    pub fn parse(stdout: &str, exit_code: Option<i32>) -> TestRun {
        let mut results = BTreeMap::new();
        for line in stdout.lines() {
            let line = line.trim();
            let (passed, rest) = if let Some(rest) = line.strip_prefix("PASS") {
                (true, rest)
            } else if let Some(rest) = line.strip_prefix("FAIL") {
                (false, rest)
            } else {
                continue;
            };
            let name = rest.trim_start_matches(':').trim();
            if !name.is_empty() && rest.starts_with([' ', ':', '\t']) {
                results.insert(name.to_string(), passed);
            }
        }
        TestRun {
            results,
            exit_code,
            timed_out: false,
            milliseconds: 0,
        }
    }

    #[must_use]
    pub fn passed(&self) -> usize {
        self.results.values().filter(|p| **p).count()
    }

    #[must_use]
    pub fn failed(&self) -> usize {
        self.results.values().filter(|p| !**p).count()
    }

    /// The failing tests' names, in order.
    #[must_use]
    pub fn failing(&self) -> Vec<&str> {
        self.results
            .iter()
            .filter(|(_, p)| !**p)
            .map(|(n, _)| n.as_str())
            .collect()
    }

    /// The run as a record: counts, the exit code, and the failing names.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({
            "passed": self.passed(),
            "failed": self.failed(),
            "failing": self.failing().iter().take(20).collect::<Vec<_>>(),
            "exit_code": self.exit_code,
            "timed_out": self.timed_out,
            "milliseconds": self.milliseconds,
        })
    }
}

/// Runs the executor's own tests, or returns `None` when the runner
/// isn't there.
pub async fn run_own(tests: &OwnTests, workdir: &Path) -> Option<TestRun> {
    if !Path::new(&tests.runner).is_file() {
        return None;
    }
    let mut command = std::process::Command::new("bash");
    command
        .arg(&tests.runner)
        .current_dir(workdir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("PAGER", "cat");
    let started = std::time::Instant::now();
    let ended = supervise::Job::from_command(command)
        .bounded(
            supervise::Limits::within(Duration::from_secs(tests.timeout_sec)).keeping(256 * 1024),
        )
        .run()
        .await;
    let (exit_code, timed_out) = match ended.ending {
        supervise::Ending::Exited(code) => (code, false),
        supervise::Ending::TimedOut => (None, true),
        supervise::Ending::Failed(_) => (None, false),
    };
    let mut run = TestRun::parse(&ended.stdout.text, exit_code);
    run.timed_out = timed_out;
    run.milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Some(run)
}

/// What one round changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Delta {
    /// Own tests that failed after the previous round and pass now.
    pub tests_fixed: usize,
    /// Own tests that passed after the previous round and fail now.
    pub tests_broken: usize,
    /// Own tests that didn't exist after the previous round.
    pub tests_added: usize,
    /// Failed scenarios and contradicted requirements before and after.
    pub checks_bad_before: Option<usize>,
    pub checks_bad_after: Option<usize>,
    /// Confirmed requirements before and after.
    pub confirmed_before: Option<usize>,
    pub confirmed_after: Option<usize>,
    /// Whether the round made progress: `Some(true)` when a test was fixed
    /// or the checks came out better; `Some(false)` when there was
    /// something to compare and nothing got better, or the round changed
    /// no file; `None` when the round only set the tests' baseline.
    pub progress: Option<bool>,
    /// Under `judge: "checks"`: what the round resolved and regressed,
    /// scenario by scenario.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resolved: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub regressed: Vec<String>,
}

impl Delta {
    /// The delta of one round. `prev` is the own-tests run after the
    /// previous round and `now` the run after this one; `before` and
    /// `after` are the checks' standings; `changed` says whether the round
    /// changed a file.
    #[must_use]
    pub fn of(
        prev: Option<&TestRun>,
        now: Option<&TestRun>,
        before: Option<&Standing>,
        after: Option<&Standing>,
        changed: bool,
    ) -> Delta {
        let bad = |s: &Standing| s.failed + s.contradicted;
        let mut delta = Delta {
            checks_bad_before: before.map(bad),
            checks_bad_after: after.map(bad),
            confirmed_before: before.map(|s| s.confirmed),
            confirmed_after: after.map(|s| s.confirmed),
            ..Delta::default()
        };
        if let Some(now) = now {
            let empty = BTreeMap::new();
            let earlier = prev.map_or(&empty, |p| &p.results);
            for (name, passed) in &now.results {
                match earlier.get(name) {
                    Some(false) if *passed => delta.tests_fixed += 1,
                    Some(true) if !*passed => delta.tests_broken += 1,
                    None => delta.tests_added += 1,
                    _ => {}
                }
            }
        }
        let checks_better = match (before, after) {
            (Some(b), Some(a)) => bad(a) < bad(b) || a.confirmed > b.confirmed,
            _ => false,
        };
        let tests_better = delta.tests_fixed > delta.tests_broken;
        delta.progress = if !changed {
            Some(false)
        } else if checks_better || tests_better {
            Some(true)
        } else if prev.is_none() && now.is_some_and(|n| !n.results.is_empty()) {
            // The round wrote the first tests: there is nothing to compare
            // them with yet.
            None
        } else {
            Some(false)
        };
        delta
    }

    /// Whether the round left more broken than it fixed: more own tests
    /// broken than fixed, with the checks no better.
    #[must_use]
    pub fn tests_worse(&self) -> bool {
        let checks_better = match (self.checks_bad_before, self.checks_bad_after) {
            (Some(b), Some(a)) => a < b,
            _ => false,
        };
        self.tests_broken > self.tests_fixed && !checks_better
    }
}

/// Which kind of executor a round runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    /// The executor that produced the candidate, or `cheap.strong`.
    Strong,
    /// One of `cheap.tiers`.
    Cheap,
    /// The strong executor, after a cheap round made no progress.
    Escalated,
}

/// What happens after a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    /// Another round may start, on the ladder's next class.
    Continue,
    /// The next round runs the strong executor.
    Escalate,
    /// No further round starts, and why.
    Stop(String),
}

/// The executor ladder across rounds.
#[derive(Clone, Debug, Default)]
pub struct Ladder {
    /// Cheap rounds run so far, which picks the next cheap tier.
    pub cheap_runs: usize,
    /// Escalations used so far.
    pub escalations: u32,
    /// Whether the next round escalates.
    pub escalate_next: bool,
}

impl Ladder {
    /// The class round `n` runs.
    #[must_use]
    pub fn class_for(&self, n: u32, cheap: Option<&CheapRounds>) -> Class {
        match cheap {
            Some(_) if self.escalate_next => Class::Escalated,
            Some(c) if n >= c.from_round => Class::Cheap,
            _ => Class::Strong,
        }
    }

    /// The cheap tier the next cheap round runs.
    #[must_use]
    pub fn cheap_tier(&self, cheap: &CheapRounds) -> Tier {
        cheap.tiers[self.cheap_runs % cheap.tiers.len()].clone()
    }

    /// Takes back the escalation `next` granted, when the rounds stop
    /// instead.
    pub fn cancel(&mut self, next: Option<&Next>) {
        if next == Some(&Next::Escalate) {
            self.escalations = self.escalations.saturating_sub(1);
            self.escalate_next = false;
        }
    }

    /// Records that a round of `class` ran.
    pub fn ran(&mut self, class: Class) {
        match class {
            Class::Cheap => self.cheap_runs += 1,
            Class::Escalated => self.escalate_next = false,
            Class::Strong => {}
        }
    }
}

/// What follows round `n`, which ran `class` and made `progress`.
///
/// Without `cheap`, a round without progress stops the rounds when
/// `stop_when_no_progress` is set. With `cheap`, a cheap round without
/// progress hands the next round to the strong executor while escalations
/// are left, and a strong round without progress stops the rounds.
#[must_use]
pub fn after_round(
    n: u32,
    class: Class,
    progress: Option<bool>,
    stop_when_no_progress: bool,
    cheap: Option<&CheapRounds>,
    ladder: &mut Ladder,
) -> Next {
    if progress != Some(false) {
        return Next::Continue;
    }
    match (cheap, class) {
        (Some(c), Class::Cheap) if ladder.escalations < c.max_escalations => {
            ladder.escalations += 1;
            ladder.escalate_next = true;
            Next::Escalate
        }
        (Some(_), Class::Cheap) => Next::Stop(format!(
            "round {n} on a cheap executor made no progress, and no escalation is left"
        )),
        (Some(_), _) => Next::Stop(format!("round {n} on the strong executor made no progress")),
        (None, _) if stop_when_no_progress => {
            Next::Stop(format!("round {n} changed no test or check outcome"))
        }
        (None, _) => Next::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing(failed: usize, confirmed: usize) -> Standing {
        Standing {
            failed,
            failed_checks: failed,
            self_reported: 0,
            contradicted: 0,
            confirmed,
            passed_scenarios: 0,
            unresolved: 0,
        }
    }

    fn run(pairs: &[(&str, bool)]) -> TestRun {
        TestRun {
            results: pairs.iter().map(|(n, p)| ((*n).to_string(), *p)).collect(),
            ..TestRun::default()
        }
    }

    fn cheap() -> CheapRounds {
        CheapRounds {
            tiers: vec![
                Tier::new("codex", "gpt-6-sol"),
                Tier::new("codex", "gpt-6-luna"),
            ],
            from_round: 2,
            max_escalations: 1,
            strong: None,
        }
    }

    #[test]
    fn a_runner_prints_one_line_per_test() {
        let out = "building...\nPASS parses header\nFAIL: totals match\nPASSED nothing\n\
                   FAIL totals match\nPASS  edge case  \nFAIL\n";
        let run = TestRun::parse(out, Some(1));
        assert_eq!(run.passed(), 2);
        assert_eq!(run.failed(), 1);
        assert_eq!(run.failing(), ["totals match"]);
        assert!(run.results.contains_key("edge case"));
        assert_eq!(run.record()["exit_code"], 1);
    }

    #[test]
    fn a_round_that_fixes_a_test_progresses_and_one_that_fixes_none_does_not() {
        let prev = run(&[("a", false), ("b", false), ("c", true)]);
        let now = run(&[("a", true), ("b", false), ("c", false), ("d", true)]);
        let s = standing(1, 2);
        let delta = Delta::of(Some(&prev), Some(&now), Some(&s), Some(&s), true);
        assert_eq!(
            (delta.tests_fixed, delta.tests_broken, delta.tests_added),
            (1, 1, 1)
        );
        // One fixed and one broken is no net progress.
        assert_eq!(delta.progress, Some(false));
        assert!(!delta.tests_worse());
        let now = run(&[("a", true), ("b", true), ("c", true)]);
        let delta = Delta::of(Some(&prev), Some(&now), Some(&s), Some(&s), true);
        assert_eq!(delta.tests_fixed, 2);
        assert_eq!(delta.progress, Some(true));
        // Nothing changed in the tests or the checks.
        let delta = Delta::of(Some(&prev), Some(&prev), Some(&s), Some(&s), true);
        assert_eq!(delta.progress, Some(false));
        // More broken than fixed, with the checks no better, is worse.
        let now = run(&[("a", false), ("b", false), ("c", false)]);
        assert!(Delta::of(Some(&prev), Some(&now), Some(&s), Some(&s), true).tests_worse());
    }

    #[test]
    fn the_checks_count_as_progress_and_the_first_tests_set_a_baseline() {
        let delta = Delta::of(
            None,
            None,
            Some(&standing(2, 1)),
            Some(&standing(1, 1)),
            true,
        );
        assert_eq!(delta.progress, Some(true));
        let delta = Delta::of(
            None,
            None,
            Some(&standing(1, 1)),
            Some(&standing(1, 2)),
            true,
        );
        assert_eq!(delta.progress, Some(true));
        // Neither the checks nor any tests moved.
        let delta = Delta::of(
            None,
            None,
            Some(&standing(1, 1)),
            Some(&standing(1, 1)),
            true,
        );
        assert_eq!(delta.progress, Some(false));
        // The round wrote the first tests: a baseline, not a verdict.
        let now = run(&[("a", false)]);
        let delta = Delta::of(
            None,
            Some(&now),
            Some(&standing(1, 1)),
            Some(&standing(1, 1)),
            true,
        );
        assert_eq!((delta.tests_added, delta.progress), (1, None));
        // A round that changed no file made no progress, whatever it says.
        let delta = Delta::of(None, Some(&now), None, None, false);
        assert_eq!(delta.progress, Some(false));
    }

    #[test]
    fn a_cheap_round_without_progress_escalates_once_then_stops() {
        let cheap = cheap();
        let mut ladder = Ladder::default();
        assert_eq!(ladder.class_for(1, Some(&cheap)), Class::Strong);
        ladder.ran(Class::Strong);
        assert_eq!(
            after_round(1, Class::Strong, None, true, Some(&cheap), &mut ladder),
            Next::Continue
        );
        // Round 2 is cheap and makes progress; round 3 is the next cheap
        // tier.
        assert_eq!(ladder.class_for(2, Some(&cheap)), Class::Cheap);
        assert_eq!(ladder.cheap_tier(&cheap).model, "gpt-6-sol");
        ladder.ran(Class::Cheap);
        assert_eq!(
            after_round(2, Class::Cheap, Some(true), true, Some(&cheap), &mut ladder),
            Next::Continue
        );
        assert_eq!(ladder.class_for(3, Some(&cheap)), Class::Cheap);
        assert_eq!(ladder.cheap_tier(&cheap).model, "gpt-6-luna");
        ladder.ran(Class::Cheap);
        // Round 3 makes none: round 4 escalates.
        assert_eq!(
            after_round(
                3,
                Class::Cheap,
                Some(false),
                true,
                Some(&cheap),
                &mut ladder
            ),
            Next::Escalate
        );
        assert_eq!(ladder.class_for(4, Some(&cheap)), Class::Escalated);
        ladder.ran(Class::Escalated);
        // The escalated round makes progress: back to cheap.
        assert_eq!(
            after_round(
                4,
                Class::Escalated,
                Some(true),
                true,
                Some(&cheap),
                &mut ladder
            ),
            Next::Continue
        );
        assert_eq!(ladder.class_for(5, Some(&cheap)), Class::Cheap);
        ladder.ran(Class::Cheap);
        // No escalation is left.
        assert_eq!(
            after_round(
                5,
                Class::Cheap,
                Some(false),
                true,
                Some(&cheap),
                &mut ladder
            ),
            Next::Stop(
                "round 5 on a cheap executor made no progress, and no escalation is left"
                    .to_string()
            )
        );
    }

    #[test]
    fn a_strong_round_without_progress_stops_the_rounds() {
        let cheap = cheap();
        let mut ladder = Ladder::default();
        assert_eq!(
            after_round(
                1,
                Class::Strong,
                Some(false),
                true,
                Some(&cheap),
                &mut ladder
            ),
            Next::Stop("round 1 on the strong executor made no progress".to_string())
        );
        let mut ladder = Ladder {
            escalate_next: true,
            escalations: 1,
            ..Ladder::default()
        };
        assert_eq!(
            after_round(
                3,
                Class::Escalated,
                Some(false),
                false,
                Some(&cheap),
                &mut ladder
            ),
            Next::Stop("round 3 on the strong executor made no progress".to_string())
        );
    }

    #[test]
    fn without_a_ladder_the_progress_rule_is_the_policys_choice() {
        let mut ladder = Ladder::default();
        assert_eq!(ladder.class_for(4, None), Class::Strong);
        assert_eq!(
            after_round(2, Class::Strong, Some(false), true, None, &mut ladder),
            Next::Stop("round 2 changed no test or check outcome".to_string())
        );
        assert_eq!(
            after_round(2, Class::Strong, Some(false), false, None, &mut ladder),
            Next::Continue
        );
        assert_eq!(
            after_round(2, Class::Strong, None, true, None, &mut ladder),
            Next::Continue
        );
    }

    #[test]
    fn the_rounds_spend_a_share_of_what_the_budget_has_left() {
        let cap = SpendCap {
            budget_usd: Some(10.0),
            share: 0.5,
        };
        // $4 spent before the rounds: they may spend half the $6 left.
        let limit = cap.cap(None, 4.0);
        assert_eq!(limit, Some(3.0));
        // The manifest's soft ceiling wins over the policy's budget.
        assert_eq!(cap.cap(Some(20.0), 4.0), Some(8.0));
        // Nothing left: nothing to spend.
        assert_eq!(cap.cap(None, 12.0), Some(0.0));
        assert_eq!(
            SpendCap {
                budget_usd: None,
                share: 0.5
            }
            .cap(None, 4.0),
            None
        );
        assert_eq!(over_cap(limit, 1.0, None), None);
        assert_eq!(over_cap(limit, 1.0, Some(1.5)), None);
        assert!(
            over_cap(limit, 1.0, Some(2.5))
                .unwrap()
                .contains("the next round would cost about $2.50")
        );
        assert!(over_cap(limit, 3.0, None).unwrap().contains("spent $3.00"));
        assert_eq!(over_cap(None, 100.0, Some(100.0)), None);
    }

    #[test]
    fn a_bad_ladder_or_cap_is_refused() {
        let mut bad = cheap();
        bad.tiers.clear();
        bad.from_round = 0;
        let problems = bad.validate().join("\n");
        assert!(problems.contains("tiers"), "{problems}");
        assert!(problems.contains("from_round"), "{problems}");
        let problems = SpendCap {
            budget_usd: Some(0.0),
            share: 0.0,
        }
        .validate()
        .join("\n");
        assert!(problems.contains("share") && problems.contains("budget_usd"));
        let problems = OwnTests {
            runner: " ".to_string(),
            timeout_sec: 0,
        }
        .validate()
        .join("\n");
        assert!(problems.contains("runner") && problems.contains("timeout_sec"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_host_runs_the_executors_runner() {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-own-tests-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let tests = OwnTests {
            runner: dir.join("run.sh").display().to_string(),
            timeout_sec: 10,
        };
        assert_eq!(run_own(&tests, &dir).await, None);
        std::fs::write(&tests.runner, "echo 'PASS one'\necho 'FAIL two'\nexit 1\n").unwrap();
        let ran = run_own(&tests, &dir).await.unwrap();
        assert_eq!((ran.passed(), ran.failed()), (1, 1));
        assert_eq!(ran.exit_code, Some(1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
