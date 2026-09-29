//! The doors graders call, behind traits, with fakes for tests.
//!
//! A `decision` grader asks Jev a typed question through
//! `POST /v1/systemone` ([`DecisionDoor`], live as [`JevDoor`]). A `judge`
//! grader asks the chat model door for PASS or FAIL ([`JudgeDoor`]); the
//! runner, which owns the door table and the pinned credentials, supplies
//! that one. A `receipt` grader replays a Wasm guest's invocation receipts
//! ([`Replayer`]); the runner, which holds the module bytes and the
//! recorded calls, supplies it over `plugin::replay`.
//!
//! Every door here is synchronous. A caller inside an async runtime runs
//! grading on a blocking thread.

use indexmap::IndexMap;
use serde_json::Value;

use crate::grader::DecisionQuestion;
use crate::record::Arm;

/// The question id a decision grader sends. Ids are caller-side only; the
/// model reads the instructions and criteria.
pub const QUESTION_ID: &str = "grade";

/// One answer from a decision door.
#[derive(Clone, Debug, PartialEq)]
pub enum DecisionAnswer {
    /// The probability of yes.
    Noul(f64),
    /// The option picked and a probability for each option.
    Choice {
        /// The option picked.
        choice: String,
        /// Each option's probability.
        probabilities: IndexMap<String, f64>,
    },
    /// The probability-weighted position, from 0 to the top level.
    Score(f64),
}

impl DecisionAnswer {
    /// The probability a decision grader compares with its threshold: the
    /// Noul itself, a Score's position over the top level, or a Choice's
    /// mass on the passing options.
    ///
    /// # Errors
    ///
    /// Returns why the answer does not fit the question.
    pub fn probability(&self, question: &DecisionQuestion) -> Result<f64, String> {
        let value = match (self, question) {
            (Self::Noul(value), DecisionQuestion::Noul { .. }) => *value,
            (Self::Score(score), DecisionQuestion::Score { levels, .. }) => {
                #[allow(clippy::cast_precision_loss)]
                let top = (levels.len().max(2) - 1) as f64;
                score / top
            }
            (
                Self::Choice {
                    choice,
                    probabilities,
                },
                DecisionQuestion::Choice { pass, .. },
            ) => {
                if probabilities.is_empty() {
                    f64::from(u8::from(pass.contains(choice)))
                } else {
                    pass.iter()
                        .filter_map(|option| probabilities.get(option))
                        .sum()
                }
            }
            _ => {
                return Err("the door answered a different question type than it was asked".into());
            }
        };
        if value.is_finite() && (-1e-9..=1.0 + 1e-9).contains(&value) {
            Ok(value.clamp(0.0, 1.0))
        } else {
            Err(format!(
                "the door answered {value}, which is not a probability"
            ))
        }
    }
}

/// A door that answers a typed question about a state.
pub trait DecisionDoor {
    /// The door's name, recorded in the report.
    fn describe(&self) -> String;

    /// Asks `question` about `state`, once.
    ///
    /// # Errors
    ///
    /// Returns why the door gave no answer; the grader fails with it.
    fn ask(
        &self,
        state: &Value,
        question: &DecisionQuestion,
        rubric: Option<&str>,
    ) -> Result<DecisionAnswer, String>;
}

/// A door that completes a chat prompt: the `judge` grader's door.
pub trait JudgeDoor {
    /// The door's name, recorded in the report.
    fn describe(&self) -> String;

    /// Sends one system prompt and one user prompt and returns the text.
    ///
    /// # Errors
    ///
    /// Returns why the door gave no answer; the grader fails with it.
    fn complete(&self, system: &str, user: &str) -> Result<String, String>;
}

/// Which run a replay is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunKey<'a> {
    /// The case name.
    pub case: &'a str,
    /// The arm.
    pub arm: Arm,
    /// The attempt, from 1.
    pub attempt: u32,
}

/// A replay's verdict, in the shared verification vocabulary
/// (`plugin::Replay`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayVerdict {
    /// The rerun matched the receipt's outcome and fuel.
    Passed,
    /// The rerun diverged.
    Failed {
        /// `outcome` or `fuel_consumed`.
        field: String,
        /// What the receipt recorded.
        expected: String,
        /// What the rerun produced.
        actual: String,
    },
    /// The replay could not run on the receipt's inputs.
    Unverifiable {
        /// Why.
        reason: String,
    },
}

