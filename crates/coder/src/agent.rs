//! The agent: a turn of conversation, Classify first, Generate on route.
//!
//! A turn runs in two phases so the terminal can draw the judgment inline
//! before the answer streams: [`Agent::classify`] reads the state and
//! returns a [`Verdict`] — the judgment plus the route it produced — and
//! [`Agent::reply`] runs Generate when the route says to. The transcript
//! folds each side in as it lands.

use std::env;

use jev::SystemOneRequest;

use crate::classify::{Judgment, Route, judgment_of, questions, route, state_of};
use crate::generate::{Door, Generate, GenerateError, Message, Meta, Role, Usage};
use crate::repo::Repo;

/// The instructions Generate hears for a plain answer.
pub const INSTRUCTIONS: &str = "You are Coder, an assistant that lives in a terminal. \
    Answer directly and tersely. Plain prose, short paragraphs, no headers. \
    If the conversation needs code, say what you would change in words. \
    The REPO CONTEXT block describes the repository the user is working in: \
    answer project questions from it and name real paths, and if the context \
    does not cover the question, say so rather than guessing.";

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
}

impl Agent {
    /// An agent from the environment: `TYPESAFE_API_KEY` builds the
    /// classifier, the door builds itself. A missing key degrades — the
    /// conversation still runs, without judgments.
    pub fn from_env() -> Self {
        Self {
            classify: jev::Client::from_env().ok(),
            generate: Door::from_env(),
            transcript: Vec::new(),
            repo: Repo::discover(&env::current_dir().unwrap_or_default()),
            task: String::new(),
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
        }
    }

    /// The repo the shell sits in, for the prompt's context block.
    pub fn with_repo(mut self, repo: Option<Repo>) -> Self {
        self.repo = repo;
        self
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
    }

    /// Classifies the current state: the questions over the task and the
    /// bounded transcript.
    pub async fn classify(&self) -> Classified {
        let Some(classify) = &self.classify else {
            return Classified::Skipped("no TYPESAFE_API_KEY — generating unrouted".to_string());
        };
        let members: &[String] = self.repo.as_ref().map_or(&[], |repo| repo.members());
        let state = state_of(&self.task, &self.transcript, members);
        match classify
            .system_one(SystemOneRequest::new(state, questions()))
            .await
        {
            Ok(response) => {
                let judgment = judgment_of(&response);
                Classified::Judged(Verdict {
                    route: route(&judgment),
                    judgment,
                })
            }
            Err(error) => {
                Classified::Skipped(format!("classify failed ({error}) — generating unrouted"))
            }
        }
    }

    /// Generates the reply the route asks for and folds it into the
    /// transcript as the assistant side. `clarify` swaps the instructions
    /// for the one-question variant. `sink` receives text deltas as they
    /// stream.
    pub async fn reply(
        &mut self,
        clarify: bool,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let mut instructions = if clarify {
            format!("{INSTRUCTIONS}{CLARIFY_SUFFIX}")
        } else {
            INSTRUCTIONS.to_string()
        };
        if let Some(repo) = &self.repo {
            instructions.push_str("\n\n");
            instructions.push_str(&repo.context_for(&self.task));
        }
        let (text, usage) = self
            .generate
            .generate(&instructions, &self.transcript, sink, meta)
            .await?;
        self.transcript.push(Message {
            role: Role::Assistant,
            text: text.clone(),
        });
        Ok((text, usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::StubGenerate;

    #[tokio::test]
    async fn classify_without_a_key_skips_with_a_note() {
        let agent = Agent::new(None, Door::Stub(StubGenerate::default()));
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
