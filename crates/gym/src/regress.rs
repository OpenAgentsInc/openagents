//! Did this commit move the numbers?
//!
//! [`crate::ab`] compares two doors, and that is the right shape for "is this
//! adapter better than that one". Nothing here compared a door with *itself*
//! a week earlier, which is the question a repository asks every day. The
//! surfaces that quietly change a door's quality are all ordinary code — the
//! schema compiler, the renderer, the state prompt, the estimator, the
//! sampling parameters — and none of them has a test that could fail on a
//! lost point of accuracy. This repository runs no CI by policy, so until
//! this module there was no automatic guard at all.
//!
//! The case that names the gap is real. On 2026-09-19, `lev::schema` stopped
//! fencing the caller's state in `<state>` tags and gave it a plain `STATE`
//! label, because Apple's guardrails refuse delimiter-fenced state. The
//! reason is good, the commit is right, no test could have failed, and
//! nobody measured what it did to the panel. `docs/gym/regression.md` runs
//! that change through this module and reports what it would have said.
//!
//! # A query, not a measurement
//!
//! Every row already carries the suite digest, the question set's digest,
//! the door's identity, the estimator, the seed base, and the permutation.
//! So a regression check is a query: take the newest recorded run of one
//! door at one perturbation, take the run before it, and ask whether the
//! difference clears the floor.
//! Nothing is re-run where the rows exist, and the receipt chain means the
//! earlier numbers cannot have been quietly rewritten — which is exactly the
//! property a baseline needs and almost never has.
//!
//! # What it refuses
//!
//! A difference is a regression only when everything except the code was
//! held. [`Refusal`] names each way that fails, and each one is a refusal to
//! answer rather than a verdict. A changed suite is the important one: a
//! suite edit is a different measurement, not a worse door, and comparing
//! across digests is how a suite edit comes to read as a model result.
//! Reworded question text is refused for the neighbouring reason — it is a
//! candidate against the same items, which is what [`crate::store`] reads
//! as a question-text comparison and what `gym compare` judges.
//!
//! # Why a floor, when a rerun reproduces exactly
//!
//! These doors reproduce a seed block exactly, so a rerun at the same seed
//! base is not a fresh trial. Every difference it shows was caused by the
//! change; none of it is trial-to-trial noise. The floor answers the next
//! question instead: *how big is that difference compared with the
//! difference the seeds alone produce?*
//! `docs/lev/measurements/2026-09-19-seed-variance.md` measured that on this
//! suite — eight disjoint blocks over the same 98 items, everything else
//! fixed, gave a standard deviation of 0.0197 accuracy — and
//! [`crate::ab::Rule`] turns it into 0.056 at one block a side. A move
//! smaller than that was measured on one block and does not survive being
//! quoted: the next block could show it the other way.
//!
//! Counts are judged exactly, because a count is not an average. A refusal
//! or a confident error is one item, and at a fixed seed block the door
//! either declined that item or it did not.
//!
//! ECE, Brier, and log loss have no measured block-to-block spread, so their
//! criteria read the direction and refuse the size: a metric that did not
//! move the wrong way has held, and one that did is
//! [`Verdict::Unverifiable`] with the move printed rather than a loss
//! nobody can size. The floors are read from [`crate::ab::Rule::v2`] rather
//! than restated here, so openagents#9376 lands them in one place and this
//! module judges them without being edited.
//!
//! # The limit, which the command prints
//!
//! [`LIMIT`] travels in the report and on screen. This catches a regression
//! on our suite, in our domain, in English, against one author's labels. A
//! change that leaves `support-v2` untouched and breaks a real workload
//! passes it. That is an argument for openagents#9379 and openagents#9381,
//! and it is on the output because the output is the only place anyone will
//! read it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::ab::{Metric, OVERALL, Rule};
use crate::calibrate::score;
use crate::eval::observations;
use crate::gate::{Basis, Criterion, Scores, Verdict};
use crate::row::{DoorIdentity, Row};

/// The schema every regression report is tagged with.
pub const SCHEMA: &str = "openagents.gym.regression_report.v1";

/// What this command does not catch, in the words it is printed in.
///
/// It is a constant rather than a line in the renderer because it is part of
/// the report: a reader of the stored document is owed it as much as a
/// reader of the terminal.
pub const LIMIT: &str = "\
This catches a regression on one suite, in one domain, in English, against \
labels one author wrote. A change that leaves these items untouched and \
breaks a real workload passes it, and a green verdict is not a claim of \
safety. Scoring the workload that actually exists is openagents#9379, and \
validating the instrument itself is openagents#9381.";

/// The perturbation axes a comparison has to hold fixed.
///
/// These are the fields the store already keys a trial on, minus the item
/// and the door's identity: the identity is checked on its own, because a
/// door that changed underneath you is a different accusation from a run
/// that drew different seeds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Perturbation {
    /// The suite's name.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// The question set the run served, when it recorded one.
    pub question_set: Option<String>,
    /// That set's content digest. `None` for a row written before
    /// [`crate::questions`] existed, which is a fact about the row and not
    /// an unknown.
    pub question_digest: Option<String>,
    /// Which estimator produced the raw signal.
    pub estimator: String,
    /// How many draws one estimate rests on. `None` when the door does not
    /// report it, which is not the same as one draw.
    ///
    /// The store's own trial key leaves this out. It is here because an
    /// estimate over one draw and an estimate over eight are different
    /// estimates, and a cheaper rerun would otherwise read as a change in
    /// the code.
    pub samples: Option<u64>,
    /// The seed block the door drew. `None` for a door that takes no seed.
    pub seed_base: Option<u64>,
    /// The option order the items were served in. `None` is the suite's own
    /// order, which is a fact and not an unknown.
    pub permutation: Option<Vec<usize>>,
}

impl Perturbation {
    /// The perturbation one row was produced at.
    #[must_use]
    pub fn of(row: &Row) -> Self {
        Self {
            suite: row.suite.clone(),
            suite_digest: row.suite_digest.clone(),
            question_set: row.question_set.clone(),
            question_digest: row.question_digest.clone(),
            estimator: row.estimator.clone(),
            samples: row.samples,
            seed_base: row.seed_base,
            permutation: row.permutation.clone(),
        }
    }

    /// Whether two runs drew the same seeds in the same option order with
    /// the same estimator over the same number of draws.
    #[must_use]
    pub fn same_trial(&self, other: &Self) -> bool {
        self.estimator == other.estimator
            && self.samples == other.samples
            && self.seed_base == other.seed_base
            && self.permutation == other.permutation
    }
}

impl fmt::Display for Perturbation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "estimator `{}`, ", self.estimator)?;
        match self.samples {
            Some(draws) => write!(f, "{draws} draws, ")?,
            None => write!(f, "an unreported number of draws, ")?,
        }
        match self.seed_base {
            Some(block) => write!(f, "seed block {block}, ")?,
            None => write!(f, "no seed, ")?,
        }
        match &self.permutation {
            None => f.write_str("the suite's own option order")?,
            Some(order) => write!(f, "option order {order:?}")?,
        }
        match &self.question_set {
            None => f.write_str(", the suite's own question text"),
            Some(set) => write!(f, ", question set `{set}`"),
        }
    }
}

