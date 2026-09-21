//! The classification envelope: the versioned request contract for the
//! `POST /v1/classify` facade, and the deterministic plan a valid
//! envelope produces.
//!
//! This module is the first slice of the facade described in
//! `docs/decision-models/decision-api.md`: it defines what a caller may
//! send and checks it completely, before any route, handler, or backend
//! exists. Nothing here runs inference, picks a threshold, guesses a
//! backend's capacity, or reports a successful outcome — a [`Plan`]
//! records every judgment the request would need, each marked
//! [`Outcome::Unattempted`].
//!
//! The product maxima are the schema design targets the contract
//! publishes: 1–1,000 inputs, up to 100 labels per label set, up to 20
//! named dimensions, and at most 1,000 item-dimension or input-label
//! judgments per synchronous call. A backend may bind tighter limits; a
//! caller passes them in as [`BackendLimits`], and a declaration above a
//! product maximum is refused rather than advertised.

use std::collections::HashSet;

use receipts::execution::Outcome;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The schema tag a classification request carries.
pub const SCHEMA: &str = "openagents.classify.v1";

/// The schema tag a decision policy identity carries.
pub const POLICY_SCHEMA: &str = "openagents.classify-policy.v1";

/// The most inputs one request may carry.
pub const MAX_INPUTS: u64 = 1_000;

/// The most labels one label set may carry, whether the request's own
/// or a dimension's.
pub const MAX_LABELS: u64 = 100;

/// The most named dimensions one request may carry.
pub const MAX_DIMENSIONS: u64 = 20;

/// The most judgments one call may plan: item-dimension decisions for
/// single-label work, input-label decisions for multi-label work.
pub const MAX_JUDGMENTS: u64 = 1_000;

/// The most characters an input, label, or dimension id may carry.
pub const MAX_ID_CHARS: u64 = 256;

/// The most bytes one input's text or serialized record may carry.
pub const MAX_INPUT_BYTES: u64 = 262_144;

/// The most bytes a request's or a dimension's instructions may carry.
pub const MAX_INSTRUCTIONS_BYTES: u64 = 16_384;

/// The most bytes a label's description may carry.
pub const MAX_LABEL_BYTES: u64 = 4_096;

/// The label minimum a single-label set observes: a categorical choice
/// needs at least two options to be a judgment.
const MIN_SINGLE_LABELS: u64 = 2;

/// How the labels on a set are judged.
///
/// The two modes answer differently, and a plan keeps them distinct: a
/// single-label judgment is one categorical distribution that compares
/// the labels, while a multi-label judgment is one independent
/// probability per label — those probabilities do not sum to one.
/// Binary filtering is multi-label work with a single label.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// One Choice per input: the labels form one distribution.
    SingleLabel,
    /// One Noul per input per label: each label is judged on its own.
    MultiLabel,
}

/// One label: a stable id and an optional description the judgment can
/// read as criteria.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    /// The label's caller-chosen id, unique within its set.
    pub id: String,
    /// What the label means, when the id alone does not say it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One input: a stable id and either text or a JSON record. Exactly one
/// of `text` and `record` is present in a valid request.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// The input's caller-chosen id, unique within the request. Results
    /// key on it, so it is never generated or rewritten here.
    pub id: String,
    /// The input as text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The input as a JSON record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<Map<String, Value>>,
}

/// A named dimension: its own mode, label set, and optional
/// instructions, judged independently of the request's other
/// dimensions.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    /// The dimension's caller-chosen id, unique within the request.
    pub id: String,
    /// How this dimension's labels are judged.
    pub mode: Mode,
    /// The dimension's label set.
    pub labels: Vec<Label>,
    /// Instructions scoped to this dimension, layered over the
    /// request's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// How a tie at the top of a categorical distribution resolves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SingleTies {
    /// The earlier label in the declared set wins.
    FirstDeclared,
    /// A tie selects the no-match outcome rather than guessing.
    NoMatch,
}

/// How a tie at a `top_n` boundary resolves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoundaryTies {
    /// Every label tied at the boundary is selected, past the cap.
    IncludeAll,
    /// The cap holds exactly; tied labels keep their declared order.
    Truncate,
}

/// What a selection reports when nothing matches.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NoMatch {
    /// The unit's `selected` is null.
    Null,
    /// The unit's `selected` is a designated label in the set — the
    /// caller's declared "none of the above".
    Label {
        /// The label id, which must appear in the label set the rule
        /// applies to.
        label: String,
    },
}

/// What an empty multi-label selection reports.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmptySelection {
    /// The unit's `selected` is an empty list.
    Empty,
    /// The unit's `selected` is null.
    Null,
}

/// The selection rule for a single-label — categorical — unit.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SingleSelect {
    /// How a tie at the top of the distribution resolves.
    pub ties: SingleTies,
    /// An explicit abstention cut: when the distribution's top
    /// probability is below it, the unit selects the no-match outcome.
    /// Absent means the argmax is always reported — the contract has
    /// no implicit confidence threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_probability: Option<f64>,
    /// What an unmatched unit selects.
    pub no_match: NoMatch,
}

/// The selection rule for a multi-label unit, where each label's
/// probability stands alone.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiSelect {
    /// The probability a label must reach to be selected. Required —
    /// the caller declares the cut its own data supports; the contract
    /// does not invent one.
    pub threshold: f64,
    /// A cap on the number of selected labels, when the caller
    /// declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u64>,
    /// How a tie at the `top_n` boundary resolves.
    pub ties: BoundaryTies,
    /// What an input with no selected label reports.
    pub no_match: EmptySelection,
}

/// The selection rules a policy declares, one entry per mode.
///
/// Each entry is optional on the document and required by the work:
/// a request that plans a mode the policy does not declare is refused
/// rather than assigned a rule it never saw.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Select {
    /// The rule for single-label units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_label: Option<SingleSelect>,
    /// The rule for multi-label units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_label: Option<MultiSelect>,
}

/// The versioned policy identity the call runs under: its name and
/// the explicit selection rules it declares — threshold, top-N, tie
/// handling, and no-match behavior, none of them defaulted.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The policy schema tag.
    pub v: String,
    /// The policy's name.
    pub name: String,
    /// The selection rules, per mode.
    pub select: Select,
}

