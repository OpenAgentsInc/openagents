//! The answers one request returns, and how a response body becomes them.
//!
//! Decoding follows the Python SDK's dispatch path: one pass reads the
//! envelope, then each answer is read on its own. An answer type the API added
//! is skipped with a warning rather than failing the call, an unknown field is
//! ignored, and a field the SDK cannot read is named by a dotted path such as
//! `answers.tone.confidence`.
//!
//! Reading is also where the numbers are held to their contract. A
//! probability is finite and from 0 to 1, a distribution's mass is 1 within
//! [`MASS_TOLERANCE`] per entry, a picked option is one the distribution
//! names, and a score lies within its legend. A door that sends a Noul of
//! `-2.0` is not read as a typed answer; the bytes stay on the raw response.
//! [`SystemOneResponse::check_against`] then holds a response to the request
//! that produced it: every question answered in its own type, and every
//! option and level named the way the question named it.

use std::borrow::Cow;
use std::collections::BTreeMap;

use indexmap::IndexMap;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

use crate::Result;
use crate::error::{Error, REQUEST_ID_HEADER, ResponseBody};
use crate::questions::{Entry, Question, Questions};

/// How far from 1 a distribution's mass may fall, per entry.
///
/// The API reports each probability rounded to two decimals, so a
/// distribution of `n` entries may carry `n` half-cents of rounding. A
/// distribution is read when its mass is within `MASS_TOLERANCE × n` of 1,
/// and never tighter than one cent.
pub const MASS_TOLERANCE: f64 = 0.005;

/// The probability that the answer to a Noul question is yes.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NoulAnswer {
    /// The probability of yes, from 0 to 1.
    pub noul: f64,
    /// The option the estimator's own distribution named, when the door
    /// reports one.
    ///
    /// `noul` is the probability of yes and stays that even when a served
    /// calibration map has pulled it below one half: on a calibrated answer
    /// this field is what `noul` was measured on, so a reader that wants
    /// the estimator's pick reads it rather than deriving one from the
    /// number. When absent, a categorical reader falls back to yes at or
    /// above one half and no below. Absence does not prove that the door is
    /// uncalibrated; legacy responses carry no selected-answer provenance.
    #[serde(default)]
    pub selected: Option<String>,
}

/// The option a Choice question picked, with a probability for each option.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChoiceAnswer {
    /// The option the model picked.
    pub choice: String,
    /// How sharp the distribution is, as the API reports it.
    pub confidence: f64,
    /// A probability for each option, in the order the response sends them.
    pub probabilities: IndexMap<String, f64>,
}

/// Where a Score question placed the state, with the rubric it read.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScoreAnswer {
    /// The probability-weighted mean level, which falls between levels.
    /// A caller that needs a categorical level reads `selected`, or falls
    /// back to the argmax of `probabilities` when the answer lacks it.
    /// `docs/decision-models/2026-09-20-score-contract.md` states the rule.
    pub score: f64,
    /// How sharp the distribution is, as the API reports it.
    pub confidence: f64,
    /// The level the estimator's own distribution named, when the door
    /// reports one.
    ///
    /// A served calibration map rescales the selected level's probability
    /// without replacing the answer, and can leave a runner-up numerically
    /// larger in `probabilities`; this field is then the only place the
    /// estimator's pick survives the wire. When absent, a categorical reader
    /// falls back to the argmax of `probabilities`, with a tied maximum
    /// resolving to the highest level. Absence is not evidence that no
    /// calibration map was served.
    #[serde(default)]
    pub selected: Option<String>,
    /// The rubric, keyed by level.
    pub legend: BTreeMap<u32, Entry>,
    /// A probability for each level. The API leaves this out on some answers,
    /// and the field is then empty.
    #[serde(default)]
    pub probabilities: BTreeMap<u32, f64>,
}

/// One answer, of whichever type its question asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The answer to a Noul question.
    Noul(NoulAnswer),
    /// The answer to a Choice question.
    Choice(ChoiceAnswer),
    /// The answer to a Score question.
    Score(ScoreAnswer),
}

impl Answer {
    /// The wire name of the answer's type.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Noul(_) => "noul",
            Self::Choice(_) => "choice",
            Self::Score(_) => "score",
        }
    }
}

