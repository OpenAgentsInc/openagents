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

/// The question Jev answers about the task once, to choose the model that
/// writes the acceptance tests.
pub const ROUTE: &str = include_str!("../route.json");

/// The question Jev answers about each knowledge-base candidate, with
/// `{entry}` where the candidate's key goes.
pub const KNOWLEDGE: &str = include_str!("../knowledge.json");

/// The question Jev answers about each frozen test that still fails when
/// the model says the task is finished.
pub const DISPUTE: &str = include_str!("../dispute.json");

/// The question Jev answers about the finished code and each highly
/// relevant knowledge entry.
pub const CONFORM: &str = include_str!("../conform.json");

/// The question Jev answers when every frozen test passes: whether the
/// task states something no test checks.
pub const COVERAGE: &str = include_str!("../coverage.json");

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

/// The embedded routing set.
///
/// # Panics
///
/// When `route.json` isn't valid, which a test checks.
#[must_use]
pub fn route_set() -> QuestionSet {
    serde_json::from_str(ROUTE).expect("route.json is valid")
}

/// The embedded knowledge relevance set.
///
/// # Panics
///
/// When `knowledge.json` isn't valid, which a test checks.
#[must_use]
pub fn knowledge_set() -> QuestionSet {
    serde_json::from_str(KNOWLEDGE).expect("knowledge.json is valid")
}

/// The embedded dispute set: one question template, repeated per failing
/// test by [`relevance_set`].
///
/// # Panics
///
/// When `dispute.json` isn't valid, which a test checks.
#[must_use]
pub fn dispute_set() -> QuestionSet {
    serde_json::from_str(DISPUTE).expect("dispute.json is valid")
}

/// The embedded conformance set: one question template, repeated per entry
/// by [`relevance_set`].
///
/// # Panics
///
/// When `conform.json` isn't valid, which a test checks.
#[must_use]
pub fn conform_set() -> QuestionSet {
    serde_json::from_str(CONFORM).expect("conform.json is valid")
}

/// The embedded coverage set.
///
/// # Panics
///
/// When `coverage.json` isn't valid, which a test checks.
#[must_use]
pub fn coverage_set() -> QuestionSet {
    serde_json::from_str(COVERAGE).expect("coverage.json is valid")
}

/// The relevance question asked once for each of `count` candidates: the
/// question with id `entry_N` asks about the entry under the state's
/// `entry_N` key.
#[must_use]
pub fn relevance_set(template: &QuestionSet, count: usize) -> QuestionSet {
    let text = template.questions.first().map_or("", |q| q.text.as_str());
    QuestionSet {
        id: template.id.clone(),
        questions: (1..=count)
            .map(|n| Question {
                id: format!("entry_{n}"),
                text: text.replace("{entry}", &format!("entry_{n}")),
            })
            .collect(),
    }
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
    fn judge(
        &self,
        set: &QuestionSet,
        state: &Value,
    ) -> impl std::future::Future<Output = Judgment>;
}

/// The model's next action.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NextAction {
    pub rationale: String,
    pub commands: Vec<String>,
    /// Files to show in full in the next step's prompt.
    #[serde(default)]
    pub view: Vec<String>,
    /// Freeze the acceptance tests written so far.
    #[serde(default)]
    pub freeze_tests: bool,
    /// Knowledge-base entries whose full bodies to show in the next step.
    #[serde(default)]
    pub expand: Vec<String>,
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
                "description": "Bash scripts to run in order in the working directory, each written as is: never wrapped in sh -c or bash -c. Empty when finished."
            },
            "view": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Paths of files to show in full in the next step's Files section. The host reads them fresh after the commands run. A non-empty list replaces the files in view; an empty list keeps them. List every file you need to see or edit; don't cat them. At most 12."
            },
            "freeze_tests": {
                "type": "boolean",
                "description": "True once the acceptance tests are written, to freeze them after this step's commands run. They freeze once; later values are ignored."
            },
            "expand": {
                "type": "array",
                "items": {"type": "string"},
                "description": "IDs of Knowledge base entries whose full bodies to show in the next step's Knowledge base section. A non-empty list replaces the bodies shown; an empty list keeps the current ones. At most 3 are shown."
            },
            "finished": {
                "type": "boolean",
                "description": "True only when the task is complete and nothing is left to run. With acceptance tests on, the host accepts it only when every frozen test passes."
            }
        },
        "required": ["rationale", "commands", "view", "freeze_tests", "expand", "finished"],
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
}