/// The limits the selected backend publishes, checked against the
/// product maxima.
///
/// Every field is an upper bound the backend declares for itself. A
/// field must be present in a deserialized limits document. Missing
/// declarations cannot establish support for the product maximum.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackendLimits {
    /// The most inputs the backend takes in one call.
    pub max_inputs: u64,
    /// The most labels the backend takes in one label set.
    pub max_labels: u64,
    /// The most dimensions the backend takes in one call.
    pub max_dimensions: u64,
    /// The most judgments the backend takes in one call.
    pub max_judgments: u64,
    /// The most characters the backend takes in an id.
    pub max_id_chars: u64,
    /// The most bytes the backend takes in one input.
    pub max_input_bytes: u64,
    /// The most bytes the backend takes in an instructions field.
    pub max_instructions_bytes: u64,
    /// The most bytes the backend takes in a label description.
    pub max_label_bytes: u64,
}

impl BackendLimits {
    /// The documented product maxima — the loosest limits the facade
    /// itself accepts.
    #[must_use]
    pub const fn product() -> Self {
        Self {
            max_inputs: MAX_INPUTS,
            max_labels: MAX_LABELS,
            max_dimensions: MAX_DIMENSIONS,
            max_judgments: MAX_JUDGMENTS,
            max_id_chars: MAX_ID_CHARS,
            max_input_bytes: MAX_INPUT_BYTES,
            max_instructions_bytes: MAX_INSTRUCTIONS_BYTES,
            max_label_bytes: MAX_LABEL_BYTES,
        }
    }

    /// Refuse a limits document a backend cannot mean: a bound of zero
    /// admits no work at all, and a bound above the product maximum is
    /// a limit the facade does not serve.
    fn check(&self) -> Result<(), Refusal> {
        let product = Self::product();
        let fields = [
            ("max_inputs", self.max_inputs, product.max_inputs),
            ("max_labels", self.max_labels, product.max_labels),
            (
                "max_dimensions",
                self.max_dimensions,
                product.max_dimensions,
            ),
            ("max_judgments", self.max_judgments, product.max_judgments),
            ("max_id_chars", self.max_id_chars, product.max_id_chars),
            (
                "max_input_bytes",
                self.max_input_bytes,
                product.max_input_bytes,
            ),
            (
                "max_instructions_bytes",
                self.max_instructions_bytes,
                product.max_instructions_bytes,
            ),
            (
                "max_label_bytes",
                self.max_label_bytes,
                product.max_label_bytes,
            ),
        ];
        for (name, declared, maximum) in fields {
            if declared == 0 {
                return Err(Refusal::UnsupportedLimits(format!(
                    "the backend's `{name}` is zero — it admits no {name} at all"
                )));
            }
            if declared > maximum {
                return Err(Refusal::UnsupportedLimits(format!(
                    "the backend's `{name}` of {declared} exceeds the facade's {maximum}"
                )));
            }
        }
        Ok(())
    }
}

