//! One result, for one item, on one door, in one run.
//!
//! A System One call is one request and one response. There is no
//! trajectory to record, so this row is the whole trace, and nothing here
//! should grow toward one.
//!
//! The row exists because three faults in one week were record-keeping
//! faults, and each one has a field here:
//!
//! - A door that refused was indistinguishable from an item that vanished,
//!   so a door whose guardrails fire on the hard questions scored better for
//!   refusing to judge them. [`Row::answered`] and [`Row::refusal`] keep the
//!   refusal in the record, with its typed code, and leave
//!   [`Row::correct`] unknown rather than wrong. A failure of the harness is
//!   never one of these: it produces no row at all.
//! - A calibration record could not say which model it was fitted against.
//!   [`DoorIdentity`] carries the model, the base model signature, and the
//!   adapter package, with a `verified` flag that is false for a hosted
//!   closed model rather than a signature invented to fill the field.
//! - A gate and a suite were replaced in one commit, leaving their effects
//!   inseparable. Every row pins [`Row::suite_digest`] and
//!   [`Row::gate_digest`], so changing either produces new rules rather than
//!   new history. [`Row::question_digest`] is the third of those, and it is
//!   here for the opposite reason: without it, rewording a question was a
//!   change to the suite, so a text variant could not be compared against
//!   the items it left alone. See [`crate::questions`].
//!
//! Every unknown number is `null`. None of them is ever `0`, because zero is
//! a measurement and `null` is the absence of one, and a run that confuses
//! the two reports an average over items it never measured.
//!
//! This module depends on no door and on no estimator. It describes what a
//! run produced; it does not know how to produce one.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The schema tag every row carries.
///
/// It is written into the row rather than inferred from the file, so a line
/// that escapes its store still says what it is and what reads it.
pub const SCHEMA: &str = "openagents.gym.eval_row.v1";

/// What a door was running when it answered.
///
/// A measurement that cannot name the thing it measured is not evidence. The
/// committed calibration maps proved that: they carried only the operating
/// system build, which is identical for every door on one machine, and so
/// they sat on disk through two adapter changes that altered which question
/// families are admitted at all.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoorIdentity {
    /// The model id the door reports.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// The base model signature, where the runtime exposes one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base_model_signature: String,
    /// The adapter package identifier, when a door serves one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub adapter: String,
    /// Whether any of the above can actually be checked.
    ///
    /// False for a hosted closed model. That is the `unknown is not zero`
    /// rule applied to identity: do not synthesise a digest for something
    /// that does not publish one.
    pub verified: bool,
}

impl DoorIdentity {
    /// Names a door that publishes nothing to check.
    ///
    /// A hosted closed model reports a name and no more. The name is worth
    /// recording, and the empty signature is worth leaving empty: a reader
    /// who sees `verified: false` knows the identity is a label, not a
    /// proof, and will not compare it against a serving runtime.
    #[must_use]
    pub fn hosted(model: impl Into<String>) -> Self {
        Self { model: model.into(), verified: false, ..Self::default() }
    }

    /// Names a door whose runtime publishes what it is running.
    ///
    /// The signature is what a calibration map has to match before it may
    /// serve, so a door that publishes one is verifiable and a door that
    /// publishes an empty one is not, whatever else it reports.
    #[must_use]
    pub fn published(
        model: impl Into<String>,
        base_model_signature: impl Into<String>,
        adapter: impl Into<String>,
    ) -> Self {
        let base_model_signature = base_model_signature.into();
        Self {
            model: model.into(),
            verified: !base_model_signature.is_empty(),
            base_model_signature,
            adapter: adapter.into(),
        }
    }
}

