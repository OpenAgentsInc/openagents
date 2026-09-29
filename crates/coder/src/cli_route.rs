//! The chat router's CLI route: descending the `openagents` command tree
//! with Jev, filling the chosen command's parameters, and proposing it.
//!
//! This is route 7 of `docs/coder/design/2026-09-28-chat-router.md`
//! ("The CLI route: descending the command tree"), behind the router's
//! [`CliRoute`] seam. The pieces:
//!
//! - [`tree`]: the [`CommandTree`](tree::CommandTree), generated from the
//!   command's own help text by `openagents-cli` and bundled here, with
//!   each command's declared effect and where it runs.
//! - [`descend`]: one Jev Choice per level, over exactly that level's
//!   commands plus `none`.
//! - [`params`]: enumerable values selected by Jev from candidates code
//!   lists, free text written by the model, and the result checked by the
//!   command's own argument parser ([`crate::argv::Args`]).
//! - [`gate`]: what each surface may be offered. Money and secrets never;
//!   on the phone, only the owner's list of read-only commands.
//!
//! [`CommandRoute`] puts them together. Nothing here runs a command: a
//! proposal is an offer, and the tap that accepts it runs the command
//! under the device's own authority, as [`Execution`] describes.
//!
//! # Running a proposal
//!
//! [`execution`] names the existing path for each place: the phone's own
//! core for `computer list`, `show`, and `workspaces` (the same
//! `coder_computers::live` client `openagents computer` uses); the user's
//! computer for the rest, as `openagents --json ARGV` over NIP-HOST
//! `terminal.open` and NIP-TERM, the path `openagents computer exec HOST
//! -- …` takes, which needs the grant's `terminal` right; and a child
//! process on the desktop or in the terminal, as `openagents mcp serve`
//! runs each tool call. The phone's tap handler for a `cli` offer is not
//! written yet (`crates/openagents-mobile` is outside this change), so on
//! the phone a proposal is shown but not yet runnable.

pub mod descend;
pub mod eval;
pub mod gate;
pub mod params;
pub mod tree;
pub mod usage;

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use jev::{RetryPolicy, SystemOneRequest};
use serde_json::{Value, json};

use crate::generate::{Generate, Message, Meta, Role};
use crate::router::seams::{CliAnswer, CliAsk, CliGroup, CliProposal, CliRoute, SeamError};
use params::{Filled, Host, Values};
use tree::{CommandTree, Effect, Leaf, RunsOn};

/// How long one Jev request of the descent may take.
pub const LEVEL_BUDGET: Duration = Duration::from_millis(1_500);

/// Writes the free text of a command's parameters: a model door.
pub trait Fill: Send + Sync {
    /// The model's reply to `instructions` over the conversation `input`.
    fn fill<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
    ) -> BoxFuture<'a, Result<String, String>>;

    /// The services the conversation reaches through this fill, for the
    /// privacy answer.
    fn recipients(&self) -> Vec<String>;
}

/// A [`Fill`] through any [`Generate`] door: the chat worker's own model.
pub struct ModelFill<G> {
    pub door: G,
    /// The service, named for a person ("Vercel AI Gateway").
    pub service: String,
}

impl<G: Generate> Fill for ModelFill<G> {
    fn fill<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
    ) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let mut sink = |_: &str| {};
            let mut meta = |_: Meta| {};
            self.door
                .generate(instructions, input, &mut sink, &mut meta)
                .await
                .map(|(text, _)| text)
                .map_err(|error| error.cause().to_string())
        })
    }

    fn recipients(&self) -> Vec<String> {
        vec![self.service.clone()]
    }
}

/// No model: a command that needs typed text is reported as missing it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFill;

impl Fill for NoFill {
    fn fill<'a>(&'a self, _: &'a str, _: &'a [Message]) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async { Err("no model is configured".to_string()) })
    }

    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
}

/// One answered level of the descent.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// The words above the choice (`["computer"]`), empty for the group.
    pub at: Vec<String>,
    pub choice: String,
    /// The choice's probability; `None` for a group the router chose.
    pub p: Option<f64>,
}

/// How an accepted proposal runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Execution {
    /// The phone's own core runs it with its `coder_computers` client.
    Device { argv: Vec<String> },
    /// The user's computer runs `command` (`openagents --json …`) through
    /// NIP-HOST `terminal.open` and NIP-TERM, as `openagents computer exec`
    /// does; the grant must carry the `terminal` right.
    Computer { command: Vec<String> },
    /// This device runs `command` as a child process.
    Local { command: Vec<String> },
}

