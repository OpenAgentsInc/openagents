//! The plugin-creation flow on a computer (#10177): the `eval.author`
//! route's steps when the chat is a terminal where Coder runs (its client
//! runs the steps that happen on the computer) and Jev's `tool` reading is
//! a new plugin.
//!
//! The steps and their fixed lines are [`openagents_chat::plugin_flow`]'s.
//! This module is the worker's half: which step a turn serves, decided from
//! our own last message (an exact comparison against those lines), the
//! chat's Coder run (its typed ending and the bounded paths it changed),
//! and Jev's typed readings of the person's reply. It writes no text but
//! fixed lines, runs nothing, and never reads the person's words to decide:
//!
//! - **Start.** Jev's `tool` reading is `make` (a skill or new code alike:
//!   Coder drafts either here), and its `scope` reading says whether the
//!   messages already say what the plugin should do. Not yet: one question
//!   ([`Step::Scope`]). Already: the draft.
//! - **Scope answered.** Any reply is the answer: Coder drafts it
//!   ([`Step::Draft`], a Run Coder offer; this computer adds
//!   [`openagents_chat::plugin_flow::BRIEF`]).
//! - **Draft.** While Coder works, a message goes to it. Once its run
//!   ended with a plugin, the reply answers the tests this computer showed
//!   ([`Step::Tests`]): Jev's `reply` reading approves (the run,
//!   [`Step::Run`]), asks for a change (Coder again), or neither (the tests
//!   again). A run that left no plugin goes back to Coder.
//! - **Run.** The reply answers the publish question this computer asked
//!   after the run ([`Step::Publish`]): Jev's `publish` reading chooses
//!   publishing, turning it on here, both, or neither ([`Step::Done`]), or
//!   asks again.

use indexmap::IndexMap;
use jev::{Choice, Entry, Questions, SystemOneRequest};
use openagents_chat::plugin_flow::{Flow, PLUGINS_DIR, Step, drafted};
use serde_json::json;

use super::{Author, AuthorError, OURS_CHARS, Reply, cut};
use crate::generate::{Generate, Message, Role};
use crate::router::seams::{AuthorAsk, AuthorStep};
use crate::router::{CoderRun, Offer, RunEnding};

/// The model a step names: its words are the flow's fixed lines.
pub const MODEL: &str = "plugin-flow";

/// The least probability of a `publish` choice that acts on it.
pub const PUBLISH_AT: f64 = 0.7;

/// The open step our last message ended at, if a plugin is being made:
/// any step but [`Step::Done`]. An exact comparison against the flow's
/// fixed lines.
#[must_use]
pub fn open(transcript: &[Message]) -> Option<Step> {
    let ours = transcript
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant)?;
    Step::from_line(&ours.text).filter(|step| *step != Step::Done)
}

fn run_coder() -> Option<Offer> {
    Some(Offer::RunCoder {
        label: "Run Coder".into(),
        engine: None,
        plan: Default::default(),
    })
}

fn step(text: String, flow: Flow, offer: Option<Offer>) -> AuthorStep {
    AuthorStep {
        text,
        draft: None,
        offer,
        model: MODEL.into(),
        plugin: Some(flow),
    }
}

fn said(lead: &str, at: Step) -> String {
    if lead.is_empty() {
        at.line().to_owned()
    } else {
        format!("{lead}\n\n{}", at.line())
    }
}

/// The draft step: Coder drafts the plugin here.
fn draft(lead: &str, slug: Option<String>) -> AuthorStep {
    step(
        said(lead, Step::Draft),
        Flow::at(Step::Draft, slug),
        run_coder(),
    )
}

/// What we say before Coder's first draft.
pub const DRAFTING: &str = "Drafting the plugin in this project's plugins folder: its package, a \
skill that says what it does and doesn't do, and its tests. Nothing is installed, published, or \
turned on yet.";

/// The flow's first step for a request for a new plugin: one question
/// when the `scope` reading isn't sure the request says what it should do,
/// else the draft.
#[must_use]
pub fn start(stated: f64) -> AuthorStep {
    if stated >= super::APPROVE_AT {
        draft(DRAFTING, None)
    } else {
        step(
            said(
                "We can build that as a plugin on this computer: we draft it here, you \
                 approve its tests, and we run them before you publish it.",
                Step::Scope,
            ),
            Flow::at(Step::Scope, None),
            None,
        )
    }
}

/// Whether the chat's Coder run has ended its turn.
fn ended(run: Option<&CoderRun>) -> bool {
    run.is_some_and(|run| {
        matches!(
            run.ending,
            RunEnding::Finished | RunEnding::Failed | RunEnding::Stopped
        )
    })
}

/// The plugin the run drafted, from the paths it reported changing.
fn plugin_of(run: Option<&CoderRun>) -> Option<(String, Vec<String>)> {
    drafted(run?.files.iter().map(|(path, _)| path.as_str()))
}

/// The tests this computer showed, as we said them: our words for the
/// `reply` reading when the transcript doesn't hold them.
fn tests_said(slug: &str, tests: &[String]) -> String {
    let listed = if tests.is_empty() {
        "no tests yet".to_owned()
    } else {
        format!("these tests: {}", tests.join(", "))
    };
    said(
        &format!("Drafted the plugin in {PLUGINS_DIR}/{slug} with {listed}."),
        Step::Tests,
    )
}

/// The `publish` reading's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publish {
    Both,
    Publish,
    Enable,
    Neither,
    Other,
}

