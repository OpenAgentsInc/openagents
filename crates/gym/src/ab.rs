//! The loop that compares two doors and decides what to keep.
//!
//! A round schedules cells, runs them interleaved, screens the result, and
//! confirms the screening win on seed blocks nobody has drawn. It emits one
//! document that carries the rule it was judged by, so a reader never has to
//! guess what bar applied.
//!
//! The design is carried from `~/work/coder/ops/gym-ab.py`,
//! `ops/gym-evidence.py`, and `ops/gym-matrix.py` and reimplemented here. The
//! Python controllers do not come over, and neither do their constants; see
//! [Where the numbers come from](#where-the-numbers-come-from).
//!
//! # Why a confirmation stage needs a seed block
//!
//! The doors here are near-deterministic. Before openagents#9370 the L2
//! estimator drew seeds `0..n` and reproduced exactly, so "confirm the win on
//! fresh trials" would have rerun the same arithmetic and reported it as
//! independent evidence. That is worse than no confirmation stage, because it
//! manufactures a second witness out of the first one.
//!
//! `crates/lev/src/estimator.rs` now takes a `seed_base`, and
//! `seed_block(seed_base, n)` tiles disjoint blocks: block 0 draws `0..n`,
//! block 1 draws `n..2n`, and no two blocks share a seed. That is the
//! randomness axis this module schedules over. A confirmation that reuses a
//! screening block is refused by [`Rule::confirm`] rather than reported as a
//! repeat.
//!
//! # What the loop does
//!
//! - **Interleaving.** [`Plan::cells`] alternates which side runs first, per
//!   family and per block, so a machine that drifts over the round does not
//!   favour one side. The schedule is a pure function of the plan, so the
//!   same plan yields the same order every time.
//! - **The rule travels inside the evidence.** [`Evidence`] carries the whole
//!   [`Rule`], its digest, and the metric order. Each [`Stage`] carries a
//!   [`crate::gate::Outcome`] whose `gate_id` and `gate_digest` name that
//!   rule, so a stored verdict can be matched to the rule that produced it.
//! - **Confirm on the metric you won on.** [`Rule::confirm`] judges the
//!   metric the screening earned, and nothing else. A different favourable
//!   metric cannot be substituted, and when one would have cleared, the
//!   verdict says so by name. This is the most important anti-fishing device
//!   in the reference implementation.
//! - **Medians agree with means.** On the earning metric, the median across
//!   blocks has to move the same way as the mean. A mean carried by one lucky
//!   block is reverted.
//! - **A hard floor.** Confident errors must not rise. No improvement
//!   anywhere buys it off. The reference's "the candidate passes every trial"
//!   has no analogue here, because accuracy is continuous and there is no
//!   per-trial pass, so this replaces that rule rather than translating it.
//! - **A per-family guard.** A candidate that improves overall while wrecking
//!   `severity` is refused, for the same reason a coding candidate that
//!   wrecks one task is.
//! - **Harness failures requeue once and never enter a comparison.** A door
//!   refusal does enter it, as a refusal: the item stays in the denominator
//!   and out of the numerator, which is the rule [`crate::row`] states and
//!   the only arrangement under which a door cannot score better by declining
//!   the questions it finds hard.
//!
//! # Where the numbers come from
//!
//! `docs/lev/measurements/2026-09-19-seed-variance.md` drew eight disjoint
//! blocks over the same 98 evaluation items, with the door, the items, the
//! machine, and the estimator all held fixed. The accuracy of that unchanged
//! door ran from 0.745 to 0.796, a standard deviation of **0.0197**. Only the
//! seeds differed. That is this suite's noise floor on the seed axis, and it
//! is the only floor this module has.
//!
//! A comparison carries both sides' noise. With `b` blocks behind the
//! baseline and `c` behind the candidate, the standard deviation of the
//! difference is `sigma * sqrt(1/b + 1/c)`. At one block a side that is
//! 0.0279, and two of them is **0.056 accuracy, 7.2% of the 0.781 mean**.
//! Below that, a comparison is reporting which seeds it drew. The bound is
//! computed per round rather than stored, so a round that draws more blocks
//! lowers the bar it has to clear — at three blocks a side it is 0.032 — and
//! a thin round raises its own.
//!
//! The reference implementation's `0.10` guard is not carried over. It was
//! chosen for tool calls and wall-clock seconds over twelve container tasks.
//! On this suite, 10% relative accuracy is 0.078, which is 2.8 of these
//! standard deviations: the right order by coincidence, not by measurement.
//! Effect sizes here are multiples of the measured block-to-block standard
//! deviation of the metric being gated.
//!
//! # What this module refuses to choose
//!
//! [`crate::gate`] sets the precedent: a rule records `pending_measurement`
//! inside its digest rather than picking a number nobody measured. The same
//! applies here, and there are four gaps.
//!
//! - **ECE, Brier, and log loss have no measured block-to-block spread.** The
//!   seed sweep measured accuracy and the mean top share and nothing else.
//!   Their floors are [`Basis::Unmeasured`], so a win on them reports
//!   `unverifiable` and can never earn a keep. A 10% relative move in ECE on
//!   50 items is well inside binning noise, and inventing a floor for it
//!   would be worse than admitting there is none.
//! - **The flip rate has no floor and is carrying a headline.** Order
//!   sensitivity falling from 0.120 to 0.040 is 6 flips against 2 on 50
//!   items, and openagents#9375 measures how wide that interval is. It is not
//!   a metric here because it is not in [`Scores`]; it is named in
//!   [`Rule::pending_measurements`], inside the digest, so the gap travels
//!   with the rule.
//! - **No family has its own floor.** The same run reports 5 of 50 `routing`
//!   items changing answer across blocks against 12 of 18 `severity` items,
//!   so the families plainly do not share one spread, and nothing measured
//!   what each one's is. The family guard multiplies the suite floor, which
//!   understates `severity`'s noise and so refuses candidates a per-family
//!   floor might admit. For a guard, over-refusing is the safe direction. The
//!   substitution is written into the bound's own `why` and into every detail
//!   line the guard produces.
//! - **The median has no floor either.** The median of three to eight blocks
//!   has no measured spread, so the median criterion asks only that the
//!   median move the same way as the mean. It sets no size.
//!
//! One more gap is worth stating and is not fillable by measurement of this
//! kind: the blocks hold the item set fixed, so everything here covers seed
//! resampling alone. The binomial standard error of an accuracy near 0.78 on
//! 98 items is about 0.042, roughly twice the seed noise. A win that survives
//! a fresh block has been shown to be more than a lucky draw; it has not been
//! shown to generalize to items the suite does not contain. [`Rule::covers`]
//! and [`Rule::does_not_cover`] say so in the document.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::gate::{Basis, Bound, Criterion, Outcome, Scores, Verdict};

/// The schema every A/B evidence document is tagged with.
pub const SCHEMA: &str = "openagents.gym.ab_evidence.v1";

/// The group name the suite-wide summary and outcome use.
pub const OVERALL: &str = "overall";

/// A measure a round can be compared on.
///
/// [`Metric::ConfidentErrors`] is here so it aggregates through the same
/// path as everything else. It is deliberately not admitted into
/// [`Rule::metric_order`]: it is a floor, not a prize, and a candidate that
/// wins by being confidently wrong less often has not been shown to be a
/// better door.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Share of items whose winning option was the labelled answer.
    Accuracy,
    /// Expected calibration error over ten bins.
    Ece,
    /// Brier score on the winning option.
    Brier,
    /// Negative log likelihood of the winning option's outcome.
    Nll,
    /// Items that were wrong at a reported probability of 0.9 or above.
    ConfidentErrors,
}

impl Metric {
    /// Every metric this module reads, in a fixed order.
    pub const ALL: [Self; 5] = [
        Self::Accuracy,
        Self::Ece,
        Self::Brier,
        Self::Nll,
        Self::ConfidentErrors,
    ];

    /// The wire label, which is also the word used in verdict details.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accuracy => "accuracy",
            Self::Ece => "ece",
            Self::Brier => "brier",
            Self::Nll => "nll",
            Self::ConfidentErrors => "confident_errors",
        }
    }

    /// Whether a smaller number is the better one.
    #[must_use]
    pub const fn lower_is_better(self) -> bool {
        !matches!(self, Self::Accuracy)
    }

    /// Reads the metric off one side's scores, or `None` when nobody
    /// measured it. Absent is never zero.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn read(self, scores: &Scores) -> Option<f64> {
        match self {
            Self::Accuracy => scores.accuracy,
            Self::Ece => scores.ece,
            Self::Brier => scores.brier,
            Self::Nll => scores.nll,
            Self::ConfidentErrors => scores.confident_errors.map(|count| count as f64),
        }
    }

    /// How much the candidate gained, with the metric's own direction
    /// applied, so a positive number is always an improvement.
    #[must_use]
    pub fn gain(self, baseline: f64, candidate: f64) -> f64 {
        if self.lower_is_better() {
            baseline - candidate
        } else {
            candidate - baseline
        }
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which side of the comparison a cell belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The door in service.
    Control,
    /// The door proposed to replace it.
    Candidate,
}

impl Side {
    /// Both sides, control first.
    pub const BOTH: [Self; 2] = [Self::Control, Self::Candidate];

    /// The word used in records and on screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Candidate => "candidate",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which half of the loop a round is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// The first look, which proposes a metric.
    Screening,
    /// The second look, on blocks nobody has drawn, which judges that metric
    /// and no other.
    Confirmation,
}

impl Phase {
    /// The word used in records and on screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Screening => "screening",
            Self::Confirmation => "confirmation",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the loop decided.
///
/// [`Verdict`] says what could be told; this says what to do about it. Only
/// [`Decision::Kept`] ships. `Reverted` is a measured loss or a breach of the
/// rule, and `Undecided` is a round nobody could read, which is not the same
/// accusation and must not be rendered as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// The candidate earned a win and repeated it on fresh blocks.
    Kept,
    /// Something measurably lost, or the rule was breached.
    Reverted,
    /// Nobody could tell. The candidate is not kept, and it did not lose.
    Undecided,
}

impl Decision {
    /// The decision a verdict carries.
    #[must_use]
    pub const fn of(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Passed => Self::Kept,
            Verdict::Failed => Self::Reverted,
            Verdict::Unverifiable => Self::Undecided,
        }
    }

    /// Whether the candidate ships.
    #[must_use]
    pub const fn kept(self) -> bool {
        matches!(self, Self::Kept)
    }

    /// The word used in records and on screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kept => "kept",
            Self::Reverted => "reverted",
            Self::Undecided => "undecided",
        }
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A metric, and the block-to-block spread a difference in it is measured
/// against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricFloor {
    /// The metric.
    pub metric: Metric,
    /// One standard deviation of this metric across seed blocks on this
    /// suite, with one door and one item set held fixed. Absent when nobody
    /// has measured it, in which case a win on this metric is
    /// [`Verdict::Unverifiable`] and cannot earn a keep.
    pub block_sigma: Bound,
}