/// How `argv` runs for `runs_on`. A screen's command does not run.
#[must_use]
pub fn execution(argv: &[String], runs_on: RunsOn) -> Option<Execution> {
    let command = || {
        let mut command = vec!["openagents".to_string(), "--json".to_string()];
        command.extend(argv.iter().cloned());
        command
    };
    match runs_on {
        RunsOn::ThisDevice if argv.first().is_some_and(|group| group == "computer") => {
            Some(Execution::Device {
                argv: argv.to_vec(),
            })
        }
        RunsOn::ThisDevice => Some(Execution::Local { command: command() }),
        RunsOn::ConnectedComputer => Some(Execution::Computer { command: command() }),
        RunsOn::Screen => None,
    }
}

/// What the descent found, with its trail.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// A complete command that passed the parser and the gate.
    Proposal {
        argv: Vec<String>,
        effect: Effect,
        runs_on: RunsOn,
        execution: Option<Execution>,
        trail: Vec<Step>,
    },
    /// The command is chosen but a required value is missing; `what` is
    /// for a person ("which computer").
    Missing {
        path: Vec<String>,
        what: String,
        trail: Vec<Step>,
    },
    /// The command is chosen but may not be offered on this surface.
    NotOffered {
        path: Vec<String>,
        effect: Effect,
        trail: Vec<Step>,
    },
    /// No command: a `none`, or a level below its confidence floor.
    NoCommand { trail: Vec<Step> },
}

impl Outcome {
    /// The descent's answered levels.
    #[must_use]
    pub fn trail(&self) -> &[Step] {
        match self {
            Outcome::Proposal { trail, .. }
            | Outcome::Missing { trail, .. }
            | Outcome::NotOffered { trail, .. }
            | Outcome::NoCommand { trail } => trail,
        }
    }

    /// The answer the router's seam carries. A command that may not be
    /// offered is no command at all: nothing of it reaches the reply.
    #[must_use]
    pub fn answer(&self) -> CliAnswer {
        match self {
            Outcome::Proposal {
                argv,
                effect,
                runs_on,
                ..
            } => CliAnswer::Proposal(CliProposal {
                argv: argv.clone(),
                effect: *effect,
                runs_on: *runs_on,
            }),
            Outcome::Missing { what, .. } => CliAnswer::Missing(what.clone()),
            Outcome::NotOffered { .. } | Outcome::NoCommand { .. } => CliAnswer::NoCommand,
        }
    }

    /// A record of the outcome for evidence: the command and the levels,
    /// never the message or a free-text value.
    #[must_use]
    pub fn evidence(&self) -> Value {
        let trail: Vec<Value> = self
            .trail()
            .iter()
            .map(|step| json!({ "at": step.at, "choice": step.choice, "p": step.p }))
            .collect();
        match self {
            Outcome::Proposal {
                argv,
                effect,
                runs_on,
                ..
            } => json!({
                "set": descend::SET, "outcome": "proposal",
                "command": command_words(argv), "effect": effect.word(),
                "runs_on": runs_on.word(), "trail": trail,
            }),
            Outcome::Missing { path, .. } => {
                json!({ "set": descend::SET, "outcome": "missing", "command": path, "trail": trail })
            }
            Outcome::NotOffered { path, effect, .. } => json!({
                "set": descend::SET, "outcome": "not_offered", "command": path,
                "effect": effect.word(), "trail": trail,
            }),
            Outcome::NoCommand { .. } => {
                json!({ "set": descend::SET, "outcome": "none", "trail": trail })
            }
        }
    }
}

/// The command words of `argv` (group and subcommands), without values.
fn command_words(argv: &[String]) -> Vec<String> {
    let Some(first) = argv.first() else {
        return Vec::new();
    };
    let mut path = vec![first.clone()];
    let mut node = tree::bundled().group(first);
    for word in &argv[1..] {
        match node.and_then(|n| n.child(word)) {
            Some(child) => {
                path.push(word.clone());
                node = Some(child);
            }
            None => break,
        }
    }
    path
}