impl<G: Generate> Author<G> {
    /// One turn of an open plugin flow at `open`.
    ///
    /// # Errors
    ///
    /// [`AuthorError::Judge`] when Jev doesn't answer.
    pub async fn plugin_step(
        &self,
        open: Step,
        ask: &AuthorAsk,
    ) -> Result<AuthorStep, AuthorError> {
        let run = ask.coder_run.as_ref();
        let ours = ask
            .transcript
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
            .map(|m| m.text.clone())
            .unwrap_or_default();
        match open {
            Step::Scope => Ok(draft(DRAFTING, None)),
            Step::Draft | Step::Tests => {
                if !ended(run) {
                    return Ok(match run {
                        // Coder works: the message goes to it.
                        Some(_) => draft(
                            "Still drafting the plugin; your message goes to the draft too.",
                            None,
                        ),
                        None => draft(DRAFTING, None),
                    });
                }
                let Some((slug, tests)) = plugin_of(run) else {
                    return Ok(draft(
                        &format!(
                            "Trying again with your message: the last run ended without a plugin in {PLUGINS_DIR}/."
                        ),
                        None,
                    ));
                };
                let shown = if open == Step::Tests {
                    ours
                } else {
                    tests_said(&slug, &tests)
                };
                match self.reply(&shown, &ask.message).await? {
                    Reply::Approve => Ok(step(
                        Step::Run.line().to_owned(),
                        Flow::at(Step::Run, Some(slug)),
                        None,
                    )),
                    Reply::Change => Ok(draft(
                        "Changing the plugin and its tests as you said.",
                        Some(slug),
                    )),
                    Reply::Other => Ok(step(
                        tests_said(&slug, &tests),
                        Flow::at(Step::Tests, Some(slug)),
                        None,
                    )),
                }
            }
            Step::Run | Step::Publish => {
                let slug = plugin_of(run).map(|(slug, _)| slug);
                let flow = |step: Step| Flow::at(step, slug.clone());
                let mut done = flow(Step::Done);
                // After a run, this computer asked the publish question.
                let shown = if open == Step::Publish {
                    ours
                } else {
                    Step::Publish.line().to_owned()
                };
                let lead = match self.publish(&shown, &ask.message).await? {
                    Publish::Both => {
                        done.publish = true;
                        done.enable = true;
                        "Publishing it to the registry and turning it on on this computer."
                    }
                    Publish::Publish => {
                        done.publish = true;
                        "Publishing it to the registry, and leaving it off on this computer."
                    }
                    Publish::Enable => {
                        done.enable = true;
                        "Turning it on on this computer, without publishing it."
                    }
                    Publish::Neither => {
                        "Leaving it as it is: not published, and off on this computer."
                    }
                    Publish::Other => {
                        return Ok(step(
                            Step::Publish.line().to_owned(),
                            flow(Step::Publish),
                            None,
                        ));
                    }
                };
                Ok(step(said(lead, Step::Done), done, None))
            }
            // A finished flow is not open.
            Step::Done => Ok(step(Step::Done.line().to_owned(), flow_done(), None)),
        }
    }

    /// How the person answered the publish question, from Jev's typed
    /// `publish` question. A choice counts at [`PUBLISH_AT`]; below it the
    /// question is asked again.
    ///
    /// # Errors
    ///
    /// [`AuthorError::Judge`] when Jev doesn't answer.
    pub async fn publish(&self, ours: &str, message: &str) -> Result<Publish, AuthorError> {
        let we_said = if ours.trim().is_empty() {
            Step::Publish.line()
        } else {
            ours
        };
        let options = IndexMap::from([
            (
                "both".to_string(),
                Some(Entry::from(
                    "They want both: publish the plugin and turn it on on this computer. A plain yes, go ahead, or sure to our question counts.",
                )),
            ),
            (
                "publish".to_string(),
                Some(Entry::from(
                    "They want it published to the registry only, not turned on here.",
                )),
            ),
            (
                "enable".to_string(),
                Some(Entry::from(
                    "They want it turned on on this computer only, not published.",
                )),
            ),
            (
                "neither".to_string(),
                Some(Entry::from(
                    "They want neither for now: no, not yet, leave it, or later.",
                )),
            ),
            (
                "other".to_string(),
                Some(Entry::from(
                    "They ask a question or say something that answers neither way.",
                )),
            ),
        ]);
        let questions = Questions::new().with(
            "publish",
            Choice::new(
                "We asked whether to publish their new plugin to the registry and turn it on on this computer. What do they want?",
                options,
            ),
        );
        let state =
            json!({"we_said": cut(we_said, OURS_CHARS), "they_replied": cut(message, 1_200)});
        let response = self
            .ask_judge(SystemOneRequest::new(Entry::from(state), questions))
            .await?;
        let answer = response
            .choice("publish")
            .map_err(|e| AuthorError::Judge(e.to_string()))?;
        let p = answer
            .probabilities
            .get(&answer.choice)
            .copied()
            .unwrap_or(0.0);
        Ok(match answer.choice.as_str() {
            _ if p < PUBLISH_AT => Publish::Other,
            "both" => Publish::Both,
            "publish" => Publish::Publish,
            "enable" => Publish::Enable,
            "neither" => Publish::Neither,
            _ => Publish::Other,
        })
    }
}

fn flow_done() -> Flow {
    Flow::at(Step::Done, None)
}
