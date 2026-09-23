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
//! sequence in both modes: the program a turn selected when it selected
//! one, the classify verdict, any judgment line the door emits, each shell
//! proposal and outcome, and the reply's deltas as they stream. The
//! terminal draws them; `--print` writes the ones that belong on standard
//! error and keeps standard output for the reply — or, under `--json`,
//! writes each of them there as an object ahead of the summary.
//!
//! # Two things a turn can be
//!
//! A turn either **runs a program** or **answers**, and the first question
//! it asks is which. [`crate::runtime`] runs a program's steps from the
//! program; this is the path from an operator's sentence to it, and it is
//! one question: which program does this request ask for, from the ones
//! this host would run, or **none**.
//!
//! Almost every turn answers `none`, and that is the point of the option
//! rather than an argument against the question. A question that can only
//! name programs has to name one, and a forced choice is how "what does
//! `ROUNDS_MAX` do" becomes a fan-out. On `none` the turn proceeds exactly
//! as it did before this existed: same classify, same route, same reply.
//!
//! The two errors here are not the same size. A missed program is a turn
//! that answers normally, which costs an operator one retry. A program
//! selected for a request that did not ask for one proposes subprocesses
//! nobody asked for — and stops there, because a selection is a proposal
//! rather than a grant: [`crate::program_authority`] is the operator's
//! answer to whether a selected program may run, and a run the grant
//! does not cover is refused before its first step.
//! `docs/decision-models/measurements/2026-09-19-program-selection.md`
//! measures them apart, against the baseline of answering `none` every
//! time.

use std::sync::Mutex;

use crate::agent::{Agent, Classified, Ending};
use crate::classify::Route;
use crate::generate::{Meta, Usage};
use crate::permit::Permit;
use crate::runtime::Run;
use crate::shell::ShellEvent;

/// What a turn reports while it runs.
pub enum Event {
    /// A program was selected, by slug. The turn runs it instead of
    /// answering.
    Program(String),
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
    /// The model's last word was a command plan the host would not run.
    /// The turn ran, the boundary held, and the reply is the host's
    /// account of what did not run and what was observed — not an answer
    /// to the request. Reported like `Declined` at the exit code and apart
    /// from it by word, because a harness that saw only the code would
    /// file a blocked action as a router decision.
    Refused,
}

impl Completion {
    /// The word a caller reports this completion as.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Completion::Answered => "answered",
            Completion::Declined => "declined",
            Completion::Refused => "refused",
        }
    }
}

/// A turn that did not finish.
///
/// The sentence is what a person reads. `cause` and `refusal` are what a
/// harness reads, and they exist because a relay that would not take the
/// job, a worker that never answered, and a worker that declined all look
/// the same in prose. `gym::eval::classify` draws the line this carries: a
/// typed refusal is an answer, a failure with no code is the harness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Why the turn did not finish, in one sentence.
    pub reason: String,
    /// The word a harness files this failure under. See
    /// [`crate::generate::GenerateError::cause`].
    pub cause: &'static str,
    /// The typed refusal code, when the failure carried one.
    pub refusal: Option<String>,
}

impl Failure {
    /// A failure the host raised rather than a door: a trace that would not
    /// open, a prompt that would not read.
    #[must_use]
    pub fn host(cause: &'static str, reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            cause,
            refusal: None,
        }
    }
}