/// One door's rows from one recorded run at one perturbation.
///
/// A run writes one timestamp for every row it produces, so the timestamp is
/// what separates this week's rows from last week's. A run that records two
/// option orders produces two panels, because the reversed pass measures
/// order sensitivity rather than reading the item a second time.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    /// The door, by the name the run used for it.
    pub door: String,
    /// What that door was running, as far as it can be verified.
    pub identity: DoorIdentity,
    /// When the run recorded these rows.
    pub recorded_at: String,
    /// What was held fixed.
    pub perturbation: Perturbation,
    /// The rows, in store order.
    pub rows: Vec<Row>,
}

impl Panel {
    /// The families these rows cover, in first-seen order.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for row in &self.rows {
            if !out.contains(&row.family) {
                out.push(row.family.clone());
            }
        }
        out
    }

    /// The rows of one group: one family, or every row when the group is
    /// [`OVERALL`].
    #[must_use]
    pub fn group(&self, group: &str) -> Vec<Row> {
        self.rows
            .iter()
            .filter(|row| group == OVERALL || row.family == group)
            .cloned()
            .collect()
    }

    /// Every item this run recorded a row for, in one group.
    ///
    /// An item the harness lost produces no row, so it is not here and it is
    /// in neither denominator. That is why two runs have to carry the same
    /// set before anything below is judged.
    #[must_use]
    pub fn asked(&self, group: &str) -> BTreeSet<String> {
        self.group(group).iter().map(|row| row.item_id.clone()).collect()
    }

    /// Every item the door answered in one group.
    #[must_use]
    pub fn answered(&self, group: &str) -> BTreeSet<String> {
        self.group(group)
            .iter()
            .filter(|row| row.is_scored())
            .map(|row| row.item_id.clone())
            .collect()
    }

    /// One group's numbers.
    #[must_use]
    pub fn measure(&self, group: &str) -> Measure {
        let rows = self.group(group);
        let answered = rows.iter().filter(|row| row.is_scored()).count();
        let refused = rows.iter().filter(|row| row.is_refused()).count();
        // An empty set of observations scores as zeros, and a zero accuracy
        // over nothing is a measurement nobody made. Unknown stays unknown.
        let scores = if answered == 0 {
            Scores::default()
        } else {
            score(&observations(&rows)).scores()
        };
        Measure {
            asked: rows.len(),
            answered,
            refused,
            refusals: crate::eval::refusals(&rows),
            scores,
        }
    }
}

/// What one group of one run produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measure {
    /// How many items the run recorded a row for. An item the harness lost
    /// is not one of them.
    pub asked: usize,
    /// How many of them the door answered.
    pub answered: usize,
    /// How many of them the door declined.
    pub refused: usize,
    /// How many rows each refusal code accounts for.
    pub refusals: BTreeMap<String, usize>,
    /// The panel of measures over the answered items. Every field is
    /// `None` when the door answered nothing.
    pub scores: Scores,
}

/// Why a comparison was not made.
///
/// Each of these is a refusal to answer rather than a verdict. A door that
/// cannot be compared has not passed and has not regressed, and rendering
/// one as the other is the mistake the three-valued verdict exists to stop.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum Refusal {
    /// The store holds one run of this door and nothing to compare it with.
    #[error(
        "`{door}` has one recorded run, from {recorded_at}, so there is nothing to compare it \
         against. Record a second run at the same suite digest and the same seed block, in \
         another store, and pass it as --against"
    )]
    OneRun {
        /// The door.
        door: String,
        /// When its only run was recorded.
        recorded_at: String,
    },
    /// The earlier run answered a different suite.
    #[error(
        "`{door}` last ran against `{before_suite}` at digest {before_digest}, and this run \
         answered `{after_suite}` at digest {after_digest}. A changed suite is a different \
         measurement, not a regression, and comparing across digests is how a suite edit comes \
         to read as a model result"
    )]
    SuiteMoved {
        /// The door.
        door: String,
        /// The suite the earlier run answered.
        before_suite: String,
        /// That suite's digest.
        before_digest: String,
        /// The suite this run answered.
        after_suite: String,
        /// This suite's digest.
        after_digest: String,
    },
    /// The two runs served different question text.
    #[error(
        "`{door}` last ran under {before} and this run served {after}. Rewording a question \
         makes a candidate against the same items rather than a regression in the door, and \
         `gym compare` reads two question sets as what they are"
    )]
    QuestionsMoved {
        /// The door.
        door: String,
        /// The question set the earlier run served.
        before: String,
        /// The question set this run served.
        after: String,
    },
    /// The door itself is no longer the same door.
    #[error(
        "`{door}` is not the door that produced the earlier run: {difference}. That is two \
         doors, and `gym compare` is the command that compares two doors"
    )]
    DoorMoved {
        /// The door.
        door: String,
        /// Which identity field moved, and to what.
        difference: String,
    },
    /// The two runs did not hold the perturbation fixed.
    #[error(
        "`{door}` last ran at {before}, and this run drew {after}. A different seed block or a \
         different option order is a different trial, so the difference between them is not \
         this commit's"
    )]
    PerturbationMoved {
        /// The door.
        door: String,
        /// What the earlier run drew.
        before: String,
        /// What this run drew.
        after: String,
    },
}

impl Refusal {
    /// The door the refusal is about.
    #[must_use]
    pub fn door(&self) -> &str {
        match self {
            Self::OneRun { door, .. }
            | Self::SuiteMoved { door, .. }
            | Self::QuestionsMoved { door, .. }
            | Self::DoorMoved { door, .. }
            | Self::PerturbationMoved { door, .. } => door,
        }
    }
}

/// One group's movement between the two runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Movement {
    /// The family, or [`OVERALL`].
    pub group: String,
    /// What the earlier run produced.
    pub before: Measure,
    /// What this run produced.
    pub after: Measure,
    /// `failed` beats `unverifiable` beats `passed`.
    pub verdict: Verdict,
    /// Every criterion, in rank order.
    pub criteria: Vec<Criterion>,
}

impl Movement {
    /// The highest-ranked criterion that carries this group's verdict.
    #[must_use]
    pub fn deciding(&self) -> Option<&Criterion> {
        self.criteria
            .iter()
            .filter(|criterion| criterion.verdict == self.verdict)
            .min_by_key(|criterion| criterion.rank)
    }

    /// The one line to print beside this group's verdict.
    ///
    /// A verdict that did not pass names the criterion that decided it,
    /// because that is what a reader has to act on. A verdict that passed
    /// names the accuracy instead: "passed: both runs asked the same 40
    /// items" is true and says nothing about whether the door still works.
    #[must_use]
    pub fn headline(&self) -> Option<&Criterion> {
        if self.verdict != Verdict::Passed {
            return self.deciding();
        }
        self.criteria
            .iter()
            .find(|criterion| criterion.name == accuracy_criterion())
            .or_else(|| self.deciding())
    }
}

/// The name of the criterion a passing group leads with.
fn accuracy_criterion() -> String {
    format!("{}_holds_within_the_noise", Metric::Accuracy)
}

