//! Acceptance rules that carry their own digest, live in a committed file,
//! and answer with three values rather than two.
//!
//! The design is carried from `crates/coder-bench/src/gate.rs` in the coder
//! repository and reimplemented here: a gate spec with a content digest over
//! the rule's identity — the prose that explains it stays outside — and a
//! verdict where `failed` beats `unverifiable` beats `passed`.
//!
//! # Why a gate is a file
//!
//! The rule this module replaces is `calibrate::admit`, which lived in
//! `crates/lev/src/calibrate.rs` and is now deleted, and where the
//! thresholds were constants in a function body. One of them, a
//! Brier tolerance, was widened from zero to a tenth after a run refused
//! maps that cut ECE from 0.157 to 0.005. The argument for widening is in
//! `docs/lev/calibration.md` and holds: a binned map is monotone in the raw
//! signal, so it cannot re-rank items by confidence, it cannot improve
//! refinement, and it can only lose to binning, and demanding zero loss
//! rejects real calibration.
//!
//! # What this rule cannot see
//!
//! A [`Comparison`] carries scores and an item count. Nothing in it
//! describes the map, so no criterion here reads a fitted table, and
//! `accuracy_does_not_fall` cannot move: the contract in
//! [`crate::calibrate`] fixes the answer a map rescales, so the answer's
//! outcome is the same on both sides by construction. That makes the
//! accuracy criterion a guard against a candidate *door*, which is the other
//! thing this rule judges, and vacuous against a candidate map.
//!
//! It also means this gate is not what stands between the repository and a
//! map whose rescale leaves a runner-up holding the larger share. Nothing
//! committed does that — openagents#9438 enumerated every record against
//! every row — and the guarantee rests on the consumers naming the selected
//! option rather than on a verdict here.
//!
//! What does not hold is the record. Nothing anywhere says which rule
//! produced which verdict, so the change is a claim in a commit message.
//! With a digest, widening a tolerance produces `probability-v2`, rows
//! scored under the old rule keep `gate:…v1`, and "which verdicts did this
//! change flip" becomes a query.
//!
//! A suite's content digest deliberately does not cover its gate. The suite
//! digest pins what was run, and tightening a floor must not make historical
//! runs read as drifted. Each row pins the gate it was judged by instead,
//! through [`Outcome::gate_digest`].
//!
//! # What the digest covers
//!
//! The digest pins the rule, not the file: the schema, the id, the `decides`
//! tag, every bound's value, basis, and evidence, the enums that say what a
//! statistic covers, and the identity of any measurement the rule is still
//! waiting on. It does not pin `question`, `$comment`, a bound's `why`, or a
//! pending measurement's `why` — the sentence that explains a threshold is
//! not the threshold, and editing one must not orphan the rows the rule
//! already scored. A verdict recorded under an earlier encoding still
//! attributes, through `previously` and [`Gate::has_digest`] — and the
//! alias is bound to the identity it was reviewed against, so a rule whose
//! policy moved cannot inherit it by keeping the string.
//! `docs/gym/gate-digests.md` states the policy.
//!
//! # Why three verdicts
//!
//! `admit` says "refused" both for a map fitted on 18 items, where nobody
//! can tell, and for a map whose Brier rose, where anyone can tell and it
//! lost. Collapsing those is what made the gate look like it refused
//! everything. Here they are [`Verdict::Unverifiable`] and
//! [`Verdict::Failed`], and the split runs through every criterion:
//!
//! - A direction that is measurably wrong is `failed`.
//! - A direction that is right but short of the effect size is
//!   `unverifiable`. The move might be noise, and the gate says so rather
//!   than calling it a loss.
//! - A measurement that is missing, or a split below the floor, is
//!   `unverifiable`, and nothing downstream of it is judged. Unknown is
//!   never zero, and an unmeasured criterion never passes by default.
//!
//! # Why three gates
//!
//! `admit` answers two product questions with one function, and
//! `docs/lev/measurements/2026-09-19-adapter-v1.md` already concludes in
//! prose what that code cannot express: ship the adapter for the decision,
//! not yet for the probability. The choice adapter won 13 points of accuracy
//! on `support-v2` while log loss rose from 1.952 to 2.323 and confident
//! errors went from six to eight. After calibration the base model sits at
//! one confident error against the adapted model's nine.
//!
//! So a candidate can be the better decision and the worse probability at
//! the same time, and one verdict cannot say that. [`Rule::Decision`] reads
//! accuracy; [`Rule::Probability`] reads log loss and confident errors first
//! and accuracy last.
//!
//! Neither of them judges time or money, and for a router sitting in front
//! of every agent turn the time is the product. Lev answers in about 1.6
//! seconds because it draws eight samples across four helpers; `kev-0.5b`
//! answers in about 180. A door that wins two points of accuracy and costs
//! 1.4 seconds a turn is the worse door for that workload, and the two
//! rules above would call it a win. [`Rule::Deployment`] reads the latency
//! percentile, the price of a decision, and the share of the workload a
//! door declines.
//!
//! # Why the deployment budget is not in the rule
//!
//! A batch job tolerates 1.6 seconds and a router does not, so a universal
//! latency constant is wrong for one of them whatever number it carries.
//! [`DeploymentRule`] therefore holds no ceiling at all. The ceilings arrive
//! with the measurement, in a [`Budget`] the caller states and names, and a
//! criterion whose ceiling is absent is [`Verdict::Unverifiable`] rather
//! than passed. The rule keeps only what is a property of the measurement
//! rather than of the workload: how many timed calls a percentile needs,
//! which percentile is read, and how far a latency has to move before the
//! move is the door rather than the machine.
//!
//! The same distinction runs through [`Cost`]. A lane with no meter on it is
//! [`Cost::UnmeteredLocalLane`], never a zero, because our own hardware
//! still costs device time and power and what it lacks is a meter rather
//! than a price. A zero in a cost column is the same lie as a zero in an
//! unknown metric, and a gate reading that zero admits a door on a price
//! nobody quoted.
//!
//! # Latency has a noise floor, and which gate has measured it
//!
//! Wall clock on a shared machine moves with whatever else is running, so a
//! latency comparison needs the same thing an accuracy comparison needs: the
//! spread of the same measurement over blocks where nothing but the clock
//! changed. The first attempt, and why it does not count, is
//! `docs/gym/measurements/2026-09-19-latency-noise-floor.md`: the machine
//! was running nine agents against several copies of the same on-device
//! model, and the sweep measured the contention rather than the door.
//!
//! So in `deployment-v1` [`DeploymentRule::latency_block_sigma_relative`]
//! is [`Basis::Unmeasured`], every latency criterion reports
//! [`Verdict::Unverifiable`], and no latency comparison can refuse a door.
//! That is the same posture [`DecisionRule::gain_standard_errors`] held
//! while openagents#9370 was outstanding, and for the same reason: a gate
//! that invents this number would refuse doors for whatever else the machine
//! was running, and the refusal would carry a digest that made it look
//! measured. `deployment-v2` carries the number once it was measured, on a
//! quiet CPU-only host over 16 blocks per local door
//! (`docs/gym/measurements/2026-09-20-kev-quiet-latency.md`), and keeps a
//! pending measurement for the doors that answer in seconds, which that
//! sweep did not reach. `deployment-v1` stays as it was, because a floor
//! moving from unmeasured to a value is a new rule.
//!
//! The cost and refusal criteria do not depend on it. Metering is a count
//! rather than a clock, and a contended machine does not change what a
//! decision is billed at.
//!
//! # Where the constants come from
//!
//! Every threshold carries a [`Bound`], and every bound records its
//! [`Basis`] — derived, tuned, convention, or unmeasured — and the records it
//! rests on as [`Evidence`]. Both are inside the digest: relabelling a
//! constant, or resting it on a different record, produces a new rule. The
//! `why` that explains the number to a reader is outside it, so correcting a
//! rationale never does.
//!
//! One bound is worth the space here, because it is the one the issue that
//! owns this module asks about. The maximum Brier a `b`-bin monotone map can
//! lose on `n` items is computable. For one equal-width bin of width `w`
//! holding items whose raw signal is `p` and whose outcomes are `y`,
//! replacing every `p` with the bin's observed accuracy `a` changes the
//! Brier contribution by `2·mean(e·y) − (p̄ − a)² − var(p)`, where
//! `e = p − p̄`. Since `mean(e·y) ≤ a·w`, the loss on that bin is at most
//! `2·a·w`, and aggregating over bins weighted by their counts bounds the
//! whole move by `2·w·accuracy`. `Map::fit` adds Jeffreys smoothing, which
//! costs a further `(c − a)²` per bin, under `0.25/(n_b + 1)²`.
//!
//! The derivation is real and it does not give you a usable tolerance.
//! `Map::fit_auto` takes one bin per fifteen observations, so the 30-item
//! `urgency` family gets two bins and `w = 0.5`, and the ceiling is about
//! 0.73 in absolute Brier against a family whose raw Brier is 0.173. The
//! structural bound is an order of magnitude looser than anything a gate
//! would want to allow. It is worth keeping as a sanity ceiling — a map that
//! loses more than that has a bug, not a bad table — and it cannot serve as
//! the acceptance threshold.
//!
//! So `max_brier_increase` is recorded as `tuned`, with the case that tuned
//! it in the bound's own `why` field, and a test pins that case.
//!
//! # What is still unmeasured
//!
//! The decision gate's effect size is not a stored number. It is `k`
//! standard errors of the measured accuracies, computed per comparison, so a
//! thin split raises its own bar instead of being waved through by a
//! constant. That standard error covers item sampling only. It says nothing
//! about what a fresh trial of the same items would do, because
//! `crates/lev/src/estimator.rs` draws seeds `0..n` and reproduces exactly —
//! there is no trial-to-trial variance to average over yet.
//!
//! openagents#9370 makes the seed base a parameter and measures that spread,
//! and that number is what an absolute effect-size floor should be derived
//! from. Until it lands, no such floor is invented here. The gap is recorded
//! on the gate itself — [`DecisionRule::variance_basis`] and the pending
//! measurement's identity are inside the digest — so adding the trial term
//! produces `decision-v2` rather than quietly rewriting `decision-v1`'s
//! history.

use std::fmt;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use sha2::{Digest, Sha256};

/// The schema every gate file is tagged with.
///
/// `v2` splits a bound's and a pending measurement's prose from their
/// identity: `why` explains and stays outside the digest, while `basis`,
/// `evidence`, and a pending measurement's `quantity` and `issue` are the
/// rule and stay inside it. A `v1` document parses far enough to be refused
/// by name rather than as a syntax error.
pub const SCHEMA: &str = "openagents.gym.gate.v2";

/// The schema the digest's projection is written in.
///
/// [`Gate::digest`] does not hash the gate file; it hashes a typed view of
/// the rule, and this is that view's version. It moves when the view's shape
/// moves, so an encoding change is a declared transition rather than a
/// silent re-identification.
pub const IDENTITY_SCHEMA: &str = "openagents.gym.gate-identity.v1";

/// The environment variable that points at a directory of gate files.
pub const GATES_DIR_VAR: &str = "GYM_GATES_DIR";

/// What went wrong loading or validating a gate.
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    /// The file could not be read.
    #[error("gate file {path}: {source}")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file is not a gate document.
    #[error("gate file {path} is not a gate: {source}")]
    Parse {
        /// The file that failed to parse.
        path: PathBuf,
        /// The underlying error.
        source: serde_json::Error,
    },
    /// The document declares a schema this build does not read.
    #[error("gate {id} carries schema {found}, and this build reads {SCHEMA}")]
    Schema {
        /// The gate that declared it.
        id: String,
        /// The schema the file declared.
        found: String,
    },
    /// The document parsed and says something impossible.
    #[error("gate {id}: {problem}")]
    Invalid {
        /// The gate that declared it.
        id: String,
        /// What is wrong, and what to do about it.
        problem: String,
    },
    /// No file in the directory declares that id.
    #[error(
        "no gate with id {id} in {dir}; set {GATES_DIR_VAR} to the directory holding the \
         gate files"
    )]
    NotFound {
        /// The id that was asked for.
        id: String,
        /// Where it was looked for.
        dir: PathBuf,
    },
}

/// Where a threshold came from.
///
/// This is inside the digest. Relabelling a constant is a change to the
/// rule, and a reader comparing digests should see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Computed from a rule or a measurement, with the derivation recorded
    /// in the bound's `why` or in this module's documentation.
    Derived,
    /// Chosen by hand after seeing a result. The `why` names the result.
    Tuned,
    /// A standard default, carried across rather than derived from this
    /// suite. Visible, so nobody mistakes it for a measurement.
    Convention,
    /// Nothing has measured what this bound should be. The value is absent
    /// and the criterion it governs reports [`Verdict::Unverifiable`].
    Unmeasured,
}

impl Basis {
    /// The word used in verdict details.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
            Self::Tuned => "tuned",
            Self::Convention => "convention",
            Self::Unmeasured => "unmeasured",
        }
    }
}

impl fmt::Display for Basis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A record a bound rests on, named rather than described.
///
/// This is inside the digest, like [`Basis`]: two bounds carrying the same
/// number on different evidence are different rules. An `id` is a name, not
/// a path — `2026-09-19-suite-v2-scores` names the same record after the
/// measurements directory moves, where the path that held it would not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Evidence {
    /// A committed measurement record, named by its dated slug.
    Measurement {
        /// The record's name.
        id: String,
    },
    /// A tracked issue, named the way the tracker names it.
    Issue {
        /// The record's name.
        id: String,
    },
    /// An implementation a number derives from or was carried over from,
    /// named as the item rather than the file that holds it.
    Code {
        /// The record's name.
        id: String,
    },
}

impl Evidence {
    /// The record's name.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Measurement { id } | Self::Issue { id } | Self::Code { id } => id,
        }
    }
}

/// One threshold, with where it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bound {
    /// The threshold. Absent when the basis is [`Basis::Unmeasured`].
    pub value: Option<f64>,
    /// Where the number came from.
    pub basis: Basis,
    /// The records the number rests on, named rather than described. Inside
    /// the digest; empty when the number rests on a derivation or a
    /// convention rather than a record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    /// The provenance, in one or two sentences. Prose for a reader, held
    /// outside the digest: the structured provenance is `basis` and
    /// `evidence`, and editing a paragraph never re-identifies the rule.
    pub why: String,
}

