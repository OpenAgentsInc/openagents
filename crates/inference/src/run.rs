//! The attempt loop (`docs/inference/gateway.md`, section 5, step 5): a
//! request's planned attempts sent to the adapters in order.
//!
//! [`Gateway::run`] plans the request ([`crate::router::plan`]), then sends
//! each attempt to its [`Upstream`] and reads its stream until the first
//! output token. An attempt that fails before then (an HTTP error, a
//! failure event, an empty stream, or no first token within the plan's
//! first-token deadline) is recorded as a fallback and the next attempt
//! goes; a request the upstream refused as malformed does not fall back,
//! since the next upstream would refuse it too. Once the first token
//! arrives the attempt is committed: nothing after it falls back, and a
//! failure after it reaches the caller as `response.failed`.
//!
//! The committed stream carries two events of ours: `openagents:route`
//! (the model and upstream that took the request) before the first output
//! item, and `openagents:cost` (the cost object from the meter's rate
//! card) before the terminal event, whose response also carries the
//! `openagents` object. Every attempt is recorded into the [`Meter`]: the
//! failed ones as they fail, the committed one when its stream ends or the
//! caller goes away.
//!
//! `openagents/auto` takes its class from a [`PickClass`]: a typed
//! judgment (Jev in the gateway), never keyword matching. With no judge,
//! or one that does not answer within [`JUDGE_BUDGET`], the class is
//! `chat`.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{Stream, StreamExt};

use crate::error::{ApiError, ErrorType, ResponseError};
use crate::event::{Event, EventBody, Lifecycle};
use crate::item::Item;
use crate::meter::{self, Api, Attempt, Meter, Recorder, Tokens};
use crate::openagents::{
    self as ext, AttemptOutcome, Cost, CostEvent, Privacy, ResponseInfo, RouteEvent,
};
use crate::request::CreateResponse;
use crate::response::{Response, ResponseStatus};
use crate::router::{
    Bench, Candidate, Capabilities, ClassJudge, ClassTable, Context, Offering, Plan, PriceLimit,
    Scores, TaskClass, plan,
};
use crate::stream::{Accumulator, Sequencer};
use crate::upstream::{AttemptError, AttemptMeter, BoxFuture, ErrorClass, EventStream, Upstream};
use crate::wire::Extra;

/// How long `openagents/auto` waits for its class judgment before it
/// takes `chat`.
pub const JUDGE_BUDGET: Duration = Duration::from_millis(1_500);

/// The router id that asks for a judged class.
const AUTO: &str = "openagents/auto";

/// The window of live rates the router ranks on.
const RATE_WINDOW_MS: u64 = 5 * 60_000;

/// Picks the task class for `openagents/auto` with a typed judgment.
pub trait PickClass: Send + Sync {
    /// The class for this request, or `None` when the judgment is not
    /// available.
    fn pick<'a>(&'a self, request: &'a CreateResponse) -> BoxFuture<'a, Option<TaskClass>>;
}

/// Who is calling, for the attempt records and the price limit.
#[derive(Clone, Debug, Default)]
pub struct Caller {
    /// The gateway's request id.
    pub request_id: String,
    pub tenant: Option<String>,
    pub key_id: Option<String>,
    pub api: Api,
    /// Limits the key's owner set.
    pub limits: PriceLimit,
    /// Admission for a paying caller: checked before planning, held
    /// between planning and sending, settled when the stream ends. Every
    /// path that runs a request (stored turns, compaction, hosted tool
    /// loops, the WebSocket) goes through it.
    pub admission: Option<Admission>,
    /// Adapters on the caller's own provider keys (bring your own key).
    /// They are offered only to `pay: "mine"`, which is offered nothing
    /// else, so a caller's key never pays for us and ours never for them.
    pub own: OwnUpstreams,
}

/// The caller's own adapters ([`Caller::own`]).
#[derive(Clone, Default)]
pub struct OwnUpstreams(pub Vec<Arc<dyn Upstream>>);

impl std::fmt::Debug for OwnUpstreams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.0.iter().map(|upstream| upstream.name()).collect();
        f.debug_tuple("OwnUpstreams").field(&names).finish()
    }
}

