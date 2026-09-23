//! Jev in three modes, behind one call.
//!
//! - `live` calls the service.
//! - `recorded` replays answers keyed by the digest of the state and the
//!   question set. Any change to either misses, so a recorded run can't
//!   pass off an old answer for a new question.
//! - `off` returns unknowns, to test the no-Jev fallback.
//!
//! Every request, whatever the mode, is one invocation in the recorder and,
//! when a call happened or was replayed, one decision step in the shape the
//! episode has always recorded.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use atif::document::{Decision, Step};
use jev::{Entry, Questions, SystemOneRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::credentials::{JEV_BASE_URL, JEV_MODEL};
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};

/// The schema of a recorded-answer file.
pub const RECORDED_SCHEMA: &str = "openagents.coder-one.jev-recorded.v1";

/// Where Jev's answers come from.
#[derive(Clone)]
pub enum JevMode {
    Live(jev::Client),
    Recorded(Recorded),
    Off,
}

impl JevMode {
    /// The mode as the command line and the records spell it.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            JevMode::Live(_) => "live",
            JevMode::Recorded(_) => "recorded",
            JevMode::Off => "off",
        }
    }
}

/// Recorded answers, keyed by [`key`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    pub schema: String,
    pub entries: BTreeMap<String, RecordedAnswer>,
}

/// One recorded answer set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedAnswer {
    /// The decision's name, such as `jev_probe`.
    pub name: String,
    /// The model that answered.
    pub model: String,
    /// The answers object, keyed by question ID.
    pub answers: Value,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub milliseconds: Option<u64>,
    /// Where the answer was recorded: a retained trace and call ID, or a
    /// live component run.
    pub source: String,
}

impl Recorded {
    /// Reads a recorded-answer file; a missing file is an empty set.
    ///
    /// # Errors
    ///
    /// Returns a message when the file exists but doesn't read.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Ok(Recorded::empty());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let recorded: Recorded = serde_json::from_str(&text).map_err(|error| {
            format!("{} is not a recorded-answer file: {error}", path.display())
        })?;
        if recorded.schema != RECORDED_SCHEMA {
            return Err(format!(
                "{} has schema {}, not {RECORDED_SCHEMA}",
                path.display(),
                recorded.schema
            ));
        }
        Ok(recorded)
    }

    /// An empty set.
    #[must_use]
    pub fn empty() -> Self {
        Recorded {
            schema: RECORDED_SCHEMA.to_string(),
            entries: BTreeMap::new(),
        }
    }

    /// Writes the set, keys sorted, so two writes of one set are
    /// byte-identical.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        crate::record::write_atomic(path, format!("{text}\n").as_bytes())
    }
}

/// The recorded-answer key: the digest of the state and the question set,
/// exactly as the request body carries them.
#[must_use]
pub fn key(state: &Value, questions: &Value) -> String {
    atif::digest(&json!({ "state": state, "questions": questions }))
}

/// Jev's published rate for `jev-1.13.0`, in dollars per million input
/// tokens, retrieved 2026-09-22. The same rate `episode::usage` applies.
pub const USD_PER_MILLION_INPUT: f64 = 0.042;

/// One Jev request's whole-call budget, its retries and the waits between
/// them included, unless the episode deadline leaves less. The SDK's
/// defaults, a 10-second attempt and two retries, fit inside it.
pub const JEV_CALL_BUDGET: std::time::Duration = std::time::Duration::from_secs(60);

/// Why a request the deadline refused has no answer.
pub const DEADLINE_SKIP: &str = "skipped: the episode deadline left no time";

/// What an answered request cost: priced when the response reports its
/// input tokens, which Jev bills; unknown when it does not.
#[must_use]
pub fn charge_answered(response: &jev::SystemOneResponse) -> Value {
    let priced = response.usage.input_tokens.is_some();
    json!({
        "charge": if priced { "priced" } else { "unknown" },
        "basis": if priced {
            "the response reported its input tokens"
        } else {
            "the response reported no input tokens"
        },
        "input_tokens": response.usage.input_tokens,
        "output_tokens": response.usage.output_tokens,
    })
}