impl Bound {
    /// The threshold, when one has been measured or chosen.
    #[must_use]
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    /// The threshold as a count.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn count(&self) -> Option<usize> {
        self.value.map(|value| value.max(0.0) as usize)
    }

    fn validate(&self, gate: &str, name: &str) -> Result<(), GateError> {
        let unmeasured = self.basis == Basis::Unmeasured;
        match (self.value, unmeasured) {
            (Some(_), true) => Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!(
                    "{name} carries a value and an unmeasured basis; drop the value or name \
                     the basis that backs it"
                ),
            }),
            (None, false) => Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!(
                    "{name} has no value and a {} basis; record it as unmeasured instead",
                    self.basis
                ),
            }),
            (Some(value), false) if !value.is_finite() || value < 0.0 => Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!("{name} must be a finite number at or above zero, got {value}"),
            }),
            _ if self.why.trim().is_empty() => Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!("{name} has no provenance; say where the number came from"),
            }),
            _ if self.evidence.iter().any(|e| e.id().trim().is_empty()) => {
                Err(GateError::Invalid {
                    id: gate.to_string(),
                    problem: format!(
                        "{name} cites a record with no name; an evidence id names what the \
                         number rests on"
                    ),
                })
            }
            _ if self
                .evidence
                .iter()
                .any(|e| e.id().contains('/') || e.id().ends_with(".md")) =>
            {
                Err(GateError::Invalid {
                    id: gate.to_string(),
                    problem: format!(
                        "{name} cites a path; evidence names the record rather than the file \
                         holding it, so moving the documentation tree does not re-identify \
                         the rule"
                    ),
                })
            }
            _ => Ok(()),
        }
    }

    /// This bound as the digest sees it: the number, its basis, and the
    /// records it rests on. `why` explains them to a reader and is not part
    /// of the rule's identity.
    fn identity(&self) -> BoundIdentity<'_> {
        BoundIdentity {
            value: self.value,
            basis: self.basis,
            evidence: &self.evidence,
        }
    }
}

/// Which noise a standard error accounts for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarianceBasis {
    /// The spread you would see over a different draw of items, and nothing
    /// else. It does not cover what a fresh trial of the same items does.
    ItemSampling,
    /// Item sampling and the trial-to-trial spread openagents#9370 measures.
    ItemSamplingAndTrialResampling,
}

impl VarianceBasis {
    /// The phrase used in verdict details.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ItemSampling => "item sampling",
            Self::ItemSamplingAndTrialResampling => "item sampling and trial resampling",
        }
    }
}

/// A measurement the rule is missing, and what fills the gap meanwhile.
///
/// `quantity` and `issue` are the gap's identity and sit inside the digest:
/// filling this gap, or recording a different one, is a different rule.
/// `why` is prose for a reader and stays outside it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    /// What is unmeasured, named as a quantity rather than described.
    pub quantity: String,
    /// The tracked issue that takes the measurement, when there is one.
    pub issue: Option<String>,
    /// What the rule does in the meantime, and why that is the safe
    /// direction.
    pub why: String,
}

impl Pending {
    /// Rejects a gap record that says nothing.
    fn validate(&self, gate: &str) -> Result<(), GateError> {
        let problem = if self.quantity.trim().is_empty() {
            Some("pending_measurement names no quantity; say what is unmeasured".to_string())
        } else if self.why.trim().is_empty() {
            Some(
                "pending_measurement has no why; say what the rule does in the meantime and \
                 why that is safe. A prose string from the v1 schema lands here: split it \
                 into quantity, issue, and why"
                    .to_string(),
            )
        } else if self
            .issue
            .as_deref()
            .is_some_and(|issue| issue.trim().is_empty())
        {
            Some(
                "pending_measurement carries an empty issue; name the issue that takes the \
                 measurement or drop the field"
                    .to_string(),
            )
        } else {
            None
        };
        match problem {
            Some(problem) => Err(GateError::Invalid {
                id: gate.to_string(),
                problem,
            }),
            None => Ok(()),
        }
    }

    /// This gap as the digest sees it: what is unmeasured and who takes it,
    /// without the prose about what the rule does meanwhile.
    fn identity(&self) -> PendingIdentity<'_> {
        PendingIdentity {
            quantity: &self.quantity,
            issue: self.issue.as_deref(),
        }
    }
}

/// Reads `pending_measurement` in either form: the structured object this
/// schema writes, or the bare prose string `openagents.gym.gate.v1` wrote.
/// The string parses into a [`Pending`] whose `why` is empty, so a v1 file
/// reaches the schema check and is refused by name rather than as a syntax
/// error, and a v2 file that writes one fails validation for carrying no
/// `why`.
///
/// The object form goes through `serde_json::from_value` rather than an
/// untagged variant, because `deny_unknown_fields` is silently ignored
/// inside untagged enums — and a pending object that quietly dropped a new
/// field would be a semantic field discarded on the way to the digest.
fn pending_field<'de, D>(deserializer: D) -> Result<Option<Pending>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<Value>::deserialize(deserializer)? {
        None => None,
        Some(Value::String(text)) => Some(Pending {
            quantity: text,
            issue: None,
            why: String::new(),
        }),
        Some(other) => Some(serde_json::from_value(other).map_err(serde::de::Error::custom)?),
    })
}

/// The rule that decides which door should answer the question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRule {
    /// Fewest scored items before the comparison is worth judging at all.
    pub min_items: Bound,
    /// How many standard errors of the measured accuracies the gain has to
    /// clear. The standard error is computed per comparison rather than
    /// stored, so a thin split raises its own bar.
    pub gain_standard_errors: Bound,
    /// What that standard error accounts for.
    pub variance_basis: VarianceBasis,
    /// The measurement that would complete this rule, when one is missing.
    #[serde(default, deserialize_with = "pending_field")]
    pub pending_measurement: Option<Pending>,
}

/// The rule that decides whether a set of probabilities may be served.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbabilityRule {
    /// Fewest items behind a fitted map, and behind the split it is scored
    /// on.
    pub min_items: Bound,
    /// The relative ECE reduction a candidate has to earn.
    pub min_ece_reduction: Bound,
    /// How much Brier may degrade to buy that reduction, as a share of the
    /// baseline.
    pub max_brier_increase: Bound,
    /// The noise a rise in confident errors has to clear before it is a
    /// rise. Absent in `probability-v1`, which compares the two counts
    /// directly and so refuses an unchanged door about half the time;
    /// `probability-v2` carries the measured floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confident_error_floor: Option<Box<ConfidentErrorFloor>>,
    /// The measurement that would complete this rule, when one is missing.
    #[serde(default, deserialize_with = "pending_field")]
    pub pending_measurement: Option<Pending>,
}

/// The seed noise on a confident-error count, and how much of it a rise has
/// to clear.
///
/// A count of confident errors is not a rate. On an unchanged door the count
/// ran 6, 6, 10, 4, 4, 10, 6, 4 across eight seed blocks of 98 items, a
/// standard deviation of 2.49, so "the count must not rise" fails a door
/// compared against itself whenever the seed lands the wrong way. The floor
/// scales that spread to the items under judgment as
/// `block_sigma · √(items / block_items)` — the spread of a count of rare
/// events grows with the square root of the count — and a rise fails only
/// when it exceeds `sigmas` standard deviations of the difference of the two
/// counts. A rise inside the floor passes: the criterion is a floor and not
/// a prize, so there is no margin to fall short of.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfidentErrorFloor {
    /// One standard deviation of the count across seed blocks, with one
    /// door and one item set held fixed. Unmeasured leaves the criterion
    /// [`Verdict::Unverifiable`].
    pub block_sigma: Bound,
    /// How many items each of those blocks scored.
    pub block_items: Bound,
    /// How many standard deviations of the difference a rise has to exceed
    /// to count as a rise.
    pub sigmas: Bound,
}

impl ConfidentErrorFloor {
    /// The largest rise that is still inside the noise for a comparison of
    /// `baseline_items` against `candidate_items`, when the floor is
    /// measured.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn allowance(&self, baseline_items: usize, candidate_items: usize) -> Option<f64> {
        let sigma = self.block_sigma.value()?;
        let block_items = self.block_items.value()?;
        let sigmas = self.sigmas.value()?;
        if block_items <= 0.0 {
            return None;
        }
        let variance = |items: usize| sigma * sigma * (items as f64 / block_items);
        Some(sigmas * (variance(baseline_items) + variance(candidate_items)).sqrt())
    }

    fn validate(&self, gate: &str) -> Result<(), GateError> {
        self.block_sigma
            .validate(gate, "confident_error_floor.block_sigma")?;
        self.block_items
            .validate(gate, "confident_error_floor.block_items")?;
        self.sigmas.validate(gate, "confident_error_floor.sigmas")?;
        if self.block_items.value().is_some_and(|items| items < 1.0) {
            return Err(GateError::Invalid {
                id: gate.to_string(),
                problem: "confident_error_floor.block_items must be at least one item; a spread \
                          measured over no items scales nothing"
                    .into(),
            });
        }
        Ok(())
    }
}

/// What one decision costs, or the fact that nobody is metering it.
///
/// There is no variant for free. A door running on hardware we own still
/// costs device time, power, and the machine it occupies; what it does not
/// have is a meter. Writing that down as a zero puts a measurement where an
/// absence is, and a gate reading that zero admits a door on a price nobody
/// quoted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "lane", rename_all = "snake_case")]
pub enum Cost {
    /// Billed, at a stated price per decision in US dollars.
    Metered {
        /// What one decision costs.
        usd_per_decision: f64,
    },
    /// Ours, and nobody is counting. Not zero.
    UnmeteredLocalLane,
}

impl Cost {
    /// The price of one decision, when one is quoted.
    #[must_use]
    pub const fn usd_per_decision(self) -> Option<f64> {
        match self {
            Self::Metered { usd_per_decision } => Some(usd_per_decision),
            Self::UnmeteredLocalLane => None,
        }
    }

    /// The word used in verdict details.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metered { .. } => "metered",
            Self::UnmeteredLocalLane => "unmetered_local_lane",
        }
    }
}

/// What a door costs to run, as distinct from how well it answers.
///
/// Every measure is optional, and absent means nobody measured it. A
/// criterion over an absent measure is [`Verdict::Unverifiable`], never a
/// pass. [`Profile::cost`] carries the distinction one step further: `None`
/// is nobody recorded what a decision costs, and
/// [`Cost::UnmeteredLocalLane`] is somebody recorded that there is no meter.
/// Those are different facts and the gate says different things about them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// How many calls were timed.
    pub calls: usize,
    /// The median call, in milliseconds.
    pub latency_p50_ms: Option<f64>,
    /// The 95th-percentile call, in milliseconds, by nearest rank.
    pub latency_p95_ms: Option<f64>,
    /// What one decision costs, or that nothing meters it.
    pub cost: Option<Cost>,
    /// How many of the timed calls the door declined.
    ///
    /// A refusal is a result, and a door that declines a tenth of the
    /// workload has a throughput its latency does not show.
    pub refusals: Option<usize>,
}

impl Profile {
    /// Reads both percentiles off a set of timed calls.
    ///
    /// The percentile is nearest rank: the `p`th percentile of `n` sorted
    /// calls is the call at position `ceil(p * n / 100)`. Nearest rank never
    /// interpolates, so every number it reports is a call that happened.
    #[must_use]
    pub fn timed(latencies: &[f64]) -> Self {
        let mut sorted: Vec<f64> = latencies
            .iter()
            .copied()
            .filter(|ms| ms.is_finite())
            .collect();
        sorted.sort_by(f64::total_cmp);
        Self {
            calls: sorted.len(),
            latency_p50_ms: percentile(&sorted, 50.0),
            latency_p95_ms: percentile(&sorted, 95.0),
            cost: None,
            refusals: None,
        }
    }

    /// Records what a decision on this door costs.
    #[must_use]
    pub const fn costing(mut self, cost: Cost) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Records how many of the timed calls the door declined.
    #[must_use]
    pub const fn refusing(mut self, refusals: usize) -> Self {
        self.refusals = Some(refusals);
        self
    }

    /// The share of timed calls the door declined, when it was counted.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn refusal_rate(&self) -> Option<f64> {
        if self.calls == 0 {
            return None;
        }
        self.refusals.map(|count| count as f64 / self.calls as f64)
    }
}

/// The `p`th percentile of a sorted slice, by nearest rank.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted.get(rank.min(sorted.len()) - 1).copied()
}

/// What a workload can afford, stated by the caller.
///
/// This is not part of the rule and it is not in the rule's digest. A batch
/// job tolerates 1.6 seconds and a router in front of every agent turn does
/// not, so a universal latency constant would be wrong for one of them
/// whichever number it carried. The gate takes its ceilings from whoever is
/// deploying the door, and where a ceiling is absent the criterion it
/// governs is [`Verdict::Unverifiable`] rather than passed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    /// The workload these ceilings belong to, named so a reader can tell
    /// whether they are the ones that apply.
    pub workload: String,
    /// The most the gated latency percentile may be, in milliseconds.
    pub max_latency_ms: Option<f64>,
    /// The most one decision may cost, in US dollars.
    pub max_cost_per_decision_usd: Option<f64>,
    /// The share of the workload the door may decline.
    pub max_refusal_rate: Option<f64>,
    /// Where these numbers came from. The caller's provenance, recorded so
    /// a verdict can be read back without asking them.
    pub source: String,
}

impl Budget {
    /// A budget with no ceilings, which judges nothing until one is set.
    #[must_use]
    pub fn new(workload: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            workload: workload.into(),
            max_latency_ms: None,
            max_cost_per_decision_usd: None,
            max_refusal_rate: None,
            source: source.into(),
        }
    }

    /// Sets the latency ceiling, in milliseconds.
    #[must_use]
    pub const fn latency_ms(mut self, ceiling: f64) -> Self {
        self.max_latency_ms = Some(ceiling);
        self
    }

    /// Sets the price ceiling for one decision, in US dollars.
    #[must_use]
    pub const fn cost_usd(mut self, ceiling: f64) -> Self {
        self.max_cost_per_decision_usd = Some(ceiling);
        self
    }

    /// Sets the share of the workload the door may decline.
    #[must_use]
    pub const fn refusal_rate(mut self, ceiling: f64) -> Self {
        self.max_refusal_rate = Some(ceiling);
        self
    }
}

/// Two doors' running costs over the same workload, and what that workload
/// can afford.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deployment {
    /// What was measured: a door, a suite, or a workload.
    pub group: String,
    /// The door in service.
    pub baseline: Profile,
    /// The door proposed to replace it.
    pub candidate: Profile,
    /// What the workload can afford. `None` means nobody stated it, and
    /// every ceiling criterion then reports [`Verdict::Unverifiable`].
    pub budget: Option<Budget>,
}

impl Deployment {
    /// Two profiles over the same workload, with no budget stated yet.
    #[must_use]
    pub fn new(group: impl Into<String>, baseline: Profile, candidate: Profile) -> Self {
        Self {
            group: group.into(),
            baseline,
            candidate,
            budget: None,
        }
    }

    /// States what the workload can afford.
    #[must_use]
    pub fn under(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }
}

/// Which latency percentile a rule gates on.
///
/// This is inside the digest, like [`VarianceBasis`], because a rule that
/// reads a different statistic is a different rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatedPercentile {
    /// The median call.
    P50,
    /// The 95th-percentile call.
    P95,
}

impl GatedPercentile {
    /// The word used in criterion names and verdict details.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P50 => "p50",
            Self::P95 => "p95",
        }
    }

    /// Reads the gated percentile off a profile.
    #[must_use]
    pub const fn read(self, profile: &Profile) -> Option<f64> {
        match self {
            Self::P50 => profile.latency_p50_ms,
            Self::P95 => profile.latency_p95_ms,
        }
    }
}