/// What to ask the person for when `key` is missing.
fn ask_for(leaf: &Leaf, key: &str) -> String {
    match key {
        "HOST" | "--host" => "which computer".to_string(),
        "--workspace" => "which workspace".to_string(),
        "TEXT" if leaf.path.last().is_some_and(|w| w == "search") => {
            "what to search for".to_string()
        }
        "CMD" => "which command to run".to_string(),
        _ => format!(
            "the {} for `{}`",
            key.trim_start_matches("--")
                .to_lowercase()
                .replace('_', " "),
            leaf.command()
        ),
    }
}

/// The CLI route: a Jev client, a model for free text, the tree, and the
/// device's computers when the caller knows them.
pub struct CommandRoute {
    jev: jev::Client,
    fill: Arc<dyn Fill>,
    tree: &'static CommandTree,
    hosts: Arc<dyn Fn() -> Vec<Host> + Send + Sync>,
}

impl CommandRoute {
    /// A route over the bundled tree. With no hosts, a command that needs
    /// a `HOST` is answered [`CliAnswer::Missing`] ("which computer"): the
    /// chat worker never learns the device's computers.
    #[must_use]
    pub fn new(jev: jev::Client, fill: Arc<dyn Fill>) -> Self {
        Self {
            jev,
            fill,
            tree: tree::bundled(),
            hosts: Arc::new(Vec::new),
        }
    }

    /// Select `HOST` values from the computers `hosts` lists, as a desktop
    /// or terminal chat can: its own device store.
    #[must_use]
    pub fn with_hosts(mut self, hosts: Arc<dyn Fn() -> Vec<Host> + Send + Sync>) -> Self {
        self.hosts = hosts;
        self
    }

    fn retry() -> RetryPolicy {
        RetryPolicy {
            max_retries: 0,
            budget: Some(LEVEL_BUDGET),
            ..RetryPolicy::default()
        }
    }

    async fn ask(
        &self,
        state: &Value,
        questions: jev::Questions,
    ) -> Result<jev::SystemOneResponse, SeamError> {
        self.jev
            .system_one(
                SystemOneRequest::new(state.clone(), questions)
                    .retry(Self::retry())
                    .timeout(LEVEL_BUDGET),
            )
            .await
            .map_err(|error| SeamError::Failed(format!("jev: {error}")))
    }

    /// The router's level-0 question asked alone: the group and its
    /// probability, or `None` below [`descend::GROUP_CONFIDENCE`] or at
    /// `none`. The router asks the same question in its own request; this
    /// is for a caller without one, such as the evaluation.
    ///
    /// # Errors
    ///
    /// A Jev failure.
    pub async fn group(&self, ask: &CliAsk) -> Result<Option<(String, f64)>, SeamError> {
        Ok(self
            .group_reading(ask)
            .await?
            .filter(|(group, p)| group != "none" && *p >= descend::GROUP_CONFIDENCE))
    }

    /// The level-0 answer as read: its argmax and probability, before the
    /// confidence floor.
    ///
    /// # Errors
    ///
    /// A Jev failure.
    pub async fn group_reading(&self, ask: &CliAsk) -> Result<Option<(String, f64)>, SeamError> {
        let state = state(ask);
        let response = self
            .ask(
                &state,
                jev::Questions::new()
                    .with(descend::GROUP_QUESTION, descend::group_question(self.tree)),
            )
            .await?;
        Ok(descend::pick(&response, descend::GROUP_QUESTION))
    }

