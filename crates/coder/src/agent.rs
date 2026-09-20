//! The agent: a turn of conversation, Classify first, Generate on route.
//!
//! A turn runs in two phases so the terminal can draw the judgment inline
//! before the answer streams: [`Agent::classify`] reads the state and
//! returns a [`Verdict`] — the judgment plus the route it produced — and
//! [`Agent::reply`] runs Generate when the route says to. The transcript
//! folds each side in as it lands.

use std::env;
use std::path::Path;
use std::time::Instant;

use atif::Decision;
use jev::SystemOneRequest;
use serde_json::Value;

use crate::classify::{
    Judgment, Route, ShellRoute, judgment_of, questions, route, shell_questions, shell_verdict_of,
    state_of,
};
use crate::generate::{Door, Generate, GenerateError, Message, Meta, Role, Usage};
use crate::repo::Repo;
use crate::shell::{self, Outcome, ShellEvent};
use crate::trace::{Recorder, answers_value};

/// The instructions Generate hears for a plain answer.
pub const INSTRUCTIONS: &str = "You are Coder, an assistant that lives in a terminal. \
    Answer directly and tersely. Plain prose, short paragraphs, no headers. \
    You can run shell commands on the user's machine: when a request needs \
    the repository inspected, code searched, tests run, or anything checked \
    on disk, do not answer from memory — emit a command plan instead of \
    prose. A plan is the whole reply as one fenced code block holding one \
    JSON object, nothing before or after the fence: \
    ```json\n{\"v\":1,\"commands\":[{\"command\":\"git grep -rn foo .\",\"why\":\"find foo\"}]}\n```. \
    Write it as ordinary reply text inside the fence: this environment \
    declares no functions, so never emit a function call or tool call. \
    At most 10 commands; prefer read-only ones unless the task asks for a \
    change. After they run you receive their output; then plan again or \
    answer. The REPO CONTEXT block describes the repository the user is \
    working in: answer project questions from it and name real paths, and \
    if the context does not cover the question, say so rather than guessing.";

/// The instructions for the turn's last word: the loop is done, prose only.
const FINAL_SUFFIX: &str = " The command loop is finished — do not emit a \
    plan; answer with what you have.";

/// The instructions for a clarifying turn: the router marked the request
/// ambiguous, so the whole reply is the question.
const CLARIFY_SUFFIX: &str = " The router marked this turn ambiguous: ask \
    one short clarifying question and nothing else.";

/// The read Classify made and where it sent the turn, for the transcript's
/// inline display.
#[derive(Clone, Debug)]
pub struct Verdict {
    /// The typed answers, kept whole.
    pub judgment: Judgment,
    /// The route the table chose.
    pub route: Route,
}

/// What classify produced.
#[derive(Clone, Debug)]
pub enum Classified {
    /// Classify ran; the verdict carries the judgment and the route.
    Judged(Verdict),
    /// Classify did not run — no key or a failed call — and the note says
    /// why. The conversation still generates, unrouted.
    Skipped(String),
}

/// The conversation: a transcript, a classifier that may be absent, and a
/// door to Generate.
pub struct Agent {
    classify: Option<jev::Client>,
    generate: Door,
    transcript: Vec<Message>,
    /// The repo the shell sits in, when it sits in one.
    repo: Option<Repo>,
    /// The draft the current turn is classifying, kept until the reply
    /// lands.
    task: String,
    /// The session's trace, when this machine is recording one.
    trace: Option<Recorder>,
    /// Why there is no trace, when there should have been one.
    trace_error: Option<String>,
}

impl Agent {
    /// An agent from the environment: `TYPESAFE_API_KEY` builds the
    /// classifier, the door builds itself. A missing key degrades — the
    /// conversation still runs, without judgments.
    ///
    /// The session's trace opens here, so a conversation is recorded without
    /// anybody asking it to be. A trace that cannot be opened is reported
    /// through [`Agent::trace_error`] and costs the conversation nothing.
    pub fn from_env() -> Self {
        let generate = Door::from_env();
        let repo = Repo::discover(&env::current_dir().unwrap_or_default());
        let where_it_ran = repo
            .as_ref()
            .map(|repo| repo.root().display().to_string())
            .unwrap_or_default();
        let (trace, trace_error) =
            match Recorder::start(generate.model(), generate.name(), &where_it_ran) {
                Ok(recorder) => (recorder, None),
                Err(error) => (None, Some(error)),
            };
        Self {
            classify: jev::Client::from_env().ok(),
            generate,
            transcript: Vec::new(),
            repo,
            task: String::new(),
            trace,
            trace_error,
        }
    }

    /// An agent over explicit parts, for tests.
    pub fn new(classify: Option<jev::Client>, generate: Door) -> Self {
        Self {
            classify,
            generate,
            transcript: Vec::new(),
            repo: None,
            task: String::new(),
            trace: None,
            trace_error: None,
        }
    }

    /// The repo the shell sits in, for the prompt's context block.
    pub fn with_repo(mut self, repo: Option<Repo>) -> Self {
        self.repo = repo;
        self
    }

