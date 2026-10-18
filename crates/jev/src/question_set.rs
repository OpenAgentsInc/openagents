//! Question sets asked through the Decisions API (`POST /v1/decisions`).
//!
//! A question set is one file in the repository's `questions/` folder: an
//! identifier, the question the caller gates on, and typed questions
//! (`noul`, `choice`, `score`) whose wording reads named fields of one JSON
//! state. [`QuestionSet::request`] turns a set and a state into a
//! [`DecisionsRequest`]; [`Judgment::from_response`] keeps the typed
//! answers with a [`Receipt`] a caller records beside the decision that
//! used them.
//!
//! The [`coordination`] sets judge the calls an agent that coordinates
//! other agents makes on every check-in. Any agent may ask them.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Result;
use crate::decisions::{
    ChoiceOption, DecisionAnswer, DecisionQuestion, DecisionsRequest, DecisionsResponse, Level,
};
use crate::error::Error;

/// The model the coordination sets are asked of: Clef first, Jev as backup.
pub const MODEL: &str = "openagents/decide";

/// One question set, in request order.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionSet {
    /// The identifier, such as `openagents.run-progress.v1`.
    pub id: String,
    pub name: String,
    /// The question whose answer the caller acts on first.
    pub gate: String,
    pub questions: Vec<DecisionQuestion>,
    /// The largest state, in bytes of JSON, the set is asked over.
    pub state_max_bytes: Option<u64>,
}

#[derive(Deserialize)]
struct File {
    v: u32,
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    gate: String,
    questions: indexmap::IndexMap<String, FileQuestion>,
    #[serde(default)]
    policy: FilePolicy,
}

#[derive(Deserialize)]
struct FileQuestion {
    #[serde(rename = "type")]
    kind: String,
    instructions: Value,
    #[serde(default)]
    criteria: Criteria,
}

/// A question's criteria, in file order: a Choice's options or a Score's
/// levels.
#[derive(Default, Deserialize)]
#[serde(untagged)]
enum Criteria {
    Options(indexmap::IndexMap<String, Value>),
    Levels(Vec<Value>),
    #[default]
    None,
}

#[derive(Default, Deserialize)]
struct FilePolicy {
    #[serde(default)]
    state_max_bytes: Option<u64>,
}

/// An entry's wording: a string as it is, anything else as compact JSON.
fn wording(entry: &Value) -> String {
    match entry {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

impl QuestionSet {
    /// Reads a question-set file (body version 1).
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a file that doesn't parse, a version this
    /// doesn't read, no questions, a gate that names no question, or a
    /// question this can't send.
    pub fn parse(text: &str) -> Result<Self> {
        let file: File = serde_json::from_str(text)
            .map_err(|error| Error::Config(format!("question set: {error}")))?;
        let refused = |why: String| Error::Config(format!("question set {}: {why}", file.id));
        if file.v != 1 {
            return Err(refused(format!("body version {} is not 1", file.v)));
        }
        if file.questions.is_empty() {
            return Err(refused("it asks no questions".into()));
        }
        if !file.gate.is_empty() && !file.questions.contains_key(&file.gate) {
            return Err(refused(format!(
                "the gate `{}` is not a question",
                file.gate
            )));
        }
        let mut questions = Vec::with_capacity(file.questions.len());
        for (name, question) in &file.questions {
            let instructions = wording(&question.instructions);
            questions.push(match question.kind.as_str() {
                "noul" | "predicate" => DecisionQuestion::Predicate {
                    name: name.clone(),
                    instructions,
                    yes: None,
                    no: None,
                },
                "choice" => {
                    let Criteria::Options(criteria) = &question.criteria else {
                        return Err(refused(format!("`{name}` names no options")));
                    };
                    DecisionQuestion::Choice {
                        name: name.clone(),
                        instructions,
                        choices: criteria
                            .iter()
                            .map(|(value, description)| ChoiceOption {
                                value: value.clone(),
                                description: (!description.is_null()).then(|| wording(description)),
                            })
                            .collect(),
                    }
                }
                "score" => {
                    let Criteria::Levels(levels) = &question.criteria else {
                        return Err(refused(format!("`{name}` names no levels")));
                    };
                    DecisionQuestion::Score {
                        name: name.clone(),
                        instructions,
                        levels: levels
                            .iter()
                            .enumerate()
                            .map(|(index, level)| Level {
                                label: level
                                    .get("summary")
                                    .and_then(Value::as_str)
                                    .map_or_else(|| format!("level {}", index + 1), str::to_owned),
                                description: Some(wording(level)),
                            })
                            .collect(),
                    }
                }
                other => return Err(refused(format!("`{name}` has type `{other}`"))),
            });
        }
        Ok(Self {
            id: file.id,
            name: file.name,
            gate: file.gate,
            questions,
            state_max_bytes: file.policy.state_max_bytes,
        })
    }

    /// The question named `name`.
    #[must_use]
    pub fn question(&self, name: &str) -> Option<&DecisionQuestion> {
        self.questions.iter().find(|q| match q {
            DecisionQuestion::Predicate { name: n, .. }
            | DecisionQuestion::Choice { name: n, .. }
            | DecisionQuestion::Score { name: n, .. } => n == name,
        })
    }

    /// The request that asks every question of the set over `state`, sent
    /// as JSON text so the wording's backticked paths name its fields. The
    /// set's identifier goes in the request's metadata.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] when the state is larger than the set admits.
    pub fn request(&self, model: &str, state: &Value) -> Result<DecisionsRequest> {
        let text = serde_json::to_string_pretty(state)
            .map_err(|error| Error::Config(format!("state: {error}")))?;
        if let Some(max) = self.state_max_bytes
            && text.len() as u64 > max
        {
            return Err(Error::Config(format!(
                "the state is {} bytes, over the {max} that {} admits",
                text.len(),
                self.id
            )));
        }
        let mut request =
            DecisionsRequest::new(model, text).metadata("question_set", self.id.clone());
        request.questions.clone_from(&self.questions);
        Ok(request)
    }
}

/// What a caller records beside a decision that used a judgment: which
/// set was asked, the call's identity, and who answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub question_set: String,
    /// The response's `id`.
    pub decision: String,
    /// The model that answered, by its served name.
    pub model: String,
    /// `clef`, `pylon:<slug>`, `jev`, or `vertex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<String>,
    /// The door's own receipt, when it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    #[serde(default)]
    pub latency_ms: u64,
}

/// The typed answers to one set over one state, with their receipt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Judgment {
    pub answers: Vec<DecisionAnswer>,
    pub receipt: Receipt,
}