impl From<&crate::generate::GenerateError> for Failure {
    fn from(error: &crate::generate::GenerateError) -> Self {
        Self {
            reason: error.to_string(),
            cause: error.cause(),
            refusal: error.refusal().map(str::to_string),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// A turn that finished.
pub struct Finished {
    /// What the agent said.
    pub reply: String,
    /// What the turn cost, when the door reports it.
    pub usage: Option<Usage>,
    /// What the turn cost in dollars, when the door reports it. The
    /// delegate door reports Jev and the executor together; the other
    /// doors report tokens only, so this is `None` for them.
    pub cost_usd: Option<f64>,
    /// Where Classify sent the turn, and `None` when the turn ran a
    /// program instead. A turn that runs a program takes no classify
    /// route, and reporting one it did not take would put a route in the
    /// record that nothing chose.
    pub route: Option<Route>,
    /// The program the turn ran, when it ran one.
    pub program: Option<Run>,
    /// Whether the agent answered or declined.
    pub completion: Completion,
}

/// Runs one turn: fold the draft in, ask whether it is a program request,
/// and either run the program or classify and answer on the route Classify
/// chose. `event` hears each phase as it happens.
///
/// This is also where the host decides what the turn may do to the
/// machine. [`Permit::for_route`] reads the route and the operator's
/// setting into one execution intent, and nothing the model writes
/// afterward widens it. Both the terminal and `--print` call this
/// function, so both get the same answer to that question.
///
/// # Errors
///
/// Returns the [`Failure`] the door failed with. The turn did not finish,
/// and the caller has nothing to show but the reason and its cause.
pub async fn run(
    agent: &mut Agent,
    draft: String,
    event: &mut (dyn FnMut(Event) + Send),
) -> Result<Finished, Failure> {
    agent.push_user(&draft);
    // A program request is a different turn, so it is asked first and it
    // is one question. `none` — nearly every turn — falls straight
    // through to the turn that was here before.
    let program = agent
        .program(&mut |slug| event(Event::Program(slug.to_string())))
        .await;
    if let Some(run) = program {
        return Ok(ran(run));
    }
    let classified = agent.classify().await;
    event(Event::Classified(classified.clone()));
    let route = match classified {
        Classified::Judged(verdict) => verdict.route,
        Classified::Skipped(_) => Route::Respond,
    };
    let canned = matches!(route, Route::End | Route::Halt(_));
    let mut completion = if matches!(route, Route::Halt(_)) {
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
                    Permit::for_route(&route),
                    &mut |delta| {
                        if let Ok(mut sink) = sink.lock() {
                            sink(Event::Delta(delta.to_string()));
                        }
                    },
                    &mut |meta| {
                        // The model a forwarding door names is for the
                        // trace, not the terminal: the answer is already
                        // on the screen by the time it lands.
                        let Meta::Judgment(line) = meta else {
                            return;
                        };
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
                .map(|turned| {
                    if let Ending::Refused { .. } = turned.ending {
                        completion = Completion::Refused;
                    }
                    (turned.text, turned.usage, turned.cost_usd)
                })
                .map_err(|error| Failure::from(&error))
        }
        Route::End => Ok(("goodbye.".to_string(), None, None)),
        Route::Halt(_) => Ok((
            "I don't have a confident next step for that.".to_string(),
            None,
            None,
        )),
    };
    // A canned answer never went through Generate, so nothing has recorded
    // it. The trace should still say what the user was told.
    if canned && let Ok((text, _, _)) = &result {
        agent.record_reply(text);
    }
    if let Err(failure) = &result {
        agent.record_failure(failure.cause, &failure.reason);
    }
    result.map(|(reply, usage, cost_usd)| Finished {
        reply,
        usage,
        cost_usd,
        route: Some(route),
        program: None,
        completion,
    })
}

/// The turn a program run comes to.
///
/// A program that finished answered, and one that stopped declined: the
/// run did what it was asked and the answer is that it would not go on.
/// That is the same distinction `Halt` draws, and a caller reading the
/// exit code should not have to read the summary to tell them apart.
fn ran(run: Run) -> Finished {
    // A program that handed one piece of work to one executor, such as
    // `review-runs` or `answer-question`, answered with what that executor
    // said: the reply leads with it, and the run's summary follows.
    let reply = match (run.finished(), run.delegations.as_slice()) {
        (true, [only]) if only.answered() && !only.answer().trim().is_empty() => {
            format!("{}\n\n{}", only.answer().trim(), run.summary())
        }
        _ => run.summary(),
    };
    Finished {
        reply,
        usage: None,
        cost_usd: None,
        route: None,
        completion: match run.finished() {
            true => Completion::Answered,
            false => Completion::Declined,
        },
        program: Some(run),
    }
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
        assert_eq!(finished.route, Some(Route::Respond));
        assert_eq!(finished.completion, Completion::Answered);
        assert!(
            finished.program.is_none(),
            "a machine with no decision door selects nothing and answers as it always has"
        );
    }

    /// A door whose only word is a plan the host cannot read: the turn
    /// finishes refused whatever the permit, the reply is the host's
    /// account rather than the plan, and the streamed deltas are what the
    /// model wrote — the caller replaces them with the reply, as the
    /// terminal does.
    #[tokio::test]
    async fn a_refused_plan_finishes_refused() {
        let plan = r#"{"v":2,"commands":[{"command":"ls","why":"look"}]}"#;
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::saying(plan)));
        let mut proposed = 0usize;
        let finished = run(
            &mut agent,
            "say something ambiguous".to_string(),
            &mut |event| {
                if let Event::Shell(ShellEvent::Proposed(_)) = event {
                    proposed += 1;
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(proposed, 0);
        assert_eq!(finished.completion, Completion::Refused);
        assert_eq!(finished.completion.word(), "refused");
        assert!(
            finished.reply.contains("none of them ran"),
            "{}",
            finished.reply
        );
        assert!(!finished.reply.contains(plan), "{}", finished.reply);
    }

    /// With no decision door the turn asks no selection question at all:
    /// no probe, no call, nothing to report. The ordinary path is the one
    /// a regression here would break, so it is tested rather than assumed.
    #[tokio::test]
    async fn a_turn_without_a_door_never_reaches_selection() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        let mut programs = Vec::new();
        let finished = run(
            &mut agent,
            "Delegate six instances of Devin, one for each of these six read-only questions.\n\
             - How many crates are there?\n\
             - What does ROUNDS_MAX do?"
                .to_string(),
            &mut |event| {
                if let Event::Program(slug) = event {
                    programs.push(slug);
                }
            },
        )
        .await
        .unwrap();

        assert!(programs.is_empty(), "nothing was selected: {programs:?}");
        assert_eq!(finished.completion, Completion::Answered);
        assert!(finished.program.is_none());
    }
}
