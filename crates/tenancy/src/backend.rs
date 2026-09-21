//! The backend capability record: what a door's backend can serve, and
//! under which identity.
//!
//! The manifest's [`Expected`] names the artifact a binding pins; this
//! record is the serving side's declaration — the primitives it answers,
//! the bounds it enforces, the capacity it can be bound under, and the
//! identity it reports it loaded. A record is versioned and
//! self-digested like the manifest itself, so a caller, a gateway, and a
//! later reviewer all read the same words the backend wrote.
//!
//! # Claims, not measurements
//!
//! A record declares capability; it does not measure quality.
//! `available` means the backend answers now — never that it answers
//! well. And capability is closed: a primitive, modality, or lane the
//! record does not declare is unsupported, not unknown. `unknown` exists
//! only as an availability state, for a backend that has not reported.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::manifest::{Expected, Lane};

/// The schema tag a backend record carries. A file that names another
/// version is refused rather than read partially.
pub const SCHEMA: &str = "openagents.tenancy.backend.v1";

/// Why a record failed to load or validate.
#[derive(Debug)]
pub enum Rejection {
    /// The document did not parse — malformed JSON, a missing field, or a
    /// field this schema does not carry.
    Malformed(String),
    /// The schema tag is not this version's.
    UnknownSchema(String),
    /// The digest does not recompute over the contents.
    Tampered,
    /// A field failed a check; the message names which and why.
    Invalid(String),
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(error) => write!(f, "malformed record: {error}"),
            Self::UnknownSchema(schema) => {
                write!(f, "record schema `{schema}` is not `{SCHEMA}`")
            }
            Self::Tampered => write!(
                f,
                "the record's digest does not recompute over its contents"
            ),
            Self::Invalid(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Rejection {}

/// A typed primitive a backend can answer — the question types of
/// `POST /v1/systemone`. A primitive absent from a record is unsupported;
/// capability is never read as unknown.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Primitive {
    /// Probability that a proposition holds — `noul`.
    Noul,
    /// One alternative from a supplied set — `choice`.
    Choice,
    /// A position on an ordered, described rubric — `score`.
    Score,
}

/// The modality a request's state may carry. Text is the only modality
/// this contract defines; a file that names another is refused at parse.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Modality {
    /// Text or structured JSON state.
    Text,
}

/// The identity a record names — what the deployment bound (`requested`)
/// or what the backend reports it loaded (`actual`).
///
/// The fields mirror [`Expected`]: a model id, an adapter when one
/// applies, a `sha256:`-prefixed content digest when the artifact is
/// pinned or reported, and the execution settings the identity was
/// measured or serves under. An empty signature is a name and nothing
/// more — the right shape for a hosted closed model, and a label rather
/// than a proof.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The model id.
    pub model: String,
    /// The adapter package, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// The content digest — `sha256:` followed by 64 lowercase hex
    /// characters — or empty when the artifact is unpinned or unreported.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact_signature: String,
    /// The execution settings — dtype, backend, and the other numerical
    /// choices a behavior was measured or is served under.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub execution: BTreeMap<String, String>,
}

impl From<Identity> for Expected {
    /// Read a record's identity as the manifest's bound expectation, so a
    /// registry or gateway compares like with like.
    fn from(identity: Identity) -> Self {
        Self {
            model: identity.model,
            adapter: identity.adapter,
            artifact_signature: identity.artifact_signature,
            execution: identity.execution,
        }
    }
}

/// The request bounds a backend enforces.
///
/// Every declared bound admits at least one unit of work — a bound of
/// zero is not a bound, it is a refusal to say. `context_tokens` is
/// required: every backend has a window, and an undeclared one is a gap
/// rather than an absence of limit. The optional bounds are declared only
/// where the backend enforces them. An absent bound is unknown; a caller
/// must resolve it through an explicit admission policy before dispatch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// The total context a call may occupy, in tokens.
    pub context_tokens: u64,
    /// The largest state a call may carry, in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_bytes: Option<u64>,
    /// The questions a call may carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub questions_per_call: Option<u64>,
    /// The options a `choice` question may carry — at least two, since
    /// one option is not a choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_per_choice: Option<u64>,
    /// The levels a `score` rubric may carry. The contract spans two to
    /// ten ordered levels; a backend's ceiling sits inside that span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_levels: Option<u64>,
    /// The labels a classification call may carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels_per_call: Option<u64>,
    /// The characters a single label may run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_chars: Option<u64>,
    /// The characters a question's instructions may run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_chars: Option<u64>,
}