/// A caller's admission ([`Admit`]), cloneable with the caller.
#[derive(Clone)]
pub struct Admission(pub Arc<dyn Admit>);

impl std::fmt::Debug for Admission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Admission")
    }
}

/// Decides whether a request may run and what it costs the caller: the
/// gateway's public API holds the worst-case price from a balance or
/// takes a free request.
pub trait Admit: Send + Sync {
    /// Before planning: the limits the caller's owner set that need no
    /// plan (and count toward a rate), returning the price limit to plan
    /// under.
    fn check<'a>(
        &'a self,
        request: &'a CreateResponse,
        caller: &'a Caller,
    ) -> BoxFuture<'a, Result<PriceLimit, ApiError>>;

    /// Between planning and sending: take the hold or the free request.
    fn admit<'a>(
        &'a self,
        request: &'a CreateResponse,
        prepared: &'a Prepared,
        caller: &'a Caller,
    ) -> BoxFuture<'a, Result<Box<dyn Admitted>, ApiError>>;
}

/// What an admitted request stands under until it ends.
pub trait Admitted: Send {
    /// Nothing was answered: release the hold, give a free request back.
    fn abandon(self: Box<Self>) -> BoxFuture<'static, ()>;

    /// The committed stream, settling as its terminal event passes.
    fn settle_on_end(self: Box<Self>, events: Events) -> Events;
}

/// A stream of our events, sequence numbers stamped. It always ends with a
/// terminal event.
pub type Events = Pin<Box<dyn Stream<Item = Event> + Send>>;

/// What a caller can show while a request waits for its first token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// The attempt (numbered from 1) passed its first-token deadline and is
    /// the last one planned, so it keeps running toward its ceiling instead
    /// of failing: tell the person it is still working.
    StillWorking { attempt: u8, waited_ms: u64 },
}

/// A planned request, not yet sent ([`Gateway::prepare`]).
#[derive(Clone, Debug)]
pub struct Prepared {
    plan: Plan,
    requested: String,
    arrived: Instant,
}

impl Prepared {
    /// The attempts that will be tried, best first.
    #[must_use]
    pub fn attempts(&self) -> &[Candidate] {
        &self.plan.attempts
    }

    /// The task class, when the request named or was given one.
    #[must_use]
    pub fn class(&self) -> Option<TaskClass> {
        self.plan.class
    }
}

/// A committed request: which model and upstream answer it, and its
/// events.
pub struct Routed {
    pub model: String,
    pub upstream: String,
    pub class: Option<TaskClass>,
    /// The attempts made before this one, and this one (`ok`).
    pub attempts: Vec<ext::Attempt>,
    pub events: Events,
}

impl std::fmt::Debug for Routed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Routed")
            .field("model", &self.model)
            .field("upstream", &self.upstream)
            .field("class", &self.class)
            .field("attempts", &self.attempts)
            .finish_non_exhaustive()
    }
}

/// The adapters, the class table, and the meter: everything a request
/// needs to be routed and sent.
pub struct Gateway {
    upstreams: Vec<Arc<dyn Upstream>>,
    classes: ClassTable,
    scores: Scores,
    meter: Arc<Meter>,
    bench: Mutex<Bench>,
    judge: Option<Arc<dyn PickClass>>,
    judge_budget: Duration,
}

/// The class a judgment already gave, as the router's sync judge.
struct Decided(Option<TaskClass>);

