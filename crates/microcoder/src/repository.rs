//! Opt-in repository execution through the common task owner.
//!
//! This profile runs the existing loop with model-written acceptance, routing,
//! and knowledge explicitly disabled. Independent checks remain host work.
//!
//! # Provider failover
//!
//! A grant names a primary route (provider, model, effort, endpoint) and may
//! admit fallback routes in the owner's preference order. When a generation
//! fails because the provider refused for a usage or rate limit, the refusal
//! is recorded in the task store's capacity book
//! ([`coder::task::capacity`]), a System step with a `route_switch`
//! extension records the switch, and the same step is generated again on the
//! next admitted route whose provider has capacity
//! ([`microcoder_loop::failover`]). When none has, the loop
//! ends with [`Ending::NoCapacity`] and the earliest reset, and the task's
//! result ending is `no_capacity`. The step's cost adds every attempt's cost,
//! so failover keeps the known, unknown, and upper-bound figures honest.
//!
//! # A reply as it is written
//!
//! The model writes a step's `reply` first ([`crate::models::next_action_schema`]),
//! and both native routes stream the action's JSON text as it is written.
//! [`Replies`] reads the reply out of that text ([`microcoder_loop::reply`])
//! and appends each paragraph to the transcript as soon as it is whole, and
//! the rest the moment the reply's string closes, as a `replying` loop
//! event, so a device reading the transcript shows the reply's first words
//! long before the step ends. The step's `generated` event then names how
//! many bytes of its reply were already shown (`reply_streamed`), and only
//! when the final reply begins with exactly those bytes; a reader shows
//! only the rest. A reader that does not know `replying` shows nothing for
//! it and the whole reply at the end, as before.
//!
//! # A warm model process
//!
//! A `claude` route's binary loads before it can take a prompt. The loop
//! starts it with the step's system text before the first step and while a
//! step's commands run ([`microcoder_loop::claude::ClaudeGenerator::warm`]),
//! so a later step's call begins with a loaded binary. A waiting binary has
//! no prompt and makes no model request; one that is never used is killed
//! when its route is dropped.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use atif::{Source, Step};
use coder::task::adapter::Route as GrantRoute;
use coder::task::autostart::StartCause;
use coder::task::capacity::{self, Provider, Refusal};
use coder::task::{self, adapter::Host};
use microcoder_loop::failover::{Failover, Journal, refused_generation};
pub use microcoder_loop::failover::{Lane, Plain};
use serde_json::{Value, json};

use crate::env::Env;
use crate::models::{Generate, Generated, Judge, Judgment, QuestionSet};
use crate::run::{Ending, Event, Limits, Models, Observer, Route};
use crate::state::{CommandResult, State, cut};
use microcoder_loop::reply::{Tap, settled};

pub struct Repository<'a> {
    pub host: &'a Host,
}

impl Env for Repository<'_> {
    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        match self.host.command(command, deadline).await {
            Ok(result) => CommandResult {
                command: command.into(),
                exit: result.exit,
                timed_out: result.timed_out,
                seconds: result.seconds,
                output: cut(
                    &result.output,
                    crate::state::OUTPUT_HEAD,
                    crate::state::OUTPUT_TAIL,
                ),
            },
            Err(error) => CommandResult {
                command: command.into(),
                exit: None,
                timed_out: false,
                seconds: 0.0,
                output: format!("The repository host refused the command: {error}"),
            },
        }
    }

    async fn read(&self, path: &str) -> Option<String> {
        let path = if self.host.configuration().container.is_some() {
            Path::new(path)
                .strip_prefix("/workspace")
                .ok()
                .and_then(Path::to_str)
                .unwrap_or(path)
        } else {
            path
        };
        self.host
            .read(path, crate::env::FILE_MAX * 4)
            .ok()
            .flatten()
            .map(|bytes| cut(&String::from_utf8_lossy(&bytes), crate::env::FILE_MAX, 0))
    }

    fn stopped(&self) -> bool {
        self.host.cancelled()
    }

    fn steering(&self) -> Vec<String> {
        self.host.steering()
    }
}

/// What an interrupted or unsent call says: the task stopped, not the
/// provider failed.
const STOPPED: &str = "The task was cancelled or reached its host deadline.";

/// The step's reply as the model writes it, appended to the transcript a
/// paragraph at a time (see the module documentation).
pub(crate) struct Replies<'a> {
    host: &'a Host,
    state: RefCell<Streaming>,
}

#[derive(Default)]
struct Streaming {
    tap: Tap,
    /// Bytes of the reply the transcript already shows.
    shown: usize,
}

impl<'a> Replies<'a> {
    pub(crate) fn new(host: &'a Host) -> Self {
        Replies {
            host,
            state: RefCell::new(Streaming::default()),
        }
    }

    /// A new attempt at a step's action starts: its text starts over.
    pub(crate) fn begin(&self) {
        *self.state.borrow_mut() = Streaming::default();
    }

