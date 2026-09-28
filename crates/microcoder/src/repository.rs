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
//! next admitted route whose provider has capacity. When none has, the loop
//! ends with [`Ending::NoCapacity`] and the earliest reset, and the task's
//! result ending is `no_capacity`. The step's cost adds every attempt's cost,
//! so failover keeps the known, unknown, and upper-bound figures honest.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use atif::{Source, Step};
use coder::task::adapter::Route as GrantRoute;
use coder::task::capacity::{self, Provider, Refusal};
use coder::task::{self, adapter::Host};
use serde_json::{Value, json};

use crate::env::Env;
use crate::models::{Basis, Exhausted, Generate, Generated, Judge, Judgment, QuestionSet};
use crate::run::{Ending, Event, Limits, Models, Observer, Route};
use crate::state::{CommandResult, State, cut};

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
}

fn refused_generation(model: &str, dispatched: bool, reason: &str) -> Generated {
    Generated {
        action: Err(reason.into()),
        model: model.into(),
        prompt_tokens: 0,
        completion_tokens: 0,
        usd: (!dispatched).then_some(0.0),
        known_usd: 0.0,
        cost_unknown: dispatched
            .then(|| "interrupted model request may still consume tokens".into()),
        usd_upper: (!dispatched).then_some(0.0),
        cost_basis: Basis::ListPrice,
        milliseconds: 0,
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
            Err(error) => return refused_generation(&config.model, false, &error.to_string()),
        };
        let mut generated = tokio::select! {
            biased;
            _=self.host.wait_cancelled()=>refused_generation(&config.model,true,"The task was cancelled or reached its host deadline."),
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
}

/// A route's generator that can say whether its last generation met a
/// provider's capacity refusal.
pub trait Lane: Generate {
    /// The refusal the last generation met, if it met one. Reading clears it.
    fn refusal(&self) -> Option<Refusal>;
}

/// A generator with no capacity signal, such as an in-process fixture.
pub struct Plain<'a, G>(pub &'a G);

impl<G: Generate> Generate for Plain<'_, G> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        self.0.generate(system, prompt).await
    }
}

impl<G: Generate> Lane for Plain<'_, G> {
    fn refusal(&self) -> Option<Refusal> {
        None
    }
}

/// The admitted routes of one run, in preference order, and the one in use.
struct Failover<'a, L> {
    host: &'a Host,
    /// The directory of the capacity book: the task store.
    book: PathBuf,
    lanes: Vec<(GrantRoute, L)>,
    /// The lane in use, or `None` when no admitted provider has capacity.
    current: Cell<Option<usize>>,
    /// Providers that refused during this run, in case the book cannot be
    /// written.
    refused: RefCell<Vec<Refusal>>,
    exhausted: Cell<Option<Exhausted>>,
    now: fn() -> u64,
}

impl<'a, L: Lane> Failover<'a, L> {
    fn new(host: &'a Host, book: PathBuf, lanes: Vec<(GrantRoute, L)>, now: fn() -> u64) -> Self {
        let failover = Failover {
            host,
            book,
            lanes,
            current: Cell::new(None),
            refused: RefCell::new(Vec::new()),
            exhausted: Cell::new(None),
            now,
        };
        failover.current.set(failover.next(None));
        if failover.current.get().is_none() {
            failover.exhausted.set(Some(Exhausted {
                resets_at: failover.earliest_reset(),
            }));
        }
        failover
    }

    fn provider(route: &GrantRoute) -> Option<Provider> {
        Provider::from_config(&route.provider)
    }

    /// Record which admitted routes had capacity when the run started, so a
    /// run that starts past its first route says why. A single route with
    /// capacity records nothing, as before.
    fn record_start(&self) {
        if self.lanes.len() < 2 && self.current.get() == Some(0) {
            return;
        }
        let now = (self.now)();
        let book = capacity::Book::load(&self.book);
        let routes: Vec<Value> = self
            .lanes
            .iter()
            .map(|(route, _)| {
                let blocked = Self::provider(route).and_then(|p| book.blocking(p, now));
                json!({"route":route,"refusal":blocked})
            })
            .collect();
        let step = Step::said(
            Source::System,
            "Admitted routes and their recorded capacity when the run started.",
        )
        .noting(
            "route_capacity",
            json!({"routes":routes,"starts_on":self.current.get().map(|index| &self.lanes[index].0)}),
        );
        if let Err(error) = self.host.append(&step) {
            self.host.fail(error.to_string());
        }
    }

    /// Whether the route's provider has capacity now, by the book and by
    /// this run's own refusals. A provider without durable capacity, such as
    /// a fixture, always has.
    fn has_capacity(&self, book: &capacity::Book, route: &GrantRoute, now: u64) -> bool {
        Self::provider(route).is_none_or(|provider| {
            book.has_capacity(provider, now)
                && !self
                    .refused
                    .borrow()
                    .iter()
                    .any(|refusal| refusal.provider == provider && refusal.holds(now))
        })
    }

