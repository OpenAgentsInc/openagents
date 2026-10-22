//! `microcoder --provider pylon`: each step's model call is a free
//! conversation job on the OpenAgents compute network, routed by the
//! `pylon` crate's router to a pylon or group serving the named model, as
//! `openagents pylon ask --model` routes one.
//!
//! The conversation lane carries text only, so the action comes back as
//! JSON in the reply's text: the step's system prompt, with
//! [`microcoder_loop::text_action::reminder`], is the job's instructions,
//! and the step's prompt is the one user turn. The job runs under
//! `pylon.deadline.v1` with [`DEADLINE`], since a step's prompt is longer
//! than a chat message. Nobody bills a free job, so its cost is zero
//! ([`Basis::FreeNetwork`]).
//!
//! The buyer key is new for each run unless the operator asks for the
//! Pylon home's (`--pylon-key home`, the `buyer.key` that `openagents pylon
//! ask` uses). Receipts are appended in the Pylon home either way.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use microcoder_loop::models::{Basis, Generate, Generated};
use microcoder_loop::state::cut;
use pylon::group::Want;
use pylon::identity::Identity;
use pylon::router::{Decide, Deciding, FailureKind, Outcome, Question, Routed, Settings, route};

/// The model a run asks for when none is named.
pub const MODEL: &str = "deepseek-v4.1-flash";

/// The default relay.
pub const RELAY: &str = "wss://relay.openagents.com";

/// How long one pylon has to answer a step. Answers usually take 3 to 10
/// seconds; a long prompt's prefill takes longer.
pub const DEADLINE: Duration = Duration::from_secs(300);

/// How long all the tries for one step may take.
pub const BUDGET: Duration = Duration::from_secs(600);

/// How long to wait before asking again when no pylon serving the model
/// is live or has a free slot: a group's beacon can lapse for a moment
/// while it finishes a long job, and a group serves one job at a time.
pub const RETRY_AFTER: Duration = Duration::from_secs(15);

/// The request text a step sends, at most: the job admission's 96 KiB,
/// less room for JSON escapes.
pub const MAX_TEXT_BYTES: usize = 80 * 1024;

/// The router never asks a decision for a request that names a model.
struct NoDecision;

impl Decide for NoDecision {
    fn decide<'a>(&'a self, _question: &'a Question) -> Deciding<'a> {
        Box::pin(async { Err("a named model takes no decision".to_string()) })
    }
}

/// Which pylon answered one step.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Answered {
    /// The group or pylon's label, such as `w16-g5b`.
    pub label: String,
    pub pylon_npub: String,
    pub model: String,
    pub answer_ms: Option<u64>,
    /// Usage fields returned by the group. Missing fields stay unknown.
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

/// One provider result from one Pylon router pass.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Attempt {
    /// The Microcoder generation call that requested this result.
    pub generation: usize,
    /// The router pass within that generation, including prompt-fit retries.
    pub route: usize,
    /// Requested model, or the model that answered.
    pub model: String,
    /// The group or pylon label, when the router identified one.
    pub label: Option<String>,
    /// `answered`, `failed`, `not_sent`, `busy`, `declined`, `unavailable`, or `no_provider`.
    pub result: String,
    /// The provider's reason for a failed or unavailable result.
    pub reason: Option<String>,
    /// Token counts reported for this result. Missing counts serialize as null.
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    /// Elapsed time for this router pass, shared by its candidate results.
    pub route_ms: u64,
}

/// Aggregate usage for Pylon model attempts. A total is unknown if any model
/// attempt omitted that count. Router results that sent no model request do
/// not count as model attempts.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Usage {
    pub attempts: usize,
    pub unreported_attempts: usize,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

/// Generation on the Pylon network.
pub struct PylonGenerator {
    pub model: String,
    pub relay: String,
    buyer: Identity,
    settings: Settings,
    /// Every step's answering pylon, in order.
    answered: RefCell<Vec<Answered>>,
    /// Every provider result, including refusals and unavailable providers.
    attempts: RefCell<Vec<Attempt>>,
    /// The next generation call number.
    generations: Cell<usize>,
    /// The prompt-token limit a pylon refused a step over, and the
    /// characters per token that refusal measured, once one has.
    limit: Cell<Option<Limit>>,
}