/// What a failed request cost. A request refused before it was sent, or
/// answered with a 4xx refusal, served no answer: a known zero. A timeout,
/// a lost connection, a 5xx, or an answer the SDK could not read may have
/// been billed: unknown, never zero.
#[must_use]
pub fn charge_failed(error: &jev::Error) -> Value {
    let (charge, basis) = match error {
        jev::Error::Config(_) | jev::Error::Question { .. } => {
            ("zero", "refused by the SDK before it was sent".to_string())
        }
        jev::Error::Api(api) if (400..500).contains(&api.status) => (
            "zero",
            format!("refused with status {}: no answer was served", api.status),
        ),
        jev::Error::Api(api) => (
            "unknown",
            format!("status {}: the door may have done billed work", api.status),
        ),
        jev::Error::Connection { .. } | jev::Error::Timeout { .. } => (
            "unknown",
            "no response arrived: the door may have done billed work".to_string(),
        ),
        _ => (
            "unknown",
            "answered, but the answer did not read: its usage is unknown".to_string(),
        ),
    };
    json!({ "charge": charge, "basis": basis })
}

/// The charge a request with no time left records: never sent, so zero.
#[must_use]
pub fn charge_skipped() -> Value {
    json!({
        "charge": "zero",
        "basis": "not sent: the episode deadline left no time",
    })
}

/// One request to make.
pub struct Ask<'a> {
    /// The component that asks, such as `evidence.probes`.
    pub component: &'a str,
    /// The decision's name, such as `jev_probe`.
    pub name: &'a str,
    /// The decision's call ID in the trajectory.
    pub id: String,
    pub state: Value,
    pub questions: Questions,
    /// The parent invocation; the innermost open one when `None`.
    pub parent: Option<String>,
    /// The episode deadline the request is bounded by; `None` grants the
    /// full [`JEV_CALL_BUDGET`].
    pub deadline: Option<crate::deadline::Deadline>,
}

/// What a request produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The answers object, or `None` when there were none.
    pub answers: Option<Value>,
    /// Why there are no answers, when there are none.
    pub error: Option<String>,
    /// `live`, `recorded`, `miss`, `off`, `failed`, or `skipped`, when
    /// the episode deadline left no time to send it.
    pub how: &'static str,
    /// The charge the request recorded: `priced`, `zero`, or `unknown`;
    /// `None` when it made no live request.
    pub charge: Option<&'static str>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// The recorded-answer key of this request.
    pub key: String,
    /// How long the answer took: measured for a live request, as recorded
    /// for a replayed one.
    pub milliseconds: Option<u64>,
}

impl Asked {
    /// The Noul answer to question `id`, or `None` when unknown.
    #[must_use]
    pub fn noul(&self, id: &str) -> Option<f64> {
        self.answers.as_ref()?.get(id)?.get("noul")?.as_f64()
    }

    /// The Choice answer to question `id`, or `None` when unknown.
    #[must_use]
    pub fn choice(&self, id: &str) -> Option<&str> {
        self.answers.as_ref()?.get(id)?.get("choice")?.as_str()
    }

    /// Whether the request produced answers.
    #[must_use]
    pub fn answered(&self) -> bool {
        self.answers.is_some()
    }
}