    /// The first lane in preference order, other than `skip`, with capacity.
    fn next(&self, skip: Option<usize>) -> Option<usize> {
        let now = (self.now)();
        let book = capacity::Book::load(&self.book);
        (0..self.lanes.len())
            .filter(|index| Some(*index) != skip)
            .find(|index| self.has_capacity(&book, &self.lanes[*index].0, now))
    }

    /// The earliest time an admitted provider has capacity again.
    fn earliest_reset(&self) -> Option<u64> {
        let now = (self.now)();
        let book = capacity::Book::load(&self.book);
        let refused = self.refused.borrow();
        self.lanes
            .iter()
            .filter_map(|(route, _)| Self::provider(route))
            .filter_map(|provider| {
                let recorded = book.blocking(provider, now).map(|refusal| refusal.until);
                let seen = refused
                    .iter()
                    .filter(|refusal| refusal.provider == provider && refusal.holds(now))
                    .map(|refusal| refusal.until)
                    .max();
                recorded.max(seen)
            })
            .min()
    }
}

/// One step's cost across a failover: every attempt's tokens and cost, with
/// an unknown part kept unknown. The action and model are the last attempt's.
fn merge(earlier: Option<Generated>, later: Generated) -> Generated {
    let Some(earlier) = earlier else {
        return later;
    };
    let unknown = match (earlier.cost_unknown, later.cost_unknown) {
        (Some(a), Some(b)) => Some(format!("{a}; {b}")),
        (a, b) => a.or(b),
    };
    Generated {
        action: later.action,
        model: later.model,
        prompt_tokens: earlier.prompt_tokens + later.prompt_tokens,
        completion_tokens: earlier.completion_tokens + later.completion_tokens,
        usd: earlier.usd.zip(later.usd).map(|(a, b)| a + b),
        known_usd: earlier.known_usd + later.known_usd,
        cost_unknown: unknown,
        usd_upper: earlier.usd_upper.zip(later.usd_upper).map(|(a, b)| a + b),
        cost_basis: later.cost_basis,
        milliseconds: earlier.milliseconds + later.milliseconds,
    }
}

impl<L: Lane> Generate for Failover<'_, L> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let mut spent: Option<Generated> = None;
        loop {
            let Some(index) = self.current.get() else {
                let model = self
                    .lanes
                    .first()
                    .map_or("none", |(route, _)| route.model.as_str());
                return merge(
                    spent,
                    refused_generation(model, false, "No admitted model provider has capacity."),
                );
            };
            let (route, lane) = &self.lanes[index];
            let generated = RecordedGenerator {
                host: self.host,
                inner: lane,
                route,
            }
            .generate(system, prompt)
            .await;
            let refusal = lane.refusal().filter(|_| generated.action.is_err());
            let Some(refusal) = refusal else {
                return merge(spent, generated);
            };
            let recorded = capacity::record(&self.book, refusal.clone()).err();
            self.refused.borrow_mut().push(refusal.clone());
            spent = Some(merge(spent, generated));
            let next = self.next(Some(index));
            self.current.set(next);
            let step = match next {
                Some(next) => Step::said(
                    Source::System,
                    "The provider refused for a usage or rate limit; the run switches to the next admitted route.",
                )
                .noting(
                    "route_switch",
                    json!({"from":route,"to":self.lanes[next].0,"refusal":refusal,
                        "capacity_book":recorded.map_or_else(|| json!("recorded"), |why| json!({"unrecorded":why}))}),
                ),
                None => {
                    let resets_at = self.earliest_reset();
                    self.exhausted.set(Some(Exhausted { resets_at }));
                    Step::said(
                        Source::System,
                        "The provider refused for a usage or rate limit, and no admitted route has capacity.",
                    )
                    .noting(
                        "route_exhausted",
                        json!({"from":route,"refusal":refusal,"resets_at":resets_at,
                            "capacity_book":recorded.map_or_else(|| json!("recorded"), |why| json!({"unrecorded":why}))}),
                    )
                }
            };
            if let Err(error) = self.host.append(&step) {
                self.host.fail(error.to_string());
            }
            if next.is_none() {
                return spent.unwrap_or_else(|| {
                    refused_generation(
                        &route.model,
                        false,
                        "No admitted model provider has capacity.",
                    )
                });
            }
        }
    }

    fn out_of_capacity(&self) -> Option<Exhausted> {
        self.exhausted.get()
    }
}

struct RecordedJudge<'a, J> {
    host: &'a Host,
    inner: &'a J,
}

