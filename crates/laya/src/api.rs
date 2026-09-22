//! The `/v1/systemone` request and response shapes: typed questions in,
//! the reference's typed answers out.
//!
//! Answer semantics follow `rl_agent_api.py`, not kev's: probabilities
//! round to four decimals, confidence is `1 − normalized entropy` for
//! every type, and each answer carries the model's own act probability
//! under `rl_agent`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{QTYPE_CHOICE, QTYPE_NOUL, QTYPE_SCORE};
use crate::encode::{InternalQuestion, json_dumps};
use crate::error::{Error, MAX_OPTIONS, Result};

/// One typed question: whether a condition holds, which option applies, or
/// where the state lands on an ordered rubric.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Whether something is true. `criteria` may describe the `true` and
    /// `false` outcomes.
    Noul {
        /// The judgment to make, as any JSON value.
        instructions: Value,
        /// Optional `{"true": …, "false": …}` outcome descriptions.
        criteria: Option<IndexMap<String, Value>>,
    },
    /// Which of the named options applies.
    Choice {
        /// The judgment to make.
        instructions: Value,
        /// Option keys to descriptions, or a bare list of option names —
        /// the reference accepts both and treats a list as
        /// `{name: null}`.
        criteria: ChoiceCriteria,
    },
    /// Where the state lands on ordered levels; level zero is first.
    Score {
        /// The judgment to make.
        instructions: Value,
        /// Level descriptions in order.
        criteria: Vec<Value>,
    },
    /// A `type` this build does not model; [`Question::validate`] refuses
    /// it.
    Other,
}

impl<'de> Deserialize<'de> for Question {
    /// The tag decides the variant, then each variant reads the two
    /// fields it declares. A field the port does not model is refused
    /// rather than silently dropped — `deny_unknown_fields` cannot sit
    /// on an internally tagged enum, so the check runs here.
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        let mut fields = serde_json::Map::<String, Value>::deserialize(deserializer)?;
        let kind = match fields.remove("type") {
            Some(Value::String(kind)) => kind,
            _ => return Err(D::Error::missing_field("type")),
        };
        const FIELDS: &[&str] = &["instructions", "criteria"];
        if let Some(extra) = fields.keys().find(|name| !FIELDS.contains(&name.as_str())) {
            return Err(D::Error::unknown_field(extra, FIELDS));
        }
        let instructions = fields.remove("instructions").unwrap_or(Value::Null);
        let criteria = fields.remove("criteria");
        match kind.as_str() {
            "noul" => Ok(Self::Noul {
                instructions,
                criteria: criteria
                    .map(|value| serde_json::from_value(value).map_err(D::Error::custom))
                    .transpose()?
                    .unwrap_or_default(),
            }),
            "choice" => Ok(Self::Choice {
                instructions,
                criteria: serde_json::from_value(
                    criteria.ok_or_else(|| D::Error::missing_field("criteria"))?,
                )
                .map_err(D::Error::custom)?,
            }),
            "score" => Ok(Self::Score {
                instructions,
                criteria: serde_json::from_value(
                    criteria.ok_or_else(|| D::Error::missing_field("criteria"))?,
                )
                .map_err(D::Error::custom)?,
            }),
            _ => Ok(Self::Other),
        }
    }
}

/// `choice` criteria: an ordered name-to-description map, or the bare
/// list the reference normalizes to `{name: null}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChoiceCriteria {
    /// `{"name": description, …}`.
    Map(IndexMap<String, Value>),
    /// `["name", …]`.
    List(Vec<String>),
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneRequest {
    /// The content to evaluate: string, object, or array.
    pub state: Value,
    /// The model to answer with; defaults to the served checkpoint.
    #[serde(default)]
    pub model: String,
    /// The typed questions, keyed by caller-chosen ids the model never
    /// sees.
    pub questions: IndexMap<String, Question>,
}

/// What a question needs to map its option distribution back into an
/// answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    /// The caller's question id.
    pub id: String,
    /// The `QTYPES` index.
    pub qtype: usize,
    /// The option keys in order; present for `choice`.
    pub keys: Option<Vec<String>>,
    /// Level index to level text; present for `score`.
    pub legend: Option<Vec<String>>,
}

