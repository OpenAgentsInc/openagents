//! System One hook 6 (#11491): each later chat message is routed before
//! Claude Code reads it, on a `UserPromptSubmit` decision hook
//! ([`coder_sdk::DecisionHook`]).
//!
//! One Decisions API choice (`message-route-v1`, Clef first) over the
//! chat's latest turns and the message picks a route:
//!
//! - **continue**: the session reads the message, as before;
//! - **answer from the host**: the message only asks something Coder
//!   already knows (which model runs the chat, what it has used so far).
//!   The prompt is blocked, so the executor never re-reads the session for
//!   it, and the host shows its own answer as the reply;
//! - **re-brief**: the message starts unrelated work, so the cached
//!   history no longer helps. The prompt is blocked and the host starts a
//!   new Claude Code session on the same chat with the current briefing in
//!   its system prompt and only this message, instead of re-reading the
//!   old history on every later call.
//!
//! It stays one conversation, with no modes: every route lands in the same
//! chat on screen.
//!
//! **Rule first.** The host notes each message it hands Claude Code
//! ([`note`]). No note for this prompt (a session's first message, a
//! message Coder didn't send), a message sent while a turn runs, or a
//! slash command: nothing is asked and nothing changes. Otherwise one
//! choice question.
//!
//! **Thresholds from written costs** (minutes of the person's attention):
//! a wrong host answer costs a re-ask ([`HOST_COSTS`], act at p >= 0.89);
//! a wrong re-brief loses context the agent needed ([`REBRIEF_COSTS`], act
//! at p >= 0.86). Below them, or on a failed or late call, the message
//! continues. Each firing is a `hook-log` line in
//! `~/.openagents/decisions/route.jsonl` (`CODER_ROUTE_LOG`).
//!
//! **Where it goes in.** It is the Token savers plugin's
//! [`crate::token_savers::Lever::Route`], registered with the floors in a
//! chat's Claude Code registry by [`crate::sdk_hooks::system_one`]: on
//! with the plugin, unless `CODER_ROUTE` says `off`, `shadow` (record what
//! it would do, do nothing) or `on`.
//!
//! The host reads what was decided through [`take_routed`] when the
//! blocked turn ends ([`crate::live`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::Duration;

use coder_sdk::decide::Decider;
use coder_sdk::decision::{Recorder, top_choice};
use coder_sdk::{Acted, Call, Costs, DecisionHook, Disclosure, Event, HookOutcome, Ruled, Timing};
use jev::decisions::{ChoiceOption, DecisionQuestion, DecisionsResponse};
use serde_json::{Value, json};

use crate::router::Said;

/// The hook's ID and log name.
pub const HOOK: &str = "route";
/// The question set's frozen name. A changed wording is a new version.
pub const QUESTION_SET: &str = "message-route-v1";
/// The choice's name.
pub const QUESTION: &str = "route";
/// How long a message waits for the decision before it continues. The
/// Clef door answered in 20 to 300 ms on 2026-10-13.
pub const DEADLINE: Duration = Duration::from_millis(1500);

/// A host answer that was wrong costs the person a re-ask (2 minutes); not
/// answering from the host costs an executor turn that re-reads the whole
/// session for a fact Coder had (a quarter minute). Act at
/// `2 / (2 + 0.25)`, about 0.89.
pub const HOST_COSTS: Costs = Costs {
    wrong_act: 2.0,
    missed: 0.25,
};

/// A wrong re-brief loses what the agent had read and the person must say
/// it again (3 minutes); a missed one re-reads stale history on every
/// later call of the new work (half a minute). Act at `3 / 3.5`, about
/// 0.86.
pub const REBRIEF_COSTS: Costs = Costs {
    wrong_act: 3.0,
    missed: 0.5,
};

/// The routes.
pub const CONTINUE: &str = "continue";
pub const REBRIEF: &str = "rebrief";
pub const HOST_MODEL: &str = "host_model";
pub const HOST_SPEND: &str = "host_spend";