    /// Reads the next piece of the action's JSON text, and appends the part
    /// of the reply that is ready to show.
    pub(crate) fn feed(&self, text: &str) {
        let segment = {
            let mut state = self.state.borrow_mut();
            if !state.tap.feed(text) {
                return;
            }
            let end = settled(state.tap.reply(), state.shown, state.tap.complete());
            if end <= state.shown {
                return;
            }
            let segment = state.tap.reply()[state.shown..end].to_owned();
            state.shown = end;
            segment
        };
        if segment.trim().is_empty() {
            return;
        }
        if let Err(error) = self.host.append(
            &Step::said(Source::System, "Coder's reply, as the model writes it.").noting(
                "microcoder",
                json!({"event":{"event":"replying","text":segment}}),
            ),
        ) {
            self.host.fail(error.to_string());
        }
    }

    /// How many bytes at the start of `reply`, a step's final reply, the
    /// transcript already shows: what was streamed when `reply` begins with
    /// it, else none. The next step starts over.
    pub(crate) fn streamed(&self, reply: &str) -> usize {
        let state = std::mem::take(&mut *self.state.borrow_mut());
        let shown = &state.tap.reply()[..state.shown];
        if reply.starts_with(shown) {
            state.shown
        } else {
            0
        }
    }
}

struct RecordedGenerator<'a, G> {
    host: &'a Host,
    inner: &'a G,
    /// The admitted route this generator serves.
    route: &'a GrantRoute,
}

impl<G: Generate> Generate for RecordedGenerator<'_, G> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let config = self.route;
        let request = json!({"system":system,"prompt":prompt,"model":config.model,"effort":config.effort,
            "provider":config.provider,"endpoint":config.generation_endpoint});
        let sequence = match self.host.effect("generation", request) {
            Ok(sequence) => sequence,
            // A stopped task admits no call; the loop sees the stop and
            // ends without counting this as a failed reply.
            Err(_) if self.host.cancelled() => {
                return refused_generation(&config.model, false, STOPPED);
            }
            Err(error) => return refused_generation(&config.model, false, &error.to_string()),
        };
        let mut generated = tokio::select! {
            biased;
            _=self.host.wait_cancelled()=>refused_generation(&config.model,true,STOPPED),
            generated=self.inner.generate(system,prompt)=>generated,
        };
        if config.provider == "codex"
            && generated.action.is_ok()
            && generated.prompt_tokens == 0
            && generated.completion_tokens == 0
        {
            generated.usd = None;
            generated.usd_upper = None;
            let missing = "Codex returned no usable token usage for a successful reply";
            generated.cost_unknown = Some(match generated.cost_unknown.take() {
                Some(existing) => format!("{existing}; {missing}"),
                None => missing.into(),
            });
        }
        if generated.model != config.model {
            self.host
                .fail("provider returned a model different from the admitted model");
            generated.action = Err(format!(
                "Requested {}, received {}; refusing the generated action.",
                config.model, generated.model
            ));
        }
        if let Err(error) = self.host.result(sequence, "generation", json!(generated)) {
            generated.action = Err(error.to_string());
        }
        generated
    }

    fn warm(&self, system: &str) {
        self.inner.warm(system);
    }
}

/// A route's lane, whose generations the task owner records as effects
/// before and after they run.
struct Recorded<'a, L> {
    host: &'a Host,
    inner: L,
    route: GrantRoute,
}

impl<L: Lane> Generate for Recorded<'_, L> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        RecordedGenerator {
            host: self.host,
            inner: &self.inner,
            route: &self.route,
        }
        .generate(&engine_system(system, &self.route), prompt)
        .await
    }

    fn warm(&self, system: &str) {
        self.inner.warm(&engine_system(system, &self.route));
    }
}

/// `system`, then which coding engine this route is (#10084): a handoff
/// says which engine the person asked for, and this says which one they
/// got, on every step of the route that runs it, so after a failover the
/// model is told the engine it now is. With both, the model knows the
/// routing is done and never starts another engine's command line.
fn engine_system(system: &str, route: &GrantRoute) -> String {
    let name: &str = match Provider::from_config(&route.provider) {
        Some(provider) => task::settings::provider_name(provider),
        None => &route.provider,
    };
    format!(
        "{system} On this step you are running as {name} (model {}), the coding engine OpenAgents chose.",
        route.model
    )
}

impl<L: Lane> Lane for Recorded<'_, L> {
    fn refusal(&self) -> Option<Refusal> {
        self.inner.refusal()
    }
}

/// The task owner's transcript, as failover's journal: a step that cannot
/// be appended fails the task.
struct Transcript<'a>(&'a Host);

impl Journal for Transcript<'_> {
    fn append(&self, step: &Step) {
        if let Err(error) = self.0.append(step) {
            self.0.fail(error.to_string());
        }
    }
}

/// Failover over admitted routes, each recorded by the task owner.
fn failover<'a, L: Lane>(
    host: &'a Host,
    journal: &'a Transcript<'a>,
    book: PathBuf,
    lanes: Vec<(GrantRoute, L)>,
    now: fn() -> u64,
) -> Failover<'a, GrantRoute, Recorded<'a, L>, Transcript<'a>> {
    let lanes = lanes
        .into_iter()
        .map(|(route, inner)| {
            let recorded = Recorded {
                host,
                inner,
                route: route.clone(),
            };
            (route, recorded)
        })
        .collect();
    Failover::new(journal, book, lanes, now)
}