impl fmt::Display for GatedPercentile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The rule that decides whether a door can be afforded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentRule {
    /// Fewest timed calls before a percentile is worth reading at all.
    pub min_calls: Bound,
    /// Which latency percentile the ceiling and the regression are read at.
    pub gated_percentile: GatedPercentile,
    /// One standard deviation of that percentile across blocks of the same
    /// workload on the same machine, as a share of the percentile itself.
    /// Relative, because a 1,600 ms door and a 180 ms door do not wobble by
    /// the same number of milliseconds.
    pub latency_block_sigma_relative: Bound,
    /// How many of those standard deviations a latency move has to clear
    /// before the gate calls it a move rather than a busy machine.
    pub regression_sigmas: Bound,
    /// The measurement that would complete this rule, when one is missing.
    #[serde(default, deserialize_with = "pending_field")]
    pub pending_measurement: Option<Pending>,
}

/// Which product question a gate answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decides", rename_all = "snake_case")]
pub enum Rule {
    /// Which door should answer the question.
    Decision(DecisionRule),
    /// Whether a set of probabilities may be served.
    Probability(ProbabilityRule),
    /// Whether a door can be afforded: what it costs in time and money to
    /// run, against what the caller's workload can pay.
    Deployment(DeploymentRule),
}

impl Rule {
    /// The measurement this rule is missing, when it is missing one.
    #[must_use]
    pub fn pending_measurement(&self) -> Option<&Pending> {
        match self {
            Self::Decision(rule) => rule.pending_measurement.as_ref(),
            Self::Probability(rule) => rule.pending_measurement.as_ref(),
            Self::Deployment(rule) => rule.pending_measurement.as_ref(),
        }
    }

    /// This rule as the digest sees it.
    fn identity(&self) -> RuleIdentity<'_> {
        match self {
            Self::Decision(rule) => RuleIdentity::Decision(DecisionRuleIdentity {
                min_items: rule.min_items.identity(),
                gain_standard_errors: rule.gain_standard_errors.identity(),
                variance_basis: rule.variance_basis,
                pending_measurement: rule.pending_measurement.as_ref().map(Pending::identity),
            }),
            Self::Probability(rule) => RuleIdentity::Probability(ProbabilityRuleIdentity {
                min_items: rule.min_items.identity(),
                min_ece_reduction: rule.min_ece_reduction.identity(),
                max_brier_increase: rule.max_brier_increase.identity(),
                confident_error_floor: rule.confident_error_floor.as_deref().map(|floor| {
                    ConfidentErrorFloorIdentity {
                        block_sigma: floor.block_sigma.identity(),
                        block_items: floor.block_items.identity(),
                        sigmas: floor.sigmas.identity(),
                    }
                }),
                pending_measurement: rule.pending_measurement.as_ref().map(Pending::identity),
            }),
            Self::Deployment(rule) => RuleIdentity::Deployment(DeploymentRuleIdentity {
                min_calls: rule.min_calls.identity(),
                gated_percentile: rule.gated_percentile,
                latency_block_sigma_relative: rule.latency_block_sigma_relative.identity(),
                regression_sigmas: rule.regression_sigmas.identity(),
                pending_measurement: rule.pending_measurement.as_ref().map(Pending::identity),
            }),
        }
    }
}

/// A digest an earlier encoding recorded for this rule, bound to the
/// identity it was reviewed against.
///
/// The binding is content, not a claim: `equivalent` is the identity
/// projection the recording was reviewed equal to, and the alias counts
/// only while the live rule still projects to it. A policy edit moves the
/// projection and the alias stops attributing, so a changed rule cannot
/// inherit the earlier encoding's identity by leaving the digest in the
/// list. Nothing recomputes `equivalent`; a rule whose policy changed gets
/// a new gate, and the next alias is written by a reviewer who checked the
/// new identity, never by a save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alias {
    /// The digest the earlier encoding recorded, as `gate:<sha256>`.
    pub digest: String,
    /// The identity projection the recording was reviewed equal to — this
    /// gate's `identity()` at migration time, committed as a JSON document
    /// so a reviewer can diff it against `rule`.
    pub equivalent: Value,
    /// The committed records that pin the digest, so "it was recorded" is a
    /// checkable claim rather than an asserted one. A digest-shaped string
    /// is not proof a digest was ever recorded; the record is.
    pub recorded_in: Vec<String>,
}

impl Alias {
    /// Rejects a binding that is malformed, unverifiable, or stale.
    ///
    /// `identity` is the canonical form of the gate's live projection.
    fn validate(&self, gate: &str, identity: &str) -> Result<(), GateError> {
        if !is_digest(&self.digest) {
            return Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!(
                    "previously carries {}, which is not a recorded digest; the field holds \
                     `gate:<sha256>` values an earlier encoding produced for this rule",
                    self.digest
                ),
            });
        }
        if canonical(&self.equivalent) != identity {
            return Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!(
                    "previously binds {} to an identity this rule no longer projects to; a \
                     changed policy is a new gate, not this one wearing the old digest",
                    self.digest
                ),
            });
        }
        if self.recorded_in.is_empty()
            || self
                .recorded_in
                .iter()
                .any(|record| record.trim().is_empty())
        {
            return Err(GateError::Invalid {
                id: gate.to_string(),
                problem: format!(
                    "previously binds {} without naming a record that carries it; a \
                     digest-shaped string is not proof the digest was recorded",
                    self.digest
                ),
            });
        }
        Ok(())
    }
}

/// Whether a string is shaped like a recorded digest: `gate:` followed by
/// sixty-four lowercase hex characters. Shape is a precondition, never the
/// proof — what proves a digest was recorded is a record that carries it.
fn is_digest(candidate: &str) -> bool {
    candidate.starts_with("gate:")
        && candidate.len() == 69
        && candidate[5..]
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// An acceptance rule, as committed to `crates/gym/gates/`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    /// Rationale prose carried in the file. Excluded from the digest:
    /// commentary on a rule is not the rule, and editing it must not orphan
    /// the rows scored under it.
    #[serde(default, rename = "$comment", skip_serializing)]
    pub comment: Option<Value>,
    /// The document schema.
    pub schema: String,
    /// The gate's id, which is also its file name. Versioned, because
    /// changing a threshold produces a new gate rather than new history.
    pub id: String,
    /// The digests earlier encodings recorded for this same rule, each bound
    /// to the identity it was reviewed equal to, so a verdict pinned to one
    /// still attributes. This is history, not policy, and stays outside the
    /// digest; `docs/gym/gate-digests.md` states the policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previously: Vec<Alias>,
    /// The product question this gate answers, in one line. Outside the
    /// digest: it is prose for the reader, and rewording it does not change
    /// what the rule decides.
    pub question: String,
    /// The thresholds, and what they decide.
    pub rule: Rule,
}

impl Gate {
    /// Reads a gate from JSON.
    ///
    /// # Errors
    ///
    /// Returns [`GateError::Parse`] when the document is not a gate, and
    /// [`GateError::Schema`] or [`GateError::Invalid`] when it parses and
    /// says something this build cannot act on.
    pub fn from_json(source: &str, path: &Path) -> Result<Self, GateError> {
        let gate: Self = serde_json::from_str(source).map_err(|source| GateError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        gate.validate()?;
        Ok(gate)
    }

    /// Reads a gate from a file.
    ///
    /// The file name is the gate id, so a rule cannot be renamed without
    /// being renamed everywhere.
    ///
    /// # Errors
    ///
    /// Returns [`GateError::Read`] when the file cannot be read, and the
    /// errors [`Gate::from_json`] returns otherwise.
    pub fn load(path: &Path) -> Result<Self, GateError> {
        let source = std::fs::read_to_string(path).map_err(|source| GateError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let gate = Self::from_json(&source, path)?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem != gate.id {
            return Err(GateError::Invalid {
                id: gate.id.clone(),
                problem: format!("lives in {stem}.json; the file name is the gate id"),
            });
        }
        Ok(gate)
    }

    /// Reads every gate in a directory, ordered by id.
    ///
    /// # Errors
    ///
    /// Returns [`GateError::Read`] when the directory cannot be listed, and
    /// the errors [`Gate::load`] returns for any file in it. A gate that
    /// does not load is an error rather than a skip: a rule that quietly
    /// disappears is how a verdict gets made by a rule nobody named.
    pub fn load_dir(dir: &Path) -> Result<Vec<Self>, GateError> {
        let entries = std::fs::read_dir(dir).map_err(|source| GateError::Read {
            path: dir.to_path_buf(),
            source,
        })?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| GateError::Read {
                path: dir.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        paths.iter().map(|path| Self::load(path)).collect()
    }

    /// Rejects a document that parsed and says something impossible.
    ///
    /// # Errors
    ///
    /// Returns [`GateError::Schema`] or [`GateError::Invalid`].
    pub fn validate(&self) -> Result<(), GateError> {
        if self.schema != SCHEMA {
            return Err(GateError::Schema {
                id: self.id.clone(),
                found: self.schema.clone(),
            });
        }
        if self.id.trim().is_empty() {
            return Err(GateError::Invalid {
                id: self.id.clone(),
                problem: "has no id; a verdict has to name the rule that produced it".into(),
            });
        }
        if self.question.trim().is_empty() {
            return Err(GateError::Invalid {
                id: self.id.clone(),
                problem: "has no question; a gate that cannot say what it decides decides too much"
                    .into(),
            });
        }
        match &self.rule {
            Rule::Decision(rule) => {
                rule.min_items.validate(&self.id, "min_items")?;
                rule.gain_standard_errors
                    .validate(&self.id, "gain_standard_errors")?;
            }
            Rule::Probability(rule) => {
                rule.min_items.validate(&self.id, "min_items")?;
                rule.min_ece_reduction
                    .validate(&self.id, "min_ece_reduction")?;
                rule.max_brier_increase
                    .validate(&self.id, "max_brier_increase")?;
                if let Some(floor) = &rule.confident_error_floor {
                    floor.validate(&self.id)?;
                }
            }
            Rule::Deployment(rule) => {
                rule.min_calls.validate(&self.id, "min_calls")?;
                rule.latency_block_sigma_relative
                    .validate(&self.id, "latency_block_sigma_relative")?;
                rule.regression_sigmas
                    .validate(&self.id, "regression_sigmas")?;
            }
        }
        if let Some(pending) = self.rule.pending_measurement() {
            pending.validate(&self.id)?;
        }
        let identity = self.identity_canonical();
        for alias in &self.previously {
            alias.validate(&self.id, &identity)?;
        }
        Ok(())
    }

    /// `gate:<sha256>` over the canonical serialization of the rule's
    /// identity — the parts that decide, without the prose that explains
    /// them.
    ///
    /// The digest does not hash the file. It hashes a typed view of it: the
    /// schema, the id, and the rule — every bound's value, basis, and
    /// evidence, the enums that say what a statistic covers, and any pending
    /// measurement's identity. `question`, every `why`, `$comment`, and
    /// `previously` are outside it: a sentence that explains a threshold is
    /// not the threshold, and editing one must not orphan the rows the rule
    /// already scored.
    ///
    /// Object keys are sorted, so the digest does not move when fields are
    /// reordered in a file or in this source. The view carries its own
    /// schema, [`IDENTITY_SCHEMA`], so an encoding change is a declared
    /// transition rather than a silent one: an earlier encoding's digests
    /// live in `previously`, each bound to the identity it was reviewed
    /// equal to, and [`Gate::has_digest`] attributes a verdict recorded
    /// under either.
    #[must_use]
    pub fn digest(&self) -> String {
        let hash = Sha256::digest(self.identity_canonical().as_bytes());
        let mut out = String::with_capacity(5 + hash.len() * 2);
        out.push_str("gate:");
        for byte in hash {
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    /// Whether `recorded` names this rule — the digest the current encoding
    /// produces, or one an earlier encoding recorded whose `previously`
    /// binding resolves to this rule's live identity.
    ///
    /// The check is against the identity as the rule projects it now, so a
    /// gate mutated in place — a threshold moved, a bound relabelled —
    /// stops attributing rows the earlier digest was recorded for. A string
    /// staying in `previously` is not enough; the binding has to hold.
    #[must_use]
    pub fn has_digest(&self, recorded: &str) -> bool {
        if recorded == self.digest() {
            return true;
        }
        let identity = self.identity_canonical();
        self.previously
            .iter()
            .any(|alias| alias.digest == recorded && canonical(&alias.equivalent) == identity)
    }

    /// The canonical form of this gate's identity projection — the string
    /// [`Gate::digest`] hashes, and what a `previously` binding is checked
    /// against.
    fn identity_canonical(&self) -> String {
        canonical(&serde_json::to_value(self.identity()).unwrap_or(Value::Null))
    }

    /// This gate as the digest sees it.
    fn identity(&self) -> GateIdentity<'_> {
        GateIdentity {
            identity_schema: IDENTITY_SCHEMA,
            schema: &self.schema,
            id: &self.id,
            rule: self.rule.identity(),
        }
    }

    /// Judges one comparison of scores. Pure.
    ///
    /// A gate that judges a deployment profile judges nothing here, and says
    /// so rather than passing a comparison it never read.
    #[must_use]
    pub fn judge(&self, comparison: &Comparison) -> Outcome {
        let criteria = match &self.rule {
            Rule::Decision(rule) => judge_decision(rule, comparison),
            Rule::Probability(rule) => judge_probability(rule, comparison),
            Rule::Deployment(_) => vec![wrong_measurement(
                &self.id,
                "a deployment profile: latency, cost, and refusals",
                "a comparison of scores carries none of them",
            )],
        };
        self.outcome(comparison.group.clone(), criteria)
    }

    /// Judges what one door costs to run against what a workload can pay.
    /// Pure.
    ///
    /// A gate that judges scores judges nothing here, for the same reason.
    #[must_use]
    pub fn judge_deployment(&self, deployment: &Deployment) -> Outcome {
        let criteria = match &self.rule {
            Rule::Deployment(rule) => judge_deployment(rule, deployment),
            Rule::Decision(_) | Rule::Probability(_) => vec![wrong_measurement(
                &self.id,
                "a comparison of scores",
                "a deployment profile carries no accuracy and no probabilities",
            )],
        };
        self.outcome(deployment.group.clone(), criteria)
    }

    fn outcome(&self, group: String, criteria: Vec<Criterion>) -> Outcome {
        Outcome {
            gate_id: self.id.clone(),
            gate_digest: self.digest(),
            group,
            verdict: Verdict::over(criteria.iter().map(|criterion| criterion.verdict)),
            criteria,
        }
    }

    /// Judges every comparison and combines the groups.
    ///
    /// One failed group beats any number of passed ones, which is the whole
    /// reason a run verdict is not an average.
    #[must_use]
    pub fn judge_all(&self, comparisons: &[Comparison]) -> Report {
        self.report(comparisons.iter().map(|one| self.judge(one)).collect())
    }

    /// Judges every deployment profile and combines the groups, by the same
    /// precedence.
    #[must_use]
    pub fn judge_all_deployments(&self, deployments: &[Deployment]) -> Report {
        self.report(
            deployments
                .iter()
                .map(|one| self.judge_deployment(one))
                .collect(),
        )
    }

    fn report(&self, groups: Vec<Outcome>) -> Report {
        Report {
            gate_id: self.id.clone(),
            gate_digest: self.digest(),
            verdict: Verdict::over(groups.iter().map(|group| group.verdict)),
            groups,
        }
    }
}

/// The directory the gate files live in.
///
/// Reads `GYM_GATES_DIR` when it is set, and falls back to the committed
/// `gates/` directory beside this crate. The path is a default, not the
/// rule: the rules themselves are loaded from those files rather than
/// compiled in.
#[must_use]
pub fn gates_dir() -> PathBuf {
    std::env::var_os(GATES_DIR_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("gates"))
}

/// Loads one gate by id from [`gates_dir`].
///
/// # Errors
///
/// Returns [`GateError::NotFound`] when no file declares that id, and the
/// errors [`Gate::load`] returns otherwise.
pub fn load(id: &str) -> Result<Gate, GateError> {
    let dir = gates_dir();
    let path = dir.join(format!("{id}.json"));
    if !path.exists() {
        return Err(GateError::NotFound {
            id: id.to_string(),
            dir,
        });
    }
    Gate::load(&path)
}

/// Loads every committed gate, ordered by id.
///
/// # Errors
///
/// Returns the errors [`Gate::load_dir`] returns.
pub fn load_all() -> Result<Vec<Gate>, GateError> {
    Gate::load_dir(&gates_dir())
}

/// One side of a comparison.
///
/// Every measure is optional, and absent means nobody measured it. A
/// criterion over an absent measure is [`Verdict::Unverifiable`], never a
/// pass, because unknown is never zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scores {
    /// How many items were scored.
    pub items: usize,
    /// Share of items whose winning option was the labelled answer.
    pub accuracy: Option<f64>,
    /// Expected calibration error over ten bins.
    pub ece: Option<f64>,
    /// Brier score on the winning option.
    pub brier: Option<f64>,
    /// Negative log likelihood of the winning option's outcome.
    pub nll: Option<f64>,
    /// Items that were wrong at a reported probability of 0.9 or above.
    pub confident_errors: Option<usize>,
}

/// Two sets of scores over the same items, and what produced the candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// What the two sides were scored on: a family, a suite, or a split.
    pub group: String,
    /// The incumbent: the raw signal, or the door in service.
    pub baseline: Scores,
    /// The challenger: the fitted map, or the door proposed to replace it.
    pub candidate: Scores,
    /// How many items the candidate was fitted on, when the candidate is a
    /// fitted map. `None` for a door against a door, where there is nothing
    /// fitted to overfit.
    pub fitted_on: Option<usize>,
}

impl Comparison {
    /// A comparison of two doors, where nothing was fitted.
    #[must_use]
    pub fn new(group: impl Into<String>, baseline: Scores, candidate: Scores) -> Self {
        Self {
            group: group.into(),
            baseline,
            candidate,
            fitted_on: None,
        }
    }

    /// Records how many items the candidate map was fitted on.
    #[must_use]
    pub const fn fitted_on(mut self, items: usize) -> Self {
        self.fitted_on = Some(items);
        self
    }
}

/// What a gate concluded about one criterion, one group, or one run.
///
/// The ordering is the precedence: `failed` beats `unverifiable` beats
/// `passed`. A measured breach is a breach whatever else could not be
/// measured, and something nobody could measure never passes by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Measured, and it held.
    Passed,
    /// Nobody could tell. Not a failure, and it must not be rendered as one.
    Unverifiable,
    /// Measured, and it lost.
    Failed,
}

impl Verdict {
    /// The word used in records and on screen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Unverifiable => "unverifiable",
            Self::Failed => "failed",
        }
    }

    /// Combines verdicts by precedence.
    ///
    /// An empty set is [`Verdict::Unverifiable`]: a gate that judged nothing
    /// has not passed anything.
    #[must_use]
    pub fn over(verdicts: impl IntoIterator<Item = Self>) -> Self {
        verdicts.into_iter().max().unwrap_or(Self::Unverifiable)
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One criterion's verdict, with the measurement it was judged on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    /// The criterion, written as the condition it checks.
    pub name: String,
    /// How much this criterion decides, with 1 the most. In the probability
    /// gates, log loss and confident errors are rank 1 and accuracy is rank
    /// 3: a candidate that is right more often does not buy its way past
    /// being confidently wrong more often.
    pub rank: u8,
    /// What the gate concluded.
    pub verdict: Verdict,
    /// The numbers behind the verdict, in one line.
    pub detail: String,
}