/// A measurement that would complete the rule, and the issue that takes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    /// What is unmeasured.
    pub quantity: String,
    /// The issue that measures it.
    pub issue: String,
    /// What the rule does in the meantime, and why that is the safe
    /// direction.
    pub why: String,
}

/// What is wrong with a rule that says something impossible.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// The rule has no id, so a verdict could not name what produced it.
    #[error("the rule has no id; a verdict has to name the rule that produced it")]
    NoId,
    /// The rule proposes no metric a win can be earned on.
    #[error("the rule orders no metrics, so no win can be earned")]
    NoMetrics,
    /// The rule orders the same metric twice.
    #[error("the rule orders {metric} twice; the order decides which win counts")]
    DuplicateMetric {
        /// The metric that appears more than once.
        metric: Metric,
    },
    /// The rule lets a win be earned on the hard floor.
    #[error(
        "the rule lets a win be earned on confident_errors, which is a floor and not a prize; \
         a door that is confidently wrong less often has not been shown to be a better door"
    )]
    ConfidentErrorsEarnAWin,
    /// A bound says something impossible about where its number came from.
    #[error("{name}: {problem}")]
    Bound {
        /// The bound that is wrong.
        name: String,
        /// What is wrong with it.
        problem: String,
    },
}

/// The acceptance rule for a two-door round.
///
/// Every threshold carries a [`Bound`], and every bound records its
/// [`Basis`], so a reader can tell a measurement from a convention. All of it
/// is inside [`Rule::digest`]: relabelling a constant produces a new rule
/// rather than new history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// The rule's id. Versioned, because changing a threshold produces a new
    /// rule.
    pub id: String,
    /// The product question this rule answers, in one line.
    pub question: String,
    /// The metrics a win may be earned on, most decisive first. The first
    /// metric whose gain clears its own noise earns the round, and it is the
    /// only metric the confirmation may judge.
    pub metric_order: Vec<MetricFloor>,
    /// How many standard deviations of the difference a win has to clear.
    pub effect_size_sigmas: Bound,
    /// How many of the same standard deviations one family may lose before
    /// the round is refused.
    pub family_regression_sigmas: Bound,
    /// Fewest seed blocks behind each side.
    pub min_blocks_per_side: Bound,
    /// How many times a cell whose harness failed is requeued.
    pub requeue_limit: u32,
    /// What the effect size accounts for.
    pub covers: String,
    /// What it does not account for, which is the larger half.
    pub does_not_cover: String,
    /// The measurements that would complete this rule.
    pub pending_measurements: Vec<Pending>,
}

impl Rule {
    /// The committed rule, derived from
    /// `docs/lev/measurements/2026-09-19-seed-variance.md`.
    ///
    /// Read [Where the numbers come from](index.html#where-the-numbers-come-from)
    /// for each derivation, and
    /// [What this module refuses to choose](index.html#what-this-module-refuses-to-choose)
    /// for the four gaps this rule declines to fill.
    #[must_use]
    pub fn v1() -> Self {
        let unmeasured = |metric: Metric| MetricFloor {
            metric,
            block_sigma: Bound {
                value: None,
                basis: Basis::Unmeasured,
                why: format!(
                    "The seed sweep in docs/lev/measurements/2026-09-19-seed-variance.md \
                     measured accuracy and the mean top share. Nothing has measured the \
                     block-to-block spread of {metric} on this suite, so a win on it is \
                     unverifiable rather than earned. A 10% relative move in {metric} on 50 \
                     items is well inside the noise of the binning that produces it."
                ),
            },
        };
        Self {
            id: "ab-v1".into(),
            question: "Should this candidate replace the baseline, on a win that repeats on \
                       seed blocks nobody has drawn?"
                .into(),
            metric_order: vec![
                MetricFloor {
                    metric: Metric::Accuracy,
                    block_sigma: Bound {
                        value: Some(0.0197),
                        basis: Basis::Derived,
                        why: "Eight disjoint seed blocks over the same 98 evaluation items, \
                              with the door, the items, and the machine held fixed, gave \
                              accuracies from 0.745 to 0.796: a standard deviation of 0.0197. \
                              docs/lev/measurements/2026-09-19-seed-variance.md. Only the seeds \
                              differed, so this is the suite's own floor on the seed axis."
                            .into(),
                    },
                },
                unmeasured(Metric::Ece),
                unmeasured(Metric::Brier),
                unmeasured(Metric::Nll),
            ],
            effect_size_sigmas: Bound {
                value: Some(2.0),
                basis: Basis::Convention,
                why: "Two standard deviations is the conventional bar for calling a difference \
                      real. It is a convention, and it is labelled one; the quantity it \
                      multiplies is the measurement. At one block a side it gives 0.056 \
                      accuracy, 7.2% of the 0.781 mean, and at three blocks a side it gives \
                      0.032. The reference implementation's 10% guard is not carried over: on \
                      this suite it is 2.8 of these standard deviations, which is the right \
                      order by coincidence rather than by measurement."
                    .into(),
            },
            family_regression_sigmas: Bound {
                value: Some(2.0),
                basis: Basis::Convention,
                why: "The same bar the win has to clear, applied to a loss, so a candidate \
                      cannot win overall on a move smaller than the one that refuses it on a \
                      family. The standard deviation it multiplies was measured over the whole \
                      98-item suite, not per family: the same run reports 5 of 50 routing items \
                      changing answer across blocks against 12 of 18 severity items, so the \
                      suite figure understates severity's spread and the guard over-refuses \
                      there. For a guard, over-refusing is the safe direction, and the \
                      substitution is named in every detail line it produces."
                    .into(),
            },
            min_blocks_per_side: Bound {
                value: Some(3.0),
                basis: Basis::Derived,
                why: "The median of one block is that block, and the median of two is their \
                      mean, so the criterion that the median agrees with the mean has no \
                      content below three blocks. Three is the fewest at which the two can \
                      disagree."
                    .into(),
            },
            requeue_limit: 1,
            covers: "Seed resampling: the spread you would see if only the seed block changed."
                .into(),
            does_not_cover: "Item sampling. The blocks hold the item set fixed, and the \
                             binomial standard error of an accuracy near 0.78 on 98 items is \
                             about 0.042, roughly twice the seed noise. A win that survives a \
                             fresh block has been shown to be more than a lucky draw; it has \
                             not been shown to generalize to items this suite does not contain."
                .into(),
            pending_measurements: vec![
                Pending {
                    quantity: "the block-to-block spread of ECE, Brier, and log loss".into(),
                    issue: "openagents#9370 measured accuracy and the mean top share only".into(),
                    why: "Their floors stay unmeasured, so a win on them reports unverifiable \
                          and can never earn a keep."
                        .into(),
                },
                Pending {
                    quantity: "the noise floor of the flip rate".into(),
                    issue: "openagents#9375".into(),
                    why: "Order sensitivity falling from 0.120 to 0.040 is 6 flips against 2 on \
                          50 items, and nothing has said how wide that interval is. The flip \
                          rate is not a metric here, because it is not in the scores this rule \
                          reads; the gap is recorded so it travels with the rule."
                        .into(),
                },
                Pending {
                    quantity: "a per-family block-to-block spread".into(),
                    issue: "openagents#9370 reports per-family item movement, not per-family \
                            accuracy spread"
                        .into(),
                    why: "The family guard multiplies the suite floor instead, which \
                          over-refuses on the noisier families. No per-family number is \
                          invented."
                        .into(),
                },
                Pending {
                    quantity: "the spread of a median over three to eight blocks".into(),
                    issue: "not scheduled".into(),
                    why: "The median criterion therefore asks only that the median move the \
                          way the mean moved. It sets no size, and it does not pretend to."
                        .into(),
                },
            ],
        }
    }

    /// Rejects a rule that says something impossible.
    ///
    /// # Errors
    ///
    /// Returns the first contradiction found.
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.id.trim().is_empty() {
            return Err(RuleError::NoId);
        }
        if self.metric_order.is_empty() {
            return Err(RuleError::NoMetrics);
        }
        let mut seen = BTreeSet::new();
        for floor in &self.metric_order {
            if floor.metric == Metric::ConfidentErrors {
                return Err(RuleError::ConfidentErrorsEarnAWin);
            }
            if !seen.insert(floor.metric) {
                return Err(RuleError::DuplicateMetric {
                    metric: floor.metric,
                });
            }
            check_bound(&floor.block_sigma, &format!("{} block_sigma", floor.metric))?;
        }
        check_bound(&self.effect_size_sigmas, "effect_size_sigmas")?;
        check_bound(&self.family_regression_sigmas, "family_regression_sigmas")?;
        check_bound(&self.min_blocks_per_side, "min_blocks_per_side")?;
        Ok(())
    }

    /// `gate:<sha256>` over the canonical serialization of the rule.
    ///
    /// Object keys are sorted, so the digest does not move when fields are
    /// reordered. Everything the rule says is inside, including each bound's
    /// basis, its provenance, and the measurements the rule declines to make
    /// up: a threshold whose justification changed is a different rule.
    ///
    /// The prefix is `gate:` because this is a rule a row can pin, in the
    /// same field [`crate::row::Row::gate_digest`] holds for a
    /// [`crate::gate::Gate`].
    #[must_use]
    pub fn digest(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        let mut canonical = String::new();
        canonicalize(&value, &mut canonical);
        let hash = Sha256::digest(canonical.as_bytes());
        let mut out = String::with_capacity(5 + hash.len() * 2);
        out.push_str("gate:");
        for byte in hash {
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    /// The metrics a win may be earned on, in order.
    #[must_use]
    pub fn metrics(&self) -> Vec<Metric> {
        self.metric_order.iter().map(|floor| floor.metric).collect()
    }

    /// The measured block-to-block spread of one metric, when there is one.
    #[must_use]
    pub fn block_sigma(&self, metric: Metric) -> Option<f64> {
        self.metric_order
            .iter()
            .find(|floor| floor.metric == metric)
            .and_then(|floor| floor.block_sigma.value())
    }

    /// The difference a win on `metric` has to clear, given the blocks behind
    /// each side.
    ///
    /// `sigma * sqrt(1/baseline + 1/candidate)` is the standard deviation of
    /// the difference between two block means, and the bound is the rule's
    /// multiple of it. `None` when the metric has no measured spread, or when
    /// a side ran no blocks.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn effect_size(&self, metric: Metric, baseline: usize, candidate: usize) -> Option<f64> {
        let sigma = self.block_sigma(metric)?;
        let multiple = self.effect_size_sigmas.value()?;
        if baseline == 0 || candidate == 0 {
            return None;
        }
        let spread = (1.0 / baseline as f64 + 1.0 / candidate as f64).sqrt();
        Some(multiple * sigma * spread)
    }

    /// The loss on one family that refuses the round.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn family_allowance(
        &self,
        metric: Metric,
        baseline: usize,
        candidate: usize,
    ) -> Option<f64> {
        let sigma = self.block_sigma(metric)?;
        let multiple = self.family_regression_sigmas.value()?;
        if baseline == 0 || candidate == 0 {
            return None;
        }
        let spread = (1.0 / baseline as f64 + 1.0 / candidate as f64).sqrt();
        Some(multiple * sigma * spread)
    }

    /// Judges a screening round, and proposes the metric it was won on.
    #[must_use]
    pub fn screen(&self, round: &Round) -> Stage {
        self.assess(round, &Mode::Screening)
    }

    /// Judges a confirmation round against the metric the screening earned.
    ///
    /// `screening_blocks` is what the screening drew. A confirmation that
    /// reuses any of them is refused: the doors reproduce a block exactly, so
    /// rerunning one is the same arithmetic and not a second witness.
    #[must_use]
    pub fn confirm(&self, round: &Round, earned: Metric, screening: &Round) -> Stage {
        self.assess(round, &Mode::Confirmation { earned, screening })
    }
}

