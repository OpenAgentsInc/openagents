//! Provider failover within one run.
//!
//! A run may admit several routes (provider, model, and so on) in a
//! preference order. When a generation fails because the provider refused
//! for a usage or rate limit, the refusal is recorded in the capacity book
//! ([`crate::capacity`]), a System step with a `route_switch` extension
//! records the switch, and the same step is generated again on the next
//! route whose provider has capacity. When none has, a `route_exhausted`
//! step records the earliest reset and [`Generate::out_of_capacity`] tells
//! the loop, which ends with [`crate::run::Ending::NoCapacity`]. A step's
//! cost adds every attempt's cost, so failover keeps the known, unknown,
//! and upper-bound figures honest.
//!
//! The task owner's repository runs (`microcoder repository`) and Coder's
//! delegate door both run the loop through this one [`Failover`]; each
//! supplies its own routes and its own [`Journal`] for the steps.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use atif::{Source, Step};
use serde::Serialize;
use serde_json::{Value, json};

use crate::capacity::{self, Provider, Refusal};
use crate::models::{Basis, Exhausted, Generate, Generated};

/// A route's generator that can say whether its last generation met a
/// provider's capacity refusal.
pub trait Lane: Generate {
    /// The refusal the last generation met, if it met one. Reading clears it.
    fn refusal(&self) -> Option<Refusal>;
}

/// The clock failover and the provider lanes use: Unix seconds.
#[must_use]
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// A Codex transport that keeps the typed usage-limit refusal (HTTP 429
/// `usage_limit_reached`) its last request met.
pub struct Refusing<T> {
    pub inner: T,
    refusal: RefCell<Option<Refusal>>,
}

impl<T> Refusing<T> {
    #[must_use]
    pub fn new(inner: T) -> Self {
        Refusing {
            inner,
            refusal: RefCell::new(None),
        }
    }
}

impl<T: codex_transport::Transport> codex_transport::Transport for Refusing<T> {
    async fn respond(
        &self,
        request: &codex_transport::Request,
    ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
        let response = self.inner.respond(request).await;
        if let Err(codex_transport::TransportError::Http { status, body }) = &response
            && let Some(refusal) = Refusal::codex(*status, body, unix_now())
        {
            *self.refusal.borrow_mut() = Some(refusal);
        }
        response
    }
}

impl<T: codex_transport::Transport> Lane for crate::models::CodexGenerator<Refusing<T>> {
    fn refusal(&self) -> Option<Refusal> {
        self.transport.refusal.borrow_mut().take()
    }
}

/// Generation through the `claude` binary that keeps the rate-limit
/// refusal (an error result with API status 429) its last call met.
pub struct ClaudeLane {
    pub inner: crate::claude::ClaudeGenerator,
    refusal: RefCell<Option<Refusal>>,
}

impl ClaudeLane {
    #[must_use]
    pub fn new(inner: crate::claude::ClaudeGenerator) -> Self {
        ClaudeLane {
            inner,
            refusal: RefCell::new(None),
        }
    }
}

impl Generate for ClaudeLane {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let invocation = self.inner.invoke(system, prompt).await;
        *self.refusal.borrow_mut() = Refusal::claude(
            invocation.api_error_status.is_some(),
            invocation.api_error_status,
            unix_now(),
        );
        invocation.generated
    }
}

