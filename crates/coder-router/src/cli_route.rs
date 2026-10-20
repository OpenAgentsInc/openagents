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
//! runs each tool call. The phone runs a tapped offer through the first two
//! (`crates/openagents-mobile/src/cli_run.rs`); the chat worker wires
//! [`CommandRoute`] as its CLI seam.

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

/// The command words of `argv` (group and subcommands), without values,
/// by the tree's names.
fn command_words(argv: &[String]) -> Vec<String> {
    let argv = tree::tree_argv(argv);
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

    /// The level-0 answer's options, most likely first, `none` included.
    ///
    /// # Errors
    ///
    /// A Jev failure.
    pub async fn group_ranked(&self, ask: &CliAsk) -> Result<Vec<(String, f64)>, SeamError> {
        let state = state(ask);
        let response = self
            .ask(
                &state,
                jev::Questions::new()
                    .with(descend::GROUP_QUESTION, descend::group_question(self.tree)),
            )
            .await?;
        Ok(descend::ranked(&response, descend::GROUP_QUESTION))
    }

    /// Descend from `ask.group`, fill the chosen command, and check it.
    ///
    /// # Errors
    ///
    /// A Jev or model failure; the router then answers with the model
    /// alone.
    pub async fn outcome(&self, ask: &CliAsk) -> Result<Outcome, SeamError> {
        let state = state(ask);
        let (group, leaf, trail) = match self.descend(&state, ask).await? {
            Descent::Found { group, leaf, trail } => (group, leaf, trail),
            Descent::NotOffered { leaf, trail } => {
                return Ok(Outcome::NotOffered {
                    path: leaf.path.clone(),
                    effect: leaf.effect,
                    trail,
                });
            }
            Descent::Nothing { trail } => return Ok(Outcome::NoCommand { trail }),
        };
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
        // Offered under the names every phone and computer knows.
        let argv = tree::wire_argv(&argv);
        Ok(Outcome::Proposal {
            execution: execution(&argv, runs_on),
            argv,
            effect: leaf.effect,
            runs_on,
            trail,
        })
    }

    /// Descend from `ask.group`, and from each group in `ask.also`, with a
    /// beam of at most [`descend::BEAM`] open paths: each level asks one
    /// question per open path in one request, keeps the argmax and a
    /// second choice at [`descend::BEAM_FLOOR`] or above, and scores a
    /// path by the geometric mean of its edges. The best path that reaches
    /// a command, scores at least [`descend::LEVEL_CONFIDENCE`], and has no
    /// edge under the floor is the answer; among those, one the surface may
    /// be offered wins over one it may not.
    async fn descend(&self, state: &Value, ask: &CliAsk) -> Result<Descent<'_>, SeamError> {
        let mut groups = vec![ask.group.as_str()];
        for other in &ask.also {
            if !groups.contains(&other.as_str()) {
                groups.push(other.as_str());
            }
        }
        let mut open: Vec<Path<'_>> = groups
            .iter()
            .filter_map(|name| self.tree.group(name))
            .map(|group| Path {
                group,
                node: group,
                words: vec![group.name.clone()],
                trail: vec![Step {
                    at: Vec::new(),
                    choice: group.name.clone(),
                    p: None,
                }],
                log_p: 0.0,
                edges: 0,
                weakest: 1.0,
            })
            .collect();
        let mut trail = open.first().map_or_else(
            || {
                vec![Step {
                    at: Vec::new(),
                    choice: ask.group.clone(),
                    p: None,
                }]
            },
            |path| path.trail.clone(),
        );
        let mut done: Vec<Path<'_>> = Vec::new();
        while !open.is_empty() {
            let (ready, asking): (Vec<Path<'_>>, Vec<Path<'_>>) = open
                .into_iter()
                .partition(|path| path.node.children.is_empty());
            done.extend(ready);
            if asking.is_empty() {
                break;
            }
            let mut questions = jev::Questions::new();
            for (k, path) in asking.iter().enumerate() {
                questions = questions.with(
                    descend::beam_question(k),
                    descend::level_question(&path.words, path.node),
                );
            }
            let response = self.ask(state, questions).await?;
            let mut next = Vec::new();
            for (k, path) in asking.iter().enumerate() {
                let ranked = descend::ranked(&response, &descend::beam_question(k));
                if k == 0
                    && let Some((choice, p)) = ranked.first()
                {
                    // The record of the most likely path, however it ends.
                    trail = path.trail.clone();
                    trail.push(Step {
                        at: path.words.clone(),
                        choice: choice.clone(),
                        p: Some(*p),
                    });
                }
                for (rank, (choice, p)) in ranked.into_iter().take(descend::BEAM).enumerate() {
                    if choice == "none" || (rank > 0 && p < descend::BEAM_FLOOR) {
                        continue;
                    }
                    let mut step = path.clone();
                    step.trail.push(Step {
                        at: path.words.clone(),
                        choice: choice.clone(),
                        p: Some(p),
                    });
                    step.log_p += p.max(f64::MIN_POSITIVE).ln();
                    step.edges += 1;
                    step.weakest = step.weakest.min(p);
                    if choice == descend::SELF {
                        done.push(step);
                        continue;
                    }
                    let Some(child) = path.node.child(&choice) else {
                        continue;
                    };
                    step.node = child;
                    step.words.push(choice);
                    if child.children.is_empty() {
                        done.push(step);
                    } else {
                        next.push(step);
                    }
                }
            }
            next.sort_by(|a, b| b.score().total_cmp(&a.score()));
            next.truncate(descend::BEAM);
            open = next;
        }
        let mut found: Vec<Path<'_>> = done
            .into_iter()
            .filter(|path| {
                path.node.leaf.is_some()
                    && path.score() >= descend::LEVEL_CONFIDENCE
                    && path.weakest >= descend::BEAM_FLOOR
            })
            .collect();
        found.sort_by(|a, b| b.score().total_cmp(&a.score()));
        let offered = found.iter().position(|path| {
            path.node
                .leaf
                .as_ref()
                .is_some_and(|leaf| gate::offered(leaf, ask.surface))
        });
        match (offered, found.first()) {
            (Some(at), _) => {
                let path = found.swap_remove(at);
                let leaf = path
                    .node
                    .leaf
                    .as_ref()
                    .expect("a found path ends at a command");
                Ok(Descent::Found {
                    group: path.group,
                    leaf,
                    trail: path.trail,
                })
            }
            (None, Some(path)) => Ok(Descent::NotOffered {
                leaf: path
                    .node
                    .leaf
                    .as_ref()
                    .expect("a found path ends at a command"),
                trail: path.trail.clone(),
            }),
            (None, None) => Ok(Descent::Nothing { trail }),
        }
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

/// One path of the descent's beam.
#[derive(Clone)]
struct Path<'t> {
    group: &'t tree::Node,
    node: &'t tree::Node,
    words: Vec<String>,
    trail: Vec<Step>,
    log_p: f64,
    edges: usize,
    weakest: f64,
}

impl Path<'_> {
    /// The geometric mean of the path's edge probabilities; 1 for a group
    /// that is itself the only command, which no level question asks.
    fn score(&self) -> f64 {
        if self.edges == 0 {
            return 1.0;
        }
        (self.log_p / self.edges as f64).exp()
    }
}

/// Where the descent ended.
enum Descent<'t> {
    Found {
        group: &'t tree::Node,
        leaf: &'t Leaf,
        trail: Vec<Step>,
    },
    NotOffered {
        leaf: &'t Leaf,
        trail: Vec<Step>,
    },
    Nothing {
        trail: Vec<Step>,
    },
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
                tree: Some(descend::group_tree(group)),
            })
            .collect()
    }

    /// The model that fills free text. Jev is not listed: the router
    /// names TypeSafe for every routed turn already.
    fn recipients(&self) -> Vec<String> {
        self.fill.recipients()
    }

    fn propose<'a>(&'a self, ask: &'a CliAsk) -> BoxFuture<'a, Result<CliAnswer, SeamError>> {
        Box::pin(async move { Ok(self.outcome(ask).await?.answer()) })
    }
}

#[cfg(test)]
mod tests;