/// Why a door declined to answer.
///
/// Each code is a property of the door, and the row keeps it, including a
/// code that names the door's own internal failure: the door answered, and
/// the answer was that it could not. What never appears here is a failure of
/// the harness — a reset connection, a timeout in the client, a body that
/// does not parse — because that is neither the door's fault nor its credit,
/// and it leaves the record set entirely.
///
/// The named codes are the ones the System One contract publishes.
/// [`RefusalCode::Other`] keeps any other code verbatim. A door is free to
/// refuse for a reason the Gym has never seen, and collapsing it into a
/// generic bucket would lose the only description of it that exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum RefusalCode {
    /// The request fails contract validation, before any call.
    InvalidRequest,
    /// A Choice names more options than the contract admits.
    TooManyOptions,
    /// The runtime is off, ineligible, or still preparing.
    ModelUnavailable,
    /// No fitted calibration map covers this question family.
    Uncalibrated,
    /// The state and the question exceed the runtime's context window.
    BranchTooLong,
    /// A guardrail blocked generation, or the model refused.
    Guardrail,
    /// The runtime rejected the compiled schema.
    UnsupportedGuide,
    /// The runtime could not decode its own constrained output.
    DecodingFailure,
    /// The attached adapter does not match the running base.
    AdapterIncompatible,
    /// The runtime rate limited or hit a concurrency limit.
    Busy,
    /// A code this crate does not name, kept as the door sent it.
    Other(String),
}

impl RefusalCode {
    /// The wire label a door publishes.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::TooManyOptions => "too_many_options",
            Self::ModelUnavailable => "model_unavailable",
            Self::Uncalibrated => "uncalibrated",
            Self::BranchTooLong => "branch_too_long",
            Self::Guardrail => "guardrail",
            Self::UnsupportedGuide => "unsupported_guide",
            Self::DecodingFailure => "decoding_failure",
            Self::AdapterIncompatible => "adapter_incompatible",
            Self::Busy => "busy",
            Self::Other(code) => code,
        }
    }
}

impl From<String> for RefusalCode {
    fn from(code: String) -> Self {
        match code.as_str() {
            "invalid_request" => Self::InvalidRequest,
            "too_many_options" => Self::TooManyOptions,
            "model_unavailable" => Self::ModelUnavailable,
            "uncalibrated" => Self::Uncalibrated,
            "branch_too_long" => Self::BranchTooLong,
            "guardrail" => Self::Guardrail,
            "unsupported_guide" => Self::UnsupportedGuide,
            "decoding_failure" => Self::DecodingFailure,
            "adapter_incompatible" => Self::AdapterIncompatible,
            "busy" => Self::Busy,
            _ => Self::Other(code),
        }
    }
}

impl From<RefusalCode> for String {
    fn from(code: RefusalCode) -> Self {
        match code {
            RefusalCode::Other(code) => code,
            named => named.label().to_string(),
        }
    }
}

/// What kind of evidence the label this row was scored against rests on.
///
/// `support-v2` is entirely `author`: the label is written by the same
/// person who reads the results, and there is no independent verifier. That
/// is a real limit on every number the Gym prints, and it travels with each
/// row so it cannot be left behind in a paragraph of prose.
///
/// `coder-turns-v1` is the first suite that is not all one thing. Its states
/// are turns from recorded sessions, and for some of its questions the
/// answer is in the session: whether the agent asked a clarifying question,
/// whether it opened the repository, whether anything was undone afterwards.
/// Those are [`LabelSource::Outcome`]. The questions the record cannot
/// settle stay [`LabelSource::Author`].
///
/// **The two are different evidence and a row says which it carries.** An
/// accuracy figure over both pooled is a number whose meaning changes with
/// the mix, and nothing downstream can recover the mix once it is gone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum LabelSource {
    /// The person who reads the results wrote the label.
    #[default]
    Author,
    /// The label is what happened next in the session the state was taken
    /// from, read by a rule the suite states and applies mechanically.
    Outcome,
    /// A source this crate does not name, kept as it was written.
    Other(String),
}

impl LabelSource {
    /// The wire label.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Author => "author",
            Self::Outcome => "outcome",
            Self::Other(source) => source,
        }
    }
}

impl From<String> for LabelSource {
    fn from(source: String) -> Self {
        match source.as_str() {
            "author" => Self::Author,
            "outcome" => Self::Outcome,
            _ => Self::Other(source),
        }
    }
}

impl From<LabelSource> for String {
    fn from(source: LabelSource) -> Self {
        match source {
            LabelSource::Other(source) => source,
            named => named.label().to_string(),
        }
    }
}