    /// The recorder this session writes to, for tests and for a caller that
    /// wants to name the directory.
    pub fn with_trace(mut self, trace: Option<Recorder>) -> Self {
        self.trace = trace;
        self
    }

    /// Where this session is being recorded, when it is.
    pub fn trace_path(&self) -> Option<&Path> {
        self.trace.as_ref().map(Recorder::path)
    }

    /// Why this session is not being recorded, or why its recording
    /// stopped.
    pub fn trace_error(&self) -> Option<&str> {
        self.trace_error
            .as_deref()
            .or_else(|| self.trace.as_ref().and_then(Recorder::failure))
    }

    /// Closes the session's trace, so the document says the session ended
    /// rather than that it was interrupted.
    pub fn finish_trace(&mut self) {
        if let Some(trace) = &mut self.trace {
            trace.finish(atif::log::ENDED);
        }
    }

    /// Records a reply the terminal produced without generating one — the
    /// canned answers an `End` or a `Halt` route gives.
    pub fn record_reply(&mut self, text: &str) {
        if let Some(trace) = &mut self.trace {
            trace.answer(text, None, 0);
        }
    }

    /// The model the door serves, for the token rail.
    pub fn model(&self) -> &str {
        self.generate.model()
    }

    /// Whether a classifier is configured.
    pub fn classifies(&self) -> bool {
        self.classify.is_some()
    }

    /// The transcript so far.
    pub fn transcript(&self) -> &[Message] {
        &self.transcript
    }

    /// Starts a turn: the draft folds into the transcript as the user side
    /// and becomes the state Classify reads.
    pub fn push_user(&mut self, draft: &str) {
        self.task = draft.to_string();
        self.transcript.push(Message {
            role: Role::User,
            text: draft.to_string(),
        });
        if let Some(trace) = &mut self.trace {
            trace.user(draft);
        }
    }

    /// Classifies the current state: the questions over the task and the
    /// bounded transcript.
    ///
    /// The call is recorded whether it answers or not, with the state's
    /// digest, the question set, the typed answers, and the route the table
    /// made of them. That is the record the Gym's rows cannot hold: a row
    /// says what one door answered, and this says what the agent did next.
    pub async fn classify(&mut self) -> Classified {
        let Some(classify) = self.classify.clone() else {
            let note = "no TYPESAFE_API_KEY — generating unrouted".to_string();
            if let Some(trace) = &mut self.trace {
                trace.note(&note);
            }
            return Classified::Skipped(note);
        };
        let members: &[String] = self.repo.as_ref().map_or(&[], |repo| repo.members());
        let state = state_of(&self.task, &self.transcript, members);
        let request = SystemOneRequest::new(state, questions());
        // The body is what goes on the wire; reading it here is what a
        // recorded exchange means.
        let asked = request
            .body(classify.default_model())
            .map_or(Value::Null, Value::Object);
        let started = Instant::now();
        let answered = classify.system_one(request).await;
        let milliseconds = started.elapsed().as_millis() as u64;
        match answered {
            Ok(response) => {
                let judgment = judgment_of(&response);
                let route = route(&judgment);
                self.record_decision(Decision {
                    id: String::new(),
                    name: "classify".to_string(),
                    door: classify.base_url().to_string(),
                    model: response.model.clone(),
                    request: asked,
                    answers: answers_value(&response.answers),
                    route: Some(route.word().to_string()),
                    error: None,
                    milliseconds,
                });
                Classified::Judged(Verdict { route, judgment })
            }
            Err(error) => {
                self.record_decision(Decision {
                    id: String::new(),
                    name: "classify".to_string(),
                    door: classify.base_url().to_string(),
                    model: classify.default_model().to_string(),
                    request: asked,
                    answers: Value::Null,
                    route: None,
                    error: Some(error.to_string()),
                    milliseconds,
                });
                Classified::Skipped(format!("classify failed ({error}) — generating unrouted"))
            }
        }
    }

    /// Puts one decision call in the trace, when there is one.
    fn record_decision(&mut self, decision: Decision) {
        if let Some(trace) = &mut self.trace {
            trace.decision(decision);
        }
    }

    /// Generates the reply the route asks for and folds it into the
    /// transcript as the assistant side. `clarify` swaps the instructions
    /// for the one-question variant. `sink` receives text deltas as they
    /// stream. One generation, no shell loop — [`Agent::turn`] is the
    /// full round.
    pub async fn reply(
        &mut self,
        clarify: bool,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let instructions = self.instructions(clarify, false);
        if let Some(trace) = &mut self.trace {
            trace.instructions(&instructions);
        }
        let started = Instant::now();
        let (text, usage) = self
            .generate
            .generate(&instructions, &self.transcript, sink, meta)
            .await?;
        let milliseconds = started.elapsed().as_millis() as u64;
        if let Some(trace) = &mut self.trace {
            trace.answer(&text, usage, milliseconds);
        }
        self.transcript.push(Message {
            role: Role::Assistant,
            text: text.clone(),
        });
        Ok((text, usage))
    }