/// What one request cost, as far as the API reports it. The API leaves a count
/// out on some responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Usage {
    /// Tokens the state and the questions took.
    #[serde(default)]
    pub input_tokens: Option<u64>,
    /// Tokens the answers took.
    #[serde(default)]
    pub output_tokens: Option<u64>,
}

/// One response as it arrived: its status, its headers, and its bytes.
///
/// The bytes are kept rather than a parsed body, so the order the API sent its
/// answers and its probabilities in survives decoding.
#[derive(Debug, Clone, PartialEq)]
pub struct RawResponse {
    /// The response status.
    pub status: u16,
    /// The response headers.
    pub headers: HeaderMap,
    /// The response body, as it arrived.
    pub bytes: Vec<u8>,
}

impl RawResponse {
    /// The body parsed as JSON when it parses, and as text when it does not.
    /// An empty body is `None`.
    #[must_use]
    pub fn body(&self) -> Option<ResponseBody> {
        crate::transport::parse_body(&self.bytes)
    }

    /// The body as text, with unreadable bytes replaced.
    #[must_use]
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }

    /// The request id, when the response carried one.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.headers
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
    }
}

/// The answers to one request, with the model that gave them and what they
/// cost.
#[derive(Debug, Clone)]
pub struct SystemOneResponse {
    /// The model the API answered with.
    pub model: String,
    /// Every answer the SDK could read, keyed by question id, in the order the
    /// response sent them.
    pub answers: IndexMap<String, Answer>,
    /// What the request cost.
    pub usage: Usage,
    raw: RawResponse,
}

