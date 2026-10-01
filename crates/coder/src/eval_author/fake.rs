//! Stand-ins for the model and Jev, so the interview runs with no door.
//!
//! [`StepModel`] answers each step with a fixed proposal built from the
//! state it is sent, and records the steps it was asked for. [`ScriptJudge`]
//! answers the tool question and the reply question with a function the
//! test writes.

use std::sync::Mutex;

use futures_util::future::BoxFuture;
use serde_json::{Value, json};

use crate::generate::{Generate, GenerateError, Message, Meta, Usage};
use crate::product_kb::Judge;

/// Which step an instruction asks for, by the heading
/// `ext_eval::author::prompt::task` writes.
#[must_use]
pub fn step_of(instructions: &str) -> &'static str {
    let this = instructions
        .split("## This turn: ")
        .nth(1)
        .and_then(|rest| rest.lines().next())
        .unwrap_or_default();
    match this {
        t if t.starts_with("step 1, the plugin we make") => "make",
        t if t.starts_with("step 1, the plugin") => "tool",
        t if t.starts_with("step 3") => "tests",
        t if t.starts_with("step 4") => "checks",
        t if t.starts_with("step 5") => "read",
        t if t.starts_with("fixing") => "fix",
        t if t.starts_with("the test set is ready") => "say",
        _ => "unknown",
    }
}

/// The state an instruction carries.
#[must_use]
pub fn state_of(instructions: &str) -> Value {
    instructions
        .split("## State\n\n")
        .nth(1)
        .and_then(|state| state.split("\n\n## Note").next())
        .and_then(|state| serde_json::from_str(state).ok())
        .unwrap_or(Value::Null)
}

/// A model that answers every step with a fixed, well-formed proposal.
#[derive(Default)]
pub struct StepModel {
    calls: Mutex<Vec<String>>,
    /// Steps answered with prose instead of JSON, to test the retry.
    garbled: Mutex<Vec<&'static str>>,
}

impl StepModel {
    /// A model whose first answer to each step in `steps` isn't JSON.
    #[must_use]
    pub fn garbling(steps: Vec<&'static str>) -> Self {
        Self {
            calls: Mutex::default(),
            garbled: Mutex::new(steps),
        }
    }