/// Which question a round is being asked.
enum Mode<'a> {
    Screening,
    Confirmation {
        earned: Metric,
        screening: &'a Round,
    },
}

fn check_bound(bound: &Bound, name: &str) -> Result<(), RuleError> {
    let unmeasured = bound.basis == Basis::Unmeasured;
    let problem = match (bound.value, unmeasured) {
        (Some(_), true) => Some(
            "carries a value and an unmeasured basis; drop the value or name the basis that \
             backs it"
                .to_string(),
        ),
        (None, false) => Some(format!(
            "has no value and a {} basis; record it as unmeasured",
            bound.basis
        )),
        (Some(value), false) if !value.is_finite() || value <= 0.0 => {
            Some(format!("must be a finite number above zero, got {value}"))
        }
        _ if bound.why.trim().is_empty() => {
            Some("has no provenance; say where the number came from".to_string())
        }
        _ => None,
    };
    match problem {
        Some(problem) => Err(RuleError::Bound {
            name: name.to_string(),
            problem,
        }),
        None => Ok(()),
    }
}

/// One scheduled unit of work: one side, one family, one seed block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// Its position in the schedule, from 1. A requeued cell gets a new
    /// index at the end rather than reusing the one that failed.
    pub index: usize,
    /// Which half of the loop.
    pub phase: Phase,
    /// The seed block this cell draws.
    pub block: u64,
    /// The question family.
    pub family: String,
    /// Which door.
    pub side: Side,
    /// 1, or 2 after a requeue.
    pub attempt: u32,
    /// The index of the attempt this one replaces.
    pub requeued_from: Option<usize>,
}

/// What one cell is scheduled over.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Which half of the loop.
    pub phase: Phase,
    /// The question families, in the suite's own order. This is the
    /// regression-guard axis.
    pub families: Vec<String>,
    /// The seed blocks, one trial each.
    pub blocks: Vec<u64>,
}

impl Plan {
    /// A plan over the named families and blocks.
    #[must_use]
    pub fn new(
        phase: Phase,
        families: impl IntoIterator<Item = impl Into<String>>,
        blocks: impl IntoIterator<Item = u64>,
    ) -> Self {
        Self {
            phase,
            families: families.into_iter().map(Into::into).collect(),
            blocks: blocks.into_iter().collect(),
        }
    }

    /// The confirmation plan for this screening: the same families, and the
    /// same number of blocks starting after the highest one drawn.
    ///
    /// Disjointness is the whole point of the stage, and the arithmetic that
    /// guarantees it belongs here rather than in a caller's head.
    #[must_use]
    pub fn confirmation(&self) -> Self {
        let next = self
            .blocks
            .iter()
            .copied()
            .max()
            .map_or(0, |block| block + 1);
        Self {
            phase: Phase::Confirmation,
            families: self.families.clone(),
            blocks: (0..self.blocks.len() as u64)
                .map(|offset| next + offset)
                .collect(),
        }
    }

    /// The schedule, with the two sides interleaved.
    ///
    /// Within each block, the family's position and the block's position
    /// decide which side goes first, so a machine that drifts over the round
    /// does not favour one side. The schedule is a pure function of the plan.
    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::with_capacity(self.blocks.len() * self.families.len() * 2);
        for (trial, &block) in self.blocks.iter().enumerate() {
            for (position, family) in self.families.iter().enumerate() {
                let order = if (trial + position) % 2 == 0 {
                    [Side::Control, Side::Candidate]
                } else {
                    [Side::Candidate, Side::Control]
                };
                for side in order {
                    cells.push(Cell {
                        index: cells.len() + 1,
                        phase: self.phase,
                        block,
                        family: family.clone(),
                        side,
                        attempt: 1,
                        requeued_from: None,
                    });
                }
            }
        }
        cells
    }
}

/// What one cell produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Attempt {
    /// The door answered the items it was given, and the scores say how well.
    ///
    /// A door refusal is inside this, not beside it: `refusals` counts the
    /// items the door declined, and those items stay in the denominator
    /// behind [`Scores::accuracy`]. That is the only arrangement under which
    /// a door cannot score better by declining the questions it finds hard.
    Scored {
        /// What the door scored on the family's items.
        scores: Scores,
        /// How many of those items the door declined to answer.
        refusals: usize,
    },
    /// The harness failed: a lost connection, a timeout in the client, a
    /// body that does not parse. Neither the door's fault nor its credit. The
    /// cell is requeued and this record never enters a comparison.
    HarnessFailure {
        /// What failed, in one line.
        detail: String,
    },
}

impl Attempt {
    /// The scores, when the cell produced any.
    #[must_use]
    pub const fn scores(&self) -> Option<&Scores> {
        match self {
            Self::Scored { scores, .. } => Some(scores),
            Self::HarnessFailure { .. } => None,
        }
    }

    /// Whether this attempt may enter a comparison.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Scored { .. })
    }
}

/// One cell, and what it produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    /// The cell that ran.
    pub cell: Cell,
    /// What it produced.
    pub attempt: Attempt,
}

/// Everything one round ran, valid and not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Round {
    /// Which half of the loop.
    pub phase: Phase,
    /// The families scheduled.
    pub families: Vec<String>,
    /// The seed blocks scheduled.
    pub blocks: Vec<u64>,
    /// The attempts that may be compared, in the order they ran.
    pub trials: Vec<Trial>,
    /// The attempts that may not: every harness failure, kept so the round
    /// can say what it lost rather than quietly reporting a smaller batch.
    pub invalid: Vec<Trial>,
    /// How many cells were requeued.
    pub requeued: usize,
}

impl Round {
    /// Runs a plan, interleaved, requeueing a harness failure once.
    ///
    /// `attempt` runs one cell. Returning [`Attempt::HarnessFailure`]
    /// requeues the cell at the end of the schedule, up to
    /// [`Rule::requeue_limit`] times; the failed record is kept in
    /// [`Round::invalid`] and never enters a comparison.
    pub fn run<F>(plan: &Plan, rule: &Rule, mut attempt: F) -> Self
    where
        F: FnMut(&Cell) -> Attempt,
    {
        let mut pending: VecDeque<Cell> = plan.cells().into();
        let mut next_index = pending.len();
        let mut trials = Vec::new();
        let mut invalid = Vec::new();
        let mut requeued = 0;
        while let Some(cell) = pending.pop_front() {
            let outcome = attempt(&cell);
            if outcome.is_valid() {
                trials.push(Trial {
                    cell,
                    attempt: outcome,
                });
                continue;
            }
            if cell.attempt <= rule.requeue_limit {
                next_index += 1;
                pending.push_back(Cell {
                    index: next_index,
                    attempt: cell.attempt + 1,
                    requeued_from: Some(cell.index),
                    ..cell.clone()
                });
                requeued += 1;
            }
            invalid.push(Trial {
                cell,
                attempt: outcome,
            });
        }
        Self {
            phase: plan.phase,
            families: plan.families.clone(),
            blocks: plan.blocks.clone(),
            trials,
            invalid,
            requeued,
        }
    }

    /// How many cells the plan asked for.
    #[must_use]
    pub fn expected(&self) -> usize {
        self.families.len() * self.blocks.len() * 2
    }

    /// The distinct seed blocks one side actually produced a valid attempt
    /// on.
    #[must_use]
    pub fn blocks_for(&self, side: Side) -> Vec<u64> {
        let blocks: BTreeSet<u64> = self
            .trials
            .iter()
            .filter(|trial| trial.cell.side == side)
            .map(|trial| trial.cell.block)
            .collect();
        blocks.into_iter().collect()
    }

    /// The seed blocks any valid attempt drew.
    #[must_use]
    pub fn drawn(&self) -> Vec<u64> {
        let blocks: BTreeSet<u64> = self.trials.iter().map(|trial| trial.cell.block).collect();
        blocks.into_iter().collect()
    }

    /// One side's summary over one family, or over the whole suite when
    /// `group` is [`OVERALL`].
    #[must_use]
    pub fn summarize(&self, side: Side, group: &str) -> Summary {
        let mut by_block: BTreeMap<u64, Vec<(Scores, usize)>> = BTreeMap::new();
        for trial in &self.trials {
            if trial.cell.side != side {
                continue;
            }
            if group != OVERALL && trial.cell.family != group {
                continue;
            }
            if let Attempt::Scored { scores, refusals } = &trial.attempt {
                by_block
                    .entry(trial.cell.block)
                    .or_default()
                    .push((*scores, *refusals));
            }
        }
        let mut blocks = Vec::new();
        let mut pooled = Vec::new();
        let mut refusals = 0;
        for (block, measurements) in by_block {
            let (scores, refused) = pool(&measurements);
            blocks.push(block);
            refusals += refused;
            pooled.push(scores);
        }
        let items = pooled
            .first()
            .map(|first| first.items)
            .filter(|first| pooled.iter().all(|scores| scores.items == *first));
        let metrics = Metric::ALL
            .iter()
            .map(|&metric| MetricStat {
                metric,
                stat: Stat::over(metric, &pooled),
            })
            .collect();
        Summary {
            side,
            group: group.to_string(),
            blocks,
            items,
            refusals,
            metrics,
        }
    }

    /// Every summary this round supports: both sides, the whole suite, and
    /// each family.
    #[must_use]
    pub fn summaries(&self) -> Vec<Summary> {
        let mut out = Vec::new();
        for side in Side::BOTH {
            out.push(self.summarize(side, OVERALL));
            for family in &self.families {
                out.push(self.summarize(side, family));
            }
        }
        out
    }
}

/// One metric's values across the blocks one side drew.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stat {
    /// The value per block, in block order.
    pub values: Vec<f64>,
    /// The mean over blocks.
    pub mean: f64,
    /// The median over blocks.
    pub median: f64,
}