impl SystemOneResponse {
    /// Read one response body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ResponseValidation`] naming the first field the SDK
    /// cannot read.
    pub fn decode(raw: RawResponse) -> Result<Self> {
        // Serde reads a struct from a JSON array as well as from an object, and
        // a response is an object, so the first byte is checked here.
        if raw.bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err(validation(&raw, "body".to_string()));
        }
        let wire: Wire = serde_json::from_slice(&raw.bytes)
            .map_err(|_| validation(&raw, blame_body(&raw.bytes)))?;
        let model = wire
            .model
            .ok_or_else(|| validation(&raw, "model".to_string()))?;
        let mut answers = IndexMap::with_capacity(wire.answers.len());
        for (id, body) in &wire.answers {
            if let Some(answer) = decode_answer(&raw, id, body)? {
                answers.insert(id.clone(), answer);
            }
        }
        Ok(Self {
            model,
            answers,
            usage: wire.usage.unwrap_or_default(),
            raw,
        })
    }

    /// The response as it arrived.
    #[must_use]
    pub fn raw(&self) -> &RawResponse {
        &self.raw
    }

    /// The request id, when the response carried one. Quote it when you report
    /// a failure to TypeSafe.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.raw.request_id()
    }

    /// The Noul answer one question id holds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingAnswer`] when no answer carries that id, and
    /// [`Error::AnswerType`] when the answer is of another type.
    pub fn noul(&self, id: &str) -> Result<&NoulAnswer> {
        match self.answer(id)? {
            Answer::Noul(answer) => Ok(answer),
            other => Err(mismatch(id, "noul", other)),
        }
    }

    /// The Choice answer one question id holds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingAnswer`] when no answer carries that id, and
    /// [`Error::AnswerType`] when the answer is of another type.
    pub fn choice(&self, id: &str) -> Result<&ChoiceAnswer> {
        match self.answer(id)? {
            Answer::Choice(answer) => Ok(answer),
            other => Err(mismatch(id, "choice", other)),
        }
    }

    /// The Score answer one question id holds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingAnswer`] when no answer carries that id, and
    /// [`Error::AnswerType`] when the answer is of another type.
    pub fn score(&self, id: &str) -> Result<&ScoreAnswer> {
        match self.answer(id)? {
            Answer::Score(answer) => Ok(answer),
            other => Err(mismatch(id, "score", other)),
        }
    }

    /// Every Noul answer with its question id, in order.
    pub fn nouls(&self) -> impl Iterator<Item = (&str, &NoulAnswer)> {
        self.answers.iter().filter_map(|(id, answer)| match answer {
            Answer::Noul(answer) => Some((id.as_str(), answer)),
            _ => None,
        })
    }

    /// Every Choice answer with its question id, in order.
    pub fn choices(&self) -> impl Iterator<Item = (&str, &ChoiceAnswer)> {
        self.answers.iter().filter_map(|(id, answer)| match answer {
            Answer::Choice(answer) => Some((id.as_str(), answer)),
            _ => None,
        })
    }

    /// Every Score answer with its question id, in order.
    pub fn scores(&self) -> impl Iterator<Item = (&str, &ScoreAnswer)> {
        self.answers.iter().filter_map(|(id, answer)| match answer {
            Answer::Score(answer) => Some((id.as_str(), answer)),
            _ => None,
        })
    }

    /// Hold the response to the request that produced it.
    ///
    /// Every question must be answered in its own type. A Choice answer
    /// must name exactly the question's options, and a Score answer's
    /// legend, when it sends one, and probabilities exactly the question's
    /// levels. A
    /// [`Question::Raw`] is held only to being answered, since the SDK does
    /// not read its options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingAnswer`] for a question with no answer,
    /// [`Error::AnswerType`] for one answered in another type, and
    /// [`Error::ResponseValidation`] naming the option or level at fault.
    pub fn check_against(&self, questions: &Questions) -> Result<()> {
        for (id, question) in questions.iter() {
            let answer = self.answer(id)?;
            match (question, answer) {
                (Question::Noul(_), Answer::Noul(_)) | (Question::Raw(_), _) => {}
                (Question::Choice(asked), Answer::Choice(answer)) => {
                    let same = asked.criteria.len() == answer.probabilities.len()
                        && asked
                            .criteria
                            .keys()
                            .all(|option| answer.probabilities.contains_key(option));
                    if !same {
                        let field = answer
                            .probabilities
                            .keys()
                            .find(|option| !asked.criteria.contains_key(*option))
                            .map_or_else(
                                || "probabilities".to_string(),
                                |option| format!("probabilities.{option}"),
                            );
                        return Err(self.mismatch_field(id, &field));
                    }
                }
                (Question::Score(asked), Answer::Score(answer)) => {
                    let levels = asked.criteria.len();
                    if !answer.legend.is_empty() && !names_every_level(answer.legend.keys(), levels)
                    {
                        return Err(self.mismatch_field(id, "legend"));
                    }
                    if !answer.probabilities.is_empty()
                        && !names_every_level(answer.probabilities.keys(), levels)
                    {
                        return Err(self.mismatch_field(id, "probabilities"));
                    }
                }
                (Question::Noul(_), found) => return Err(mismatch(id, "noul", found)),
                (Question::Choice(_), found) => return Err(mismatch(id, "choice", found)),
                (Question::Score(_), found) => return Err(mismatch(id, "score", found)),
            }
        }
        Ok(())
    }

    /// The validation error for an answer field that does not match the
    /// question.
    fn mismatch_field(&self, id: &str, field: &str) -> Error {
        validation(&self.raw, format!("answers.{id}.{field}"))
    }

    /// One answer of any type.
    fn answer(&self, id: &str) -> Result<&Answer> {
        self.answers
            .get(id)
            .ok_or_else(|| Error::MissingAnswer { id: id.to_string() })
    }
}

/// The response body, with each answer left as bytes so an unknown type is
/// skipped one answer at a time.
#[derive(Deserialize)]
struct Wire {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    answers: IndexMap<String, Box<RawValue>>,
}

/// An answer's type, read before the answer itself.
#[derive(Deserialize)]
struct Tag {
    #[serde(default)]
    r#type: Option<String>,
}

/// One answer, or `None` when its type is one this crate does not model.
fn decode_answer(raw: &RawResponse, id: &str, body: &RawValue) -> Result<Option<Answer>> {
    let tag: Tag = serde_json::from_str(body.get())
        .map_err(|_| validation(raw, format!("answers.{id}.type")))?;
    let kind = tag
        .r#type
        .ok_or_else(|| validation(raw, format!("answers.{id}.type")))?;
    match kind.as_str() {
        "noul" => {
            let answer: NoulAnswer = read(raw, id, body, NOUL_FIELDS)?;
            if let Some(field) = noul_fault(&answer) {
                return Err(validation(raw, format!("answers.{id}.{field}")));
            }
            Ok(Some(Answer::Noul(answer)))
        }
        "choice" => {
            let answer: ChoiceAnswer = read(raw, id, body, CHOICE_FIELDS)?;
            if let Some(field) = choice_fault(&answer) {
                return Err(validation(raw, format!("answers.{id}.{field}")));
            }
            Ok(Some(Answer::Choice(answer)))
        }
        "score" => {
            let answer: ScoreAnswer = read(raw, id, body, SCORE_FIELDS)?;
            if let Some(field) = score_fault(&answer) {
                return Err(validation(raw, format!("answers.{id}.{field}")));
            }
            Ok(Some(Answer::Score(answer)))
        }
        other => {
            // A newer API may answer with a type this crate does not model. The
            // rest of the response still reads, and the bytes stay on `raw()`.
            tracing::warn!(
                target: "jev",
                answer = id,
                answer_type = other,
                "skipping an answer of a type this client does not read"
            );
            Ok(None)
        }
    }
}

