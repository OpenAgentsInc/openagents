//! Discrimination by authority class: does a class tell a task's passing
//! candidates from its failing ones (issue #9629)?
//!
//! Agreement with the verifier over many tasks can come from judging the
//! task rather than the candidate: the 2026-09-24 suites gave every trial
//! of 11 of 13 tasks the same call. A class that may hold the loop has to
//! do better inside one task, so [`measure`] counts, for each class and
//! only within groups that share one task and one suite and hold both a
//! passing and a failing candidate:
//!
//! - how many passing candidates the class keeps green, since a red here
//!   stops or reverses a correct fix;
//! - how many failing candidates it calls red, since a class that is never
//!   red can't stop anything;
//! - over every passing and failing pair in a group, how often the passing
//!   candidate has more of the class's tests green, which is what ranking
//!   candidates needs.
//!
//! Each is a count with a 95% Wilson interval ([`Rate`]). Candidates of a
//! task share one suite, so the effective sample is closer to the number
//! of groups than the number of candidates; the counts say both. A
//! retained candidate that is its trial's submitted workspace is left out,
//! since the trial's final row already counts it.
//!
//! Tasks are split by [`truth::split_of`], the digest-parity split fixed
//! before any signal was measured, and a task named as in-sample moves to
//! the calibration side. The bar ([`bar`]) is read on the held-out side
//! only, on workspaces the verifier graded.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use super::authority::{Authority, Record};
use super::offline::{Joined, Kind};
use crate::checks::truth::{self, Rate, Split};

/// The fewest held-out tasks with both a passing and a failing candidate
/// that a class needs before its interval can promote it.
pub const MIN_TASKS: usize = 2;

/// A holding class must keep at least this share of passing candidates
/// green, at the low end of its interval.
pub const KEEP_LOW: f64 = 0.8;

/// A holding class must call at least this share of failing candidates
/// red, at the low end of its interval.
pub const CATCH_LOW: f64 = 0.2;

/// A ranking class must order more pairs right than wrong: the low end of
/// its interval above one half.
pub const ORDER_LOW: f64 = 0.5;

/// One class's reading of one candidate: how many of its tests were green.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Call {
    pub green: usize,
    pub total: usize,
}

impl Call {
    fn fraction(self) -> f64 {
        self.green as f64 / self.total as f64
    }
    fn is_green(self) -> bool {
        self.green == self.total
    }
}

/// What `class` says about `row`, or `None` when the row's suite has no
/// test of that class.
#[must_use]
pub fn call(row: &Joined, record: &Record, class: Authority) -> Option<Call> {
    let mut out = Call { green: 0, total: 0 };
    for (id, green) in &row.tests {
        if record.class(&row.digest, id) == Some(class) {
            out.total += 1;
            out.green += usize::from(*green);
        }
    }
    (out.total > 0).then_some(out)
}

/// A retained candidate whose files are its trial's submitted workspace:
/// the same workspace as the trial's final row, so it isn't counted twice.
#[must_use]
pub fn duplicate(row: &Joined) -> bool {
    row.kind == Kind::Candidate && row.reward_source.as_deref() == Some("submitted")
}

/// The verifier graded this very workspace: a final workspace, a candidate
/// or reconstruction with a grade of its own, or a snapshot that every
/// check candidate shared.
#[must_use]
pub fn graded(row: &Joined) -> bool {
    match row.kind {
        Kind::Snapshot => row.snapshot_graded,
        Kind::Final | Kind::Candidate | Kind::Reconstruction => true,
    }
}

/// The split a task falls in: [`truth::split_of`], unless it is named in
/// `in_sample`.
#[must_use]
pub fn split(task: &str, in_sample: &[String]) -> Split {
    if in_sample.iter().any(|t| t == task) {
        Split::Calibration
    } else {
        truth::split_of(task)
    }
}