impl Stat {
    /// The statistic for one metric, or `None` when any block is missing it.
    ///
    /// A mean over the blocks that happened to report a number is a different
    /// measurement from a mean over the blocks that ran, and the two must not
    /// be confused.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn over(metric: Metric, pooled: &[Scores]) -> Option<Self> {
        if pooled.is_empty() {
            return None;
        }
        let values: Option<Vec<f64>> = pooled.iter().map(|scores| metric.read(scores)).collect();
        let values = values?;
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        let middle = sorted.len() / 2;
        let median = if sorted.len() % 2 == 0 {
            (sorted[middle - 1] + sorted[middle]) / 2.0
        } else {
            sorted[middle]
        };
        Some(Self {
            values,
            mean,
            median,
        })
    }
}

/// One metric's statistic, or the record that nobody measured it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricStat {
    /// The metric.
    pub metric: Metric,
    /// Its values across blocks. `None` is unknown, never zero.
    pub stat: Option<Stat>,
}

/// One side's numbers over one group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Which door.
    pub side: Side,
    /// The family, or [`OVERALL`].
    pub group: String,
    /// The seed blocks behind these numbers.
    pub blocks: Vec<u64>,
    /// How many items each block scored. `None` when the blocks disagree,
    /// which means they did not score the same thing and cannot be averaged.
    pub items: Option<usize>,
    /// How many items the door declined, across every block.
    pub refusals: usize,
    /// One entry per metric, in [`Metric::ALL`] order.
    pub metrics: Vec<MetricStat>,
}

impl Summary {
    /// One metric's statistic.
    #[must_use]
    pub fn stat(&self, metric: Metric) -> Option<&Stat> {
        self.metrics
            .iter()
            .find(|entry| entry.metric == metric)
            .and_then(|entry| entry.stat.as_ref())
    }
}

/// One round, judged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    /// Which half of the loop.
    pub phase: Phase,
    /// The verdict, with every criterion and the rule that produced it.
    pub outcome: Outcome,
    /// The metric this round was won on, when one earned it. On a
    /// confirmation this is the metric the screening earned, repeated.
    pub earned: Option<Metric>,
    /// Both sides, over the suite and over each family.
    pub summaries: Vec<Summary>,
}

impl Stage {
    /// One side's summary over one group.
    #[must_use]
    pub fn summary(&self, side: Side, group: &str) -> Option<&Summary> {
        self.summaries
            .iter()
            .find(|summary| summary.side == side && summary.group == group)
    }
}

/// The experiment: two doors, one suite, one rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Experiment {
    /// The suite both sides answered.
    pub suite: String,
    /// The suite's content digest, so a suite edit cannot read as a door
    /// improvement.
    pub suite_digest: String,
    /// The door in service, by the name the round used for it.
    pub control: String,
    /// The door proposed to replace it.
    pub candidate: String,
    /// When the round ran, as an RFC 3339 timestamp in UTC.
    pub recorded_at: String,
    /// The rule in force.
    pub rule: Rule,
}

impl Experiment {
    /// Runs the loop and returns the evidence.
    ///
    /// The screening runs first. It runs the confirmation only when the
    /// screening earned a win: a round that already lost has nothing to
    /// confirm, and spending a fresh block on it would spend the one thing
    /// the confirmation has.
    pub fn run<F>(&self, screening: &Plan, confirmation: &Plan, mut attempt: F) -> Evidence
    where
        F: FnMut(&Cell) -> Attempt,
    {
        let screening_round = Round::run(screening, &self.rule, &mut attempt);
        let screened = self.rule.screen(&screening_round);
        // A stage keeps its earning metric only when every criterion held, so
        // this is also the test that the screening passed.
        let mut confirmed = None;
        if let Some(earned) = screened.earned {
            let round = Round::run(confirmation, &self.rule, &mut attempt);
            confirmed = Some(self.rule.confirm(&round, earned, &screening_round));
        }
        let verdict = Verdict::over(
            std::iter::once(screened.outcome.verdict)
                .chain(confirmed.iter().map(|stage| stage.outcome.verdict)),
        );
        let last = confirmed.as_ref().unwrap_or(&screened);
        let reason = last.outcome.deciding().map_or_else(
            || format!("{} judged nothing", last.phase),
            |criterion| format!("{}: {}", last.phase, criterion.detail),
        );
        Evidence {
            schema: SCHEMA.to_string(),
            recorded_at: self.recorded_at.clone(),
            suite: self.suite.clone(),
            suite_digest: self.suite_digest.clone(),
            control: self.control.clone(),
            candidate: self.candidate.clone(),
            rule_digest: self.rule.digest(),
            metric_order: self.rule.metrics(),
            rule: self.rule.clone(),
            screening: screened,
            confirmation: confirmed,
            decision: Decision::of(verdict),
            reason,
            screening_round,
        }
    }
}

/// What one A/B produced, with the rule it was judged by inside it.
///
/// A reader of this document never has to guess what bar applied: the whole
/// [`Rule`] is here, its digest is here, the metric order is here, and every
/// [`Criterion`] names the numbers behind its verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// What this document is.
    pub schema: String,
    /// When the round ran.
    pub recorded_at: String,
    /// The suite both sides answered.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// The door in service.
    pub control: String,
    /// The door proposed to replace it.
    pub candidate: String,
    /// The rule in force, thresholds, provenance, and gaps included.
    pub rule: Rule,
    /// That rule's content digest, which every outcome here repeats.
    pub rule_digest: String,
    /// The metrics a win could be earned on, most decisive first.
    pub metric_order: Vec<Metric>,
    /// Everything the screening ran.
    pub screening_round: Round,
    /// The screening, judged.
    pub screening: Stage,
    /// The confirmation, judged, when there was one to run.
    pub confirmation: Option<Stage>,
    /// What to do about it.
    pub decision: Decision,
    /// The deciding criterion's detail, in one line.
    pub reason: String,
}

impl Evidence {
    /// The metric the round was earned on, when it was earned.
    #[must_use]
    pub fn earned(&self) -> Option<Metric> {
        self.confirmation
            .as_ref()
            .map_or(self.screening.earned, |stage| stage.earned)
    }
}

// The judging.

impl Rule {
    #[allow(clippy::too_many_lines)]
    fn assess(&self, round: &Round, mode: &Mode<'_>) -> Stage {
        let summaries = round.summaries();
        let mut criteria: Vec<Criterion> = Vec::new();
        let mut blocked: Option<String> = None;

        let expected = round.expected();
        let valid = round.trials.len();
        if expected == 0 {
            let reason = "the plan named no families or no seed blocks".to_string();
            criteria.push(unverifiable(
                "every_cell_has_a_valid_attempt",
                1,
                reason.clone(),
            ));
            blocked = Some(reason);
        } else if valid == expected {
            criteria.push(passed(
                "every_cell_has_a_valid_attempt",
                1,
                format!(
                    "{expected} cells ran, {} requeued after a harness failure and {} left \
                     out of the comparison",
                    round.requeued,
                    round.invalid.len()
                ),
            ));
        } else {
            let reason = format!("{valid} of {expected} cells produced a valid attempt");
            criteria.push(unverifiable(
                "every_cell_has_a_valid_attempt",
                1,
                format!(
                    "{reason}; a harness failure requeues once and never enters a comparison, \
                     and {} did not come back",
                    round.invalid.len()
                ),
            ));
            blocked = Some(reason);
        }

        let control_blocks = round.blocks_for(Side::Control).len();
        let candidate_blocks = round.blocks_for(Side::Candidate).len();
        let (floor, floor_reason) =
            blocks_floor(&self.min_blocks_per_side, control_blocks, candidate_blocks);
        criteria.push(floor);
        blocked = blocked.or(floor_reason);

        if let Mode::Confirmation { screening, .. } = mode {
            criteria.push(freshness(round, screening));
            criteria.push(same_families(round, screening));
        }

        criteria.push(same_items(&summaries, blocked.as_deref()));
        criteria.push(confident_errors(&summaries, blocked.as_deref()));
        criteria.push(self.family_guard(round, &summaries, blocked.as_deref()));

        let earned = match mode {
            Mode::Screening => {
                let (criterion, earned) = self.screening_win(
                    &summaries,
                    blocked.as_deref(),
                    round.families.len(),
                    control_blocks,
                    candidate_blocks,
                );
                criteria.push(criterion);
                earned
            }
            Mode::Confirmation { earned, .. } => {
                criteria.push(self.repeat(
                    *earned,
                    &summaries,
                    blocked.as_deref(),
                    round.families.len(),
                    control_blocks,
                    candidate_blocks,
                ));
                Some(*earned)
            }
        };

        criteria.push(median_agrees(earned, &summaries, blocked.as_deref()));

        let earned =
            earned.filter(|_| {
                Verdict::over(criteria.iter().map(|criterion| criterion.verdict))
                    == Verdict::Passed
            });
        Stage {
            phase: round.phase,
            outcome: Outcome {
                gate_id: self.id.clone(),
                gate_digest: self.digest(),
                group: round.phase.as_str().to_string(),
                verdict: Verdict::over(criteria.iter().map(|criterion| criterion.verdict)),
                criteria,
            },
            earned,
            summaries,
        }
    }

    fn screening_win(
        &self,
        summaries: &[Summary],
        blocked: Option<&str>,
        families: usize,
        control_blocks: usize,
        candidate_blocks: usize,
    ) -> (Criterion, Option<Metric>) {
        let name = "a_metric_gain_clears_the_noise";
        if let Some(reason) = blocked {
            return (not_judged(name, 2, reason), None);
        }
        let mut lines = Vec::new();
        for floor in &self.metric_order {
            let metric = floor.metric;
            match self.read_gain(
                metric,
                summaries,
                families,
                control_blocks,
                candidate_blocks,
            ) {
                Reading::Unmeasured => lines.push(format!(
                    "{metric} has no measured block-to-block spread on this suite, so a win on \
                     it cannot be told from the seeds"
                )),
                Reading::Missing { reason } => lines.push(reason),
                Reading::Measured {
                    gain,
                    bound,
                    before,
                    after,
                } => {
                    lines.push(format!(
                        "{metric} {before:.3} to {after:.3}, a gain of {gain:+.3} against a \
                         bound of {bound:.3}"
                    ));
                    if gain >= bound {
                        return (
                            Criterion {
                                name: name.to_string(),
                                rank: 2,
                                verdict: Verdict::Passed,
                                detail: format!(
                                    "won on {metric}: {}. The bound covers seed resampling \
                                     only.",
                                    lines.join("; ")
                                ),
                            },
                            Some(metric),
                        );
                    }
                }
            }
        }
        (
            Criterion {
                name: name.to_string(),
                rank: 2,
                verdict: Verdict::Unverifiable,
                detail: format!("no metric earned the round: {}", lines.join("; ")),
            },
            None,
        )
    }

