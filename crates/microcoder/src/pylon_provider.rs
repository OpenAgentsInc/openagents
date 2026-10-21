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
use pylon::router::{Decide, Deciding, Outcome, Question, Settings, route};

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
}

/// Generation on the Pylon network.
pub struct PylonGenerator {
    pub model: String,
    pub relay: String,
    buyer: Identity,
    settings: Settings,
    /// Every step's answering pylon, in order.
    answered: RefCell<Vec<Answered>>,
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
fn tokens(usage: Option<&serde_json::Value>) -> (u64, u64) {
    let Some(usage) = usage else {
        return (0, 0);
    };
    let read = |names: [&str; 2]| {
        names
            .iter()
            .find_map(|name| usage[*name].as_u64())
            .unwrap_or(0)
    };
    (
        read(["input", "prompt_tokens"]),
        read(["output", "completion_tokens"]),
    )
}

impl Generate for PylonGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let started = Instant::now();
        let system = format!("{system}{}", microcoder_loop::text_action::brief_reminder());
        let mut turns = vec![pylon::engine::Turn {
            role: "user".into(),
            content: fitted(&system, prompt, self.limit.get()),
        }];
        let mut shrunk = 0;
        // Ask again while no pylon serving the model is live, and with a
        // shorter prompt when one refuses the prompt's length, within the
        // step's budget.
        let routed = loop {
            let routed = route(
                &self.buyer,
                &self.settings,
                &NoDecision,
                &turns,
                Some(&system),
            )
            .await;
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
                free(action, model, used)
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
        assert_eq!(tokens(Some(&pylon)), (12, 3));
        assert_eq!(tokens(Some(&openai)), (7, 2));
        assert_eq!(tokens(None), (0, 0));
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