impl Judgment {
    /// The judgment in a response to [`QuestionSet::request`].
    ///
    /// # Errors
    ///
    /// [`Error::Config`] when a question of the set has no answer.
    pub fn from_response(set: &QuestionSet, response: DecisionsResponse) -> Result<Self> {
        for question in &set.questions {
            let (DecisionQuestion::Predicate { name, .. }
            | DecisionQuestion::Choice { name, .. }
            | DecisionQuestion::Score { name, .. }) = question;
            if response.answer(name).is_none() {
                return Err(Error::Config(format!(
                    "no answer to `{name}` of {}",
                    set.id
                )));
            }
        }
        Ok(Self {
            receipt: Receipt {
                question_set: set.id.clone(),
                decision: response.id.clone(),
                model: response.model.clone(),
                door: response.openagents.door.clone(),
                receipt: response.openagents.receipt.clone(),
                latency_ms: response.openagents.latency_ms,
            },
            answers: response.answers,
        })
    }

    /// The answer to question `name`.
    #[must_use]
    pub fn answer(&self, name: &str) -> Option<&DecisionAnswer> {
        self.answers.iter().find(|a| a.name() == name)
    }

    /// A Noul's probability.
    #[must_use]
    pub fn probability(&self, name: &str) -> Option<f64> {
        match self.answer(name)? {
            DecisionAnswer::Predicate { probability, .. } => Some(*probability),
            _ => None,
        }
    }

    /// A Choice's picked option.
    #[must_use]
    pub fn choice(&self, name: &str) -> Option<&str> {
        match self.answer(name)? {
            DecisionAnswer::Choice { choice, .. } => Some(choice),
            _ => None,
        }
    }

    /// One plain line per answer (`progressed 0.82`, `activity stopped`),
    /// for a log a person reads.
    #[must_use]
    pub fn summary(&self) -> String {
        self.answers
            .iter()
            .map(|answer| match answer {
                DecisionAnswer::Predicate { name, probability } => {
                    format!("{name} {probability:.2}")
                }
                DecisionAnswer::Choice {
                    name,
                    choice,
                    confidence,
                    ..
                } => format!("{name} {choice} ({confidence:.2})"),
                DecisionAnswer::Score { name, score, .. } => format!("{name} {score:.2}"),
                DecisionAnswer::Refusal { name, code, .. } => format!("{name} refused: {code}"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub mod coordination {
    //! The sets an agent coordinating other agents asks on a check-in
    //! (`docs/decision-models/measurements/2026-10-18-coordination-judgments.md`):
    //!
    //! - [`RUN_PROGRESS`]: has this run moved its work forward since the
    //!   last check-in?
    //! - [`FAILURE_ON_MAIN`]: does this branch's failing test also fail on
    //!   `main`?
    //! - [`CHANGE_OVERLAP`]: do these two changes waiting to land overlap?
    //! - [`READY_TO_CLOSE`]: is this issue's code complete?

    use super::{BTreeMap, LazyLock, QuestionSet};

    /// `questions/run-progress.json`.
    pub const RUN_PROGRESS: &str = "run-progress";
    /// `questions/failure-on-main.json`.
    pub const FAILURE_ON_MAIN: &str = "failure-on-main";
    /// `questions/change-overlap.json`.
    pub const CHANGE_OVERLAP: &str = "change-overlap";
    /// `questions/ready-to-close.json`.
    pub const READY_TO_CLOSE: &str = "ready-to-close";

    const FILES: [(&str, &str); 4] = [
        (
            RUN_PROGRESS,
            include_str!("../../../questions/run-progress.json"),
        ),
        (
            FAILURE_ON_MAIN,
            include_str!("../../../questions/failure-on-main.json"),
        ),
        (
            CHANGE_OVERLAP,
            include_str!("../../../questions/change-overlap.json"),
        ),
        (
            READY_TO_CLOSE,
            include_str!("../../../questions/ready-to-close.json"),
        ),
    ];

    static SETS: LazyLock<BTreeMap<&'static str, QuestionSet>> = LazyLock::new(|| {
        FILES
            .iter()
            .map(|(name, text)| {
                let set = QuestionSet::parse(text)
                    .unwrap_or_else(|error| panic!("questions/{name}.json: {error}"));
                (*name, set)
            })
            .collect()
    });

    /// The set named `name` (its file name without `.json`).
    #[must_use]
    pub fn set(name: &str) -> Option<&'static QuestionSet> {
        SETS.get(name)
    }

    /// Every set's name.
    #[must_use]
    pub fn names() -> Vec<&'static str> {
        SETS.keys().copied().collect()
    }
}

#[cfg(test)]
#[path = "question_set_tests.rs"]
mod tests;