/// Asks one request through `mode`, recording it as an invocation and, when
/// answers were produced or a call failed, as a decision step.
pub async fn ask(mode: &JevMode, recorder: &Recorder, ask: Ask<'_>) -> Asked {
    let request = SystemOneRequest::new(Entry::from(ask.state.clone()), ask.questions);
    let body = request
        .body(JEV_MODEL)
        .map(Value::Object)
        .unwrap_or_else(|_| json!({}));
    let state = body.get("state").cloned().unwrap_or(Value::Null);
    let questions = body.get("questions").cloned().unwrap_or(Value::Null);
    let key = key(&state, &questions);
    let invocation = recorder.begin(
        Start::new(
            ask.component,
            Implementation::new(
                ask.component,
                ask.name,
                &json!({ "model": JEV_MODEL, "questions": atif::digest(&questions) }),
            ),
        )
        .named(ask.name)
        .reading_digest(key.clone())
        .under(ask.parent.as_deref()),
    );
    let mut decision = Decision {
        id: ask.id,
        name: ask.name.to_string(),
        door: JEV_BASE_URL.to_string(),
        model: JEV_MODEL.to_string(),
        request: body,
        answers: Value::Null,
        route: None,
        error: None,
        attempts: Vec::new(),
        review: None,
        milliseconds: 0,
    };
    let credit = |step: Step| step.noting(crate::record::ATTRIBUTION_KEY, json!(invocation));
    let asked = match mode {
        JevMode::Off => Asked {
            answers: None,
            error: Some("Jev is off".to_string()),
            how: "off",
            charge: None,
            input_tokens: None,
            output_tokens: None,
            key,
            milliseconds: None,
        },
        JevMode::Recorded(recorded) => match recorded.entries.get(&key) {
            Some(entry) => {
                decision.model.clone_from(&entry.model);
                decision.answers = entry.answers.clone();
                recorder.push(credit(Step::called(decision.call()).taking(0).noting(
                    "jev_recorded",
                    json!({
                        "key": key,
                        "source": entry.source,
                        "input_tokens": entry.input_tokens,
                        "output_tokens": entry.output_tokens,
                    }),
                )));
                Asked {
                    answers: Some(entry.answers.clone()),
                    error: None,
                    how: "recorded",
                    charge: None,
                    input_tokens: entry.input_tokens,
                    output_tokens: entry.output_tokens,
                    key,
                    milliseconds: entry.milliseconds,
                }
            }
            None => Asked {
                answers: None,
                error: Some("no recorded answer for this state and question set".to_string()),
                how: "miss",
                charge: None,
                input_tokens: None,
                output_tokens: None,
                key,
                milliseconds: None,
            },
        },
        JevMode::Live(_)
            if ask
                .deadline
                .as_ref()
                .is_some_and(|deadline| deadline.grant(ask.name, JEV_CALL_BUDGET).is_none()) =>
        {
            decision.error = Some(DEADLINE_SKIP.to_string());
            recorder.push(credit(
                Step::called(decision.call()).noting("jev_usage", charge_skipped()),
            ));
            println!("  {} ▸ {DEADLINE_SKIP}", ask.name);
            Asked {
                answers: None,
                error: Some(DEADLINE_SKIP.to_string()),
                how: "skipped",
                charge: Some("zero"),
                input_tokens: None,
                output_tokens: None,
                key,
                milliseconds: None,
            }
        }
        JevMode::Live(client) => {
            // The grant above passed; this one reads what is left now.
            let budget = ask.deadline.as_ref().map_or(JEV_CALL_BUDGET, |deadline| {
                deadline
                    .allowance()
                    .map_or(JEV_CALL_BUDGET, |left| left.min(JEV_CALL_BUDGET))
                    .max(crate::deadline::MINIMUM_GRANT)
            });
            let request = request.retry(jev::RetryPolicy {
                budget: Some(budget),
                ..jev::RetryPolicy::default()
            });
            let started = Instant::now();
            let result = client.system_one(request).await;
            let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            decision.milliseconds = milliseconds;
            match result {
                Ok(response) => {
                    decision.model.clone_from(&response.model);
                    decision.answers = serde_json::from_str::<Value>(&response.raw().text())
                        .ok()
                        .and_then(|body| body.get("answers").cloned())
                        .unwrap_or(Value::Null);
                    let charge = charge_answered(&response);
                    recorder.push(credit(
                        Step::called(decision.clone().call())
                            .taking(milliseconds)
                            .noting("jev_usage", charge.clone()),
                    ));
                    Asked {
                        answers: Some(decision.answers),
                        error: None,
                        how: "live",
                        charge: Some(if charge["charge"] == "priced" {
                            "priced"
                        } else {
                            "unknown"
                        }),
                        input_tokens: response.usage.input_tokens,
                        output_tokens: response.usage.output_tokens,
                        key,
                        milliseconds: Some(milliseconds),
                    }
                }
                Err(error) => {
                    decision.error = Some(error.to_string());
                    let charge = charge_failed(&error);
                    recorder.push(credit(
                        Step::called(decision.call())
                            .taking(milliseconds)
                            .noting("jev_usage", charge.clone()),
                    ));
                    Asked {
                        answers: None,
                        error: Some(error.to_string()),
                        how: "failed",
                        charge: Some(if charge["charge"] == "zero" {
                            "zero"
                        } else {
                            "unknown"
                        }),
                        input_tokens: None,
                        output_tokens: None,
                        key,
                        milliseconds: Some(milliseconds),
                    }
                }
            }
        }
    };
    let cost = match asked.how {
        "live" => asked
            .input_tokens
            .map_or_else(Cost::unknown, |tokens| Cost {
                usd: Some(tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0),
                provenance: "price_estimate".to_string(),
            }),
        "recorded" => Cost {
            usd: Some(0.0),
            provenance: "recorded_replay".to_string(),
        },
        "off" | "miss" | "skipped" => Cost::none(),
        _ if asked.charge == Some("zero") => Cost::none(),
        _ => Cost::unknown(),
    };
    let outcome = match asked.how {
        "live" | "recorded" => Outcome::Completed,
        "off" | "skipped" => Outcome::Skipped,
        _ => Outcome::Failed,
    };
    recorder.end(
        &invocation,
        Finish::new(outcome)
            .summary(json!({
                "how": asked.how,
                "answers": asked.answers,
                "error": asked.error,
                "input_tokens": asked.input_tokens,
            }))
            .cost(cost),
    );
    asked
}