/// A gate's verdict over one group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// The rule that judged it.
    pub gate_id: String,
    /// That rule's content digest, which every row records.
    pub gate_digest: String,
    /// What was judged: a family, a suite, or a split.
    pub group: String,
    /// `failed` beats `unverifiable` beats `passed`.
    pub verdict: Verdict,
    /// Every criterion, in rank order.
    pub criteria: Vec<Criterion>,
}

impl Outcome {
    /// The criteria that kept the gate from passing.
    pub fn breaches(&self) -> impl Iterator<Item = &Criterion> {
        self.criteria
            .iter()
            .filter(|criterion| criterion.verdict != Verdict::Passed)
    }

    /// The highest-ranked criterion that carries the group's verdict.
    #[must_use]
    pub fn deciding(&self) -> Option<&Criterion> {
        self.criteria
            .iter()
            .filter(|criterion| criterion.verdict == self.verdict)
            .min_by_key(|criterion| criterion.rank)
    }
}

/// A gate's verdict over every group it judged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The rule that judged them.
    pub gate_id: String,
    /// That rule's content digest.
    pub gate_digest: String,
    /// `failed` beats `unverifiable` beats `passed`, across groups.
    pub verdict: Verdict,
    /// One outcome per group.
    pub groups: Vec<Outcome>,
}

impl Report {
    /// The groups that kept the gate from passing.
    pub fn breaches(&self) -> impl Iterator<Item = &Outcome> {
        self.groups
            .iter()
            .filter(|group| group.verdict != Verdict::Passed)
    }
}

/// What the digest sees of a [`Gate`]: the parts that decide, and nothing
/// that only explains.
///
/// The gate file carries prose for a reader — `question`, every `why`, the
/// `$comment` — and this view does not. The digest covers the schema, the
/// id, and the rule: each bound's value, basis, and evidence, the enums
/// that say what a statistic covers, and the identity of any pending
/// measurement. Editing an explanation never re-identifies the rule;
/// changing what it decides, or the records it rests on, always does.
#[derive(Serialize)]
struct GateIdentity<'a> {
    /// This view's own schema, so the digest records what produced it.
    identity_schema: &'static str,
    /// The gate file's schema.
    schema: &'a str,
    /// The gate's id, which is also its file name.
    id: &'a str,
    /// The rule, projected the same way.
    rule: RuleIdentity<'a>,
}

/// A rule as the digest sees it: the same `decides` tag the file writes,
/// and the same fields minus the prose.
#[derive(Serialize)]
#[serde(tag = "decides", rename_all = "snake_case")]
enum RuleIdentity<'a> {
    /// The rule that decides which door answers.
    Decision(DecisionRuleIdentity<'a>),
    /// The rule that decides whether probabilities may be served.
    Probability(ProbabilityRuleIdentity<'a>),
    /// The rule that decides whether a door can be afforded.
    Deployment(DeploymentRuleIdentity<'a>),
}

/// A decision rule's identity.
#[derive(Serialize)]
struct DecisionRuleIdentity<'a> {
    min_items: BoundIdentity<'a>,
    gain_standard_errors: BoundIdentity<'a>,
    variance_basis: VarianceBasis,
    pending_measurement: Option<PendingIdentity<'a>>,
}

/// A probability rule's identity.
///
/// `confident_error_floor` is written only when the rule carries one, so
/// `probability-v1`, which carries none, projects exactly as it did before
/// the field existed and keeps the digest its records name.
#[derive(Serialize)]
struct ProbabilityRuleIdentity<'a> {
    min_items: BoundIdentity<'a>,
    min_ece_reduction: BoundIdentity<'a>,
    max_brier_increase: BoundIdentity<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confident_error_floor: Option<ConfidentErrorFloorIdentity<'a>>,
    pending_measurement: Option<PendingIdentity<'a>>,
}

/// A confident-error floor's identity: its three bounds, minus their prose.
#[derive(Serialize)]
struct ConfidentErrorFloorIdentity<'a> {
    block_sigma: BoundIdentity<'a>,
    block_items: BoundIdentity<'a>,
    sigmas: BoundIdentity<'a>,
}

/// A deployment rule's identity.
#[derive(Serialize)]
struct DeploymentRuleIdentity<'a> {
    min_calls: BoundIdentity<'a>,
    gated_percentile: GatedPercentile,
    latency_block_sigma_relative: BoundIdentity<'a>,
    regression_sigmas: BoundIdentity<'a>,
    pending_measurement: Option<PendingIdentity<'a>>,
}

/// A bound as the digest sees it: the number, where the number came from,
/// and the records it rests on. `why` explains them to a reader and is not
/// part of the rule's identity.
#[derive(Serialize)]
struct BoundIdentity<'a> {
    value: Option<f64>,
    basis: Basis,
    evidence: &'a [Evidence],
}

/// A pending measurement as the digest sees it: what is unmeasured and who
/// takes it, without the prose about what the rule does meanwhile.
#[derive(Serialize)]
struct PendingIdentity<'a> {
    quantity: &'a str,
    issue: Option<&'a str>,
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
        // Numbers normalize through f64 so `30` and `30.0` write the same
        // way — an equivalence snapshot may carry either for what the
        // projection serializes as a float.
        Value::Number(number) => match number.as_f64().and_then(Number::from_f64) {
            Some(float) => out.push_str(&float.to_string()),
            None => out.push_str(&number.to_string()),
        },
        other => out.push_str(&other.to_string()),
    }
}

/// The canonical form of a JSON document: sorted keys, normalized numbers.
fn canonical(value: &Value) -> String {
    let mut out = String::new();
    canonicalize(value, &mut out);
    out
}

/// A criterion nothing downstream of a missing floor is judged against.
fn not_judged(name: String, rank: u8, reason: &str) -> Criterion {
    Criterion {
        name,
        rank,
        verdict: Verdict::Unverifiable,
        detail: format!("not judged: {reason}"),
    }
}

/// The floor criterion, and the reason downstream criteria are not judged.
fn items_floor(name: &str, rank: u8, bound: &Bound, items: usize) -> (Criterion, Option<String>) {
    let Some(floor) = bound.count() else {
        let reason = format!("no floor has been measured for {name} ({})", bound.why);
        return (
            Criterion {
                name: format!("{name}>=?"),
                rank,
                verdict: Verdict::Unverifiable,
                detail: reason.clone(),
            },
            Some(reason),
        );
    };
    if items < floor {
        let reason = format!("{name} is {items}, below the floor of {floor}");
        return (
            Criterion {
                name: format!("{name}>={floor}"),
                rank,
                verdict: Verdict::Unverifiable,
                detail: format!("{reason}, which is too few to tell"),
            },
            Some(reason),
        );
    }
    (
        Criterion {
            name: format!("{name}>={floor}"),
            rank,
            verdict: Verdict::Passed,
            detail: format!("{name} is {items}, at or above the floor of {floor}"),
        },
        None,
    )
}

fn judge_decision(rule: &DecisionRule, comparison: &Comparison) -> Vec<Criterion> {
    let (floor, blocked) = items_floor(
        "scored_items",
        1,
        &rule.min_items,
        comparison.candidate.items,
    );
    let mut criteria = vec![floor];
    let baseline = comparison.baseline;
    let candidate = comparison.candidate;

    if let Some(reason) = blocked {
        criteria.push(not_judged("accuracy_does_not_fall".into(), 1, &reason));
        criteria.push(not_judged(
            "accuracy_gain_clears_the_noise".into(),
            2,
            &reason,
        ));
        return criteria;
    }

    let (Some(before), Some(after)) = (baseline.accuracy, candidate.accuracy) else {
        let reason = "accuracy was not measured on both sides";
        criteria.push(not_judged("accuracy_does_not_fall".into(), 1, reason));
        criteria.push(not_judged(
            "accuracy_gain_clears_the_noise".into(),
            2,
            reason,
        ));
        return criteria;
    };

    criteria.push(Criterion {
        name: "accuracy_does_not_fall".into(),
        rank: 1,
        verdict: if after < before {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        detail: format!(
            "accuracy {before:.3} to {after:.3} over {} items",
            candidate.items
        ),
    });

    criteria.push(gain_criterion(rule, &baseline, &candidate, before, after));
    criteria
}

fn gain_criterion(
    rule: &DecisionRule,
    baseline: &Scores,
    candidate: &Scores,
    before: f64,
    after: f64,
) -> Criterion {
    let name = "accuracy_gain_clears_the_noise".to_string();
    let Some(multiple) = rule.gain_standard_errors.value() else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "no effect size has been measured for this suite ({})",
                rule.gain_standard_errors.why
            ),
        };
    };
    let Some(error) = standard_error(baseline, before, candidate, after) else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "the standard error behind this bound needs at least five expected outcomes on \
                 each side, and {} items at {before:.3} against {} at {after:.3} do not reach it",
                baseline.items, candidate.items
            ),
        };
    };
    let gain = after - before;
    let bound = multiple * error;
    let covers = rule.variance_basis.as_str();
    let detail = format!(
        "accuracy gain {gain:+.3} against {multiple:.1} standard errors of {error:.3}, which is \
         {bound:.3}; the standard error covers {covers} only"
    );
    Criterion {
        name,
        rank: 2,
        verdict: if gain >= bound {
            Verdict::Passed
        } else {
            Verdict::Unverifiable
        },
        detail,
    }
}

/// The unpaired standard error of the difference between two accuracies.
///
/// The two doors answer the same items, so their errors are correlated and
/// this overstates the noise. That is the conservative direction, and it is
/// what the gate should do while the paired spread is unmeasured.
///
/// `None` when the normal approximation behind it does not hold, which needs
/// at least five expected outcomes on each side of each door.
fn standard_error(baseline: &Scores, before: f64, candidate: &Scores, after: f64) -> Option<f64> {
    let variance = |items: usize, rate: f64| -> Option<f64> {
        if !(0.0..=1.0).contains(&rate) || items == 0 {
            return None;
        }
        let items = items as f64;
        if items * rate < 5.0 || items * (1.0 - rate) < 5.0 {
            return None;
        }
        Some(rate * (1.0 - rate) / items)
    };
    let left = variance(baseline.items, before)?;
    let right = variance(candidate.items, after)?;
    Some((left + right).sqrt())
}

fn judge_probability(rule: &ProbabilityRule, comparison: &Comparison) -> Vec<Criterion> {
    let mut criteria = Vec::new();
    let mut blocked: Option<String> = None;

    if let Some(fitted) = comparison.fitted_on {
        let (criterion, reason) = items_floor("fitted_on", 1, &rule.min_items, fitted);
        criteria.push(criterion);
        blocked = blocked.or(reason);
    }
    let (criterion, reason) = items_floor(
        "scored_items",
        1,
        &rule.min_items,
        comparison.candidate.items,
    );
    criteria.push(criterion);
    let blocked = blocked.or(reason);

    let baseline = comparison.baseline;
    let candidate = comparison.candidate;

    criteria.push(direction(
        "log_loss_does_not_rise",
        1,
        blocked.as_deref(),
        baseline.nll,
        candidate.nll,
        "log loss",
    ));
    criteria.push(confident_errors_within_floor(
        rule.confident_error_floor.as_deref(),
        blocked.as_deref(),
        &baseline,
        &candidate,
    ));
    criteria.push(direction(
        "ece_does_not_rise",
        2,
        blocked.as_deref(),
        baseline.ece,
        candidate.ece,
        "ECE",
    ));
    criteria.push(ece_reduction(
        rule,
        blocked.as_deref(),
        &baseline,
        &candidate,
    ));
    criteria.push(brier_tolerance(
        rule,
        blocked.as_deref(),
        &baseline,
        &candidate,
    ));
    criteria.push(direction(
        "accuracy_does_not_fall",
        3,
        blocked.as_deref(),
        baseline.accuracy.map(|accuracy| 1.0 - accuracy),
        candidate.accuracy.map(|accuracy| 1.0 - accuracy),
        "error rate",
    ));
    criteria
}

