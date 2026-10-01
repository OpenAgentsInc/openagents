//! A delegated turn answered by the Microcoder loop, in this process.
//!
//! The loop is `crates/microcoder-loop`'s [`microcoder_loop::run::run`], the
//! same loop `microcoder repository` runs for the task owner: Jev judges the
//! state, one structured model call returns the next commands, and the
//! commands run in the working directory inside a `coder-boundary`
//! boundary, read-only unless the turn's permit runs commands.
//!
//! # Which provider generates
//!
//! [`providers`] lists the providers in preference order, Codex (GPT-6 Luna
//! on the operator's Codex login), then Claude (through the `claude`
//! binary), then, last and always, Vertex through the OpenAgents cloud
//! ([`crate::cloud`]), the no-setup fallback that needs no login or token
//! on this host, each with whether it has a usable login and whether the
//! capacity book (`capacity.json` in the task store, the book the
//! auto-start policy reads) holds a refusal for it. The turn starts on the
//! first connected provider with capacity. When a provider refuses for a
//! usage or rate limit during the turn, [`microcoder_loop::failover`]
//! records the refusal with its reset time and generates the same step on
//! the next connected provider with capacity, and the turn's trace holds a
//! `route_switch` step. When none is left, the turn ends with one sentence
//! that names each provider and when it resets.
//!
//! No stronger model is routed to: the loop's acceptance tests and routing
//! are off here, as they are for repository runs.
//!
//! # A turn that works an issue
//!
//! [`respond`] runs `coder_delegate::issue`'s flow when a turn asks to work
//! a GitHub issue and the operator's permit lets work run: code finds the
//! issue references in the request and the conversation, and Jev chooses
//! among them or answers none. The flow checks out a new branch under
//! `~/.openagents/coder/issues/`, and its sessions (the work, a review, and
//! up to three fixes after the host's gate) each run as a Microcoder turn
//! in that checkout ([`IssueWorker`]), with the same providers and
//! failover. It ends in a draft pull request. With no provider that has
//! capacity, it ends before anything is checked out, with the plain
//! no-capacity sentence.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use atif::{Source, Step};
use microcoder_loop::capacity::{self, Connection, Kind, Provider, Refusal};
use microcoder_loop::failover::{self, Admitted, ClaudeLane, Failover, Journal, Lane, Refusing};
use microcoder_loop::models::{
    Basis, CodexGenerator, Generate, Generated, JevJudge, Judge, Judgment, NextAction, QuestionSet,
};
use microcoder_loop::run::{Ending, Event, Limits, Models, Observer, Route as Routing};
use microcoder_loop::state::{CommandResult, State};
use serde::Serialize;
use serde_json::{Value, json};

use super::{Delegated, Update};
use crate::generate::{GenerateError, Usage};
use crate::shell::{Outcome, Proposal, Status};
use coder_delegate::terminal;

/// The word the door, the trace, and `coder doctor` name this executor by.
pub const WORD: &str = "microcoder";

/// The Codex model a turn generates with unless the operator names one.
pub const CODEX_MODEL: &str = microcoder_loop::MODEL;

/// The Claude model a turn generates with: Claude Code's `opus` alias.
pub const CLAUDE_MODEL: &str = microcoder_loop::claude::DEFAULT_ALIAS;

/// The model the lineup names for the cloud fallback. The host can't
/// choose it: the OpenAgents cloud worker answers on its own lane and names
/// the model in each result.
pub const CLOUD_MODEL: &str = crate::cloud::MODEL;

/// The reasoning effort a Codex step asks for, as `microcoder` defaults.
pub const CODEX_EFFORT: &str = "medium";

// A turn has no step or time limit (#10103): it ends when the loop
// finishes or asks, when the person stops it, or when the loop's stuck
// guard finds it repeating a failed approach without progress. Its spend
// stays capped, below.

/// Dollars of model and Jev spend one turn may reach.
pub const MAX_USD: f64 = 2.0;

/// Dollars one session of the issue flow may reach.
pub const ISSUE_MAX_USD: f64 = 5.0;