impl Policy {
    /// The checks a policy's own values pass before any unit is
    /// validated against them: a declared cut is a probability and a
    /// declared cap admits at least one label.
    fn check(&self) -> Result<(), Refusal> {
        if let Some(rule) = &self.select.single_label {
            if let Some(cut) = rule.min_probability {
                check_probability("min_probability", cut)?;
            }
        }
        if let Some(rule) = &self.select.multi_label {
            check_probability("threshold", rule.threshold)?;
            if rule.top_n == Some(0) {
                return Err(Refusal::InvalidRequest(
                    "the policy's `top_n` of 0 selects no labels".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl SingleSelect {
    /// Resolve a categorical unit's selected output: the label the
    /// distribution's top probability names, or the no-match outcome
    /// when a declared tie rule or abstention cut says so.
    ///
    /// `probabilities` pairs each declared label with its answered
    /// probability, in declared order — the order `first-declared`
    /// tie-breaking reads.
    pub fn select(&self, probabilities: &[(String, f64)]) -> Value {
        let top = probabilities
            .iter()
            .map(|(_, probability)| *probability)
            .fold(f64::NEG_INFINITY, f64::max);
        let tied = probabilities
            .iter()
            .filter(|(_, probability)| *probability == top)
            .count();
        let abstained = self.min_probability.is_some_and(|cut| top < cut);
        let tied_out = tied > 1 && self.ties == SingleTies::NoMatch;
        if abstained || tied_out {
            return self.unmatched();
        }
        probabilities
            .iter()
            .find(|(_, probability)| *probability == top)
            .map_or_else(|| self.unmatched(), |(label, _)| {
                Value::String(label.clone())
            })
    }

    /// What the unit selects when nothing matched.
    fn unmatched(&self) -> Value {
        match &self.no_match {
            NoMatch::Null => Value::Null,
            NoMatch::Label { label } => Value::String(label.clone()),
        }
    }
}

impl MultiSelect {
    /// Resolve a multi-label unit's selected labels: every label at or
    /// above the declared threshold, best first, then capped and
    /// tie-broken as the policy declares.
    ///
    /// `probabilities` pairs each declared label with its answered
    /// probability, in declared order — the order equal probabilities
    /// keep.
    pub fn select(&self, probabilities: &[(String, f64)]) -> Value {
        let mut passed: Vec<&(String, f64)> = probabilities
            .iter()
            .filter(|(_, probability)| *probability >= self.threshold)
            .collect();
        // The sort is stable, so equal probabilities keep their
        // declared order.
        passed.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut selected: Vec<String> = passed.iter().map(|(label, _)| label.clone()).collect();
        if let Some(cap) = self.top_n.map(|top_n| top_n as usize) {
            if selected.len() > cap {
                match self.ties {
                    BoundaryTies::Truncate => selected.truncate(cap),
                    BoundaryTies::IncludeAll => {
                        let boundary = passed[cap - 1].1;
                        selected = passed
                            .iter()
                            .take_while(|(_, probability)| *probability >= boundary)
                            .map(|(label, _)| label.clone())
                            .collect();
                    }
                }
            }
        }
        if selected.is_empty() {
            return match self.no_match {
                EmptySelection::Empty => Value::Array(Vec::new()),
                EmptySelection::Null => Value::Null,
            };
        }
        Value::Array(selected.into_iter().map(Value::String).collect())
    }
}

/// A declared cut must be a probability — finite and from 0 to 1.
fn check_probability(field: &'static str, value: f64) -> Result<(), Refusal> {
    if !(0.0..=1.0).contains(&value) {
        return Err(Refusal::InvalidRequest(format!(
            "the policy's `{field}` of {value} is not a probability"
        )));
    }
    Ok(())
}

/// The parsed request envelope.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// The schema tag — `openagents.classify.v1`.
    pub v: String,
    /// The door the call names, as in `POST /v1/systemone`.
    pub model: String,
    /// The capacity selection the call runs under.
    pub capacity: String,
    /// The decision policy identity.
    pub policy: Policy,
    /// Instructions shared by every judgment in the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// The inputs, in the order results must preserve.
    pub inputs: Vec<Input>,
    /// How the request's own label set is judged. Required with
    /// `labels`, and refused alongside `dimensions` — a dimensional
    /// request carries its mode on each dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// The request's label set, for a request without dimensions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<Label>,
    /// The request's named dimensions, for a dimensional request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<Dimension>,
}

/// What the envelope validation refused, as a typed code and a message.
///
/// The codes follow the gateway's refusal vocabulary: stable strings a
/// caller can match on, with the detail in the message.
#[derive(Debug)]
pub enum Refusal {
    /// The body did not parse to the envelope — malformed JSON, a field
    /// of the wrong shape, or an unknown field.
    Malformed(String),
    /// The `v` tag names a schema this build does not serve.
    UnsupportedSchema(String),
    /// The envelope's shape is invalid: a missing identity, no inputs,
    /// an input carrying neither or both content forms, a label set
    /// under its minimum, or `labels` mixed with `dimensions`.
    InvalidRequest(String),
    /// The mode is one this facade does not serve. Reserved for modes
    /// the schema cannot name yet; the serde layer refuses the rest.
    UnsupportedMode(String),
    /// The declared backend limits are unservable: a zero bound, or a
    /// bound above the product maximum.
    UnsupportedLimits(String),
    /// An id is empty, overlong, or holds a control or invisible format
    /// character.
    InvalidId {
        /// Which collection the id belongs to: `input`, `label`, or
        /// `dimension`.
        field: &'static str,
        /// Why the id was refused.
        reason: &'static str,
    },
    /// An id appears twice in a collection that requires uniqueness.
    DuplicateId {
        /// Which collection the id belongs to.
        field: &'static str,
        /// The repeated id.
        id: String,
    },
    /// A count exceeds its bound.
    TooMany {
        /// What ran over: `inputs`, `labels`, `dimensions`, or
        /// `judgments`.
        what: &'static str,
        /// The count the request carried.
        got: u64,
        /// The effective bound.
        limit: u64,
    },
    /// A field's content exceeds its byte bound. The request is
    /// refused whole; content is never truncated to fit.
    Oversize {
        /// Which field ran over.
        what: &'static str,
        /// The size the request carried.
        got: u64,
        /// The effective bound.
        limit: u64,
    },
    /// The judgment count overflowed `u64` arithmetic — a request too
    /// large to count is too large to serve.
    Overflow,
}

impl Refusal {
    /// The refusal's stable wire code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Malformed(_) | Self::InvalidRequest(_) => "invalid_request",
            Self::UnsupportedSchema(_) => "unsupported_schema",
            Self::UnsupportedMode(_) => "unsupported_mode",
            Self::UnsupportedLimits(_) => "unsupported_limits",
            Self::InvalidId { .. } => "invalid_id",
            Self::DuplicateId { .. } => "duplicate_id",
            Self::TooMany { what, .. } => match *what {
                "inputs" => "too_many_inputs",
                "labels" => "too_many_labels",
                "dimensions" => "too_many_dimensions",
                _ => "too_many_judgments",
            },
            Self::Oversize { .. } => "content_too_large",
            Self::Overflow => "overflow",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(message) => write!(f, "{message}"),
            Self::UnsupportedSchema(got) => {
                write!(f, "schema `{got}` is not `{SCHEMA}`")
            }
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::UnsupportedMode(message) => write!(f, "{message}"),
            Self::UnsupportedLimits(message) => write!(f, "{message}"),
            Self::InvalidId { field, reason } => {
                write!(f, "a {field} id is invalid: {reason}")
            }
            Self::DuplicateId { field, id } => {
                write!(f, "the {field} id `{id}` appears twice")
            }
            Self::TooMany { what, got, limit } => {
                write!(f, "the request carries {got} {what}; the bound is {limit}")
            }
            Self::Oversize { what, got, limit } => {
                write!(f, "the {what} is {got} bytes; the bound is {limit}")
            }
            Self::Overflow => write!(f, "the request's judgment count overflowed"),
        }
    }
}

impl std::error::Error for Refusal {}

/// The primitive one planned judgment asks for.
///
/// The variant keeps the two probability semantics distinct on the
/// wire: a `choice` answer is one categorical distribution over
/// `options`, a `noul` answer is one independent probability for
/// `label`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "primitive", rename_all = "kebab-case")]
pub enum Primitive {
    /// One categorical distribution over the label set.
    Choice {
        /// The label ids, in the order the request declared them.
        options: Vec<String>,
    },
    /// One independent probability for a single label.
    Noul {
        /// The label id this judgment scores.
        label: String,
    },
}

/// One unit of classification work: a dimension's or the request's own
/// label set, the mode it is judged under, and the labels it names.
///
/// A plan carries the units in request order so the executor can build
/// each input's questions and read each input's answers without
/// re-walking the envelope.
#[derive(Clone, Debug, Serialize)]
pub struct Unit {
    /// The dimension's id, absent for a request without dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    /// How the unit's labels are judged.
    pub mode: Mode,
    /// The unit's label set, in declared order.
    pub labels: Vec<Label>,
}

/// One judgment the request would need, in the order it must report:
/// inputs in request order, then dimensions in request order, then
/// labels in their set's order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Judgment {
    /// The input's caller-chosen id.
    pub input: String,
    /// The dimension's id, absent for a request without dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    /// What the judgment asks.
    #[serde(flatten)]
    pub primitive: Primitive,
    /// The judgment's outcome at plan time — always
    /// `unattempted`. Nothing here has run, and a plan never claims an
    /// answer.
    pub outcome: Outcome,
}

/// What a valid request resolves to: the identity it named and every
/// judgment it needs, counted and ordered before anything is
/// dispatched.
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    /// The schema tag the request carried.
    pub v: String,
    /// The door the request named.
    pub model: String,
    /// The capacity selection.
    pub capacity: String,
    /// The policy identity.
    pub policy: Policy,
    /// How many inputs the request carried.
    pub inputs: u64,
    /// How many judgments the work list holds — the call's total work
    /// count, checked against the effective bound.
    pub judgments: u64,
    /// The units every input is asked, in request order.
    pub units: Vec<Unit>,
    /// The judgments, ordered input-first so results can be
    /// reconstructed without re-sorting.
    pub work: Vec<Judgment>,
}

impl Request {
    /// Parse a request body. Unknown fields, wrong shapes, and
    /// malformed JSON all refuse as `invalid_request`; the schema tag
    /// itself is checked by [`Request::plan`] so it can refuse as
    /// `unsupported_schema`.
    pub fn parse(body: &[u8]) -> Result<Self, Refusal> {
        serde_json::from_slice(body)
            .map_err(|error| Refusal::Malformed(format!("the envelope did not parse: {error}")))
    }