    /// A whole turn: generate, run any plan the reply carries, judge the
    /// round, and go again until the model answers in prose or the round
    /// cap lands. `shell` hears each proposal, outcome, and verdict as it
    /// happens so the terminal can draw the loop.
    pub async fn turn(
        &mut self,
        clarify: bool,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
        shell: &mut (dyn FnMut(ShellEvent) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let mut total: Option<Usage> = None;
        let mut rounds = 0usize;
        let mut final_only = false;
        loop {
            let instructions = self.instructions(clarify, final_only);
            if let Some(trace) = &mut self.trace {
                trace.instructions(&instructions);
            }
            let started = Instant::now();
            let (text, usage) = self
                .generate
                .generate(&instructions, &self.transcript, sink, meta)
                .await?;
            let milliseconds = started.elapsed().as_millis() as u64;
            if let Some(trace) = &mut self.trace {
                trace.answer(&text, usage, milliseconds);
            }
            if let Some(usage) = usage {
                let entry = total.get_or_insert(Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                });
                entry.input_tokens += usage.input_tokens;
                entry.output_tokens += usage.output_tokens;
            }
            let proposals = if final_only {
                None
            } else {
                shell::parse_plan(&text)
            };
            let Some(proposals) = proposals else {
                self.transcript.push(Message {
                    role: Role::Assistant,
                    text: text.clone(),
                });
                return Ok((text, total));
            };
            rounds += 1;
            self.transcript.push(Message {
                role: Role::Assistant,
                text,
            });
            let mut outcomes: Vec<Outcome> = Vec::new();
            for proposal in proposals.into_iter().take(shell::COMMANDS_MAX) {
                shell(ShellEvent::Proposed(proposal.clone()));
                let outcome = shell::run(&proposal).await;
                if let Some(trace) = &mut self.trace {
                    trace.command(&outcome);
                }
                shell(ShellEvent::Ran(outcome.clone()));
                outcomes.push(outcome);
            }
            let route = self.judge(&outcomes, shell).await;
            self.transcript.push(Message {
                role: Role::User,
                text: shell::transcript_of(&outcomes),
            });
            if route == ShellRoute::Stop || rounds >= shell::ROUNDS_MAX {
                final_only = true;
            }
        }
    }

    /// The instructions for one generation: the base text, the clarify or
    /// final suffix, and the repo context block.
    fn instructions(&self, clarify: bool, final_only: bool) -> String {
        let mut instructions = if clarify {
            format!("{INSTRUCTIONS}{CLARIFY_SUFFIX}")
        } else {
            INSTRUCTIONS.to_string()
        };
        if final_only {
            instructions.push_str(FINAL_SUFFIX);
        }
        if let Some(repo) = &self.repo {
            instructions.push_str("\n\n");
            instructions.push_str(&repo.context_for(&self.task));
        }
        instructions
    }

    /// The judge's read on a round of outcomes: `Pass` without a
    /// classifier, the verdict's route otherwise, with the display line
    /// reported to `shell` either way.
    async fn judge(
        &mut self,
        outcomes: &[Outcome],
        shell: &mut (dyn FnMut(ShellEvent) + Send),
    ) -> ShellRoute {
        let Some(classify) = self.classify.clone() else {
            return ShellRoute::Pass;
        };
        let state = shell::state_of(&self.task, outcomes);
        let request = SystemOneRequest::new(state, shell_questions());
        let asked = request
            .body(classify.default_model())
            .map_or(Value::Null, Value::Object);
        let started = Instant::now();
        let answered = classify.system_one(request).await;
        let milliseconds = started.elapsed().as_millis() as u64;
        match answered {
            Ok(response) => {
                let verdict = shell_verdict_of(&response);
                let route = verdict.route();
                self.record_decision(Decision {
                    id: String::new(),
                    name: "shell_judge".to_string(),
                    door: classify.base_url().to_string(),
                    model: response.model.clone(),
                    request: asked,
                    answers: answers_value(&response.answers),
                    route: Some(route.word().to_string()),
                    error: None,
                    milliseconds,
                });
                shell(ShellEvent::Verdict(verdict.line()));
                route
            }
            Err(error) => {
                self.record_decision(Decision {
                    id: String::new(),
                    name: "shell_judge".to_string(),
                    door: classify.base_url().to_string(),
                    model: classify.default_model().to_string(),
                    request: asked,
                    answers: Value::Null,
                    route: None,
                    error: Some(error.to_string()),
                    milliseconds,
                });
                ShellRoute::Pass
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::StubGenerate;

    #[tokio::test]
    async fn classify_without_a_key_skips_with_a_note() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        let Classified::Skipped(note) = agent.classify().await else {
            panic!("expected a skipped classify");
        };
        assert!(note.contains("TYPESAFE_API_KEY"));
    }

    #[tokio::test]
    async fn a_reply_streams_and_joins_the_transcript() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        agent.push_user("hello");
        let mut seen = String::new();
        let (text, _) = agent
            .reply(false, &mut |d| seen.push_str(d), &mut |_| {})
            .await
            .unwrap();
        assert_eq!(text, seen);
        assert_eq!(agent.transcript().len(), 2);
        assert_eq!(agent.transcript()[1].role, Role::Assistant);
    }
}