/// A row that contradicts itself.
///
/// The store rejects one rather than appending it, because a receipt chain
/// over incoherent rows is a tamper-proof record of nonsense.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RowError {
    /// The row is tagged as some other document.
    #[error("the row is tagged {found}, not {SCHEMA}")]
    UnknownSchema {
        /// The tag the row carried.
        found: String,
    },
    /// The row claims the door both answered and refused.
    #[error("the row is answered and carries the refusal {code}")]
    AnsweredAndRefused {
        /// The refusal the answered row carried.
        code: String,
    },
    /// The row claims neither an answer nor a refusal.
    #[error("the row is neither answered nor refused, so it records nothing")]
    NoOutcome,
    /// A refused row carries a score.
    ///
    /// A refusal has nothing to score. A number here would enter an average
    /// as though the door had answered, which is the fault this crate exists
    /// to stop.
    #[error("the row is refused and carries a score in {field}")]
    RefusalCarriesScore {
        /// The field that holds the score it should not have.
        field: &'static str,
    },
    /// An answered row does not say whether the answer was right.
    ///
    /// The item is in the denominator, so leaving it out of the numerator
    /// divides a partial numerator by a full denominator.
    #[error("the row is answered but does not say whether the answer was correct")]
    AnsweredWithoutVerdict,
}

/// One result, for one item, on one door, in one run.
///
/// Build one with [`Row::new`], then close it with [`Row::scored`] or
/// [`Row::refused`]. Those two set the outcome fields together, so a row
/// cannot claim an answer and a refusal at once or carry a score it did not
/// earn. [`Row::check`] catches a row assembled by hand or read from disk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// What this document is. Always [`SCHEMA`] for a row this crate wrote.
    pub schema: String,
    /// When the run recorded the row, as an RFC 3339 timestamp in UTC.
    ///
    /// The time the result was written, not the time it was judged. A gate
    /// runs later and changes nothing here.
    pub recorded_at: String,
    /// The suite the item came from.
    pub suite: String,
    /// The suite's content digest.
    ///
    /// The name is stable while the content is not. Two runs of `support-v2`
    /// are comparable only if this matches, and pinning it is what stops a
    /// suite edit from reading as a model improvement.
    pub suite_digest: String,
    /// The question set the door was served, by the id of a file in
    /// `crates/gym/questions/`.
    ///
    /// `null` on a row written before openagents#9386, and on a run of a
    /// suite whose items carry their question text inline: there the suite
    /// digest already covers the text, because the text is in the items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_set: Option<String>,
    /// That set's content digest.
    ///
    /// The third digest a run pins, beside the suite's and the gate's. The
    /// suite says what was asked about, this says how it was asked, and the
    /// gate says what bar judged it. Two runs that share a suite digest and
    /// differ here are a question-text comparison over unchanged items,
    /// which is the comparison openagents#9386 exists to make expressible.
    ///
    /// `null` is unknown and never the authored text.
    /// [`crate::store::admit_comparison`] refuses to compare a run that
    /// records this against one that does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_digest: Option<String>,
    /// Which partition of the suite the item sits in.
    ///
    /// A number from the calibration split and a number from the locked set
    /// mean different things, and a reader who cannot tell them apart will
    /// quote the one that was fitted on.
    pub split: String,
    /// The question family, which is the unit calibration is fitted over.
    pub family: String,
    /// The item, unique within the suite.
    pub item_id: String,
    /// The door that was asked, by the name the run used for it.
    pub door: String,
    /// What that door was running, as far as it can be verified.
    #[serde(default)]
    pub door_identity: DoorIdentity,
    /// Which estimator produced the raw signal.
    ///
    /// A greedy read and an ensemble of draws answer the same contract and
    /// produce differently distributed numbers, so a row that does not name
    /// the estimator cannot be pooled with another.
    pub estimator: String,
    /// How many draws the estimate rests on.
    ///
    /// Unknown is `null`. Zero would say the door was never asked, and one
    /// would be a guess about a door that does not report this.
    #[serde(default)]
    pub samples: Option<u64>,
    /// The seed the estimator started from.
    ///
    /// This is a perturbation axis, and it exists because the doors here are
    /// near-deterministic: repeating a run reproduces it exactly, so there
    /// is no trial-to-trial variance to average over and a fresh trial has
    /// to be manufactured. A door that takes no seed records `null` rather
    /// than `0`, which would read as a seed that was chosen.
    #[serde(default)]
    pub seed_base: Option<u64>,
    /// The option order the item was served in, as indices into the suite's
    /// own order.
    ///
    /// The second perturbation axis. A model that answers differently when
    /// the options are reordered is reading the list rather than the state,
    /// and that gap shows up in neither accuracy nor calibration. `null`
    /// means the suite's own order was served, which the harness always
    /// knows; it is not an unknown.
    #[serde(default)]
    pub permutation: Option<Vec<usize>>,
    /// Whether the door answered.
    ///
    /// Stated as its own field, rather than left implicit in `refusal`,
    /// because the row is read by things that never load this type.
    /// [`Row::check`] enforces that the two agree.
    pub answered: bool,
    /// Why the door declined, when it declined.
    ///
    /// A refusal is a result. The item stays in the denominator and out of
    /// the numerator, which is the only arrangement under which a door
    /// cannot improve its score by refusing the questions it finds hard.
    #[serde(default)]
    pub refusal: Option<RefusalCode>,
    /// The top probability the door reported, before any calibration map.
    ///
    /// Raw, so that fitting a map later does not require rerunning the door,
    /// and so a map's effect stays separable from the model's.
    #[serde(default)]
    pub raw_top: Option<f64>,
    /// The full distribution over options, in the order they were served.
    ///
    /// The whole distribution rather than the argmax, because a wrong answer
    /// at 0.51 and a wrong answer at 0.99 are different failures and only
    /// one of them is a calibration problem.
    #[serde(default)]
    pub distribution: Option<IndexMap<String, f64>>,
    /// Whether the answer matched the label.
    ///
    /// `null` on a refusal. A refused item is not a wrong answer, and
    /// counting it as one would make a refusing door look inaccurate rather
    /// than absent — the opposite of the original fault, and no more honest.
    #[serde(default)]
    pub correct: Option<bool>,
    /// How long the call took, in milliseconds.
    ///
    /// Unknown is `null`. Zero is a latency, and a run that records unknown
    /// latency as zero reports a mean that no call ever achieved.
    #[serde(default)]
    pub latency_ms: Option<f64>,
    /// Who wrote the label this row was scored against.
    pub label_source: LabelSource,
    /// The acceptance rule that judged this row.
    ///
    /// `null` means no rule has judged it, which is a fact about the row
    /// rather than an unnamed rule.
    #[serde(default)]
    pub gate_id: Option<String>,
    /// That rule's content digest.
    ///
    /// Pinned so that editing a gate produces a new gate. A rule that can be
    /// changed without changing its digest rewrites the meaning of every
    /// verdict already recorded under it.
    #[serde(default)]
    pub gate_digest: Option<String>,
    /// The receipt of the row before this one in the store.
    ///
    /// `null` for the first row in a file. The store computes both this and
    /// [`Row::receipt`]; they are declared here because they are part of
    /// what a row is.
    #[serde(default)]
    pub previous_receipt: Option<String>,
    /// This row's receipt.
    ///
    /// The chain is what stops a worse result from being quietly removed and
    /// a better one from being quietly inserted.
    #[serde(default)]
    pub receipt: Option<String>,
}

