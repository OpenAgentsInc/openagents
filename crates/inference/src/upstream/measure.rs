//! What an adapter measures about one attempt, reported as a
//! [`meter::Attempt`] through a [`meter::Recorder`].
//!
//! An [`AttemptMeter`] is shared between the adapter's stream and whoever
//! reads it. As events pass, it notes the time to the first output token;
//! from the terminal event it takes the token counts and any cost the
//! upstream reported; from a failure it takes the error class and the
//! upstream's status. It never holds prompt or completion text, nor an
//! upstream's error message. The cost from the rate row is left to the
//! [`meter::Meter`], which prices every record it is handed.
//!
//! Two ways to report: [`AttemptMeter::attempt`] fills a record on demand
//! (the router's choice when it decides the outcome itself), and
//! [`AttemptMeter::report_to`] records once, automatically, when the
//! stream ends, fails, or is dropped.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures_util::Stream;
use serde_json::Value;

use super::{AttemptError, ErrorClass, EventStream, ModelRow, Upstream};
use crate::event::{Event, EventBody};
use crate::item::Item;
use crate::meter::{self, Attempt, Recorder, Tokens};
use crate::response::Response;

/// Where an attempt stands, as far as the adapter knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The stream is still being read.
    Running,
    /// `response.completed`.
    Completed,
    /// `response.incomplete`: cut short by length or a filter.
    Incomplete,
    /// A failure: before the first token (an `Err`), or the caller's
    /// `response.failed` after it.
    Failed,
    /// The reader dropped the stream before it ended.
    Canceled,
}

/// One attempt's numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
    pub upstream: String,
    pub account: String,
    /// The public model id.
    pub model: String,
    pub upstream_model: String,
    pub stage: Stage,
    /// Unix milliseconds when the attempt started.
    pub at_ms: u64,
    /// Milliseconds from `send` to the first output token.
    pub first_token_ms: Option<u64>,
    /// Milliseconds from `send` to the end.
    pub total_ms: Option<u64>,
    pub tokens: Tokens,
    /// Whether the upstream sent usage (else the counts are zero).
    pub usage_reported: bool,
    /// The cost the upstream reported, in micro-dollars.
    pub reported_cost: Option<u64>,
    pub error: Option<ErrorClass>,
    pub upstream_status: Option<u16>,
}

struct Report {
    recorder: Arc<dyn Recorder>,
    template: Attempt,
}

struct State {
    measure: Measure,
    started: Instant,
    report: Option<Report>,
}

/// A shared handle on one attempt's [`Measure`].
#[derive(Clone)]
pub struct AttemptMeter(Arc<Mutex<State>>);

impl std::fmt::Debug for AttemptMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AttemptMeter")
            .field(&self.snapshot())
            .finish()
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

impl AttemptMeter {
    /// A meter started now, for an attempt on `row` through `upstream`.
    pub fn start<U: Upstream + ?Sized>(upstream: &U, row: &ModelRow) -> Self {
        Self(Arc::new(Mutex::new(State {
            measure: Measure {
                upstream: upstream.name().to_owned(),
                account: upstream.account().id.clone(),
                model: row.id.clone(),
                upstream_model: row.upstream_model.clone(),
                stage: Stage::Running,
                at_ms: unix_ms(),
                first_token_ms: None,
                total_ms: None,
                tokens: Tokens::default(),
                usage_reported: false,
                reported_cost: None,
                error: None,
                upstream_status: None,
            },
            started: Instant::now(),
            report: None,
        })))
    }

    fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let mut guard = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }

    /// The measure as it stands.
    #[must_use]
    pub fn snapshot(&self) -> Measure {
        self.with(|state| state.measure.clone())
    }

    /// `template` (the request id, attempt number, tenant, API, task
    /// class, requested model, and queue time the router knows) filled
    /// with this attempt's numbers.
    ///
    /// The outcome: `ok` for a completed or incomplete answer; `fallback`
    /// for a failure before the first token whose class the router falls
    /// back on; `failed` for any other failure; `canceled` when the reader
    /// went away.
    #[must_use]
    pub fn attempt(&self, template: Attempt) -> Attempt {
        let measure = self.snapshot();
        fill(template, &measure)
    }

    /// Records the attempt into `recorder` once it ends (at once, if it
    /// already has).
    pub fn report_to(&self, recorder: Arc<dyn Recorder>, template: Attempt) {
        let now = self.with(|state| {
            if state.measure.stage == Stage::Running {
                state.report = Some(Report { recorder, template });
                None
            } else {
                Some((recorder, fill(template, &state.measure)))
            }
        });
        if let Some((recorder, attempt)) = now {
            recorder.record(attempt);
        }
    }

    fn end(&self, f: impl FnOnce(&mut Measure)) {
        let report = self.with(|state| {
            if state.measure.stage != Stage::Running {
                return None;
            }
            f(&mut state.measure);
            if state.measure.total_ms.is_none() {
                state.measure.total_ms =
                    Some(u64::try_from(state.started.elapsed().as_millis()).unwrap_or(u64::MAX));
            }
            state
                .report
                .take()
                .map(|report| (report.recorder, fill(report.template, &state.measure)))
        });
        if let Some((recorder, attempt)) = report {
            recorder.record(attempt);
        }
    }

    /// Records a failure.
    pub fn fail(&self, error: &AttemptError) {
        self.end(|measure| {
            measure.stage = Stage::Failed;
            measure.error = Some(error.class);
            if error.status.is_some() {
                measure.upstream_status = error.status;
            }
        });
    }

    /// Records the HTTP status the upstream answered with.
    pub fn status(&self, status: u16) {
        self.with(|state| state.measure.upstream_status = Some(status));
    }

    fn observe(&self, event: &Event) {
        let first = is_first_token(&event.body);
        if first {
            self.with(|state| {
                if state.measure.first_token_ms.is_none() {
                    state.measure.first_token_ms = Some(
                        u64::try_from(state.started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                }
            });
        }
        let stage = match &event.body {
            EventBody::Completed(_) => Stage::Completed,
            EventBody::Incomplete(_) => Stage::Incomplete,
            EventBody::Failed(_) => Stage::Failed,
            _ => return,
        };
        self.end(|measure| {
            if let Some(response) = event.body.response() {
                take_usage(measure, response);
            }
            if stage == Stage::Failed && measure.error.is_none() {
                measure.error = Some(ErrorClass::Upstream);
            }
            measure.stage = stage;
        });
    }

    fn ended(&self) {
        self.end(|measure| {
            measure.stage = Stage::Failed;
            measure.error = Some(ErrorClass::Decode);
        });
    }

    fn canceled(&self) {
        self.end(|measure| measure.stage = Stage::Canceled);
    }

    /// Wraps `events` so this meter sees every item.
    #[must_use]
    pub fn wrap(&self, events: EventStream) -> EventStream {
        Box::pin(Metered {
            inner: events,
            meter: self.clone(),
            done: false,
        })
    }
}

fn fill(mut attempt: Attempt, measure: &Measure) -> Attempt {
    attempt.upstream.clone_from(&measure.upstream);
    attempt.model.clone_from(&measure.model);
    if attempt.requested_model.is_empty() {
        attempt.requested_model.clone_from(&measure.model);
    }
    attempt.account = Some(measure.account.clone());
    if attempt.at_ms == 0 {
        attempt.at_ms = measure.at_ms;
    }
    attempt.first_token_ms = measure.first_token_ms;
    attempt.total_ms = measure.total_ms.unwrap_or_default();
    attempt.tokens = measure.tokens;
    attempt.tokens_counted = false;
    attempt.usage_reported = measure.usage_reported;
    attempt.reported_cost = measure.reported_cost;
    attempt.upstream_status = measure.upstream_status;
    attempt.error = measure
        .error
        .map(|class| class.record(measure.upstream_status, measure.first_token_ms.is_some()));
    attempt.outcome = match measure.stage {
        Stage::Completed | Stage::Incomplete | Stage::Running => meter::Outcome::Ok,
        Stage::Canceled => meter::Outcome::Canceled,
        Stage::Failed => {
            let early = measure.first_token_ms.is_none();
            if early && measure.error.is_some_and(ErrorClass::falls_back) {
                meter::Outcome::Fallback
            } else {
                meter::Outcome::Failed
            }
        }
    };
    attempt
}

fn is_first_token(body: &EventBody) -> bool {
    match body {
        EventBody::OutputTextDelta(_)
        | EventBody::RefusalDelta(_)
        | EventBody::ReasoningDelta(_)
        | EventBody::ReasoningTextDelta(_)
        | EventBody::ReasoningSummaryTextDelta(_)
        | EventBody::FunctionCallArgumentsDelta(_) => true,
        EventBody::OutputItemAdded(added) => matches!(added.item, Item::FunctionCall(_)),
        _ => false,
    }
}

fn take_usage(measure: &mut Measure, response: &Response) {
    if let Some(usage) = &response.usage {
        measure.usage_reported = true;
        measure.tokens = Tokens {
            input: usage.input_tokens,
            cached_input: usage.input_tokens_details.cached_tokens,
            cache_write: usage
                .input_tokens_details
                .extra
                .get("cache_write_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            output: usage.output_tokens,
            reasoning: usage.output_tokens_details.reasoning_tokens,
        };
        // OpenRouter: `usage.cost`, dollars as a number.
        if let Some(cost) = usage.extra.get("cost") {
            measure.reported_cost = usd_micros(cost);
        }
    }
    // Vercel: `provider_metadata.gateway.cost`, dollars as a string.
    if let Some(cost) = response
        .extra
        .get("provider_metadata")
        .and_then(|metadata| metadata.get("gateway"))
        .and_then(|gateway| gateway.get("cost"))
    {
        measure.reported_cost = usd_micros(cost);
    }
}

/// A dollar amount (a decimal string or a JSON number) in micro-dollars,
/// rounded to the nearest micro. Parsed from the decimal text, never
/// through a float.
#[must_use]
pub fn usd_micros(value: &Value) -> Option<u64> {
    let text = match value {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    if text.contains(['e', 'E', '-']) {
        // Exponent form: rare (tiny or huge numbers); a float is fine here.
        let dollars: f64 = text.parse().ok()?;
        if dollars < 0.0 {
            return None;
        }
        return Some((dollars * 1_000_000.0).round() as u64);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut digits: Vec<u64> = fraction.bytes().map(|b| u64::from(b - b'0')).collect();
    digits.resize(7, 0);
    let micros = digits[..6].iter().fold(0, |acc, d| acc * 10 + d);
    let round_up = u64::from(digits[6] >= 5);
    Some(
        whole
            .checked_mul(1_000_000)?
            .checked_add(micros + round_up)?,
    )
}

struct Metered {
    inner: EventStream,
    meter: AttemptMeter,
    done: bool,
}

impl Stream for Metered {
    type Item = Result<Event, AttemptError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let poll = self.inner.as_mut().poll_next(cx);
        match &poll {
            Poll::Ready(Some(Ok(event))) => self.meter.observe(event),
            Poll::Ready(Some(Err(error))) => {
                self.meter.fail(error);
                self.done = true;
            }
            Poll::Ready(None) => {
                self.meter.ended();
                self.done = true;
            }
            Poll::Pending => {}
        }
        poll
    }
}

impl Drop for Metered {
    fn drop(&mut self) {
        if !self.done {
            self.meter.canceled();
        }
    }
}