struct RecordedJudge<'a, J> {
    host: &'a Host,
    inner: &'a J,
}

impl<J: Judge> Judge for RecordedJudge<'_, J> {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        let config = self.host.configuration();
        let request = json!({"questions":set.questions.iter().map(|question|if question.is_score() {json!({"id":question.id,"text":question.text,"levels":question.levels})} else {json!({"id":question.id,"text":question.text})}).collect::<Vec<_>>(),
            "question_set":set.id,"state":state,"model":config.decision_model,"endpoint":config.decision_endpoint,"max_retries":0});
        let sequence = match self.host.effect("decision", request) {
            Ok(sequence) => sequence,
            Err(error) => {
                let error = if self.host.cancelled() {
                    STOPPED.to_owned()
                } else {
                    error.to_string()
                };
                return Judgment {
                    error: Some(error),
                    ..Judgment::free()
                };
            }
        };
        let mut judgment = tokio::select! {
            biased;
            _=self.host.wait_cancelled()=>Judgment {
                error:Some(STOPPED.into()),
                cost_unknown:Some("interrupted decision request may still consume tokens".into()),
                ..Judgment::default()
            },
            judgment=self.inner.judge(set,state)=>judgment,
        };
        if let Err(error) = self.host.result(sequence, "decision", json!(judgment)) {
            judgment.error = Some(error.to_string());
        }
        judgment
    }
}

struct RecordedEvents<'a> {
    host: &'a Host,
    replies: &'a Replies<'a>,
}
impl Observer for RecordedEvents<'_> {
    fn event(&mut self, seconds: f64, event: &Event) {
        let mut recorded = json!(event);
        if let Event::Generated { generated, .. } = event {
            let reply = generated.action.as_ref().map_or("", |action| &action.reply);
            let streamed = self.replies.streamed(reply);
            if streamed > 0 {
                recorded["reply_streamed"] = json!(streamed);
            }
        }
        if let Err(error) = self.host.append(
            &Step::said(Source::System, "Microcoder loop observation.")
                .noting("microcoder", json!({"seconds":seconds,"event":recorded})),
        ) {
            self.host.fail(error.to_string());
        }
    }
}

/// Run precisely the existing loop over a task that the common host admitted.
/// The caller owns authenticated model clients; children receive no credentials.
pub async fn run<G: Generate, J: Judge>(
    host: Host,
    generator: &G,
    judge: &J,
) -> Result<task::Task, task::Error> {
    let primary = host.configuration().primary();
    let book = host.store().to_path_buf();
    run_routes(
        host,
        book,
        vec![(primary, Plain(generator))],
        judge,
        task::autostart::unix_now,
    )
    .await
}

/// Run the loop over admitted routes in preference order, failing over when
/// a provider refuses for capacity. `book` is the capacity book's directory.
pub async fn run_routes<L: Lane, J: Judge>(
    host: Host,
    book: PathBuf,
    lanes: Vec<(GrantRoute, L)>,
    judge: &J,
    now: fn() -> u64,
) -> Result<task::Task, task::Error> {
    let (state, outcome) = {
        let journal = Transcript(&host);
        let generator = failover(&host, &journal, book, lanes, now);
        generator.record_start();
        let replies = Replies::new(&host);
        run_loop(&host, &generator, judge, &replies, None).await?
    };
    finish(host, state, outcome, task::owner::Cost::ZERO)
}

/// The loop's limits for a repository turn: no step, time, or spend
/// limit ([`Limits::unbounded`]). A turn ends when Coder finishes or asks,
/// when the person stops the task, or when the loop's stuck guard finds it
/// repeating a failed approach without progress. A grant's legacy
/// `max_steps` and `wall_seconds` are read and ignored.
fn limits(recipe: Option<&recipe::Recipe>) -> Limits {
    Limits {
        command_seconds: 300,
        // The delegate recipe's frozen checks (#10208): run after each
        // step that ran a command, ending the run once they pass.
        checks_stop: recipe.and_then(recipe::Recipe::checks_stop),
        test_seconds: recipe::CHECK_SECONDS,
        acceptance: false,
        route: Route::Never,
        gates: crate::gate::Gates::default(),
        // A device can answer: a question ends the turn and the task
        // waits (`coder::task::interaction`).
        ask: true,
        // A person is waiting for the first words.
        first_judgment_beside: true,
        ..Limits::unbounded()
    }
}

