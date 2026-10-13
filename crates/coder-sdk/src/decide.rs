//! `decide`: a tool that asks the Decisions API one question for the agent
//! (`docs/coder/design/coder-sdk.md`, "Everything else", item 6).
//!
//! It sends what the agent passes it to a decision model, so it is a
//! disclosure like a decision hook's, and it passes the same check: it is
//! offered only when the session's [`Admission`] covers [`disclosure`].
//! The model is `openagents/decide` (Clef first, Jev as backup). Every
//! answer, and every failed call, is one line in hook-log's `decide` log.

use std::sync::Arc;
use std::time::{Duration, Instant};

use claude_agent_sdk::ToolResult;
use futures::future::BoxFuture;
use jev::decisions::{DecisionAnswer, DecisionsRequest, DecisionsResponse};
use serde_json::{Value, json};

use super::admission::{Admission, DisclosureRefused};
use super::registry::Disclosure;
use super::tools::Tool;

/// The tool's name.
pub const TOOL: &str = "decide";
/// The decision model: Clef first, Jev as backup.
pub const MODEL: &str = "openagents/decide";
/// The question's name in the request and the log.
const QUESTION: &str = "agent_question";
/// How long one decision may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// What `decide` sends out, and to whom.
#[must_use]
pub fn disclosure() -> Disclosure {
    Disclosure {
        sends: "the text and question the agent passes to decide".to_string(),
        to: "the Decisions API".to_string(),
    }
}

/// Asks the Decisions API.
pub trait Decider: Send + Sync {
    fn decide(
        &self,
        request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>>;
}

/// The Decisions API at `jev::decisions::OPENAGENTS`, or
/// `OPENAGENTS_DECISIONS_URL`.
pub struct Api;

impl Decider for Api {
    fn decide(
        &self,
        request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
        Box::pin(async move {
            let url = std::env::var("OPENAGENTS_DECISIONS_URL")
                .ok()
                .filter(|url| !url.trim().is_empty())
                .map_or_else(
                    || jev::decisions::OPENAGENTS.to_owned(),
                    |url| url.trim_end_matches('/').to_owned(),
                );
            let client = jev::decisions::DecisionsClient::new(url)
                .map_err(|error| error.to_string())?
                .timeout(TIMEOUT);
            client
                .create(&request)
                .await
                .map_err(|error| error.to_string())
        })
    }
}

/// The tool's arguments.
fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "input": {"type": "string", "description": "The evidence the question is about: a command, an output, a plan."},
            "question": {"type": "string", "description": "The question, in plain words."},
            "kind": {
                "type": "string",
                "enum": ["predicate", "choice", "score"],
                "description": "predicate: how likely it is true (the default); choice: which option; score: which level, lowest first."
            },
            "options": {
                "type": "array", "items": {"type": "string"},
                "description": "The options of a choice, or the levels of a score, lowest first."
            }
        },
        "required": ["input", "question"]
    })
}

const DESCRIPTION: &str = "Ask a fast decision model one question about some text and get a calibrated answer: the probability a statement is true, which of several options fits, or a score on levels you give. Use it for a judgment call you would otherwise guess at.";

/// The `decide` tool, when the admission lets it send what it sends.
///
/// # Errors
///
/// The refusal to record when the admission doesn't cover it.
pub fn tool(admission: &Admission, decider: Arc<dyn Decider>) -> Result<Tool, DisclosureRefused> {
    admission.check(TOOL, Some(&disclosure()))?;
    Ok(Tool::new(
        TOOL,
        DESCRIPTION,
        schema(),
        move |args: Value| {
            let decider = decider.clone();
            async move { Ok(ask(decider.as_ref(), &args).await) }
        },
    ))
}