impl SystemOneRequest {
    /// Check the bounds the contract enforces. Question ids in errors are
    /// the caller's keys.
    ///
    /// # Errors
    ///
    /// Returns a typed [`Error`] for an empty question map, an unsupported
    /// type, missing or mis-shaped criteria, or an out-of-bounds option
    /// count.
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
                    let count = match criteria {
                        ChoiceCriteria::Map(map) => map.len(),
                        ChoiceCriteria::List(list) => list.len(),
                    };
                    if count > MAX_OPTIONS {
                        return Err(Error::TooManyOptions {
                            id: id.clone(),
                            count,
                        });
                    }
                    if count == 0 {
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
                Question::Noul { criteria, .. } => {
                    // The reference reads `criteria["false"]` and
                    // `criteria["true"]` only; any other key is a field
                    // the model never sees, so it is refused rather than
                    // silently dropped.
                    if let Some(key) = criteria.as_ref().and_then(|c| {
                        c.keys()
                            .find(|k| k.as_str() != "true" && k.as_str() != "false")
                    }) {
                        return Err(Error::UnsupportedNoulCriteria {
                            id: id.clone(),
                            key: key.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// `_to_internal` for every question: normalize the criteria shape and
    /// serialize non-string instructions. Runs after `validate`, so
    /// criteria presence is already established.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRequest`] only through `validate`.
    pub fn to_internal(&self) -> Result<Vec<(InternalQuestion, Meta)>> {
        self.validate()?;
        let mut out = Vec::with_capacity(self.questions.len());
        for (id, question) in &self.questions {
            let (internal, meta) = match question {
                Question::Noul {
                    instructions,
                    criteria,
                } => (
                    InternalQuestion {
                        qtype: QTYPE_NOUL,
                        instructions: instruction_text(instructions),
                        choice: None,
                        score: None,
                        noul: Some((
                            criteria.as_ref().and_then(|c| c.get("false").cloned()),
                            criteria.as_ref().and_then(|c| c.get("true").cloned()),
                        )),
                    },
                    Meta {
                        id: id.clone(),
                        qtype: QTYPE_NOUL,
                        keys: None,
                        legend: None,
                    },
                ),
                Question::Choice {
                    instructions,
                    criteria,
                } => {
                    // `if t == "choice" and isinstance(crit, list)`:
                    // a bare list becomes `{c: None}` in the reference.
                    let pairs: Vec<(String, Value)> = match criteria {
                        ChoiceCriteria::Map(map) => {
                            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                        }
                        ChoiceCriteria::List(list) => list
                            .iter()
                            .map(|name| (name.clone(), Value::Null))
                            .collect(),
                    };
                    (
                        InternalQuestion {
                            qtype: QTYPE_CHOICE,
                            instructions: instruction_text(instructions),
                            choice: Some(pairs.clone()),
                            score: None,
                            noul: None,
                        },
                        Meta {
                            id: id.clone(),
                            qtype: QTYPE_CHOICE,
                            keys: Some(pairs.iter().map(|(k, _)| k.clone()).collect()),
                            legend: None,
                        },
                    )
                }
                Question::Score {
                    instructions,
                    criteria,
                } => (
                    InternalQuestion {
                        qtype: QTYPE_SCORE,
                        instructions: instruction_text(instructions),
                        choice: None,
                        score: Some(criteria.clone()),
                        noul: None,
                    },
                    Meta {
                        id: id.clone(),
                        qtype: QTYPE_SCORE,
                        keys: None,
                        legend: Some(criteria.iter().map(crate::encode::py_str).collect()),
                    },
                ),
                Question::Other => unreachable!("validate() refuses Other"),
            };
            out.push((internal, meta));
        }
        Ok(out)
    }
}

/// `qdef["instructions"] if isinstance(str) else json.dumps(...)`: a
/// string is itself; anything else serializes.
fn instruction_text(instructions: &Value) -> String {
    match instructions {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => json_dumps(other),
    }
}

/// `round(x, 4)` with Python's ties-to-even semantics, for the answer
/// fields the reference reports at four decimals.
#[must_use]
pub fn r4(x: f64) -> f64 {
    (x * 10000.0).round_ties_even() / 10000.0
}

/// `confidence_from_probs`: `1 − normalized entropy` of the answer
/// distribution, the Jev-style figure the reference reports for every
/// question type.
#[must_use]
pub fn confidence(p: &[f64], k: usize) -> f64 {
    if k < 2 {
        return 1.0;
    }
    let ent: f64 = p[..k.min(p.len())]
        .iter()
        .map(|&pi| -pi * pi.clamp(1e-12, 1.0).ln())
        .sum();
    1.0 - ent / (k as f64).ln()
}

/// The `rl_agent` extension every answer carries: the model's own
/// act-path probability.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RlAgent {
    /// `softmax(act_logits)[0]` — the reference's act head, emitted as
    /// measured; the port assigns it no policy meaning.
    pub act_probability: f64,
}

/// The probability that a `noul` answer is yes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoulAnswer {
    /// `p(true)`, rounded to four decimals.
    pub noul: f64,
    /// The model's act-path extension.
    pub rl_agent: RlAgent,
}

/// The option a `choice` question picked, with the full distribution.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChoiceAnswer {
    /// The option with the greatest probability; ties resolve to the
    /// first, matching `argmax` in the reference.
    pub choice: String,
    /// The probability of each option, keyed by option name.
    pub probabilities: IndexMap<String, f64>,
    /// `1 − normalized entropy`, rounded to four decimals.
    pub confidence: f64,
    /// The model's act-path extension.
    pub rl_agent: RlAgent,
}

/// The level a `score` question placed the state at.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreAnswer {
    /// The probability-weighted mean level, `Σ i · p_i`, which can land
    /// between levels.
    pub score: f64,
    /// Level index to level text.
    pub legend: IndexMap<String, String>,
    /// The probability of each level.
    pub probabilities: IndexMap<String, f64>,
    /// `1 − normalized entropy`, rounded to four decimals.
    pub confidence: f64,
    /// The model's act-path extension.
    pub rl_agent: RlAgent,
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

/// Turn per-question option distributions and act probabilities into the
/// typed answers the reference returns, in request order.
///
/// `probs[r]` is the temperature-scaled softmax over question `r`'s
/// options; `acts[r]` is `softmax(act_logits)[0]`.
#[must_use]
pub fn to_answers(probs: &[Vec<f64>], acts: &[f64], meta: &[Meta]) -> IndexMap<String, Answer> {
    let mut out = IndexMap::with_capacity(meta.len());
    for ((p, act), m) in probs.iter().zip(acts.iter()).zip(meta.iter()) {
        let ext = RlAgent {
            act_probability: *act,
        };
        let k = p.len();
        let answer = match m.qtype {
            QTYPE_NOUL => Answer::Noul(NoulAnswer {
                noul: r4(p.get(1).copied().unwrap_or(0.0)),
                rl_agent: ext,
            }),
            QTYPE_CHOICE => {
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
                    probabilities: keys.iter().cloned().zip(p.iter().map(|v| r4(*v))).collect(),
                    confidence: r4(confidence(p, k)),
                    rl_agent: ext,
                })
            }
            _ => {
                let score: f64 = p.iter().enumerate().map(|(i, pi)| i as f64 * pi).sum();
                Answer::Score(ScoreAnswer {
                    score: r4(score),
                    legend: m
                        .legend
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .enumerate()
                        .map(|(i, level)| (i.to_string(), level.clone()))
                        .collect(),
                    probabilities: p
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), r4(*v)))
                        .collect(),
                    confidence: r4(confidence(p, k)),
                    rl_agent: ext,
                })
            }
        };
        out.insert(m.id.clone(), answer);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(questions: Value) -> SystemOneRequest {
        serde_json::from_value(json!({
            "state": "s", "model": "m", "questions": questions
        }))
        .unwrap()
    }

    #[test]
    fn validation_refuses_empty_and_unsupported() {
        let empty = request(json!({}));
        assert!(matches!(empty.validate(), Err(Error::EmptyQuestions)));
        let other = request(json!({"q": {"type": "rank", "instructions": "x"}}));
        assert!(matches!(
            other.validate(),
            Err(Error::UnsupportedQuestionType { .. })
        ));
        let no_criteria =
            request(json!({"q": {"type": "choice", "instructions": "x", "criteria": {}}}));
        assert!(matches!(
            no_criteria.validate(),
            Err(Error::MissingChoiceCriteria { .. })
        ));
        let one_level =
            request(json!({"q": {"type": "score", "instructions": "x", "criteria": ["a"]}}));
        assert!(matches!(
            one_level.validate(),
            Err(Error::TooFewLevels { count: 1, .. })
        ));
    }

    #[test]
    fn fields_the_model_never_reads_are_refused() {
        // A field on a question the port does not model is a request
        // error, not silent input.
        assert!(
            serde_json::from_value::<SystemOneRequest>(json!({
                "state": "s", "model": "m",
                "questions": {"q": {"type": "choice", "criteria": ["a", "b"], "weight": 2}}
            }))
            .is_err()
        );
        // A noul criteria key outside `true`/`false` reaches validation
        // and is refused there, naming the question.
        let stray = request(json!({
            "q": {"type": "noul", "instructions": "x", "criteria": {"maybe": "?"}}
        }));
        assert!(matches!(
            stray.validate(),
            Err(Error::UnsupportedNoulCriteria { .. })
        ));
        // The two outcomes the model reads still pass.
        let fine = request(json!({
            "q": {"type": "noul", "instructions": "x", "criteria": {"true": "yes", "false": "no"}}
        }));
        fine.validate().unwrap();
    }

    #[test]
    fn choice_list_criteria_becomes_null_descriptions() {
        let req = request(json!({
            "q": {"type": "choice", "instructions": "x", "criteria": ["a", "b"]}
        }));
        let (internal, meta) = req.to_internal().unwrap().remove(0);
        let pairs = internal.choice.unwrap();
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].1.is_null());
        assert_eq!(meta.keys.unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn confidence_is_one_minus_normalized_entropy() {
        // Uniform over 4: entropy ln 4, confidence 0.
        assert_eq!(confidence(&[0.25; 4], 4), 0.0);
        // One-hot: entropy 0, confidence 1.
        assert!((confidence(&[1.0, 0.0, 0.0], 3) - 1.0).abs() < 1e-9);
        // Single option: k < 2, always 1.
        assert_eq!(confidence(&[1.0], 1), 1.0);
    }

    #[test]
    fn rounding_uses_ties_to_even_at_four_decimals() {
        assert_eq!(r4(0.12345), 0.1234); // wait: 0.12345 * 1e4 = 1234.5 -> ties even -> 1234
        assert_eq!(r4(0.12355), 0.1236);
        assert_eq!(r4(0.5), 0.5);
        assert_eq!(r4(0.99999), 1.0);
    }

    #[test]
    fn answers_follow_the_reference_shape() {
        let meta = vec![
            Meta {
                id: "n".to_string(),
                qtype: QTYPE_NOUL,
                keys: None,
                legend: None,
            },
            Meta {
                id: "c".to_string(),
                qtype: QTYPE_CHOICE,
                keys: Some(vec!["a".to_string(), "b".to_string()]),
                legend: None,
            },
            Meta {
                id: "s".to_string(),
                qtype: QTYPE_SCORE,
                keys: None,
                legend: Some(vec!["lo".to_string(), "hi".to_string()]),
            },
        ];
        let probs = vec![vec![0.3, 0.7], vec![0.4, 0.6], vec![0.25, 0.75]];
        let acts = vec![0.9, 0.8, 0.7];
        let answers = to_answers(&probs, &acts, &meta);
        match &answers["n"] {
            Answer::Noul(a) => {
                assert_eq!(a.noul, 0.7);
                assert_eq!(a.rl_agent.act_probability, 0.9);
            }
            _ => panic!("noul answer expected"),
        }
        match &answers["c"] {
            Answer::Choice(a) => {
                assert_eq!(a.choice, "b");
                assert_eq!(a.probabilities["a"], 0.4);
            }
            _ => panic!("choice answer expected"),
        }
        match &answers["s"] {
            Answer::Score(a) => {
                assert_eq!(a.score, 0.75);
                assert_eq!(a.legend["0"], "lo");
                assert_eq!(a.probabilities["1"], 0.75);
            }
            _ => panic!("score answer expected"),
        }
        let wire = serde_json::to_value(&answers["n"]).unwrap();
        assert_eq!(wire["type"], "noul");
        assert!(wire.get("rl_agent").is_some());
    }
}