/// One door, compared with itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// What this document is.
    pub schema: String,
    /// The door.
    pub door: String,
    /// What it was running. The same on both sides, or there is no report.
    pub identity: DoorIdentity,
    /// What both runs held fixed.
    pub perturbation: Perturbation,
    /// When the earlier run was recorded.
    pub before: String,
    /// When this run was recorded.
    pub after: String,
    /// The rule the floors came from.
    pub rule_id: String,
    /// That rule's content digest, so the bar is pinned to a rule a reader
    /// can look up rather than to a number in this source.
    pub rule_digest: String,
    /// `failed` beats `unverifiable` beats `passed`, across groups.
    pub verdict: Verdict,
    /// The whole suite, then each family.
    pub groups: Vec<Movement>,
    /// What this comparison does not catch. Always [`LIMIT`].
    pub limit: String,
}

impl Report {
    /// The groups that kept the report from passing.
    pub fn breaches(&self) -> impl Iterator<Item = &Movement> {
        self.groups.iter().filter(|group| group.verdict != Verdict::Passed)
    }

    /// The whole suite's movement.
    #[must_use]
    pub fn overall(&self) -> Option<&Movement> {
        self.groups.iter().find(|group| group.group == OVERALL)
    }
}

/// What the command found about one door.
#[derive(Clone, Debug, PartialEq)]
pub enum Finding {
    /// The two runs were comparable, and here is the comparison.
    Compared(Box<Report>),
    /// They were not, and here is why.
    Refused(Refusal),
}

impl Finding {
    /// The door this finding is about.
    #[must_use]
    pub fn door(&self) -> &str {
        match self {
            Self::Compared(report) => &report.door,
            Self::Refused(refusal) => refusal.door(),
        }
    }

    /// The verdict, when one was reached. A refusal has none: it did not
    /// pass, and it did not fail.
    #[must_use]
    pub fn verdict(&self) -> Option<Verdict> {
        match self {
            Self::Compared(report) => Some(report.verdict),
            Self::Refused(_) => None,
        }
    }
}

/// Every run in a set of rows, grouped by door, timestamp, and perturbation.
///
/// Ordered by door, then by the time the run was recorded, oldest first.
#[must_use]
pub fn panels(rows: &[Row]) -> Vec<Panel> {
    let mut panels: Vec<Panel> = Vec::new();
    for row in rows {
        let perturbation = Perturbation::of(row);
        let found = panels.iter_mut().find(|panel| {
            panel.door == row.door
                && panel.recorded_at == row.recorded_at
                && panel.identity == row.door_identity
                && panel.perturbation == perturbation
        });
        match found {
            Some(panel) => panel.rows.push(row.clone()),
            None => panels.push(Panel {
                door: row.door.clone(),
                identity: row.door_identity.clone(),
                recorded_at: row.recorded_at.clone(),
                perturbation,
                rows: vec![row.clone()],
            }),
        }
    }
    panels.sort_by(|left, right| {
        left.door
            .cmp(&right.door)
            .then_with(|| left.recorded_at.cmp(&right.recorded_at))
    });
    panels
}

/// Compares each door's newest run with its own previous one.
///
/// `earlier` holds the rows the newest run is measured against. Pass `None`
/// to take both runs from `latest`, which is what a single store holding a
/// door's history supports.
///
/// One finding per door in `latest`, ordered by door. A door that cannot be
/// compared produces [`Finding::Refused`] rather than dropping out, because
/// a door that quietly disappears from a pre-push check is worse than no
/// check.
#[must_use]
pub fn review(earlier: Option<&[Row]>, latest: &[Row], rule: &Rule) -> Vec<Finding> {
    let current = panels(latest);
    let baseline = earlier.map(panels);
    let mut doors: Vec<String> = Vec::new();
    for panel in &current {
        if !doors.contains(&panel.door) {
            doors.push(panel.door.clone());
        }
    }
    doors.sort();
    doors
        .into_iter()
        .filter_map(|door| {
            let mine: Vec<&Panel> =
                current.iter().filter(|panel| panel.door == door).collect();
            let after = *mine.last()?;
            let candidates: Vec<&Panel> = match &baseline {
                Some(panels) => panels.iter().filter(|panel| panel.door == door).collect(),
                None => mine[..mine.len() - 1].to_vec(),
            };
            Some(match pick(&candidates, after) {
                Ok(before) => Finding::Compared(Box::new(compare(before, after, rule))),
                Err(refusal) => Finding::Refused(refusal),
            })
        })
        .collect()
}

/// The newest earlier run this one may be compared with, or why none is.
fn pick<'a>(candidates: &[&'a Panel], after: &Panel) -> Result<&'a Panel, Refusal> {
    let comparable = candidates.iter().rev().find(|before| {
        before.perturbation.suite_digest == after.perturbation.suite_digest
            && before.perturbation.question_digest == after.perturbation.question_digest
            && before.identity == after.identity
            && before.perturbation.same_trial(&after.perturbation)
    });
    if let Some(before) = comparable {
        return Ok(before);
    }
    // Name the newest run that was not comparable, and the first reason it
    // was not. A reader who is told "the suite moved" can act; a reader who
    // is told "no match" cannot.
    let Some(before) = candidates.last() else {
        return Err(Refusal::OneRun {
            door: after.door.clone(),
            recorded_at: after.recorded_at.clone(),
        });
    };
    if before.perturbation.suite_digest != after.perturbation.suite_digest {
        return Err(Refusal::SuiteMoved {
            door: after.door.clone(),
            before_suite: before.perturbation.suite.clone(),
            before_digest: short(&before.perturbation.suite_digest),
            after_suite: after.perturbation.suite.clone(),
            after_digest: short(&after.perturbation.suite_digest),
        });
    }
    if before.perturbation.question_digest != after.perturbation.question_digest {
        return Err(Refusal::QuestionsMoved {
            door: after.door.clone(),
            before: name_questions(&before.perturbation),
            after: name_questions(&after.perturbation),
        });
    }
    if before.identity != after.identity {
        return Err(Refusal::DoorMoved {
            door: after.door.clone(),
            difference: identity_difference(&before.identity, &after.identity),
        });
    }
    Err(Refusal::PerturbationMoved {
        door: after.door.clone(),
        before: before.perturbation.to_string(),
        after: after.perturbation.to_string(),
    })
}

/// Compares two runs of one door, group by group.
///
/// The caller has already established that the two are comparable. This
/// function measures and judges; it does not re-run anything.
#[must_use]
pub fn compare(before: &Panel, after: &Panel, rule: &Rule) -> Report {
    let mut groups = vec![movement(OVERALL, before, after, rule)];
    // Every family either run saw, so a family that vanished is visible
    // rather than absent.
    let mut families = before.families();
    for family in after.families() {
        if !families.contains(&family) {
            families.push(family);
        }
    }
    for family in families {
        groups.push(movement(&family, before, after, rule));
    }
    Report {
        schema: SCHEMA.to_string(),
        door: after.door.clone(),
        identity: after.identity.clone(),
        perturbation: after.perturbation.clone(),
        before: before.recorded_at.clone(),
        after: after.recorded_at.clone(),
        rule_id: rule.id.clone(),
        rule_digest: rule.digest(),
        verdict: Verdict::over(groups.iter().map(|group| group.verdict)),
        groups,
        limit: LIMIT.to_string(),
    }
}