/// One class's counts over some rows.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Counts {
    /// Rows with a known reward whose suite has a test of the class.
    pub rows: usize,
    pub tasks: usize,
    /// Across tasks: green on a pass, red on a failure.
    pub green_right: Rate,
    pub red_right: Rate,
    /// Groups of one task and one suite holding both a pass and a failure.
    pub mixed_groups: usize,
    pub mixed_tasks: usize,
    /// Within those groups: passes kept green, failures called red.
    pub passes_green: Rate,
    pub failures_red: Rate,
    /// Pairs of a pass and a failure in one group: the pass had more of
    /// the class's tests green (concordant), fewer (discordant), or as
    /// many (tied). `order` is concordant of the untied.
    pub concordant: usize,
    pub discordant: usize,
    pub tied: usize,
    pub order: Rate,
    /// Groups where the class kept every pass green and called at least
    /// one failure red.
    pub separating_groups: usize,
}

/// `class`'s counts over `rows`.
#[must_use]
pub fn counts(rows: &[&Joined], record: &Record, class: Authority) -> Counts {
    let none = Rate::of(0, 0);
    let mut out = Counts {
        rows: 0,
        tasks: 0,
        green_right: none,
        red_right: none,
        mixed_groups: 0,
        mixed_tasks: 0,
        passes_green: none,
        failures_red: none,
        concordant: 0,
        discordant: 0,
        tied: 0,
        order: none,
        separating_groups: 0,
    };
    let mut groups: BTreeMap<(String, String), Vec<(bool, Call)>> = BTreeMap::new();
    let mut tasks = BTreeSet::new();
    let (mut green_calls, mut green_hits, mut red_calls, mut red_hits) = (0, 0, 0, 0);
    for row in rows {
        let (Some(reward), Some(c)) = (row.reward, call(row, record, class)) else {
            continue;
        };
        let passed = reward >= 1.0;
        out.rows += 1;
        tasks.insert(row.task.clone());
        if c.is_green() {
            green_calls += 1;
            green_hits += usize::from(passed);
        } else {
            red_calls += 1;
            red_hits += usize::from(!passed);
        }
        groups
            .entry((row.task.clone(), row.digest.clone()))
            .or_default()
            .push((passed, c));
    }
    out.tasks = tasks.len();
    out.green_right = Rate::of(green_hits, green_calls);
    out.red_right = Rate::of(red_hits, red_calls);
    let mut mixed_tasks = BTreeSet::new();
    let (mut passes, mut kept, mut failures, mut caught) = (0, 0, 0, 0);
    for ((task, _), members) in &groups {
        let pass: Vec<Call> = members.iter().filter(|m| m.0).map(|m| m.1).collect();
        let fail: Vec<Call> = members.iter().filter(|m| !m.0).map(|m| m.1).collect();
        if pass.is_empty() || fail.is_empty() {
            continue;
        }
        out.mixed_groups += 1;
        mixed_tasks.insert(task.clone());
        let kept_here = pass.iter().filter(|c| c.is_green()).count();
        let caught_here = fail.iter().filter(|c| !c.is_green()).count();
        passes += pass.len();
        kept += kept_here;
        failures += fail.len();
        caught += caught_here;
        if kept_here == pass.len() && caught_here > 0 {
            out.separating_groups += 1;
        }
        for p in &pass {
            for f in &fail {
                let (a, b) = (p.fraction(), f.fraction());
                if (a - b).abs() < 1e-12 {
                    out.tied += 1;
                } else if a > b {
                    out.concordant += 1;
                } else {
                    out.discordant += 1;
                }
            }
        }
    }
    out.mixed_tasks = mixed_tasks.len();
    out.passes_green = Rate::of(kept, passes);
    out.failures_red = Rate::of(caught, failures);
    out.order = Rate::of(out.concordant, out.concordant + out.discordant);
    out
}