/// Whether a level map's keys are exactly `0..levels`: as many keys as
/// levels, each below the count, with no repeats possible in a map.
fn names_every_level<'a>(keys: impl ExactSizeIterator<Item = &'a u32>, levels: usize) -> bool {
    keys.len() == levels && keys.into_iter().all(|level| (*level as usize) < levels)
}

/// Whether a number is a probability: finite, from 0 to 1.
fn is_probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// The first entry of a distribution that is not a probability, or
/// `probabilities` itself when the entries do not sum to 1 within tolerance.
/// An empty distribution is not checked; the API leaves one out on some
/// Score answers.
fn mass_fault<'a, K: std::fmt::Display>(
    probabilities: impl ExactSizeIterator<Item = (K, &'a f64)>,
) -> Option<String> {
    let entries = probabilities.len();
    if entries == 0 {
        return None;
    }
    let mut mass = 0.0;
    for (key, value) in probabilities {
        if !is_probability(*value) {
            return Some(format!("probabilities.{key}"));
        }
        mass += value;
    }
    let allowed = (MASS_TOLERANCE * entries as f64).max(0.01);
    ((mass - 1.0).abs() > allowed).then(|| "probabilities".to_string())
}

/// The field of a Noul answer that breaks the contract, if one does.
fn noul_fault(answer: &NoulAnswer) -> Option<&'static str> {
    if !is_probability(answer.noul) {
        return Some("noul");
    }
    if answer
        .selected
        .as_deref()
        .is_some_and(|option| !matches!(option, "no" | "yes"))
    {
        return Some("selected");
    }
    None
}

/// The field of a Choice answer that breaks the contract, if one does: a
/// confidence or probability out of range, a mass away from 1, an empty
/// distribution, or a choice the distribution does not name.
fn choice_fault(answer: &ChoiceAnswer) -> Option<String> {
    if !is_probability(answer.confidence) {
        return Some("confidence".to_string());
    }
    if answer.probabilities.is_empty() {
        return Some("probabilities".to_string());
    }
    if let Some(field) = mass_fault(answer.probabilities.iter()) {
        return Some(field);
    }
    if !answer.probabilities.contains_key(&answer.choice) {
        return Some("choice".to_string());
    }
    None
}

/// The field of a Score answer that breaks the contract, if one does: no
/// levels at all, a confidence or probability out of range, a mass away
/// from 1, a probability for a level the legend lacks, a score outside the
/// answer's levels, or a selected level the distribution does not name.
///
/// The legend names the levels when it has any; a door asked with level
/// indices alone sends an empty legend, and then the distribution names
/// them.
fn score_fault(answer: &ScoreAnswer) -> Option<String> {
    let (lowest, highest) = if answer.legend.is_empty() {
        match (
            answer.probabilities.keys().next(),
            answer.probabilities.keys().next_back(),
        ) {
            (Some(lowest), Some(highest)) => (*lowest, *highest),
            _ => return Some("legend".to_string()),
        }
    } else {
        let lowest = *answer.legend.keys().next()?;
        let highest = *answer.legend.keys().next_back()?;
        if let Some(level) = answer
            .probabilities
            .keys()
            .find(|level| !answer.legend.contains_key(level))
        {
            return Some(format!("probabilities.{level}"));
        }
        (lowest, highest)
    };
    if !is_probability(answer.confidence) {
        return Some("confidence".to_string());
    }
    if !answer.score.is_finite()
        || answer.score < f64::from(lowest)
        || answer.score > f64::from(highest)
    {
        return Some("score".to_string());
    }
    if let Some(field) = mass_fault(answer.probabilities.iter()) {
        return Some(field);
    }
    if let Some(selected) = &answer.selected {
        let valid = selected.parse::<u32>().ok().is_some_and(|level| {
            level.to_string() == *selected && answer.probabilities.contains_key(&level)
        });
        if !valid {
            return Some("selected".to_string());
        }
    }
    None
}