fn movement(group: &str, before: &Panel, after: &Panel, rule: &Rule) -> Movement {
    let measured = (before.measure(group), after.measure(group));
    let criteria = judge(group, before, after, &measured, rule);
    Movement {
        group: group.to_string(),
        before: measured.0,
        after: measured.1,
        verdict: Verdict::over(criteria.iter().map(|criterion| criterion.verdict)),
        criteria,
    }
}

fn judge(
    group: &str,
    before: &Panel,
    after: &Panel,
    measured: &(Measure, Measure),
    rule: &Rule,
) -> Vec<Criterion> {
    let (was, now) = measured;
    let mut criteria = Vec::new();
    let (asked, blocked) = same_items(group, before, after);
    criteria.push(asked);
    criteria.push(answered(group, before, after, was, now, blocked.as_deref()));
    for floor in &rule.metric_order {
        criteria.push(metric_holds(
            floor.metric,
            group,
            rule,
            was,
            now,
            blocked.as_deref(),
        ));
    }
    criteria.push(confident_errors(was, now, blocked.as_deref()));
    criteria
}

/// The two runs asked the same items, or nothing below is comparable.
fn same_items(group: &str, before: &Panel, after: &Panel) -> (Criterion, Option<String>) {
    let name = "the_same_items_were_asked";
    let (was, now) = (before.asked(group), after.asked(group));
    if was == now {
        return (
            passed(
                name,
                1,
                format!("both runs asked the same {} items of `{group}`", now.len()),
            ),
            None,
        );
    }
    let gone: Vec<&String> = was.difference(&now).collect();
    let fresh: Vec<&String> = now.difference(&was).collect();
    let reason = format!(
        "the earlier run asked {} items of `{group}` and this one asked {}: {} dropped{} and {} \
         added{}",
        was.len(),
        now.len(),
        gone.len(),
        name_a_few(&gone),
        fresh.len(),
        name_a_few(&fresh),
    );
    (
        Criterion {
            name: name.to_string(),
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: format!("{reason}. Two runs over different items are two measurements"),
        },
        Some(reason),
    )
}