impl Default for Row {
    fn default() -> Self {
        Self {
            schema: SCHEMA.to_string(),
            recorded_at: String::new(),
            suite: String::new(),
            suite_digest: String::new(),
            question_set: None,
            question_digest: None,
            split: String::new(),
            family: String::new(),
            item_id: String::new(),
            door: String::new(),
            door_identity: DoorIdentity::default(),
            estimator: String::new(),
            samples: None,
            seed_base: None,
            permutation: None,
            answered: false,
            refusal: None,
            raw_top: None,
            distribution: None,
            correct: None,
            latency_ms: None,
            label_source: LabelSource::Author,
            gate_id: None,
            gate_digest: None,
            previous_receipt: None,
            receipt: None,
        }
    }
}

impl Row {
    /// Starts a row for one item on one door, with no outcome yet.
    ///
    /// Fill the rest by assignment, then close the row with [`Row::scored`]
    /// or [`Row::refused`]. A row with no outcome records nothing, and
    /// [`Row::check`] says so.
    #[must_use]
    pub fn new(
        suite: impl Into<String>,
        suite_digest: impl Into<String>,
        item_id: impl Into<String>,
        door: impl Into<String>,
    ) -> Self {
        Self {
            suite: suite.into(),
            suite_digest: suite_digest.into(),
            item_id: item_id.into(),
            door: door.into(),
            ..Self::default()
        }
    }