/// The request for the tool's arguments.
///
/// # Errors
///
/// Missing text or question, or a choice or score with fewer than two
/// options.
pub fn request(args: &Value) -> Result<DecisionsRequest, String> {
    let text = |key: &str| {
        args[key]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let input = text("input").ok_or("Give decide the text the question is about.")?;
    let question = text("question").ok_or("Give decide a question.")?;
    let options: Vec<String> = args["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| option.as_str().map(str::to_owned))
        .collect();
    let request = DecisionsRequest::new(MODEL, input);
    match args["kind"].as_str().unwrap_or("predicate") {
        "predicate" => Ok(request.predicate(QUESTION, question)),
        kind @ ("choice" | "score") if options.len() < 2 => {
            Err(format!("A {kind} needs at least two options."))
        }
        "choice" => Ok(request.choice(
            QUESTION,
            question,
            options
                .iter()
                .map(|option| (option.clone(), option.clone())),
        )),
        "score" => Ok(request.score(
            QUESTION,
            question,
            options
                .iter()
                .map(|option| (option.clone(), option.clone())),
        )),
        other => Err(format!(
            "decide asks a predicate, a choice or a score, not {other}."
        )),
    }
}

/// Asks, records, and answers in the tool's reply shape.
async fn ask(decider: &dyn Decider, args: &Value) -> ToolResult {
    let request = match request(args) {
        Ok(request) => request,
        Err(message) => return ToolResult::error(message),
    };
    let kind = args["kind"].as_str().unwrap_or("predicate").to_owned();
    let started = Instant::now();
    let answered = decider.decide(request).await;
    let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let (reply, decision) = match answered {
        Err(error) => {
            let mut decision =
                hook_log::Decision::new(TOOL, QUESTION, "none", hook_log::How::Fallback);
            decision.latency_ms = Some(latency);
            decision.detail = json!({"kind": kind, "error": error});
            (
                ToolResult::error(format!("The decision model didn't answer: {error}")),
                decision,
            )
        }
        Ok(response) => {
            let answer = response.answer(QUESTION).cloned();
            let mut decision =
                hook_log::Decision::new(TOOL, QUESTION, "answered", hook_log::How::Decision);
            decision.model = Some(response.model.clone());
            decision.door.clone_from(&response.openagents.door);
            decision.latency_ms = Some(latency);
            let mut reply = json!({
                "kind": kind,
                "model": response.model,
                "door": response.openagents.door,
                "latency_ms": latency,
            });
            match answer {
                Some(DecisionAnswer::Predicate { probability, .. }) => {
                    decision.p = Some(probability);
                    reply["probability"] = json!(probability);
                }
                Some(DecisionAnswer::Choice {
                    choice,
                    probabilities,
                    ..
                }) => {
                    reply["choice"] = json!(choice);
                    reply["probabilities"] = json!(probabilities);
                }
                Some(DecisionAnswer::Score {
                    score,
                    probabilities,
                    ..
                }) => {
                    reply["score"] = json!(score);
                    reply["probabilities"] = json!(probabilities);
                }
                Some(DecisionAnswer::Refusal { message, .. }) => {
                    decision.action = "refused".to_owned();
                    reply["refused"] = json!(message);
                }
                None => {
                    decision.action = "no answer".to_owned();
                    reply["refused"] = json!("The model gave no answer.");
                }
            }
            decision.detail = reply.clone();
            (ToolResult::text(reply.to_string()), decision)
        }
    };
    hook_log::append_default(&decision);
    reply
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Result<DecisionsResponse, String>);

    impl Decider for Fixed {
        fn decide(
            &self,
            request: DecisionsRequest,
        ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
            assert_eq!(request.model, MODEL);
            let answer = self.0.clone();
            Box::pin(async move { answer })
        }
    }

    fn response(answer: Value) -> DecisionsResponse {
        serde_json::from_value(json!({
            "id": "dec_1", "object": "decision", "model": "clef-flash",
            "answers": [answer], "openagents": {"door": "clef", "latency_ms": 200},
        }))
        .unwrap()
    }

    fn logged(dir: &tempfile::TempDir) -> Vec<Value> {
        std::fs::read_to_string(dir.path().join("decide.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn decide_is_offered_only_when_the_admission_covers_it() {
        let decider: Arc<dyn Decider> = Arc::new(Api);
        let refused = tool(&Admission::none(), decider.clone()).unwrap_err();
        assert_eq!(refused.id, TOOL);
        assert_eq!(refused.disclosure, disclosure());
        let admitted = Admission::none().admitting(disclosure());
        assert_eq!(tool(&admitted, decider).unwrap().name, TOOL);
    }

    #[test]
    fn requests_are_built_from_the_arguments() {
        assert!(request(&json!({"question": "q"})).is_err());
        assert!(
            request(&json!({"input": "x", "question": "q", "kind": "choice", "options": ["a"]}))
                .is_err()
        );
        assert!(request(&json!({"input": "x", "question": "q", "kind": "rank"})).is_err());
        let choice = request(
            &json!({"input": "x", "question": "q", "kind": "choice", "options": ["a", "b"]}),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&choice).unwrap()["questions"][0]["type"],
            "choice"
        );
    }

    // One test owns the log variable, so parallel tests don't race on it.
    #[tokio::test]
    async fn answers_and_failures_are_replied_and_recorded() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: only this test sets this variable.
        unsafe { std::env::set_var(hook_log::log_var(TOOL), dir.path().join("decide.jsonl")) };
        let admitted = Admission::none().admitting(disclosure());
        let yes = Fixed(Ok(response(
            json!({"type": "predicate", "name": QUESTION, "probability": 0.91}),
        )));
        let decide = tool(&admitted, Arc::new(yes)).unwrap();
        let reply = decide
            .invoke(json!({"input": "rm -rf target", "question": "Is it safe?"}))
            .await
            .unwrap();
        let body: Value =
            serde_json::from_str(reply.to_value()["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["probability"], 0.91);
        assert_eq!(body["door"], "clef");

        let down = tool(&admitted, Arc::new(Fixed(Err("timed out".into())))).unwrap();
        let failed = down
            .invoke(json!({"input": "x", "question": "q"}))
            .await
            .unwrap();
        assert!(failed.is_error);

        let lines = logged(&dir);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["p"], 0.91);
        assert_eq!(lines[0]["how"], "decision");
        assert_eq!(lines[0]["model"], "clef-flash");
        assert_eq!(lines[1]["how"], "fallback");
    }

    /// The live Decisions API; run with `--ignored`.
    #[tokio::test]
    #[ignore = "calls the live Decisions API"]
    async fn live_decide_answers_a_predicate() {
        let admitted = Admission::none().admitting(disclosure());
        let decide = tool(&admitted, Arc::new(Api)).unwrap();
        let reply = decide
            .invoke(json!({
                "input": "rm -rf / --no-preserve-root",
                "question": "Does this command delete files?",
            }))
            .await
            .unwrap();
        let body: Value =
            serde_json::from_str(reply.to_value()["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(body["probability"].as_f64().unwrap() > 0.5, "{body}");
    }
}