#[cfg(feature = "plugin")]
impl From<plugin::Replay> for ReplayVerdict {
    fn from(replay: plugin::Replay) -> Self {
        match replay {
            plugin::Replay::Passed { .. } => Self::Passed,
            plugin::Replay::Failed {
                field,
                expected,
                actual,
            } => Self::Failed {
                field: field.to_string(),
                expected,
                actual,
            },
            plugin::Replay::Unverifiable { reason } => Self::Unverifiable { reason },
        }
    }
}

/// Replays the invocation receipts a run recorded for one guest operation.
pub trait Replayer {
    /// Replays every receipt `run` recorded for `operation`, in order. An
    /// empty list means the run recorded none.
    ///
    /// # Errors
    ///
    /// Returns why the receipts could not be read.
    fn replay(&self, run: RunKey<'_>, operation: &str) -> Result<Vec<ReplayVerdict>, String>;
}

/// The doors a grading pass may call. A grader whose door is absent fails
/// with that reason rather than passing unasked.
#[derive(Clone, Copy, Default)]
pub struct Doors<'a> {
    /// For `decision` graders.
    pub decision: Option<&'a dyn DecisionDoor>,
    /// For `judge` graders.
    pub judge: Option<&'a dyn JudgeDoor>,
    /// For `receipt` graders.
    pub replayer: Option<&'a dyn Replayer>,
}

/// The Jev question a decision grader asks.
#[must_use]
pub fn jev_questions(question: &DecisionQuestion, rubric: Option<&str>) -> jev::Questions {
    let asked: jev::Question = match question {
        DecisionQuestion::Noul { instructions } => match rubric {
            Some(rubric) => jev::Noul::with_criteria(
                instructions.as_str(),
                jev::NoulCriteria::new().when_true(rubric),
            )
            .into(),
            None => jev::Noul::new(instructions.as_str()).into(),
        },
        DecisionQuestion::Score {
            instructions,
            levels,
        } => jev::Score::new(
            instructions.as_str(),
            levels
                .iter()
                .map(|level| Some(jev::Entry::from(level.as_str())))
                .collect(),
        )
        .into(),
        DecisionQuestion::Choice {
            instructions,
            options,
            ..
        } => {
            let mut choice = jev::Choice {
                instructions: Some(instructions.as_str().into()),
                ..jev::Choice::default()
            };
            for (name, meaning) in options {
                choice = choice.option(name.as_str(), meaning.as_str());
            }
            choice.into()
        }
    };
    jev::Questions::new().with(QUESTION_ID, asked)
}

/// Jev, through `POST /v1/systemone`, with the SDK's blocking client.
pub struct JevDoor {
    client: jev::BlockingClient,
    model: Option<String>,
}

impl JevDoor {
    /// A door over `config`; `model` names the model a request asks, and
    /// `None` asks the client's default.
    ///
    /// # Errors
    ///
    /// Returns the SDK's configuration error, such as a missing key.
    pub fn new(config: jev::Config, model: Option<String>) -> Result<Self, String> {
        let client = jev::BlockingClient::new(config).map_err(|error| error.to_string())?;
        Ok(Self { client, model })
    }

    /// A door from `TYPESAFE_API_KEY` and the SDK's defaults.
    ///
    /// # Errors
    ///
    /// Returns the SDK's configuration error.
    pub fn from_env() -> Result<Self, String> {
        Self::new(jev::Config::new(), None)
    }
}

impl DecisionDoor for JevDoor {
    fn describe(&self) -> String {
        format!(
            "jev {} {}",
            self.client.client().base_url(),
            self.model
                .as_deref()
                .unwrap_or_else(|| self.client.client().default_model())
        )
    }

    fn ask(
        &self,
        state: &Value,
        question: &DecisionQuestion,
        rubric: Option<&str>,
    ) -> Result<DecisionAnswer, String> {
        let mut request =
            jev::SystemOneRequest::new(state.clone(), jev_questions(question, rubric));
        if let Some(model) = &self.model {
            request = request.model(model.as_str());
        }
        let response = self
            .client
            .system_one(request)
            .map_err(|error| format!("decision door: {error}"))?;
        let unread = |error: jev::Error| format!("decision door answer: {error}");
        Ok(match question {
            DecisionQuestion::Noul { .. } => {
                DecisionAnswer::Noul(response.noul(QUESTION_ID).map_err(unread)?.noul)
            }
            DecisionQuestion::Score { .. } => {
                DecisionAnswer::Score(response.score(QUESTION_ID).map_err(unread)?.score)
            }
            DecisionQuestion::Choice { .. } => {
                let answer = response.choice(QUESTION_ID).map_err(unread)?;
                DecisionAnswer::Choice {
                    choice: answer.choice.clone(),
                    probabilities: answer.probabilities.clone(),
                }
            }
        })
    }
}