    /// Records what the door answered.
    ///
    /// `raw_top` is the largest probability in the distribution. An empty
    /// distribution leaves it `null`: a door that returned no options
    /// reported no confidence, and `0.0` would be a confidence it never
    /// expressed.
    #[must_use]
    pub fn scored(mut self, distribution: IndexMap<String, f64>, correct: bool) -> Self {
        self.raw_top = distribution.values().copied().reduce(f64::max);
        self.distribution = Some(distribution);
        self.answered = true;
        self.refusal = None;
        self.correct = Some(correct);
        self
    }

    /// Records that the door declined to answer.
    ///
    /// The score fields are cleared rather than left as they were, so a
    /// refusal can never carry a number that an average would pick up.
    #[must_use]
    pub fn refused(mut self, refusal: RefusalCode) -> Self {
        self.answered = false;
        self.refusal = Some(refusal);
        self.raw_top = None;
        self.distribution = None;
        self.correct = None;
        self
    }

    /// Whether the door answered this item.
    #[must_use]
    pub fn is_scored(&self) -> bool {
        self.answered && self.refusal.is_none()
    }

    /// Whether the door declined this item.
    #[must_use]
    pub fn is_refused(&self) -> bool {
        !self.answered && self.refusal.is_some()
    }

    /// Reports whether the row can be believed.
    ///
    /// The checks are all about one boundary: a row says either what the
    /// door answered or why it would not, never both and never neither, and
    /// a refusal carries no score.
    ///
    /// # Errors
    ///
    /// Returns the first contradiction found.
    pub fn check(&self) -> Result<(), RowError> {
        if self.schema != SCHEMA {
            return Err(RowError::UnknownSchema { found: self.schema.clone() });
        }
        match (self.answered, &self.refusal) {
            (true, Some(code)) => {
                return Err(RowError::AnsweredAndRefused { code: code.label().to_string() });
            }
            (false, None) => return Err(RowError::NoOutcome),
            _ => {}
        }
        if self.is_refused() {
            if self.raw_top.is_some() {
                return Err(RowError::RefusalCarriesScore { field: "raw_top" });
            }
            if self.distribution.is_some() {
                return Err(RowError::RefusalCarriesScore { field: "distribution" });
            }
            if self.correct.is_some() {
                return Err(RowError::RefusalCarriesScore { field: "correct" });
            }
        } else if self.correct.is_none() {
            return Err(RowError::AnsweredWithoutVerdict);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA_PREFIX;

    /// The field names in the order the schema fixes them.
    const FIELDS: [&str; 26] = [
        "schema",
        "recorded_at",
        "suite",
        "suite_digest",
        "question_set",
        "question_digest",
        "split",
        "family",
        "item_id",
        "door",
        "door_identity",
        "estimator",
        "samples",
        "seed_base",
        "permutation",
        "answered",
        "refusal",
        "raw_top",
        "distribution",
        "correct",
        "latency_ms",
        "label_source",
        "gate_id",
        "gate_digest",
        "previous_receipt",
        "receipt",
    ];

    fn distribution(pairs: &[(&str, f64)]) -> IndexMap<String, f64> {
        pairs.iter().map(|(key, value)| ((*key).to_string(), *value)).collect()
    }

    fn scored_row() -> Row {
        let mut row = Row::new("support-v2", "sha256:abc", "item-7", "lev");
        row.recorded_at = "2026-09-19T12:00:00Z".to_string();
        row.split = "evaluation".to_string();
        row.family = "sentiment".to_string();
        row.estimator = "l2".to_string();
        row.door_identity = DoorIdentity::published("lev", "sig:base-1", "band-v1");
        row.question_set = Some("support-v2-three-way-v1".to_string());
        row.question_digest = Some("sha256:questions".to_string());
        row.scored(distribution(&[("no", 0.25), ("yes", 0.75)]), true)
    }

    #[test]
    fn the_schema_tag_belongs_to_this_crate() {
        assert_eq!(SCHEMA, "openagents.gym.eval_row.v1");
        assert!(SCHEMA.starts_with(SCHEMA_PREFIX), "the row is tagged into the Gym family");
    }

    #[test]
    fn a_row_serializes_to_the_named_fields_in_order() {
        let rendered = serde_json::to_string(&scored_row()).expect("a row serializes");
        let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("it is JSON");
        let object = parsed.as_object().expect("a row is an object");
        let keys: Vec<&str> = object.keys().map(String::as_str).collect();
        assert_eq!(keys, FIELDS, "every field is present, in the order the schema fixes");
    }

    #[test]
    fn a_row_round_trips_through_json() {
        let row = scored_row();
        let rendered = serde_json::to_string(&row).expect("a row serializes");
        let read: Row = serde_json::from_str(&rendered).expect("a row parses");
        assert_eq!(read, row);
    }

    #[test]
    fn an_unknown_number_renders_as_null_and_never_as_zero() {
        let mut row = scored_row();
        row.latency_ms = None;
        row.samples = None;
        row.seed_base = None;
        let rendered = serde_json::to_string(&row).expect("a row serializes");

        assert!(rendered.contains("\"latency_ms\":null"), "unknown latency reads as unknown");
        assert!(
            !rendered.contains("\"latency_ms\":0"),
            "unknown latency never reads as a measured zero: {rendered}"
        );
        assert!(!rendered.contains("\"samples\":0"), "unknown sample count is not zero draws");
        assert!(!rendered.contains("\"seed_base\":0"), "an absent seed is not seed zero");
    }

    #[test]
    fn a_measured_zero_is_kept_apart_from_an_unknown() {
        let mut row = scored_row();
        row.latency_ms = Some(0.0);
        let rendered = serde_json::to_string(&row).expect("a row serializes");
        assert!(rendered.contains("\"latency_ms\":0"), "a measured zero is a measurement");

        let read: Row = serde_json::from_str(&rendered).expect("a row parses");
        assert_eq!(read.latency_ms, Some(0.0));
        assert_ne!(read.latency_ms, None, "zero and unknown stay distinguishable");
    }

    #[test]
    fn an_empty_distribution_leaves_the_top_probability_unknown() {
        let row = Row::new("support-v2", "sha256:abc", "item-7", "lev")
            .scored(IndexMap::new(), false);
        assert_eq!(row.raw_top, None, "no options reported means no confidence reported");
    }

    #[test]
    fn a_score_takes_the_top_probability_from_the_distribution() {
        let row = scored_row();
        assert_eq!(row.raw_top, Some(0.75));
    }

    #[test]
    fn a_hosted_identity_round_trips_without_inventing_a_signature() {
        let hosted = DoorIdentity::hosted("jev-latest");
        assert!(!hosted.verified, "a hosted closed model publishes nothing to check");
        assert!(hosted.base_model_signature.is_empty());
        assert!(hosted.adapter.is_empty());

        let rendered = serde_json::to_string(&hosted).expect("an identity serializes");
        assert!(
            !rendered.contains("base_model_signature"),
            "an absent signature stays absent: {rendered}"
        );
        assert!(rendered.contains("\"verified\":false"), "the row says it cannot be checked");

        let read: DoorIdentity = serde_json::from_str(&rendered).expect("an identity parses");
        assert_eq!(read, hosted);
    }

    #[test]
    fn a_row_for_a_hosted_door_carries_the_unverified_identity() {
        let mut row = scored_row();
        row.door = "jev".to_string();
        row.door_identity = DoorIdentity::hosted("jev-latest");
        let rendered = serde_json::to_string(&row).expect("a row serializes");
        let read: Row = serde_json::from_str(&rendered).expect("a row parses");

        assert_eq!(read.door_identity.model, "jev-latest");
        assert!(!read.door_identity.verified);
        assert!(read.door_identity.base_model_signature.is_empty());
    }

    #[test]
    fn an_identity_without_a_signature_is_not_verified() {
        let identity = DoorIdentity::published("lev", "", "band-v1");
        assert!(!identity.verified, "an empty signature is nothing to match against");
    }

    #[test]
    fn an_on_device_identity_round_trips_with_its_signature() {
        let identity = DoorIdentity::published("lev", "sig:base-1", "band-v1");
        assert!(identity.verified);
        let rendered = serde_json::to_string(&identity).expect("an identity serializes");
        let read: DoorIdentity = serde_json::from_str(&rendered).expect("an identity parses");
        assert_eq!(read, identity);
        assert_eq!(read.base_model_signature, "sig:base-1");
        assert_eq!(read.adapter, "band-v1");
    }

    #[test]
    fn a_refused_row_is_distinguishable_from_a_scored_one() {
        let refused = Row::new("support-v2", "sha256:abc", "item-7", "lev")
            .refused(RefusalCode::Guardrail);
        let scored = scored_row();

        assert!(refused.is_refused() && !refused.is_scored());
        assert!(scored.is_scored() && !scored.is_refused());
        assert_eq!(refused.refusal, Some(RefusalCode::Guardrail));
        assert_eq!(scored.refusal, None);

        // The distinction that matters: a refusal is not a wrong answer, and
        // it is not a missing item either.
        assert_eq!(refused.correct, None, "a refused item is not scored wrong");
        assert_eq!(scored.correct, Some(true));

        let rendered = serde_json::to_string(&refused).expect("a row serializes");
        assert!(rendered.contains("\"refusal\":\"guardrail\""), "{rendered}");
        assert!(rendered.contains("\"correct\":null"), "{rendered}");
        assert!(rendered.contains("\"answered\":false"), "{rendered}");

        let read: Row = serde_json::from_str(&rendered).expect("a row parses");
        assert_eq!(read, refused);
        assert_ne!(read, scored);
    }

    #[test]
    fn a_refusal_clears_a_score_that_was_already_set() {
        let row = scored_row().refused(RefusalCode::BranchTooLong);
        assert_eq!(row.raw_top, None);
        assert_eq!(row.distribution, None);
        assert_eq!(row.correct, None);
        row.check().expect("the row is coherent");
    }

    #[test]
    fn every_named_refusal_code_round_trips_on_its_wire_label() {
        let codes = [
            RefusalCode::InvalidRequest,
            RefusalCode::TooManyOptions,
            RefusalCode::ModelUnavailable,
            RefusalCode::Uncalibrated,
            RefusalCode::BranchTooLong,
            RefusalCode::Guardrail,
            RefusalCode::UnsupportedGuide,
            RefusalCode::DecodingFailure,
            RefusalCode::AdapterIncompatible,
            RefusalCode::Busy,
        ];
        for code in codes {
            let rendered = serde_json::to_string(&code).expect("a code serializes");
            assert_eq!(rendered, format!("\"{}\"", code.label()));
            let read: RefusalCode = serde_json::from_str(&rendered).expect("a code parses");
            assert_eq!(read, code);
        }
    }

    #[test]
    fn an_unnamed_refusal_code_is_kept_rather_than_bucketed() {
        let read: RefusalCode =
            serde_json::from_str("\"policy_withheld\"").expect("a code parses");
        assert_eq!(read, RefusalCode::Other("policy_withheld".to_string()));
        assert_eq!(read.label(), "policy_withheld");
        let rendered = serde_json::to_string(&read).expect("a code serializes");
        assert_eq!(rendered, "\"policy_withheld\"", "the door's own word survives the trip");
    }

    #[test]
    fn the_label_source_travels_with_the_row() {
        let row = scored_row();
        assert_eq!(row.label_source, LabelSource::Author);
        let rendered = serde_json::to_string(&row).expect("a row serializes");
        assert!(rendered.contains("\"label_source\":\"author\""), "{rendered}");

        let read: LabelSource =
            serde_json::from_str("\"independent_verifier\"").expect("a source parses");
        assert_eq!(read, LabelSource::Other("independent_verifier".to_string()));
        assert_ne!(read, LabelSource::Author, "an unknown source never reads as the author");
    }

    #[test]
    fn the_perturbation_axes_record_what_was_varied() {
        let mut row = scored_row();
        row.seed_base = Some(41);
        row.permutation = Some(vec![2, 0, 1]);
        let rendered = serde_json::to_string(&row).expect("a row serializes");
        let read: Row = serde_json::from_str(&rendered).expect("a row parses");
        assert_eq!(read.seed_base, Some(41));
        assert_eq!(read.permutation, Some(vec![2, 0, 1]));

        let unpermuted = scored_row();
        assert_eq!(unpermuted.permutation, None, "the suite's own order reads as no permutation");
    }

    #[test]
    fn the_gate_and_the_chain_are_declared_but_not_computed_here() {
        let row = scored_row();
        assert_eq!(row.gate_id, None, "no rule has judged a row this module built");
        assert_eq!(row.gate_digest, None);
        assert_eq!(row.previous_receipt, None, "the store owns the chain");
        assert_eq!(row.receipt, None);

        let rendered = serde_json::to_string(&row).expect("a row serializes");
        for field in ["gate_id", "gate_digest", "previous_receipt", "receipt"] {
            assert!(rendered.contains(&format!("\"{field}\":null")), "{field} is null: {rendered}");
        }
    }

    #[test]
    fn a_coherent_row_passes_its_check() {
        scored_row().check().expect("a scored row is coherent");
        Row::new("support-v2", "sha256:abc", "item-7", "lev")
            .refused(RefusalCode::Uncalibrated)
            .check()
            .expect("a refused row is coherent");
    }

    #[test]
    fn a_row_that_answered_and_refused_is_rejected() {
        let mut row = scored_row();
        row.refusal = Some(RefusalCode::Guardrail);
        assert_eq!(
            row.check(),
            Err(RowError::AnsweredAndRefused { code: "guardrail".to_string() })
        );
    }

    #[test]
    fn a_row_with_no_outcome_is_rejected() {
        let row = Row::new("support-v2", "sha256:abc", "item-7", "lev");
        assert_eq!(row.check(), Err(RowError::NoOutcome), "a row must record something");
    }

    #[test]
    fn a_refusal_that_carries_a_score_is_rejected() {
        let mut row = Row::new("support-v2", "sha256:abc", "item-7", "lev")
            .refused(RefusalCode::Guardrail);
        row.raw_top = Some(0.9);
        assert_eq!(row.check(), Err(RowError::RefusalCarriesScore { field: "raw_top" }));

        let mut row = Row::new("support-v2", "sha256:abc", "item-7", "lev")
            .refused(RefusalCode::Guardrail);
        row.correct = Some(false);
        assert_eq!(
            row.check(),
            Err(RowError::RefusalCarriesScore { field: "correct" }),
            "a refusal counted as a wrong answer is still a fabricated number"
        );
    }

    #[test]
    fn an_answered_row_without_a_verdict_is_rejected() {
        let mut row = scored_row();
        row.correct = None;
        assert_eq!(
            row.check(),
            Err(RowError::AnsweredWithoutVerdict),
            "an item in the denominator has to be in the numerator too"
        );
    }

    #[test]
    fn a_row_tagged_as_another_document_is_rejected() {
        let mut row = scored_row();
        row.schema = "openagents.bench_result.v3".to_string();
        assert_eq!(
            row.check(),
            Err(RowError::UnknownSchema { found: "openagents.bench_result.v3".to_string() })
        );
    }

    #[test]
    fn a_row_that_pins_no_question_set_omits_both_fields() {
        // Every row written before openagents#9386 is this shape, and the
        // receipt chain over them has to keep verifying, so an absent
        // question set writes nothing rather than a null.
        let mut row = scored_row();
        row.question_set = None;
        row.question_digest = None;
        let rendered = serde_json::to_string(&row).expect("a row serializes");
        assert!(!rendered.contains("question_set"), "{rendered}");
        assert!(!rendered.contains("question_digest"), "{rendered}");
        let read: Row = serde_json::from_str(&rendered).expect("a row parses");
        assert_eq!(read, row);
    }

    #[test]
    fn a_row_must_name_its_item_and_may_omit_what_is_unknown() {
        let minimal = serde_json::json!({
            "schema": SCHEMA,
            "recorded_at": "2026-09-19T12:00:00Z",
            "suite": "support-v2",
            "suite_digest": "sha256:abc",
            "split": "evaluation",
            "family": "sentiment",
            "item_id": "item-7",
            "door": "lev",
            "estimator": "l2",
            "answered": false,
            "refusal": "guardrail",
            "label_source": "author",
        });
        let read: Row = serde_json::from_value(minimal).expect("an abbreviated row parses");
        assert_eq!(read.latency_ms, None, "an omitted number reads as unknown");
        assert_eq!(read.receipt, None);
        assert_eq!(read.door_identity, DoorIdentity::default());
        read.check().expect("the row is coherent");

        let nameless = serde_json::json!({
            "schema": SCHEMA,
            "recorded_at": "2026-09-19T12:00:00Z",
            "suite": "support-v2",
            "suite_digest": "sha256:abc",
            "split": "evaluation",
            "family": "sentiment",
            "door": "lev",
            "estimator": "l2",
            "answered": false,
            "refusal": "guardrail",
            "label_source": "author",
        });
        serde_json::from_value::<Row>(nameless)
            .expect_err("a row that cannot name its item is not a record");
    }
}
