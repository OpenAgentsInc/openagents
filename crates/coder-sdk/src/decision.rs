//! Decision hooks: System One as a hook kind (#11498,
//! `docs/coder/design/coder-sdk.md`, "Decision hooks").
//!
//! Every System One hook has one shape, declared once here:
//!
//! - **rule first**: [`DecisionHook::rule`] reads the call and answers
//!   [`Ruled::Pass`] (nothing to judge), [`Ruled::Decided`] (the rule's
//!   outcome), or [`Ruled::Unsure`] with the question set to ask;
//! - **questions**: a Decisions API request (predicate / choice / score)
//!   over the state the rule chose, through [`crate::decide::Decider`],
//!   `openagents/decide` by default: our hosted Clef first, Jev as backup;
//! - **act**: [`DecisionHook::act`] turns the answers into a
//!   [`HookOutcome`]. Thresholds live in the hook's code with their
//!   [`Costs`] beside them; a model's answer never allows anything
//!   (an `Allow` from `act` counts as no opinion, so no probability lets a
//!   call past a rule);
//! - **timing**: a deadline per call; [`Timing::Blocking`] answers on this
//!   firing, [`Timing::Async`] (monitoring) answers on the hook's next
//!   firing and never holds the session up;
//! - **fallback**: a failed or late call is [`DecisionHook::fallback`],
//!   the rule's answer or no opinion;
//! - **pinned**: the model and question-set version join the hook set
//!   digest ([`crate::digest`]);
//! - **disclosure**: the hook declares what it sends; [`DecisionHook::register`]
//!   registers it only when the session's [`Admission`] covers that, and
//!   records the refusal otherwise;
//! - **record**: every firing that judged something is one `hook-log` line
//!   with the raw probabilities, threshold, decision, model, door and
//!   latency, under the run id, for the outcome join (#11492).

use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use hook_log::{Decision, How};
use jev::decisions::{DecisionQuestion, DecisionsRequest, DecisionsResponse};
use serde::Serialize;
use serde_json::{Value, json};

use super::admission::{Admission, DisclosureRefused};
use super::decide::Decider;
use super::events::{Call, Event};
use super::outcome::{HookOutcome, Permission};
use super::registry::{Disclosure, Hook, Registry, Source, Versions};

/// The decision model: our hosted Clef first, Jev as backup.
pub const MODEL: &str = "openagents/decide";

/// What the rule says about one call.
pub enum Ruled {
    /// Nothing here to judge: no opinion, and no record.
    Pass,
    /// The rule decided; nothing is asked.
    Decided {
        outcome: HookOutcome,
        action: String,
    },
    /// The rule can't tell: ask these questions about `input`. `memo` is
    /// what the rule saw, handed to `act` (and kept in the record).
    Unsure {
        input: String,
        questions: Vec<DecisionQuestion>,
        memo: Value,
    },
}

/// What `act` made of the answers.
#[derive(Clone, Debug, PartialEq)]
pub struct Acted {
    pub outcome: HookOutcome,
    /// What the hook did, in one word: `continue`, `steer`, `deny`.
    pub action: String,
    /// The question whose probability decided it.
    pub question: String,
    pub p: Option<f64>,
    pub threshold: Option<f64>,
}

/// The written costs a hook's thresholds come from, in minutes of the
/// person's attention. Recorded with each decision.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Costs {
    /// Acting (steering, escalating, blocking) when it was wrong to:
    /// C(wrong act).
    pub wrong_act: f64,
    /// Not acting when it was needed: what a missed stall or a stale
    /// context costs, the price of not escalating.
    pub missed: f64,
}

impl Costs {
    /// The probability past which acting costs less than not acting:
    /// act when `p * missed > (1 - p) * wrong_act`.
    #[must_use]
    pub fn threshold(self) -> f64 {
        self.wrong_act / (self.wrong_act + self.missed)
    }
}

/// When the answer is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    /// The firing waits for the answer, up to the deadline.
    Blocking,
    /// The firing returns at once; the answer is the outcome of the hook's
    /// next firing.
    Async,
}