    /// Validate the request against the backend's declared limits and
    /// produce the work it asks for.
    ///
    /// The checks run in a fixed order: schema, identity, limits,
    /// inputs, shape, label sets, then the total work count. Every
    /// refusal is typed; nothing is truncated, defaulted away, or
    /// silently dropped.
    pub fn plan(&self, limits: &BackendLimits) -> Result<Plan, Refusal> {
        if self.v != SCHEMA {
            return Err(Refusal::UnsupportedSchema(self.v.clone()));
        }
        for (field, value) in [
            ("model", self.model.as_str()),
            ("capacity", self.capacity.as_str()),
            ("policy.name", self.policy.name.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(Refusal::InvalidRequest(format!(
                    "the envelope's `{field}` is empty"
                )));
            }
        }
        if self.policy.v != POLICY_SCHEMA {
            return Err(Refusal::InvalidRequest(format!(
                "the policy's `v` is `{}`, not `{POLICY_SCHEMA}`",
                self.policy.v
            )));
        }
        self.policy.check()?;
        limits.check()?;
        if let Some(instructions) = &self.instructions {
            check_bytes(
                "instructions",
                instructions.len() as u64,
                limits.max_instructions_bytes,
            )?;
        }
        self.check_inputs(limits)?;

        // Normalize the two shapes into one list of (dimension, mode,
        // labels) units: a flat request is one anonymous unit.
        let units: Vec<(Option<&str>, Mode, &[Label])> = if self.dimensions.is_empty() {
            if self.mode.is_none() {
                return Err(Refusal::InvalidRequest(
                    "the envelope names no `mode` for its `labels`".to_string(),
                ));
            }
            vec![(None, self.mode.unwrap_or(Mode::SingleLabel), &self.labels)]
        } else {
            if self.mode.is_some() || !self.labels.is_empty() {
                return Err(Refusal::InvalidRequest(
                    "a dimensional request carries `mode` and `labels` on each \
                     dimension, not on the envelope"
                        .to_string(),
                ));
            }
            if self.dimensions.len() as u64 > limits.max_dimensions {
                return Err(Refusal::TooMany {
                    what: "dimensions",
                    got: self.dimensions.len() as u64,
                    limit: limits.max_dimensions,
                });
            }
            check_unique("dimension", self.dimensions.iter().map(|d| d.id.as_str()))?;
            self.dimensions
                .iter()
                .map(|dimension| {
                    check_id("dimension", &dimension.id, limits.max_id_chars)?;
                    if let Some(instructions) = &dimension.instructions {
                        check_bytes(
                            "instructions",
                            instructions.len() as u64,
                            limits.max_instructions_bytes,
                        )?;
                    }
                    check_labels(&dimension.labels, dimension.mode, limits)?;
                    Ok((
                        Some(dimension.id.as_str()),
                        dimension.mode,
                        dimension.labels.as_slice(),
                    ))
                })
                .collect::<Result<_, Refusal>>()?
        };
        if self.dimensions.is_empty() {
            check_labels(&self.labels, self.mode.unwrap_or(Mode::SingleLabel), limits)?;
        }

        // The policy must declare a rule for every mode the request
        // plans, and a named no-match label must be in each set it
        // could select from.
        for (dimension, mode, labels) in &units {
            match mode {
                Mode::SingleLabel => {
                    let rule = self.policy.select.single_label.as_ref().ok_or_else(|| {
                        Refusal::InvalidRequest(
                            "the policy declares no `single_label` selection rule".to_string(),
                        )
                    })?;
                    if let NoMatch::Label { label } = &rule.no_match {
                        if !labels.iter().any(|candidate| &candidate.id == label) {
                            return Err(Refusal::InvalidRequest(format!(
                                "the policy's no-match label `{label}` is not in the label set{}",
                                dimension.map_or_else(String::new, |d| format!(" of `{d}`")),
                            )));
                        }
                    }
                }
                Mode::MultiLabel => {
                    let rule = self.policy.select.multi_label.as_ref().ok_or_else(|| {
                        Refusal::InvalidRequest(
                            "the policy declares no `multi_label` selection rule".to_string(),
                        )
                    })?;
                    if let Some(top_n) = rule.top_n {
                        if top_n > labels.len() as u64 {
                            return Err(Refusal::InvalidRequest(format!(
                                "the policy's `top_n` of {top_n} exceeds the {} labels{}",
                                labels.len(),
                                dimension.map_or_else(String::new, |d| format!(" of `{d}`")),
                            )));
                        }
                    }
                }
            }
        }

        // Count first with checked arithmetic, then build the work in
        // the same order — inputs, then dimensions, then labels.
        let inputs = self.inputs.len() as u64;
        let mut judgments: u64 = 0;
        for (_, mode, labels) in &units {
            let per_input = match mode {
                Mode::SingleLabel => 1_u64,
                Mode::MultiLabel => labels.len() as u64,
            };
            judgments = judgments
                .checked_add(inputs.checked_mul(per_input).ok_or(Refusal::Overflow)?)
                .ok_or(Refusal::Overflow)?;
        }
        if judgments > limits.max_judgments {
            return Err(Refusal::TooMany {
                what: "judgments",
                got: judgments,
                limit: limits.max_judgments,
            });
        }

        let mut work = Vec::with_capacity(judgments as usize);
        for input in &self.inputs {
            for (dimension, mode, labels) in &units {
                match mode {
                    Mode::SingleLabel => work.push(Judgment {
                        input: input.id.clone(),
                        dimension: dimension.map(str::to_string),
                        primitive: Primitive::Choice {
                            options: labels.iter().map(|label| label.id.clone()).collect(),
                        },
                        outcome: Outcome::Unattempted,
                    }),
                    Mode::MultiLabel => {
                        for label in *labels {
                            work.push(Judgment {
                                input: input.id.clone(),
                                dimension: dimension.map(str::to_string),
                                primitive: Primitive::Noul {
                                    label: label.id.clone(),
                                },
                                outcome: Outcome::Unattempted,
                            });
                        }
                    }
                }
            }
        }
        Ok(Plan {
            v: self.v.clone(),
            model: self.model.clone(),
            capacity: self.capacity.clone(),
            policy: self.policy.clone(),
            inputs,
            judgments,
            units: units
                .iter()
                .map(|(dimension, mode, labels)| Unit {
                    dimension: dimension.map(str::to_string),
                    mode: *mode,
                    labels: labels.to_vec(),
                })
                .collect(),
            work,
        })
    }