/// One provider the loop may generate through, and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderState {
    pub provider: Provider,
    /// The model the loop asks this provider for.
    pub model: String,
    /// Whether this host has a usable login for it.
    pub connection: Connection,
    /// The refusal the capacity book holds for it now, if one holds.
    pub refusal: Option<Refusal>,
}

impl ProviderState {
    /// Whether a turn can start on it: connected, and no refusal holds.
    #[must_use]
    pub fn usable(&self) -> bool {
        self.connection.is_connected() && self.refusal.is_none()
    }

    /// The provider's login, in words.
    fn login(&self) -> &'static str {
        match self.provider {
            Provider::Codex => "the Codex login",
            Provider::Claude => "the Claude Code login",
            Provider::Vertex => "the OpenAgents cloud",
            Provider::Devin => "the Devin CLI login",
            Provider::OpenCode => "OpenCode",
            Provider::Grok => "the Grok Build login",
        }
    }

    /// Where it stands, in one clause without a subject: `has capacity`,
    /// `is out of its usage limit until …`, or why it can't be used.
    #[must_use]
    pub fn standing(&self) -> String {
        match (&self.connection, &self.refusal) {
            (Connection::Missing(why), _) => format!("can't be used ({why})"),
            (Connection::Connected, Some(refusal)) => blocked(refusal),
            (Connection::Connected, None) => "has capacity".to_string(),
        }
    }

    /// One sentence fragment: the login and where it stands.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{} {}", self.login(), self.standing())
    }
}

/// `is out of its usage limit until …` or `is rate limited until …`, the
/// recorded time. A refusal whose reset nobody reported says the time is a
/// hold rather than the provider's.
#[must_use]
pub fn blocked(refusal: &Refusal) -> String {
    let until = capacity::utc(refusal.until);
    let held = if refusal.resets_at.is_none() {
        " (held 30 minutes; the reset wasn't reported)"
    } else {
        ""
    };
    match refusal.kind {
        Kind::UsageLimit => format!("is out of its usage limit until {until}{held}"),
        Kind::RateLimit => format!("is rate limited until {until}{held}"),
    }
}

/// The providers a turn may use and the model each is asked for, in
/// preference order: Codex, then Claude, then Vertex through the
/// OpenAgents cloud. The cloud is always there and always last: it is the
/// fallback when nothing on this host is configured or has capacity.
/// `model` names the Codex model when the operator named one.
#[must_use]
pub fn lineup(model: Option<&str>) -> Vec<(Provider, String)> {
    vec![
        (Provider::Codex, model.unwrap_or(CODEX_MODEL).to_string()),
        (Provider::Claude, CLAUDE_MODEL.to_string()),
        (Provider::Vertex, CLOUD_MODEL.to_string()),
    ]
}

/// Where each of `lineup`'s providers stands, in its order. `probe` says
/// whether each is connected; `book` is the capacity book's directory,
/// read at `now`.
#[must_use]
pub fn providers(
    lineup: &[(Provider, String)],
    book: &std::path::Path,
    probe: &dyn Fn(Provider) -> Connection,
    now: u64,
) -> Vec<ProviderState> {
    let recorded = capacity::Book::load(book);
    lineup
        .iter()
        .cloned()
        .map(|(provider, model)| ProviderState {
            provider,
            model,
            connection: probe(provider),
            refusal: recorded.blocking(provider, now).cloned(),
        })
        .collect()
}

/// Every provider, in one sentence: why each can't answer, and when those
/// out of capacity reset.
#[must_use]
pub fn none_left(providers: &[ProviderState]) -> String {
    let parts: Vec<String> = providers.iter().map(ProviderState::describe).collect();
    format!(
        "Microcoder has no provider to answer with: {}.",
        join(&parts)
    )
}

/// `a`, `a and b`, or `a, b, and c`.
fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// A route as failover records it.
#[derive(Clone, Debug, Serialize)]
struct Route {
    provider: Provider,
    model: String,
}