impl Lane for ClaudeLane {
    fn refusal(&self) -> Option<Refusal> {
        self.refusal.borrow_mut().take()
    }
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

/// What a route says about itself to failover. The route is serialized
/// into the steps failover writes.
pub trait Admitted: Serialize {
    /// The provider whose capacity the route depends on, or `None` for one
    /// without durable capacity, such as a fixture.
    fn provider(&self) -> Option<Provider>;
    /// The model the route generates with.
    fn model(&self) -> &str;
}

/// Where failover writes its steps.
pub trait Journal {
    /// Keep `step` in the run's transcript.
    fn append(&self, step: &Step);
}

/// A generation that never reached a model, or was interrupted after it
/// was sent (`dispatched`), so its cost is unknown.
#[must_use]
pub fn refused_generation(model: &str, dispatched: bool, reason: &str) -> Generated {
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

/// What failover says when no route has capacity.
pub const NO_CAPACITY: &str = "No admitted model provider has capacity.";

/// The routes of one run, in preference order, and the one in use.
pub struct Failover<'a, R, L, J: ?Sized> {
    journal: &'a J,
    /// The directory of the capacity book.
    book: PathBuf,
    lanes: Vec<(R, L)>,
    /// The lane in use, or `None` when no provider has capacity.
    current: Cell<Option<usize>>,
    /// Providers that refused during this run, in case the book cannot be
    /// written.
    refused: RefCell<Vec<Refusal>>,
    exhausted: Cell<Option<Exhausted>>,
    now: fn() -> u64,
}

impl<'a, R: Admitted, L: Lane, J: Journal + ?Sized> Failover<'a, R, L, J> {
    /// Failover over `lanes`, reading and writing the book in `book`, with
    /// `now` as the clock in Unix seconds.
    pub fn new(journal: &'a J, book: PathBuf, lanes: Vec<(R, L)>, now: fn() -> u64) -> Self {
        let failover = Failover {
            journal,
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

    /// The route in use, or `None` when no provider has capacity.
    #[must_use]
    pub fn route(&self) -> Option<&R> {
        self.current.get().map(|index| &self.lanes[index].0)
    }

    /// The refusals this run met, in the order it met them.
    #[must_use]
    pub fn refusals(&self) -> Vec<Refusal> {
        self.refused.borrow().clone()
    }

    /// Record which routes had capacity when the run started, so a run
    /// that starts past its first route says why. A single route with
    /// capacity records nothing.
    pub fn record_start(&self) {
        if self.lanes.len() < 2 && self.current.get() == Some(0) {
            return;
        }
        let now = (self.now)();
        let book = capacity::Book::load(&self.book);
        let routes: Vec<Value> = self
            .lanes
            .iter()
            .map(|(route, _)| {
                let blocked = route.provider().and_then(|p| book.blocking(p, now));
                json!({"route":route,"refusal":blocked})
            })
            .collect();
        self.journal.append(
            &Step::said(
                Source::System,
                "Admitted routes and their recorded capacity when the run started.",
            )
            .noting(
                "route_capacity",
                json!({"routes":routes,"starts_on":self.route()}),
            ),
        );
    }

    /// Whether the route's provider has capacity now, by the book and by
    /// this run's own refusals. A provider without durable capacity, such as
    /// a fixture, always has.
    fn has_capacity(&self, book: &capacity::Book, route: &R, now: u64) -> bool {
        route.provider().is_none_or(|provider| {
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

    /// The earliest time a route's provider has capacity again.
    #[must_use]
    pub fn earliest_reset(&self) -> Option<u64> {
        let now = (self.now)();
        let book = capacity::Book::load(&self.book);
        let refused = self.refused.borrow();
        self.lanes
            .iter()
            .filter_map(|(route, _)| route.provider())
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
#[must_use]
pub fn merge(earlier: Option<Generated>, later: Generated) -> Generated {
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

impl<R: Admitted, L: Lane, J: Journal + ?Sized> Generate for Failover<'_, R, L, J> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let mut spent: Option<Generated> = None;
        loop {
            let Some(index) = self.current.get() else {
                let model = self
                    .lanes
                    .first()
                    .map_or("none", |(route, _)| route.model());
                return merge(spent, refused_generation(model, false, NO_CAPACITY));
            };
            let (route, lane) = &self.lanes[index];
            let generated = lane.generate(system, prompt).await;
            let refusal = lane.refusal().filter(|_| generated.action.is_err());
            let Some(refusal) = refusal else {
                return merge(spent, generated);
            };
            let recorded = capacity::record(&self.book, refusal.clone()).err();
            self.refused.borrow_mut().push(refusal.clone());
            spent = Some(merge(spent, generated));
            let next = self.next(Some(index));
            self.current.set(next);
            let book = recorded.map_or_else(|| json!("recorded"), |why| json!({"unrecorded":why}));
            let step = match next {
                Some(next) => Step::said(
                    Source::System,
                    "The provider refused for a usage or rate limit; the run switches to the next admitted route.",
                )
                .noting(
                    "route_switch",
                    json!({"from":route,"to":self.lanes[next].0,"refusal":refusal,"capacity_book":book}),
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
                        json!({"from":route,"refusal":refusal,"resets_at":resets_at,"capacity_book":book}),
                    )
                }
            };
            self.journal.append(&step);
            if next.is_none() {
                return spent
                    .unwrap_or_else(|| refused_generation(route.model(), false, NO_CAPACITY));
            }
        }
    }

    fn out_of_capacity(&self) -> Option<Exhausted> {
        self.exhausted.get()
    }
}
