//! The `/v1/systemone` request and response shapes, and the mapping from
//! typed questions onto the pointer primitive: every question becomes a list
//! of options whose distribution is the answer.
//!
//! Mirrors `kev/api.py`: `noul` renders as the two options `no`/`yes`,
//! `choice` as one option per criteria entry (`name` or `name: description`),
//! `score` as one option per ordered level. Confidence values are arithmetic
//! on the returned distribution, not learned quantities.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, MAX_OPTIONS, Result};
use crate::render::{option_text, render};

/// One typed question: whether a condition holds, which option applies, or
/// where the state lands on an ordered rubric.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Whether something is true. `criteria` may describe the yes and no.
    Noul {
        /// The judgment to make, as any JSON value.
        #[serde(default)]
        instructions: Value,
        /// Optional `{"true": …, "false": …}` outcome descriptions.
        #[serde(default)]
        criteria: Option<IndexMap<String, Value>>,
    },
    /// Which of the named options applies.
    Choice {
        /// The judgment to make.
        #[serde(default)]
        instructions: Value,
        /// Option keys to descriptions; a null description renders the key.
        criteria: IndexMap<String, Value>,
    },
    /// Where the state lands on ordered levels; level zero is first.
    Score {
        /// The judgment to make.
        #[serde(default)]
        instructions: Value,
        /// Level descriptions in order.
        criteria: Vec<Value>,
    },
    /// A `type` this build does not model; [`Question::validate`] refuses it.
    #[serde(other)]
    Other,
}

impl Question {
    /// The wire name of the question's type.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
            Self::Other => "other",
        }
    }
}

/// The body of `POST /v1/systemone`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SystemOneRequest {
    /// The content to evaluate: string, object, or array.
    pub state: Value,
    /// The model to answer with; defaults to the served checkpoint.
    #[serde(default = "default_model")]
    pub model: String,
    /// The typed questions, keyed by caller-chosen ids the model never sees.
    pub questions: IndexMap<String, Question>,
}

fn default_model() -> String {
    "kev-latest".to_string()
}

/// One question rendered for the packed record: instruction text plus the
/// option texts the pointer head scores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordQuestion {
    /// The rendered instructions.
    pub instr: String,
    /// The rendered option texts, in order.
    pub options: Vec<String>,
    /// The labelled outcome; always `0` at serving time.
    pub label: usize,
}

/// A request flattened into the record `encode()` packs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The rendered state text.
    pub state: String,
    /// The rendered questions.
    pub questions: Vec<RecordQuestion>,
}

/// What a question needs to map its option distribution back into an answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Meta {
    /// The caller's question id.
    pub id: String,
    /// The question's wire type.
    #[serde(rename = "type")]
    pub kind: String,
    /// The option keys in order; present for `choice`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
    /// Level index to level text; present for `score`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<IndexMap<String, String>>,
}

/// The probability that a `noul` answer is yes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename = "noul")]
pub struct NoulAnswer {
    /// `p(yes)`.
    ///
    /// Kev serves no calibration map, so the pick is what the number itself
    /// implies — yes at or above one half — and the answer carries no
    /// `selected`. A calibrated door writes that field because its `noul`
    /// can sit below one half while the pick stays yes.
    pub noul: f64,
}

/// The option a `choice` question picked, with the full distribution.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename = "choice")]
pub struct ChoiceAnswer {
    /// The option with the greatest probability. Equal leaders resolve to
    /// the last option listed, the convention `gym::calibrate::selected`
    /// declares for categorical selection.
    pub choice: String,
    /// `(p_max − 1/K) / (1 − 1/K)`: how far the leader stands above uniform.
    pub confidence: f64,
    /// The probability of each option, keyed by option name.
    pub probabilities: IndexMap<String, f64>,
}

/// The level a `score` question placed the state at.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename = "score")]
pub struct ScoreAnswer {
    /// The probability-weighted mean level, `Σ i · p_i`, which can land
    /// between levels. The wire shape carries no categorical level: a
    /// reader that wants one takes the argmax of `probabilities`, and the
    /// retained categorical accuracy measurements compare that level with
    /// the label. `docs/decision-models/2026-09-20-score-contract.md`
    /// states both, and the tie convention.
    ///
    /// Kev serves no calibration map, so the pick is always the argmax and
    /// the answer carries no `selected`. A calibrated door writes that
    /// field because a map can leave the pick below a runner-up in
    /// `probabilities`.
    pub score: f64,
    /// Distance-from-mode statistic; TypeSafe's formula is unpublished, so
    /// this uses `1 − E|level − mode| / (L − 1)` like the reference.
    pub confidence: f64,
    /// Level index to level text.
    pub legend: IndexMap<String, String>,
    /// The probability of each level.
    pub probabilities: IndexMap<String, f64>,
}

/// One answer, tagged by its question's type.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    /// The answer to a `noul` question.
    Noul(NoulAnswer),
    /// The answer to a `choice` question.
    Choice(ChoiceAnswer),
    /// The answer to a `score` question.
    Score(ScoreAnswer),
}