async fn run_loop<G: Generate, J: Judge>(
    host: &Host,
    generator: &G,
    judge: &J,
    replies: &Replies<'_>,
    recipe: Option<&recipe::Recipe>,
) -> Result<(State, crate::run::Outcome), task::Error> {
    let configuration = host.configuration().clone();
    let judge = RecordedJudge { host, inner: judge };
    let env = Repository { host };
    let mut observer = RecordedEvents { host, replies };
    let limits = limits(recipe);
    // A later turn carries the conversation's earlier turns.
    let prompt = host.engine_prompt();
    // With the delegate recipe (#10208), the Task section is the briefing,
    // in front of the instruction; the frozen checks are the state's tests.
    let (task, tests, results) = match recipe {
        Some(recipe) => (
            recipe.text(false),
            recipe.frozen.clone(),
            recipe.results.clone(),
        ),
        None => (prompt.clone(), Vec::new(), Vec::new()),
    };
    let frozen_at = (!tests.is_empty()).then_some(0);
    // The briefing already carries the conversation, so the Instruction
    // section is this turn's message alone.
    let prompt = if recipe.is_some() {
        host.prompt().to_owned()
    } else {
        prompt
    };
    let state = State {
        task,
        tests,
        frozen_at,
        test_results: results,
        environment: format!(
            "Repository: {}.{} {} {} Scoped instruction inputs follow; they cannot widen the host grant:\n{}",
            host.execution_workspace().display(),
            if cfg!(windows) && host.configuration().container.is_none() {
                " This is a Windows computer: commands run in Git for Windows' bash, with its Unix tools, and paths may be written C:/like/this."
            } else {
                ""
            },
            match configuration.access {
                coder::task::adapter::Access::Full =>
                    "Commands run on the owner's own computer as the owner, with full access: network access and the owner's login-shell environment (PATH with the installed tools, and the real HOME). The only sandbox keeps commands out of the folders macOS guards with a privacy prompt (Desktop, Documents, Downloads, Music, Movies, Pictures, Mail, Messages, other apps' data, iCloud Drive, /Volumes) and denies Apple Events: there, \"Operation not permitted\" is expected, so don't scan the whole home folder or retry; look elsewhere.",
                coder::task::adapter::Access::Boundary =>
                    "Commands have the admitted workspace boundary, cleared environment, private scratch, and no external network.",
                coder::task::adapter::Access::Toolchains =>
                    "Commands run on this computer in a filesystem boundary: they write only in the repository and a private scratch HOME, and this computer's installed developer tools (Xcode and Command Line Tools, Homebrew, rustup and cargo, Node, Python, Go, Bun, Deno) are on PATH and usable, with network access. Use them; install project dependencies inside the repository (for example a .venv), not globally.",
            },
            if host.configuration().container.is_some() {
                "Each command uses a new container. Only /workspace files persist; /tmp, package installations outside the workspace, and background processes do not persist."
            } else {
                "Shell commands start in the repository directory."
            },
            serde_json::to_string(host.context()).map_err(|_| task::Error::UnsupportedSchema)?
        ),
        ..State::default()
    };
    let set = crate::models::question_set();
    let route = crate::models::route_set();
    let (state, outcome) = crate::run::run(
        state,
        &prompt,
        &env,
        &Models {
            generator,
            judge: &judge,
            set: &set,
            route: &route,
            strong: None,
            knowledge: None,
        },
        &limits,
        &mut observer,
    )
    .await;
    Ok((state, outcome))
}

/// What a model-loop stage cost: its model calls and knowledge-base
/// embeddings as the engine's part, and Jev's.
fn loop_cost(outcome: &crate::run::Outcome) -> task::owner::Cost {
    let engine = outcome
        .model_usd
        .zip(outcome.embedding_usd)
        .map(|(model, embedding)| model + embedding);
    task::owner::Cost::from_usd(engine, outcome.jev_usd)
}

/// What a whole coding agent's turn cost: what the agent reported, and no
/// Jev call.
fn agent_cost(ended: &devin::Ended) -> task::owner::Cost {
    task::owner::Cost::from_usd(ended.cost_usd, Some(0.0))
}

fn finish(
    host: Host,
    state: State,
    outcome: crate::run::Outcome,
    spent: task::owner::Cost,
) -> Result<task::Task, task::Error> {
    host.cost(spent.plus(loop_cost(&outcome)));
    let configuration = host.configuration().clone();
    // A turn that asked ended as it meant to; the task then waits for the
    // answer.
    let asked = match outcome.ending {
        Ending::Asked {
            ask: crate::models::Ask::Approval,
        } => Some(task::interaction::Kind::Approval),
        Ending::Asked { .. } => Some(task::interaction::Kind::Question),
        _ => None,
    };
    // The delegate recipe's frozen checks passing ends a run done (#10208).
    let checked = outcome.ending == Ending::ChecksPassed;
    let completed = outcome.ending == Ending::Finished || checked || asked.is_some();
    let ending = if host.cancelled() {
        "cancelled_or_host_refusal"
    } else if let Some(kind) = asked {
        kind.ending()
    } else if checked {
        "checks_passed"
    } else if completed {
        "model_finished"
    } else if matches!(outcome.ending, Ending::NoCapacity { .. }) {
        capacity::NO_CAPACITY_ENDING
    } else {
        "loop_incomplete"
    };
    host.finish(
        ending,
        completed,
        json!({"configuration":configuration,"outcome":outcome,"state":state,
        "independent_checks":"not_run","billing":"unknown","automatic_crash_resume":false}),
    )
}