impl Admitted for Route {
    fn provider(&self) -> Option<Provider> {
        Some(self.provider)
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// One scripted reply, for a test: the next action, or the refusal the
/// provider meets.
pub type Scripted = Result<NextAction, Refusal>;

/// A provider's scripted replies, shared across a door's turns so each
/// reply plays once.
pub type Script = std::sync::Arc<std::sync::Mutex<VecDeque<Scripted>>>;

/// A provider's scripted replies, played once each, in order.
struct ScriptedLane {
    model: String,
    replies: Script,
    refusal: RefCell<Option<Refusal>>,
}

impl Generate for ScriptedLane {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        let next = self
            .replies
            .lock()
            .ok()
            .and_then(|mut replies| replies.pop_front());
        let (action, usd) = match next {
            Some(Ok(action)) => (Ok(action), 0.001),
            Some(Err(refusal)) => {
                *self.refusal.borrow_mut() = Some(refusal);
                (
                    Err("the provider refused for a usage limit".to_string()),
                    0.0,
                )
            }
            None => (Err("the script is spent".to_string()), 0.0),
        };
        Generated {
            action,
            model: self.model.clone(),
            prompt_tokens: 100,
            completion_tokens: 10,
            usd: Some(usd),
            known_usd: usd,
            cost_unknown: None,
            usd_upper: Some(usd),
            cost_basis: Basis::ListPrice,
            milliseconds: 1,
        }
    }
}

impl Lane for ScriptedLane {
    fn refusal(&self) -> Option<Refusal> {
        self.refusal.borrow_mut().take()
    }
}

/// One provider's generator.
enum Provided {
    Codex(Box<CodexGenerator<Refusing<codex_transport::codex::CodexTransport>>>),
    Claude(ClaudeLane),
    Vertex(crate::cloud::CloudLane),
    Scripted(ScriptedLane),
}

impl Generate for Provided {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        match self {
            Provided::Codex(lane) => lane.generate(system, prompt).await,
            Provided::Claude(lane) => lane.generate(system, prompt).await,
            Provided::Vertex(lane) => lane.generate(system, prompt).await,
            Provided::Scripted(lane) => lane.generate(system, prompt).await,
        }
    }
}

impl Lane for Provided {
    fn refusal(&self) -> Option<Refusal> {
        match self {
            Provided::Codex(lane) => lane.refusal(),
            Provided::Claude(lane) => lane.refusal(),
            Provided::Vertex(lane) => lane.refusal(),
            Provided::Scripted(lane) => lane.refusal(),
        }
    }
}

/// The generator for one connected provider, or why it can't be built.
/// Building one makes no model call. `cloud` is the door the cloud lane
/// uses in place of the OpenAgents relay, for a test.
fn provided(
    state: &ProviderState,
    session: &str,
    cloud: Option<&std::sync::Arc<crate::generate::Door>>,
) -> Result<Provided, String> {
    match state.provider {
        Provider::Codex => {
            let login =
                codex_transport::codex::Login::default_path().ok_or("no Codex login path")?;
            let transport = codex_transport::codex::CodexTransport::new(login, session)
                .map_err(|error| error.to_string())?;
            Ok(Provided::Codex(Box::new(CodexGenerator {
                transport: Refusing::new(transport),
                model: state.model.clone(),
                effort: Some(CODEX_EFFORT.to_string()),
                cache_key: session.to_string(),
                images: Vec::new(),
            })))
        }
        Provider::Claude => microcoder_loop::claude::ClaudeGenerator::from_env(&state.model, None)
            .map(|generator| Provided::Claude(ClaudeLane::new(generator))),
        Provider::Vertex => {
            let door = match cloud {
                Some(door) => std::sync::Arc::clone(door),
                None => std::sync::Arc::new(crate::generate::Door::Relay(Box::new(
                    crate::cloud::door(&super::env_value)?,
                ))),
            };
            Ok(Provided::Vertex(crate::cloud::CloudLane::new(door)))
        }
        // Devin is a whole coding agent; the loop's steps don't generate
        // through it, and the lineup never names it.
        Provider::Devin => Err("the loop does not generate through Devin".into()),
        Provider::OpenCode => Err("the loop does not generate through OpenCode".into()),
        Provider::Grok => Err("the loop does not generate through Grok Build".into()),
    }
}