/// How a backend serves batch work.
///
/// A bulk payload and inference batching are different capabilities, and
/// the record says which this backend has: `native` packs items into one
/// execution and names its bound, `caller-loop` is bounded independent
/// calls and carries no batch bound — there is no batch to bound.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BatchKind {
    /// The backend packs items into one execution.
    Native,
    /// Batching is the caller's loop of independent calls.
    CallerLoop,
}

/// The batching declaration: the kind, and the bound when there is one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Batching {
    /// Whether the backend packs items natively or the caller loops.
    pub kind: BatchKind,
    /// The items a native batch may pack — at least two, since one item
    /// is a call, not a batch. Required on `native`; refused on
    /// `caller-loop`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
}

/// How a backend's probabilities are read.
///
/// These fields describe semantics, not accuracy — a record can say its
/// distributions normalize without claiming they are calibrated, which
/// is a measurement's business rather than a declaration's.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scoring {
    /// Each multi-option answer's probabilities form a normalized
    /// distribution over the question's options.
    pub normalized: bool,
    /// A `score` answer is the probability-weighted position on the
    /// rubric, not a sampled level.
    pub probability_weighted: bool,
    /// The backend reports a concentration `confidence` beside the
    /// answer, the way `choice` and `score` carry one and `noul` does
    /// not.
    pub reports_confidence: bool,
}

/// Whether the backend answers now.
///
/// `unavailable` and `unknown` are different claims: unavailable means
/// the service knows the backend does not answer and names the cause,
/// unknown means nothing has reported and the record cannot say. Neither
/// is `available`, and neither erases the declared capabilities — an
/// unavailable backend still declares what it would serve when it
/// returns.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AvailabilityState {
    /// The backend answers now.
    Available,
    /// The backend is known not to answer.
    Unavailable,
    /// Nothing has reported — the record cannot claim the backend
    /// serves.
    Unknown,
}

/// The availability declaration: the state, and the cause when there is
/// one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Availability {
    /// Whether the backend answers now.
    pub state: AvailabilityState,
    /// Why the backend does not answer. Required on `unavailable`;
    /// refused otherwise — a cause on an available backend contradicts
    /// the state it rides beside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// The record itself: one backend's declared capabilities under one
/// digest.
///
/// `digest` covers every field but itself, canonicalized key-sorted JSON
/// over SHA-256 — the same self-verifying shape the manifest and the
/// execution receipt use. A file that cannot recompute its own digest is
/// refused before it is read.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// The schema tag.
    pub v: String,
    /// The deployment's name for this backend — the door or adapter the
    /// record describes.
    pub name: String,
    /// The primitives the backend answers. Declared, never guessed: a
    /// primitive absent here is unsupported, not unknown.
    pub primitives: Vec<Primitive>,
    /// The modalities a request's state may carry.
    pub modalities: Vec<Modality>,
    /// The languages the backend claims, as BCP-47 tags.
    pub languages: Vec<String>,
    /// The request bounds the backend enforces.
    pub limits: Limits,
    /// How batch work is served.
    pub batching: Batching,
    /// How the backend's probabilities are read.
    pub scoring: Scoring,
    /// The lanes the backend may be bound under — the capacity shapes it
    /// supports. Capacity is a separate axis from the artifact: the same
    /// checkpoint can serve shared and dedicated capacity, and a lane
    /// change must not silently change which artifact answers.
    pub capacity: Vec<Lane>,
    /// The identity the backend was asked to serve — what the deployment
    /// bound.
    pub requested: Identity,
    /// The identity the backend reports it loaded, when it has reported.
    /// Required when the record claims `available`; refused when the
    /// state is `unknown`, since a backend that has not reported cannot
    /// name what it loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<Identity>,
    /// Whether the backend answers now.
    pub availability: Availability,
    /// The digest over every field above.
    pub digest: String,
}