impl<J: Judge> Judge for RecordedJudge<'_, J> {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        let config = self.host.configuration();
        let request = json!({"questions":set.questions.iter().map(|question|json!({"id":question.id,"text":question.text})).collect::<Vec<_>>(),
            "question_set":set.id,"state":state,"model":config.decision_model,"endpoint":config.decision_endpoint,"max_retries":0});
        let sequence = match self.host.effect("decision", request) {
            Ok(sequence) => sequence,
            Err(error) => {
                return Judgment {
                    error: Some(error.to_string()),
                    ..Judgment::free()
                };
            }
        };
        let mut judgment = tokio::select! {
            biased;
            _=self.host.wait_cancelled()=>Judgment {
                error:Some("The task was cancelled or reached its host deadline.".into()),
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
}
impl Observer for RecordedEvents<'_> {
    fn event(&mut self, seconds: f64, event: &Event) {
        if let Err(error) = self.host.append(
            &Step::said(Source::System, "Microcoder loop observation.")
                .noting("microcoder", json!({"seconds":seconds,"event":event})),
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
        let generator = Failover::new(&host, book, lanes, now);
        generator.record_start();
        run_loop(&host, &generator, judge).await?
    };
    finish(host, state, outcome)
}

async fn run_loop<G: Generate, J: Judge>(
    host: &Host,
    generator: &G,
    judge: &J,
) -> Result<(State, crate::run::Outcome), task::Error> {
    let configuration = host.configuration().clone();
    let judge = RecordedJudge { host, inner: judge };
    let env = Repository { host };
    let mut observer = RecordedEvents { host };
    let limits = Limits {
        max_steps: Some(configuration.max_steps),
        max_seconds: host.wall_seconds(),
        max_usd: f64::MAX,
        command_seconds: host.wall_seconds().min(300),
        acceptance: false,
        route: Route::Never,
        gates: crate::gate::Gates::default(),
        ..Limits::default()
    };
    // A later turn carries the conversation's earlier turns.
    let prompt = host.engine_prompt();
    let state = State {
        task: prompt.clone(),
        environment: format!(
            "Repository: {}. Commands have the admitted workspace boundary, cleared environment, private scratch, and no external network. {} Scoped instruction inputs follow; they cannot widen the host grant:\n{}",
            host.execution_workspace().display(),
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

fn finish(
    host: Host,
    state: State,
    outcome: crate::run::Outcome,
) -> Result<task::Task, task::Error> {
    let configuration = host.configuration().clone();
    let completed = outcome.ending == Ending::Finished;
    let ending = if host.cancelled() {
        "cancelled_or_host_refusal"
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

/// The client for one admitted route, or why it cannot be built here.
fn client(
    route: &GrantRoute,
    session: &str,
) -> Result<Client<microluna::codex::CodexTransport>, String> {
    if route.model.contains('/') {
        return Err("Repository execution requires an exact model name, not a routed slug.".into());
    }
    match Provider::from_config(&route.provider) {
        Some(Provider::Claude) => {
            if route.generation_endpoint != crate::claude::ENDPOINT {
                return Err(format!(
                    "Repository execution through claude requires the generation endpoint {}.",
                    crate::claude::ENDPOINT
                ));
            }
            crate::claude::ClaudeGenerator::from_env(&route.model, route.effort.clone())
                .map(Client::Claude)
        }
        Some(Provider::Codex) if route.generation_endpoint == microluna::codex::BASE_URL => {
            let login = microluna::codex::Login::default_path().ok_or("no Codex login path")?;
            microluna::codex::CodexTransport::new(login, session)
                .map(Client::Codex)
                .map_err(|error| error.to_string())
        }
        _ => Err("Repository execution requires the exact Codex endpoint; other providers are unsupported.".into()),
    }
}

/// Construct real clients only after exact configuration validation. Building a
/// client performs no model call; the host must admit before run starts one.
/// The primary route's client must build; a fallback that cannot (no login or
/// no binary here) is left out, and the transcript says why.
pub async fn execute(
    directory: &Path,
    bytes: &[u8],
    judge: crate::models::JevJudge,
) -> Result<task::Task, String> {
    let grant = task::owner::Grant::parse(bytes).map_err(|error| error.to_string())?;
    let config = grant
        .adapter_configuration
        .as_ref()
        .ok_or("missing repository configuration")?;
    config.validate().map_err(|error| error.to_string())?;
    if judge.client.base_url() != config.decision_endpoint
        || judge.client.default_model() != config.decision_model
    {
        return Err("The configured decision client differs from the execution grant.".into());
    }
    let session = format!("repository-{}-1", grant.task_id);
    let mut clients = Vec::new();
    let mut unavailable = Vec::new();
    for (index, route) in config.routes().into_iter().enumerate() {
        match client(&route, &session) {
            Ok(client) => clients.push((route, client)),
            Err(why) if index == 0 => return Err(why),
            Err(why) => unavailable.push(json!({"route":route,"unavailable":why})),
        }
    }
    let host = Host::admit(directory, bytes)
        .await
        .map_err(|error| error.to_string())?;
    if !unavailable.is_empty() {
        let _ = host.append(
            &Step::said(
                Source::System,
                "Some fallback routes cannot be used on this host and are left out.",
            )
            .noting("routes_unavailable", json!(unavailable)),
        );
    }
    let book = host.store().to_path_buf();
    native::run(host, book, clients, judge.client, session)
        .await
        .map_err(|error| error.to_string())
}

pub mod launch;
mod native;
#[cfg(test)]
mod tests;