/// Jev, or a stand-in that says Jev couldn't answer when there's no key.
enum Jev {
    Live(JevJudge),
    Absent(String),
}

impl Judge for Jev {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        match self {
            Jev::Live(judge) => judge.judge(set, state).await,
            Jev::Absent(why) => Judgment {
                error: Some(why.clone()),
                ..Judgment::free()
            },
        }
    }
}

/// The turn's steps, in order: the loop's observations and failover's
/// switches.
#[derive(Default)]
struct Steps(RefCell<Vec<Step>>);

impl Journal for Steps {
    fn append(&self, step: &Step) {
        self.0.borrow_mut().push(step.clone());
    }
}

/// What the loop reports, turned into the turn's updates and steps.
struct Watch<'a> {
    steps: &'a Steps,
    on: Rc<dyn Fn(Update) + 'a>,
    /// The last action's reply, which is the turn's answer when it ends.
    reply: String,
    /// The model of the last generation that answered.
    model: Option<String>,
    usage: Usage,
    commands: usize,
}

impl Observer for Watch<'_> {
    fn event(&mut self, seconds: f64, event: &Event) {
        self.steps.append(
            &Step::said(Source::System, "Microcoder loop observation.")
                .noting("microcoder", json!({"seconds":seconds,"event":event})),
        );
        match event {
            Event::Judged { step, judgment } => {
                let line = match &judgment.error {
                    Some(why) => format!("jev ▸ step {step}: no judgment ({why})"),
                    None => format!(
                        "jev ▸ step {step}: {}",
                        judgment
                            .answers
                            .iter()
                            .map(|(id, p)| format!("{id} {p:.2}"))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    ),
                };
                (self.on)(Update::Line(line));
            }
            Event::Generated {
                step, generated, ..
            } => {
                self.usage.input_tokens += generated.prompt_tokens;
                self.usage.output_tokens += generated.completion_tokens;
                match &generated.action {
                    Ok(action) => {
                        self.model = Some(generated.model.clone());
                        self.reply.clone_from(&action.reply);
                        (self.on)(Update::Line(format!(
                            "step {step} ▸ {}: {}",
                            generated.model,
                            action.rationale.trim()
                        )));
                    }
                    Err(why) => (self.on)(Update::Line(format!(
                        "step {step} ▸ {} gave no usable reply: {why}",
                        generated.model
                    ))),
                }
            }
            Event::Ran { result, .. } => {
                self.commands += 1;
                let (proposal, outcome) = outcome(result);
                (self.on)(Update::Proposed(proposal));
                (self.on)(Update::Ran(outcome));
            }
            _ => {}
        }
    }
}

/// A finished command in the shell's terms.
fn outcome(result: &CommandResult) -> (Proposal, Outcome) {
    let proposal = Proposal {
        command: result.command.clone(),
        why: "microcoder ran it".to_string(),
    };
    let status = match (result.exit, result.timed_out) {
        (_, true) => Status::Failed("the command reached its deadline".to_string()),
        (Some(code), false) => Status::Exit(code),
        (None, false) => Status::Failed("the command reported no exit code".to_string()),
    };
    let outcome = Outcome {
        proposal: proposal.clone(),
        status,
        bytes: result.output.len() as u64,
        output: result.output.clone(),
        elapsed: std::time::Duration::from_secs_f64(result.seconds.max(0.0)),
    };
    (proposal, outcome)
}

/// The prompt for a turn: the request, after the conversation before it.
fn task(request: &str, earlier: &str) -> String {
    if earlier.trim().is_empty() {
        return request.to_string();
    }
    format!(
        "This continues an earlier conversation. Earlier messages, oldest first:\n{earlier}\n\n\
         The user's new message:\n{request}"
    )
}