const INSTRUCTIONS: &str = "How should Coder handle the person's latest message?";

/// What Coder knows about the chat without asking the agent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// The model's name, as the person reads it.
    pub model: Option<String>,
    /// Tokens and dollars the chat has used so far.
    pub spend: Option<(u64, f64)>,
}

/// One message Coder is about to hand Claude Code.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    /// The chat before this message: the person's messages and the replies,
    /// oldest first.
    pub earlier: Vec<Said>,
    /// The person's message, as typed.
    pub latest: String,
    /// A turn is running: the message joins it, and is not routed.
    pub midturn: bool,
    pub facts: Facts,
}

/// What the hook decided, for the host to carry out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Routed {
    /// Show this as the reply; the executor never read the message.
    Host(String),
    /// Start a new session for this message with the current briefing.
    Rebrief,
}

static NOTES: LazyLock<Mutex<HashMap<PathBuf, Note>>> = LazyLock::new(Mutex::default);
static ROUTED: LazyLock<Mutex<HashMap<PathBuf, Routed>>> = LazyLock::new(Mutex::default);

/// Records the message the chat in `cwd` is about to hand Claude Code.
pub fn note(cwd: &Path, note: Note) {
    NOTES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(cwd.to_owned(), note);
}

/// What the hook decided for the chat in `cwd`'s last blocked message.
#[must_use]
pub fn take_routed(cwd: &Path) -> Option<Routed> {
    ROUTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(cwd)
}

fn put_routed(cwd: &Path, routed: Routed) {
    ROUTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(cwd.to_owned(), routed);
}

fn take_note(cwd: &Path) -> Option<Note> {
    NOTES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(cwd)
}

/// The host's answer for `route`, from the facts in `memo`.
#[must_use]
pub fn host_answer(route: &str, memo: &Value) -> Option<String> {
    match route {
        HOST_MODEL => memo["facts"]["model"]
            .as_str()
            .map(|model| format!("This chat runs on {model}, in Claude Code.")),
        HOST_SPEND => {
            let tokens = memo["facts"]["tokens"].as_u64()?;
            let dollars = memo["facts"]["cost_usd"].as_f64().unwrap_or(0.0);
            Some(if dollars > 0.0 {
                format!("This chat has used {tokens} tokens so far, about ${dollars:.2}.")
            } else {
                format!("This chat has used {tokens} tokens so far.")
            })
        }
        _ => None,
    }
}

/// The choice's options for what the host knows.
fn options(facts: &Facts) -> Vec<(&'static str, &'static str)> {
    let mut options = vec![
        (
            CONTINUE,
            "The message belongs with what the chat is doing: it continues, corrects, answers, asks about or reacts to it, or needs what the agent already read or did.",
        ),
        (
            REBRIEF,
            "The message starts new work unrelated to everything earlier in the chat: nothing the agent read or did so far helps with it.",
        ),
    ];
    if facts.model.is_some() {
        options.push((
            HOST_MODEL,
            "The message only asks which AI model or engine is answering in this chat, nothing about the work.",
        ));
    }
    if facts.spend.is_some() {
        options.push((
            HOST_SPEND,
            "The message only asks how much this chat has cost or how many tokens it has used, nothing about the work.",
        ));
    }
    options
}

/// The rule: what to ask about `prompt`, given the host's note for it.
#[must_use]
pub fn rule(note: Option<Note>, prompt: &str) -> Ruled {
    let Some(note) = note else {
        return Ruled::Pass;
    };
    let latest = note.latest.trim();
    // The note must be this prompt's: Coder may put a briefing or a model
    // switch ahead of the person's words, never after.
    if latest.is_empty() || !prompt.trim_end().ends_with(latest) {
        return Ruled::Pass;
    }
    if note.midturn || note.earlier.is_empty() || latest.starts_with('/') {
        return Ruled::Pass;
    }
    let choices = options(&note.facts)
        .into_iter()
        .map(|(value, description)| ChoiceOption {
            value: value.to_owned(),
            description: Some(description.to_owned()),
        })
        .collect();
    Ruled::Unsure {
        input: crate::router::state(&note.earlier, latest),
        questions: vec![DecisionQuestion::Choice {
            name: QUESTION.to_owned(),
            instructions: INSTRUCTIONS.to_owned(),
            choices,
        }],
        memo: json!({
            "facts": {
                "model": note.facts.model,
                "tokens": note.facts.spend.map(|(tokens, _)| tokens),
                "cost_usd": note.facts.spend.map(|(_, dollars)| dollars),
            },
            "earlier_turns": note.earlier.len(),
        }),
    }
}