impl SystemOneRequest {
    /// Check the bounds the model and the contract enforce. Question ids in
    /// errors are the caller's keys.
    ///
    /// # Errors
    ///
    /// Returns a typed [`Error`] for an empty question map, an unsupported
    /// type, missing or mis-shaped criteria, or an out-of-bounds option count.
    pub fn validate(&self) -> Result<()> {
        if self.questions.is_empty() {
            return Err(Error::EmptyQuestions);
        }
        for (id, question) in &self.questions {
            match question {
                Question::Other => {
                    return Err(Error::UnsupportedQuestionType {
                        id: id.clone(),
                        found: "other".to_string(),
                    });
                }
                Question::Choice { criteria, .. } => {
                    if criteria.len() > MAX_OPTIONS {
                        return Err(Error::TooManyOptions {
                            id: id.clone(),
                            count: criteria.len(),
                        });
                    }
                    if criteria.is_empty() {
                        return Err(Error::MissingChoiceCriteria { id: id.clone() });
                    }
                }
                Question::Score { criteria, .. } => {
                    if criteria.len() > MAX_OPTIONS {
                        return Err(Error::TooManyOptions {
                            id: id.clone(),
                            count: criteria.len(),
                        });
                    }
                    if criteria.len() < 2 {
                        return Err(Error::TooFewLevels {
                            id: id.clone(),
                            count: criteria.len(),
                        });
                    }
                }
                Question::Noul { .. } => {}
            }
        }
        Ok(())
    }
}

/// `round(x, 2)` with Python's ties-to-even semantics, for the answer fields
/// the API reports at two decimals.
#[must_use]
pub fn r2(x: f64) -> f64 {
    (x * 100.0).round_ties_even() / 100.0
}

/// Choice confidence: `(p_max − 1/K) / (1 − 1/K)`, `1.0` for a single option.
#[must_use]
pub fn choice_confidence(p: &[f64]) -> f64 {
    let k = p.len();
    if k == 1 {
        return 1.0;
    }
    let max = p.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (max - 1.0 / k as f64) / (1.0 - 1.0 / k as f64)
}

/// Score confidence: `1 − E|level − mode| / (L − 1)`, the stand-in formula
/// the reference uses while TypeSafe's stays unpublished.
#[must_use]
pub fn score_confidence(p: &[f64]) -> f64 {
    let levels = p.len();
    let mode = p
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i);
    let spread: f64 = p
        .iter()
        .enumerate()
        .map(|(i, pi)| pi * i.abs_diff(mode) as f64)
        .sum();
    1.0 - spread / (levels - 1) as f64
}

/// Flatten a validated request into the packed record plus the metadata that
/// maps each question's distribution back into a typed answer.
///
/// # Errors
///
/// Returns the [`SystemOneRequest::validate`] error for a request outside the
/// contract's bounds.
pub fn to_record(request: &SystemOneRequest) -> Result<(Record, Vec<Meta>)> {
    request.validate()?;
    let mut questions = Vec::with_capacity(request.questions.len());
    let mut meta = Vec::with_capacity(request.questions.len());
    for (id, question) in &request.questions {
        match question {
            Question::Noul {
                instructions,
                criteria,
            } => {
                let empty = IndexMap::new();
                let criteria = criteria.as_ref().unwrap_or(&empty);
                questions.push(RecordQuestion {
                    instr: render(instructions),
                    options: vec![
                        option_text("no", criteria.get("false").unwrap_or(&Value::Null)),
                        option_text("yes", criteria.get("true").unwrap_or(&Value::Null)),
                    ],
                    label: 0,
                });
                meta.push(Meta {
                    id: id.clone(),
                    kind: "noul".to_string(),
                    keys: None,
                    legend: None,
                });
            }
            Question::Choice {
                instructions,
                criteria,
            } => {
                questions.push(RecordQuestion {
                    instr: render(instructions),
                    options: criteria
                        .iter()
                        .map(|(name, description)| option_text(name, description))
                        .collect(),
                    label: 0,
                });
                meta.push(Meta {
                    id: id.clone(),
                    kind: "choice".to_string(),
                    keys: Some(criteria.keys().cloned().collect()),
                    legend: None,
                });
            }
            Question::Score {
                instructions,
                criteria,
            } => {
                questions.push(RecordQuestion {
                    instr: render(instructions),
                    options: criteria.iter().map(render).collect(),
                    label: 0,
                });
                meta.push(Meta {
                    id: id.clone(),
                    kind: "score".to_string(),
                    keys: None,
                    legend: Some(
                        criteria
                            .iter()
                            .enumerate()
                            .map(|(i, level)| (i.to_string(), render(level)))
                            .collect(),
                    ),
                });
            }
            Question::Other => unreachable!("validate() refuses Other"),
        }
    }
    Ok((
        Record {
            state: render(&request.state),
            questions,
        },
        meta,
    ))
}

/// Turn per-question option distributions into the typed answers the
/// contract returns, in request order.
#[must_use]
pub fn to_answers(probs: &[Vec<f64>], meta: &[Meta]) -> IndexMap<String, Answer> {
    let mut out = IndexMap::with_capacity(meta.len());
    for (p, m) in probs.iter().zip(meta.iter()) {
        let answer = match m.kind.as_str() {
            "noul" => Answer::Noul(NoulAnswer {
                noul: r2(p.get(1).copied().unwrap_or(0.0)),
            }),
            "choice" => {
                let keys = m.keys.as_deref().unwrap_or(&[]);
                let choice = p
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .and_then(|(i, _)| keys.get(i))
                    .cloned()
                    .unwrap_or_default();
                Answer::Choice(ChoiceAnswer {
                    choice,
                    confidence: r2(choice_confidence(p)),
                    probabilities: keys.iter().cloned().zip(p.iter().map(|v| r2(*v))).collect(),
                })
            }
            _ => {
                let score: f64 = p.iter().enumerate().map(|(i, pi)| i as f64 * pi).sum();
                Answer::Score(ScoreAnswer {
                    score: r2(score),
                    confidence: r2(score_confidence(p)),
                    legend: m.legend.clone().unwrap_or_default(),
                    probabilities: p
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), r2(*v)))
                        .collect(),
                })
            }
        };
        out.insert(m.id.clone(), answer);
    }
    out
}