/// One turn, as the door hands it to the loop.
#[derive(Clone)]
pub struct Turn {
    pub request: String,
    pub earlier: String,
    pub read_only: bool,
    pub workdir: PathBuf,
    pub jev: Option<jev::Client>,
    /// Why there's no Jev client, when there's none.
    pub jev_missing: String,
    /// The capacity book's directory.
    pub book: PathBuf,
    /// Every provider, in preference order.
    pub providers: Vec<ProviderState>,
    /// Scripted replies per provider in place of real calls, for a test.
    pub script: Option<Vec<(Provider, Script)>>,
    /// The door the cloud lane talks to in place of the OpenAgents relay,
    /// for a test.
    pub cloud: Option<std::sync::Arc<crate::generate::Door>>,
    /// The loop's spend bound, in dollars. The loop has no step or time
    /// bound.
    pub max_usd: f64,
    /// Whether a step may end the turn by asking the user.
    pub ask: bool,
    /// Whether a request to work a GitHub issue may start the issue flow.
    /// The operator's permit decides, not the turn's route.
    pub issues: bool,
    /// The clock, in Unix seconds.
    pub now: fn() -> u64,
}

/// Runs one turn through the loop. `on` hears the turn's updates.
pub async fn answer(turn: Turn, on: Rc<dyn Fn(Update)>) -> Delegated {
    let failed = |failure: GenerateError, steps: Vec<Step>, summary: Value| Delegated {
        text: String::new(),
        failure: Some(failure),
        usage: None,
        cost_usd: Some(0.0),
        steps,
        summary,
        model: WORD.to_string(),
        commands: 0,
    };
    let session = format!("coder-microcoder-{}-{}", std::process::id(), atif::now_ms());
    // Only connected providers get a lane; failover skips those whose
    // capacity the book says is gone.
    let mut lanes = Vec::new();
    let mut skipped = Vec::new();
    for state in turn
        .providers
        .iter()
        .filter(|state| state.connection.is_connected())
    {
        let route = Route {
            provider: state.provider,
            model: state.model.clone(),
        };
        let built = match &turn.script {
            Some(script) => Ok(Provided::Scripted(ScriptedLane {
                model: state.model.clone(),
                replies: script
                    .iter()
                    .find(|(provider, _)| *provider == state.provider)
                    .map(|(_, replies)| std::sync::Arc::clone(replies))
                    .unwrap_or_default(),
                refusal: RefCell::new(None),
            })),
            None => provided(state, &session, turn.cloud.as_ref()),
        };
        match built {
            Ok(lane) => lanes.push((route, lane)),
            Err(why) => skipped.push(json!({"route":route,"unavailable":why})),
        }
    }
    let summary = |ending: Value, model: &str, extra: Value| {
        json!({
            "agent": WORD,
            "model": model,
            "providers": turn.providers.iter().map(|state| json!({
                "provider": state.provider,
                "model": state.model,
                "standing": state.standing(),
            })).collect::<Vec<_>>(),
            "unavailable": skipped,
            "ending": ending,
            "detail": extra,
        })
    };
    let steps = Steps::default();
    if !skipped.is_empty() {
        steps.append(
            &Step::said(
                Source::System,
                "Some providers are connected but their clients can't be built here.",
            )
            .noting("routes_unavailable", json!(skipped)),
        );
    }
    let env = match microcoder_loop::env::Bounded::new(turn.workdir.clone(), !turn.read_only) {
        Ok(env) => env,
        Err(why) => {
            return failed(
                GenerateError::Stream(format!("microcoder could not run: {why}")),
                steps.0.into_inner(),
                summary(json!("unbounded"), WORD, json!(why)),
            );
        }
    };
    let generator = Failover::new(&steps, turn.book.clone(), lanes, turn.now);
    if generator.route().is_none() {
        let providers = refreshed(&turn.providers, &generator.refusals(), &turn.book, turn.now);
        drop(generator);
        return failed(
            GenerateError::NoCapacity(none_left(&providers)),
            steps.0.into_inner(),
            summary(json!("no_capacity"), WORD, Value::Null),
        );
    }
    generator.record_start();
    let judge = match turn.jev {
        Some(client) => Jev::Live(JevJudge { client }),
        None => Jev::Absent(turn.jev_missing.clone()),
    };
    let set = microcoder_loop::models::question_set();
    let routing = microcoder_loop::models::route_set();
    let limits = Limits {
        max_usd: turn.max_usd,
        acceptance: false,
        route: Routing::Never,
        ask: turn.ask,
        ..Limits::unbounded()
    };
    let state = State {
        task: task(&turn.request, &turn.earlier),
        environment: format!(
            "Working directory: {}. {}",
            turn.workdir.display(),
            if turn.read_only {
                "This turn only reads: commands run in a boundary that refuses every write to the \
                 working directory, so answer from what you can read."
            } else {
                "Commands may write inside the working directory and a private temporary \
                 directory, and nowhere else."
            }
        ),
        ..State::default()
    };
    let mut watch = Watch {
        steps: &steps,
        on,
        reply: String::new(),
        model: None,
        usage: Usage::default(),
        commands: 0,
    };
    let (_, outcome) = microcoder_loop::run::run(
        state,
        &turn.request,
        &env,
        &Models {
            generator: &generator,
            judge: &judge,
            set: &set,
            route: &routing,
            strong: None,
            knowledge: None,
        },
        &limits,
        &mut watch,
    )
    .await;
    let refusals = generator.refusals();
    let model = watch
        .model
        .clone()
        .or_else(|| generator.route().map(|route| route.model.clone()))
        .unwrap_or_else(|| WORD.to_string());
    let failure = match &outcome.ending {
        Ending::Finished | Ending::Asked { .. } if !watch.reply.trim().is_empty() => None,
        Ending::Finished | Ending::Asked { .. } => Some(GenerateError::Stream(
            "microcoder finished without a reply".to_string(),
        )),
        Ending::NoCapacity { .. } => Some(GenerateError::NoCapacity(none_left(&refreshed(
            &turn.providers,
            &refusals,
            &turn.book,
            turn.now,
        )))),
        ending => Some(GenerateError::Stream(format!(
            "microcoder stopped before it answered: {}",
            serde_json::to_string(ending).unwrap_or_default()
        ))),
    };
    let text = watch.reply.trim().to_string();
    let usage = watch.usage;
    let commands = watch.commands;
    drop(watch);
    let summary = summary(
        json!(outcome.ending),
        &model,
        json!({"steps": outcome.steps, "usd": outcome.usd, "usd_upper": outcome.usd_upper,
            "refusals": refusals}),
    );
    drop(generator);
    Delegated {
        text,
        failure,
        usage: Some(usage),
        cost_usd: outcome.usd,
        steps: steps.0.into_inner(),
        summary,
        model,
        commands,
    }
}