    fn repeat(
        &self,
        earned: Metric,
        summaries: &[Summary],
        blocked: Option<&str>,
        families: usize,
        control_blocks: usize,
        candidate_blocks: usize,
    ) -> Criterion {
        let name = format!("the_win_repeats_on_{earned}");
        if let Some(reason) = blocked {
            return not_judged(&name, 1, reason);
        }
        let substitutes: Vec<String> = self
            .metric_order
            .iter()
            .map(|floor| floor.metric)
            .filter(|metric| *metric != earned)
            .filter_map(|metric| {
                match self.read_gain(
                    metric,
                    summaries,
                    families,
                    control_blocks,
                    candidate_blocks,
                ) {
                    Reading::Measured { gain, bound, .. } if gain >= bound => Some(format!(
                        "{metric} cleared instead, gaining {gain:+.3} against \
                                      {bound:.3}"
                    )),
                    _ => None,
                }
            })
            .collect();
        let instead = if substitutes.is_empty() {
            String::new()
        } else {
            format!(
                "; {}, and a different metric cannot be substituted",
                substitutes.join("; and ")
            )
        };
        match self.read_gain(
            earned,
            summaries,
            families,
            control_blocks,
            candidate_blocks,
        ) {
            Reading::Unmeasured => Criterion {
                name,
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!("{earned} has no measured block-to-block spread{instead}"),
            },
            Reading::Missing { reason } => Criterion {
                name,
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!("{reason}{instead}"),
            },
            Reading::Measured {
                gain,
                bound,
                before,
                after,
            } => {
                let held = gain >= bound;
                Criterion {
                    name,
                    rank: 1,
                    verdict: if held {
                        Verdict::Passed
                    } else {
                        Verdict::Failed
                    },
                    detail: if held {
                        format!(
                            "the screening won on {earned}, and on fresh blocks {earned} went \
                             {before:.3} to {after:.3}, a gain of {gain:+.3} against a bound of \
                             {bound:.3}"
                        )
                    } else {
                        format!(
                            "the screening won on {earned}, and on fresh blocks {earned} went \
                             {before:.3} to {after:.3}, a gain of {gain:+.3} against a bound of \
                             {bound:.3}{instead}"
                        )
                    },
                }
            }
        }
    }

    fn family_guard(
        &self,
        round: &Round,
        summaries: &[Summary],
        blocked: Option<&str>,
    ) -> Criterion {
        let name = "no_family_loses_more_than_the_noise";
        if let Some(reason) = blocked {
            return not_judged(name, 1, reason);
        }
        let mut guarded = Vec::new();
        let mut losses = Vec::new();
        let mut unknown = Vec::new();
        for family in &round.families {
            let (Some(control), Some(candidate)) = (
                find(summaries, Side::Control, family),
                find(summaries, Side::Candidate, family),
            ) else {
                unknown.push(format!("{family} was not summarized on both sides"));
                continue;
            };
            for floor in &self.metric_order {
                let metric = floor.metric;
                let Some(allowance) =
                    self.family_allowance(metric, control.blocks.len(), candidate.blocks.len())
                else {
                    continue;
                };
                guarded.push(metric.to_string());
                let (Some(before), Some(after)) = (control.stat(metric), candidate.stat(metric))
                else {
                    unknown.push(format!("{family} {metric} was not measured on both sides"));
                    continue;
                };
                let gain = metric.gain(before.mean, after.mean);
                if -gain > allowance {
                    losses.push(format!(
                        "{family} {metric} {:.3} to {:.3}, a loss of {:.3} against an allowance \
                         of {allowance:.3}",
                        before.mean, after.mean, -gain
                    ));
                }
            }
        }
        let scope = if guarded.is_empty() {
            "no metric has a measured spread, so the guard has nothing to apply".to_string()
        } else {
            let mut unique: Vec<String> = guarded.clone();
            unique.dedup();
            format!(
                "guarded on {}, at the suite's measured spread rather than each family's, \
                 which nobody has measured",
                unique.join(" and ")
            )
        };
        if !losses.is_empty() {
            return Criterion {
                name: name.to_string(),
                rank: 1,
                verdict: Verdict::Failed,
                detail: format!("{}; {scope}", losses.join("; ")),
            };
        }
        if guarded.is_empty() || !unknown.is_empty() {
            return Criterion {
                name: name.to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: if unknown.is_empty() {
                    scope
                } else {
                    unknown.join("; ")
                },
            };
        }
        Criterion {
            name: name.to_string(),
            rank: 1,
            verdict: Verdict::Passed,
            detail: format!(
                "no family lost more than the noise allows over {} families; {scope}",
                round.families.len()
            ),
        }
    }

    fn read_gain(
        &self,
        metric: Metric,
        summaries: &[Summary],
        families: usize,
        control_blocks: usize,
        candidate_blocks: usize,
    ) -> Reading {
        let Some(bound) = self.effect_size(metric, control_blocks, candidate_blocks) else {
            return Reading::Unmeasured;
        };
        let missing = || {
            if metric == Metric::Ece && families > 1 {
                Reading::Missing {
                    reason: format!(
                        "{metric} is computed inside bins, so the suite's is not the \
                         item-weighted mean of its {families} families' and this round pools \
                         none; a win on it would have to be judged per family"
                    ),
                }
            } else {
                Reading::Missing {
                    reason: format!("{metric} was not measured on both sides"),
                }
            }
        };
        let (Some(control), Some(candidate)) = (
            find(summaries, Side::Control, OVERALL),
            find(summaries, Side::Candidate, OVERALL),
        ) else {
            return missing();
        };
        let (Some(before), Some(after)) = (control.stat(metric), candidate.stat(metric)) else {
            return missing();
        };
        Reading::Measured {
            gain: metric.gain(before.mean, after.mean),
            bound,
            before: before.mean,
            after: after.mean,
        }
    }
}

enum Reading {
    /// The metric has no measured block-to-block spread.
    Unmeasured,
    /// The suite has no number for it, and this is why.
    Missing { reason: String },
    /// Both sides reported it, and there is a bound to judge it against.
    Measured {
        gain: f64,
        bound: f64,
        before: f64,
        after: f64,
    },
}

fn blocks_floor(bound: &Bound, control: usize, candidate: usize) -> (Criterion, Option<String>) {
    let Some(floor) = bound.count() else {
        let reason = format!(
            "no floor has been measured for blocks per side ({})",
            bound.why
        );
        return (
            unverifiable("blocks_per_side>=?", 1, reason.clone()),
            Some(reason),
        );
    };
    let name = format!("blocks_per_side>={floor}");
    let fewest = control.min(candidate);
    if fewest < floor {
        let reason =
            format!("the thinner side drew {fewest} seed blocks, below the floor of {floor}");
        return (
            unverifiable(
                &name,
                1,
                format!(
                    "{reason}; below three blocks the median is the mean, so the round cannot \
                     tell whether they agree"
                ),
            ),
            Some(reason),
        );
    }
    (
        passed(
            &name,
            1,
            format!(
                "control drew {control} seed blocks and the candidate drew {candidate}, at or \
                 above the floor of {floor}"
            ),
        ),
        None,
    )
}

fn freshness(round: &Round, screening: &Round) -> Criterion {
    let name = "the_confirmation_draws_fresh_blocks";
    let spent: BTreeSet<u64> = screening.drawn().into_iter().collect();
    let reused: Vec<String> = round
        .drawn()
        .into_iter()
        .filter(|block| spent.contains(block))
        .map(|block| block.to_string())
        .collect();
    if reused.is_empty() {
        return passed(
            name,
            1,
            format!(
                "the confirmation drew blocks {} against the screening's {}, and no seed is \
                 shared",
                list(&round.drawn()),
                list(&screening.drawn())
            ),
        );
    }
    Criterion {
        name: name.to_string(),
        rank: 1,
        verdict: Verdict::Failed,
        detail: format!(
            "the confirmation redrew block {}, which the screening already drew; the door \
             reproduces a block exactly, so rerunning one is the same arithmetic and not a \
             second witness",
            reused.join(", ")
        ),
    }
}

fn same_families(round: &Round, screening: &Round) -> Criterion {
    let name = "the_confirmation_scores_the_same_families";
    if round.families == screening.families {
        return passed(
            name,
            1,
            format!("both rounds scored {}", round.families.join(", ")),
        );
    }
    Criterion {
        name: name.to_string(),
        rank: 1,
        verdict: Verdict::Failed,
        detail: format!(
            "the screening scored {} and the confirmation scored {}, so they are not the same \
             experiment",
            screening.families.join(", "),
            round.families.join(", ")
        ),
    }
}

fn same_items(summaries: &[Summary], blocked: Option<&str>) -> Criterion {
    let name = "the_blocks_scored_the_same_items";
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let mut counts = Vec::new();
    for side in Side::BOTH {
        let Some(summary) = find(summaries, side, OVERALL) else {
            return unverifiable(name, 1, format!("{side} produced no summary"));
        };
        let Some(items) = summary.items else {
            return unverifiable(
                name,
                1,
                format!(
                    "{side}'s blocks scored different numbers of items, so they did not score \
                     the same thing and cannot be averaged"
                ),
            );
        };
        counts.push((side, items, summary.refusals));
    }
    if counts[0].1 != counts[1].1 {
        return unverifiable(
            name,
            1,
            format!(
                "control scored {} items a block and the candidate scored {}, so the two sides \
                 answered different questions",
                counts[0].1, counts[1].1
            ),
        );
    }
    passed(
        name,
        1,
        format!(
            "both sides scored {} items in every block; the control declined {} of them across \
             its blocks and the candidate declined {}, and a declined item stays in the \
             denominator",
            counts[0].1, counts[0].2, counts[1].2
        ),
    )
}