/// One answer of a known type, or the first field that stopped it.
fn read<T: for<'de> Deserialize<'de>>(
    raw: &RawResponse,
    id: &str,
    body: &RawValue,
    fields: &[Field],
) -> Result<T> {
    serde_json::from_str(body.get()).map_err(|_| {
        let path = match blame_field(body.get(), fields) {
            Some(field) => format!("answers.{id}.{field}"),
            None => format!("answers.{id}"),
        };
        validation(raw, path)
    })
}

/// One field of an answer: its name, the JSON shape it takes, and whether the
/// API may leave it out.
struct Field {
    name: &'static str,
    shape: Shape,
    required: bool,
}

/// The JSON shape a field takes on the wire.
enum Shape {
    Number,
    Text,
    Object,
}

const NOUL_FIELDS: &[Field] = &[
    Field {
        name: "noul",
        shape: Shape::Number,
        required: true,
    },
    Field {
        name: "selected",
        shape: Shape::Text,
        required: false,
    },
];

const CHOICE_FIELDS: &[Field] = &[
    Field {
        name: "choice",
        shape: Shape::Text,
        required: true,
    },
    Field {
        name: "confidence",
        shape: Shape::Number,
        required: true,
    },
    Field {
        name: "probabilities",
        shape: Shape::Object,
        required: true,
    },
];

const SCORE_FIELDS: &[Field] = &[
    Field {
        name: "score",
        shape: Shape::Number,
        required: true,
    },
    Field {
        name: "confidence",
        shape: Shape::Number,
        required: true,
    },
    Field {
        name: "legend",
        shape: Shape::Object,
        required: true,
    },
    Field {
        name: "selected",
        shape: Shape::Text,
        required: false,
    },
    Field {
        name: "probabilities",
        shape: Shape::Object,
        required: false,
    },
];

/// The first field that is missing or the wrong shape, for the dotted path an
/// error names.
fn blame_field(body: &str, fields: &[Field]) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    for field in fields {
        match value.get(field.name) {
            None => {
                if field.required {
                    return Some(field.name.to_string());
                }
            }
            Some(found) => {
                let fits = match field.shape {
                    Shape::Number => found.is_number(),
                    Shape::Text => found.is_string(),
                    Shape::Object => found.is_object(),
                };
                if !fits {
                    return Some(field.name.to_string());
                }
                // A level map decodes only when every key is a level, and a
                // probability map only when every value is a number. The path
                // names the entry at fault, the way the Python SDK does.
                if matches!(field.name, "legend" | "probabilities")
                    && let Some(bad) = blame_entry(field.name, found)
                {
                    return Some(bad);
                }
            }
        }
    }
    None
}

/// The entry of a level map that does not decode: a key that is not a level
/// number, or for `probabilities` a value that is not a number.
fn blame_entry(field: &str, map: &Value) -> Option<String> {
    for (key, value) in map.as_object()? {
        if key.parse::<u32>().is_err() || (field == "probabilities" && !value.is_number()) {
            return Some(format!("{field}.{key}"));
        }
    }
    None
}

/// Which top-level field stopped the body from reading at all.
fn blame_body(bytes: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return "body".to_string();
    };
    let Some(object) = value.as_object() else {
        return "body".to_string();
    };
    if object.get("model").is_some_and(|model| !model.is_string()) {
        return "model".to_string();
    }
    match object.get("answers") {
        Some(answers) if !answers.is_object() => "answers".to_string(),
        _ => match object.get("usage") {
            Some(usage) if !usage.is_object() && !usage.is_null() => "usage".to_string(),
            _ => "body".to_string(),
        },
    }
}

/// The error a body the SDK cannot read raises.
fn validation(raw: &RawResponse, field_path: String) -> Error {
    Error::ResponseValidation {
        status: raw.status,
        field_path,
        body: raw.body().map(Box::new),
        request_id: raw.request_id().map(str::to_string),
    }
}

/// The error a typed accessor raises when the id holds another type.
fn mismatch(id: &str, expected: &'static str, found: &Answer) -> Error {
    Error::AnswerType {
        id: id.to_string(),
        expected,
        found: found.kind(),
    }
}