/// The providers as they stand after a turn: the book read again, and the
/// turn's own refusals in case the book could not be written.
fn refreshed(
    providers: &[ProviderState],
    refusals: &[Refusal],
    book: &std::path::Path,
    now: fn() -> u64,
) -> Vec<ProviderState> {
    let at = now();
    let recorded = capacity::Book::load(book);
    providers
        .iter()
        .map(|state| {
            let seen = refusals
                .iter()
                .filter(|refusal| refusal.provider == state.provider && refusal.holds(at))
                .max_by_key(|refusal| refusal.until);
            ProviderState {
                refusal: recorded.blocking(state.provider, at).or(seen).cloned(),
                ..state.clone()
            }
        })
        .collect()
}

/// The Unix clock a live turn uses.
#[must_use]
pub fn now() -> u64 {
    failover::unix_now()
}

/// The request a turn is, in the terms of `coder_delegate`'s terminal
/// turn, which the issue flow reads.
fn flow_request(turn: &Turn) -> terminal::Request {
    terminal::Request {
        workdir: turn.workdir.clone(),
        request: turn.request.clone(),
        earlier: turn.earlier.clone(),
        resume: None,
        read_only: turn.read_only,
        clarify: false,
        agent: coder_delegate::delegate::Agent::Codex,
        model: None,
        binary: None,
        credential: coder_delegate::delegate::Credential::Missing,
        jev: turn.jev.clone(),
        artifacts: std::env::temp_dir(),
        issues: turn.issues,
        issue: false,
        review: false,
        extra: (),
    }
}

