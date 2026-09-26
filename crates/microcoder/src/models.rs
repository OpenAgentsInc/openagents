//! The two model calls a step makes: Jev judges the state, and one
//! OpenRouter call returns the next action.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Jev's published rate, in dollars per million input tokens (`jev-1.13.0`,
/// retrieved 2026-09-22). Output tokens are free.
pub const JEV_USD_PER_MILLION: f64 = 0.042;

/// Characters of state Jev reads, at most.
pub const JEV_STATE_CHARS: usize = 16_000;

/// The question set, embedded so its digest is the file's.
pub const QUESTIONS: &str = include_str!("../questions.json");

/// One question in the set.
#[derive(Clone, Debug, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
}

/// The question set.
#[derive(Clone, Debug, Deserialize)]
pub struct QuestionSet {
    pub id: String,
    pub questions: Vec<Question>,
}

/// The embedded question set.
///
/// # Panics
///
/// When `questions.json` isn't valid, which a test checks.
#[must_use]
pub fn question_set() -> QuestionSet {
    serde_json::from_str(QUESTIONS).expect("questions.json is valid")
}

/// Jev's answers for one step.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Judgment {
    /// Each question's id, its text, and the probability of yes.
    pub answers: Vec<(String, f64)>,
    pub usd: f64,
    pub milliseconds: u64,
    /// Why there are no answers, when there are none.
    pub error: Option<String>,
}

impl Judgment {
    /// The judgment as the prompt shows it.
    #[must_use]
    pub fn render(&self, set: &QuestionSet) -> String {
        if let Some(error) = &self.error {
            return format!("Jev couldn't answer this step ({error}).");
        }
        self.answers
            .iter()
            .map(|(id, p)| {
                let text = set
                    .questions
                    .iter()
                    .find(|q| &q.id == id)
                    .map_or(id.as_str(), |q| q.text.as_str());
                format!("- {id}: probability {p:.2} that the answer is yes. The question: {text}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Asks Jev about a state.
pub trait Judge {
    fn judge(&self, state: &Value) -> impl std::future::Future<Output = Judgment>;
}

/// The model's next action.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NextAction {
    pub rationale: String,
    pub commands: Vec<String>,
    pub finished: bool,
}

/// The JSON schema of [`NextAction`].
#[must_use]
pub fn next_action_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rationale": {
                "type": "string",
                "description": "Why these commands, in one or two sentences."
            },
            "commands": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Shell commands to run in order, each with sh -c in the working directory. Empty when finished."
            },
            "finished": {
                "type": "boolean",
                "description": "True only when the task is complete and nothing is left to run."
            }
        },
        "required": ["rationale", "commands", "finished"],
        "additionalProperties": false
    })
}

/// One generation's result.
#[derive(Clone, Debug, Serialize)]
pub struct Generated {
    pub action: Result<NextAction, String>,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub usd: f64,
    pub milliseconds: u64,
}

/// Returns the next action for a prompt.
pub trait Generate {
    fn generate(&self, system: &str, prompt: &str) -> impl std::future::Future<Output = Generated>;
}

/// Jev through `crates/jev`.
pub struct JevJudge {
    pub client: jev::Client,
    pub set: QuestionSet,
}

impl Judge for JevJudge {
    async fn judge(&self, state: &Value) -> Judgment {
        let started = Instant::now();
        let mut questions = jev::Questions::new();
        for q in &self.set.questions {
            questions = questions.with(q.id.clone(), jev::Noul::new(q.text.clone()));
        }
        let request = jev::SystemOneRequest::new(state.clone(), questions);
        let milliseconds = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match self.client.system_one(request).await {
            Ok(response) => Judgment {
                answers: self
                    .set
                    .questions
                    .iter()
                    .filter_map(|q| {
                        response
                            .noul(&q.id)
                            .ok()
                            .map(|answer| (q.id.clone(), answer.noul))
                    })
                    .collect(),
                usd: response.usage.input_tokens.unwrap_or(0) as f64 * JEV_USD_PER_MILLION
                    / 1_000_000.0,
                milliseconds: milliseconds(),
                error: None,
            },
            Err(error) => Judgment {
                error: Some(error.to_string()),
                milliseconds: milliseconds(),
                ..Judgment::default()
            },
        }
    }
}

/// Generation through `crates/openrouter`.
pub struct OpenRouterGenerator {
    pub client: openrouter::Client,
    pub model: String,
    /// `low`, `medium`, or `high`, or `None` for the model's default.
    pub effort: Option<String>,
}

impl Generate for OpenRouterGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let mut request = openrouter::ChatRequest::new(
            &self.model,
            vec![
                openrouter::Message::system(system),
                openrouter::Message::user(prompt),
            ],
        );
        if let Some(effort) = &self.effort {
            request = request.effort(effort);
        }
        let started = Instant::now();
        match self
            .client
            .structured::<NextAction>(request, "next_action", next_action_schema())
            .await
        {
            Ok(reply) => Generated {
                action: Ok(reply.value),
                model: reply.model,
                prompt_tokens: reply.usage.prompt_tokens,
                completion_tokens: reply.usage.completion_tokens,
                usd: reply.usage.cost.unwrap_or(0.0),
                milliseconds: reply.milliseconds,
            },
            Err(error) => {
                // A reply that misses the format still cost what it cost.
                let usage = match &error {
                    openrouter::Error::Schema { usage, .. } => usage.clone(),
                    _ => openrouter::Usage::default(),
                };
                Generated {
                    action: Err(error.to_string()),
                    model: self.model.clone(),
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    usd: usage.cost.unwrap_or(0.0),
                    milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_question_set_parses_and_names_three_questions() {
        let set = question_set();
        assert_eq!(set.id, "openagents.microcoder.judge.v1");
        let ids: Vec<&str> = set.questions.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["done", "progress", "repeating"]);
    }

    #[test]
    fn a_judgment_renders_each_answer_with_its_question() {
        let set = question_set();
        let judgment = Judgment {
            answers: vec![("done".to_string(), 0.12)],
            ..Judgment::default()
        };
        let text = judgment.render(&set);
        assert!(text.starts_with("- done: probability 0.12"));
        assert!(text.contains("is the task complete"));
    }

    #[test]
    fn the_schema_requires_every_field() {
        let schema = next_action_schema();
        assert_eq!(
            schema["required"],
            json!(["rationale", "commands", "finished"])
        );
        assert_eq!(schema["additionalProperties"], false);
    }
}