/// The answers to an action: the top route, kept only past its threshold.
#[must_use]
pub fn act(memo: &Value, response: &DecisionsResponse) -> Acted {
    let continued = |p: Option<f64>, threshold: Option<f64>| Acted {
        outcome: HookOutcome::none(),
        action: CONTINUE.to_owned(),
        question: QUESTION.to_owned(),
        p,
        threshold,
    };
    let Some((route, p)) = top_choice(response, QUESTION) else {
        return continued(None, None);
    };
    let threshold = match route.as_str() {
        REBRIEF => REBRIEF_COSTS.threshold(),
        HOST_MODEL | HOST_SPEND => HOST_COSTS.threshold(),
        _ => return continued(Some(p), None),
    };
    if p < threshold {
        return continued(Some(p), Some(threshold));
    }
    let reason = if route == REBRIEF {
        "Coder is starting a new session for this message, with a fresh briefing.".to_owned()
    } else {
        match host_answer(&route, memo) {
            Some(answer) => answer,
            None => return continued(Some(p), Some(threshold)),
        }
    };
    Acted {
        outcome: HookOutcome::deny(&reason),
        action: route,
        question: QUESTION.to_owned(),
        p: Some(p),
        threshold: Some(threshold),
    }
}

/// What a routed action leaves for the host.
#[must_use]
pub fn routed(acted: &Acted, memo: &Value) -> Option<Routed> {
    match acted.action.as_str() {
        REBRIEF => Some(Routed::Rebrief),
        HOST_MODEL | HOST_SPEND => host_answer(&acted.action, memo).map(Routed::Host),
        _ => None,
    }
}

/// What the hook sends out.
#[must_use]
pub fn disclosure() -> Disclosure {
    Disclosure {
        sends: "the person's latest message and the chat's latest turns".to_owned(),
        to: "the Decisions API".to_owned(),
    }
}

/// The hook for the chat in `cwd`, asking `decider` and recording to
/// `recorder`. In `shadow` it records what it would do and does nothing.
#[must_use]
pub fn hook(
    cwd: PathBuf,
    decider: Arc<dyn Decider>,
    recorder: Recorder,
    shadow: bool,
) -> DecisionHook {
    let rule_cwd = cwd.clone();
    DecisionHook {
        id: HOOK.to_owned(),
        event: Event::UserPromptSubmit,
        matcher: None,
        question_set: QUESTION_SET.to_owned(),
        model: coder_sdk::decision::MODEL.to_owned(),
        disclosure: disclosure(),
        costs: REBRIEF_COSTS,
        timing: Timing::Blocking,
        deadline: DEADLINE,
        rule: Arc::new(move |call: &Call| {
            let prompt = call.input["prompt"].as_str().unwrap_or_default();
            rule(take_note(&rule_cwd), prompt)
        }),
        act: Arc::new(move |memo: &Value, response: &DecisionsResponse| {
            let acted = act(memo, response);
            if !shadow && let Some(routed) = routed(&acted, memo) {
                put_routed(&cwd, routed);
            }
            acted
        }),
        fallback: HookOutcome::none(),
        decider,
        recorder,
        shadow,
    }
}

#[path = "message_route_replay.rs"]
pub mod replay;

#[cfg(test)]
#[path = "message_route_tests.rs"]
mod tests;