/// Answers a turn: the issue flow when the turn asks to work a GitHub
/// issue and [`Turn::issues`] lets it, and one Microcoder turn otherwise.
pub async fn respond(turn: Turn, on: Rc<dyn Fn(Update)>) -> Delegated {
    if !turn.issues || turn.jev.is_none() {
        return answer(turn, on).await;
    }
    let recorder = coder_delegate::record::Recorder::default();
    let request = flow_request(&turn);
    let chosen = {
        let heard = on.clone();
        let _captured = coder_delegate::say::capture(Box::new(move |line| {
            heard(Update::Line(line.trim().to_string()));
        }));
        coder_delegate::issue::asked(&request, &recorder).await
    };
    match chosen {
        Some(reference) => issue(turn, &request, reference, on, recorder).await,
        None => {
            let mut done = answer(turn, on).await;
            let mut steps = recorder.steps();
            steps.append(&mut done.steps);
            done.steps = steps;
            done
        }
    }
}

/// Runs the issue flow for `reference` on Microcoder.
pub(crate) async fn issue(
    turn: Turn,
    request: &terminal::Request,
    reference: coder_delegate::issue::Reference,
    on: Rc<dyn Fn(Update)>,
    recorder: coder_delegate::record::Recorder,
) -> Delegated {
    let number = reference.number;
    // A flow no provider can work checks nothing out.
    if !turn.providers.iter().any(ProviderState::usable) {
        return Delegated {
            text: String::new(),
            failure: Some(GenerateError::NoCapacity(none_left(&turn.providers))),
            usage: None,
            cost_usd: Some(0.0),
            steps: recorder.steps(),
            summary: json!({"agent": WORD, "issue": number, "ending": "no_capacity"}),
            model: WORD.to_string(),
            commands: 0,
        };
    }
    let heard = on.clone();
    let _captured = coder_delegate::say::capture(Box::new(move |line| {
        heard(Update::Line(line.trim().to_string()));
    }));
    coder_delegate::say::say!(
        "route ▸ issue #{number} asks for work, so Coder works it on a new branch with Microcoder"
    );
    let worker = IssueWorker::new(turn, on);
    let progress: Rc<dyn Fn(terminal::Progress)> = Rc::new(|_| {});
    let answer = coder_delegate::issue::run(&worker, request, reference, progress, &recorder).await;
    worker.delegated(&answer, number)
}

/// Microcoder as the issue flow's worker: each session is a Microcoder
/// turn in the flow's checkout, on the turn's providers with failover.
pub struct IssueWorker {
    turn: Turn,
    on: Rc<dyn Fn(Update)>,
    /// The no-capacity sentence of the first session, when no session had
    /// worked before it.
    no_capacity: RefCell<Option<String>>,
    commands: std::cell::Cell<usize>,
    usage: RefCell<Usage>,
    model: RefCell<Option<String>>,
    /// Whether any session did work: a no-capacity ending before any
    /// ends the flow with the no-capacity sentence.
    worked: std::cell::Cell<bool>,
}

impl IssueWorker {
    /// A worker for `turn`'s providers, reporting to `on`.
    #[must_use]
    pub fn new(turn: Turn, on: Rc<dyn Fn(Update)>) -> Self {
        IssueWorker {
            turn,
            on,
            no_capacity: RefCell::new(None),
            commands: std::cell::Cell::new(0),
            usage: RefCell::new(Usage::default()),
            model: RefCell::new(None),
            worked: std::cell::Cell::new(false),
        }
    }