    /// The input checks: a nonempty, bounded list of unique, valid ids,
    /// each carrying exactly one bounded content form.
    fn check_inputs(&self, limits: &BackendLimits) -> Result<(), Refusal> {
        if self.inputs.is_empty() {
            return Err(Refusal::InvalidRequest(
                "the envelope carries no inputs".to_string(),
            ));
        }
        if self.inputs.len() as u64 > limits.max_inputs {
            return Err(Refusal::TooMany {
                what: "inputs",
                got: self.inputs.len() as u64,
                limit: limits.max_inputs,
            });
        }
        check_unique("input", self.inputs.iter().map(|input| input.id.as_str()))?;
        for input in &self.inputs {
            check_id("input", &input.id, limits.max_id_chars)?;
            let bytes = match (&input.text, &input.record) {
                (Some(text), None) => text.len() as u64,
                (None, Some(record)) => serde_json::to_string(record)
                    .map(|text| text.len() as u64)
                    .unwrap_or(u64::MAX),
                (None, None) => {
                    return Err(Refusal::InvalidRequest(format!(
                        "input `{}` carries neither `text` nor `record`",
                        input.id
                    )));
                }
                (Some(_), Some(_)) => {
                    return Err(Refusal::InvalidRequest(format!(
                        "input `{}` carries both `text` and `record`",
                        input.id
                    )));
                }
            };
            check_bytes("input", bytes, limits.max_input_bytes)?;
        }
        Ok(())
    }
}

/// A label set's checks: bounded, unique, valid ids, and at least two
/// labels where the mode is a categorical choice.
fn check_labels(labels: &[Label], mode: Mode, limits: &BackendLimits) -> Result<(), Refusal> {
    let minimum = match mode {
        Mode::SingleLabel => MIN_SINGLE_LABELS,
        Mode::MultiLabel => 1,
    };
    if (labels.len() as u64) < minimum {
        return Err(Refusal::InvalidRequest(format!(
            "a {mode_name} label set needs at least {minimum} labels, and this one \
             carries {}",
            labels.len(),
            mode_name = match mode {
                Mode::SingleLabel => "single-label",
                Mode::MultiLabel => "multi-label",
            },
        )));
    }
    if labels.len() as u64 > limits.max_labels {
        return Err(Refusal::TooMany {
            what: "labels",
            got: labels.len() as u64,
            limit: limits.max_labels,
        });
    }
    check_unique("label", labels.iter().map(|label| label.id.as_str()))?;
    for label in labels {
        check_id("label", &label.id, limits.max_id_chars)?;
        if let Some(description) = &label.description {
            check_bytes("label", description.len() as u64, limits.max_label_bytes)?;
        }
    }
    Ok(())
}

/// An id is nonempty, within the character bound, and free of control
/// and invisible format characters — the characters that make two
/// distinct ids render alike.
fn check_id(field: &'static str, id: &str, max_chars: u64) -> Result<(), Refusal> {
    if id.is_empty() {
        return Err(Refusal::InvalidId {
            field,
            reason: "it is empty",
        });
    }
    if id.chars().count() as u64 > max_chars {
        return Err(Refusal::InvalidId {
            field,
            reason: "it exceeds the character bound",
        });
    }
    if id.chars().any(bidirectional_or_invisible) {
        return Err(Refusal::InvalidId {
            field,
            reason: "it holds a control or invisible format character",
        });
    }
    Ok(())
}

/// Whether a character is a control or an invisible format character:
/// C0/C1 controls and DEL through `char::is_control`, plus the Unicode
/// format points for zero-width spacing, bidirectional override, word
/// joining, and annotation tagging.
fn bidirectional_or_invisible(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{061C}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0020}'..='\u{E007F}'
        )
}

/// A collection's ids must not repeat.
fn check_unique<'a>(
    field: &'static str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), Refusal> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(Refusal::DuplicateId {
                field,
                id: id.to_string(),
            });
        }
    }
    Ok(())
}