/// The door answered the items it answered before.
///
/// This is the criterion the state-prompt change would have moved. A door
/// whose guardrails begin firing loses items out of the numerator while they
/// stay in the denominator, and the accuracy below is then an average over a
/// different set of answers.
fn answered(
    group: &str,
    before: &Panel,
    after: &Panel,
    was: &Measure,
    now: &Measure,
    blocked: Option<&str>,
) -> Criterion {
    let name = "the_door_answered_the_same_items";
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let (had, has) = (before.answered(group), after.answered(group));
    let lost: Vec<&String> = had.difference(&has).collect();
    let gained: Vec<&String> = has.difference(&had).collect();
    let codes = |measure: &Measure| -> String {
        if measure.refusals.is_empty() {
            "none".to_string()
        } else {
            measure
                .refusals
                .iter()
                .map(|(code, count)| format!("`{code}` x{count}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let refusals = format!(
        "refusals {} to {} ({} to {})",
        was.refused,
        now.refused,
        codes(was),
        codes(now)
    );
    if !lost.is_empty() {
        return Criterion {
            name: name.to_string(),
            rank: 1,
            verdict: Verdict::Failed,
            detail: format!(
                "the door declined {} items it answered before{}; {refusals}. A declined item \
                 leaves the numerator and stays in the denominator, so the measures below are \
                 no longer over the same answers",
                lost.len(),
                name_a_few(&lost),
            ),
        };
    }
    if !gained.is_empty() {
        return passed(
            name,
            1,
            format!(
                "the door answered {} items it declined before{}; {refusals}",
                gained.len(),
                name_a_few(&gained),
            ),
        );
    }
    passed(
        name,
        1,
        format!("the door answered the same {} items; {refusals}", has.len()),
    )
}

/// One metric moved by less than the seed blocks move it, or it did not.
fn metric_holds(
    metric: Metric,
    group: &str,
    rule: &Rule,
    was: &Measure,
    now: &Measure,
    blocked: Option<&str>,
) -> Criterion {
    let name = format!("{metric}_holds_within_the_noise");
    let rank = if metric == Metric::Accuracy { 1 } else { 2 };
    if let Some(reason) = blocked {
        return not_judged(&name, rank, reason);
    }
    let (Some(before), Some(after)) = (metric.read(&was.scores), metric.read(&now.scores)) else {
        return Criterion {
            name,
            rank,
            verdict: Verdict::Unverifiable,
            detail: format!("{metric} was not measured on both sides"),
        };
    };
    let gain = metric.gain(before, after);
    let moved = format!("{metric} {before:.3} to {after:.3}, a move of {gain:+.3}");
    // One recorded run is one seed block a side. A door that records several
    // blocks in one run is not something this command has met, and inventing
    // a count here would lower a bar nobody measured.
    let floor = if group == OVERALL {
        rule.effect_size(metric, 1, 1)
    } else {
        rule.family_allowance(metric, 1, 1)
    };
    let Some(floor) = floor else {
        // No floor, so the size of an adverse move cannot be read. The
        // direction still can: a metric that did not move the wrong way has
        // held, and one that did is reported without being called a loss.
        if gain >= 0.0 {
            return passed(
                &name,
                rank,
                format!(
                    "{moved}, which is not the wrong way. Nothing has measured this suite's \
                     block-to-block spread of {metric}, so the size of a move in it is not \
                     judged here"
                ),
            );
        }
        return Criterion {
            name,
            rank,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "{moved}. Nothing has measured this suite's block-to-block spread of {metric}, \
                 so whether a loss of that size means anything cannot be told"
            ),
        };
    };
    let against = if group == OVERALL {
        format!("against a floor of {floor:.3}")
    } else {
        format!(
            "against a floor of {floor:.3}, which is the suite's measured spread standing in \
             for this family's, which nobody has measured"
        )
    };
    if -gain > floor {
        return Criterion {
            name,
            rank,
            verdict: Verdict::Failed,
            detail: format!("{moved} {against}: the change lost more than the seeds do"),
        };
    }
    let reading = if gain > floor {
        "the change moved it further than the seeds do"
    } else {
        "inside the floor, so the move is smaller than one seed block's difference from the next"
    };
    passed(&name, rank, format!("{moved} {against}: {reading}"))
}

/// Confident errors did not rise. A direction, judged on its sign.
fn confident_errors(was: &Measure, now: &Measure, blocked: Option<&str>) -> Criterion {
    let name = "confident_errors_do_not_rise";
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let (Some(before), Some(after)) = (was.scores.confident_errors, now.scores.confident_errors)
    else {
        return Criterion {
            name: name.to_string(),
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: "confident errors were not counted on both sides, and unknown is never zero"
                .to_string(),
        };
    };
    let detail = format!(
        "confident errors {before} to {after} over {} answered items. A count is judged on its \
         sign: nothing has measured how far one seed block's count sits from the next, so this \
         criterion sets no size",
        now.answered
    );
    Criterion {
        name: name.to_string(),
        rank: 1,
        verdict: if after > before { Verdict::Failed } else { Verdict::Passed },
        detail,
    }
}

/// Renders a finding as the Markdown the command prints.
///
/// The renderer lives beside the judging so that what a reader sees is
/// tested with what produced it. Two things it must never do: print an
/// unknown as a zero, and print a verdict without the limit under it. The
/// second is why [`LIMIT`] is written here rather than left to a caller who
/// might forget it.
#[must_use]
pub fn render(finding: &Finding) -> String {
    match finding {
        Finding::Refused(refusal) => render_refusal(refusal),
        Finding::Compared(report) => render_report(report),
    }
}

/// Renders where each metric's floor came from, and what it does not cover.
///
/// Printed once beside a run's findings rather than under each of them: the
/// provenance is the rule's, not one door's.
#[must_use]
pub fn render_floors(rule: &Rule) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## Where the floors come from\n");
    for floor in &rule.metric_order {
        let basis = floor.block_sigma.basis;
        let value = match floor.block_sigma.value() {
            Some(sigma) => format!("one seed block's spread is {sigma:.4}"),
            None => "no measured spread".to_string(),
        };
        let _ = writeln!(out, "- **{}** — {value}, {basis}. {}", floor.metric, floor.block_sigma.why);
        if basis == Basis::Unmeasured {
            let _ = writeln!(
                out,
                "  Until that is measured, a move in {} is printed and not judged.",
                floor.metric
            );
        }
    }
    let _ = writeln!(
        out,
        "\nA rerun at the same seed block reproduces exactly, so every difference above was \
         caused by the change rather than by the seeds. The floor answers a second question: \
         whether the difference is larger than the one the seeds produce on their own, which is \
         what decides whether it survives being quoted.\n"
    );
    out
}

fn render_refusal(refusal: &Refusal) -> String {
    format!(
        "## `{}`\n\nRefused: {refusal}.\n\nNothing was compared, which is neither a pass nor a \
         regression.\n",
        refusal.door()
    )
}

#[allow(clippy::too_many_lines)]
fn render_report(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## `{}` against itself\n", report.door);
    let _ = writeln!(
        out,
        "`{}` at digest `{}`, {}. Recorded {} and {}; the door's identity is the same on both \
         sides, so what changed between them is this repository.\n",
        report.perturbation.suite,
        short(&report.perturbation.suite_digest),
        report.perturbation,
        report.before,
        report.after,
    );
    let _ = writeln!(
        out,
        "Floors from `{}`, digest `{}`.\n",
        report.rule_id, report.rule_digest
    );
    if report.before > report.after {
        // The flags decide which run is which, and a store passed as the
        // baseline can hold the later rows. Say so rather than letting
        // "before" and "after" imply a clock nobody read.
        let _ = writeln!(
            out,
            "The run being measured was recorded earlier than the one it is measured against. \
             These labels follow the flags, not the clock.\n"
        );
    }

    let _ = writeln!(
        out,
        "| Group | Asked | Answered | Accuracy | ECE | Brier | NLL | Confident errors |"
    );
    let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- | --- | --- |");
    for group in &report.groups {
        let _ = writeln!(out, "{}", measure_row(&group.group, "before", &group.before));
        let _ = writeln!(out, "{}", measure_row(&group.group, "after", &group.after));
    }
    let _ = writeln!(
        out,
        "\nA declined item stays in the denominator and out of the numerator, so a door that \
         begins declining the hard items does not score better for it.\n"
    );

    let _ = writeln!(out, "| Group | Verdict | Criterion |");
    let _ = writeln!(out, "| --- | --- | --- |");
    for group in &report.groups {
        let headline = group.headline().map_or_else(
            || "nothing was judged".to_string(),
            |criterion| format!("{}: {}", criterion.name, criterion.detail),
        );
        let _ = writeln!(out, "| `{}` | {} | {headline} |", group.group, group.verdict);
    }
    let _ = writeln!(out, "\n**{}**\n", report.verdict);

    let breaches: Vec<&Movement> = report.breaches().collect();
    if !breaches.is_empty() {
        let _ = writeln!(out, "### What did not pass\n");
        for group in breaches {
            for criterion in
                group.criteria.iter().filter(|criterion| criterion.verdict != Verdict::Passed)
            {
                let _ = writeln!(
                    out,
                    "- `{}` {}: {} — {}",
                    group.group, criterion.verdict, criterion.name, criterion.detail
                );
            }
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "### What this does not catch\n");
    let _ = writeln!(out, "{}\n", report.limit);
    out
}

fn measure_row(group: &str, side: &str, measure: &Measure) -> String {
    format!(
        "| `{group}`, {side} | {} | {} | {} | {} | {} | {} | {} |",
        measure.asked,
        measure.answered,
        number(measure.scores.accuracy, 3),
        number(measure.scores.ece, 3),
        number(measure.scores.brier, 3),
        number(measure.scores.nll, 3),
        count(measure.scores.confident_errors),
    )
}

/// A measured number, or the word for the absence of one.
fn number(value: Option<f64>, places: usize) -> String {
    value.map_or_else(|| "unknown".to_string(), |value| format!("{value:.places$}"))
}

fn count(value: Option<usize>) -> String {
    value.map_or_else(|| "unknown".to_string(), |value| value.to_string())
}

/// A few item ids, for a message that has to stay one line.
fn name_a_few(items: &[&String]) -> String {
    const SHOWN: usize = 3;
    if items.is_empty() {
        return String::new();
    }
    let head: Vec<&str> = items.iter().take(SHOWN).map(|item| item.as_str()).collect();
    if items.len() <= SHOWN {
        format!(" ({})", head.join(", "))
    } else {
        format!(" ({}, and {} more)", head.join(", "), items.len() - SHOWN)
    }
}

/// Which identity field moved, in the words a reader can act on.
fn identity_difference(before: &DoorIdentity, after: &DoorIdentity) -> String {
    let mut moved = Vec::new();
    let named = [
        ("the model", &before.model, &after.model),
        (
            "the base model signature",
            &before.base_model_signature,
            &after.base_model_signature,
        ),
        ("the adapter", &before.adapter, &after.adapter),
    ];
    for (field, was, now) in named {
        if was != now {
            moved.push(format!(
                "{field} went from {} to {}",
                or_none(was),
                or_none(now)
            ));
        }
    }
    if before.verified != after.verified {
        moved.push(format!(
            "the identity went from {} to {}",
            verifiable(before.verified),
            verifiable(after.verified)
        ));
    }
    if moved.is_empty() {
        "the identities differ in a field this message does not name".to_string()
    } else {
        moved.join(", and ")
    }
}

fn or_none(value: &str) -> String {
    if value.is_empty() {
        "none".to_string()
    } else {
        format!("`{value}`")
    }
}

fn verifiable(verified: bool) -> &'static str {
    if verified { "verifiable" } else { "not verifiable" }
}

/// The question text a run served, for a refusal that has to name both.
fn name_questions(perturbation: &Perturbation) -> String {
    match (&perturbation.question_set, &perturbation.question_digest) {
        (Some(set), Some(digest)) => format!("question set `{set}` at {}", short(digest)),
        (Some(set), None) => format!("question set `{set}`, which pins no digest"),
        (None, Some(digest)) => format!("an unnamed question set at {}", short(digest)),
        (None, None) => "the suite's own question text".to_string(),
    }
}

fn short(digest: &str) -> String {
    digest.chars().take(16).collect()
}

fn passed(name: &str, rank: u8, detail: String) -> Criterion {
    Criterion {
        name: name.to_string(),
        rank,
        verdict: Verdict::Passed,
        detail,
    }
}

fn not_judged(name: &str, rank: u8, reason: &str) -> Criterion {
    Criterion {
        name: name.to_string(),
        rank,
        verdict: Verdict::Unverifiable,
        detail: format!("not judged: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::Bound;
    use crate::row::RefusalCode;
    use indexmap::IndexMap;

    const SUITE: &str = "support-v2";
    const DIGEST: &str = "sha256:one";
    const BEFORE: &str = "2026-09-12T09:00:00Z";
    const AFTER: &str = "2026-09-19T09:00:00Z";

    fn identity() -> DoorIdentity {
        DoorIdentity::published("lev", "sig-1", "band-v1")
    }

    fn start(item: &str, family: &str, at: &str) -> Row {
        let mut row = Row::new(SUITE, DIGEST, item, "lev");
        row.recorded_at = at.to_string();
        row.split = "development".to_string();
        row.family = family.to_string();
        row.door_identity = identity();
        row.estimator = "l2".to_string();
        row.samples = Some(8);
        row.seed_base = Some(0);
        row
    }

    /// One answered item, at the probability the door reported for the
    /// option it picked.
    fn answered_at(item: &str, family: &str, at: &str, correct: bool, top: f64) -> Row {
        let distribution: IndexMap<String, f64> =
            [("yes".to_string(), top), ("no".to_string(), 1.0 - top)].into_iter().collect();
        start(item, family, at).scored(distribution, correct)
    }

    fn declined(item: &str, family: &str, at: &str) -> Row {
        start(item, family, at).refused(RefusalCode::Guardrail)
    }

    /// One run over twenty `routing` items, `right` of them answered
    /// correctly, every answer reported at 0.8.
    fn run_of(at: &str, right: usize) -> Vec<Row> {
        (0..20)
            .map(|index| {
                answered_at(&format!("routing/{index:03}"), "routing", at, index < right, 0.8)
            })
            .collect()
    }

    fn rule() -> Rule {
        Rule::v2()
    }

    fn only(findings: Vec<Finding>) -> Finding {
        assert_eq!(findings.len(), 1, "one door, one finding");
        findings.into_iter().next().expect("a finding")
    }

    fn report_of(findings: Vec<Finding>) -> Report {
        match only(findings) {
            Finding::Compared(report) => *report,
            Finding::Refused(refusal) => panic!("expected a comparison, got: {refusal}"),
        }
    }

    fn refusal_of(findings: Vec<Finding>) -> Refusal {
        match only(findings) {
            Finding::Refused(refusal) => refusal,
            Finding::Compared(report) => {
                panic!("expected a refusal, got a comparison of {}", report.door)
            }
        }
    }

    fn criterion<'a>(report: &'a Report, group: &str, name: &str) -> &'a Criterion {
        report
            .groups
            .iter()
            .find(|movement| movement.group == group)
            .unwrap_or_else(|| panic!("a movement for {group}"))
            .criteria
            .iter()
            .find(|criterion| criterion.name == name)
            .unwrap_or_else(|| panic!("a criterion named {name}"))
    }

    #[test]
    fn the_schema_tag_belongs_to_this_crate() {
        assert!(SCHEMA.starts_with(crate::SCHEMA_PREFIX), "{SCHEMA}");
    }

    #[test]
    fn a_rerun_that_changed_nothing_passes() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 16)), &run_of(AFTER, 16), &rule()));
        assert_eq!(report.verdict, Verdict::Passed, "{:#?}", report.groups);
        assert_eq!(report.before, BEFORE);
        assert_eq!(report.after, AFTER);
        // The unmeasured metrics must not hold a clean rerun hostage: they
        // did not move, and a metric that did not move needs no floor.
        assert_eq!(
            criterion(&report, OVERALL, "ece_holds_within_the_noise").verdict,
            Verdict::Passed
        );
    }

    #[test]
    fn a_fall_larger_than_the_floor_is_a_regression() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 16)), &run_of(AFTER, 14), &rule()));
        assert_eq!(report.verdict, Verdict::Failed);
        let accuracy = criterion(&report, OVERALL, "accuracy_holds_within_the_noise");
        assert_eq!(accuracy.verdict, Verdict::Failed);
        assert!(accuracy.detail.contains("0.800 to 0.700"), "{}", accuracy.detail);
        assert!(
            accuracy.detail.contains("0.056"),
            "the floor is on the line: {}",
            accuracy.detail
        );
    }

    #[test]
    fn a_fall_inside_the_floor_holds_and_says_it_is_inside() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 16)), &run_of(AFTER, 15), &rule()));
        let accuracy = criterion(&report, OVERALL, "accuracy_holds_within_the_noise");
        assert_eq!(accuracy.verdict, Verdict::Passed, "0.050 is under the 0.056 floor");
        assert!(accuracy.detail.contains("inside the floor"), "{}", accuracy.detail);
        assert_ne!(report.verdict, Verdict::Failed, "a move inside the floor is not a loss");
    }

    #[test]
    fn a_gain_larger_than_the_floor_is_reported_as_one() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 14)), &run_of(AFTER, 16), &rule()));
        let accuracy = criterion(&report, OVERALL, "accuracy_holds_within_the_noise");
        assert_eq!(accuracy.verdict, Verdict::Passed);
        assert!(
            accuracy.detail.contains("further than the seeds"),
            "a real gain is named as one: {}",
            accuracy.detail
        );
    }

    #[test]
    fn a_door_that_begins_declining_fails_even_when_accuracy_rises() {
        // The shape of the state-prompt change: the guardrail fires, the
        // items it fires on leave the numerator, and the average over what
        // is left goes up.
        let before = run_of(BEFORE, 16);
        let mut after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|row| {
                if row.correct == Some(false) {
                    declined(&row.item_id, &row.family, AFTER)
                } else {
                    row
                }
            })
            .collect();
        after.sort_by(|left, right| left.item_id.cmp(&right.item_id));

        let report = report_of(review(Some(&before), &after, &rule()));
        assert_eq!(report.verdict, Verdict::Failed);
        let answered = criterion(&report, OVERALL, "the_door_answered_the_same_items");
        assert_eq!(answered.verdict, Verdict::Failed);
        assert!(
            answered.detail.contains("declined 4 items it answered before"),
            "{}",
            answered.detail
        );
        assert!(answered.detail.contains("`guardrail` x4"), "{}", answered.detail);

        let overall = report.overall().expect("the suite's movement");
        assert_eq!(overall.after.scores.accuracy, Some(1.0), "the average rose");
        assert_eq!(overall.after.asked, 20, "and the items stayed in the denominator");
        assert_eq!(overall.after.answered, 16);
        assert_eq!(
            overall.deciding().map(|criterion| criterion.name.as_str()),
            Some("the_door_answered_the_same_items"),
            "the refusal decides, not the flattered average"
        );
    }

    #[test]
    fn a_door_that_stops_declining_passes_and_names_the_items() {
        let before: Vec<Row> = run_of(BEFORE, 16)
            .into_iter()
            .map(|row| {
                if row.item_id == "routing/019" {
                    declined(&row.item_id, &row.family, BEFORE)
                } else {
                    row
                }
            })
            .collect();
        let report = report_of(review(Some(&before), &run_of(AFTER, 16), &rule()));
        let answered = criterion(&report, OVERALL, "the_door_answered_the_same_items");
        assert_eq!(answered.verdict, Verdict::Passed);
        assert!(
            answered.detail.contains("answered 1 items it declined before (routing/019)"),
            "{}",
            answered.detail
        );
    }

    #[test]
    fn a_rise_in_confident_errors_is_a_regression() {
        // One item answered wrong at 0.95 on the later run and right on the
        // earlier one. Accuracy moves by 0.050, inside the floor; the
        // confident error is what carries the verdict.
        let mut before = run_of(BEFORE, 20);
        before.pop();
        before.push(answered_at("routing/019", "routing", BEFORE, true, 0.95));
        let mut after = run_of(AFTER, 20);
        after.pop();
        after.push(answered_at("routing/019", "routing", AFTER, false, 0.95));

        let report = report_of(review(Some(&before), &after, &rule()));
        let errors = criterion(&report, OVERALL, "confident_errors_do_not_rise");
        assert_eq!(errors.verdict, Verdict::Failed);
        assert!(errors.detail.contains("0 to 1"), "{}", errors.detail);
        assert!(errors.detail.contains("sets no size"), "{}", errors.detail);
        assert_eq!(report.verdict, Verdict::Failed);
    }

    #[test]
    fn a_metric_with_no_measured_spread_is_reported_and_not_judged() {
        // Every item answered correctly on both runs, and reported less
        // confidently on the later one. Accuracy does not move, no answer is
        // wrong at all, and the calibration measures all move the wrong way.
        //
        // openagents#9376 measured every floor the committed rule carries,
        // so the rule that exercises this path is built for it. It is the
        // behaviour a metric added before its measurement will meet.
        let mut unmeasured = rule();
        for floor in &mut unmeasured.metric_order {
            if floor.metric == Metric::Brier {
                floor.block_sigma = Bound {
                    value: None,
                    basis: Basis::Unmeasured,
                    why: "Nothing has measured this.".into(),
                };
            }
        }
        let before: Vec<Row> = (0..20)
            .map(|index| {
                answered_at(&format!("routing/{index:03}"), "routing", BEFORE, true, 0.8)
            })
            .collect();
        let after: Vec<Row> = (0..20)
            .map(|index| {
                answered_at(&format!("routing/{index:03}"), "routing", AFTER, true, 0.6)
            })
            .collect();
        let report = report_of(review(Some(&before), &after, &unmeasured));
        let brier = criterion(&report, OVERALL, "brier_holds_within_the_noise");
        assert_eq!(brier.verdict, Verdict::Unverifiable, "{}", brier.detail);
        assert!(
            brier.detail.contains("Nothing has measured this suite's block-to-block spread"),
            "{}",
            brier.detail
        );
        // The report itself fails, on ece and log loss, which do carry
        // floors and did move further the wrong way than those floors
        // allow. What this pins is that brier, which has no floor here, is
        // reported rather than counted against the door.
        assert_eq!(report.verdict, Verdict::Failed);
        assert_ne!(
            criterion(&report, OVERALL, "ece_holds_within_the_noise").verdict,
            Verdict::Unverifiable,
            "a metric with a floor is judged by it"
        );
    }

    #[test]
    fn a_changed_suite_refuses_the_comparison() {
        let before = run_of(BEFORE, 16);
        let after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|mut row| {
                row.suite_digest = "sha256:two".to_string();
                row
            })
            .collect();
        let refusal = refusal_of(review(Some(&before), &after, &rule()));
        assert!(matches!(refusal, Refusal::SuiteMoved { .. }), "{refusal:?}");
        let said = refusal.to_string();
        assert!(said.contains("sha256:one"), "{said}");
        assert!(said.contains("sha256:two"), "{said}");
        assert!(
            said.contains("a different measurement, not a regression"),
            "the reason is on the refusal: {said}"
        );
    }

    #[test]
    fn a_changed_door_refuses_the_comparison_and_names_the_field() {
        let before = run_of(BEFORE, 16);
        let after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|mut row| {
                row.door_identity = DoorIdentity::published("lev", "sig-2", "band-v1");
                row
            })
            .collect();
        let refusal = refusal_of(review(Some(&before), &after, &rule()));
        let said = refusal.to_string();
        assert!(matches!(refusal, Refusal::DoorMoved { .. }), "{refusal:?}");
        assert!(said.contains("base model signature"), "{said}");
        assert!(said.contains("`sig-1`") && said.contains("`sig-2`"), "{said}");
    }

    #[test]
    fn a_different_seed_block_refuses_the_comparison() {
        let before = run_of(BEFORE, 16);
        let after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|mut row| {
                row.seed_base = Some(1);
                row
            })
            .collect();
        let refusal = refusal_of(review(Some(&before), &after, &rule()));
        assert!(matches!(refusal, Refusal::PerturbationMoved { .. }), "{refusal:?}");
        let said = refusal.to_string();
        assert!(said.contains("seed block 0") && said.contains("seed block 1"), "{said}");
    }

    #[test]
    fn reworded_question_text_refuses_the_comparison() {
        let before: Vec<Row> = run_of(BEFORE, 16)
            .into_iter()
            .map(|mut row| {
                row.question_set = Some("support-v2-v1".to_string());
                row.question_digest = Some("sha256:text-one".to_string());
                row
            })
            .collect();
        let after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|mut row| {
                row.question_set = Some("support-v2-v2".to_string());
                row.question_digest = Some("sha256:text-two".to_string());
                row
            })
            .collect();
        let refusal = refusal_of(review(Some(&before), &after, &rule()));
        assert!(matches!(refusal, Refusal::QuestionsMoved { .. }), "{refusal:?}");
        let said = refusal.to_string();
        assert!(said.contains("support-v2-v1") && said.contains("support-v2-v2"), "{said}");
        assert!(
            said.contains("a candidate against the same items"),
            "a reworded question is not a regression in the door: {said}"
        );
    }

    #[test]
    fn a_different_number_of_draws_refuses_the_comparison() {
        let before = run_of(BEFORE, 16);
        let after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|mut row| {
                row.samples = Some(1);
                row
            })
            .collect();
        let refusal = refusal_of(review(Some(&before), &after, &rule()));
        assert!(matches!(refusal, Refusal::PerturbationMoved { .. }), "{refusal:?}");
        let said = refusal.to_string();
        assert!(said.contains("8 draws") && said.contains("1 draws"), "{said}");
    }

    #[test]
    fn one_recorded_run_has_nothing_to_compare_against() {
        let refusal = refusal_of(review(None, &run_of(AFTER, 16), &rule()));
        assert!(matches!(refusal, Refusal::OneRun { .. }), "{refusal:?}");
        assert!(refusal.to_string().contains("--against"), "it says what to do next");
    }

    #[test]
    fn one_store_holding_two_runs_needs_no_second_store() {
        let mut rows = run_of(BEFORE, 16);
        rows.extend(run_of(AFTER, 14));
        let report = report_of(review(None, &rows, &rule()));
        assert_eq!(report.before, BEFORE);
        assert_eq!(report.after, AFTER);
        assert_eq!(report.verdict, Verdict::Failed);
    }

    #[test]
    fn the_newest_run_is_the_one_judged() {
        let mut rows = run_of("2026-09-05T09:00:00Z", 10);
        rows.extend(run_of(BEFORE, 16));
        rows.extend(run_of(AFTER, 16));
        let report = report_of(review(None, &rows, &rule()));
        assert_eq!(report.after, AFTER, "the newest run is this commit's");
        assert_eq!(report.before, BEFORE, "and it is measured against the one before it");
    }

    #[test]
    fn a_reversed_pass_is_a_run_of_its_own() {
        let mut rows = run_of(AFTER, 16);
        rows.extend(run_of(AFTER, 16).into_iter().map(|mut row| {
            row.permutation = Some(vec![1, 0]);
            row
        }));
        let panels = panels(&rows);
        assert_eq!(panels.len(), 2, "one option order is not the other");
        assert_eq!(panels[0].perturbation.permutation, None);
        assert_eq!(panels[1].perturbation.permutation, Some(vec![1, 0]));
    }

    #[test]
    fn each_family_is_judged_on_its_own() {
        let mut before = run_of(BEFORE, 16);
        before.extend((0..20).map(|index| {
            answered_at(&format!("severity/{index:03}"), "severity", BEFORE, index < 18, 0.8)
        }));
        let mut after = run_of(AFTER, 16);
        after.extend((0..20).map(|index| {
            answered_at(&format!("severity/{index:03}"), "severity", AFTER, index < 14, 0.8)
        }));

        let report = report_of(review(Some(&before), &after, &rule()));
        assert_eq!(
            criterion(&report, "routing", "accuracy_holds_within_the_noise").verdict,
            Verdict::Passed,
            "routing did not move"
        );
        let severity = criterion(&report, "severity", "accuracy_holds_within_the_noise");
        assert_eq!(severity.verdict, Verdict::Failed, "severity lost 0.200");
        assert!(
            severity.detail.contains("standing in for this family's"),
            "the substitution is named where it is used: {}",
            severity.detail
        );
        assert_eq!(report.verdict, Verdict::Failed, "one family carries the report");
    }

    #[test]
    fn two_runs_over_different_items_are_two_measurements() {
        let before = run_of(BEFORE, 16);
        let after: Vec<Row> = run_of(AFTER, 16).into_iter().take(18).collect();
        let report = report_of(review(Some(&before), &after, &rule()));
        let asked = criterion(&report, OVERALL, "the_same_items_were_asked");
        assert_eq!(asked.verdict, Verdict::Unverifiable);
        assert!(asked.detail.contains("2 dropped"), "{}", asked.detail);
        assert_eq!(
            criterion(&report, OVERALL, "accuracy_holds_within_the_noise").verdict,
            Verdict::Unverifiable,
            "nothing below an unequal item set is judged"
        );
        assert_eq!(
            report.verdict,
            Verdict::Unverifiable,
            "and it is not called a regression"
        );
    }

    #[test]
    fn a_door_that_answered_nothing_reports_unknown_and_never_zero() {
        let before = run_of(BEFORE, 16);
        let after: Vec<Row> = run_of(AFTER, 16)
            .into_iter()
            .map(|row| declined(&row.item_id, &row.family, AFTER))
            .collect();
        let report = report_of(review(Some(&before), &after, &rule()));
        let overall = report.overall().expect("the suite's movement");
        assert_eq!(overall.after.scores.accuracy, None, "an average over nothing is not zero");
        assert_eq!(overall.after.answered, 0);
        assert_eq!(overall.after.asked, 20);
        let rendered = render(&Finding::Compared(Box::new(report)));
        assert!(rendered.contains("| 20 | 0 | unknown |"), "{rendered}");
        assert!(!rendered.contains("| 20 | 0 | 0.000 |"), "{rendered}");
    }

    #[test]
    fn a_group_that_passed_leads_with_the_accuracy_rather_than_the_item_count() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 14)), &run_of(AFTER, 16), &rule()));
        let overall = report.overall().expect("the suite's movement");
        assert_eq!(overall.verdict, Verdict::Passed);
        assert_eq!(
            overall.deciding().map(|criterion| criterion.name.as_str()),
            Some("the_same_items_were_asked"),
            "the first rank-1 criterion that passed says nothing about the door"
        );
        assert_eq!(
            overall.headline().map(|criterion| criterion.name.as_str()),
            Some("accuracy_holds_within_the_noise"),
            "so a passing group leads with the accuracy instead"
        );
    }

    #[test]
    fn the_report_carries_the_rule_that_set_its_floors() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 16)), &run_of(AFTER, 16), &rule()));
        assert_eq!(report.rule_id, "ab-v2");
        assert_eq!(report.rule_digest, rule().digest());
        assert!(report.rule_digest.starts_with("gate:"), "{}", report.rule_digest);
    }

    #[test]
    fn the_limit_is_on_the_report_and_on_the_output() {
        let report =
            report_of(review(Some(&run_of(BEFORE, 16)), &run_of(AFTER, 16), &rule()));
        assert_eq!(report.limit, LIMIT);
        let rendered = render(&Finding::Compared(Box::new(report)));
        assert!(rendered.contains("### What this does not catch"), "{rendered}");
        assert!(rendered.contains(LIMIT), "the limit is printed verbatim: {rendered}");
        assert!(
            rendered.contains("openagents#9379") && rendered.contains("openagents#9381"),
            "and it names where the answer is: {rendered}"
        );
    }

    #[test]
    fn a_refusal_renders_as_neither_a_pass_nor_a_failure() {
        let findings = review(None, &run_of(AFTER, 16), &rule());
        let rendered = render(&findings[0]);
        assert!(rendered.contains("Refused:"), "{rendered}");
        assert!(rendered.contains("neither a pass nor a regression"), "{rendered}");
        assert_eq!(findings[0].verdict(), None, "a refusal reaches no verdict");
    }

    #[test]
    fn every_door_in_the_store_is_answered_for() {
        let mut rows = run_of(BEFORE, 16);
        rows.extend(run_of(AFTER, 16));
        // A second door with one run, which cannot be compared and must not
        // vanish from a check somebody is about to trust.
        rows.extend(run_of(AFTER, 16).into_iter().map(|mut row| {
            row.door = "kev".to_string();
            row
        }));
        let findings = review(None, &rows, &rule());
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].door(), "kev");
        assert_eq!(findings[0].verdict(), None, "one run, so no comparison");
        assert_eq!(findings[1].door(), "lev");
        assert_eq!(findings[1].verdict(), Some(Verdict::Passed));
    }
}