/// A prompt-token limit learned from a refusal.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Limit {
    max_tokens: u64,
    chars_per_token: f64,
}

impl Limit {
    /// The characters a whole request (instructions and prompt) may hold,
    /// with a tenth to spare for the template and the estimate.
    fn chars(self) -> usize {
        (self.max_tokens as f64 * self.chars_per_token * 0.9) as usize
    }
}

/// The token counts in a refusal such as "the prompt is 3378 tokens; this
/// group takes at most 2872".
fn refused_over(why: &str) -> Option<(u64, u64)> {
    let number = |after: &str| -> Option<u64> {
        let rest = &why[why.find(after)? + after.len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    };
    Some((number("the prompt is ")?, number("takes at most ")?))
}

impl PylonGenerator {
    /// A generator for `model` on `relay`, as `buyer`, with receipts in the
    /// Pylon home.
    #[must_use]
    pub fn new(model: &str, relay: &str, buyer: Identity) -> Self {
        let mut settings = Settings::new(relay, pylon::home());
        settings.want = Some(if model.starts_with("sha256:") {
            Want::Artifact(model.to_string())
        } else {
            Want::Model(model.to_string())
        });
        settings.try_wait = DEADLINE;
        settings.budget = BUDGET;
        settings.deadline = Some(DEADLINE);
        PylonGenerator {
            model: model.to_string(),
            relay: relay.to_string(),
            buyer,
            settings,
            answered: RefCell::new(Vec::new()),
            attempts: RefCell::new(Vec::new()),
            generations: Cell::new(0),
            limit: Cell::new(None),
        }
    }

    /// The buyer key: `fresh` (new for this run) or `home` (the Pylon
    /// home's `buyer.key`).
    ///
    /// # Errors
    ///
    /// Another name, or a home key that can't be read or made.
    pub fn buyer(which: &str) -> Result<Identity, String> {
        match which {
            "fresh" => Ok(Identity::generate()),
            "home" => Identity::load_or_create(&pylon::home().join("buyer.key")),
            other => Err(format!("--pylon-key wants fresh or home, not {other}")),
        }
    }

    /// Every step's answering pylon so far.
    #[must_use]
    pub fn answered(&self) -> Vec<Answered> {
        self.answered.borrow().clone()
    }

    /// How many steps each pylon answered, by label.
    #[must_use]
    pub fn answered_by(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for answered in self.answered.borrow().iter() {
            *counts.entry(answered.label.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Every provider result so far.
    #[must_use]
    pub fn attempts(&self) -> Vec<Attempt> {
        self.attempts.borrow().clone()
    }

    /// Pylon token totals, left unknown when any result omitted a count.
    #[must_use]
    pub fn usage(&self) -> Usage {
        let attempts = self.attempts.borrow();
        let model_attempts = attempts
            .iter()
            .filter(|attempt| matches!(attempt.result.as_str(), "answered" | "failed"))
            .collect::<Vec<_>>();
        let sum = |get: fn(&Attempt) -> Option<u64>| {
            if model_attempts.is_empty() {
                Some(0)
            } else {
                model_attempts
                    .iter()
                    .map(|attempt| get(attempt))
                    .collect::<Option<Vec<_>>>()
                    .map(|counts| counts.into_iter().sum())
            }
        };
        Usage {
            attempts: model_attempts.len(),
            unreported_attempts: model_attempts
                .iter()
                .filter(|attempt| {
                    attempt.prompt_tokens.is_none() || attempt.completion_tokens.is_none()
                })
                .count(),
            prompt_tokens: sum(|attempt| attempt.prompt_tokens),
            completion_tokens: sum(|attempt| attempt.completion_tokens),
        }
    }
}

/// `prompt` cut so it and `system` fit [`MAX_TEXT_BYTES`] and, once one
/// is known, the pylon's prompt-token `limit`. The cut keeps the start
/// (the task and the files in view) and more of the end (the latest
/// steps).
fn fitted(system: &str, prompt: &str, limit: Option<Limit>) -> String {
    let system_chars = system.chars().count();
    let mut room = MAX_TEXT_BYTES.saturating_sub(system.len()) / 4;
    if prompt.len() <= MAX_TEXT_BYTES.saturating_sub(system.len()) {
        room = prompt.chars().count();
    }
    if let Some(limit) = limit {
        room = room.min(limit.chars().saturating_sub(system_chars));
    }
    if prompt.chars().count() <= room {
        return prompt.to_string();
    }
    // The cut's own marker takes about 40 characters.
    let room = room.saturating_sub(40);
    cut(prompt, room * 2 / 5, room - room * 2 / 5)
}

/// A usage object's input and output tokens, under either naming.
fn tokens(usage: Option<&serde_json::Value>) -> (Option<u64>, Option<u64>) {
    let Some(usage) = usage else {
        return (None, None);
    };
    let read = |names: [&str; 2]| names.iter().find_map(|name| usage[*name].as_u64());
    (
        read(["input", "prompt_tokens"]),
        read(["output", "completion_tokens"]),
    )
}

fn route_attempts(
    routed: &Routed,
    model: &str,
    generation: usize,
    route: usize,
    route_ms: u64,
) -> Vec<Attempt> {
    let mut attempts = routed
        .failures
        .iter()
        .map(|failure| {
            let sent = match failure.kind {
                FailureKind::LocalRefusal => false,
                FailureKind::RequestFailed => true,
            };
            Attempt {
                generation,
                route,
                model: model.to_owned(),
                label: Some(failure.label.clone()),
                result: if sent { "failed" } else { "not_sent" }.into(),
                reason: Some(failure.reason.clone()),
                prompt_tokens: (!sent).then_some(0),
                completion_tokens: (!sent).then_some(0),
                route_ms,
            }
        })
        .collect::<Vec<_>>();
    match &routed.outcome {
        Outcome::Answered(answer) => {
            let (prompt_tokens, completion_tokens) = tokens(answer.usage.as_ref());
            attempts.push(Attempt {
                generation,
                route,
                model: if answer.model.is_empty() {
                    model.to_owned()
                } else {
                    answer.model.clone()
                },
                label: Some(answer.label.clone()),
                result: "answered".into(),
                reason: answer.error.clone(),
                prompt_tokens,
                completion_tokens,
                route_ms,
            });
        }
        Outcome::Busy | Outcome::Declined if attempts.is_empty() => {
            attempts.push(Attempt {
                generation,
                route,
                model: model.to_owned(),
                label: None,
                result: if matches!(routed.outcome, Outcome::Busy) {
                    "busy"
                } else {
                    "declined"
                }
                .into(),
                reason: None,
                prompt_tokens: None,
                completion_tokens: None,
                route_ms,
            });
        }
        Outcome::Unavailable(reason) if attempts.is_empty() => {
            attempts.push(Attempt {
                generation,
                route,
                model: model.to_owned(),
                label: None,
                result: if routed.candidates == 0 {
                    "no_provider"
                } else {
                    "unavailable"
                }
                .into(),
                reason: Some(reason.clone()),
                prompt_tokens: None,
                completion_tokens: None,
                route_ms,
            });
        }
        _ => {}
    }
    attempts
}

impl Generate for PylonGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let started = Instant::now();
        let generation = self.generations.get() + 1;
        self.generations.set(generation);
        let system = format!("{system}{}", microcoder_loop::text_action::brief_reminder());
        let mut turns = vec![pylon::engine::Turn {
            role: "user".into(),
            content: fitted(&system, prompt, self.limit.get()),
        }];
        let mut shrunk = 0;
        let mut routes = 0;
        // Ask again while no pylon serving the model is live, and with a
        // shorter prompt when one refuses the prompt's length, within the
        // step's budget.
        let routed = loop {
            routes += 1;
            let route_started = Instant::now();
            let routed = route(
                &self.buyer,
                &self.settings,
                &NoDecision,
                &turns,
                Some(&system),
            )
            .await;
            let route_ms = u64::try_from(route_started.elapsed().as_millis()).unwrap_or(u64::MAX);
            self.attempts.borrow_mut().extend(route_attempts(
                &routed,
                &self.model,
                generation,
                routes,
                route_ms,
            ));
            let over = routed.failed.iter().find_map(|(_, why)| refused_over(why));
            if let (Some((tokens, max_tokens)), Outcome::Unavailable(_)) = (over, &routed.outcome)
                && shrunk < 2
                && tokens > 0
            {
                let sent = system.chars().count() + turns[0].content.chars().count();
                let limit = Limit {
                    max_tokens,
                    chars_per_token: sent as f64 / tokens as f64,
                };
                self.limit.set(Some(limit));
                shrunk += 1;
                turns[0].content = fitted(&system, prompt, Some(limit));
                eprintln!(
                    "        pylon: the prompt was {tokens} tokens and the group takes {max_tokens}; asking again with {} characters",
                    turns[0].content.chars().count()
                );
                continue;
            }
            // No pylon serving the model is live, or every one that is had
            // no free slot.
            let busy = !routed.failed.is_empty()
                && routed
                    .failed
                    .iter()
                    .all(|(_, why)| why.contains("rate_limited"));
            let none_live = matches!(&routed.outcome, Outcome::Unavailable(_))
                && (routed.candidates == 0 || busy);
            if !none_live || started.elapsed() + RETRY_AFTER > BUDGET {
                break routed;
            }
            eprintln!(
                "        pylon: no live pylon serving {} has a free slot; asking again in {} s",
                self.model,
                RETRY_AFTER.as_secs()
            );
            tokio::time::sleep(RETRY_AFTER).await;
        };
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let free =
            |action: Result<_, String>, model: String, (input, output): (u64, u64)| Generated {
                action,
                model,
                prompt_tokens: input,
                completion_tokens: output,
                usd: Some(0.0),
                known_usd: 0.0,
                cost_unknown: None,
                usd_upper: Some(0.0),
                cost_basis: Basis::FreeNetwork,
                milliseconds,
            };
        match routed.outcome {
            Outcome::Answered(answer) => {
                let used = tokens(answer.usage.as_ref());
                eprintln!(
                    "        pylon: {} ({}) answered in {} ms",
                    answer.label,
                    answer.model,
                    answer.answer_ms.unwrap_or(milliseconds)
                );
                self.answered.borrow_mut().push(Answered {
                    label: answer.label.clone(),
                    pylon_npub: answer.pylon_npub.clone(),
                    model: answer.model.clone(),
                    answer_ms: answer.answer_ms,
                    prompt_tokens: used.0,
                    completion_tokens: used.1,
                });
                let action = match (&answer.text, &answer.error) {
                    (Some(text), _) => microcoder_loop::text_action::parse(text),
                    (None, error) => Err(format!(
                        "{} answered with no text: {}",
                        answer.label,
                        error.as_deref().unwrap_or("no reason given")
                    )),
                };
                let model = if answer.model.is_empty() {
                    self.model.clone()
                } else {
                    answer.model.clone()
                };
                free(action, model, (used.0.unwrap_or(0), used.1.unwrap_or(0)))
            }
            Outcome::Busy => free(
                Err("every pylon serving this model is busy; try again shortly".into()),
                self.model.clone(),
                (0, 0),
            ),
            Outcome::Declined => free(
                Err("the pylon router handed the step back".into()),
                self.model.clone(),
                (0, 0),
            ),
            Outcome::Unavailable(why) => {
                let tried = routed
                    .failed
                    .iter()
                    .map(|(pylon, why)| format!("{pylon}: {why}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                free(
                    Err(if tried.is_empty() {
                        format!("no pylon answered: {why}")
                    } else {
                        format!("no pylon answered: {why} ({tried})")
                    }),
                    self.model.clone(),
                    (0, 0),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_prompt_is_cut_to_fit_the_job() {
        let system = "s".repeat(10_000);
        let prompt = "p".repeat(200_000);
        let fitted = fitted(&system, &prompt, None);
        assert!(fitted.len() + system.len() <= MAX_TEXT_BYTES);
        assert_eq!(super::fitted("s", "short", None), "short");
    }

    #[test]
    fn a_refused_length_teaches_the_limit() {
        let why = "30200:abc:w16-g5b-head: limit_exceeded: the prompt is 3378 tokens; this group takes at most 2872";
        assert_eq!(refused_over(why), Some((3378, 2872)));
        assert_eq!(
            refused_over("unsupported_feature: ask with a deadline"),
            None
        );
        let limit = Limit {
            max_tokens: 2872,
            chars_per_token: 3.0,
        };
        let system = "s".repeat(3_000);
        let prompt = format!("# Task\n{}\n# Steps so far\nlatest", "p".repeat(20_000));
        let fitted = fitted(&system, &prompt, Some(limit));
        assert!(system.len() + fitted.chars().count() <= limit.chars());
        assert!(fitted.starts_with("# Task"));
        assert!(fitted.ends_with("latest"));
    }

    #[test]
    fn tokens_read_either_naming() {
        let pylon = serde_json::json!({"input": 12, "output": 3});
        let openai = serde_json::json!({"prompt_tokens": 7, "completion_tokens": 2});
        assert_eq!(tokens(Some(&pylon)), (Some(12), Some(3)));
        assert_eq!(tokens(Some(&openai)), (Some(7), Some(2)));
        assert_eq!(tokens(None), (None, None));
        assert_eq!(
            tokens(Some(&serde_json::json!({"input": 0}))),
            (Some(0), None)
        );
    }

    #[test]
    fn usage_totals_stay_unknown_when_a_result_omits_counts() {
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator.attempts.borrow_mut().extend([
            Attempt {
                generation: 1,
                route: 1,
                model: MODEL.into(),
                label: Some("group-a".into()),
                result: "answered".into(),
                reason: None,
                prompt_tokens: Some(0),
                completion_tokens: Some(4),
                route_ms: 10,
            },
            Attempt {
                generation: 2,
                route: 1,
                model: MODEL.into(),
                label: Some("group-b".into()),
                result: "answered".into(),
                reason: None,
                prompt_tokens: None,
                completion_tokens: None,
                route_ms: 11,
            },
        ]);
        let usage = generator.usage();
        assert_eq!(usage.attempts, 2);
        assert_eq!(usage.unreported_attempts, 1);
        assert_eq!(usage.prompt_tokens, None);
        assert_eq!(usage.completion_tokens, None);
        let json = serde_json::to_value(usage).unwrap();
        assert_eq!(json["prompt_tokens"], serde_json::Value::Null);
        assert_eq!(json["completion_tokens"], serde_json::Value::Null);
    }

    #[test]
    fn usage_totals_preserve_reported_zero_and_sum_complete_results() {
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator.attempts.borrow_mut().extend([
            Attempt {
                generation: 1,
                route: 1,
                model: MODEL.into(),
                label: Some("group-a".into()),
                result: "answered".into(),
                reason: None,
                prompt_tokens: Some(0),
                completion_tokens: Some(4),
                route_ms: 10,
            },
            Attempt {
                generation: 2,
                route: 1,
                model: MODEL.into(),
                label: Some("group-b".into()),
                result: "answered".into(),
                reason: None,
                prompt_tokens: Some(7),
                completion_tokens: Some(2),
                route_ms: 11,
            },
        ]);
        let usage = generator.usage();
        assert_eq!(usage.attempts, 2);
        assert_eq!(usage.unreported_attempts, 0);
        assert_eq!(usage.prompt_tokens, Some(7));
        assert_eq!(usage.completion_tokens, Some(6));
    }

    #[test]
    fn usage_totals_keep_known_fields_when_the_other_count_is_missing() {
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator.attempts.borrow_mut().push(Attempt {
            generation: 1,
            route: 1,
            model: MODEL.into(),
            label: Some("group-a".into()),
            result: "answered".into(),
            reason: None,
            prompt_tokens: Some(7),
            completion_tokens: None,
            route_ms: 10,
        });
        let usage = generator.usage();
        assert_eq!(usage.unreported_attempts, 1);
        assert_eq!(usage.prompt_tokens, Some(7));
        assert_eq!(usage.completion_tokens, None);
    }

    #[test]
    fn a_router_result_without_a_model_request_uses_zero_tokens() {
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator.attempts.borrow_mut().push(Attempt {
            generation: 1,
            route: 1,
            model: MODEL.into(),
            label: None,
            result: "no_provider".into(),
            reason: Some("no live pylon serves the requested model".into()),
            prompt_tokens: None,
            completion_tokens: None,
            route_ms: 5,
        });
        let usage = generator.usage();
        assert_eq!(usage.attempts, 0);
        assert_eq!(usage.unreported_attempts, 0);
        assert_eq!(usage.prompt_tokens, Some(0));
        assert_eq!(usage.completion_tokens, Some(0));
    }

    fn routed(failures: Vec<pylon::router::Failure>, outcome: Outcome) -> Routed {
        Routed {
            outcome,
            how: pylon::router::How::Ranked {
                why: "fixture".into(),
            },
            candidates: failures.len() + 1,
            deciders: 0,
            failed: failures
                .iter()
                .map(|f| (f.label.clone(), f.reason.clone()))
                .collect(),
            failures,
            group: None,
            sent_tokens: None,
        }
    }

    fn refusal(kind: FailureKind) -> pylon::router::Failure {
        pylon::router::Failure {
            label: "group-a".into(),
            reason: match kind {
                FailureKind::LocalRefusal => {
                    "the message is longer than this pylon accepts (32 KiB)"
                }
                FailureKind::RequestFailed => "the request timed out",
            }
            .into(),
            kind,
        }
    }

    fn answer_with_usage() -> Outcome {
        Outcome::Answered(Box::new(pylon::client::Answer {
            pylon: "group-b".into(),
            pylon_npub: String::new(),
            label: "group-b".into(),
            profile: "consumer-exact".into(),
            lossy: false,
            model: MODEL.into(),
            prompt: String::new(),
            text: Some("answer".into()),
            outcome: "answered".into(),
            error: None,
            request: String::new(),
            discover_ms: 0,
            contact_ms: None,
            answer_ms: Some(5),
            usage: Some(serde_json::json!({"input": 7, "output": 2})),
            reasoning: None,
            receipt: None,
            receipt_error: None,
            receipt_event: None,
            paid_msat: None,
            declared: None,
        }))
    }

    #[test]
    fn local_fit_refusal_is_recorded_without_unknown_model_usage() {
        let routed = routed(
            vec![refusal(FailureKind::LocalRefusal)],
            Outcome::Unavailable("no pylon answered".into()),
        );
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator
            .attempts
            .borrow_mut()
            .extend(route_attempts(&routed, MODEL, 1, 1, 5));
        assert_eq!(generator.attempts()[0].result, "not_sent");
        assert_eq!(generator.attempts()[0].prompt_tokens, Some(0));
        let usage = generator.usage();
        assert_eq!(usage.attempts, 0);
        assert_eq!(usage.unreported_attempts, 0);
        assert_eq!(
            (usage.prompt_tokens, usage.completion_tokens),
            (Some(0), Some(0))
        );
    }

    #[test]
    fn local_refusal_before_a_known_answer_preserves_known_totals() {
        let routed = routed(
            vec![refusal(FailureKind::LocalRefusal)],
            answer_with_usage(),
        );
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator
            .attempts
            .borrow_mut()
            .extend(route_attempts(&routed, MODEL, 1, 1, 5));
        let usage = generator.usage();
        assert_eq!(generator.attempts().len(), 2);
        assert_eq!(usage.attempts, 1);
        assert_eq!(usage.unreported_attempts, 0);
        assert_eq!(
            (usage.prompt_tokens, usage.completion_tokens),
            (Some(7), Some(2))
        );
    }

    #[test]
    fn an_actual_failed_request_still_makes_usage_unknown() {
        let routed = routed(
            vec![refusal(FailureKind::RequestFailed)],
            Outcome::Unavailable("no pylon answered".into()),
        );
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        generator
            .attempts
            .borrow_mut()
            .extend(route_attempts(&routed, MODEL, 1, 1, 5));
        assert_eq!(generator.attempts()[0].result, "failed");
        let usage = generator.usage();
        assert_eq!(usage.attempts, 1);
        assert_eq!(usage.unreported_attempts, 1);
        assert_eq!((usage.prompt_tokens, usage.completion_tokens), (None, None));
    }

    #[test]
    fn the_generator_wants_the_model_under_a_deadline() {
        let generator = PylonGenerator::new(MODEL, RELAY, Identity::generate());
        assert_eq!(
            generator.settings.want,
            Some(Want::Model(MODEL.to_string()))
        );
        assert_eq!(generator.settings.deadline, Some(DEADLINE));
        assert!(generator.answered_by().is_empty());
        assert!(PylonGenerator::buyer("fresh").is_ok());
        assert!(PylonGenerator::buyer("mine").is_err());
    }
}