/// Doors that answer from a script, for tests. No network.
pub mod fake {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, VecDeque};

    use serde_json::Value;

    use super::{DecisionAnswer, DecisionDoor, JudgeDoor, ReplayVerdict, Replayer, RunKey};
    use crate::grader::DecisionQuestion;
    use crate::record::Arm;

    type Answering = Box<dyn Fn(&Value, &DecisionQuestion) -> Result<DecisionAnswer, String>>;

    /// A decision door that answers with a function of the state, or from
    /// a queue, and counts its calls.
    pub struct FakeDecisionDoor {
        answer: Answering,
        queue: RefCell<VecDeque<Result<DecisionAnswer, String>>>,
        calls: RefCell<Vec<Value>>,
    }

    impl FakeDecisionDoor {
        /// Answers every call with `answer(state, question)`.
        pub fn answering(
            answer: impl Fn(&Value, &DecisionQuestion) -> Result<DecisionAnswer, String> + 'static,
        ) -> Self {
            Self {
                answer: Box::new(answer),
                queue: RefCell::new(VecDeque::new()),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// Answers calls from `script` in order, then fails.
        #[must_use]
        pub fn scripted(script: Vec<Result<DecisionAnswer, String>>) -> Self {
            let door = Self::answering(|_, _| Err("the script ran out".into()));
            *door.queue.borrow_mut() = script.into();
            door
        }

        /// The states the door was asked about, in order.
        #[must_use]
        pub fn calls(&self) -> Vec<Value> {
            self.calls.borrow().clone()
        }
    }

    impl DecisionDoor for FakeDecisionDoor {
        fn describe(&self) -> String {
            "fake-decision".into()
        }

        fn ask(
            &self,
            state: &Value,
            question: &DecisionQuestion,
            _rubric: Option<&str>,
        ) -> Result<DecisionAnswer, String> {
            self.calls.borrow_mut().push(state.clone());
            if let Some(next) = self.queue.borrow_mut().pop_front() {
                return next;
            }
            (self.answer)(state, question)
        }
    }

    /// A judge door that answers from a queue, then repeats its last answer.
    pub struct FakeJudgeDoor {
        queue: RefCell<VecDeque<Result<String, String>>>,
        calls: RefCell<Vec<(String, String)>>,
    }

    impl FakeJudgeDoor {
        /// Answers calls from `script` in order; the last answer repeats.
        #[must_use]
        pub fn scripted(script: Vec<Result<String, String>>) -> Self {
            Self {
                queue: RefCell::new(script.into()),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// The `(system, user)` prompts the door received.
        #[must_use]
        pub fn calls(&self) -> Vec<(String, String)> {
            self.calls.borrow().clone()
        }
    }

    impl JudgeDoor for FakeJudgeDoor {
        fn describe(&self) -> String {
            "fake-judge".into()
        }

        fn complete(&self, system: &str, user: &str) -> Result<String, String> {
            self.calls
                .borrow_mut()
                .push((system.to_string(), user.to_string()));
            let mut queue = self.queue.borrow_mut();
            match queue.len() {
                0 => Err("the script is empty".into()),
                1 => queue
                    .front()
                    .cloned()
                    .unwrap_or_else(|| Err("empty".into())),
                _ => queue.pop_front().unwrap_or_else(|| Err("empty".into())),
            }
        }
    }

    type ReplayKey = (String, &'static str, u32, String);

    /// A replayer that returns recorded verdicts per `(case, arm, attempt,
    /// operation)`, and none for anything else.
    #[derive(Default)]
    pub struct FakeReplayer {
        verdicts: BTreeMap<ReplayKey, Result<Vec<ReplayVerdict>, String>>,
    }

    impl FakeReplayer {
        /// Records what a replay of `operation` in that run returns.
        #[must_use]
        pub fn with(
            mut self,
            case: &str,
            arm: Arm,
            attempt: u32,
            operation: &str,
            verdicts: Result<Vec<ReplayVerdict>, String>,
        ) -> Self {
            self.verdicts.insert(
                (case.to_string(), arm.word(), attempt, operation.to_string()),
                verdicts,
            );
            self
        }
    }

    impl Replayer for FakeReplayer {
        fn replay(&self, run: RunKey<'_>, operation: &str) -> Result<Vec<ReplayVerdict>, String> {
            self.verdicts
                .get(&(
                    run.case.to_string(),
                    run.arm.word(),
                    run.attempt,
                    operation.to_string(),
                ))
                .cloned()
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }
}
