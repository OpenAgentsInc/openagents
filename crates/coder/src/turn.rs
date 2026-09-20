//! One turn, in one place: classify, route, answer, record.
//!
//! The terminal runs a turn and so does `coder --print`. A turn written
//! twice is two turns that drift, and the drift is invisible — one of them
//! would keep routing, recording, or capping in a way the other stopped
//! doing, and nothing would say so. An episode judged against a golden
//! would then be judging whichever copy the harness happened to call. So
//! the turn lives here and both callers call [`run`].
//!
//! What a caller supplies is where the events go. [`Event`] is the same
//! sequence in both modes: the classify verdict, any judgment line the
//! door emits, each shell proposal and outcome, and the reply's deltas as
//! they stream. The terminal draws them; `--print` writes the ones that
//! belong on standard error and keeps standard output for the reply.

use std::sync::Mutex;

use crate::agent::{Agent, Classified};
use crate::classify::Route;
use crate::generate::{Meta, Usage};
use crate::shell::ShellEvent;

/// What a turn reports while it runs.
pub enum Event {
    /// Classify finished; the verdict, or the note saying why it did not
    /// run.
    Classified(Classified),
    /// A remote worker's judgment feedback line (NIP-CJ).
    Judgment(String),
    /// A shell-loop event: a proposal, an outcome, or the judge's verdict.
    Shell(ShellEvent),
    /// A reply delta, as it streams.
    Delta(String),
}

/// How a turn that finished finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// The agent answered the request.
    Answered,
    /// The router declined it. The turn ran, nothing went wrong, and the
    /// answer is that there is no confident next step — which a caller
    /// should be able to tell from both an answer and a failure.
    Declined,
}

impl Completion {
    /// The word a caller reports this completion as.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Completion::Answered => "answered",
            Completion::Declined => "declined",
        }
    }
}

/// A turn that finished.
pub struct Finished {
    /// What the agent said.
    pub reply: String,
    /// What the turn cost, when the door reports it.
    pub usage: Option<Usage>,
    /// Where Classify sent the turn.
    pub route: Route,
    /// Whether the agent answered or declined.
    pub completion: Completion,
}

/// Runs one turn: fold the draft in, classify it, and answer on the route
/// Classify chose. `event` hears each phase as it happens.
///
/// # Errors
///
/// Returns the sentence the door failed with. The turn did not finish, and
/// the caller has nothing to show but the reason.
pub async fn run(
    agent: &mut Agent,
    draft: String,
    event: &mut (dyn FnMut(Event) + Send),
) -> Result<Finished, String> {
    agent.push_user(&draft);
    let classified = agent.classify().await;
    event(Event::Classified(classified.clone()));
    let route = match classified {
        Classified::Judged(verdict) => verdict.route,
        Classified::Skipped(_) => Route::Respond,
    };
    let canned = matches!(route, Route::End | Route::Halt(_));
    let completion = if matches!(route, Route::Halt(_)) {
        Completion::Declined
    } else {
        Completion::Answered
    };
    let result = match &route {
        Route::Respond | Route::Clarify => {
            // `Agent::turn` takes three sinks and they are all live at
            // once, so the caller's one callback is shared through a lock
            // rather than split into three.
            let sink = Mutex::new(event);
            agent
                .turn(
                    route == Route::Clarify,
                    &mut |delta| {
                        if let Ok(mut sink) = sink.lock() {
                            sink(Event::Delta(delta.to_string()));
                        }
                    },
                    &mut |meta| {
                        let Meta::Judgment(line) = meta;
                        if let Ok(mut sink) = sink.lock() {
                            sink(Event::Judgment(line));
                        }
                    },
                    &mut |shell| {
                        if let Ok(mut sink) = sink.lock() {
                            sink(Event::Shell(shell));
                        }
                    },
                )
                .await
                .map_err(|error| error.to_string())
        }
        Route::End => Ok(("goodbye.".to_string(), None)),
        Route::Halt(_) => Ok((
            "I don't have a confident next step for that.".to_string(),
            None,
        )),
    };
    // A canned answer never went through Generate, so nothing has recorded
    // it. The trace should still say what the user was told.
    if canned && let Ok((text, _)) = &result {
        agent.record_reply(text);
    }
    result.map(|(reply, usage)| Finished {
        reply,
        usage,
        route,
        completion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::{Door, StubGenerate};

    /// Without a classifier the turn generates unrouted, answers, and
    /// reports the classify skip on the way.
    #[tokio::test]
    async fn a_turn_without_a_classifier_answers() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        let mut skipped = false;
        let mut streamed = String::new();
        let finished = run(&mut agent, "hello".to_string(), &mut |event| match event {
            Event::Classified(Classified::Skipped(_)) => skipped = true,
            Event::Delta(delta) => streamed.push_str(&delta),
            _ => {}
        })
        .await
        .unwrap();

        assert!(skipped);
        assert_eq!(finished.reply, streamed);
        assert_eq!(finished.route, Route::Respond);
        assert_eq!(finished.completion, Completion::Answered);
    }
}
