//! The System One request and answer shapes, as `crates/jev` sends and reads
//! them.
//!
//! These types are deliberately a separate implementation from the ones in
//! `crates/jev` and `crates/kev`. Three implementations of one contract only
//! mean something if each one can be wrong on its own and a conformance test
//! catches it.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Refusal, RefusalCode, Result};

/// The most options a Choice admits.
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// The fewest options a Choice admits. A one-option Choice is not a choice.
pub const MIN_CHOICE_OPTIONS: usize = 2;
/// The fewest levels a Score admits.
pub const MIN_SCORE_LEVELS: usize = 2;
/// The most levels a Score admits.
pub const MAX_SCORE_LEVELS: usize = 10;

/// One request: one state, one map of questions.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SystemOneRequest {
    /// The document every question reads.
    pub state: Value,
    /// The model id, or none for the door's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The questions, keyed by ids the model never sees.
    pub questions: IndexMap<String, Question>,
    /// Opt-in response extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
}

/// Opt-in additions a caller asks for. An unmodified client sends none.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Extensions {
    /// Ask each answer to carry which estimator ran and what it recorded.
    #[serde(default)]
    pub estimator: bool,
    /// Refuse rather than return a number no fitted calibration map backs.
    #[serde(default)]
    pub require_calibration: bool,
    /// The question family this request belongs to.
    ///
    /// A calibration map is fitted per family, and the contract carries a
    /// state and a question with nothing in either that says which family
    /// they are. Naming it is how a caller reaches a fitted map; a request
    /// that names none is answered with the raw signal, and is refused when
    /// it also sets `require_calibration`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
}

impl Extensions {
    /// Whether the caller asked for nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.estimator && !self.require_calibration && self.family.is_none()
    }
}

/// The criteria a Noul may carry.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NoulCriteria {
    /// What a yes means.
    #[serde(rename = "true", default, skip_serializing_if = "Option::is_none")]
    pub yes: Option<Value>,
    /// What a no means.
    #[serde(rename = "false", default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,
}

/// One typed question.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    /// Is this true?
    Noul {
        /// The judgment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
        /// What yes and no mean.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// Which one of these named options?
    Choice {
        /// The judgment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
        /// The options, keyed by name, each with an optional description.
        criteria: IndexMap<String, Option<Value>>,
    },
    /// Which level on this ordered rubric?
    Score {
        /// The judgment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
        /// The levels, in order.
        criteria: Vec<Option<Value>>,
    },
}

impl Question {
    /// The wire label for this question's type.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
        }
    }
}

impl SystemOneRequest {
    /// Checks every bound the contract sets, before any call.
    pub fn validate(&self) -> Result<()> {
        if self.questions.is_empty() {
            return Err(Refusal::new(
                RefusalCode::InvalidRequest,
                "a request asks at least one question",
            ));
        }
        for (id, question) in &self.questions {
            validate_question(id, question)?;
        }
        Ok(())
    }
}

fn validate_question(id: &str, question: &Question) -> Result<()> {
    match question {
        Question::Noul { .. } => Ok(()),
        Question::Choice { criteria, .. } => {
            let options = criteria.len();
            if options > MAX_CHOICE_OPTIONS {
                return Err(Refusal::question(
                    RefusalCode::TooManyOptions,
                    id,
                    format!(
                        "a Choice question names at most {MAX_CHOICE_OPTIONS} options, and this one names {options}"
                    ),
                ));
            }
            if options < MIN_CHOICE_OPTIONS {
                return Err(Refusal::question(
                    RefusalCode::InvalidRequest,
                    id,
                    format!(
                        "a Choice question names at least {MIN_CHOICE_OPTIONS} options, and this one names {options}"
                    ),
                ));
            }
            for key in criteria.keys() {
                if key.trim().is_empty() {
                    return Err(Refusal::question(
                        RefusalCode::InvalidRequest,
                        id,
                        "a Choice option name is blank",
                    ));
                }
                if key.trim() != key {
                    return Err(Refusal::question(
                        RefusalCode::InvalidRequest,
                        id,
                        format!(
                            "the Choice option '{key}' carries surrounding whitespace, which a constrained enum cannot round-trip"
                        ),
                    ));
                }
            }
            Ok(())
        }
        Question::Score { criteria, .. } => {
            let levels = criteria.len();
            if levels < MIN_SCORE_LEVELS {
                return Err(Refusal::question(
                    RefusalCode::InvalidRequest,
                    id,
                    format!(
                        "a Score question names at least {MIN_SCORE_LEVELS} levels, and this one names {levels}"
                    ),
                ));
            }
            if levels > MAX_SCORE_LEVELS {
                return Err(Refusal::question(
                    RefusalCode::InvalidRequest,
                    id,
                    format!(
                        "a Score question names at most {MAX_SCORE_LEVELS} levels, and this one names {levels}"
                    ),
                ));
            }
            Ok(())
        }
    }
}