/// A measure that must not rise. Rising is a measured loss, so it fails.
fn direction(
    name: &str,
    rank: u8,
    blocked: Option<&str>,
    before: Option<f64>,
    after: Option<f64>,
    measure: &str,
) -> Criterion {
    if let Some(reason) = blocked {
        return not_judged(name.to_string(), rank, reason);
    }
    let (Some(before), Some(after)) = (before, after) else {
        return Criterion {
            name: name.to_string(),
            rank,
            verdict: Verdict::Unverifiable,
            detail: format!("{measure} was not measured on both sides"),
        };
    };
    Criterion {
        name: name.to_string(),
        rank,
        verdict: if after > before {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        detail: format!("{measure} {before:.3} to {after:.3}"),
    }
}

/// The confident-error count must not rise beyond its seed noise.
///
/// Without a floor the two counts are compared directly, which is what
/// `probability-v1` recorded and what openagents#9401 measured refusing an
/// unchanged door half the time. With a floor whose spread is unmeasured the
/// criterion is unverifiable rather than judged on a number nobody took.
#[allow(clippy::cast_precision_loss)]
fn confident_errors_within_floor(
    floor: Option<&ConfidentErrorFloor>,
    blocked: Option<&str>,
    baseline: &Scores,
    candidate: &Scores,
) -> Criterion {
    let name = "confident_errors_do_not_rise".to_string();
    let rank = 1;
    if let Some(reason) = blocked {
        return not_judged(name, rank, reason);
    }
    let (Some(before), Some(after)) = (baseline.confident_errors, candidate.confident_errors)
    else {
        return Criterion {
            name,
            rank,
            verdict: Verdict::Unverifiable,
            detail: "confident errors were not counted on both sides".into(),
        };
    };
    let items = candidate.items;
    let Some(floor) = floor else {
        return Criterion {
            name,
            rank,
            verdict: if after > before {
                Verdict::Failed
            } else {
                Verdict::Passed
            },
            // Recorded verdict lines regenerate from this text; do not change it.
            detail: format!("confident errors {before} to {after} over {items} items"),
        };
    };
    let Some(allowance) = floor.allowance(baseline.items, items) else {
        return Criterion {
            name,
            rank,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "confident errors {before} to {after} over {items} items; no seed noise floor has \
                 been measured for the count ({})",
                floor.block_sigma.why
            ),
        };
    };
    let rise = after as f64 - before as f64;
    Criterion {
        name,
        rank,
        verdict: if rise > allowance {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        detail: format!(
            "confident errors {before} to {after} over {items} items; a rise of {rise:+.0} against \
             a seed noise floor of {allowance:.2}"
        ),
    }
}

fn ece_reduction(
    rule: &ProbabilityRule,
    blocked: Option<&str>,
    baseline: &Scores,
    candidate: &Scores,
) -> Criterion {
    let name = "ece_reduction_clears_the_margin".to_string();
    if let Some(reason) = blocked {
        return not_judged(name, 2, reason);
    }
    let Some(margin) = rule.min_ece_reduction.value() else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "no reduction margin has been measured ({})",
                rule.min_ece_reduction.why
            ),
        };
    };
    let (Some(before), Some(after)) = (baseline.ece, candidate.ece) else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: "ECE was not measured on both sides".into(),
        };
    };
    if before <= 0.0 {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!("the baseline ECE is {before:.3}, so there is no reduction to measure"),
        };
    }
    let reduction = (before - after) / before;
    Criterion {
        name,
        rank: 2,
        verdict: if reduction >= margin {
            Verdict::Passed
        } else {
            Verdict::Unverifiable
        },
        detail: format!(
            "ECE {before:.3} to {after:.3}, a reduction of {:.0}% against the {:.0}% a candidate \
             has to earn",
            reduction * 100.0,
            margin * 100.0
        ),
    }
}

fn brier_tolerance(
    rule: &ProbabilityRule,
    blocked: Option<&str>,
    baseline: &Scores,
    candidate: &Scores,
) -> Criterion {
    let name = "brier_stays_within_tolerance".to_string();
    if let Some(reason) = blocked {
        return not_judged(name, 2, reason);
    }
    let Some(tolerance) = rule.max_brier_increase.value() else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "no tolerance has been measured ({})",
                rule.max_brier_increase.why
            ),
        };
    };
    let (Some(before), Some(after)) = (baseline.brier, candidate.brier) else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: "Brier was not measured on both sides".into(),
        };
    };
    let ceiling = before * (1.0 + tolerance);
    Criterion {
        name,
        rank: 2,
        verdict: if after > ceiling {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        detail: format!(
            "Brier {before:.3} to {after:.3} against a ceiling of {ceiling:.3}, the {:.0}% the \
             binning is allowed to cost",
            tolerance * 100.0
        ),
    }
}

/// A gate asked to judge a measurement it does not read.
///
/// Unverifiable rather than failed: the door did nothing wrong, and the
/// caller reached for the wrong rule.
fn wrong_measurement(gate: &str, judges: &str, and: &str) -> Criterion {
    Criterion {
        name: "measurement_matches_the_rule".to_string(),
        rank: 1,
        verdict: Verdict::Unverifiable,
        detail: format!("{gate} judges {judges}, and {and}"),
    }
}

fn judge_deployment(rule: &DeploymentRule, deployment: &Deployment) -> Vec<Criterion> {
    let (floor, blocked) = items_floor(
        "timed_calls",
        1,
        &rule.min_calls,
        deployment.candidate.calls,
    );
    let blocked = blocked.as_deref();
    let budget = deployment.budget.as_ref();
    vec![
        floor,
        latency_ceiling(rule, blocked, budget, &deployment.candidate),
        cost_ceiling(blocked, budget, &deployment.candidate),
        refusal_ceiling(rule, blocked, budget, &deployment.candidate),
        latency_regression(rule, blocked, &deployment.baseline, &deployment.candidate),
        refusal_regression(rule, blocked, &deployment.baseline, &deployment.candidate),
    ]
}

/// The band inside which a latency move is the machine rather than the door.
///
/// `None` when the rule carries no measured spread, in which case nothing
/// that depends on it is judged.
fn latency_band(rule: &DeploymentRule, of: f64) -> Option<f64> {
    let sigma = rule.latency_block_sigma_relative.value()?;
    let sigmas = rule.regression_sigmas.value()?;
    Some(sigmas * sigma * of.abs())
}

/// What the band is worth saying about itself, every time it is used.
fn band_note(rule: &DeploymentRule) -> String {
    let sigmas = rule.regression_sigmas.value().unwrap_or_default();
    let sigma = rule
        .latency_block_sigma_relative
        .value()
        .unwrap_or_default();
    format!(
        "the band is {sigmas:.1} block-to-block standard deviations of {:.1}% and was measured \
         on one door on one machine",
        sigma * 100.0
    )
}

/// The ceiling criteria all report the same thing when nobody stated one.
fn no_ceiling(name: String, rank: u8, budget: Option<&Budget>, what: &str) -> Criterion {
    let detail = match budget {
        None => format!(
            "no budget was stated, and {what} is workload-dependent: a batch job tolerates what \
             a router in front of every turn does not. A gate that picks a number here is \
             inventing one"
        ),
        Some(budget) => format!(
            "the {} budget states no {what} ceiling ({})",
            budget.workload, budget.source
        ),
    };
    Criterion {
        name,
        rank,
        verdict: Verdict::Unverifiable,
        detail,
    }
}

fn latency_ceiling(
    rule: &DeploymentRule,
    blocked: Option<&str>,
    budget: Option<&Budget>,
    candidate: &Profile,
) -> Criterion {
    let percentile = rule.gated_percentile;
    let name = format!("latency_{percentile}_within_budget");
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let Some(ceiling) = budget.and_then(|budget| budget.max_latency_ms) else {
        return no_ceiling(name, 1, budget, "a latency ceiling");
    };
    let workload = budget.map_or("", |budget| budget.workload.as_str());
    let Some(measured) = percentile.read(candidate) else {
        return Criterion {
            name,
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "the run recorded no {percentile}; a median is not an answer to a tail question, \
                 and the tail is what a caller waits through"
            ),
        };
    };
    let Some(band) = latency_band(rule, measured) else {
        return Criterion {
            name,
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "no block-to-block latency spread has been measured ({})",
                rule.latency_block_sigma_relative.why
            ),
        };
    };
    let verdict = if measured > ceiling + band {
        Verdict::Failed
    } else if measured <= ceiling - band {
        Verdict::Passed
    } else {
        Verdict::Unverifiable
    };
    Criterion {
        name,
        rank: 1,
        verdict,
        detail: format!(
            "{percentile} {measured:.0} ms against the {workload} ceiling of {ceiling:.0} ms, \
             give or take {band:.0} ms; {}",
            band_note(rule)
        ),
    }
}

fn cost_ceiling(blocked: Option<&str>, budget: Option<&Budget>, candidate: &Profile) -> Criterion {
    let name = "cost_per_decision_within_budget".to_string();
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let Some(ceiling) = budget.and_then(|budget| budget.max_cost_per_decision_usd) else {
        return no_ceiling(name, 1, budget, "a price ceiling");
    };
    let workload = budget.map_or("", |budget| budget.workload.as_str());
    let Some(cost) = candidate.cost else {
        return Criterion {
            name,
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: "nothing recorded what a decision on this door costs".into(),
        };
    };
    let Some(price) = cost.usd_per_decision() else {
        return Criterion {
            name,
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "the lane is {}: it runs on hardware we own and nothing meters it. An unmetered \
                 lane has no price to compare with the {workload} ceiling of \
                 ${ceiling:.6} per decision, and the absence of a meter is not a price of zero",
                cost.as_str()
            ),
        };
    };
    Criterion {
        name,
        rank: 1,
        verdict: if price > ceiling {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        detail: format!(
            "${price:.6} per decision against the {workload} ceiling of ${ceiling:.6}, which is \
             ${:.2} against ${:.2} per 100,000 decisions",
            price * 100_000.0,
            ceiling * 100_000.0
        ),
    }
}

fn refusal_ceiling(
    rule: &DeploymentRule,
    blocked: Option<&str>,
    budget: Option<&Budget>,
    candidate: &Profile,
) -> Criterion {
    let name = "refusal_rate_within_budget".to_string();
    if let Some(reason) = blocked {
        return not_judged(name, 1, reason);
    }
    let Some(ceiling) = budget.and_then(|budget| budget.max_refusal_rate) else {
        return no_ceiling(name, 1, budget, "a refusal ceiling");
    };
    let workload = budget.map_or("", |budget| budget.workload.as_str());
    let Some(rate) = candidate.refusal_rate() else {
        return Criterion {
            name,
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: "refusals were not counted, and an uncounted refusal is not a refusal of \
                     zero"
                .into(),
        };
    };
    let Some(sigmas) = rule.regression_sigmas.value() else {
        return Criterion {
            name,
            rank: 1,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "no effect size has been recorded ({})",
                rule.regression_sigmas.why
            ),
        };
    };
    // The standard error of the ceiling rather than of the observed one, so
    // a door that refused nothing is still judged against a real interval.
    #[allow(clippy::cast_precision_loss)]
    let error = (ceiling * (1.0 - ceiling) / candidate.calls as f64).sqrt();
    let band = sigmas * error;
    let verdict = if rate > ceiling + band {
        Verdict::Failed
    } else if rate <= ceiling - band {
        Verdict::Passed
    } else {
        Verdict::Unverifiable
    };
    Criterion {
        name,
        rank: 1,
        verdict,
        detail: format!(
            "{} of {} calls declined, {:.1}%, against the {workload} ceiling of {:.1}%, give or \
             take {:.1} points at {sigmas:.1} binomial standard errors of that ceiling",
            candidate.refusals.unwrap_or_default(),
            candidate.calls,
            rate * 100.0,
            ceiling * 100.0,
            band * 100.0
        ),
    }
}

fn latency_regression(
    rule: &DeploymentRule,
    blocked: Option<&str>,
    baseline: &Profile,
    candidate: &Profile,
) -> Criterion {
    let percentile = rule.gated_percentile;
    let name = format!("latency_{percentile}_does_not_rise");
    if let Some(reason) = blocked {
        return not_judged(name, 2, reason);
    }
    let (Some(before), Some(after)) = (percentile.read(baseline), percentile.read(candidate))
    else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!("{percentile} was not measured on both sides"),
        };
    };
    if after <= before {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Passed,
            detail: format!("{percentile} {before:.0} ms to {after:.0} ms"),
        };
    }
    // A difference carries both sides' spread, and the spread is relative,
    // so the slower door contributes more of it.
    let Some(band) = latency_band(rule, before.hypot(after)) else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "no block-to-block latency spread has been measured ({})",
                rule.latency_block_sigma_relative.why
            ),
        };
    };
    Criterion {
        name,
        rank: 2,
        verdict: if after > before + band {
            Verdict::Failed
        } else {
            Verdict::Unverifiable
        },
        detail: format!(
            "{percentile} {before:.0} ms to {after:.0} ms, a rise of {:.0} ms against a band of \
             {band:.0} ms; {}",
            after - before,
            band_note(rule)
        ),
    }
}