/// A field's byte size must fit its bound — oversize content is a
/// refusal, never a truncation.
fn check_bytes(what: &'static str, got: u64, limit: u64) -> Result<(), Refusal> {
    if got > limit {
        return Err(Refusal::Oversize { what, got, limit });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A policy declaring both selection rules — the tests' default.
    fn policy() -> Value {
        json!({
            "v": POLICY_SCHEMA,
            "name": "test",
            "select": {
                "single_label": {"ties": "first-declared", "no_match": {"kind": "null"}},
                "multi_label": {"threshold": 0.5, "ties": "include-all", "no_match": "empty"},
            },
        })
    }

    fn envelope() -> Value {
        json!({
            "v": SCHEMA,
            "model": "shared-kev",
            "capacity": "shared",
            "policy": policy(),
            "inputs": [
                {"id": "a", "text": "charged twice"},
                {"id": "b", "text": "where is my order"},
            ],
            "mode": "single-label",
            "labels": [{"id": "billing"}, {"id": "shipping"}],
        })
    }

    fn parse(value: &Value) -> Result<Request, Refusal> {
        Request::parse(&serde_json::to_vec(value).unwrap())
    }

    fn plan(value: &Value) -> Result<Plan, Refusal> {
        parse(value).and_then(|request| request.plan(&BackendLimits::product()))
    }

    fn code(refusal: &Refusal) -> &'static str {
        refusal.code()
    }

    #[test]
    fn a_single_label_request_plans_one_choice_per_input() {
        let plan = plan(&envelope()).unwrap();
        assert_eq!(plan.inputs, 2);
        assert_eq!(plan.judgments, 2);
        assert_eq!(plan.work.len(), 2);
        assert_eq!(plan.work[0].input, "a");
        assert_eq!(plan.work[1].input, "b");
        assert_eq!(
            plan.work[0].primitive,
            Primitive::Choice {
                options: vec!["billing".to_string(), "shipping".to_string()],
            }
        );
        assert_eq!(plan.work[0].outcome, Outcome::Unattempted);
    }

    #[test]
    fn multi_label_plans_one_noul_per_input_label_pair() {
        let mut value = envelope();
        value["mode"] = json!("multi-label");
        let plan = plan(&value).unwrap();
        assert_eq!(plan.judgments, 4);
        let labels: Vec<&str> = plan
            .work
            .iter()
            .map(|judgment| match &judgment.primitive {
                Primitive::Noul { label } => label.as_str(),
                Primitive::Choice { .. } => panic!("a multi-label plan holds no choice"),
            })
            .collect();
        assert_eq!(labels, ["billing", "shipping", "billing", "shipping"]);
        assert_eq!(plan.work[2].input, "b");
    }

    #[test]
    fn input_order_is_preserved_under_dimensions() {
        let value = json!({
            "v": SCHEMA,
            "model": "shared-kev",
            "capacity": "dedicated",
            "policy": policy(),
            "inputs": [
                {"id": "z", "text": "z"},
                {"id": "y", "record": {"body": "y"}},
            ],
            "dimensions": [
                {"id": "topic", "mode": "single-label",
                 "labels": [{"id": "a"}, {"id": "b"}]},
                {"id": "flags", "mode": "multi-label",
                 "labels": [{"id": "urgent"}]},
            ],
        });
        let plan = plan(&value).unwrap();
        // 2 inputs * 1 choice + 2 inputs * 1 noul = 4 judgments.
        assert_eq!(plan.judgments, 4);
        let order: Vec<(&str, Option<&str>)> = plan
            .work
            .iter()
            .map(|j| (j.input.as_str(), j.dimension.as_deref()))
            .collect();
        assert_eq!(
            order,
            [
                ("z", Some("topic")),
                ("z", Some("flags")),
                ("y", Some("topic")),
                ("y", Some("flags")),
            ]
        );
    }

    #[test]
    fn the_schema_tag_is_checked() {
        let mut value = envelope();
        value["v"] = json!("openagents.classify.v0");
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "unsupported_schema");
    }

    #[test]
    fn unknown_fields_are_refused() {
        let mut value = envelope();
        value["surprise"] = json!(true);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");

        let mut nested = envelope();
        nested["inputs"] = json!([{"id": "a", "text": "x", "extra": 1}]);
        let refusal = plan(&nested).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn the_shape_is_exactly_one_of_labels_or_dimensions() {
        let mut neither = envelope();
        neither["labels"] = json!([]);
        neither["mode"] = Value::Null;
        let refusal = plan(&neither).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");

        let mut both = envelope();
        both["dimensions"] = json!([{
            "id": "d", "mode": "multi-label", "labels": [{"id": "x"}],
        }]);
        let refusal = plan(&both).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");

        let mut modeless = envelope();
        modeless["mode"] = Value::Null;
        let refusal = plan(&modeless).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn empty_and_duplicate_ids_are_refused() {
        let mut duplicate = envelope();
        duplicate["inputs"] = json!([
            {"id": "a", "text": "one"},
            {"id": "a", "text": "two"},
        ]);
        let refusal = plan(&duplicate).unwrap_err();
        assert_eq!(code(&refusal), "duplicate_id");

        let mut empty = envelope();
        empty["inputs"] = json!([{"id": "", "text": "x"}]);
        let refusal = plan(&empty).unwrap_err();
        assert_eq!(code(&refusal), "invalid_id");

        let mut labels = envelope();
        labels["labels"] = json!([{"id": "x"}, {"id": "x"}]);
        let refusal = plan(&labels).unwrap_err();
        assert_eq!(code(&refusal), "duplicate_id");

        let mut dimensions = envelope();
        dimensions["mode"] = Value::Null;
        dimensions["labels"] = json!([]);
        dimensions["dimensions"] = json!([
            {"id": "d", "mode": "multi-label", "labels": [{"id": "x"}]},
            {"id": "d", "mode": "multi-label", "labels": [{"id": "y"}]},
        ]);
        let refusal = plan(&dimensions).unwrap_err();
        assert_eq!(code(&refusal), "duplicate_id");
    }

    #[test]
    fn unicode_ids_are_checked_not_escaped() {
        // Everyday Unicode is valid.
        let mut value = envelope();
        value["inputs"] = json!([
            {"id": "café-☕", "text": "héllo"},
            {"id": "b", "text": "emoji works 🎉"},
        ]);
        assert!(plan(&value).is_ok());

        // A bidi override, a zero-width space, a newline, and a BOM are not.
        for bad in [
            "a\u{202E}b",
            "a\u{200B}b",
            "a\nb",
            "\u{FEFF}a",
            "a\u{2066}b",
            "a\u{061C}b",
        ] {
            let mut value = envelope();
            value["inputs"] = json!([{"id": bad, "text": "x"}]);
            let refusal = plan(&value).unwrap_err();
            assert_eq!(code(&refusal), "invalid_id", "id {bad:?}");
        }
    }

    #[test]
    fn an_input_needs_exactly_one_content_form() {
        let mut neither = envelope();
        neither["inputs"] = json!([{"id": "a"}]);
        let refusal = plan(&neither).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");

        let mut both = envelope();
        both["inputs"] = json!([{"id": "a", "text": "x", "record": {"k": 1}}]);
        let refusal = plan(&both).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn label_minimums_follow_the_mode() {
        let mut single = envelope();
        single["labels"] = json!([{"id": "only"}]);
        let refusal = plan(&single).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");

        // One label is a valid multi-label set: that is binary filtering.
        let mut binary = envelope();
        binary["mode"] = json!("multi-label");
        binary["labels"] = json!([{"id": "keep"}]);
        let plan = plan(&binary).unwrap();
        assert_eq!(plan.judgments, 2);
    }

    #[test]
    fn the_boundaries_are_enforced() {
        // 1,001 inputs is over; 1,000 is on the line.
        let over: Vec<Value> = (0..=MAX_INPUTS)
            .map(|n| json!({"id": format!("i{n}"), "text": "x"}))
            .collect();
        let mut value = envelope();
        value["inputs"] = json!(over);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "too_many_inputs");

        let on: Vec<Value> = (0..MAX_INPUTS)
            .map(|n| json!({"id": format!("i{n}"), "text": "x"}))
            .collect();
        let mut value = envelope();
        value["inputs"] = json!(on);
        assert!(plan(&value).is_ok());

        // 101 labels is over the product maximum.
        let labels: Vec<Value> = (0..=MAX_LABELS)
            .map(|n| json!({"id": format!("l{n}")}))
            .collect();
        let mut value = envelope();
        value["labels"] = json!(labels);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "too_many_labels");

        // 21 dimensions is over; 20 is on the line.
        let dims: Vec<Value> = (0..=MAX_DIMENSIONS)
            .map(|n| {
                json!({"id": format!("d{n}"), "mode": "multi-label",
                       "labels": [{"id": "x"}]})
            })
            .collect();
        let mut value = envelope();
        value["mode"] = Value::Null;
        value["labels"] = json!([]);
        value["dimensions"] = json!(dims);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "too_many_dimensions");
    }

    #[test]
    fn the_judgment_ceiling_counts_input_label_pairs() {
        // 34 inputs * 30 labels = 1,020 judgments — over the ceiling
        // even though each side is within its own bound.
        let inputs: Vec<Value> = (0..34)
            .map(|n| json!({"id": format!("i{n}"), "text": "x"}))
            .collect();
        let labels: Vec<Value> = (0..30).map(|n| json!({"id": format!("l{n}")})).collect();
        let mut value = envelope();
        value["mode"] = json!("multi-label");
        value["inputs"] = json!(inputs);
        value["labels"] = json!(labels);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "too_many_judgments");

        // Dimensions accumulate: 2 inputs * (2 dims * 15 labels) = 60,
        // and 100 inputs * 2 dims * 5 labels = 1,000 exactly.
        let inputs: Vec<Value> = (0..100)
            .map(|n| json!({"id": format!("i{n}"), "text": "x"}))
            .collect();
        let five: Vec<Value> = (0..5).map(|n| json!({"id": format!("l{n}")})).collect();
        let mut value = envelope();
        value["mode"] = Value::Null;
        value["labels"] = json!([]);
        value["inputs"] = json!(inputs);
        value["dimensions"] = json!([
            {"id": "d1", "mode": "multi-label", "labels": five},
            {"id": "d2", "mode": "multi-label", "labels": five},
        ]);
        let plan = plan(&value).unwrap();
        assert_eq!(plan.judgments, MAX_JUDGMENTS);
    }

    #[test]
    fn missing_backend_limits_never_imply_support() {
        assert!(serde_json::from_str::<BackendLimits>("{}").is_err());
        let complete = serde_json::to_value(BackendLimits::product()).unwrap();
        for key in complete.as_object().unwrap().keys() {
            let mut partial = complete.clone();
            partial.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<BackendLimits>(partial).is_err(),
                "{key}"
            );
        }
    }

    #[test]
    fn tighter_backend_limits_apply_and_looser_ones_refuse() {
        let mut tight = BackendLimits::product();
        tight.max_labels = 2;

        let mut value = envelope();
        value["labels"] = json!([{"id": "a"}, {"id": "b"}, {"id": "c"}]);
        let request = parse(&value).unwrap();
        let refusal = request.plan(&tight).unwrap_err();
        assert_eq!(code(&refusal), "too_many_labels");
        assert!(matches!(
            refusal,
            Refusal::TooMany {
                what: "labels",
                got: 3,
                limit: 2
            }
        ));

        // A backend may not promise more than the facade serves.
        let mut loose = BackendLimits::product();
        loose.max_inputs = MAX_INPUTS + 1;
        let request = parse(&envelope()).unwrap();
        let refusal = request.plan(&loose).unwrap_err();
        assert_eq!(code(&refusal), "unsupported_limits");

        let mut zero = BackendLimits::product();
        zero.max_judgments = 0;
        let request = parse(&envelope()).unwrap();
        let refusal = request.plan(&zero).unwrap_err();
        assert_eq!(code(&refusal), "unsupported_limits");
    }

    #[test]
    fn oversize_content_is_refused_not_truncated() {
        let mut tight = BackendLimits::product();
        tight.max_input_bytes = 4;
        let mut value = envelope();
        value["inputs"] = json!([{"id": "a", "text": "twelve bytes!"}]);
        let request = parse(&value).unwrap();
        let refusal = request.plan(&tight).unwrap_err();
        assert_eq!(code(&refusal), "content_too_large");

        let mut tight = BackendLimits::product();
        tight.max_instructions_bytes = 3;
        let mut value = envelope();
        value["instructions"] = json!("too long to admit");
        let request = parse(&value).unwrap();
        let refusal = request.plan(&tight).unwrap_err();
        assert_eq!(code(&refusal), "content_too_large");

        // A record is measured by its serialized bytes.
        let mut tight = BackendLimits::product();
        tight.max_input_bytes = 8;
        let mut value = envelope();
        value["inputs"] = json!([{"id": "a", "record": {"key": "a longer value"}}]);
        let request = parse(&value).unwrap();
        let refusal = request.plan(&tight).unwrap_err();
        assert_eq!(code(&refusal), "content_too_large");
    }

    #[test]
    fn identity_fields_are_required() {
        for field in ["model", "capacity"] {
            let mut value = envelope();
            value[field] = json!("  ");
            let refusal = plan(&value).unwrap_err();
            assert_eq!(code(&refusal), "invalid_request", "field {field}");
        }
        let mut value = envelope();
        let mut wrong = policy();
        wrong["v"] = json!("other.v9");
        value["policy"] = wrong;
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn the_policy_must_declare_the_rules_the_request_uses() {
        // A single-label request with no declared single_label rule is
        // refused rather than assigned one.
        let mut value = envelope();
        value["policy"]["select"]["single_label"] = Value::Null;
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");

        // And the same for multi-label.
        let mut value = envelope();
        value["mode"] = json!("multi-label");
        value["policy"]["select"]["multi_label"] = Value::Null;
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn a_declared_cut_is_a_probability_and_a_cap_admits_a_label() {
        for cut in [-0.1, 1.1] {
            let mut value = envelope();
            value["mode"] = json!("multi-label");
            value["policy"]["select"]["multi_label"]["threshold"] = json!(cut);
            let refusal = plan(&value).unwrap_err();
            assert_eq!(code(&refusal), "invalid_request", "cut {cut}");
        }
        let mut value = envelope();
        value["mode"] = json!("multi-label");
        value["policy"]["select"]["multi_label"]["top_n"] = json!(0);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn a_no_match_label_must_be_in_the_set_it_selects_from() {
        // The designated no-match label has to be a real option.
        let mut value = envelope();
        value["labels"] = json!([{"id": "billing"}, {"id": "other"}]);
        value["policy"]["select"]["single_label"]["no_match"] =
            json!({"kind": "label", "label": "other"});
        assert!(plan(&value).is_ok());

        let mut value = envelope();
        value["policy"]["select"]["single_label"]["no_match"] =
            json!({"kind": "label", "label": "not-a-label"});
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn a_top_n_past_the_set_is_refused() {
        let mut value = envelope();
        value["mode"] = json!("multi-label");
        value["policy"]["select"]["multi_label"]["top_n"] = json!(3);
        let refusal = plan(&value).unwrap_err();
        assert_eq!(code(&refusal), "invalid_request");
    }

    #[test]
    fn single_select_applies_ties_abstention_and_no_match() {
        let rule = SingleSelect {
            ties: SingleTies::FirstDeclared,
            min_probability: None,
            no_match: NoMatch::Null,
        };
        let probabilities = |entries: &[(&str, f64)]| -> Vec<(String, f64)> {
            entries
                .iter()
                .map(|(label, p)| (label.to_string(), *p))
                .collect()
        };

        // The argmax wins; a tie keeps declared order.
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.7), ("b", 0.3)])),
            json!("a")
        );
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.5), ("b", 0.5)])),
            json!("a")
        );

        // With `no-match` tie handling the same tie reports no match.
        let rule = SingleSelect {
            ties: SingleTies::NoMatch,
            min_probability: None,
            no_match: NoMatch::Label {
                label: "other".to_string(),
            },
        };
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.5), ("b", 0.5)])),
            json!("other")
        );
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.9), ("b", 0.1)])),
            json!("a")
        );

        // An explicit abstention cut sends a weak argmax to no-match —
        // and only an explicit one.
        let rule = SingleSelect {
            ties: SingleTies::FirstDeclared,
            min_probability: Some(0.6),
            no_match: NoMatch::Null,
        };
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.55), ("b", 0.45)])),
            Value::Null
        );
    }

    #[test]
    fn multi_select_applies_threshold_top_n_and_ties() {
        let probabilities = |entries: &[(&str, f64)]| -> Vec<(String, f64)> {
            entries
                .iter()
                .map(|(label, p)| (label.to_string(), *p))
                .collect()
        };
        let rule = MultiSelect {
            threshold: 0.5,
            top_n: None,
            ties: BoundaryTies::IncludeAll,
            no_match: EmptySelection::Empty,
        };
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.9), ("b", 0.4), ("c", 0.7)])),
            json!(["a", "c"])
        );
        // Nothing over the cut reports the declared empty outcome.
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.1), ("b", 0.2)])),
            json!([])
        );
        let rule = MultiSelect {
            no_match: EmptySelection::Null,
            ..rule.clone()
        };
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.1), ("b", 0.2)])),
            Value::Null
        );

        // top-n truncates exactly, or includes the boundary tie.
        let rule = MultiSelect {
            top_n: Some(1),
            ties: BoundaryTies::Truncate,
            ..rule.clone()
        };
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.9), ("b", 0.9), ("c", 0.3)])),
            json!(["a"])
        );
        let rule = MultiSelect {
            top_n: Some(1),
            ties: BoundaryTies::IncludeAll,
            ..rule.clone()
        };
        assert_eq!(
            rule.select(&probabilities(&[("a", 0.9), ("b", 0.9), ("c", 0.3)])),
            json!(["a", "b"])
        );
    }

    #[test]
    fn a_plan_keeps_choice_and_noul_distinct_on_the_wire() {
        let value = json!({
            "v": SCHEMA,
            "model": "m",
            "capacity": "c",
            "policy": policy(),
            "inputs": [{"id": "i", "text": "x"}],
            "dimensions": [
                {"id": "cat", "mode": "single-label",
                 "labels": [{"id": "a"}, {"id": "b"}]},
                {"id": "tags", "mode": "multi-label",
                 "labels": [{"id": "x"}, {"id": "y"}]},
            ],
        });
        let plan = plan(&value).unwrap();
        let wire = serde_json::to_value(&plan.work).unwrap();
        assert_eq!(
            wire[0],
            json!({"input": "i", "dimension": "cat",
                   "primitive": "choice", "options": ["a", "b"],
                   "outcome": "unattempted"})
        );
        assert_eq!(
            wire[1],
            json!({"input": "i", "dimension": "tags",
                   "primitive": "noul", "label": "x",
                   "outcome": "unattempted"})
        );
    }
}