/// Whether `class` passes the offline bar on `held_out`, and why not.
#[must_use]
pub fn bar(class: Authority, held_out: &Counts) -> (bool, String) {
    let enough = held_out.mixed_tasks >= MIN_TASKS;
    let low = |r: &Rate| r.low.unwrap_or(0.0);
    if class.can_hold() {
        let keeps = low(&held_out.passes_green) >= KEEP_LOW;
        let catches = low(&held_out.failures_red) >= CATCH_LOW;
        let why = if !enough {
            format!(
                "{} held-out task{} with both a pass and a failure; {MIN_TASKS} needed",
                held_out.mixed_tasks,
                if held_out.mixed_tasks == 1 { "" } else { "s" }
            )
        } else if !keeps {
            format!(
                "keeps passes green {}; the low end must reach {KEEP_LOW}",
                held_out.passes_green.text()
            )
        } else if !catches {
            format!(
                "calls failures red {}; the low end must reach {CATCH_LOW}",
                held_out.failures_red.text()
            )
        } else {
            "passes".to_string()
        };
        (enough && keeps && catches, why)
    } else if class == Authority::WriterDerived {
        let orders = low(&held_out.order) > ORDER_LOW;
        let why = if !enough {
            format!(
                "{} held-out task{} with both a pass and a failure; {MIN_TASKS} needed",
                held_out.mixed_tasks,
                if held_out.mixed_tasks == 1 { "" } else { "s" }
            )
        } else if !orders {
            format!(
                "orders pairs {}; the low end must pass {ORDER_LOW}",
                held_out.order.text()
            )
        } else {
            "passes".to_string()
        };
        (enough && orders, why)
    } else {
        (false, "this class never holds power".to_string())
    }
}

/// The measurement: for each set of workspaces (the ones the verifier
/// graded, and every one with a known reward) and each split, every
/// class's [`Counts`]; and each class's verdict on the bar, read on the
/// graded held-out side.
#[must_use]
pub fn measure(joined: &[Joined], record: &Record, in_sample: &[String]) -> Value {
    let mut sets = serde_json::Map::new();
    let mut verdicts = serde_json::Map::new();
    for (set, keep) in [
        ("graded", graded as fn(&Joined) -> bool),
        ("known_reward", |_: &Joined| true),
    ] {
        let mut splits = serde_json::Map::new();
        for (name, which) in [
            ("all", None),
            ("calibration", Some(Split::Calibration)),
            ("held_out", Some(Split::HeldOut)),
        ] {
            let rows: Vec<&Joined> = joined
                .iter()
                .filter(|j| {
                    keep(j) && !duplicate(j) && which.is_none_or(|s| split(&j.task, in_sample) == s)
                })
                .collect();
            let mut classes = serde_json::Map::new();
            for class in Authority::ALL {
                let c = counts(&rows, record, class);
                if set == "graded" && name == "held_out" {
                    let (passes, why) = bar(class, &c);
                    verdicts.insert(
                        class.word().to_string(),
                        json!({ "passes": passes, "why": why }),
                    );
                }
                classes.insert(class.word().to_string(), json!(c));
            }
            splits.insert(name.to_string(), Value::Object(classes));
        }
        sets.insert(set.to_string(), Value::Object(splits));
    }
    let mut tasks: BTreeMap<String, &str> = BTreeMap::new();
    for j in joined {
        tasks.insert(j.task.clone(), split(&j.task, in_sample).word());
    }
    json!({
        "bar": {
            "min_tasks": MIN_TASKS,
            "keep_low": KEEP_LOW,
            "catch_low": CATCH_LOW,
            "order_low": ORDER_LOW,
            "read_on": "graded workspaces of held-out tasks",
        },
        "splits": tasks,
        "in_sample": in_sample,
        "verdicts": verdicts,
        "sets": sets,
    })
}

/// The class calls per candidate, for reading the failures one by one:
/// each row's reward and, per class, its green and total tests and the
/// red test IDs.
#[must_use]
pub fn rows(joined: &[Joined], record: &Record) -> Vec<Value> {
    joined
        .iter()
        .filter(|j| record.suites.contains_key(&j.digest))
        .map(|j| {
            let mut classes = serde_json::Map::new();
            for class in Authority::ALL {
                let red: Vec<&String> = j
                    .tests
                    .iter()
                    .filter(|(id, green)| !green && record.class(&j.digest, id) == Some(class))
                    .map(|(id, _)| id)
                    .collect();
                if let Some(c) = call(j, record, class) {
                    classes.insert(
                        class.word().to_string(),
                        json!({ "green": c.green, "total": c.total, "red": red }),
                    );
                }
            }
            json!({
                "task": j.task,
                "suite": &j.digest[..j.digest.len().min(12)],
                "trial": j.trial,
                "kind": j.kind,
                "graded": graded(j),
                "reward": j.reward,
                "classes": classes,
            })
        })
        .collect()
}