impl Judge for JevJudge {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        let started = Instant::now();
        let mut questions = jev::Questions::new();
        for q in &set.questions {
            questions = questions.with(q.id.clone(), jev::Noul::new(q.text.clone()));
        }
        let request = jev::SystemOneRequest::new(state.clone(), questions);
        let milliseconds = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match self.client.system_one(request).await {
            Ok(response) => Judgment {
                answers: set
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

/// Generation through the operator's logged-in Codex session, with
/// Microluna's transport: one request per step, and the action is the
/// arguments of a call to the one declared tool, `next_action`.
pub struct CodexGenerator<T: microluna::Transport = microluna::codex::CodexTransport> {
    pub transport: T,
    /// The Codex model slug, such as `gpt-6-luna`.
    pub model: String,
    /// `low`, `medium`, or `high`, or `None` for the model's default.
    pub effort: Option<String>,
    /// The prompt-cache key; steps of one run share it.
    pub cache_key: String,
}

/// The one tool a Codex step declares: its parameters are the action.
#[must_use]
pub fn next_action_tool() -> Value {
    json!({
        "type": "function",
        "name": "next_action",
        "description": "Give the next step: the commands to run and why, the files to keep in view, and whether the task is finished.",
        "parameters": next_action_schema(),
        "strict": true,
    })
}

impl<T: microluna::Transport> Generate for CodexGenerator<T> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let request = microluna::Request {
            model: self.model.clone(),
            instructions: format!("{system} Reply by calling next_action exactly once."),
            input: vec![json!({
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }],
            })],
            tools: vec![next_action_tool()],
            effort: self.effort.clone(),
            cache_key: self.cache_key.clone(),
            parallel_tools: false,
        };
        let started = Instant::now();
        let mut attempt = 0u32;
        let reply = loop {
            match self.transport.respond(&request).await {
                Ok(reply) => break Ok(reply),
                Err(error) if error.transient() && attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
                }
                Err(error) => break Err(error.to_string()),
            }
        };
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match reply {
            Ok(reply) => {
                let usd = microluna::price::cost(&self.model, reply.usage).unwrap_or(0.0);
                let action = reply
                    .calls()
                    .into_iter()
                    .find(|call| call.name == "next_action")
                    .ok_or_else(|| {
                        format!(
                            "the reply called no next_action tool; it said: {}",
                            reply.text().chars().take(300).collect::<String>()
                        )
                    })
                    .and_then(|call| {
                        serde_json::from_str::<NextAction>(&call.arguments)
                            .map_err(|e| format!("next_action's arguments didn't parse: {e}"))
                    });
                Generated {
                    action,
                    model: if reply.model.is_empty() {
                        self.model.clone()
                    } else {
                        reply.model.clone()
                    },
                    prompt_tokens: reply.usage.input,
                    completion_tokens: reply.usage.output,
                    usd,
                    milliseconds,
                }
            }
            Err(error) => Generated {
                action: Err(error),
                model: self.model.clone(),
                prompt_tokens: 0,
                completion_tokens: 0,
                usd: 0.0,
                milliseconds,
            },
        }
    }
}

/// Either generator, chosen at run time.
pub enum AnyGenerator {
    Codex(CodexGenerator),
    OpenRouter(OpenRouterGenerator),
}

impl Generate for AnyGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        match self {
            AnyGenerator::Codex(g) => g.generate(system, prompt).await,
            AnyGenerator::OpenRouter(g) => g.generate(system, prompt).await,
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
    fn the_route_set_asks_whether_the_task_is_hard() {
        let set = route_set();
        assert_eq!(set.questions.len(), 1);
        assert_eq!(set.questions[0].id, "hard");
    }

    #[test]
    fn the_coverage_set_asks_one_question() {
        assert_eq!(coverage_set().questions[0].id, "uncovered");
    }

    #[test]
    fn the_conform_set_has_one_template() {
        let set = relevance_set(&conform_set(), 2);
        assert!(set.questions[0].text.contains("`entry_1`"));
    }

    #[test]
    fn the_dispute_set_has_one_template() {
        let set = relevance_set(&dispute_set(), 2);
        assert_eq!(set.questions.len(), 2);
        assert!(set.questions[1].text.contains("`entry_2`"));
    }

    #[test]
    fn the_knowledge_set_asks_one_question_per_candidate() {
        let set = relevance_set(&knowledge_set(), 3);
        assert_eq!(set.id, "openagents.microcoder.knowledge.v1");
        let ids: Vec<&str> = set.questions.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["entry_1", "entry_2", "entry_3"]);
        assert!(
            set.questions[2]
                .text
                .contains("the knowledge entry in `entry_3`")
        );
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
            json!([
                "rationale",
                "commands",
                "view",
                "freeze_tests",
                "expand",
                "finished"
            ])
        );
        assert_eq!(schema["additionalProperties"], false);
    }
}

#[cfg(test)]
mod codex_tests {
    use super::*;
    use microluna::fake::FakeTransport;

    fn call(arguments: &str) -> microluna::Reply {
        microluna::Reply {
            id: None,
            model: "gpt-6-luna".to_string(),
            items: vec![json!({
                "type": "function_call", "call_id": "c1", "name": "next_action",
                "arguments": arguments,
            })],
            usage: microluna::TokenUsage {
                input: 1_000,
                output: 100,
                ..Default::default()
            },
        }
    }

    fn generator(replies: Vec<microluna::Reply>) -> CodexGenerator<FakeTransport> {
        CodexGenerator {
            transport: FakeTransport::new(replies),
            model: "gpt-6-luna".to_string(),
            effort: Some("medium".to_string()),
            cache_key: "run".to_string(),
        }
    }

    #[tokio::test]
    async fn a_next_action_call_becomes_the_action() {
        let g = generator(vec![call(
            r#"{"rationale":"look","commands":["ls"],"view":[],"expand":[],"freeze_tests":false,"finished":false}"#,
        )]);
        let out = g.generate("system", "prompt").await;
        let action = out.action.unwrap();
        assert_eq!(action.commands, ["ls"]);
        assert_eq!((out.prompt_tokens, out.completion_tokens), (1_000, 100));
        assert!(out.usd > 0.0, "Luna has a list price");
        let sent = g.transport.requests();
        assert_eq!(sent[0].tools[0]["name"], "next_action");
        assert_eq!(sent[0].tools[0]["parameters"], next_action_schema());
        assert!(
            sent[0]
                .instructions
                .ends_with("Reply by calling next_action exactly once.")
        );
    }

    #[tokio::test]
    async fn a_reply_without_the_call_is_an_unusable_reply() {
        let mut reply = call("{}");
        reply.items =
            vec![json!({"type": "message", "content": [{"type": "output_text", "text": "hello"}]})];
        let out = generator(vec![reply]).generate("s", "p").await;
        assert!(
            out.action
                .unwrap_err()
                .contains("called no next_action tool")
        );
    }
}