fn refusal_regression(
    rule: &DeploymentRule,
    blocked: Option<&str>,
    baseline: &Profile,
    candidate: &Profile,
) -> Criterion {
    let name = "refusal_rate_does_not_rise".to_string();
    if let Some(reason) = blocked {
        return not_judged(name, 2, reason);
    }
    let (Some(before), Some(after)) = (baseline.refusal_rate(), candidate.refusal_rate()) else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: "refusals were not counted on both sides, and an uncounted refusal is not a \
                     refusal of zero"
                .into(),
        };
    };
    if after <= before {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Passed,
            detail: format!(
                "refusal rate {:.1}% to {:.1}%",
                before * 100.0,
                after * 100.0
            ),
        };
    }
    let baseline_scores = Scores {
        items: baseline.calls,
        ..Scores::default()
    };
    let candidate_scores = Scores {
        items: candidate.calls,
        ..Scores::default()
    };
    let (Some(error), Some(sigmas)) = (
        standard_error(&baseline_scores, before, &candidate_scores, after),
        rule.regression_sigmas.value(),
    ) else {
        return Criterion {
            name,
            rank: 2,
            verdict: Verdict::Unverifiable,
            detail: format!(
                "the standard error behind this bound needs at least five expected refusals and \
                 five expected answers on each side, and {} calls at {:.1}% against {} at \
                 {:.1}% do not reach it",
                baseline.calls,
                before * 100.0,
                candidate.calls,
                after * 100.0
            ),
        };
    };
    let band = sigmas * error;
    Criterion {
        name,
        rank: 2,
        verdict: if after > before + band {
            Verdict::Failed
        } else {
            Verdict::Unverifiable
        },
        detail: format!(
            "refusal rate {:.1}% to {:.1}% against {sigmas:.1} standard errors of {:.1} points, \
             which is {:.1} points",
            before * 100.0,
            after * 100.0,
            error * 100.0,
            band * 100.0
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision() -> Gate {
        load("decision-v1").expect("decision-v1 loads")
    }

    fn probability() -> Gate {
        load("probability-v1").expect("probability-v1 loads")
    }

    fn probability_v2() -> Gate {
        load("probability-v2").expect("probability-v2 loads")
    }

    /// One seed block of the unchanged `lev-base` door on `support-v2`'s
    /// evaluation split, with the confident-error count the block drew.
    /// `docs/lev/measurements/2026-09-19-calibration-variance.md`.
    fn unchanged_block(confident_errors: usize) -> Scores {
        Scores {
            items: 98,
            accuracy: Some(0.77),
            ece: Some(0.120),
            brier: Some(0.182),
            nll: Some(2.036),
            confident_errors: Some(confident_errors),
        }
    }

    fn confident_errors_criterion(gate: &Gate, comparison: &Comparison) -> Criterion {
        gate.judge(comparison)
            .criteria
            .into_iter()
            .find(|criterion| criterion.name == "confident_errors_do_not_rise")
            .expect("the probability gate judges confident errors")
    }

    /// The eight confident-error counts the unchanged door drew.
    const UNCHANGED_DOOR_BLOCKS: [usize; 8] = [6, 6, 10, 4, 4, 10, 6, 4];

    #[test]
    fn probability_v1_refuses_the_unchanged_door_about_half_the_time() {
        // Every ordered pair of blocks is one comparison of the door against
        // itself. Under a raw count comparison, the pairs where the second
        // block drew more confident errors fail: 21 of 56, which is one
        // direction of every pair whose counts differ.
        let mut failed = 0;
        let mut pairs = 0;
        for (left, before) in UNCHANGED_DOOR_BLOCKS.iter().enumerate() {
            for (right, after) in UNCHANGED_DOOR_BLOCKS.iter().enumerate() {
                if left == right {
                    continue;
                }
                pairs += 1;
                let comparison = Comparison::new(
                    "support-v2",
                    unchanged_block(*before),
                    unchanged_block(*after),
                );
                if confident_errors_criterion(&probability(), &comparison).verdict
                    == Verdict::Failed
                {
                    failed += 1;
                }
            }
        }
        assert_eq!(pairs, 56);
        assert_eq!(failed, 21, "the raw count refuses the door against itself");
    }

    #[test]
    fn probability_v2_passes_the_unchanged_door_on_every_pair_of_blocks() {
        for before in UNCHANGED_DOOR_BLOCKS {
            for after in UNCHANGED_DOOR_BLOCKS {
                let comparison = Comparison::new(
                    "support-v2",
                    unchanged_block(before),
                    unchanged_block(after),
                );
                let criterion = confident_errors_criterion(&probability_v2(), &comparison);
                assert_eq!(
                    criterion.verdict,
                    Verdict::Passed,
                    "{before} to {after}: {}",
                    criterion.detail
                );
                // Two sigmas of the difference of two 98-item counts at a
                // block spread of 2.4928 is 7.05, the bound the record
                // published as seven.
                assert!(
                    criterion.detail.contains("floor of 7.05"),
                    "{}",
                    criterion.detail
                );
            }
        }
    }

    #[test]
    fn a_rise_beyond_the_seed_floor_still_fails_probability_v2() {
        let comparison = Comparison::new("support-v2", unchanged_block(1), unchanged_block(9));
        let criterion = confident_errors_criterion(&probability_v2(), &comparison);
        assert_eq!(criterion.verdict, Verdict::Failed, "{}", criterion.detail);

        let inside = Comparison::new("support-v2", unchanged_block(6), unchanged_block(8));
        let criterion = confident_errors_criterion(&probability_v2(), &inside);
        assert_eq!(
            criterion.verdict,
            Verdict::Passed,
            "six to eight is two items inside a spread of 2.49: {}",
            criterion.detail
        );
        assert_eq!(
            confident_errors_criterion(&probability(), &inside).verdict,
            Verdict::Failed,
            "the same two items fail the raw count"
        );
    }

    #[test]
    fn the_seed_floor_scales_with_the_items_under_judgment() {
        // 2.4928 · √(40/98) is 1.5926 a side; two sigmas of the difference
        // is 4.50. Five more confident errors on forty items fail, four pass.
        let forty = |confident_errors: usize| Scores {
            items: 40,
            confident_errors: Some(confident_errors),
            ..unchanged_block(0)
        };
        let four = Comparison::new("routing", forty(2), forty(6)).fitted_on(40);
        let criterion = confident_errors_criterion(&probability_v2(), &four);
        assert_eq!(criterion.verdict, Verdict::Passed, "{}", criterion.detail);
        assert!(
            criterion.detail.contains("floor of 4.50"),
            "{}",
            criterion.detail
        );
        let five = Comparison::new("routing", forty(2), forty(7)).fitted_on(40);
        assert_eq!(
            confident_errors_criterion(&probability_v2(), &five).verdict,
            Verdict::Failed
        );

        let Rule::Probability(rule) = &probability_v2().rule else {
            panic!("probability-v2 carries a probability rule");
        };
        let floor = rule
            .confident_error_floor
            .as_ref()
            .expect("probability-v2 carries the floor");
        let allowance = floor.allowance(98, 98).expect("the floor is measured");
        assert!((allowance - 2.0 * 2.4928 * 2.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn an_unmeasured_seed_floor_leaves_confident_errors_unverifiable() {
        // A rule that names the floor and has not measured it judges nothing
        // on the count: neither the raw comparison v1 recorded nor a pass.
        let mut gate = probability_v2();
        let Rule::Probability(rule) = &mut gate.rule else {
            panic!("probability-v2 carries a probability rule");
        };
        let floor = rule
            .confident_error_floor
            .as_mut()
            .expect("probability-v2 carries the floor");
        floor.block_sigma.value = None;
        floor.block_sigma.basis = Basis::Unmeasured;
        floor.block_sigma.evidence.clear();
        gate.validate()
            .expect("an unmeasured floor is a valid rule");
        for (before, after) in [(6, 8), (8, 6)] {
            let comparison = Comparison::new(
                "support-v2",
                unchanged_block(before),
                unchanged_block(after),
            );
            let criterion = confident_errors_criterion(&gate, &comparison);
            assert_eq!(
                criterion.verdict,
                Verdict::Unverifiable,
                "{}",
                criterion.detail
            );
            assert!(
                criterion
                    .detail
                    .contains("no seed noise floor has been measured"),
                "{}",
                criterion.detail
            );
        }
    }

    #[test]
    fn uncounted_confident_errors_are_unverifiable_under_both_probability_gates() {
        let uncounted = Scores {
            confident_errors: None,
            ..unchanged_block(0)
        };
        let comparison = Comparison::new("support-v2", uncounted, uncounted);
        for gate in [probability(), probability_v2()] {
            assert_eq!(
                confident_errors_criterion(&gate, &comparison).verdict,
                Verdict::Unverifiable,
                "{}",
                gate.id
            );
        }
    }

    #[test]
    fn the_seed_floor_is_inside_probability_v2s_identity_and_absent_from_v1s() {
        let v1 = probability();
        assert!(
            !v1.identity_canonical().contains("confident_error_floor"),
            "a rule without a floor projects as it did before the field existed"
        );
        let v2 = probability_v2();
        assert!(v2.identity_canonical().contains("confident_error_floor"));
        assert_ne!(v1.digest(), v2.digest());

        let mut widened = v2.clone();
        let Rule::Probability(rule) = &mut widened.rule else {
            panic!("probability-v2 carries a probability rule");
        };
        rule.confident_error_floor
            .as_mut()
            .expect("the floor")
            .sigmas
            .value = Some(3.0);
        assert_ne!(v2.digest(), widened.digest(), "a wider floor is a new rule");

        let mut reworded = v2.clone();
        let Rule::Probability(rule) = &mut reworded.rule else {
            panic!("probability-v2 carries a probability rule");
        };
        rule.confident_error_floor
            .as_mut()
            .expect("the floor")
            .block_sigma
            .why = "a different sentence about the same eight blocks".into();
        assert_eq!(
            v2.digest(),
            reworded.digest(),
            "the floor's prose is outside the digest"
        );
    }

    #[test]
    fn a_seed_floor_measured_over_no_items_is_refused() {
        let mut gate = probability_v2();
        let Rule::Probability(rule) = &mut gate.rule else {
            panic!("probability-v2 carries a probability rule");
        };
        rule.confident_error_floor
            .as_mut()
            .expect("the floor")
            .block_items
            .value = Some(0.0);
        let error = gate.validate().unwrap_err();
        assert!(matches!(error, GateError::Invalid { .. }), "{error}");
    }

    /// The base door on the evaluation split, with its one admitted map.
    /// `docs/lev/measurements/2026-09-19-adapter-v1.md`.
    fn base_calibrated() -> Scores {
        Scores {
            items: 98,
            accuracy: Some(0.77),
            ece: Some(0.065),
            brier: Some(0.153),
            nll: Some(0.713),
            confident_errors: Some(1),
        }
    }

    /// The adapted door on the same split, with its two admitted maps.
    fn adapted_calibrated() -> Scores {
        Scores {
            items: 98,
            accuracy: Some(0.90),
            ece: Some(0.090),
            brier: Some(0.098),
            nll: Some(1.115),
            confident_errors: Some(9),
        }
    }

    fn verdict_of(gate: &Gate, comparison: &Comparison) -> Verdict {
        gate.judge(comparison).verdict
    }

    #[test]
    fn every_committed_gate_loads_and_validates() {
        let gates = load_all().expect("the committed gates load");
        let ids: Vec<&str> = gates.iter().map(|gate| gate.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "decision-v1",
                "deployment-v1",
                "deployment-v2",
                "probability-v1",
                "probability-v2"
            ]
        );
        for gate in &gates {
            assert_eq!(gate.schema, SCHEMA);
            assert!(gate.schema.starts_with(crate::SCHEMA_PREFIX));
            assert!(gate.digest().starts_with("gate:"));
            assert_eq!(gate.digest().len(), 69);
        }
    }

    #[test]
    fn the_digest_ignores_comment() {
        let gate = decision();
        assert!(gate.comment.is_some(), "the committed gate carries prose");
        let mut stripped = gate.clone();
        stripped.comment = None;
        assert_eq!(gate.digest(), stripped.digest());

        let mut rewritten = gate.clone();
        rewritten.comment = Some(Value::String("different prose entirely".into()));
        assert_eq!(gate.digest(), rewritten.digest());
    }

    #[test]
    fn the_digest_is_stable_across_serialization_order() {
        let ordered = r#"{
            "schema": "openagents.gym.gate.v2",
            "id": "example-v1",
            "question": "Does key order change the rule?",
            "rule": {
                "decides": "decision",
                "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
                "gain_standard_errors": {
                    "value": 2.0, "basis": "convention", "why": "a multiple"
                },
                "variance_basis": "item_sampling",
                "pending_measurement": null
            }
        }"#;
        let shuffled = r#"{
            "rule": {
                "pending_measurement": null,
                "variance_basis": "item_sampling",
                "gain_standard_errors": {
                    "why": "a multiple", "basis": "convention", "value": 2.0
                },
                "min_items": { "basis": "derived", "why": "a floor", "value": 10 },
                "decides": "decision"
            },
            "question": "Does key order change the rule?",
            "id": "example-v1",
            "$comment": "prose that arrives first and still does not count",
            "schema": "openagents.gym.gate.v2"
        }"#;
        let path = Path::new("example-v1.json");
        let left = Gate::from_json(ordered, path).expect("the ordered document loads");
        let right = Gate::from_json(shuffled, path).expect("the shuffled document loads");
        assert_eq!(left.digest(), right.digest());
    }

    #[test]
    fn widening_a_tolerance_produces_a_new_rule() {
        let gate = probability();
        let before = gate.digest();
        let mut widened = gate.clone();
        if let Rule::Probability(rule) = &mut widened.rule {
            rule.max_brier_increase.value = Some(0.20);
        }
        assert_ne!(
            before,
            widened.digest(),
            "a widened tolerance is a different rule"
        );

        // Relabelling where a number came from is also a change to the rule.
        let mut relabelled = gate.clone();
        if let Rule::Probability(rule) = &mut relabelled.rule {
            rule.max_brier_increase.basis = Basis::Derived;
        }
        assert_ne!(before, relabelled.digest());
    }

    #[test]
    fn the_verdict_precedence_is_failed_then_unverifiable_then_passed() {
        assert!(Verdict::Failed > Verdict::Unverifiable);
        assert!(Verdict::Unverifiable > Verdict::Passed);
        assert_eq!(
            Verdict::over([Verdict::Passed, Verdict::Unverifiable, Verdict::Failed]),
            Verdict::Failed
        );
        assert_eq!(
            Verdict::over([Verdict::Passed, Verdict::Unverifiable]),
            Verdict::Unverifiable
        );
        assert_eq!(
            Verdict::over([Verdict::Passed, Verdict::Passed]),
            Verdict::Passed
        );
        assert_eq!(
            Verdict::over([]),
            Verdict::Unverifiable,
            "a gate that judged nothing has not passed anything"
        );
    }

    #[test]
    fn one_failed_group_beats_any_number_of_passed_ones() {
        let gate = probability();
        let clean = Scores {
            items: 40,
            accuracy: Some(0.80),
            ece: Some(0.200),
            brier: Some(0.150),
            nll: Some(0.600),
            confident_errors: Some(2),
        };
        let better = Scores {
            ece: Some(0.020),
            nll: Some(0.400),
            ..clean
        };
        let worse = Scores {
            ece: Some(0.020),
            nll: Some(0.900),
            ..clean
        };

        let mut comparisons: Vec<Comparison> = (0..7)
            .map(|index| Comparison::new(format!("family-{index}"), clean, better).fitted_on(40))
            .collect();
        let all_passed = gate.judge_all(&comparisons);
        assert_eq!(all_passed.verdict, Verdict::Passed);

        comparisons.push(Comparison::new("hedged", clean, worse).fitted_on(40));
        let report = gate.judge_all(&comparisons);
        assert_eq!(report.verdict, Verdict::Failed);
        assert_eq!(report.breaches().count(), 1);
        assert_eq!(report.groups.len(), 8);
    }

    #[test]
    fn the_adapter_is_the_better_decision_and_the_worse_probability() {
        // The measured situation on 2026-09-19: +13 points of accuracy, and
        // both measures that punish confident wrongness got worse. One gate
        // cannot say that; two can.
        let comparison = Comparison::new("support-v2", base_calibrated(), adapted_calibrated());

        let decided = decision().judge(&comparison);
        assert_eq!(decided.verdict, Verdict::Passed, "{:?}", decided.criteria);

        let probabilities = probability().judge(&comparison);
        assert_eq!(probabilities.verdict, Verdict::Failed);
        let deciding = probabilities
            .deciding()
            .expect("a failed group has a deciding criterion");
        assert_eq!(
            deciding.rank, 1,
            "log loss and confident errors decide this gate"
        );
        let failed: Vec<&str> = probabilities
            .breaches()
            .filter(|criterion| criterion.verdict == Verdict::Failed)
            .map(|criterion| criterion.name.as_str())
            .collect();
        assert!(failed.contains(&"log_loss_does_not_rise"), "{failed:?}");
        assert!(
            failed.contains(&"confident_errors_do_not_rise"),
            "{failed:?}"
        );
        assert!(
            !failed.contains(&"accuracy_does_not_fall"),
            "accuracy rose, and it does not rescue the gate either"
        );
    }

    #[test]
    fn thirteen_points_of_accuracy_clears_the_noise_and_says_what_it_covers() {
        let comparison = Comparison::new("support-v2", base_calibrated(), adapted_calibrated());
        let outcome = decision().judge(&comparison);
        let gain = outcome
            .criteria
            .iter()
            .find(|criterion| criterion.name == "accuracy_gain_clears_the_noise")
            .expect("the gain criterion is emitted");
        assert_eq!(gain.verdict, Verdict::Passed);
        // Two unpaired standard errors of 0.77 and 0.90 over 98 items each
        // is 0.104, so the win clears the bound by under three points.
        assert!(gain.detail.contains("0.104"), "{}", gain.detail);
        assert!(
            gain.detail.contains("item sampling only"),
            "the verdict carries what the bound does not cover: {}",
            gain.detail
        );
    }

    #[test]
    fn a_smaller_win_on_the_same_suite_is_unverifiable_rather_than_failed() {
        let baseline = base_calibrated();
        let modest = Scores {
            accuracy: Some(0.82),
            ..adapted_calibrated()
        };
        let outcome = decision().judge(&Comparison::new("support-v2", baseline, modest));
        assert_eq!(
            outcome.verdict,
            Verdict::Unverifiable,
            "five points over 98 items is inside the noise, which is not a loss"
        );
    }

    #[test]
    fn a_door_that_is_less_accurate_fails_the_decision_gate() {
        let comparison = Comparison::new("support-v2", adapted_calibrated(), base_calibrated());
        let outcome = decision().judge(&comparison);
        assert_eq!(outcome.verdict, Verdict::Failed);
        assert_eq!(
            outcome.deciding().map(|criterion| criterion.name.as_str()),
            Some("accuracy_does_not_fall")
        );
    }

    #[test]
    fn a_map_fitted_on_eighteen_items_is_unverifiable_not_failed() {
        // `severity` on the base door: ECE rose, which under the old gate
        // read as a refusal. Eighteen items cannot fill two bins of fifteen,
        // so the honest answer is that nobody can tell.
        let raw = Scores {
            items: 18,
            accuracy: Some(0.67),
            ece: Some(0.097),
            brier: Some(0.172),
            nll: Some(0.487),
            confident_errors: Some(1),
        };
        let mapped = Scores {
            ece: Some(0.126),
            brier: Some(0.216),
            nll: Some(0.641),
            ..raw
        };
        let outcome = probability().judge(&Comparison::new("severity", raw, mapped).fitted_on(18));
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        assert!(
            outcome
                .criteria
                .iter()
                .all(|criterion| criterion.verdict != Verdict::Failed),
            "nothing downstream of a missing floor is judged: {:?}",
            outcome.criteria
        );
        assert!(
            outcome
                .deciding()
                .is_some_and(|criterion| criterion.name == "fitted_on>=30")
        );
    }

    #[test]
    fn a_map_whose_brier_rose_is_failed() {
        // `urgency` on the base door: ECE 0.129 to 0.025 and Brier 0.173 to
        // 0.196, which is a 13% move against a 10% tolerance. We could tell,
        // and it lost.
        let raw = Scores {
            items: 30,
            accuracy: Some(0.73),
            ece: Some(0.129),
            brier: Some(0.173),
            nll: Some(1.319),
            confident_errors: Some(2),
        };
        let mapped = Scores {
            ece: Some(0.025),
            brier: Some(0.196),
            nll: Some(0.582),
            ..raw
        };
        let outcome = probability().judge(&Comparison::new("urgency", raw, mapped).fitted_on(30));
        assert_eq!(outcome.verdict, Verdict::Failed);
        assert_eq!(
            outcome.deciding().map(|criterion| criterion.name.as_str()),
            Some("brier_stays_within_tolerance")
        );
    }

    #[test]
    fn the_case_that_widened_the_brier_tolerance() {
        // Measured on the urgency family of the 196-item suite. A gate with
        // no tolerance refused a map that cut ECE from 0.157 to 0.005 over a
        // Brier move of 5.5%, and that refusal is what widened the constant.
        // The reasoning survives here rather than in a commit message.
        let raw = Scores {
            items: 30,
            accuracy: Some(0.73),
            ece: Some(0.157),
            brier: Some(0.199),
            nll: Some(0.600),
            confident_errors: Some(3),
        };
        let mapped = Scores {
            ece: Some(0.005),
            brier: Some(0.210),
            nll: Some(0.580),
            confident_errors: Some(2),
            ..raw
        };
        let comparison = Comparison::new("urgency", raw, mapped).fitted_on(30);
        assert_eq!(verdict_of(&probability(), &comparison), Verdict::Passed);

        let mut zero_tolerance = probability();
        if let Rule::Probability(rule) = &mut zero_tolerance.rule {
            rule.max_brier_increase.value = Some(0.0);
        }
        assert_eq!(
            verdict_of(&zero_tolerance, &comparison),
            Verdict::Failed,
            "the rule that refused this case is still expressible, and it has its own digest"
        );
        assert_ne!(probability().digest(), zero_tolerance.digest());
    }

    #[test]
    fn the_same_case_with_confident_errors_unrecorded_is_unverifiable() {
        // Unknown is never zero. The gate that admitted this map did so
        // while counting no confident errors on either side, because the
        // metric defaulted to zero rather than to absent.
        let raw = Scores {
            items: 30,
            accuracy: Some(0.73),
            ece: Some(0.157),
            brier: Some(0.199),
            nll: Some(0.600),
            confident_errors: None,
        };
        let mapped = Scores {
            ece: Some(0.005),
            brier: Some(0.210),
            nll: Some(0.580),
            ..raw
        };
        let outcome = probability().judge(&Comparison::new("urgency", raw, mapped).fitted_on(30));
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        assert!(
            outcome
                .deciding()
                .is_some_and(|criterion| criterion.name == "confident_errors_do_not_rise")
        );
    }

    #[test]
    fn the_twelve_item_maps_the_small_suite_produced_are_unverifiable() {
        // The 52-item suite: routing went from a raw ECE of 0.031 to 0.113
        // when mapped. The machinery was right and the evidence was missing,
        // which is a different sentence from "the map lost".
        let raw = Scores {
            items: 12,
            accuracy: Some(0.75),
            ece: Some(0.031),
            brier: Some(0.012),
            nll: Some(0.100),
            confident_errors: Some(0),
        };
        let mapped = Scores {
            ece: Some(0.113),
            brier: Some(0.013),
            ..raw
        };
        let outcome = probability().judge(&Comparison::new("routing", raw, mapped).fitted_on(12));
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
    }

    #[test]
    fn a_map_that_hedges_everything_into_the_middle_fails() {
        let raw = Scores {
            items: 30,
            accuracy: Some(0.80),
            ece: Some(0.200),
            brier: Some(0.150),
            nll: Some(0.400),
            confident_errors: Some(2),
        };
        let hedged = Scores {
            ece: Some(0.010),
            brier: Some(0.155),
            nll: Some(0.900),
            ..raw
        };
        let outcome = probability().judge(&Comparison::new("hedged", raw, hedged).fitted_on(30));
        assert_eq!(outcome.verdict, Verdict::Failed);
        assert_eq!(
            outcome.deciding().map(|criterion| criterion.name.as_str()),
            Some("log_loss_does_not_rise")
        );
    }

    #[test]
    fn a_map_that_improves_ece_too_little_is_unverifiable() {
        // `routing` on the adapted door: 0.075 to 0.070, a 7% reduction
        // against a 10% margin. The map did not lose; it did not clearly
        // win either.
        let raw = Scores {
            items: 50,
            accuracy: Some(0.92),
            ece: Some(0.075),
            brier: Some(0.077),
            nll: Some(1.713),
            confident_errors: Some(4),
        };
        let mapped = Scores {
            ece: Some(0.070),
            brier: Some(0.079),
            nll: Some(0.378),
            ..raw
        };
        let outcome = probability().judge(&Comparison::new("routing", raw, mapped).fitted_on(50));
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        assert!(
            outcome
                .deciding()
                .is_some_and(|criterion| criterion.name == "ece_reduction_clears_the_margin")
        );
    }

    #[test]
    fn an_unmeasured_bound_reports_unverifiable_and_never_passes() {
        let source = r#"{
            "schema": "openagents.gym.gate.v2",
            "id": "unmeasured-v1",
            "question": "What happens when nobody has measured the bound?",
            "rule": {
                "decides": "decision",
                "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
                "gain_standard_errors": {
                    "value": null,
                    "basis": "unmeasured",
                    "why": "openagents#9370 measures the spread this bound needs"
                },
                "variance_basis": "item_sampling",
                "pending_measurement": {
                    "quantity": "the suite's trial-to-trial resampling variance",
                    "issue": "openagents#9370",
                    "why": "the standard error covers item sampling only"
                }
            }
        }"#;
        let gate = Gate::from_json(source, Path::new("unmeasured-v1.json"))
            .expect("an unmeasured bound is a valid rule");
        let outcome = gate.judge(&Comparison::new(
            "support-v2",
            base_calibrated(),
            adapted_calibrated(),
        ));
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        assert_eq!(
            gate.rule
                .pending_measurement()
                .and_then(|pending| pending.issue.as_deref()),
            Some("openagents#9370")
        );
    }

    #[test]
    fn the_decision_bound_records_what_it_does_not_cover() {
        // The gate cannot honestly carry an absolute effect-size floor until
        // openagents#9370 measures the suite's trial-to-trial spread. That
        // gap is on the gate and inside the digest, so filling it produces
        // decision-v2.
        let gate = decision();
        let Rule::Decision(rule) = &gate.rule else {
            panic!("decision-v1 carries a decision rule");
        };
        assert_eq!(rule.variance_basis, VarianceBasis::ItemSampling);
        assert_eq!(rule.gain_standard_errors.basis, Basis::Convention);
        let pending = rule
            .pending_measurement
            .as_ref()
            .expect("the gap is recorded");
        assert_eq!(pending.issue.as_deref(), Some("openagents#9370"));

        let mut measured = gate.clone();
        if let Rule::Decision(rule) = &mut measured.rule {
            rule.variance_basis = VarianceBasis::ItemSamplingAndTrialResampling;
            rule.pending_measurement = None;
        }
        assert_ne!(gate.digest(), measured.digest());
    }

    #[test]
    fn every_committed_bound_says_where_it_came_from() {
        for gate in load_all().expect("the committed gates load") {
            let bounds: Vec<&Bound> = match &gate.rule {
                Rule::Decision(rule) => vec![&rule.min_items, &rule.gain_standard_errors],
                Rule::Probability(rule) => {
                    let mut bounds = vec![
                        &rule.min_items,
                        &rule.min_ece_reduction,
                        &rule.max_brier_increase,
                    ];
                    if let Some(floor) = rule.confident_error_floor.as_deref() {
                        bounds.extend([&floor.block_sigma, &floor.block_items, &floor.sigmas]);
                    }
                    bounds
                }
                Rule::Deployment(rule) => vec![
                    &rule.min_calls,
                    &rule.latency_block_sigma_relative,
                    &rule.regression_sigmas,
                ],
            };
            for bound in bounds {
                assert!(
                    bound.why.len() > 40,
                    "{} carries a thin provenance",
                    gate.id
                );
                if bound.basis == Basis::Tuned {
                    assert!(
                        bound.why.contains("2026-09-19") || bound.why.contains("calibrate.rs"),
                        "a tuned bound names the case that tuned it: {}",
                        bound.why
                    );
                }
            }
        }
    }

    #[test]
    fn a_gate_with_an_unreadable_schema_is_refused() {
        let source = r#"{
            "schema": "openagents.gym.gate.v3",
            "id": "future-v1",
            "question": "Does a newer schema load?",
            "rule": {
                "decides": "decision",
                "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
                "gain_standard_errors": {
                    "value": 2.0, "basis": "convention", "why": "a multiple"
                },
                "variance_basis": "item_sampling",
                "pending_measurement": null
            }
        }"#;
        let error = Gate::from_json(source, Path::new("future-v1.json")).unwrap_err();
        assert!(matches!(error, GateError::Schema { .. }), "{error}");
    }

    #[test]
    fn a_bound_with_a_value_and_no_basis_behind_it_is_refused() {
        let source = r#"{
            "schema": "openagents.gym.gate.v2",
            "id": "wrong-v1",
            "question": "Does an unmeasured bound get to carry a number?",
            "rule": {
                "decides": "decision",
                "min_items": { "value": 10, "basis": "unmeasured", "why": "nothing measured it" },
                "gain_standard_errors": {
                    "value": 2.0, "basis": "convention", "why": "a multiple"
                },
                "variance_basis": "item_sampling",
                "pending_measurement": null
            }
        }"#;
        let error = Gate::from_json(source, Path::new("wrong-v1.json")).unwrap_err();
        assert!(matches!(error, GateError::Invalid { .. }), "{error}");
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let source = r#"{
            "schema": "openagents.gym.gate.v2",
            "id": "typo-v1",
            "question": "Does a misspelled threshold pass unnoticed?",
            "rule": {
                "decides": "decision",
                "min_items": { "value": 10, "basis": "derived", "why": "a floor" },
                "gain_standard_errors": {
                    "value": 2.0, "basis": "convention", "why": "a multiple"
                },
                "variance_basis": "item_sampling",
                "pending_measurement": null,
                "max_brier_incraese": 0.1
            }
        }"#;
        let error = Gate::from_json(source, Path::new("typo-v1.json")).unwrap_err();
        assert!(matches!(error, GateError::Parse { .. }), "{error}");
    }

    #[test]
    fn a_gate_whose_file_name_is_not_its_id_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("renamed.json");
        let source = serde_json::to_string(&decision()).expect("the gate serializes");
        std::fs::write(&path, source).expect("the file is written");
        let error = Gate::load(&path).unwrap_err();
        assert!(matches!(error, GateError::Invalid { .. }), "{error}");
    }

    fn deployment() -> Gate {
        load("deployment-v1").expect("deployment-v1 loads")
    }

    /// The committed rule with a latency floor in it.
    ///
    /// The value is a stand-in and not a measurement, which is why it lives
    /// in a test rather than in `crates/gym/gates/`. It is here to show that
    /// the criterion decides once somebody measures the floor, without
    /// putting a number nobody measured inside a digest.
    fn deployment_with_a_floor(sigma: f64) -> Gate {
        let mut gate = deployment();
        if let Rule::Deployment(rule) = &mut gate.rule {
            rule.latency_block_sigma_relative = Bound {
                value: Some(sigma),
                basis: Basis::Convention,
                evidence: Vec::new(),
                why: "A stand-in, used only by this test to show that a measured floor makes \
                      the latency criteria decidable."
                    .into(),
            };
            rule.pending_measurement = None;
        }
        gate
    }

    /// A door that answers in about 180 ms, profiled over a full pass.
    fn quick_door() -> Profile {
        Profile {
            calls: 98,
            latency_p50_ms: Some(180.0),
            latency_p95_ms: Some(240.0),
            cost: Some(Cost::UnmeteredLocalLane),
            refusals: Some(0),
        }
    }

    /// A door that draws eight samples across four helpers for every answer.
    fn ensemble_door() -> Profile {
        Profile {
            calls: 98,
            latency_p50_ms: Some(1_600.0),
            latency_p95_ms: Some(1_900.0),
            cost: Some(Cost::UnmeteredLocalLane),
            refusals: Some(0),
        }
    }

    /// A router in front of every agent turn. The ceilings are the caller's,
    /// stated here because this test is the caller.
    fn router_budget() -> Budget {
        Budget::new(
            "router",
            "stated by this test, which is the caller; the gate carries no ceiling of its own",
        )
        .latency_ms(300.0)
        .cost_usd(0.0001)
        .refusal_rate(0.02)
    }

    /// An overnight batch job over the same items.
    fn batch_budget() -> Budget {
        Budget::new(
            "nightly batch",
            "stated by this test, for a workload nobody waits on",
        )
        .latency_ms(5_000.0)
        .cost_usd(0.01)
        .refusal_rate(0.10)
    }

    fn criterion_named<'a>(outcome: &'a Outcome, name: &str) -> &'a Criterion {
        outcome
            .criteria
            .iter()
            .find(|criterion| criterion.name == name)
            .unwrap_or_else(|| panic!("{name} is emitted: {:?}", outcome.criteria))
    }

    #[test]
    fn a_gate_judges_only_the_measurement_it_reads() {
        let comparison = Comparison::new("support-v2", base_calibrated(), adapted_calibrated());
        let profiles = Deployment::new("support-v2", ensemble_door(), quick_door());

        let wrong_way = deployment().judge(&comparison);
        assert_eq!(wrong_way.verdict, Verdict::Unverifiable);
        assert_eq!(
            wrong_way
                .deciding()
                .map(|criterion| criterion.name.as_str()),
            Some("measurement_matches_the_rule")
        );

        let other_way = decision().judge_deployment(&profiles);
        assert_eq!(other_way.verdict, Verdict::Unverifiable);
        assert_eq!(
            other_way
                .deciding()
                .map(|criterion| criterion.name.as_str()),
            Some("measurement_matches_the_rule")
        );
    }

    #[test]
    fn a_ceiling_nobody_stated_is_unverifiable_rather_than_passed() {
        // The whole argument for taking the budget from the caller: a door
        // at 240 ms is fast for one workload and too slow for another, and
        // the gate that does not know which one is being deployed has not
        // learned anything by looking at 240.
        let outcome = deployment().judge_deployment(&Deployment::new(
            "support-v2",
            ensemble_door(),
            quick_door(),
        ));
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        for name in [
            "latency_p95_within_budget",
            "cost_per_decision_within_budget",
            "refusal_rate_within_budget",
        ] {
            let criterion = criterion_named(&outcome, name);
            assert_eq!(criterion.verdict, Verdict::Unverifiable, "{name}");
            assert!(
                criterion.detail.contains("no budget was stated"),
                "{}",
                criterion.detail
            );
        }
    }

    #[test]
    fn an_unmetered_lane_is_not_a_price_of_zero() {
        // A lane with no meter on it cannot clear a price ceiling, however
        // low the ceiling is. Recording it as zero would clear every one.
        let unmetered =
            Deployment::new("support-v2", ensemble_door(), quick_door()).under(router_budget());
        let outcome = deployment().judge_deployment(&unmetered);
        let cost = criterion_named(&outcome, "cost_per_decision_within_budget");
        assert_eq!(cost.verdict, Verdict::Unverifiable);
        assert!(
            cost.detail.contains("unmetered_local_lane"),
            "{}",
            cost.detail
        );
        assert!(
            cost.detail.contains("not a price of zero"),
            "{}",
            cost.detail
        );

        let as_a_zero = Deployment::new(
            "support-v2",
            ensemble_door(),
            quick_door().costing(Cost::Metered {
                usd_per_decision: 0.0,
            }),
        )
        .under(router_budget());
        let pretended = deployment().judge_deployment(&as_a_zero);
        assert_eq!(
            criterion_named(&pretended, "cost_per_decision_within_budget").verdict,
            Verdict::Passed,
            "a zero in the cost column passes, which is exactly why an unmetered lane must not \
             be written as one"
        );
    }

    #[test]
    fn a_metered_lane_over_its_ceiling_fails() {
        // The hosted lane at the rate openagents#9382 records for it,
        // $1.82 per 100,000 decisions, against two stated ceilings.
        let hosted = quick_door().costing(Cost::Metered {
            usd_per_decision: 0.0000182,
        });
        let under_router =
            Deployment::new("support-v2", ensemble_door(), hosted).under(router_budget());
        assert_eq!(
            criterion_named(
                &deployment().judge_deployment(&under_router),
                "cost_per_decision_within_budget"
            )
            .verdict,
            Verdict::Passed
        );

        let tight = Budget::new("free tier", "stated by this test").cost_usd(0.000001);
        let under_tight = Deployment::new("support-v2", ensemble_door(), hosted).under(tight);
        let outcome = deployment().judge_deployment(&under_tight);
        assert_eq!(outcome.verdict, Verdict::Failed);
        assert_eq!(
            outcome.deciding().map(|criterion| criterion.name.as_str()),
            Some("cost_per_decision_within_budget")
        );
    }

    #[test]
    fn the_router_and_the_batch_job_disagree_about_the_same_door() {
        // One door, one measurement, two workloads, two verdicts. This is
        // the reason the ceiling is not in the rule: a constant that refused
        // this door for the router would refuse it for the batch job too,
        // and a constant that admitted it for the batch job would admit it
        // in front of every agent turn.
        let gate = deployment_with_a_floor(0.05);
        let profiles = Deployment::new("support-v2", quick_door(), ensemble_door());

        let router = gate.judge_deployment(&profiles.clone().under(router_budget()));
        assert_eq!(router.verdict, Verdict::Failed);
        assert_eq!(
            criterion_named(&router, "latency_p95_within_budget").verdict,
            Verdict::Failed
        );

        let batch = gate.judge_deployment(&profiles.under(batch_budget()));
        assert_eq!(
            criterion_named(&batch, "latency_p95_within_budget").verdict,
            Verdict::Passed,
            "1.9 seconds is inside a batch job's budget and outside a router's"
        );
    }

    #[test]
    fn a_latency_rise_inside_the_noise_is_unverifiable_rather_than_failed() {
        let gate = deployment_with_a_floor(0.05);
        let baseline = quick_door();
        // Twelve milliseconds on a 240 ms p95, against a band of two 5%
        // standard deviations of both sides together.
        let nudged = Profile {
            latency_p95_ms: Some(252.0),
            ..baseline
        };
        let inside = gate.judge_deployment(&Deployment::new("support-v2", baseline, nudged));
        let criterion = criterion_named(&inside, "latency_p95_does_not_rise");
        assert_eq!(criterion.verdict, Verdict::Unverifiable);
        assert!(
            criterion.detail.contains("on one door on one machine"),
            "{}",
            criterion.detail
        );

        let doubled = Profile {
            latency_p95_ms: Some(520.0),
            ..baseline
        };
        let outside = gate.judge_deployment(&Deployment::new("support-v2", baseline, doubled));
        assert_eq!(
            criterion_named(&outside, "latency_p95_does_not_rise").verdict,
            Verdict::Failed
        );
    }

    #[test]
    fn a_door_that_declines_a_tenth_of_the_workload_fails_its_ceiling() {
        // The refusal criteria do not wait on the latency floor: metering
        // and counting are not clocks.
        let declining = Profile {
            refusals: Some(10),
            ..quick_door()
        };
        let outcome = deployment().judge_deployment(
            &Deployment::new("support-v2", quick_door(), declining).under(router_budget()),
        );
        assert_eq!(outcome.verdict, Verdict::Failed);
        let ceiling = criterion_named(&outcome, "refusal_rate_within_budget");
        assert_eq!(ceiling.verdict, Verdict::Failed);
        assert!(
            ceiling.detail.contains("10 of 98 calls declined"),
            "{}",
            ceiling.detail
        );
    }

    #[test]
    fn an_uncounted_refusal_is_not_a_refusal_of_zero() {
        let unknown = Profile {
            refusals: None,
            ..quick_door()
        };
        let outcome = deployment().judge_deployment(
            &Deployment::new("support-v2", quick_door(), unknown).under(router_budget()),
        );
        assert_eq!(
            criterion_named(&outcome, "refusal_rate_within_budget").verdict,
            Verdict::Unverifiable
        );
        assert_eq!(
            criterion_named(&outcome, "refusal_rate_does_not_rise").verdict,
            Verdict::Unverifiable
        );
    }

    #[test]
    fn fewer_than_twenty_timed_calls_cannot_produce_a_percentile() {
        // Nineteen calls put the 95th percentile on the slowest one, which
        // is a maximum wearing a percentile's name.
        let thin = Profile {
            calls: 19,
            ..quick_door()
        };
        let outcome = deployment().judge_deployment(
            &Deployment::new("support-v2", quick_door(), thin).under(router_budget()),
        );
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        assert!(
            outcome
                .deciding()
                .is_some_and(|criterion| criterion.name == "timed_calls>=20")
        );
        assert!(
            outcome
                .criteria
                .iter()
                .all(|criterion| criterion.verdict != Verdict::Failed),
            "nothing downstream of a missing floor is judged: {:?}",
            outcome.criteria
        );
    }

    #[test]
    fn the_committed_gate_cannot_refuse_a_door_on_a_clock_yet() {
        // The machine that would have measured the floor was running nine
        // agents against several copies of the same on-device model. The gap
        // is on the gate and inside the digest, so filling it produces
        // deployment-v2 rather than rewriting deployment-v1's history.
        let gate = deployment();
        let Rule::Deployment(rule) = &gate.rule else {
            panic!("deployment-v1 carries a deployment rule");
        };
        assert_eq!(rule.gated_percentile, GatedPercentile::P95);
        assert_eq!(rule.latency_block_sigma_relative.basis, Basis::Unmeasured);
        let pending = rule
            .pending_measurement
            .as_ref()
            .expect("the gap is recorded");
        assert!(
            pending.quantity.contains("uncontended"),
            "{}",
            pending.quantity
        );

        // A door seven times over a router's ceiling still cannot be refused
        // on latency, because nobody knows how wide the band is.
        let outcome = gate.judge_deployment(
            &Deployment::new("support-v2", quick_door(), ensemble_door()).under(router_budget()),
        );
        let latency = criterion_named(&outcome, "latency_p95_within_budget");
        assert_eq!(latency.verdict, Verdict::Unverifiable);
        assert!(latency.detail.contains("contended"), "{}", latency.detail);

        assert_ne!(
            gate.digest(),
            deployment_with_a_floor(0.05).digest(),
            "filling the floor is a different rule"
        );
    }

    #[test]
    fn the_four_way_table_kept_the_statistic_that_hides_the_tail() {
        // docs/lev/disposition.md ranks four doors on accuracy, ECE, Brier,
        // log loss, and confident errors, and records a median latency for
        // each. Not one of them has a p95 on record, so the criterion that
        // decides a router's verdict cannot be read off that table at all.
        // docs/gym/measurements/2026-09-19-deployment-ranking.md works it
        // through.
        let median_only = |p50: f64| Profile {
            calls: 26,
            latency_p50_ms: Some(p50),
            latency_p95_ms: None,
            cost: Some(Cost::UnmeteredLocalLane),
            refusals: Some(0),
        };
        let outcome = deployment().judge_deployment(
            &Deployment::new("support-v1", median_only(180.0), median_only(2_100.0))
                .under(router_budget()),
        );
        assert_eq!(outcome.verdict, Verdict::Unverifiable);
        assert!(
            outcome
                .deciding()
                .is_some_and(|criterion| criterion.name == "timed_calls>=20")
                || outcome
                    .criteria
                    .iter()
                    .any(|criterion| criterion.name == "latency_p95_does_not_rise"
                        && criterion.detail.contains("not measured on both sides")),
            "{:?}",
            outcome.criteria
        );
    }

    #[test]
    fn a_percentile_is_always_a_call_that_happened() {
        let calls: Vec<f64> = (1..=100).map(f64::from).collect();
        let profile = Profile::timed(&calls);
        assert_eq!(profile.calls, 100);
        assert_eq!(profile.latency_p50_ms, Some(50.0));
        assert_eq!(profile.latency_p95_ms, Some(95.0));

        // Nearest rank never interpolates, so every number reported is a
        // call somebody waited through.
        let odd = Profile::timed(&[10.0, 20.0, 30.0]);
        assert_eq!(odd.latency_p50_ms, Some(20.0));
        assert_eq!(odd.latency_p95_ms, Some(30.0));

        assert_eq!(Profile::timed(&[]).latency_p50_ms, None);
        assert_eq!(Profile::timed(&[]).calls, 0);
    }

    #[test]
    fn binning_cannot_lose_more_brier_than_twice_the_bin_width() {
        // The derivation in this module's documentation, checked rather than
        // asserted. It holds, and the ceiling it gives is far too loose to
        // serve as an acceptance tolerance, which is why max_brier_increase
        // is recorded as tuned.
        let mut raw: Vec<(f64, bool)> = Vec::new();
        for index in 0..240_usize {
            let signal = (index % 40) as f64 / 40.0 + 0.0125;
            // Correlate the outcome with the signal, and put the awkward
            // items at the bin edges where binning loses the most.
            let correct = (index * 7 + index / 40) % 5 != 0;
            raw.push((signal, correct));
        }
        for bins in [2_usize, 3, 5, 10] {
            let width = 1.0 / bins as f64;
            let brier = |pairs: &[(f64, bool)]| -> f64 {
                pairs
                    .iter()
                    .map(|(p, y)| (p - if *y { 1.0 } else { 0.0 }).powi(2))
                    .sum::<f64>()
                    / pairs.len() as f64
            };
            let mut fitted = vec![0.0_f64; bins];
            for (index, value) in fitted.iter_mut().enumerate() {
                let lo = index as f64 / bins as f64;
                let hi = (index + 1) as f64 / bins as f64;
                let inside: Vec<(f64, bool)> = raw
                    .iter()
                    .copied()
                    .filter(|(p, _)| *p >= lo && (*p < hi || index + 1 == bins))
                    .collect();
                *value = if inside.is_empty() {
                    0.0
                } else {
                    inside.iter().filter(|(_, y)| *y).count() as f64 / inside.len() as f64
                };
            }
            let binned: Vec<(f64, bool)> = raw
                .iter()
                .map(|(p, y)| {
                    let index = ((p * bins as f64) as usize).min(bins - 1);
                    (fitted[index], *y)
                })
                .collect();
            let accuracy = raw.iter().filter(|(_, y)| *y).count() as f64 / raw.len() as f64;
            let loss = brier(&binned) - brier(&raw);
            let ceiling = 2.0 * width * accuracy;
            assert!(
                loss <= ceiling,
                "{bins} bins lost {loss:.4} against {ceiling:.4}"
            );
        }
    }
}