/// Records every answered request in `steps` into `recorded`, for a live
/// run that should be replayable. Returns how many were added.
pub fn record_answers(steps: &[Step], source: &str, recorded: &mut Recorded) -> usize {
    let mut added = 0;
    for step in steps {
        let Some(call) = step.call.as_ref().filter(|call| call.is_decision()) else {
            continue;
        };
        // A replayed answer is already recorded, with its provenance.
        if step.extensions.contains_key("jev_recorded") {
            continue;
        }
        if call.extra.get("error").is_some() || call.extra.get("answers").is_none_or(Value::is_null)
        {
            continue;
        }
        let state = call.arguments.get("state").cloned().unwrap_or(Value::Null);
        let questions = call
            .arguments
            .get("questions")
            .cloned()
            .unwrap_or(Value::Null);
        let usage = step.extensions.get("jev_usage");
        let entry = RecordedAnswer {
            name: call.name.clone(),
            model: call
                .extra
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(JEV_MODEL)
                .to_string(),
            answers: call.extra["answers"].clone(),
            input_tokens: usage.and_then(|usage| usage.get("input_tokens")?.as_u64()),
            output_tokens: usage.and_then(|usage| usage.get("output_tokens")?.as_u64()),
            milliseconds: Some(call.milliseconds),
            source: format!("{source}#{}", call.id),
        };
        if recorded
            .entries
            .insert(key(&state, &questions), entry)
            .is_none()
        {
            added += 1;
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev::Noul;

    fn questions(wording: &str) -> Questions {
        Questions::new().with("done", Noul::new(wording))
    }

    fn recorded(state: &Value, wording: &str) -> Recorded {
        let request = SystemOneRequest::new(Entry::from(state.clone()), questions(wording));
        let body = request.body(JEV_MODEL).unwrap();
        let mut recorded = Recorded::empty();
        recorded.entries.insert(
            key(&body["state"], &body["questions"]),
            RecordedAnswer {
                name: "jev_close".to_string(),
                model: JEV_MODEL.to_string(),
                answers: json!({ "done": { "type": "noul", "noul": 0.67 } }),
                input_tokens: Some(582),
                output_tokens: Some(20),
                milliseconds: Some(178),
                source: "test".to_string(),
            },
        );
        recorded
    }

    fn asking(state: &Value, wording: &str) -> Ask<'static> {
        Ask {
            component: "verify.close",
            name: "jev_close",
            id: "jev-1".to_string(),
            state: state.clone(),
            questions: questions(wording),
            parent: None,
            deadline: None,
        }
    }

    #[tokio::test]
    async fn a_recorded_answer_replays_for_the_same_state_and_questions() {
        let state = json!({ "issue": { "title": "t", "body": "b" } });
        let mode = JevMode::Recorded(recorded(&state, "Is it done?"));
        let recorder = Recorder::default();
        let asked = ask(&mode, &recorder, asking(&state, "Is it done?")).await;
        assert_eq!(asked.how, "recorded");
        assert_eq!(asked.noul("done"), Some(0.67));
        let invocations = crate::record::invocations(&recorder.steps());
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].outcome(), "completed");
    }

    #[tokio::test]
    async fn a_changed_question_or_state_misses_the_cache() {
        let state = json!({ "issue": { "title": "t", "body": "b" } });
        let mode = JevMode::Recorded(recorded(&state, "Is it done?"));
        let recorder = Recorder::default();
        let asked = ask(&mode, &recorder, asking(&state, "Is it finished?")).await;
        assert_eq!(asked.how, "miss");
        assert_eq!(asked.noul("done"), None);
        let other = json!({ "issue": { "title": "t", "body": "c" } });
        let asked = ask(&mode, &recorder, asking(&other, "Is it done?")).await;
        assert_eq!(asked.how, "miss");
    }

    #[tokio::test]
    async fn off_returns_unknowns() {
        let state = json!({});
        let asked = ask(
            &JevMode::Off,
            &Recorder::default(),
            asking(&state, "Is it done?"),
        )
        .await;
        assert_eq!(asked.how, "off");
        assert!(!asked.answered());
    }
}