/// One typed answer, shaped as `crates/jev` decodes it.
///
/// Every answer names the option the estimator's own distribution picked:
/// `choice` carries it on a Choice, and `selected` carries it on a Noul or
/// a Score, where the other fields cannot. A served calibration map rescales
/// that option's probability without replacing the answer, so on a
/// calibrated answer `selected` can name an option that is not the largest
/// number the answer reports. Doors that serve an uncalibrated distribution
/// leave `selected` out, and a reader derives the same pick from the
/// answer's own numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    /// A probability of yes.
    Noul {
        /// The probability of yes, from 0 to 1. It is that probability even
        /// when a served calibration map has pulled it below one half.
        noul: f64,
        /// The option `noul` was measured on: the estimator's pick before
        /// any map ran. `crates/jev` leaves the field off an answer that
        /// carries none, and the number's own implication stands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected: Option<String>,
    },
    /// One option, with the distribution behind it.
    Choice {
        /// The option that won.
        choice: String,
        /// How sharp the distribution is. Omitted, with `probabilities`, on
        /// an answer for a family no admitted calibration record covers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confidence: Option<f64>,
        /// A probability per option. Omitted for a family no admitted
        /// calibration record covers; see `docs/lev/calibration.md`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        probabilities: Option<IndexMap<String, f64>>,
    },
    /// A position on an ordered rubric.
    Score {
        /// The probability-weighted mean level.
        score: f64,
        /// How sharp the distribution is. Omitted, with `probabilities`, on
        /// an answer for a family no admitted calibration record covers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confidence: Option<f64>,
        /// The level the estimator's distribution picked before any map
        /// ran. A map can leave a runner-up numerically larger in
        /// `probabilities`; this field is then the only place the pick
        /// survives the wire.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected: Option<String>,
        /// The rubric, keyed by level index.
        legend: IndexMap<String, Value>,
        /// A probability per level index. Omitted for a family no admitted
        /// calibration record covers; see `docs/lev/calibration.md`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        probabilities: Option<IndexMap<String, f64>>,
    },
}

impl Answer {
    /// The same answer with `probabilities` and `confidence` left off.
    ///
    /// This is what a door serves for a named family that has no admitted
    /// calibration record: the typed pick stands, and the numbers behind it,
    /// which no measurement backs, do not travel. A Noul carries its one
    /// number as the answer itself and is unchanged.
    #[must_use]
    pub fn without_probabilities(self) -> Self {
        match self {
            Self::Choice { choice, .. } => Self::Choice {
                choice,
                confidence: None,
                probabilities: None,
            },
            Self::Score {
                score,
                selected,
                legend,
                ..
            } => Self::Score {
                score,
                confidence: None,
                selected,
                legend,
                probabilities: None,
            },
            noul @ Self::Noul { .. } => noul,
        }
    }

    /// The distribution this answer carries, when it carries one.
    #[must_use]
    pub fn probabilities(&self) -> Option<&IndexMap<String, f64>> {
        match self {
            Self::Choice { probabilities, .. } | Self::Score { probabilities, .. } => {
                probabilities.as_ref()
            }
            Self::Noul { .. } => None,
        }
    }
}

/// What a request cost, as far as the runtime honestly reports it.
///
/// Apple bills no tokens, and the first bridge this workspace built computed
/// `prompt.count / 4` and called it an input token count. Lev reports a field
/// only when the runtime supplies it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens the state and questions took, when the runtime says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Tokens the answers took, when the runtime says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