impl ClassJudge for Decided {
    fn judge(&self, _request: &CreateResponse) -> Option<TaskClass> {
        self.0
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

impl Gateway {
    /// A gateway over `upstreams`, recording into `meter`. The adapters'
    /// price rows join the meter's rate card (rows the config set win).
    #[must_use]
    pub fn new(upstreams: Vec<Arc<dyn Upstream>>, meter: Arc<Meter>) -> Self {
        for upstream in &upstreams {
            meter.add_rates(upstream.rate_rows());
        }
        Self {
            upstreams,
            classes: ClassTable::default(),
            scores: Scores::default(),
            meter,
            bench: Mutex::new(Bench::default()),
            judge: None,
            judge_budget: JUDGE_BUDGET,
        }
    }

    /// The same gateway with another class table.
    #[must_use]
    pub fn with_classes(mut self, classes: ClassTable) -> Self {
        self.classes = classes;
        self
    }

    /// The same gateway with Gym scores for the quality floor.
    #[must_use]
    pub fn with_scores(mut self, scores: Scores) -> Self {
        self.scores = scores;
        self
    }

    /// The same gateway judging `openagents/auto` with `judge`, waiting at
    /// most `budget` for it.
    #[must_use]
    pub fn with_judge(mut self, judge: Arc<dyn PickClass>, budget: Duration) -> Self {
        self.judge = Some(judge);
        self.judge_budget = budget;
        self
    }

    /// The class table the router plans task classes with.
    #[must_use]
    pub fn classes(&self) -> &ClassTable {
        &self.classes
    }

    /// The meter attempts are recorded into.
    #[must_use]
    pub fn meter(&self) -> &Arc<Meter> {
        &self.meter
    }

    /// The adapters' names, with whether each has its key.
    #[must_use]
    pub fn upstreams(&self) -> Vec<(String, bool)> {
        self.upstreams
            .iter()
            .map(|upstream| (upstream.name().to_owned(), upstream.configured()))
            .collect()
    }

    /// What every configured adapter serves, as the router reads it. An
    /// adapter without its key is left out, so it never costs a request
    /// an attempt.
    #[must_use]
    pub fn offerings(&self) -> Vec<Offering> {
        offerings_of(&self.upstreams, &ext::Payer::Ours)
    }

    /// What `caller` may be routed to: ours, and the adapters on its own
    /// keys, marked `mine` and billed to no account of ours.
    fn offerings_for(&self, caller: &Caller) -> Vec<Offering> {
        let mut offerings = self.offerings();
        offerings.extend(offerings_of(&caller.own.0, &ext::Payer::Mine));
        offerings
    }

    fn upstream(&self, name: &str) -> Option<&Arc<dyn Upstream>> {
        self.upstreams
            .iter()
            .find(|upstream| upstream.name() == name)
    }
}

/// The offerings of every configured adapter in `upstreams`, paid by
/// `payer`. An adapter without its key is left out, so it never costs a
/// request an attempt.
fn offerings_of(upstreams: &[Arc<dyn Upstream>], payer: &ext::Payer) -> Vec<Offering> {
    let mut offerings = Vec::new();
    for upstream in upstreams.iter().filter(|upstream| upstream.configured()) {
        let zero_retention = upstream.privacy().allows(&Privacy::Strict);
        for row in upstream.models() {
            let caps = row.capabilities;
            offerings.push(Offering {
                upstream: upstream.name().to_owned(),
                model: row.id.clone(),
                capabilities: Capabilities {
                    tools: caps.tools,
                    json_schema: caps.json_schema,
                    images: caps.images,
                    files: false,
                    reasoning: caps.reasoning,
                    context: caps.context,
                    max_output: caps.max_output,
                },
                zero_retention,
                payer: payer.clone(),
                account: (*payer == ext::Payer::Ours).then(|| upstream.account().id.clone()),
            });
        }
    }
    offerings
}

impl Gateway {
    /// Routes and sends `request`, answering once an attempt has its first
    /// output token.
    ///
    /// # Errors
    ///
    /// The router's refusal (`404`, `403 limit_reached`, `503 no_route`),
    /// `400` when an upstream refused the request as malformed, or `502
    /// upstream_failed` when every attempt failed before its first token.
    pub async fn run(&self, request: &CreateResponse, caller: &Caller) -> Result<Routed, ApiError> {
        let Some(Admission(admission)) = &caller.admission else {
            let prepared = self.prepare(request, caller).await?;
            return self.send(request, caller, prepared).await;
        };
        let limits = admission.check(request, caller).await?;
        let caller = Caller {
            limits: caller.limits.min(limits),
            ..caller.clone()
        };
        let prepared = self.prepare(request, &caller).await?;
        let admitted = admission.admit(request, &prepared, &caller).await?;
        match self.send(request, &caller, prepared).await {
            Ok(mut routed) => {
                let events =
                    std::mem::replace(&mut routed.events, Box::pin(futures_util::stream::empty()));
                routed.events = admitted.settle_on_end(events);
                Ok(routed)
            }
            Err(refusal) => {
                admitted.abandon().await;
                Err(refusal)
            }
        }
    }

    /// Judges and plans `request` without sending anything: the attempts
    /// [`Gateway::send`] will make, so a caller can hold their worst-case
    /// price first.
    ///
    /// # Errors
    ///
    /// The router's refusal (`404`, `403 limit_reached`, `503 no_route`).
    pub async fn prepare(
        &self,
        request: &CreateResponse,
        caller: &Caller,
    ) -> Result<Prepared, ApiError> {
        let arrived = Instant::now();
        let pays_own = request
            .openagents
            .as_ref()
            .and_then(|options| options.pay.as_ref())
            .is_some_and(|payer| *payer == ext::Payer::Mine);
        if pays_own && caller.own.0.iter().all(|upstream| !upstream.configured()) {
            return Err(ApiError::invalid_request(
                "openagents.pay",
                "To pay with your own keys, add an OpenRouter or Vercel AI Gateway key to your account first.",
            ));
        }
        let requested = request.model.clone().unwrap_or_default();
        let picked = if requested == AUTO {
            match &self.judge {
                Some(judge) => tokio::time::timeout(self.judge_budget, judge.pick(request))
                    .await
                    .ok()
                    .flatten(),
                None => None,
            }
        } else {
            None
        };
        let now_ms = unix_ms();
        let offerings = self.offerings_for(caller);
        let (card, ledger) = self.meter.snapshot();
        let rates = self.meter.rates(RATE_WINDOW_MS, now_ms);
        let bench = self
            .bench
            .lock()
            .map(|bench| bench.clone())
            .unwrap_or_default();
        let plan_as = |judged: Option<TaskClass>| {
            let decided = Decided(judged);
            plan(
                request,
                &Context {
                    offerings: &offerings,
                    classes: &self.classes,
                    card: &card,
                    ledger: &ledger,
                    rates: &rates,
                    scores: &self.scores,
                    bench: &bench,
                    limits: caller.limits,
                    judge: Some(&decided),
                    now_ms,
                },
            )
        };
        // A judged class with nothing to run it on right now answers as
        // `chat`, the class `openagents/auto` takes without a judgment.
        let planned = match plan_as(picked) {
            Err(refusal)
                if refusal.kind == ErrorType::NoRoute
                    && requested == AUTO
                    && picked.is_some_and(|class| class != TaskClass::Chat) =>
            {
                plan_as(Some(TaskClass::Chat))?
            }
            other => other?,
        };
        Ok(Prepared {
            plan: planned,
            requested,
            arrived,
        })
    }

    /// Sends the attempts [`Gateway::prepare`] planned, answering once one
    /// has its first output token.
    ///
    /// # Errors
    ///
    /// `400` when an upstream refused the request as malformed, or `502
    /// upstream_failed` when every attempt failed before its first token.
    pub async fn send(
        &self,
        request: &CreateResponse,
        caller: &Caller,
        prepared: Prepared,
    ) -> Result<Routed, ApiError> {
        self.send_observed(request, caller, prepared, &|_| {}).await
    }

    /// [`Gateway::send`], telling `observe` when the last planned attempt
    /// outlives its first-token deadline and is given until the class's
    /// ceiling ([`Progress::StillWorking`]).
    ///
    /// Every attempt but the last falls back at the class's first-token
    /// deadline. The last has nothing to fall back to, so it runs to the
    /// ceiling (`last_ms`) before it counts as a miss. Nothing switches
    /// after a first token.
    ///
    /// # Errors
    ///
    /// As [`Gateway::send`].
    pub async fn send_observed(
        &self,
        request: &CreateResponse,
        caller: &Caller,
        prepared: Prepared,
        observe: &(dyn Fn(Progress) + Send + Sync),
    ) -> Result<Routed, ApiError> {
        let Prepared {
            plan: planned,
            requested,
            arrived,
        } = prepared;
        let class = planned.class;
        let deadline = Duration::from_millis(planned.first_token_ms);
        let ceiling = Duration::from_millis(planned.last_ms).max(deadline);
        let last = planned
            .attempts
            .iter()
            .rposition(|candidate| self.upstream(&candidate.upstream).is_some());
        let mut tried: Vec<ext::Attempt> = Vec::new();
        for (index, candidate) in planned.attempts.iter().enumerate() {
            let found = if candidate.payer == ext::Payer::Mine {
                caller
                    .own
                    .0
                    .iter()
                    .find(|upstream| upstream.name() == candidate.upstream)
            } else {
                self.upstream(&candidate.upstream)
            };
            let Some(upstream) = found.cloned() else {
                continue;
            };
            let number = u8::try_from(index + 1).unwrap_or(u8::MAX);
            let template = Attempt {
                request_id: caller.request_id.clone(),
                attempt: number,
                at_ms: unix_ms(),
                tenant: caller.tenant.clone(),
                key_id: caller.key_id.clone(),
                api: caller.api,
                class: class.map(|class| class.as_str().to_owned()),
                requested_model: requested.clone(),
                model: candidate.model.clone(),
                upstream: candidate.upstream.clone(),
                account: candidate.account.clone(),
                queue_ms: millis(arrived),
                ..Attempt::default()
            };
            let started = Instant::now();
            let extra = if last == Some(index) { ceiling } else { deadline };
            let waiting = move || {
                observe(Progress::StillWorking {
                    attempt: number,
                    waited_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
                });
            };
            match first_token(&*upstream, request, &candidate.model, deadline, extra, &waiting)
                .await
            {
                Ok(open) => {
                    tried.push(ext::Attempt {
                        model: candidate.model.clone(),
                        upstream: candidate.upstream.clone(),
                        outcome: AttemptOutcome::Ok,
                        ms: millis(started),
                        reason: None,
                        extra: Extra::new(),
                    });
                    let recorder: Arc<dyn Recorder> = if candidate.payer == ext::Payer::Mine {
                        Arc::new(TheirAccount(self.meter.clone()))
                    } else {
                        self.meter.clone()
                    };
                    open.meter.report_to(recorder, template);
                    let info = ResponseInfo {
                        model: candidate.model.clone(),
                        upstream: candidate.upstream.clone(),
                        attempts: tried.clone(),
                        cost: None,
                        extra: Extra::new(),
                    };
                    let row = self.meter.rate_row(&candidate.upstream, &candidate.model);
                    return Ok(Routed {
                        model: candidate.model.clone(),
                        upstream: candidate.upstream.clone(),
                        class,
                        attempts: tried,
                        events: committed(
                            open.held,
                            open.rest,
                            info,
                            row,
                            request.reasoning.clone(),
                        ),
                    });
                }
                Err(failure) => {
                    let falls_back = failure.error.class.falls_back();
                    let mut record = match &failure.meter {
                        Some(measured) => measured.attempt(template),
                        None => {
                            let mut record = template;
                            record.total_ms = millis(started);
                            record.upstream_status = failure.error.status;
                            record.error =
                                Some(failure.error.class.record(failure.error.status, false));
                            record
                        }
                    };
                    if candidate.payer == ext::Payer::Mine {
                        record.account = Some(CALLER_KEY.to_owned());
                    }
                    if failure.deadline {
                        record.error = Some(meter::ErrorClass::FirstTokenDeadline);
                    }
                    record.first_token_ms = None;
                    record.outcome = if falls_back {
                        meter::Outcome::Fallback
                    } else {
                        meter::Outcome::Failed
                    };
                    let class_record = record.error.unwrap_or(meter::ErrorClass::Other);
                    if failure.error.class.benches()
                        && let Ok(mut bench) = self.bench.lock()
                    {
                        let as_record = match failure.error.class {
                            ErrorClass::Payment => meter::ErrorClass::Payment,
                            _ => meter::ErrorClass::Auth,
                        };
                        bench.observe(&candidate.upstream, as_record, unix_ms());
                    }
                    self.meter.record(record);
                    tried.push(ext::Attempt {
                        model: candidate.model.clone(),
                        upstream: candidate.upstream.clone(),
                        outcome: if falls_back {
                            AttemptOutcome::Fallback
                        } else {
                            AttemptOutcome::Failed
                        },
                        ms: millis(started),
                        reason: Some(error_word(class_record).to_owned()),
                        extra: Extra::new(),
                    });
                    if !falls_back {
                        return Err(ApiError::invalid_request(
                            "input",
                            format!(
                                "{} refused the request as sent{}.",
                                candidate.model,
                                failure
                                    .error
                                    .status
                                    .map(|status| format!(" ({status})"))
                                    .unwrap_or_default()
                            ),
                        ));
                    }
                }
            }
        }
        let summary = tried
            .iter()
            .map(|attempt| {
                format!(
                    "{} through {}: {}",
                    attempt.model,
                    attempt.upstream,
                    attempt.reason.as_deref().unwrap_or("failed")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        Err(ApiError::new(
            ErrorType::UpstreamFailed,
            format!("No model answered before its first words ({summary})."),
        ))
    }
}

/// The account an attempt on the caller's own key is recorded under: none
/// of our credit accounts, so no ledger is debited, and usage can say the
/// caller paid.
pub const CALLER_KEY: &str = "caller-key";

/// Records an attempt on the caller's own key under [`CALLER_KEY`].
struct TheirAccount(Arc<Meter>);

impl Recorder for TheirAccount {
    fn record(&self, mut attempt: Attempt) {
        attempt.account = Some(CALLER_KEY.to_owned());
        self.0.record(attempt);
    }
}

/// The record word for an error class, as attempts name it.
fn error_word(class: meter::ErrorClass) -> &'static str {
    match class {
        meter::ErrorClass::Auth => "auth",
        meter::ErrorClass::Payment => "payment",
        meter::ErrorClass::RateLimited => "rate_limited",
        meter::ErrorClass::BadRequest => "bad_request",
        meter::ErrorClass::Server => "server",
        meter::ErrorClass::FirstTokenDeadline => "first_token_deadline",
        meter::ErrorClass::Timeout => "timeout",
        meter::ErrorClass::EmptyStream => "empty_stream",
        meter::ErrorClass::StreamFailed => "stream_failed",
        meter::ErrorClass::Network => "network",
        meter::ErrorClass::Other => "other",
    }
}

/// An attempt that has its first output token.
struct Open {
    meter: AttemptMeter,
    /// The events read so far, the first token's last.
    held: Vec<Event>,
    rest: EventStream,
}

/// An attempt that failed before its first output token.
struct Failure {
    meter: Option<AttemptMeter>,
    error: AttemptError,
    /// No first token within the deadline.
    deadline: bool,
}

/// Whether `body` is an output token: the moment an attempt is committed.
fn is_first_token(body: &EventBody) -> bool {
    match body {
        EventBody::OutputTextDelta(_)
        | EventBody::RefusalDelta(_)
        | EventBody::ReasoningDelta(_)
        | EventBody::ReasoningTextDelta(_)
        | EventBody::ReasoningSummaryTextDelta(_)
        | EventBody::FunctionCallArgumentsDelta(_) => true,
        EventBody::OutputItemAdded(added) => matches!(added.item, Item::FunctionCall(_)),
        // A terminal event that arrives with output (a short answer sent
        // whole) commits too; one with none is an empty stream.
        EventBody::Completed(event) | EventBody::Incomplete(event) => {
            !event.response.output.is_empty()
        }
        _ => false,
    }
}

/// Sends one attempt and reads it to its first output token, within
/// `deadline` of sending, or within `ceiling` when this is the last
/// attempt (`ceiling > deadline`); `waiting` runs when it outlives
/// `deadline` and keeps going.
async fn first_token(
    upstream: &dyn Upstream,
    request: &CreateResponse,
    model: &str,
    deadline: Duration,
    ceiling: Duration,
    waiting: &(dyn Fn() + Send + Sync),
) -> Result<Open, Failure> {
    let measured: Mutex<Option<AttemptMeter>> = Mutex::new(None);
    let reading = async {
        let sent = upstream
            .send(request, model)
            .await
            .map_err(|error| (error, false))?;
        if let Ok(mut slot) = measured.lock() {
            *slot = Some(sent.meter.clone());
        }
        let mut events = sent.events;
        let mut held = Vec::new();
        loop {
            match events.next().await {
                Some(Ok(event)) => {
                    let first = is_first_token(&event.body);
                    let failed = matches!(event.body, EventBody::Failed(_) | EventBody::Error(_));
                    held.push(event);
                    if first {
                        return Ok((sent.meter, held, events));
                    }
                    if failed {
                        return Err((
                            AttemptError::new(
                                ErrorClass::Upstream,
                                "the upstream failed before its first token",
                            ),
                            false,
                        ));
                    }
                }
                Some(Err(error)) => return Err((error, false)),
                None => {
                    return Err((
                        AttemptError::new(
                            ErrorClass::Empty,
                            "the stream ended before its first token",
                        ),
                        false,
                    ));
                }
            }
        }
    };
    let taken = |measured: &Mutex<Option<AttemptMeter>>| {
        measured.lock().ok().and_then(|mut slot| slot.take())
    };
    let bounded = async {
        tokio::pin!(reading);
        if ceiling > deadline {
            if let Ok(done) = tokio::time::timeout(deadline, &mut reading).await {
                return Ok(done);
            }
            waiting();
            return tokio::time::timeout(ceiling - deadline, &mut reading).await;
        }
        tokio::time::timeout(deadline, &mut reading).await
    };
    let deadline = ceiling.max(deadline);
    match bounded.await {
        Ok(Ok((meter, held, rest))) => Ok(Open { meter, held, rest }),
        Ok(Err((error, deadline))) => Err(Failure {
            meter: taken(&measured),
            error,
            deadline,
        }),
        Err(_) => {
            let error = AttemptError::new(
                ErrorClass::Timeout,
                format!("no first token in {} ms", deadline.as_millis()),
            );
            let meter = taken(&measured);
            if let Some(meter) = &meter {
                meter.fail(&error);
            }
            Err(Failure {
                meter,
                error,
                deadline: true,
            })
        }
    }
}

/// The committed attempt's stream: what was held, with `openagents:route`
/// before the first output item, then the rest, with `openagents:cost`
/// before the terminal event. A stream that breaks or ends without a
/// terminal event ends with `response.failed`.
fn committed(
    held: Vec<Event>,
    rest: EventStream,
    info: ResponseInfo,
    row: Option<meter::RateRow>,
    reasoning: Option<crate::request::ReasoningConfig>,
) -> Events {
    let mut pending: VecDeque<EventBody> = VecDeque::new();
    let mut routed = false;
    let route = EventBody::Route(RouteEvent {
        model: info.model.clone(),
        upstream: info.upstream.clone(),
        extra: Extra::new(),
    });
    let mut last: Option<Response> = None;
    for event in held {
        let body = event.body.normalized();
        if !routed && body.response().is_none() {
            pending.push_back(route.clone());
            routed = true;
        }
        if let Some(response) = body.response() {
            last = Some(response.clone());
        }
        pending.push_back(body);
    }
    if !routed {
        pending.push_back(route);
    }
    let pump = Pump {
        pending,
        rest: Some(rest),
        sequencer: Sequencer::new(),
        info,
        row,
        reasoning,
        last,
        done: false,
    };
    // A held terminal event (a whole answer in one event) gets its cost
    // and info before the stream starts.
    let pump = pump.settle_held();
    Box::pin(futures_util::stream::unfold(pump, |mut pump| async move {
        pump.next().await.map(|event| (event, pump))
    }))
}

struct Pump {
    pending: VecDeque<EventBody>,
    rest: Option<EventStream>,
    sequencer: Sequencer,
    info: ResponseInfo,
    row: Option<meter::RateRow>,
    /// The reasoning settings the caller sent. Every response we send
    /// carries these, not the upstream's own words for them (Vercel's GLM
    /// lane answers `effort: "max"`, which the spec does not name).
    reasoning: Option<crate::request::ReasoningConfig>,
    last: Option<Response>,
    done: bool,
}

impl Pump {
    /// Moves a terminal event already held behind its cost event.
    fn settle_held(mut self) -> Self {
        if let Some(at) = self.pending.iter().position(EventBody::is_terminal) {
            let terminal = self
                .pending
                .remove(at)
                .unwrap_or_else(|| route_placeholder());
            let mut tail: VecDeque<EventBody> = self.pending.split_off(at);
            for body in self.finish(terminal) {
                self.pending.push_back(body);
            }
            self.pending.append(&mut tail);
            self.done = true;
            self.rest = None;
        }
        self
    }

    /// The cost event and the terminal event carrying our `openagents`
    /// object.
    fn finish(&mut self, mut terminal: EventBody) -> Vec<EventBody> {
        let usage = terminal
            .response()
            .and_then(|response| response.usage.clone());
        let cost = match (&self.row, &usage) {
            (Some(row), Some(usage)) => {
                let tokens = Tokens {
                    input: usage.input_tokens,
                    cached_input: usage.input_tokens_details.cached_tokens,
                    cache_write: 0,
                    output: usage.output_tokens,
                    reasoning: usage.output_tokens_details.reasoning_tokens,
                };
                let priced = row.price(&tokens);
                Some(Cost::from_micros(priced.cost, priced.margin, None))
            }
            _ => None,
        };
        self.info.cost.clone_from(&cost);
        if let Some(response) = lifecycle_response(&mut terminal) {
            response.openagents = Some(self.info.clone());
        }
        let mut out = Vec::new();
        if let Some(cost) = cost {
            out.push(EventBody::Cost(CostEvent {
                cost,
                extra: Extra::new(),
            }));
        }
        out.push(terminal);
        out
    }

    /// `response.failed` built from the last response seen.
    fn failed(&mut self, message: &str) -> Vec<EventBody> {
        let mut response = self.last.clone().unwrap_or_else(|| Response {
            model: self.info.model.clone(),
            ..Response::from_request(
                String::new(),
                0,
                self.info.model.clone(),
                &CreateResponse::default(),
            )
        });
        response.status = ResponseStatus::Failed;
        response.error = Some(ResponseError {
            code: "upstream_failed".to_owned(),
            message: message.to_owned(),
            extra: Extra::new(),
        });
        self.finish(EventBody::lifecycle(Lifecycle::Failed, response))
    }

    async fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(mut body) = self.pending.pop_front() {
                if let Some(response) = lifecycle_response(&mut body) {
                    response.reasoning.clone_from(&self.reasoning);
                }
                return Some(self.sequencer.stamp(body));
            }
            if self.done {
                return None;
            }
            let Some(rest) = self.rest.as_mut() else {
                self.done = true;
                continue;
            };
            match rest.next().await {
                Some(Ok(event)) => {
                    let body = event.body.normalized();
                    match &body {
                        // Ours alone; an upstream's are dropped.
                        EventBody::Route(_) | EventBody::Cost(_) => {}
                        terminal if terminal.is_terminal() => {
                            let out = self.finish(body);
                            self.pending.extend(out);
                            self.done = true;
                            self.rest = None;
                        }
                        _ => {
                            if let Some(response) = body.response() {
                                self.last = Some(response.clone());
                            }
                            self.pending.push_back(body);
                        }
                    }
                }
                Some(Err(error)) => {
                    let out = self.failed(&format!(
                        "The model's stream failed ({}).",
                        error.class.as_str()
                    ));
                    self.pending.extend(out);
                    self.done = true;
                    self.rest = None;
                }
                None => {
                    let out = self.failed("The model's stream ended before it finished.");
                    self.pending.extend(out);
                    self.done = true;
                    self.rest = None;
                }
            }
        }
    }
}

fn route_placeholder() -> EventBody {
    EventBody::Unknown(serde_json::Value::Null)
}

fn lifecycle_response(body: &mut EventBody) -> Option<&mut Response> {
    match body {
        EventBody::Created(event)
        | EventBody::Queued(event)
        | EventBody::InProgress(event)
        | EventBody::Completed(event)
        | EventBody::Incomplete(event)
        | EventBody::Failed(event) => Some(&mut event.response),
        _ => None,
    }
}

/// Reads a committed stream to its end and folds it into one response,
/// for a caller that did not ask for a stream.
pub async fn collect(mut events: Events) -> Option<Response> {
    let mut folded = Accumulator::new();
    while let Some(event) = events.next().await {
        folded.push(&event);
    }
    folded.finish()
}