    /// Descend from `ask.group`, fill the chosen command, and check it.
    ///
    /// # Errors
    ///
    /// A Jev or model failure; the router then answers with the model
    /// alone.
    pub async fn outcome(&self, ask: &CliAsk) -> Result<Outcome, SeamError> {
        let state = state(ask);
        let mut trail = vec![Step {
            at: Vec::new(),
            choice: ask.group.clone(),
            p: None,
        }];
        let Some(group) = self.tree.group(&ask.group) else {
            return Ok(Outcome::NoCommand { trail });
        };
        let mut node = group;
        let mut path = vec![group.name.clone()];
        while !node.children.is_empty() {
            let response = self
                .ask(&state, descend::level_questions(&path, node))
                .await?;
            let Some((choice, p)) = descend::pick(&response, descend::LEVEL_QUESTION) else {
                return Ok(Outcome::NoCommand { trail });
            };
            trail.push(Step {
                at: path.clone(),
                choice: choice.clone(),
                p: Some(p),
            });
            if choice == "none" || p < descend::LEVEL_CONFIDENCE {
                return Ok(Outcome::NoCommand { trail });
            }
            if choice == descend::SELF {
                break;
            }
            let Some(child) = node.child(&choice) else {
                return Ok(Outcome::NoCommand { trail });
            };
            node = child;
            path.push(choice);
        }
        let Some(leaf) = node.leaf.as_ref() else {
            return Ok(Outcome::NoCommand { trail });
        };
        if !gate::offered(leaf, ask.surface) {
            return Ok(Outcome::NotOffered {
                path: leaf.path.clone(),
                effect: leaf.effect,
                trail,
            });
        }
        let form = &leaf.forms[0];
        let mut values = self.select(&state, leaf, form).await?;
        if params::needs_model(form) {
            let input = input(ask);
            let text = self
                .fill
                .fill(&params::fill_instructions(leaf, form), &input)
                .await
                .map_err(|why| SeamError::Failed(format!("fill: {why}")))?;
            match params::read_fill(&text, form) {
                Ok(filled) => {
                    for (key, value) in filled {
                        values.entry(key).or_insert(value);
                    }
                }
                Err(_) => {
                    let what = params::params(form)
                        .into_iter()
                        .find(|p| p.required && matches!(p.kind, params::Kind::Text { .. }))
                        .map_or_else(|| "the details".to_string(), |p| ask_for(leaf, &p.key));
                    return Ok(Outcome::Missing {
                        path: leaf.path.clone(),
                        what,
                        trail,
                    });
                }
            }
        }
        let argv = match params::argv(leaf, form, &values) {
            Ok(argv) => argv,
            Err(missing) => {
                return Ok(Outcome::Missing {
                    path: leaf.path.clone(),
                    what: ask_for(leaf, &missing[0]),
                    trail,
                });
            }
        };
        if params::validate(leaf, group, &argv).is_err() {
            return Ok(Outcome::Missing {
                path: leaf.path.clone(),
                what: format!("the details for `{}`", leaf.command()),
                trail,
            });
        }
        let runs_on = gate::runs_on(leaf, ask.surface);
        Ok(Outcome::Proposal {
            execution: execution(&argv, runs_on),
            argv,
            effect: leaf.effect,
            runs_on,
            trail,
        })
    }

    async fn select(
        &self,
        state: &Value,
        leaf: &Leaf,
        form: &usage::Form,
    ) -> Result<Values, SeamError> {
        let hosts = (self.hosts)();
        let (questions, candidates) = params::selection(leaf, form, &hosts, None);
        let mut values = if questions.is_empty() {
            Values::new()
        } else {
            params::selected(&self.ask(state, questions).await?, &candidates)
        };
        let chosen = match values.get("HOST").or_else(|| values.get("--host")) {
            Some(Filled::Word(id)) => hosts.iter().find(|host| &host.id == id),
            _ => None,
        };
        if let Some(host) = chosen {
            let (questions, candidates) =
                params::selection(leaf, form, &[], Some(&host.workspaces));
            if !questions.is_empty() {
                let more = params::selected(&self.ask(state, questions).await?, &candidates);
                values.extend(more);
            }
        }
        Ok(values)
    }
}

/// The state every question of the descent reads: the same bounded
/// conversation the router's own questions read.
fn state(ask: &CliAsk) -> Value {
    crate::classify::state_of(&ask.message, &ask.transcript, &[])
}

/// The conversation the model fills from, ending with the latest message.
fn input(ask: &CliAsk) -> Vec<Message> {
    let mut input = ask.transcript.clone();
    let ends_with_it = input
        .last()
        .is_some_and(|last| last.role == Role::User && last.text.trim() == ask.message.trim());
    if !ends_with_it {
        input.push(Message {
            role: Role::User,
            text: ask.message.clone(),
        });
    }
    input
}

impl CliRoute for CommandRoute {
    fn groups(&self) -> Vec<CliGroup> {
        self.tree
            .groups
            .iter()
            .map(|group| CliGroup {
                id: group.name.clone(),
                summary: descend::group_summary(group),
            })
            .collect()
    }

    fn recipients(&self) -> Vec<String> {
        let mut recipients = vec!["TypeSafe (Jev)".to_string()];
        recipients.extend(self.fill.recipients());
        recipients
    }

    fn propose<'a>(&'a self, ask: &'a CliAsk) -> BoxFuture<'a, Result<CliAnswer, SeamError>> {
        Box::pin(async move { Ok(self.outcome(ask).await?.answer()) })
    }
}

#[cfg(test)]
mod tests;
