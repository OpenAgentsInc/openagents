//! OpenAgents #12042: `judge`, typed judgments for the calls an agent that
//! coordinates other agents makes on a check-in.
//!
//! The tool asks one question set from `questions/`
//! (`jev::question_set::coordination`) over the state the agent gives it,
//! through the Decisions API (`openagents/decide`, Clef first), and returns
//! the typed answers with a receipt. The agent passes that receipt to
//! `record_decision` in `judgments`, so the decision log says which
//! judgment a decision used. Any agent may ask; nothing here depends on
//! which agent it is.

use std::time::Duration;

use futures::future::BoxFuture;
use jev::decisions::{DecisionsRequest, DecisionsResponse};
use jev::question_set::{Judgment, MODEL, coordination};
use openagents_proto::foreman::JudgmentReceipt;
use serde_json::{Value, json};

use crate::tools::ToolDef;

pub const NAME: &str = "judge";
/// How long one judgment may take.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Asks the Decisions API.
pub trait Decider: Send + Sync {
    fn decide(
        &self,
        request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>>;
}

/// The Decisions API at `OPENAGENTS_DECISIONS_URL`, else openagents.com.
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
                .unwrap_or_else(|| jev::decisions::OPENAGENTS.to_owned());
            jev::decisions::DecisionsClient::new(url)
                .map_err(|e| e.to_string())?
                .timeout(TIMEOUT)
                .create(&request)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Ask a fast decision model a typed question about the work instead of guessing. `set` picks the question: run-progress (has this run moved its work forward since the last check-in? state: run {holds, kind}, window {from, to, minutes}, status, outputs [{at, text}], commits [{at, subject, branch}], builds [{at, event}]), failure-on-main (does this branch's failing test also fail on main? state: failure {test, package, message}, main {commit, failures [{test, message}]}; ask once per failure), change-overlap (do two changes waiting to land overlap? state: a and b, each {summary, files, packages}), or ready-to-close (does what landed do what the issue's acceptance text asks? state: issue {number, title, acceptance}, landed [{commit, subject, files}], last_comment). Returns each answer (a probability, or a choice with its confidence) and a receipt; pass the receipt to record_decision in `judgments` when a decision uses it. A judgment is evidence, not permission.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "set": {
                    "type": "string",
                    "enum": coordination::names(),
                    "description": "Which question set to ask."
                },
                "state": {
                    "type": "object",
                    "description": "The facts the questions read, in the shape the set names."
                }
            },
            "required": ["set", "state"]
        }),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    set: String,
    state: Value,
}

/// The receipt `record_decision` keeps for `judgment`.
pub fn receipt(judgment: &Judgment) -> JudgmentReceipt {
    JudgmentReceipt {
        question_set: judgment.receipt.question_set.clone(),
        decision: judgment.receipt.decision.clone(),
        model: judgment.receipt.model.clone(),
        door: judgment.receipt.door.clone(),
        receipt: judgment.receipt.receipt.clone(),
        answers: judgment.summary(),
    }
}

/// Ask the set the arguments name.
pub async fn call(decider: &dyn Decider, args: Value) -> Result<Value, String> {
    let args: Args = serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))?;
    let set = coordination::set(args.set.trim()).ok_or_else(|| {
        format!(
            "unknown set {:?}; ask one of: {}",
            args.set,
            coordination::names().join(", ")
        )
    })?;
    if !args.state.is_object() {
        return Err("give `state` as an object with the fields the set reads".into());
    }
    let request = set.request(MODEL, &args.state).map_err(|e| e.to_string())?;
    let response = decider
        .decide(request)
        .await
        .map_err(|e| format!("the Decisions API didn't answer: {e}"))?;
    let judgment = Judgment::from_response(set, response).map_err(|e| e.to_string())?;
    Ok(json!({
        "questionSet": set.id,
        "gate": set.gate,
        "answers": judgment.answers,
        "receipt": receipt(&judgment),
        "next": "Pass `receipt` to record_decision in `judgments` when a decision uses this judgment.",
    }))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Answers every question with a fixed response and keeps the requests.
    #[derive(Default)]
    pub struct Fake {
        pub requests: Arc<Mutex<Vec<DecisionsRequest>>>,
        pub fail: bool,
    }

    impl Decider for Fake {
        fn decide(
            &self,
            request: DecisionsRequest,
        ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
            self.requests.lock().unwrap().push(request);
            let fail = self.fail;
            Box::pin(async move {
                if fail {
                    return Err("offline".into());
                }
                Ok(serde_json::from_value(json!({
                    "id": "dec_7",
                    "object": "decision",
                    "model": "clef-flash",
                    "answers": [
                        {"type": "predicate", "name": "progressed", "probability": 0.08},
                        {"type": "choice", "name": "activity", "choice": "stopped",
                         "probabilities": [{"value": "stopped", "probability": 0.9}],
                         "confidence": 0.81}
                    ],
                    "openagents": {"door": "clef", "latency_ms": 30}
                }))
                .unwrap())
            })
        }
    }

    #[tokio::test]
    async fn a_judgment_comes_back_typed_with_its_receipt() {
        let fake = Fake::default();
        let state = json!({"run": {"holds": "#12012"}, "window": {"from": "19:00", "to": "19:20"}, "outputs": []});
        let reply = call(&fake, json!({"set": "run-progress", "state": state}))
            .await
            .unwrap();
        assert_eq!(reply["questionSet"], "openagents.run-progress.v1");
        assert_eq!(reply["gate"], "progressed");
        assert_eq!(reply["answers"][0]["probability"], 0.08);
        assert_eq!(reply["answers"][1]["choice"], "stopped");
        let receipt: JudgmentReceipt = serde_json::from_value(reply["receipt"].clone()).unwrap();
        assert_eq!(
            receipt,
            JudgmentReceipt {
                question_set: "openagents.run-progress.v1".into(),
                decision: "dec_7".into(),
                model: "clef-flash".into(),
                door: Some("clef".into()),
                receipt: None,
                answers: "progressed 0.08, activity stopped (0.81)".into(),
            }
        );
        let requests = fake.requests.lock().unwrap();
        assert_eq!(requests[0].model, "openagents/decide");
        assert_eq!(requests[0].questions.len(), 2);
    }

    #[tokio::test]
    async fn bad_sets_states_and_silent_doors_are_plain_errors() {
        let fake = Fake::default();
        let unknown = call(&fake, json!({"set": "is-it-done", "state": {}}))
            .await
            .unwrap_err();
        assert!(unknown.contains("run-progress"), "{unknown}");
        let text = call(&fake, json!({"set": "run-progress", "state": "stalled"}))
            .await
            .unwrap_err();
        assert!(text.contains("object"), "{text}");
        assert!(fake.requests.lock().unwrap().is_empty());
        let offline = Fake {
            fail: true,
            ..Fake::default()
        };
        let error = call(
            &offline,
            json!({"set": "change-overlap", "state": {"a": {}, "b": {}}}),
        )
        .await
        .unwrap_err();
        assert!(error.contains("didn't answer"), "{error}");
    }
}