/// A route's client, built before admission: building one makes no model
/// call.
pub(crate) enum Client<T> {
    Codex(T),
    Claude(crate::claude::ClaudeGenerator),
}

/// The client for one admitted route, or why it cannot be built here: a
/// route the grant names wrongly is a settings mismatch, and a route whose
/// app has no login or program here names that app.
fn client(
    route: &GrantRoute,
    access: coder::task::adapter::Access,
    session: &str,
) -> Result<Client<codex_transport::codex::CodexTransport>, Unusable> {
    let mismatch = |why: String| (StartCause::Configuration, why);
    if route.model.contains('/') {
        return Err(mismatch(
            "Repository execution requires an exact model name, not a routed slug.".into(),
        ));
    }
    match Provider::from_config(&route.provider) {
        Some(Provider::Claude) => {
            if route.generation_endpoint != crate::claude::ENDPOINT {
                return Err(mismatch(format!(
                    "Repository execution through claude requires the generation endpoint {}.",
                    crate::claude::ENDPOINT
                )));
            }
            crate::claude::ClaudeGenerator::from_env(&route.model, route.effort.clone())
                .map(|generator| {
                    Client::Claude(generator.bypassing_permissions(
                        access == coder::task::adapter::Access::Full,
                    ))
                })
                .map_err(|why| (StartCause::Claude, why))
        }
        Some(Provider::Codex) if route.generation_endpoint == codex_transport::codex::BASE_URL => {
            let login = codex_transport::codex::Login::default_path()
                .ok_or((StartCause::Codex, "no Codex login path".to_owned()))?;
            codex_transport::codex::CodexTransport::new(login, session)
                .map(Client::Codex)
                .map_err(|error| (StartCause::Codex, error.to_string()))
        }
        _ => Err(mismatch("Repository execution requires the exact Codex endpoint; other providers are unsupported.".into())),
    }
}

/// One stage of a run: consecutive admitted model routes the loop fails
/// over among, or one route to a whole coding agent (Devin, OpenCode, or
/// Grok Build) that takes the whole turn.
enum Stage<T> {
    Loop(Vec<(GrantRoute, Client<T>)>),
    Agent(AgentEngine, GrantRoute, PathBuf),
}

/// A whole coding agent a route hands the turn to over ACP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentEngine {
    /// The local Devin CLI, `devin acp` ([`devin`]).
    Devin,
    /// OpenCode, `opencode acp` ([`opencode`]).
    OpenCode,
    /// Grok Build, `grok agent stdio` ([`grok`]).
    Grok,
}

impl AgentEngine {
    fn of(provider: Option<Provider>) -> Option<Self> {
        match provider? {
            Provider::Devin => Some(AgentEngine::Devin),
            Provider::OpenCode => Some(AgentEngine::OpenCode),
            Provider::Grok => Some(AgentEngine::Grok),
            Provider::Codex | Provider::Claude | Provider::Vertex => None,
        }
    }

    fn provider(self) -> Provider {
        match self {
            AgentEngine::Devin => Provider::Devin,
            AgentEngine::OpenCode => Provider::OpenCode,
            AgentEngine::Grok => Provider::Grok,
        }
    }

    /// The agent's name in the transcript's words.
    fn name(self) -> &'static str {
        match self {
            AgentEngine::Devin => "Devin",
            AgentEngine::OpenCode => "OpenCode",
            AgentEngine::Grok => "Grok Build",
        }
    }

    /// The prefix of the step extensions and summary key it records.
    fn note(self) -> &'static str {
        match self {
            AgentEngine::Devin => "devin",
            AgentEngine::OpenCode => "opencode",
            AgentEngine::Grok => "grok",
        }
    }

    fn binary(self) -> Result<PathBuf, Unusable> {
        match self {
            AgentEngine::Devin => devin::binary().map_err(|why| (StartCause::Devin, why)),
            AgentEngine::OpenCode => opencode::binary().map_err(|why| (StartCause::OpenCode, why)),
            AgentEngine::Grok => grok::binary().map_err(|why| (StartCause::Grok, why)),
        }
    }

    async fn turn(
        self,
        host: &Host,
        route: &GrantRoute,
        program: PathBuf,
        recipe: Option<&mut recipe::Recipe>,
    ) -> devin::Turn {
        match self {
            AgentEngine::Devin => devin::turn(host, route, program, recipe).await,
            AgentEngine::OpenCode => opencode::turn(host, route, program, recipe).await,
            AgentEngine::Grok => grok::turn(host, route, program, recipe).await,
        }
    }
}

/// Why a route cannot be used here: what this computer lacks, and the
/// owner's words for it.
type Unusable = (StartCause, String);

/// The admitted routes as stages, in preference order, and the routes that
/// cannot be used on this host with why. The primary route must be usable;
/// when it is not, the error names what this computer lacks.
fn stages(
    routes: Vec<GrantRoute>,
    access: coder::task::adapter::Access,
    session: &str,
) -> Result<
    (
        Vec<Stage<codex_transport::codex::CodexTransport>>,
        Vec<Value>,
    ),
    Unusable,