/// Where a hook's record lines go.
pub type Recorder = Arc<dyn Fn(&Decision) + Send + Sync>;

/// One decision hook.
#[derive(Clone)]
pub struct DecisionHook {
    pub id: String,
    pub event: Event,
    pub matcher: Option<String>,
    /// The question set's version: any wording change is a new version.
    pub question_set: String,
    /// The pinned model.
    pub model: String,
    /// What the hook sends to the decision model.
    pub disclosure: Disclosure,
    pub costs: Costs,
    pub timing: Timing,
    /// How long one decision may take.
    pub deadline: Duration,
    pub rule: Arc<dyn Fn(&Call) -> Ruled + Send + Sync>,
    pub act: Arc<dyn Fn(&Value, &DecisionsResponse) -> Acted + Send + Sync>,
    /// The answer when the call fails or is late.
    pub fallback: HookOutcome,
    pub decider: Arc<dyn Decider>,
    pub recorder: Recorder,
    /// Record what it would do, and do nothing: for a hook still being
    /// measured.
    pub shadow: bool,
}

impl std::fmt::Debug for DecisionHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionHook")
            .field("id", &self.id)
            .field("event", &self.event)
            .field("question_set", &self.question_set)
            .field("model", &self.model)
            .field("timing", &self.timing)
            .finish_non_exhaustive()
    }
}

/// Records to `~/.openagents/decisions/<hook>.jsonl` (`CODER_<HOOK>_LOG`).
#[must_use]
pub fn default_recorder() -> Recorder {
    Arc::new(hook_log::append_default)
}

impl DecisionHook {
    /// Registers the hook when `admission` covers what it sends.
    ///
    /// # Errors
    ///
    /// The refusal, after recording it; or a registry error as a refusal.
    pub fn register(
        self,
        registry: &mut Registry,
        admission: &Admission,
    ) -> Result<(), DisclosureRefused> {
        if let Err(refused) = admission.check(&self.id, Some(&self.disclosure)) {
            let mut line = Decision::new(&self.id, &self.question_set, "refused", How::Rule);
            line.model = Some(self.model.clone());
            line.detail = json!({"refused": refused.message, "disclosure": refused.disclosure});
            (self.recorder)(&line);
            return Err(refused);
        }
        let id = self.id.clone();
        let disclosure = self.disclosure.clone();
        registry
            .register(self.into_hook())
            .map_err(|message| DisclosureRefused {
                id,
                disclosure,
                message,
            })
    }

    /// The hook a registry runs. Prefer [`DecisionHook::register`], which
    /// checks the disclosure first.
    #[must_use]
    pub fn into_hook(self) -> Hook {
        let id = self.id.clone();
        let event = self.event;
        let matcher = self.matcher.clone();
        let versions = Versions {
            question_set: Some(self.question_set.clone()),
            policy: None,
            model: Some(self.model.clone()),
        };
        let disclosure = self.disclosure.clone();
        // The runner's timeout is past the decision's own deadline, so a
        // late decision falls back here, recorded, rather than there.
        let timeout = self.deadline + Duration::from_secs(2);
        let pending: Arc<Mutex<Option<HookOutcome>>> = Arc::default();
        let this = Arc::new(self);
        let mut hook = Hook::new(&id, event, Source::Decision, move |call: &Call| {
            Ok(match this.timing {
                Timing::Blocking => this.fire(call),
                Timing::Async => {
                    let earlier = pending
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take();
                    let (hook, call, slot) = (this.clone(), call.clone(), pending.clone());
                    std::thread::spawn(move || {
                        let outcome = hook.fire(&call);
                        if outcome != HookOutcome::none() {
                            *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(outcome);
                        }
                    });
                    earlier.unwrap_or_default()
                }
            })
        })
        .disclosing(&disclosure.sends, &disclosure.to)
        .pinned(versions)
        .timeout(timeout);
        hook.matcher = matcher;
        hook
    }