fn confident_errors(summaries: &[Summary], blocked: Option<&str>) -> Criterion {
    let name = "confident_errors_do_not_rise";
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let (Some(control), Some(candidate)) = (
        find(summaries, Side::Control, OVERALL),
        find(summaries, Side::Candidate, OVERALL),
    ) else {
        return unverifiable(name, 1, "one side produced no summary".to_string());
    };
    let (Some(before), Some(after)) = (
        control.stat(Metric::ConfidentErrors),
        candidate.stat(Metric::ConfidentErrors),
    ) else {
        return unverifiable(
            name,
            1,
            "confident errors were not counted on both sides, and unknown is never zero".into(),
        );
    };
    let detail = format!(
        "confident errors {:.2} to {:.2} a block. This is a direction and not an effect size: \
         at this suite's counts, a relative threshold would be a threshold on one item.",
        before.mean, after.mean
    );
    Criterion {
        name: name.to_string(),
        rank: 1,
        verdict: if after.mean > before.mean {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        detail,
    }
}

fn median_agrees(
    earned: Option<Metric>,
    summaries: &[Summary],
    blocked: Option<&str>,
) -> Criterion {
    let name = "the_median_agrees_with_the_mean";
    if let Some(reason) = blocked {
        return not_judged(name, 2, reason);
    }
    let Some(metric) = earned else {
        return not_judged(name, 2, "no metric earned the round");
    };
    let (Some(control), Some(candidate)) = (
        find(summaries, Side::Control, OVERALL),
        find(summaries, Side::Candidate, OVERALL),
    ) else {
        return unverifiable(name, 2, "one side produced no summary".to_string());
    };
    let (Some(before), Some(after)) = (control.stat(metric), candidate.stat(metric)) else {
        return unverifiable(name, 2, format!("{metric} was not measured on both sides"));
    };
    let mean_gain = metric.gain(before.mean, after.mean);
    let median_gain = metric.gain(before.median, after.median);
    let detail = format!(
        "{metric} mean {:.3} to {:.3}, a gain of {mean_gain:+.3}; median {:.3} to {:.3}, a gain \
         of {median_gain:+.3}. The median carries no measured spread, so the criterion asks for \
         direction and sets no size.",
        before.mean, after.mean, before.median, after.median
    );
    Criterion {
        name: name.to_string(),
        rank: 2,
        verdict: if median_gain > 0.0 {
            Verdict::Passed
        } else {
            Verdict::Failed
        },
        detail,
    }
}

/// Pools one block's families into one set of scores.
///
/// Accuracy, Brier, log loss, and confident errors are means or sums of
/// per-item quantities, so item weighting recovers them exactly. ECE is not:
/// it is computed inside bins, and the suite's ECE is not the item-weighted
/// mean of its families'. Rather than report a number that looks like one and
/// is not, the pooled ECE is left unknown whenever more than one family is
/// pooled. A family's own ECE survives untouched.
#[allow(clippy::cast_precision_loss)]
fn pool(measurements: &[(Scores, usize)]) -> (Scores, usize) {
    let items: usize = measurements.iter().map(|(scores, _)| scores.items).sum();
    let refusals: usize = measurements.iter().map(|(_, refused)| *refused).sum();
    let weighted = |read: fn(&Scores) -> Option<f64>| -> Option<f64> {
        if items == 0 {
            return None;
        }
        let mut total = 0.0;
        for (scores, _) in measurements {
            total += read(scores)? * scores.items as f64;
        }
        Some(total / items as f64)
    };
    let confident_errors = measurements
        .iter()
        .map(|(scores, _)| scores.confident_errors)
        .try_fold(0_usize, |total, count| count.map(|count| total + count));
    let ece = if measurements.len() == 1 {
        measurements[0].0.ece
    } else {
        None
    };
    (
        Scores {
            items,
            accuracy: weighted(|scores| scores.accuracy),
            ece,
            brier: weighted(|scores| scores.brier),
            nll: weighted(|scores| scores.nll),
            confident_errors,
        },
        refusals,
    )
}

fn find<'a>(summaries: &'a [Summary], side: Side, group: &str) -> Option<&'a Summary> {
    summaries
        .iter()
        .find(|summary| summary.side == side && summary.group == group)
}