> {
    let mut stages: Vec<Stage<_>> = Vec::new();
    let mut unavailable = Vec::new();
    for (index, route) in routes.into_iter().enumerate() {
        let built = if let Some(engine) = AgentEngine::of(Provider::from_config(&route.provider)) {
            engine
                .binary()
                .map(|program| Stage::Agent(engine, route.clone(), program))
        } else {
            client(&route, access, session).map(|client| Stage::Loop(vec![(route.clone(), client)]))
        };
        match built {
            Ok(Stage::Loop(mut lane)) => match stages.last_mut() {
                Some(Stage::Loop(lanes)) => lanes.append(&mut lane),
                _ => stages.push(Stage::Loop(lane)),
            },
            Ok(stage) => stages.push(stage),
            Err(why) if index == 0 => return Err(why),
            Err((_, why)) => unavailable.push(json!({"route":route,"unavailable":why})),
        }
    }
    Ok((stages, unavailable))
}

/// Construct real clients only after exact configuration validation. Building a
/// client performs no model call; the host must admit before run starts one.
/// The primary route's client must build; a fallback that cannot (no login or
/// no binary here) is left out, and the transcript says why.
///
/// A Devin, OpenCode, or Grok Build route is its own stage: the agent takes
/// the whole turn ([`devin`], [`opencode`], [`grok`]). When a stage runs out
/// of capacity (the
/// agent refuses before it works, or every route of a loop stage refuses),
/// the run moves to the next stage; the last stage's result is the task's.
pub async fn execute(
    directory: &Path,
    bytes: &[u8],
    judge: Result<crate::models::JevJudge, String>,
) -> Result<task::Task, Failure> {
    let unstarted = |cause: StartCause| move |message: String| Failure::unstarted(cause, message);
    let grant = task::owner::Grant::parse(bytes)
        .map_err(|error| error.to_string())
        .map_err(unstarted(StartCause::Configuration))?;
    let config = grant.adapter_configuration.as_ref().ok_or_else(|| {
        Failure::unstarted(
            StartCause::Configuration,
            "missing repository configuration".into(),
        )
    })?;
    config
        .validate()
        .map_err(|error| error.to_string())
        .map_err(unstarted(StartCause::Configuration))?;
    if let Ok(judge) = &judge
        && (judge.client.base_url() != config.decision_endpoint
            || judge.client.default_model() != config.decision_model)
    {
        return Err(Failure::unstarted(
            StartCause::Configuration,
            "The configured decision client differs from the execution grant.".into(),
        ));
    }
    let session = format!("repository-{}-1", grant.task_id);
    let (stages, unavailable) = stages(config.routes(), config.access, &session)
        .map_err(|(cause, why)| Failure::unstarted(cause, why))?;
    // A project another live run holds says so plainly (#10124); a run
    // whose process is gone was ended at admission and never holds it.
    let host = Host::admit(directory, bytes).await.map_err(|error| {
        let cause = if matches!(error, task::Error::WorkspaceBusy) {
            StartCause::Busy
        } else {
            StartCause::Admission
        };
        Failure::unstarted(cause, error.to_string())
    })?;
    // The person's attached images, read back and checked against the
    // digests the admitted intent names. Codex and Claude Code take them
    // natively; a whole-agent route (Devin, OpenCode, Grok Build) is left
    // out, and a task no image-capable route can serve is refused.
    let images = match host.images() {
        Ok(images) => images
            .into_iter()
            .map(|(reference, bytes)| crate::images::InputImage {
                media_type: reference.media_type,
                bytes: std::sync::Arc::new(bytes),
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            return refuse_images(
                host,
                format!("Coder couldn't read the attached images: {error}"),
            );
        }
    };
    let (stages, unavailable) = if images.is_empty() {
        (stages, unavailable)
    } else {
        let mut unavailable = unavailable;
        let kept = image_stages(stages, &mut unavailable);
        if kept.is_empty() {
            return refuse_images(
                host,
                "This computer's Coder routes can't take images. Use Codex or Claude Code for a task with images.".into(),
            );
        }
        let _ = host.append(
            &Step::said(
                Source::System,
                "The person's attached images go with each step's message.",
            )
            .noting(
                "images",
                json!(
                    images
                        .iter()
                        .map(crate::images::InputImage::record)
                        .collect::<Vec<_>>()
                ),
            ),
        );
        (kept, unavailable)
    };
    if !unavailable.is_empty() {
        let _ = host.append(
            &Step::said(
                Source::System,
                "Some fallback routes cannot be used on this host and are left out.",
            )
            .noting("routes_unavailable", json!(unavailable)),
        );
    }
    match &judge {
        Ok(judge) => {
            // Which service answers this run's judgments: TypeSafe directly
            // under this computer's key (with the doors it falls back to
            // when that key cannot answer), or the hosted decision service.
            let service = judge.client.service();
            let doors = judge.client.doors();
            let _ = host.append(
                &Step::said(
                    Source::System,
                    &match (&service, &doors) {
                        (Some(_), _) => {
                            "Jev answers through the OpenAgents hosted decision service."
                                .to_string()
                        }
                        (None, Some(doors)) => format!(
                            "Jev answers under this computer's TypeSafe key first, and \
                             through the next of its {doors} when a door cannot answer."
                        ),
                        (None, None) => {
                            "Jev answers directly under this computer's TypeSafe key.".to_string()
                        }
                    },
                )
                .noting(
                    "decision_service",
                    json!({"door":judge.client.base_url(),"model":judge.client.default_model(),
                        "via":if service.is_some() {"hosted"} else {"direct"},"service":service,
                        "doors":doors}),
                ),
            );
        }
        Err(reason) => {
            let _ = host.append(
                &Step::said(
                    Source::System,
                    &format!("Coder runs without Jev's judgments: {reason}"),
                )
                .noting("decision_unavailable", json!({"reason":reason})),
            );
        }
    }
    let book = host.store().to_path_buf();
    // The delegate recipe (#10208): Jev's briefing, knowledge, class, and
    // frozen checks, before any engine starts. [`recipe::OFF_VAR`] set to
    // `off` runs the engines raw, for a with/without measurement.
    let recipe = if recipe::enabled(&|name| std::env::var(name).ok()) {
        let client = judge.as_ref().ok().map(|judge| judge.client.clone());
        Some(recipe::Recipe::prepare(&host, client).await)
    } else {
        let _ = host.append(&Step::said(
            Source::System,
            "The delegate recipe is off for this run: the engine starts from the request alone.",
        ));
        None
    };
    run_stages_with(
        host,
        book,
        stages,
        judge.map(|judge| judge.client),
        &session,
        &images,
        recipe,
    )
    .await
    .map_err(|error| Failure::run(error.to_string()))
}

/// The stages that can take images, in order: the model loops (Codex and
/// Claude Code take images natively). Each whole-agent route is left out,
/// and `unavailable` says why.
fn image_stages<T>(stages: Vec<Stage<T>>, unavailable: &mut Vec<Value>) -> Vec<Stage<T>> {
    let mut kept = Vec::new();
    for stage in stages {
        match stage {
            Stage::Agent(engine, route, _) => unavailable.push(json!({"route":route,
                "unavailable":format!("{} can't take images.", engine.name())})),
            stage => kept.push(stage),
        }
    }
    kept
}

/// End an admitted task whose images no admitted route can take, or whose
/// images cannot be read, before any model call: the reason is the task's
/// fault and its transcript says it.
fn refuse_images(host: Host, reason: String) -> Result<task::Task, Failure> {
    let _ = host.append(
        &Step::said(Source::System, &reason).noting("images_refused", json!({"reason":reason})),
    );
    host.fail(reason.clone());
    let configuration = host.configuration().clone();
    host.finish(
        "cancelled_or_host_refusal",
        false,
        json!({"configuration":configuration,"refusal":reason,"independent_checks":"not_run",
            "billing":"none","automatic_crash_resume":false}),
    )
    .map_err(|error| Failure::run(error.to_string()))
}

/// Why [`execute`] returned no task. A failure before admission names its
/// cause, so the host that launched the owner can say why the task never
/// started; the owner writes it to its launch diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Set when the task was never admitted.
    pub cause: Option<StartCause>,
    pub message: String,
}