/// The response body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemOneResponse {
    /// The model that answered.
    pub model: String,
    /// One answer per question id.
    pub answers: IndexMap<String, Answer>,
    /// What it cost.
    pub usage: Usage,
    /// Present when the caller asked for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn choice(options: usize) -> Question {
        let mut criteria = IndexMap::new();
        for index in 0..options {
            criteria.insert(format!("option{index}"), None);
        }
        Question::Choice {
            instructions: None,
            criteria,
        }
    }

    fn request(question: Question) -> SystemOneRequest {
        let mut questions = IndexMap::new();
        questions.insert("q".to_string(), question);
        SystemOneRequest {
            state: json!("a state"),
            model: None,
            questions,
            extensions: Extensions::default(),
        }
    }

    #[test]
    fn an_empty_question_map_is_refused() {
        let request = SystemOneRequest {
            state: json!("a state"),
            model: None,
            questions: IndexMap::new(),
            extensions: Extensions::default(),
        };
        let refusal = request.validate().unwrap_err();
        assert_eq!(refusal.code, RefusalCode::InvalidRequest);
    }

    #[test]
    fn a_choice_over_the_bound_is_refused_by_code() {
        let refusal = request(choice(256)).validate().unwrap_err();
        assert_eq!(refusal.code, RefusalCode::TooManyOptions);
        assert_eq!(refusal.question.as_deref(), Some("q"));
    }

    #[test]
    fn a_one_option_choice_is_not_a_choice() {
        let refusal = request(choice(1)).validate().unwrap_err();
        assert_eq!(refusal.code, RefusalCode::InvalidRequest);
    }

    #[test]
    fn score_levels_are_bounded_at_both_ends() {
        let one = Question::Score {
            instructions: None,
            criteria: vec![None],
        };
        assert_eq!(
            request(one).validate().unwrap_err().code,
            RefusalCode::InvalidRequest
        );
        let eleven = Question::Score {
            instructions: None,
            criteria: vec![None; 11],
        };
        assert_eq!(
            request(eleven).validate().unwrap_err().code,
            RefusalCode::InvalidRequest
        );
    }

    #[test]
    fn a_blank_or_padded_option_name_is_refused() {
        let mut criteria = IndexMap::new();
        criteria.insert("  ".to_string(), None);
        criteria.insert("real".to_string(), None);
        let blank = Question::Choice {
            instructions: None,
            criteria,
        };
        assert_eq!(
            request(blank).validate().unwrap_err().code,
            RefusalCode::InvalidRequest
        );

        let mut criteria = IndexMap::new();
        criteria.insert(" padded".to_string(), None);
        criteria.insert("real".to_string(), None);
        let padded = Question::Choice {
            instructions: None,
            criteria,
        };
        assert_eq!(
            request(padded).validate().unwrap_err().code,
            RefusalCode::InvalidRequest
        );
    }

    #[test]
    fn a_question_serializes_with_its_type_tag() {
        let question = choice(2);
        let wire = serde_json::to_value(&question).unwrap();
        assert_eq!(wire["type"], "choice");
        assert!(wire["criteria"].is_object());
    }

    #[test]
    fn an_answer_carries_the_tag_jev_reads() {
        let answer = Answer::Noul {
            noul: 0.92,
            selected: None,
        };
        let wire = serde_json::to_value(&answer).unwrap();
        assert_eq!(wire, json!({"type": "noul", "noul": 0.92}));
    }

    #[test]
    fn a_calibrated_answer_names_its_selected_option_on_the_wire() {
        // A map that rescaled the picked option below one half: `noul`
        // still reads as the calibrated probability of yes, and `selected`
        // says which option that probability was measured on.
        let answer = Answer::Noul {
            noul: 0.25,
            selected: Some("yes".to_string()),
        };
        let wire = serde_json::to_value(&answer).unwrap();
        assert_eq!(
            wire,
            json!({"type": "noul", "noul": 0.25, "selected": "yes"})
        );
        let back: Answer = serde_json::from_value(wire).unwrap();
        assert_eq!(back, answer, "the selection round-trips");
    }
}