    /// One firing, blocking: the rule, then the question when the rule is
    /// unsure, recorded either way.
    #[must_use]
    pub fn fire(&self, call: &Call) -> HookOutcome {
        let (input, questions, memo) = match (self.rule)(call) {
            Ruled::Pass => return HookOutcome::none(),
            Ruled::Decided { outcome, action } => {
                let mut line = self.line(call, &action, How::Rule);
                line.detail["memo"] = Value::Null;
                (self.recorder)(&line);
                return self.obeyed(outcome);
            }
            Ruled::Unsure {
                input,
                questions,
                memo,
            } => (input, questions, memo),
        };
        let mut request = DecisionsRequest::new(self.model.clone(), input)
            .metadata("hook", self.id.clone())
            .metadata("question_set", self.question_set.clone());
        request.questions = questions;
        let started = Instant::now();
        let answered = self.ask(request);
        let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match answered {
            Ok(response) => {
                let acted = (self.act)(&memo, &response);
                let mut line = self.line(call, &acted.action, How::Decision);
                line.question.clone_from(&acted.question);
                line.p = acted.p;
                line.threshold = acted.threshold;
                line.model = Some(response.model.clone());
                line.door.clone_from(&response.openagents.door);
                line.latency_ms = Some(latency);
                line.detail["answers"] = json!(response.answers);
                line.detail["memo"] = memo;
                (self.recorder)(&line);
                self.obeyed(acted.outcome)
            }
            Err(error) => {
                let mut line = self.line(call, "fallback", How::Fallback);
                line.latency_ms = Some(latency);
                line.detail["error"] = json!(error);
                line.detail["memo"] = memo;
                (self.recorder)(&line);
                self.obeyed(self.fallback.clone())
            }
        }
    }

    /// Asks within the deadline. A late answer is discarded.
    fn ask(&self, request: DecisionsRequest) -> Result<DecisionsResponse, String> {
        let (sender, receiver) = mpsc::channel();
        let decider = self.decider.clone();
        std::thread::spawn(move || {
            let answer = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
                .and_then(|runtime| runtime.block_on(decider.decide(request)));
            let _ = sender.send(answer);
        });
        match receiver.recv_timeout(self.deadline) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
                "late: no answer within {} ms",
                self.deadline.as_millis()
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("the decision call stopped without an answer".to_owned())
            }
        }
    }

    /// What the session gets: nothing in shadow, and never an allow a
    /// decision hook can't give.
    fn obeyed(&self, mut outcome: HookOutcome) -> HookOutcome {
        if self.shadow {
            return HookOutcome::none();
        }
        if outcome.permission == Permission::Allow {
            outcome.permission = Permission::NoOpinion;
        }
        outcome.rewrite = None;
        outcome
    }

    /// The record line every firing starts from.
    fn line(&self, call: &Call, action: &str, how: How) -> Decision {
        let mut line = Decision::new(&self.id, &self.question_set, action, how);
        line.model = Some(self.model.clone());
        line.detail = json!({
            "event": call.event,
            "tool": call.tool,
            "question_set": self.question_set,
            "costs": self.costs,
            "timing": if self.timing == Timing::Async { "async" } else { "blocking" },
            "shadow": self.shadow,
        });
        line
    }
}

/// The highest-probability option of choice `name`, with its probability.
#[must_use]
pub fn top_choice(response: &DecisionsResponse, name: &str) -> Option<(String, f64)> {
    match response.answer(name)? {
        jev::decisions::DecisionAnswer::Choice {
            choice,
            probabilities,
            ..
        } => {
            let p = probabilities
                .iter()
                .find(|option| &option.value == choice)
                .map_or(0.0, |option| option.probability);
            Some((choice.clone(), p))
        }
        _ => None,
    }
}

/// The probability choice `name` gave `value`.
#[must_use]
pub fn choice_p(response: &DecisionsResponse, name: &str, value: &str) -> f64 {
    match response.answer(name) {
        Some(jev::decisions::DecisionAnswer::Choice { probabilities, .. }) => probabilities
            .iter()
            .find(|option| option.value == value)
            .map_or(0.0, |option| option.probability),
        _ => 0.0,
    }
}

#[cfg(test)]
#[path = "decision_tests.rs"]
mod tests;