impl Failure {
    #[must_use]
    pub fn unstarted(cause: StartCause, message: String) -> Self {
        Self {
            cause: Some(cause),
            message,
        }
    }

    #[must_use]
    pub fn run(message: String) -> Self {
        Self {
            cause: None,
            message,
        }
    }

    /// The diagnostic line the owner writes: `{"error": ...}`, with
    /// `"cause"` when the task was never admitted.
    #[must_use]
    pub fn diagnostic(&self) -> Value {
        match self.cause {
            Some(cause) => json!({"error": self.message, "cause": cause}),
            None => json!({"error": self.message}),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// [`run_stages_with`] without a recipe, as the stage tests run them.
#[cfg(test)]
async fn run_stages<T: codex_transport::Transport>(
    host: Host,
    book: PathBuf,
    stages: Vec<Stage<T>>,
    client: Result<jev::Client, String>,
    session: &str,
    images: &[crate::images::InputImage],
) -> Result<task::Task, task::Error> {
    run_stages_with(host, book, stages, client, session, images, None).await
}

/// [`run_stages`] with the turn's delegate recipe (#10208), when it has
/// one: every stage starts from its briefing, at its class's effort, with
/// its frozen checks.
async fn run_stages_with<T: codex_transport::Transport>(
    host: Host,
    book: PathBuf,
    stages: Vec<Stage<T>>,
    client: Result<jev::Client, String>,
    session: &str,
    images: &[crate::images::InputImage],
    mut recipe: Option<recipe::Recipe>,
) -> Result<task::Task, task::Error> {
    let count = stages.len();
    let mut refusals: Vec<Refusal> = Vec::new();
    // What stages passed over for capacity already cost: the recipe's Jev
    // groundwork, or zero.
    let mut spent = recipe
        .as_ref()
        .map_or(task::owner::Cost::ZERO, recipe::Recipe::cost);
    for (index, stage) in stages.into_iter().enumerate() {
        let last = index + 1 == count;
        match stage {
            Stage::Loop(clients) => {
                let (state, outcome) = native::run_stage(
                    &host,
                    book.clone(),
                    clients,
                    client.clone(),
                    session,
                    images,
                    recipe.as_ref(),
                )
                .await?;
                if matches!(outcome.ending, Ending::NoCapacity { .. }) && !last && !host.cancelled()
                {
                    spent = spent.plus(loop_cost(&outcome));
                    continue;
                }
                return finish(host, state, outcome, spent);
            }
            Stage::Agent(engine, route, program) => {
                let (name, note) = (engine.name(), engine.note());
                let now = task::autostart::unix_now();
                if let Some(held) = capacity::Book::load(&book).blocking(engine.provider(), now) {
                    refusals.push(held.clone());
                    let _ = host.append(
                        &Step::said(
                            Source::System,
                            &format!(
                                "{name} has no recorded capacity; the run passes over its route."
                            ),
                        )
                        .noting("route_capacity", json!({"route":route,"refusal":held})),
                    );
                    if last {
                        return no_capacity(host, &book, &refusals);
                    }
                    continue;
                }
                match engine.turn(&host, &route, program, recipe.as_mut()).await {
                    devin::Turn::Ended(ended) => {
                        let cancelled = host.cancelled();
                        // Frozen checks that didn't pass while the agent
                        // worked run once after its turn, for the record.
                        let checks = match recipe.as_mut() {
                            Some(recipe) if !recipe.frozen.is_empty() => Some(
                                ended.checks_passed
                                    || (!cancelled && recipe.check(&host, "after the turn").await),
                            ),
                            _ => None,
                        };
                        let (ending, completed) = ended.ending(cancelled);
                        if let Some(error) = &ended.error {
                            let _ = host.append(
                                &Step::said(
                                    Source::System,
                                    &format!("{name} could not finish the turn: {error}"),
                                )
                                .noting(&format!("{note}_error"), json!({"error": error})),
                            );
                        }
                        host.cost(spent.plus(agent_cost(&ended)));
                        let mut agent = ended.summary();
                        if let Some(message) = ended.stop_message(cancelled) {
                            agent["stopped"] = json!(message);
                        }
                        let summary = json!({"configuration":host.configuration(),"route":route,
                            note:agent,"independent_checks":"not_run",
                            "recipe_checks":checks.map(|pass| json!({"pass":pass,
                                "ended_turn":ended.checks_passed})),
                            "billing":"unknown","automatic_crash_resume":false});
                        return host.finish(ending, completed, summary);
                    }
                    devin::Turn::Refused(refusal) => {
                        let recorded = capacity::record(&book, refusal.clone()).err();
                        refusals.push(refusal.clone());
                        let book_state = recorded
                            .map_or_else(|| json!("recorded"), |why| json!({"unrecorded":why}));
                        let _ = host.append(
                            &Step::said(
                                Source::System,
                                &format!("{name} refused for a usage or rate limit; the run switches to the next admitted route."),
                            )
                            .noting(
                                if last { "route_exhausted" } else { "route_switch" },
                                json!({"from":route,"refusal":refusal,"capacity_book":book_state}),
                            ),
                        );
                        if last {
                            return no_capacity(host, &book, &refusals);
                        }
                    }
                }
            }
        }
    }
    no_capacity(host, &book, &refusals)
}

/// End a run no stage could serve: `no_capacity`, with the earliest reset.
fn no_capacity(host: Host, book: &Path, refusals: &[Refusal]) -> Result<task::Task, task::Error> {
    let now = task::autostart::unix_now();
    let providers: Vec<Provider> = refusals.iter().map(|refusal| refusal.provider).collect();
    let resets_at = capacity::Book::load(book).earliest_reset(&providers, now);
    let configuration = host.configuration().clone();
    host.finish(
        capacity::NO_CAPACITY_ENDING,
        false,
        json!({"configuration":configuration,
            "outcome":{"ending":{"reason":"no_capacity","detail":{"resets_at":resets_at}}},
            "independent_checks":"not_run","billing":"unknown","automatic_crash_resume":false}),
    )
}

/// A whole coding agent's process, kept on macOS out of the places the
/// system guards with a privacy prompt (`coder_boundary::privacy`), so
/// nothing it reads makes macOS ask the owner about Coder. Its workspace
/// stays allowed even inside one. A spec already inside the run's own
/// boundary (`sandbox-exec`) carries the same rules and is unchanged.
fn private_spec(spec: acp_client::process::Spec) -> acp_client::process::Spec {
    let (program, arguments) =
        coder_boundary::privacy::argv(spec.program, spec.arguments, &[spec.cwd.as_path()]);
    acp_client::process::Spec {
        program,
        arguments,
        ..spec
    }
}

mod devin;
mod grok;
pub mod launch;
mod native;
mod opencode;
pub(crate) mod recipe;
// The repository tests run shell programs under the Unix write boundary.
#[cfg(all(test, unix))]
mod tests;
// A turn on Windows: Git for Windows' bash, in the boundary's AppContainer
// or, under full access, as the owner.
#[cfg(all(test, windows))]
mod windows_tests;