impl Record {
    /// Fill in `digest` over the record's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a record serializes");
        value
            .as_object_mut()
            .expect("a record is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Serialize the record as it is written — pretty JSON carrying its
    /// own digest.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a record serializes")
    }

    /// Read and validate a record's text.
    ///
    /// Refused: a field this schema does not carry, a schema tag that is
    /// not this version's, a digest that does not recompute, or any of
    /// the consistency checks [`Record::validate`] runs.
    pub fn parse(text: &str) -> Result<Self, Rejection> {
        let record: Self =
            serde_json::from_str(text).map_err(|error| Rejection::Malformed(error.to_string()))?;
        record.validate()?;
        Ok(record)
    }

    /// The checks a record must pass, in the order they run — schema,
    /// digest, then each field's bounds and the consistency between
    /// availability and the identity the backend reports. The order is
    /// fixed so the same record always fails on the same check.
    pub fn validate(&self) -> Result<(), Rejection> {
        if self.v != SCHEMA {
            return Err(Rejection::UnknownSchema(self.v.clone()));
        }
        if self.digest != self.compute_digest() {
            return Err(Rejection::Tampered);
        }
        self.checks()
    }

    /// Whether the record declares a primitive. An undeclared primitive
    /// is unsupported — capability is binary here, and `unknown` is a
    /// state of availability, never of capability.
    #[must_use]
    pub fn supports_primitive(&self, primitive: Primitive) -> bool {
        self.primitives.contains(&primitive)
    }

    /// Whether the record declares a lane it may be bound under.
    #[must_use]
    pub fn supports_capacity(&self, lane: Lane) -> bool {
        self.capacity.contains(&lane)
    }