    /// The steps it was asked for, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// The proposal for `step` given `state`.
    #[must_use]
    pub fn answer(step: &str, state: &Value) -> Value {
        let tool = state["tool"]["name"]
            .as_str()
            .unwrap_or("the tool")
            .to_string();
        let operation = state["tool"]["operations"][0]
            .as_str()
            .unwrap_or_default()
            .to_string();
        match step {
            "make" => json!({
                "say": "Here's what we'd make: a short guide Coder follows when it writes changelog entries, plus Project map to find the change.",
                "name": "Changelog helper",
                "summary": "Writes changelog entries in your style.",
                "skill": "Write one changelog line per change, in the past tense, starting with the area it touches.",
                "uses": ["Project map"],
            }),
            "tool" => json!({
                "say": format!("{tool} gives Coder a head start. It does its one job; it leaves the answer to Coder."),
            }),
            "tests" => json!({
                "say": "Here are six tests: five where the tool should help and one where it shouldn't.",
                "tests": [
                    {"id": "largest-file", "kind": "should-fire", "task": "Create a small Rust project with three source files of different sizes, then tell us which file is largest.", "good": "It names the largest file."},
                    {"id": "where-tests-live", "kind": "should-fire", "task": "Create a Python package with a tests folder, then tell us where its tests live.", "good": "It names the tests folder."},
                    {"id": "count-languages", "kind": "should-fire", "task": "Create two Go files and one Markdown file, then count the files per language.", "good": "It counts two Go files and one Markdown file."},
                    {"id": "build-manifest", "kind": "should-fire", "task": "Create a Node project with a package.json, then say which file declares the build.", "good": "It names package.json."},
                    {"id": "names-the-tool", "kind": "should-fire", "task": format!("Use {tool} to describe the layout."), "good": "Anything."},
                    {"id": "define-idempotent", "kind": "should-not-fire", "task": "What does idempotent mean? One sentence.", "good": "A correct definition."},
                ],
            }),
            "checks" => {
                let checks: Vec<Value> = state["tests"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .map(|test| {
                        let fire = test["kind"] == "should-fire";
                        let mut graders = vec![json!({
                            "type": "decision",
                            "name": "right-answer",
                            "question": "Does the answer do what the task asked, correctly?",
                            "rubric": "The answer is correct and complete.",
                        })];
                        if !operation.is_empty() {
                            graders.push(if fire {
                                json!({"type": "operation_used", "name": "reached", "operation": operation, "min": 1})
                            } else {
                                json!({"type": "operation_used", "name": "stayed-out", "operation": operation, "min": 0, "max": 0})
                            });
                        }
                        json!({"test": test["test"], "graders": graders})
                    })
                    .collect();
                json!({"say": "Each test checks Coder's last message, and whether the tool ran.", "checks": checks})
            }
            "read" => json!({
                "say": format!(
                    "With the tool, Coder passed {} of {} tests; without it, {}. The test where the tool should stay out of the way passed both ways. We could make the easiest test harder.",
                    state["result"]["passed_with_the_tool"], state["result"]["tests"], state["result"]["passed_without_the_tool"]
                ),
            }),
            "fix" => json!({
                "say": "We removed the last test where the tool should help.",
                "tests": state["tests"].as_array().map(|tests| {
                    let mut kept: Vec<Value> = tests.iter().map(|t| json!({"id": t["test"], "kind": t["kind"], "task": t["task"]})).collect();
                    if let Some(index) = kept.iter().rposition(|t| t["kind"] == "should-fire") {
                        kept.remove(index);
                    }
                    kept
                }),
            }),
            _ => {
                json!({"say": "The test set is ready; tap Run the full test set when you want the real numbers."})
            }
        }
    }
}

impl Generate for StepModel {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        _input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let step = step_of(instructions);
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(step.to_string());
        }
        let garble = self.garbled.lock().ok().is_some_and(|mut garbled| {
            garbled
                .iter()
                .position(|s| *s == step)
                .map(|index| garbled.remove(index))
                .is_some()
        });
        let text = if garble {
            "Sure, here are some ideas for the tests.".to_string()
        } else {
            format!(
                "```json\n{}\n```",
                Self::answer(step, &state_of(instructions))
            )
        };
        sink(&text);
        Ok((text, None))
    }
}

type Answering = Box<dyn Fn(&str, &Value) -> (String, f64) + Send + Sync>;

/// A Jev stand-in: `answer(question_id, state)` gives the choice and its
/// probability; the rest of the mass is spread over the other options.
pub struct ScriptJudge {
    answer: Answering,
    asked: Mutex<Vec<(String, Value)>>,
}

impl ScriptJudge {
    /// A judge answering with `answer`.
    pub fn answering(
        answer: impl Fn(&str, &Value) -> (String, f64) + Send + Sync + 'static,
    ) -> Self {
        Self {
            answer: Box::new(answer),
            asked: Mutex::default(),
        }
    }

    /// Every question asked, with its state.
    #[must_use]
    pub fn asked(&self) -> Vec<(String, Value)> {
        self.asked.lock().map(|a| a.clone()).unwrap_or_default()
    }
}

impl Judge for ScriptJudge {
    fn judge(
        &self,
        request: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>> {
        let state = request.state.to_value();
        let mut answers = serde_json::Map::new();
        for (id, question) in request.questions.iter() {
            if let Ok(mut asked) = self.asked.lock() {
                asked.push((id.to_string(), state.clone()));
            }
            let jev::Question::Choice(choice) = question else {
                continue;
            };
            let (chosen, p) = (self.answer)(id, &state);
            let options: Vec<String> = choice.criteria.keys().cloned().collect();
            let rest = if options.len() > 1 {
                (1.0 - p) / (options.len() - 1) as f64
            } else {
                0.0
            };
            let probabilities: serde_json::Map<String, Value> = options
                .iter()
                .map(|o| (o.clone(), json!(if *o == chosen { p } else { rest })))
                .collect();
            answers.insert(
                id.to_string(),
                json!({"type": "choice", "choice": chosen, "confidence": p, "probabilities": probabilities}),
            );
        }
        let bytes = json!({"model": "jev-test", "answers": answers})
            .to_string()
            .into_bytes();
        Box::pin(async move {
            jev::SystemOneResponse::decode(jev::RawResponse {
                status: 200,
                headers: Default::default(),
                bytes,
            })
            .map_err(|e| e.to_string())
        })
    }
}
