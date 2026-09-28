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
//! binary), then Vertex (Vertex AI's OpenAI-compatible endpoint with the
//! operator's access token file) when it is configured, each with whether it has a usable login and whether the
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

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use atif::{Source, Step};
use microcoder_loop::capacity::{self, Connection, Kind, Provider, Refusal};
use microcoder_loop::failover::{
    self, Admitted, ClaudeLane, Failover, Journal, Lane, Refusing, VertexLane,
};
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

/// The word the door, the trace, and `coder doctor` name this executor by.
pub const WORD: &str = "microcoder";

/// The Codex model a turn generates with unless the operator names one.
pub const CODEX_MODEL: &str = microcoder_loop::MODEL;

/// The Claude model a turn generates with: Claude Code's `opus` alias.
pub const CLAUDE_MODEL: &str = microcoder_loop::claude::DEFAULT_ALIAS;

/// The Vertex model a turn generates with unless `CODER_VERTEX_MODEL`
/// names one.
pub const VERTEX_MODEL: &str = microcoder_loop::vertex::DEFAULT_MODEL;

/// The Vertex model Microcoder asks for, when Vertex is configured on this
/// host: `named` (from `CODER_VERTEX_MODEL`), else [`VERTEX_MODEL`] when
/// the Vertex token file exists. `None` leaves Vertex out of the providers.
#[must_use]
pub fn vertex_model(named: Option<String>, token_exists: bool) -> Option<String> {
    named
        .filter(|model| !model.trim().is_empty())
        .or_else(|| token_exists.then(|| VERTEX_MODEL.to_string()))
}

/// Whether the Vertex token file exists here, without reading it.
#[must_use]
pub fn vertex_token_exists() -> bool {
    microcoder_loop::vertex::token_path().is_some_and(|path| path.is_file())
}

/// The reasoning effort a Codex step asks for, as `microcoder` defaults.
pub const CODEX_EFFORT: &str = "medium";

/// Steps one turn may take.
pub const MAX_STEPS: usize = 40;

/// Dollars of model and Jev spend one turn may reach.
pub const MAX_USD: f64 = 2.0;

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
            Provider::Vertex => "Vertex",
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
/// preference order: Codex, then Claude, then Vertex when `vertex` names
/// its model ([`vertex_model`]). `model` names the Codex model when the
/// operator named one.
#[must_use]
pub fn lineup(model: Option<&str>, vertex: Option<&str>) -> Vec<(Provider, String)> {
    [
        Some((Provider::Codex, model.unwrap_or(CODEX_MODEL).to_string())),
        Some((Provider::Claude, CLAUDE_MODEL.to_string())),
        vertex.map(|model| (Provider::Vertex, model.to_string())),
    ]
    .into_iter()
    .flatten()
    .collect()
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
    Vertex(VertexLane),
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
/// Building one makes no model call.
fn provided(state: &ProviderState, session: &str) -> Result<Provided, String> {
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
            })))
        }
        Provider::Claude => microcoder_loop::claude::ClaudeGenerator::from_env(&state.model, None)
            .map(|generator| Provided::Claude(ClaudeLane::new(generator))),
        Provider::Vertex => microcoder_loop::vertex::VertexGenerator::from_env(&state.model, None)
            .map(|generator| Provided::Vertex(VertexLane::new(generator))),
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
    /// The loop's wall-clock bound, in seconds.
    pub max_seconds: u64,
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
            None => provided(state, &session),
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
        max_steps: Some(MAX_STEPS),
        max_seconds: turn.max_seconds,
        max_usd: MAX_USD,
        acceptance: false,
        route: Routing::Never,
        ask: true,
        ..Limits::default()
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