    /// Whether the backend answers now. `unknown` is not `available`, and
    /// `unavailable` is not `unknown`.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.availability.state == AvailabilityState::Available
    }

    /// The field-level checks [`Record::validate`] runs after schema and
    /// digest.
    fn checks(&self) -> Result<(), Rejection> {
        let invalid = |message: &str| Rejection::Invalid(message.to_string());
        if self.name.is_empty() {
            return Err(invalid("the record names no backend"));
        }
        if self.primitives.is_empty() {
            return Err(invalid(
                "the record declares no primitives — a backend that answers nothing \
                 declares an empty capability, not a supported one",
            ));
        }
        unique(&self.primitives, "primitive")?;
        if self.modalities.is_empty() {
            return Err(invalid("the record declares no modality a state may carry"));
        }
        unique(&self.modalities, "modality")?;
        if self.languages.is_empty() {
            return Err(invalid("the record claims no language"));
        }
        for language in &self.languages {
            if !valid_language_tag(language) {
                return Err(invalid(
                    "a language tag must be non-empty ASCII letters, digits, and hyphens",
                ));
            }
        }
        unique(
            &self
                .languages
                .iter()
                .map(|tag| tag.to_ascii_lowercase())
                .collect::<Vec<_>>(),
            "language",
        )?;
        if self.capacity.is_empty() {
            return Err(invalid(
                "the record names no capacity it may be bound under",
            ));
        }
        unique(&self.capacity, "capacity lane")?;
        check_limits(&self.limits)?;
        check_batching(&self.batching)?;
        check_identity(&self.requested, "requested")?;
        match self.availability.state {
            AvailabilityState::Available => {
                if self.availability.cause.is_some() {
                    return Err(invalid(
                        "an available backend carries no cause — the cause contradicts the state",
                    ));
                }
                match &self.actual {
                    Some(actual) => check_identity(actual, "actual")?,
                    None => {
                        return Err(invalid(
                            "an available backend reports the identity it loaded — \
                             `actual` cannot be absent when the record claims it answers",
                        ));
                    }
                }
            }
            AvailabilityState::Unavailable => {
                if self.availability.cause.as_deref().is_none_or(str::is_empty) {
                    return Err(invalid(
                        "an unavailable backend names its cause — `unavailable` without \
                         a cause is a claim that cannot be checked",
                    ));
                }
                if let Some(actual) = &self.actual {
                    check_identity(actual, "actual")?;
                }
            }
            AvailabilityState::Unknown => {
                if self.availability.cause.is_some() {
                    return Err(invalid(
                        "an unknown backend carries no cause — nothing has reported one",
                    ));
                }
                if self.actual.is_some() {
                    return Err(invalid(
                        "an unknown backend reports no `actual` identity — nothing has \
                         reported what it loaded",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A declared item appears once — duplicates are refused rather than
/// silently deduplicated.
fn unique<T: PartialEq>(items: &[T], what: &str) -> Result<(), Rejection> {
    for (index, item) in items.iter().enumerate() {
        if items[..index].contains(item) {
            return Err(Rejection::Invalid(format!("a {what} is declared twice")));
        }
    }
    Ok(())
}

/// A language tag is a label, checked for shape rather than against a
/// registry: non-empty, ASCII letters and digits, hyphen-separated.
fn valid_language_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.split('-').all(|part| {
            !part.is_empty()
                && part.len() <= 8
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        && tag
            .split('-')
            .next()
            .is_some_and(|part| part.bytes().all(|byte| byte.is_ascii_alphabetic()))
}

/// The checks `Limits` must pass: every declared bound admits work.
fn check_limits(limits: &Limits) -> Result<(), Rejection> {
    if limits.context_tokens == 0 {
        return Err(Rejection::Invalid(
            "`context_tokens` admits nothing — a bound of zero is not a bound".to_string(),
        ));
    }
    for (name, bound) in [
        ("state_bytes", limits.state_bytes),
        ("questions_per_call", limits.questions_per_call),
        ("labels_per_call", limits.labels_per_call),
        ("label_chars", limits.label_chars),
        ("instructions_chars", limits.instructions_chars),
    ] {
        if bound == Some(0) {
            return Err(Rejection::Invalid(format!(
                "`{name}` admits nothing — a bound of zero is not a bound"
            )));
        }
    }
    if limits.options_per_choice.is_some_and(|bound| bound < 2) {
        return Err(Rejection::Invalid(
            "`options_per_choice` under two — one option is not a choice".to_string(),
        ));
    }
    if limits
        .score_levels
        .is_some_and(|bound| !(2..=10).contains(&bound))
    {
        return Err(Rejection::Invalid(
            "`score_levels` sits inside the contract's two-to-ten span".to_string(),
        ));
    }
    Ok(())
}

/// The checks `Batching` must pass: the kind and the bound agree.
fn check_batching(batching: &Batching) -> Result<(), Rejection> {
    match batching.kind {
        BatchKind::Native => {
            if !batching.max_items.is_some_and(|bound| bound >= 2) {
                return Err(Rejection::Invalid(
                    "native batching names a `max_items` of two or more — one item is a \
                     call, not a batch"
                        .to_string(),
                ));
            }
        }
        BatchKind::CallerLoop => {
            if batching.max_items.is_some() {
                return Err(Rejection::Invalid(
                    "a caller loop carries no `max_items` — there is no batch to bound".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// The checks an `Identity` must pass wherever it appears.
fn check_identity(identity: &Identity, which: &str) -> Result<(), Rejection> {
    if identity.model.is_empty() {
        return Err(Rejection::Invalid(format!(
            "the {which} identity names no model"
        )));
    }
    if !identity.artifact_signature.is_empty()
        && !identity
            .artifact_signature
            .strip_prefix("sha256:")
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
    {
        return Err(Rejection::Invalid(format!(
            "the {which} identity pins artifact signature `{}`, which is not `sha256:` \
             followed by 64 hex characters",
            identity.artifact_signature
        )));
    }
    Ok(())
}

/// Canonical JSON: keys sorted, whitespace gone, so two writers digest the
/// same content to the same bytes. The same canonicalization the manifest
/// and the execution receipt digest by. The keys are sorted here rather
/// than trusted to the map: `preserve_order` makes a `serde_json` map
/// insertion-ordered whenever a sibling crate enables it, and the digest
/// agreement must not depend on who wrote the bytes.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_of(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    /// A complete record: every capability declared, `available` with the
    /// identity it reports, sealed. The names are illustrative — a record
    /// is a claim shape, not a measurement of any model.
    fn valid_record() -> Record {
        let mut record = Record {
            v: SCHEMA.to_string(),
            name: "example-backend".to_string(),
            primitives: vec![Primitive::Noul, Primitive::Choice, Primitive::Score],
            modalities: vec![Modality::Text],
            languages: vec!["en".to_string(), "mul".to_string()],
            limits: Limits {
                context_tokens: 8192,
                state_bytes: Some(64 * 1024),
                questions_per_call: Some(32),
                options_per_choice: Some(16),
                score_levels: Some(10),
                labels_per_call: Some(100),
                label_chars: Some(64),
                instructions_chars: Some(4096),
            },
            batching: Batching {
                kind: BatchKind::CallerLoop,
                max_items: None,
            },
            scoring: Scoring {
                normalized: true,
                probability_weighted: true,
                reports_confidence: true,
            },
            capacity: vec![Lane::Shared, Lane::Dedicated],
            requested: Identity {
                model: "example-1".to_string(),
                artifact_signature: digest_of('a'),
                execution: [("dtype".to_string(), "bf16".to_string())]
                    .into_iter()
                    .collect(),
                ..Identity::default()
            },
            actual: Some(Identity {
                model: "example-1".to_string(),
                artifact_signature: digest_of('a'),
                execution: [("dtype".to_string(), "bf16".to_string())]
                    .into_iter()
                    .collect(),
                ..Identity::default()
            }),
            availability: Availability {
                state: AvailabilityState::Available,
                cause: None,
            },
            digest: String::new(),
        };
        record.seal();
        record
    }

    #[test]
    fn a_sealed_record_round_trips_and_validates() {
        let record = valid_record();
        let parsed = Record::parse(&record.to_json()).unwrap();
        assert_eq!(parsed, record);
        assert!(parsed.is_available());
    }

    #[test]
    fn the_digest_is_stable_and_field_sensitive() {
        // Two writers of the same record agree on the digest.
        assert_eq!(valid_record().compute_digest(), valid_record().digest);
        // Any covered field changes it.
        let mut other = valid_record();
        other.name = "example-backend-2".to_string();
        other.seal();
        assert_ne!(other.digest, valid_record().digest);
    }

    #[test]
    fn a_tampered_record_refuses() {
        let text = valid_record()
            .to_json()
            .replacen("example-1", "example-9", 1);
        assert!(matches!(Record::parse(&text), Err(Rejection::Tampered)));
    }

    #[test]
    fn an_unknown_schema_is_refused() {
        let mut record = valid_record();
        record.v = "openagents.tenancy.backend.v0".to_string();
        record.seal();
        assert!(matches!(
            Record::parse(&record.to_json()),
            Err(Rejection::UnknownSchema(_))
        ));
    }

    #[test]
    fn an_unknown_field_is_refused() {
        let mut value: Value = serde_json::from_str(&valid_record().to_json()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("telemetry".to_string(), Value::from(true));
        assert!(matches!(
            Record::parse(&value.to_string()),
            Err(Rejection::Malformed(_))
        ));
        // Nested structs refuse unknown fields too.
        let mut value: Value = serde_json::from_str(&valid_record().to_json()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .get_mut("limits")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("queue_depth".to_string(), Value::from(8));
        assert!(matches!(
            Record::parse(&value.to_string()),
            Err(Rejection::Malformed(_))
        ));
    }

    #[test]
    fn an_available_record_reports_what_it_loaded() {
        let mut record = valid_record();
        record.actual = None;
        record.seal();
        assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));

        let mut caused = valid_record();
        caused.availability.cause = Some("warmed".to_string());
        caused.seal();
        assert!(matches!(caused.validate(), Err(Rejection::Invalid(_))));
    }

    #[test]
    fn an_unknown_record_reports_nothing() {
        let mut record = valid_record();
        record.availability.state = AvailabilityState::Unknown;
        record.seal();
        // `unknown` with a reported identity contradicts itself.
        assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));
        record.actual = None;
        record.seal();
        record.validate().unwrap();
        assert!(!record.is_available());
    }

    #[test]
    fn an_unavailable_record_names_its_cause() {
        let mut record = valid_record();
        record.availability.state = AvailabilityState::Unavailable;
        record.seal();
        assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));
        record.availability.cause = Some("backend process down".to_string());
        record.seal();
        record.validate().unwrap();
        assert!(!record.is_available());
    }

    #[test]
    fn malformed_language_labels_and_noncanonical_signatures_refuse() {
        for tag in ["--", "en--US", "-en", "en-", "123", "toolonglanguage"] {
            let mut record = valid_record();
            record.languages = vec![tag.into()];
            record.seal();
            assert!(record.validate().is_err(), "{tag}");
        }
        let mut record = valid_record();
        record.languages = vec!["en-US".into(), "EN-us".into()];
        record.seal();
        assert!(record.validate().is_err());
        let mut record = valid_record();
        record.requested.artifact_signature = digest_of('A');
        record.seal();
        assert!(record.validate().is_err());
    }

    #[test]
    fn invalid_bounds_are_refused() {
        for mutate in [
            (|record: &mut Record| record.limits.context_tokens = 0) as fn(&mut Record),
            (|record: &mut Record| record.limits.questions_per_call = Some(0)) as fn(&mut Record),
            (|record: &mut Record| record.limits.options_per_choice = Some(1)) as fn(&mut Record),
            (|record: &mut Record| record.limits.score_levels = Some(11)) as fn(&mut Record),
            (|record: &mut Record| record.limits.label_chars = Some(0)) as fn(&mut Record),
        ] {
            let mut record = valid_record();
            mutate(&mut record);
            record.seal();
            assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));
        }
    }

    #[test]
    fn batching_kind_and_bound_must_agree() {
        // Native batching names its bound.
        let mut record = valid_record();
        record.batching = Batching {
            kind: BatchKind::Native,
            max_items: None,
        };
        record.seal();
        assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));

        // One item is a call, not a batch.
        record.batching = Batching {
            kind: BatchKind::Native,
            max_items: Some(1),
        };
        record.seal();
        assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));

        // A caller loop has no batch to bound.
        record.batching = Batching {
            kind: BatchKind::CallerLoop,
            max_items: Some(8),
        };
        record.seal();
        assert!(matches!(record.validate(), Err(Rejection::Invalid(_))));

        record.batching = Batching {
            kind: BatchKind::Native,
            max_items: Some(64),
        };
        record.seal();
        record.validate().unwrap();
    }

    #[test]
    fn capacity_is_a_separate_axis_from_artifact_identity() {
        let shared = valid_record();
        let mut dedicated = valid_record();
        dedicated.capacity = vec![Lane::Dedicated];
        dedicated.seal();
        // The checkpoint did not move — only the capacity axis did.
        assert_eq!(shared.requested, dedicated.requested);
        assert_eq!(shared.actual, dedicated.actual);
        assert_ne!(shared.digest, dedicated.digest);
        assert!(shared.supports_capacity(Lane::Shared));
        assert!(!dedicated.supports_capacity(Lane::Shared));
    }

    #[test]
    fn a_drifted_actual_identity_is_recorded_not_refused() {
        // A backend honestly reporting a drifted artifact is not
        // malformed — the comparison against `requested` is the check a
        // registry or gateway runs, not this record's.
        let mut record = valid_record();
        record.actual.as_mut().unwrap().artifact_signature = digest_of('f');
        record.seal();
        let parsed = Record::parse(&record.to_json()).unwrap();
        assert_ne!(
            parsed.requested.artifact_signature,
            parsed.actual.unwrap().artifact_signature
        );
    }

    #[test]
    fn an_undeclared_capability_is_unsupported_not_unknown() {
        let mut record = valid_record();
        record.primitives.retain(|p| *p != Primitive::Score);
        record.seal();
        assert!(!record.supports_primitive(Primitive::Score));
        assert!(record.supports_primitive(Primitive::Noul));

        // Duplicates are refused rather than deduplicated.
        let mut duplicated = valid_record();
        duplicated.primitives.push(Primitive::Noul);
        duplicated.seal();
        assert!(matches!(duplicated.validate(), Err(Rejection::Invalid(_))));

        // A record that declares nothing is refused.
        let mut empty = valid_record();
        empty.primitives.clear();
        empty.seal();
        assert!(matches!(empty.validate(), Err(Rejection::Invalid(_))));
    }
}