fn list(blocks: &[u64]) -> String {
    blocks
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn passed(name: &str, rank: u8, detail: String) -> Criterion {
    Criterion {
        name: name.to_string(),
        rank,
        verdict: Verdict::Passed,
        detail,
    }
}

fn unverifiable(name: &str, rank: u8, detail: String) -> Criterion {
    Criterion {
        name: name.to_string(),
        rank,
        verdict: Verdict::Unverifiable,
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

fn canonicalize(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String((*key).to_string()).to_string());
                out.push(':');
                if let Some(inner) = map.get(*key) {
                    canonicalize(inner, out);
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonicalize(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTING: usize = 50;
    const SEVERITY: usize = 18;

    fn families() -> Vec<String> {
        vec!["routing".to_string(), "severity".to_string()]
    }

    fn items_of(family: &str) -> usize {
        if family == "routing" {
            ROUTING
        } else {
            SEVERITY
        }
    }

    /// One cell's scores, with every measure present.
    fn scored(items: usize, accuracy: f64, confident_errors: usize) -> Attempt {
        Attempt::Scored {
            scores: Scores {
                items,
                accuracy: Some(accuracy),
                ece: Some(0.065),
                brier: Some(0.153),
                nll: Some(0.713),
                confident_errors: Some(confident_errors),
            },
            refusals: 0,
        }
    }

    /// The accuracy one door reports for one family on one block.
    type Accuracy = Box<dyn Fn(Side, &str, u64) -> f64>;

    /// A door whose accuracy is a function of its side, family, and block.
    struct Bench {
        accuracy: Accuracy,
        confident_errors: Box<dyn Fn(Side) -> usize>,
    }

    impl Bench {
        /// A flat pair of doors: one accuracy each, every family, every
        /// block.
        fn flat(control: f64, candidate: f64) -> Self {
            Self {
                accuracy: Box::new(move |side, _, _| match side {
                    Side::Control => control,
                    Side::Candidate => candidate,
                }),
                confident_errors: Box::new(|_| 1),
            }
        }

        fn attempt(&self, cell: &Cell) -> Attempt {
            scored(
                items_of(&cell.family),
                (self.accuracy)(cell.side, &cell.family, cell.block),
                (self.confident_errors)(cell.side),
            )
        }
    }

    fn plan(blocks: &[u64]) -> Plan {
        Plan::new(Phase::Screening, families(), blocks.iter().copied())
    }

    fn experiment() -> Experiment {
        Experiment {
            suite: "support-v2".into(),
            suite_digest: "sha256:abc".into(),
            control: "lev-base".into(),
            candidate: "lev-band-v1".into(),
            recorded_at: "2026-09-19T12:00:00Z".into(),
            rule: Rule::v1(),
        }
    }

    fn run(bench: &Bench, screening: &Plan) -> Evidence {
        experiment().run(screening, &screening.confirmation(), |cell| {
            bench.attempt(cell)
        })
    }

    fn criterion_named<'a>(stage: &'a Stage, name: &str) -> &'a Criterion {
        stage
            .outcome
            .criteria
            .iter()
            .find(|criterion| criterion.name == name)
            .unwrap_or_else(|| panic!("{name} is emitted: {:?}", stage.outcome.criteria))
    }

    #[test]
    fn the_schema_tag_belongs_to_this_crate() {
        assert_eq!(SCHEMA, "openagents.gym.ab_evidence.v1");
        assert!(SCHEMA.starts_with(crate::SCHEMA_PREFIX));
    }

    #[test]
    fn the_committed_rule_validates_and_carries_a_digest() {
        let rule = Rule::v1();
        rule.validate().expect("the committed rule is coherent");
        assert!(rule.digest().starts_with("gate:"));
        assert_eq!(rule.digest().len(), 69);
        assert_eq!(
            rule.metrics(),
            vec![Metric::Accuracy, Metric::Ece, Metric::Brier, Metric::Nll]
        );
    }

    #[test]
    fn the_effect_size_is_the_measured_block_spread_and_not_a_carried_constant() {
        // docs/lev/measurements/2026-09-19-seed-variance.md: eight blocks
        // over 98 unchanged items gave an accuracy standard deviation of
        // 0.0197, so a two-door comparison at one block a side needs 0.056 to
        // clear two sigma. Every number here comes from that run.
        let rule = Rule::v1();
        assert_eq!(rule.block_sigma(Metric::Accuracy), Some(0.0197));

        let one = rule
            .effect_size(Metric::Accuracy, 1, 1)
            .expect("one block a side");
        assert!((one - 0.0557).abs() < 0.0005, "{one}");
        // 7.2% of the suite's 0.781 mean accuracy.
        assert!((one / 0.781 - 0.072).abs() < 0.002, "{one}");

        let three = rule
            .effect_size(Metric::Accuracy, 3, 3)
            .expect("three blocks a side");
        assert!((three - 0.0322).abs() < 0.0005, "{three}");
        // The reference implementation's guard, carried onto this suite, is
        // 2.8 standard deviations at one block a side: the right order by
        // coincidence, which is why it is not the rule here.
        let carried = 0.10 * 0.781;
        let sigma_of_the_difference = 0.0197 * f64::sqrt(2.0);
        assert!((carried / sigma_of_the_difference - 2.8).abs() < 0.05);
    }

    #[test]
    fn a_metric_with_no_measured_spread_can_never_earn_a_win() {
        let rule = Rule::v1();
        for metric in [Metric::Ece, Metric::Brier, Metric::Nll] {
            assert_eq!(
                rule.block_sigma(metric),
                None,
                "{metric} has no measured floor"
            );
            assert_eq!(rule.effect_size(metric, 8, 8), None);
        }
        let floors: Vec<&MetricFloor> = rule
            .metric_order
            .iter()
            .filter(|floor| floor.block_sigma.basis == Basis::Unmeasured)
            .collect();
        assert_eq!(
            floors.len(),
            3,
            "three of the four ordered metrics have no floor"
        );
        for floor in floors {
            assert!(floor.block_sigma.value.is_none());
            assert!(
                floor.block_sigma.why.contains("9370") || floor.block_sigma.why.contains("seed")
            );
        }
    }

    #[test]
    fn the_rule_names_the_measurements_it_refuses_to_invent() {
        let rule = Rule::v1();
        let quantities: Vec<&str> = rule
            .pending_measurements
            .iter()
            .map(|pending| pending.quantity.as_str())
            .collect();
        assert!(
            quantities.iter().any(|q| q.contains("flip rate")),
            "{quantities:?}"
        );
        assert!(
            quantities.iter().any(|q| q.contains("per-family")),
            "{quantities:?}"
        );
        assert!(
            quantities.iter().any(|q| q.contains("median")),
            "{quantities:?}"
        );
        let flip = rule
            .pending_measurements
            .iter()
            .find(|pending| pending.quantity.contains("flip rate"))
            .expect("the flip rate gap is recorded");
        assert_eq!(flip.issue, "openagents#9375");

        // The gap is inside the digest, so filling it produces ab-v2 rather
        // than rewriting ab-v1's history.
        let mut filled = rule.clone();
        filled.pending_measurements.clear();
        assert_ne!(rule.digest(), filled.digest());
    }

    #[test]
    fn changing_a_threshold_or_its_provenance_produces_a_new_rule() {
        let rule = Rule::v1();
        let before = rule.digest();

        let mut widened = rule.clone();
        widened.effect_size_sigmas.value = Some(1.0);
        assert_ne!(
            before,
            widened.digest(),
            "a widened bar is a different rule"
        );

        let mut relabelled = rule.clone();
        relabelled.effect_size_sigmas.basis = Basis::Derived;
        assert_ne!(
            before,
            relabelled.digest(),
            "relabelling a constant is a change to the rule"
        );
    }

    #[test]
    fn a_rule_that_lets_confident_errors_earn_a_win_is_refused() {
        let mut rule = Rule::v1();
        rule.metric_order.push(MetricFloor {
            metric: Metric::ConfidentErrors,
            block_sigma: Bound {
                value: Some(0.5),
                basis: Basis::Tuned,
                why: "a number somebody liked".into(),
            },
        });
        assert_eq!(rule.validate(), Err(RuleError::ConfidentErrorsEarnAWin));
    }

    #[test]
    fn interleaving_is_deterministic_and_covers_both_orders() {
        let plan = plan(&[0, 1, 2]);
        let cells = plan.cells();
        assert_eq!(
            cells,
            plan.cells(),
            "the schedule is a pure function of the plan"
        );
        assert_eq!(cells.len(), 12, "two families, three blocks, two sides");

        let firsts: Vec<Side> = cells.chunks(2).map(|pair| pair[0].side).collect();
        assert_eq!(
            firsts,
            vec![
                Side::Control,
                Side::Candidate,
                Side::Candidate,
                Side::Control,
                Side::Control,
                Side::Candidate,
            ],
            "which side runs first alternates by family and by block"
        );
        assert!(firsts.contains(&Side::Control) && firsts.contains(&Side::Candidate));

        // Both sides run the same cells, and the indices are dense.
        for side in Side::BOTH {
            assert_eq!(cells.iter().filter(|cell| cell.side == side).count(), 6);
        }
        assert_eq!(
            cells.iter().map(|cell| cell.index).collect::<Vec<_>>(),
            (1..=12).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_confirmation_plan_draws_blocks_the_screening_did_not() {
        let screening = plan(&[0, 1, 2]);
        let confirmation = screening.confirmation();
        assert_eq!(confirmation.phase, Phase::Confirmation);
        assert_eq!(confirmation.blocks, vec![3, 4, 5]);
        assert_eq!(confirmation.families, screening.families);
    }

    #[test]
    fn a_full_ab_produces_an_evidence_document_carrying_its_own_rule() {
        let evidence = run(&Bench::flat(0.78, 0.86), &plan(&[0, 1, 2]));
        assert_eq!(evidence.decision, Decision::Kept, "{}", evidence.reason);
        assert_eq!(evidence.earned(), Some(Metric::Accuracy));

        // The rule travels inside the evidence: its id, its digest, the
        // metric order, and every threshold in force.
        assert_eq!(evidence.schema, SCHEMA);
        assert_eq!(evidence.rule_digest, Rule::v1().digest());
        assert_eq!(evidence.rule, Rule::v1());
        assert_eq!(evidence.metric_order, Rule::v1().metrics());
        let confirmation = evidence
            .confirmation
            .as_ref()
            .expect("the win was confirmed");
        for stage in [&evidence.screening, confirmation] {
            assert_eq!(stage.outcome.gate_id, "ab-v1");
            assert_eq!(stage.outcome.gate_digest, evidence.rule_digest);
            assert_eq!(stage.outcome.verdict, Verdict::Passed);
        }

        // A reader can recover the bar that applied without leaving the
        // document.
        let win = criterion_named(&evidence.screening, "a_metric_gain_clears_the_noise");
        assert!(win.detail.contains("won on accuracy"), "{}", win.detail);
        assert!(win.detail.contains("0.032"), "{}", win.detail);
        assert!(
            win.detail.contains("seed resampling only"),
            "{}",
            win.detail
        );

        let rendered = serde_json::to_string(&evidence).expect("the evidence serializes");
        assert!(
            rendered.contains("\"block_sigma\""),
            "the thresholds are in the document"
        );
        assert!(
            rendered.contains("0.0197"),
            "the measured floor is in the document"
        );
        let read: Evidence = serde_json::from_str(&rendered).expect("the evidence parses");
        assert_eq!(read, evidence);
    }

    #[test]
    fn a_win_inside_the_noise_is_undecided_rather_than_kept_or_reverted() {
        // Two points of accuracy over three blocks a side is inside a bound
        // of 0.032. The candidate did not lose; nobody can tell that it won.
        let evidence = run(&Bench::flat(0.78, 0.80), &plan(&[0, 1, 2]));
        assert_eq!(
            evidence.decision,
            Decision::Undecided,
            "{}",
            evidence.reason
        );
        assert!(
            evidence.confirmation.is_none(),
            "a round that earned nothing has nothing to confirm"
        );
        let win = criterion_named(&evidence.screening, "a_metric_gain_clears_the_noise");
        assert_eq!(win.verdict, Verdict::Unverifiable);
        assert!(
            win.detail.contains("no metric earned the round"),
            "{}",
            win.detail
        );
        assert!(
            win.detail.contains("has no measured block-to-block spread"),
            "the reader is told which metrics have no floor: {}",
            win.detail
        );
    }

    #[test]
    fn a_win_on_one_metric_confirmed_on_a_different_one_is_reverted() {
        // A rule with two measured floors, so the substitution is
        // expressible. The committed rule has one, which the test above
        // pins; this one exists to exercise the anti-fishing criterion.
        let mut rule = Rule::v1();
        for floor in &mut rule.metric_order {
            if floor.metric == Metric::Brier {
                floor.block_sigma = Bound {
                    value: Some(0.010),
                    basis: Basis::Tuned,
                    why: "A hypothetical floor, used by one test to exercise the rule that a \
                          confirmation may not substitute a metric. Nothing has measured this."
                        .into(),
                };
            }
        }
        let experiment = Experiment {
            rule,
            ..experiment()
        };
        let screening = plan(&[0, 1, 2]);
        let evidence = experiment.run(&screening, &screening.confirmation(), |cell| {
            let accuracy = match (cell.side, cell.phase) {
                (Side::Control, _) => 0.78,
                // The screening wins on accuracy. The confirmation's accuracy
                // gain is inside the noise, and Brier improves instead.
                (Side::Candidate, Phase::Screening) => 0.86,
                (Side::Candidate, Phase::Confirmation) => 0.79,
            };
            let brier = match (cell.side, cell.phase) {
                (Side::Candidate, Phase::Confirmation) => 0.080,
                _ => 0.153,
            };
            Attempt::Scored {
                scores: Scores {
                    items: items_of(&cell.family),
                    accuracy: Some(accuracy),
                    ece: Some(0.065),
                    brier: Some(brier),
                    nll: Some(0.713),
                    confident_errors: Some(1),
                },
                refusals: 0,
            }
        });

        assert_eq!(evidence.screening.earned, Some(Metric::Accuracy));
        assert_eq!(evidence.decision, Decision::Reverted, "{}", evidence.reason);
        let confirmation = evidence
            .confirmation
            .as_ref()
            .expect("the confirmation ran");
        let repeat = criterion_named(confirmation, "the_win_repeats_on_accuracy");
        assert_eq!(repeat.verdict, Verdict::Failed);
        assert!(
            repeat.detail.contains("brier cleared instead"),
            "the verdict names the substitute by name: {}",
            repeat.detail
        );
        assert!(
            repeat
                .detail
                .contains("a different metric cannot be substituted"),
            "{}",
            repeat.detail
        );
        assert_eq!(
            repeat.rank, 1,
            "the anti-fishing criterion outranks the win it guards"
        );
    }

    #[test]
    fn a_win_that_repeats_on_its_own_metric_is_kept() {
        let evidence = run(&Bench::flat(0.78, 0.86), &plan(&[0, 1, 2]));
        let confirmation = evidence
            .confirmation
            .as_ref()
            .expect("the confirmation ran");
        let repeat = criterion_named(confirmation, "the_win_repeats_on_accuracy");
        assert_eq!(repeat.verdict, Verdict::Passed);
        assert!(
            repeat.detail.contains("on fresh blocks"),
            "{}",
            repeat.detail
        );
        assert_eq!(evidence.decision, Decision::Kept);
    }

    #[test]
    fn a_mean_that_improves_while_the_median_does_not_is_reverted() {
        // One lucky block carries the mean. Two of the three blocks are
        // worse than the control, so the median disagrees.
        let bench = Bench {
            accuracy: Box::new(|side, _, block| match (side, block) {
                (Side::Control, _) => 0.78,
                (Side::Candidate, 2) => 1.00,
                (Side::Candidate, _) => 0.76,
            }),
            confident_errors: Box::new(|_| 1),
        };
        let evidence = run(&bench, &plan(&[0, 1, 2]));
        let median = criterion_named(&evidence.screening, "the_median_agrees_with_the_mean");
        assert_eq!(median.verdict, Verdict::Failed, "{}", median.detail);
        assert!(
            median.detail.contains("mean 0.780 to 0.840"),
            "{}",
            median.detail
        );
        assert!(
            median.detail.contains("median 0.780 to 0.760"),
            "{}",
            median.detail
        );
        assert_eq!(evidence.decision, Decision::Reverted, "{}", evidence.reason);
        assert!(
            evidence.confirmation.is_none(),
            "a reverted screening spends no fresh block"
        );
    }

    #[test]
    fn an_increase_in_confident_errors_rejects_whatever_else_improved() {
        let bench = Bench {
            // The candidate is better on everything the rule can read.
            accuracy: Box::new(|side, _, _| match side {
                Side::Control => 0.70,
                Side::Candidate => 0.95,
            }),
            confident_errors: Box::new(|side| match side {
                Side::Control => 1,
                Side::Candidate => 2,
            }),
        };
        let evidence = run(&bench, &plan(&[0, 1, 2]));
        assert_eq!(evidence.decision, Decision::Reverted, "{}", evidence.reason);
        let floor = criterion_named(&evidence.screening, "confident_errors_do_not_rise");
        assert_eq!(floor.verdict, Verdict::Failed);
        assert_eq!(floor.rank, 1);
        assert!(
            floor
                .detail
                .contains("This is a direction and not an effect size"),
            "{}",
            floor.detail
        );

        // The accuracy win is still recorded. It simply does not buy the
        // floor off.
        let win = criterion_named(&evidence.screening, "a_metric_gain_clears_the_noise");
        assert_eq!(win.verdict, Verdict::Passed);
        assert_eq!(
            evidence
                .screening
                .outcome
                .deciding()
                .map(|criterion| criterion.name.as_str()),
            Some("confident_errors_do_not_rise")
        );
        assert_eq!(
            evidence.screening.earned, None,
            "a refused round earned nothing"
        );
    }

    #[test]
    fn a_candidate_that_improves_overall_while_wrecking_one_family_is_refused() {
        let bench = Bench {
            accuracy: Box::new(|side, family, _| match (side, family) {
                (Side::Control, "routing") => 0.78,
                (Side::Control, _) => 0.70,
                (Side::Candidate, "routing") => 0.92,
                (Side::Candidate, _) => 0.55,
            }),
            confident_errors: Box::new(|_| 1),
        };
        let evidence = run(&bench, &plan(&[0, 1, 2]));
        let win = criterion_named(&evidence.screening, "a_metric_gain_clears_the_noise");
        assert_eq!(
            win.verdict,
            Verdict::Passed,
            "the candidate does win overall"
        );

        let guard = criterion_named(&evidence.screening, "no_family_loses_more_than_the_noise");
        assert_eq!(guard.verdict, Verdict::Failed);
        assert!(
            guard.detail.contains("severity accuracy 0.700 to 0.550"),
            "{}",
            guard.detail
        );
        assert!(
            guard
                .detail
                .contains("at the suite's measured spread rather than each family's"),
            "the guard says the spread it used is not the family's own: {}",
            guard.detail
        );
        assert_eq!(evidence.decision, Decision::Reverted, "{}", evidence.reason);
    }

    #[test]
    fn a_harness_failure_requeues_once_and_never_enters_the_comparison() {
        let mut failed = 0;
        let rule = Rule::v1();
        let plan = plan(&[0, 1, 2]);
        let round = Round::run(&plan, &rule, |cell| {
            if cell.index == 3 && cell.attempt == 1 {
                failed += 1;
                return Attempt::HarnessFailure {
                    detail: "the connection reset".into(),
                };
            }
            scored(items_of(&cell.family), 0.80, 1)
        });
        assert_eq!(failed, 1);
        assert_eq!(round.requeued, 1);
        assert_eq!(
            round.trials.len(),
            round.expected(),
            "the retry filled the gap"
        );
        assert_eq!(round.invalid.len(), 1);
        assert!(round.invalid[0].attempt.scores().is_none());
        assert!(
            round.trials.iter().all(|trial| trial.attempt.is_valid()),
            "no harness failure is in the comparison"
        );
        let retry = round
            .trials
            .iter()
            .find(|trial| trial.cell.requeued_from == Some(3))
            .expect("the requeued cell ran again");
        assert_eq!(retry.cell.attempt, 2);
        assert_eq!(
            retry.cell.index,
            round.expected() + 1,
            "a retry takes a new index"
        );

        let stage = rule.screen(&round);
        let complete = criterion_named(&stage, "every_cell_has_a_valid_attempt");
        assert_eq!(complete.verdict, Verdict::Passed);
        assert!(
            complete.detail.contains("1 requeued"),
            "{}",
            complete.detail
        );
    }

    #[test]
    fn a_cell_that_fails_twice_leaves_the_round_undecided() {
        let rule = Rule::v1();
        let plan = plan(&[0, 1, 2]);
        let round = Round::run(&plan, &rule, |cell| {
            if cell.family == "severity" && cell.block == 1 && cell.side == Side::Candidate {
                return Attempt::HarnessFailure {
                    detail: "the box never became ready".into(),
                };
            }
            scored(items_of(&cell.family), 0.80, 1)
        });
        assert_eq!(round.requeued, 1, "a cell is requeued once, not forever");
        assert_eq!(round.invalid.len(), 2);
        assert_eq!(round.trials.len(), round.expected() - 1);

        let stage = rule.screen(&round);
        assert_eq!(stage.outcome.verdict, Verdict::Unverifiable);
        assert_eq!(
            stage
                .outcome
                .deciding()
                .map(|criterion| criterion.name.as_str()),
            Some("every_cell_has_a_valid_attempt")
        );
        assert!(
            stage
                .outcome
                .criteria
                .iter()
                .all(|criterion| criterion.verdict != Verdict::Failed),
            "nothing downstream of an incomplete round is judged: {:?}",
            stage.outcome.criteria
        );
    }

    #[test]
    fn a_door_refusal_enters_the_comparison_as_a_refusal() {
        // The refusing door keeps the item in its denominator, so its
        // accuracy falls. The round compares it as it stands rather than
        // dropping the item, and the record says how many it declined.
        let rule = Rule::v1();
        let plan = plan(&[0, 1, 2]);
        let round = Round::run(&plan, &rule, |cell| {
            let items = items_of(&cell.family);
            match cell.side {
                Side::Control => scored(items, 0.78, 1),
                Side::Candidate => Attempt::Scored {
                    scores: Scores {
                        items,
                        // Five declined items stay in the denominator.
                        accuracy: Some(0.78 * (items - 5) as f64 / items as f64),
                        ece: Some(0.065),
                        brier: Some(0.153),
                        nll: Some(0.713),
                        confident_errors: Some(1),
                    },
                    refusals: 5,
                },
            }
        });
        assert_eq!(round.invalid.len(), 0, "a refusal is not a harness failure");
        assert_eq!(round.trials.len(), round.expected());

        let stage = rule.screen(&round);
        let summary = stage
            .summary(Side::Candidate, OVERALL)
            .expect("the candidate is summarized");
        assert_eq!(
            summary.refusals, 30,
            "five items a family, two families, three blocks"
        );
        assert_eq!(summary.items, Some(ROUTING + SEVERITY));
        let control = stage
            .summary(Side::Control, OVERALL)
            .expect("the control is summarized");
        assert!(
            summary.stat(Metric::Accuracy).expect("accuracy").mean
                < control.stat(Metric::Accuracy).expect("accuracy").mean,
            "a door cannot score better by declining the questions it finds hard"
        );
        let items = criterion_named(&stage, "the_blocks_scored_the_same_items");
        assert!(
            items.detail.contains("candidate declined 30"),
            "{}",
            items.detail
        );
        assert!(
            items.detail.contains("stays in the denominator"),
            "{}",
            items.detail
        );
    }

    #[test]
    fn a_confirmation_that_redraws_a_screening_block_is_refused() {
        let rule = Rule::v1();
        let bench = Bench::flat(0.78, 0.86);
        let screening_plan = plan(&[0, 1, 2]);
        let screening = Round::run(&screening_plan, &rule, |cell| bench.attempt(cell));
        let screened = rule.screen(&screening);
        assert_eq!(screened.earned, Some(Metric::Accuracy));

        // The same blocks again. The door reproduces them exactly, so this is
        // the same arithmetic rather than a second witness.
        let stale = Plan::new(Phase::Confirmation, families(), [1, 2, 3]);
        let round = Round::run(&stale, &rule, |cell| bench.attempt(cell));
        let stage = rule.confirm(&round, Metric::Accuracy, &screening);
        let fresh = criterion_named(&stage, "the_confirmation_draws_fresh_blocks");
        assert_eq!(fresh.verdict, Verdict::Failed);
        assert!(
            fresh.detail.contains("redrew block 1, 2"),
            "{}",
            fresh.detail
        );
        assert_eq!(Decision::of(stage.outcome.verdict), Decision::Reverted);

        let honest = Round::run(&screening_plan.confirmation(), &rule, |cell| {
            bench.attempt(cell)
        });
        let stage = rule.confirm(&honest, Metric::Accuracy, &screening);
        assert_eq!(
            stage.outcome.verdict,
            Verdict::Passed,
            "{:?}",
            stage.outcome.criteria
        );
    }

    #[test]
    fn a_confirmation_over_different_families_is_not_the_same_experiment() {
        let rule = Rule::v1();
        let bench = Bench::flat(0.78, 0.86);
        let screening = Round::run(&plan(&[0, 1, 2]), &rule, |cell| bench.attempt(cell));
        let narrowed = Plan::new(Phase::Confirmation, ["routing"], [3, 4, 5]);
        let round = Round::run(&narrowed, &rule, |cell| bench.attempt(cell));
        let stage = rule.confirm(&round, Metric::Accuracy, &screening);
        let same = criterion_named(&stage, "the_confirmation_scores_the_same_families");
        assert_eq!(same.verdict, Verdict::Failed);
        assert!(
            same.detail.contains("not the same experiment"),
            "{}",
            same.detail
        );
    }

    #[test]
    fn two_blocks_a_side_is_below_the_floor_the_median_criterion_needs() {
        let evidence = run(&Bench::flat(0.78, 0.90), &plan(&[0, 1]));
        assert_eq!(
            evidence.decision,
            Decision::Undecided,
            "{}",
            evidence.reason
        );
        let floor = criterion_named(&evidence.screening, "blocks_per_side>=3");
        assert_eq!(floor.verdict, Verdict::Unverifiable);
        assert!(
            floor.detail.contains("the median is the mean"),
            "{}",
            floor.detail
        );
        assert!(
            evidence
                .screening
                .outcome
                .criteria
                .iter()
                .all(|criterion| criterion.verdict != Verdict::Failed),
            "nothing downstream of a missing floor is judged"
        );
    }

    #[test]
    fn a_pooled_ece_is_unknown_rather_than_an_item_weighted_invention() {
        // ECE is computed inside bins, so the suite's ECE is not the
        // item-weighted mean of its families'. A family keeps its own.
        let rule = Rule::v1();
        let round = Round::run(&plan(&[0, 1, 2]), &rule, |cell| {
            scored(items_of(&cell.family), 0.80, 1)
        });
        let stage = rule.screen(&round);
        let overall = stage
            .summary(Side::Control, OVERALL)
            .expect("the suite is summarized");
        assert!(
            overall.stat(Metric::Ece).is_none(),
            "a pooled ECE is not reported"
        );
        assert!(
            overall.stat(Metric::Accuracy).is_some(),
            "accuracy pools exactly"
        );
        assert!(overall.stat(Metric::Brier).is_some(), "so does Brier");

        let family = stage
            .summary(Side::Control, "routing")
            .expect("routing is summarized");
        let ece = family
            .stat(Metric::Ece)
            .expect("a family keeps its own ECE");
        assert!((ece.mean - 0.065).abs() < 1e-9);
    }

    #[test]
    fn an_unmeasured_metric_leaves_the_round_undecided_rather_than_passing_by_default() {
        let rule = Rule::v1();
        let round = Round::run(&plan(&[0, 1, 2]), &rule, |cell| Attempt::Scored {
            scores: Scores {
                items: items_of(&cell.family),
                accuracy: None,
                ece: Some(0.065),
                brier: Some(0.153),
                nll: Some(0.713),
                confident_errors: Some(1),
            },
            refusals: 0,
        });
        let stage = rule.screen(&round);
        assert_eq!(stage.outcome.verdict, Verdict::Unverifiable);
        assert_eq!(stage.earned, None);
        let win = criterion_named(&stage, "a_metric_gain_clears_the_noise");
        assert!(
            win.detail
                .contains("accuracy was not measured on both sides"),
            "{}",
            win.detail
        );
    }

    #[test]
    fn unknown_confident_errors_are_unverifiable_and_never_zero() {
        let rule = Rule::v1();
        let round = Round::run(&plan(&[0, 1, 2]), &rule, |cell| Attempt::Scored {
            scores: Scores {
                items: items_of(&cell.family),
                accuracy: Some(if cell.side == Side::Control {
                    0.78
                } else {
                    0.90
                }),
                ece: Some(0.065),
                brier: Some(0.153),
                nll: Some(0.713),
                confident_errors: None,
            },
            refusals: 0,
        });
        let stage = rule.screen(&round);
        let floor = criterion_named(&stage, "confident_errors_do_not_rise");
        assert_eq!(floor.verdict, Verdict::Unverifiable);
        assert!(
            floor.detail.contains("unknown is never zero"),
            "{}",
            floor.detail
        );
        assert_eq!(Decision::of(stage.outcome.verdict), Decision::Undecided);
    }

    #[test]
    fn the_verdict_precedence_carries_into_the_decision() {
        assert_eq!(Decision::of(Verdict::Passed), Decision::Kept);
        assert_eq!(Decision::of(Verdict::Failed), Decision::Reverted);
        assert_eq!(Decision::of(Verdict::Unverifiable), Decision::Undecided);
        assert!(Decision::Kept.kept());
        assert!(!Decision::Undecided.kept(), "undecided does not ship");
        assert!(!Decision::Reverted.kept());
    }
}