    /// The flow's answer in the door's terms.
    #[must_use]
    pub fn delegated(&self, answer: &terminal::Answer, number: u64) -> Delegated {
        let failure = self
            .no_capacity
            .borrow()
            .clone()
            .map(GenerateError::NoCapacity);
        let usage = *self.usage.borrow();
        Delegated {
            text: answer.report.summary.result.clone().unwrap_or_default(),
            failure,
            usage: Some(usage),
            cost_usd: answer.cost_usd(),
            steps: answer.steps.clone(),
            summary: json!({
                "agent": WORD,
                "issue": number,
                "status": answer.report.status.word(),
                "stuck": answer.stuck,
                "summaries": answer.summaries,
            }),
            model: self
                .model
                .borrow()
                .clone()
                .unwrap_or_else(|| WORD.to_string()),
            commands: self.commands.get(),
        }
    }
}

impl coder_delegate::issue::Worker<()> for IssueWorker {
    async fn answer(
        &self,
        request: &terminal::Request,
        _on: Rc<dyn Fn(terminal::Progress)>,
    ) -> terminal::Answer {
        use coder_delegate::delegate::{Agent, Briefing, Report, Status, Summary};
        let directions = if request.review {
            format!(
                "{} {}",
                terminal::ISSUE_DIRECTIONS,
                terminal::REVIEW_DIRECTIONS
            )
        } else {
            terminal::ISSUE_DIRECTIONS.to_string()
        };
        let task = format!("{}\n\n{directions}", request.request.trim());
        let turn = Turn {
            request: task.clone(),
            earlier: request.earlier.clone(),
            read_only: request.read_only,
            workdir: request.workdir.clone(),
            max_usd: ISSUE_MAX_USD,
            ask: false,
            issues: false,
            ..self.turn.clone()
        };
        let done = answer(turn, self.on.clone()).await;
        self.commands.set(self.commands.get() + done.commands);
        if let Some(usage) = done.usage {
            let mut total = self.usage.borrow_mut();
            total.input_tokens += usage.input_tokens;
            total.output_tokens += usage.output_tokens;
        }
        let status = match &done.failure {
            None => {
                self.worked.set(true);
                Status::Answered
            }
            Some(GenerateError::NoCapacity(sentence)) => {
                if !self.worked.get() && self.no_capacity.borrow().is_none() {
                    *self.no_capacity.borrow_mut() = Some(sentence.clone());
                }
                Status::Refused(sentence.clone())
            }
            Some(failure) => Status::Harness(failure.to_string()),
        };
        let provider = self
            .turn
            .providers
            .iter()
            .find(|state| state.model == done.model);
        if done.model != WORD {
            *self.model.borrow_mut() = Some(done.model.clone());
        }
        let text = if done.text.is_empty() {
            done.failure
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        } else {
            done.text.clone()
        };
        terminal::Answer {
            report: Report {
                status,
                summary: Summary {
                    has_result: true,
                    result: Some(text.clone()),
                    is_error: Some(done.failure.is_some()),
                    total_cost_usd: done.cost_usd,
                    ..Summary::default()
                },
                milliseconds: 0,
                stderr: String::new(),
                stream: None,
            },
            briefing: Briefing {
                text: task,
                cap: 0,
                included: Vec::new(),
                omitted: Vec::new(),
            },
            steps: done.steps,
            usage: json!({"cost": {"amount_usd": done.cost_usd}}),
            session_id: None,
            resumed: false,
            agent: match provider.map(|state| state.provider) {
                Some(Provider::Claude) => Agent::ClaudeCode,
                _ => Agent::Codex,
            },
            model: done.model,
            boundary: if request.read_only {
                "read-only"
            } else {
                "workspace-writable"
            }
            .to_string(),
            summaries: if done.failure.is_none() && !text.trim().is_empty() {
                vec![text]
            } else {
                Vec::new()
            },
            stuck: false,
        }
    }

    fn runs(&self) -> Option<PathBuf> {
        coder_delegate::credentials::openagents_dir().map(|home| home.join("coder").join("issues"))
    }
}
