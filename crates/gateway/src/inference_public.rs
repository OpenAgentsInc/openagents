//! The public inference API (`docs/inference/gateway.md`, sections 3, 7,
//! and 8; P1, #11065): who may call `/v1/responses` and
//! `/v1/chat/completions` besides our own services, what they pay, and the
//! limits they set for themselves.
//!
//! - **Keys.** Any active `oak_` key may call once `inference.public` is
//!   configured. Keys are issued and revoked through the account routes
//!   (`/v1/workspaces/{ws}/keys`) and the website's Settings.
//! - **Free tier.** A fixed number of requests per workspace per day (UTC),
//!   only on the models `inference.public.free_tier.models` lists, and only
//!   when every planned attempt is one of them. A free request that fails
//!   before its first token gives its count back.
//! - **Paying.** Every other request holds its worst-case price from the
//!   workspace's balance (`tenancy::money`, policy `observed-usage-v1`)
//!   before anything is sent: one hold per distinct model the plan may
//!   try, priced at that model's rate card row plus margin. When the
//!   answer ends, the answering model's hold settles at the usage the
//!   upstream reported and the others are released (attempts that fell
//!   back before their first token are never charged). A stream that ends
//!   without usage, or a caller that goes away, leaves the answering hold
//!   outstanding for reconciliation, the decision gateway's rule.
//! - **Limits the key's owner sets** (`PUT
//!   /v1/workspaces/{ws}/keys/{key}/limits`, never by the key itself): a
//!   spending cap per day, month, or in total; a maximum price per million
//!   tokens; the models the key may call; requests per minute; and an
//!   expiry. A request that hits one gets `403 limit_reached` naming it in
//!   `param`. We set none of these.
//!
//! The key book (`inference-keys.json` beside the registry) keeps each
//! key's limits and spend and each workspace's free count for the day.
//! `GET /v1/key` shows a key its own; `GET /v1/usage/{request_id}` shows
//! one finished request's tokens, cost, attempts, and charge.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use futures_util::StreamExt;
use inference::error::{ApiError, ErrorType};
use inference::event::EventBody;
use inference::request::CreateResponse;
use inference::router::{Needs, PriceLimit, caller_rate, micros_usd, usd_micros};
use inference::run::{Events, Gateway, Prepared};
use inference::upstream::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tenancy::money::{Rate, Resource, Usage};
use tenancy::{Registry, keys};

use crate::money;
use crate::serve::ServeState;

pub const KEY: &str = "/v1/key";
pub const USAGE: &str = "/v1/usage/{request_id}";
pub const LIMITS: &str = "/v1/workspaces/{workspace}/keys/{key}/limits";

/// The key book's file, beside the registry.
pub const BOOK: &str = "inference-keys.json";

/// The capacity name inference prices carry in the money ledger.
pub const CAPACITY: &str = "inference";

/// The price version inference holds are taken under.
pub const PRICE_VERSION: &str = "inference-rate-card-v1";

/// Runs admitted by this process, so each run's hold has its own name.
static RUNS: AtomicU64 = AtomicU64::new(0);

/// Finished requests whose charge `GET /v1/usage/{id}` can still show.
const CHARGES_KEPT: usize = 20_000;

/// Input tokens added to a request's byte count for the hold, for the
/// upstream's message framing.
const FRAMING_TOKENS: u64 = 4_096;

/// Input tokens held for a request with images or files, whose token
/// count the gateway cannot read from the bytes.
const MEDIA_TOKENS: u64 = 65_536;

/// Output tokens held when neither the request nor the model names a
/// maximum.
const DEFAULT_MAX_OUTPUT: u64 = 32_768;

pub fn routes(state: &ServeState) -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    let mut routes = vec![(KEY, get(key_view)), (USAGE, get(usage_view))];
    if state.config.accounts.is_some() {
        routes.push((LIMITS, get(limits_read).put(limits_write)));
    }
    routes
}

// ---------------------------------------------------------------- limits

/// How often a spending cap starts over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    #[default]
    Day,
    Month,
    Total,
}

impl Period {
    fn word(self) -> &'static str {
        match self {
            Self::Day => "today",
            Self::Month => "this month",
            Self::Total => "in total",
        }
    }
}

/// A spending cap in dollars (a decimal string, such as `"5.00"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpendCap {
    pub usd: String,
    #[serde(default)]
    pub period: Period,
}

/// The highest price per million tokens the key pays, in dollars,
/// margin included.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaxPrice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// The limits a key's owner set on it. Every field is optional; none is
/// set by us.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_cap: Option<SpendCap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<MaxPrice>,
    /// The model ids (or router ids, such as `openagents/chat`) the key
    /// may call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<u32>,
    /// When the key stops working for inference, Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl Limits {
    /// Whether every amount reads as dollars.
    ///
    /// # Errors
    ///
    /// The field that does not, and why.
    pub fn check(&self) -> Result<(), (String, String)> {
        let dollars = "Write dollars as a number, such as \"5.00\".";
        if let Some(cap) = &self.spend_cap
            && usd_micros(&cap.usd).is_none()
        {
            return Err(("spend_cap.usd".into(), dollars.into()));
        }
        if let Some(max) = &self.max_price {
            for (name, value) in [("input", &max.input), ("output", &max.output)] {
                if let Some(value) = value
                    && usd_micros(value).is_none()
                {
                    return Err((format!("max_price.{name}"), dollars.into()));
                }
            }
        }
        if self.requests_per_minute == Some(0) {
            return Err((
                "requests_per_minute".into(),
                "Allow at least one request a minute, or leave it out.".into(),
            ));
        }
        Ok(())
    }

    fn price_limit(&self) -> PriceLimit {
        let max = self.max_price.clone().unwrap_or_default();
        PriceLimit {
            input: max.input.as_deref().and_then(usd_micros),
            output: max.output.as_deref().and_then(usd_micros),
        }
    }

    fn cap_micros(&self) -> Option<(u64, Period)> {
        let cap = self.spend_cap.as_ref()?;
        Some((usd_micros(&cap.usd)?, cap.period))
    }
}

// ------------------------------------------------------------------ book

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DayCount {
    day: u64,
    used: u32,
}

/// One key's spend, micros of USD.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    pub day: u64,
    pub day_micros: u64,
    /// Months since 1970-01.
    pub month: u64,
    pub month_micros: u64,
    pub total_micros: u64,
}

impl Spend {
    fn rolled(mut self, now_day: u64) -> Self {
        if self.day != now_day {
            self.day = now_day;
            self.day_micros = 0;
        }
        let month = month_of(now_day);
        if self.month != month {
            self.month = month;
            self.month_micros = 0;
        }
        self
    }

    fn in_period(&self, period: Period) -> u64 {
        match period {
            Period::Day => self.day_micros,
            Period::Month => self.month_micros,
            Period::Total => self.total_micros,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    limits: BTreeMap<String, Limits>,
    #[serde(default)]
    free: BTreeMap<String, DayCount>,
    #[serde(default)]
    spend: BTreeMap<String, Spend>,
}

/// What one request was charged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Charge {
    pub tenant: String,
    pub free: bool,
    /// Micros of USD settled; `None` while held or outstanding.
    pub micros: Option<u64>,
    pub settlement: &'static str,
}

/// The key book: limits, spend, free counts, and recent charges.
#[derive(Debug)]
pub struct Book {
    path: PathBuf,
    saved: Saved,
    /// Request times per key over the last minute, ms.
    recent: HashMap<String, VecDeque<u64>>,
    charges: VecDeque<(String, Charge)>,
}

impl Book {
    /// The book in `dir`, empty when it has no file yet.
    ///
    /// # Errors
    ///
    /// The file exists and cannot be read.
    pub fn open(dir: &std::path::Path) -> Result<Self, String> {
        let path = dir.join(BOOK);
        let saved = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        Ok(Self {
            path,
            saved,
            recent: HashMap::new(),
            charges: VecDeque::new(),
        })
    }

    fn save(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.saved).map_err(|e| e.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
    }

    #[must_use]
    pub fn limits(&self, key: &str) -> Limits {
        self.saved.limits.get(key).cloned().unwrap_or_default()
    }

    /// Replaces a key's limits.
    ///
    /// # Errors
    ///
    /// The book cannot be written.
    pub fn set_limits(&mut self, key: &str, limits: Limits) -> Result<(), String> {
        if limits == Limits::default() {
            self.saved.limits.remove(key);
        } else {
            self.saved.limits.insert(key.to_owned(), limits);
        }
        self.save()
    }

    #[must_use]
    pub fn spend(&self, key: &str, now_day: u64) -> Spend {
        self.saved
            .spend
            .get(key)
            .copied()
            .unwrap_or_default()
            .rolled(now_day)
    }

    fn free_used(&self, tenant: &str, now_day: u64) -> u32 {
        self.saved
            .free
            .get(tenant)
            .filter(|count| count.day == now_day)
            .map_or(0, |count| count.used)
    }

    /// Takes one free request for `tenant` today, if one is left.
    fn take_free(&mut self, tenant: &str, per_day: u32, now_day: u64) -> bool {
        let used = self.free_used(tenant, now_day);
        if used >= per_day {
            return false;
        }
        self.saved.free.insert(
            tenant.to_owned(),
            DayCount {
                day: now_day,
                used: used + 1,
            },
        );
        self.save().is_ok()
    }

    fn give_back_free(&mut self, tenant: &str, now_day: u64) {
        if let Some(count) = self.saved.free.get_mut(tenant)
            && count.day == now_day
        {
            count.used = count.used.saturating_sub(1);
            let _ = self.save();
        }
    }

    fn add_spend(&mut self, key: &str, micros: u64, now_day: u64) {
        let mut spend = self.spend(key, now_day);
        spend.day_micros = spend.day_micros.saturating_add(micros);
        spend.month_micros = spend.month_micros.saturating_add(micros);
        spend.total_micros = spend.total_micros.saturating_add(micros);
        self.saved.spend.insert(key.to_owned(), spend);
        let _ = self.save();
    }

    /// Counts a request against the key's rate cap; false when over it.
    fn admit_rate(&mut self, key: &str, per_minute: u32, now_ms: u64) -> bool {
        let times = self.recent.entry(key.to_owned()).or_default();
        while times.front().is_some_and(|at| *at + 60_000 <= now_ms) {
            times.pop_front();
        }
        if times.len() >= per_minute as usize {
            return false;
        }
        times.push_back(now_ms);
        true
    }

    pub(crate) fn record_charge(&mut self, request_id: &str, mut charge: Charge) {
        if let Some(found) = self.charges.iter_mut().find(|(id, _)| id == request_id) {
            if found.1.settlement == "settled" || found.1.free {
                // An earlier run of the same request: add this one to it.
                if let (Some(before), Some(now)) = (found.1.micros, charge.micros) {
                    charge.micros = Some(before.saturating_add(now));
                }
                charge.free &= found.1.free;
            }
            found.1 = charge;
            return;
        }
        while self.charges.len() >= CHARGES_KEPT {
            self.charges.pop_front();
        }
        self.charges.push_back((request_id.to_owned(), charge));
    }

    #[must_use]
    pub fn charge(&self, request_id: &str) -> Option<&Charge> {
        self.charges
            .iter()
            .find(|(id, _)| id == request_id)
            .map(|(_, charge)| charge)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| u64::try_from(span.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

fn today() -> u64 {
    now_ms() / 86_400_000
}

/// Months since 1970-01 for a day count since the epoch (UTC).
#[must_use]
pub fn month_of(day: u64) -> u64 {
    // Howard Hinnant's civil_from_days.
    let z = i64::try_from(day).unwrap_or(i64::MAX) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    u64::try_from((year - 1970) * 12 + (month - 1)).unwrap_or_default()
}

// ------------------------------------------------------------- admission

/// The public API's [`inference::run::Admit`]: a key's own limits, then
/// the free request or the balance hold, around every run its requests
/// make.
pub(crate) struct PublicAdmission {
    state: Arc<ServeState>,
    public: Public,
}

impl PublicAdmission {
    pub(crate) fn new(state: Arc<ServeState>, public: Public) -> Self {
        Self { state, public }
    }
}

impl inference::run::Admit for PublicAdmission {
    fn check<'a>(
        &'a self,
        request: &'a CreateResponse,
        caller: &'a inference::run::Caller,
    ) -> BoxFuture<'a, Result<PriceLimit, ApiError>> {
        Box::pin(async move {
            let mut caller = caller.clone();
            check_key(&self.state, &self.public, request, &mut caller).await?;
            Ok(caller.limits)
        })
    }

    fn admit<'a>(
        &'a self,
        request: &'a CreateResponse,
        prepared: &'a Prepared,
        caller: &'a inference::run::Caller,
    ) -> BoxFuture<'a, Result<Box<dyn inference::run::Admitted>, ApiError>> {
        Box::pin(async move {
            let Some(gateway) = self.state.inference.clone() else {
                return Err(ApiError::new(
                    ErrorType::NotFound,
                    "Inference is not set up here.",
                ));
            };
            let ticket = admit(
                &self.state,
                &gateway,
                &self.public,
                request,
                prepared,
                &caller.request_id,
            )
            .await?;
            Ok(Box::new(Held {
                state: self.state.clone(),
                ticket,
            }) as Box<dyn inference::run::Admitted>)
        })
    }
}

/// An admitted public run.
struct Held {
    state: Arc<ServeState>,
    ticket: Ticket,
}

impl inference::run::Admitted for Held {
    fn abandon(self: Box<Self>) -> BoxFuture<'static, ()> {
        Box::pin(async move { abandon(&self.state, self.ticket).await })
    }

    fn settle_on_end(self: Box<Self>, events: Events) -> Events {
        settle_on_end(self.state, self.ticket, events)
    }
}

/// A public caller: an `oak_` key outside the service tenants.
#[derive(Clone, Debug)]
pub(crate) struct Public {
    pub tenant: String,
    pub key_id: String,
    pub token: String,
    pub scopes: Option<keys::Scopes>,
    /// The `X-Workspace-Id` header, when sent.
    pub workspace: Option<String>,
}

fn limit(param: &str, message: impl Into<String>) -> ApiError {
    ApiError {
        param: Some(param.to_owned()),
        ..ApiError::new(ErrorType::LimitReached, message)
    }
}

fn book_trouble() -> ApiError {
    ApiError::new(
        ErrorType::ServerError,
        "Your key's settings can't be read right now. Try again in a minute.",
    )
}

/// The checks a key's owner asked for that need no plan: expiry, the
/// models it may call, and its rate. Fills the caller's price limit.
pub(crate) async fn check_key(
    state: &ServeState,
    public: &Public,
    request: &CreateResponse,
    caller: &mut inference::run::Caller,
) -> Result<(), ApiError> {
    let Some(book) = &state.inference_book else {
        return Err(book_trouble());
    };
    let mut book = book.lock().await;
    let limits = book.limits(&public.key_id);
    let now = now_ms();
    if limits
        .expires_at
        .is_some_and(|at| at.saturating_mul(1_000) <= now)
    {
        return Err(limit(
            "limits.expires_at",
            "This key has expired, as its owner set. Make a new key in Settings.",
        ));
    }
    let model = request.model.clone().unwrap_or_default();
    let mut named = vec![model.clone()];
    if let Some(options) = &request.openagents {
        named.extend(options.fallbacks.iter().cloned());
    }
    if let Some(allowed) = &limits.models
        && let Some(outside) = named.iter().find(|name| !allowed.contains(name))
    {
        return Err(limit(
            "limits.models",
            format!(
                "This key isn't allowed to call {outside}. Its owner can change that in Settings."
            ),
        ));
    }
    if let Some(scopes) = &public.scopes
        && let Some(outside) = named.iter().find(|name| !scopes.permits_model(name))
    {
        return Err(limit(
            "scopes.models",
            format!("This key isn't allowed to call {outside}."),
        ));
    }
    if let Some(per_minute) = limits.requests_per_minute
        && !book.admit_rate(&public.key_id, per_minute, now)
    {
        return Err(limit(
            "limits.requests_per_minute",
            format!(
                "This key is set to {per_minute} requests a minute. Wait a moment, or raise the limit in Settings."
            ),
        ));
    }
    caller.limits = caller.limits.min(limits.price_limit());
    Ok(())
}

/// What a public request stands under until it ends.
pub(crate) struct Ticket {
    request_id: String,
    tenant: String,
    key_id: String,
    kind: Kind,
}

enum Kind {
    Free,
    /// Every attempt is on the caller's own key: no fee in P1.
    Mine,
    Paid {
        /// The answering model's hold settles; the rest are released.
        holds: Vec<(String, money::Hold)>,
    },
}

/// One model's worst case: its price (rate card plus margin) and the
/// largest usage the request can report.
pub(crate) fn priced(
    gateway: &Gateway,
    request: &CreateResponse,
    model: &str,
    upstreams: &[&str],
) -> Option<money::Priced> {
    let meter = gateway.meter();
    let rows: Vec<_> = upstreams
        .iter()
        .filter_map(|upstream| meter.rate_row(upstream, model))
        .collect();
    let currency = rows.first()?.currency.clone();
    let rate = |pick: fn(&inference::meter::RateRow) -> u64| {
        rows.iter()
            .map(|row| caller_rate(pick(row), row))
            .max()
            .unwrap_or(0)
    };
    let input = rate(|row| row.input);
    let cached = rate(|row| row.cached_input.unwrap_or(row.input));
    let output = rate(|row| row.output);
    let offerings = gateway.offerings();
    let caps: Vec<_> = offerings
        .iter()
        .filter(|offering| {
            offering.model == model && upstreams.contains(&offering.upstream.as_str())
        })
        .map(|offering| offering.capabilities.clone())
        .collect();
    let context = caps.iter().map(|caps| caps.context).max().unwrap_or(0);
    let model_max_output = caps.iter().map(|caps| caps.max_output).max().unwrap_or(0);
    let needs = Needs::of(request);
    // Bytes bound text tokens from above.
    let mut max_input = needs
        .input_tokens
        .saturating_mul(4)
        .saturating_add(4)
        .saturating_add(FRAMING_TOKENS);
    if needs.images || needs.files {
        max_input = max_input.saturating_add(MEDIA_TOKENS);
    }
    if context > 0 {
        max_input = max_input.min(context);
    }
    let ceiling = if model_max_output > 0 {
        model_max_output
    } else {
        DEFAULT_MAX_OUTPUT
    };
    let max_output = request.max_output_tokens.unwrap_or(ceiling).min(ceiling);
    let per_million = |millionths: u64| Rate {
        millionths,
        per_units: 1_000_000,
    };
    Some(money::Priced {
        offer: None,
        price: tenancy::money::Price {
            // The ledger keeps one set of terms per version.
            version: format!("{PRICE_VERSION}:{model}:{input}:{cached}:{output}:{currency}"),
            currency,
            model: model.to_owned(),
            capacity: CAPACITY.to_owned(),
            policy: money::POLICY.to_owned(),
            rates: BTreeMap::from([
                (Resource::InputTokens, per_million(input)),
                (Resource::CachedInputTokens, per_million(cached)),
                (Resource::OutputTokens, per_million(output)),
            ]),
        },
        maximum_usage: Usage::from([
            (Resource::InputTokens, max_input),
            (Resource::CachedInputTokens, max_input),
            (Resource::OutputTokens, max_output),
        ]),
    })
}

/// The workspace a public key pays from: the `X-Workspace-Id` it named, or
/// the one workspace on its tenant, checked for current membership when
/// the account surface is configured.
fn workspace_of(state: &ServeState, public: &Public) -> Result<String, ApiError> {
    if state.config.accounts.is_none() {
        return Ok(public
            .workspace
            .clone()
            .unwrap_or_else(|| public.tenant.clone()));
    }
    let unavailable = || {
        ApiError::new(
            ErrorType::ServerError,
            "Your account can't be checked right now. Try again in a minute.",
        )
    };
    let accounts = tenancy::Accounts::open(&state.dir).map_err(|_| unavailable())?;
    let workspace = match &public.workspace {
        Some(named) => named.clone(),
        None => {
            let store = accounts.store().map_err(|_| unavailable())?;
            let mut found = store
                .workspaces
                .iter()
                .filter(|(_, ws)| ws.tenant == public.tenant)
                .map(|(id, _)| id.clone());
            match (found.next(), found.next()) {
                (Some(one), None) => one,
                (None, _) => return Ok(public.tenant.clone()),
                (Some(_), Some(_)) => {
                    return Err(ApiError::invalid_request(
                        "X-Workspace-Id",
                        "Your key reaches more than one workspace. Send an X-Workspace-Id header naming the one to pay from.",
                    ));
                }
            }
        }
    };
    let registry = Registry::open(&state.dir).map_err(|_| unavailable())?;
    accounts
        .authenticate_key(registry.manifest(), &workspace, &public.token)
        .map_err(|_| {
            ApiError::new(
                ErrorType::Unauthorized,
                "Your key doesn't belong to a current member of that workspace.",
            )
        })?;
    Ok(workspace)
}

/// Admits a planned public request: free when the free tier covers every
/// attempt and has a request left today, otherwise held from the
/// workspace balance.
pub(crate) async fn admit(
    state: &ServeState,
    gateway: &Gateway,
    public: &Public,
    request: &CreateResponse,
    prepared: &Prepared,
    request_id: &str,
) -> Result<Ticket, ApiError> {
    let Some(book) = &state.inference_book else {
        return Err(book_trouble());
    };
    let day = today();
    let ticket = |kind| Ticket {
        request_id: request_id.to_owned(),
        tenant: public.tenant.clone(),
        key_id: public.key_id.clone(),
        kind,
    };
    if !prepared.attempts().is_empty()
        && prepared
            .attempts()
            .iter()
            .all(|attempt| attempt.payer == inference::openagents::Payer::Mine)
    {
        book.lock().await.record_charge(
            request_id,
            Charge {
                tenant: public.tenant.clone(),
                free: false,
                micros: Some(0),
                settlement: "your_key",
            },
        );
        return Ok(ticket(Kind::Mine));
    }
    let free_tier = state
        .config
        .inference
        .as_ref()
        .and_then(|inference| inference.public.as_ref())
        .and_then(|public| public.free_tier.as_ref());
    if let Some(free) = free_tier
        && !prepared.attempts().is_empty()
        && prepared
            .attempts()
            .iter()
            .all(|attempt| free.models.contains(&attempt.model))
    {
        let mut book = book.lock().await;
        if book.take_free(&public.tenant, free.requests_per_day, day) {
            book.record_charge(
                request_id,
                Charge {
                    tenant: public.tenant.clone(),
                    free: true,
                    micros: Some(0),
                    settlement: "free",
                },
            );
            return Ok(ticket(Kind::Free));
        }
    }
    // One hold per distinct model, in plan order.
    let mut models: Vec<(String, Vec<&str>)> = Vec::new();
    for attempt in prepared.attempts() {
        match models.iter_mut().find(|(model, _)| *model == attempt.model) {
            Some((_, upstreams)) => upstreams.push(&attempt.upstream),
            None => models.push((attempt.model.clone(), vec![&attempt.upstream])),
        }
    }
    let mut priced_models = Vec::new();
    for (model, upstreams) in &models {
        let Some(priced) = priced(gateway, request, model, upstreams) else {
            return Err(ApiError::new(
                ErrorType::NoRoute,
                format!("{model} has no price yet, so it can't be called on a balance."),
            ));
        };
        priced_models.push((model.clone(), priced));
    }
    let worst = priced_models
        .iter()
        .filter_map(|(_, priced)| priced.price.quote(&priced.maximum_usage).ok())
        .max()
        .unwrap_or(0);
    {
        let book = book.lock().await;
        let limits = book.limits(&public.key_id);
        if let Some((cap, period)) = limits.cap_micros() {
            let spent = book.spend(&public.key_id, day).in_period(period);
            if spent.saturating_add(worst) > cap {
                return Err(limit(
                    "limits.spend_cap",
                    format!(
                        "This key has spent ${} {} of its ${} cap, and this request could cost up to ${}. Raise the cap in Settings, or ask for fewer output tokens.",
                        micros_usd(spent),
                        period.word(),
                        micros_usd(cap),
                        micros_usd(worst)
                    ),
                ));
            }
        }
    }
    let no_balance = || {
        ApiError::new(
            ErrorType::InsufficientBalance,
            "Add credit to your account to call this model. Free models are listed at GET /v1/models.",
        )
    };
    let Some(mut ledger) = state.money_lock().await else {
        return Err(no_balance());
    };
    let workspace = workspace_of(state, public)?;
    if ledger.shared_mode(&workspace).is_some() {
        return Err(ApiError::new(
            ErrorType::LimitReached,
            "This workspace's spending is shared with a team, which the model API doesn't take yet. Use a key from your own workspace.",
        ));
    }
    if let Err(why) =
        crate::card_funding::controller::check_money_profile(state, &ledger, &workspace)
    {
        return Err(ApiError::new(ErrorType::InsufficientBalance, why));
    }
    let mut holds = Vec::new();
    let run = format!("{request_id}.{}", RUNS.fetch_add(1, Ordering::Relaxed));
    for (index, (model, priced)) in priced_models.iter().enumerate() {
        let attempt = u32::try_from(index + 1).unwrap_or(u32::MAX);
        match money::reserve(
            &mut ledger,
            &workspace,
            &run,
            attempt,
            request_id,
            priced,
            (model, CAPACITY),
        ) {
            Ok(hold) => holds.push((model.clone(), hold)),
            Err(refusal) => {
                for (_, hold) in &holds {
                    money::release(&mut ledger, hold);
                }
                return Err(match refusal {
                    money::Refusal::Funds(_) => ApiError::new(
                        ErrorType::InsufficientBalance,
                        format!(
                            "Your balance is too low for this request, which could cost up to ${}. Add credit, set a lower max_output_tokens, or use a free model.",
                            micros_usd(worst)
                        ),
                    ),
                    money::Refusal::Budget(blocked) => limit("budget", format!("{blocked}")),
                    other => ApiError::new(
                        ErrorType::ServerError,
                        format!("This request can't be charged right now: {other}"),
                    ),
                });
            }
        }
    }
    drop(ledger);
    book.lock().await.record_charge(
        request_id,
        Charge {
            tenant: public.tenant.clone(),
            free: false,
            micros: None,
            settlement: "held",
        },
    );
    Ok(ticket(Kind::Paid { holds }))
}

/// The request failed before any answer: no charge, the free count back.
pub(crate) async fn abandon(state: &ServeState, ticket: Ticket) {
    match &ticket.kind {
        Kind::Free => {
            if let Some(book) = &state.inference_book {
                book.lock().await.give_back_free(&ticket.tenant, today());
            }
        }
        Kind::Mine => {}
        Kind::Paid { holds } => {
            if let Some(mut ledger) = state.money_lock().await {
                for (_, hold) in holds {
                    money::release(&mut ledger, hold);
                }
            }
        }
    }
    if let Some(book) = &state.inference_book {
        book.lock().await.record_charge(
            &ticket.request_id,
            Charge {
                tenant: ticket.tenant.clone(),
                free: matches!(ticket.kind, Kind::Free),
                micros: Some(0),
                settlement: "released",
            },
        );
    }
}

/// Settles when the stream ends: what the answer carried, or outstanding
/// when it carried no usage or the caller left.
struct Settler {
    state: Arc<ServeState>,
    ticket: Option<Ticket>,
}

impl Settler {
    async fn finish(&mut self, model: Option<String>, usage: Option<Usage>) {
        let Some(ticket) = self.ticket.take() else {
            return;
        };
        settle(&self.state, ticket, model, usage).await;
    }
}

impl Drop for Settler {
    fn drop(&mut self) {
        if let Some(ticket) = self.ticket.take() {
            let state = self.state.clone();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move { settle(&state, ticket, None, None).await });
            }
        }
    }
}

async fn settle(state: &ServeState, ticket: Ticket, model: Option<String>, usage: Option<Usage>) {
    let Kind::Paid { holds } = &ticket.kind else {
        return;
    };
    let mut charged = None;
    let mut label = "outstanding";
    if let Some(mut ledger) = state.money_lock().await {
        // The answering model's hold; a stream that never named one
        // leaves the first hold outstanding.
        let answering = model
            .as_deref()
            .and_then(|model| holds.iter().position(|(held, _)| held == model))
            .unwrap_or(0);
        for (index, (_, hold)) in holds.iter().enumerate() {
            if index == answering {
                let settlement =
                    money::settle(&mut ledger, hold, usage.clone(), &ticket.request_id);
                label = settlement.label();
                if settlement == money::Settlement::Settled
                    && let Some(usage) = &usage
                {
                    charged = hold.price.quote(usage).ok();
                }
            } else {
                money::release(&mut ledger, hold);
            }
        }
    }
    if let Some(book) = &state.inference_book {
        let mut book = book.lock().await;
        if let Some(micros) = charged {
            book.add_spend(&ticket.key_id, micros, today());
        }
        book.record_charge(
            &ticket.request_id,
            Charge {
                tenant: ticket.tenant.clone(),
                free: false,
                micros: charged,
                settlement: label,
            },
        );
    }
}

/// The usage an answer reports, under the inference price's resources.
fn usage_of(response: &inference::response::Response) -> Option<Usage> {
    let usage = response.usage.as_ref()?;
    let cached = usage
        .input_tokens_details
        .cached_tokens
        .min(usage.input_tokens);
    Some(Usage::from([
        (Resource::InputTokens, usage.input_tokens - cached),
        (Resource::CachedInputTokens, cached),
        (Resource::OutputTokens, usage.output_tokens),
    ]))
}

/// The committed stream, settling the ticket at its terminal event.
pub(crate) fn settle_on_end(state: Arc<ServeState>, ticket: Ticket, events: Events) -> Events {
    if matches!(ticket.kind, Kind::Free | Kind::Mine) {
        return events;
    }
    let settler = Settler {
        state,
        ticket: Some(ticket),
    };
    Box::pin(futures_util::stream::unfold(
        (events, settler),
        |(mut events, mut settler)| async move {
            let event = events.next().await?;
            if event.body.is_terminal()
                && let Some(response) = event.body.response()
            {
                let model = response.openagents.as_ref().map(|info| info.model.clone());
                let usage =
                    if matches!(event.body, EventBody::Failed(_)) && response.usage.is_none() {
                        None
                    } else {
                        usage_of(response)
                    };
                settler.finish(model, usage).await;
            }
            Some((event, (events, settler)))
        },
    ))
}

// ---------------------------------------------------------------- catalog

/// The model catalog `GET /v1/models` lists, public: the rate card's
/// catalog ([`crate::inference_rates::catalog`], OpenAI's list shape with
/// each provider's prices and live rates), each model marked `free` when
/// the free tier covers it, then the router ids whose classes have a model
/// to run on.
pub(crate) fn catalog(state: &ServeState) -> Option<Value> {
    let gateway = state.inference.as_ref()?;
    let mut catalog = crate::inference_rates::catalog(state)?;
    let free: Vec<String> = state
        .config
        .inference
        .as_ref()
        .and_then(|inference| inference.public.as_ref())
        .and_then(|public| public.free_tier.as_ref())
        .map(|free| free.models.clone())
        .unwrap_or_default();
    let Some(data) = catalog.get_mut("data").and_then(Value::as_array_mut) else {
        return Some(catalog);
    };
    let mut served = Vec::new();
    for model in data.iter_mut() {
        let id = model["id"].as_str().unwrap_or_default().to_owned();
        model["openagents"]["free"] = json!(free.contains(&id));
        served.push(id);
    }
    for (class, entry) in &gateway.classes().classes {
        let mut runs: Vec<&str> = entry
            .models
            .iter()
            .filter(|wanted| served.contains(&wanted.model))
            .map(|wanted| wanted.model.as_str())
            .collect();
        runs.dedup();
        if runs.is_empty() {
            continue;
        }
        data.push(json!({
            "id": format!("openagents/{}", class.as_str()),
            "object": "model",
            "created": 0,
            "owned_by": "openagents",
            "openagents": {"router": true, "models": runs},
        }));
    }
    if gateway
        .classes()
        .classes
        .contains_key(&inference::router::TaskClass::Chat)
    {
        data.push(json!({
            "id": "openagents/auto",
            "object": "model",
            "created": 0,
            "owned_by": "openagents",
            "openagents": {"router": true},
        }));
    }
    Some(catalog)
}

// ----------------------------------------------------------------- routes

fn api_error(error: &ApiError) -> Response {
    let status = StatusCode::from_u16(error.kind.status()).unwrap_or(StatusCode::BAD_REQUEST);
    (status, Json(json!({"error": error}))).into_response()
}

/// The calling key, any `oak_` key.
fn calling_key(state: &ServeState, headers: &HeaderMap) -> Result<keys::Authenticated, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| {
            ApiError::new(
                ErrorType::Unauthorized,
                "Send your API key in the `Authorization: Bearer` header.",
            )
        })?;
    let registry = Registry::open(&state.dir)
        .map_err(|_| ApiError::new(ErrorType::ServerError, "Keys can't be checked right now."))?;
    keys::authenticate(&state.dir, registry.manifest(), token)
        .map_err(|_| ApiError::new(ErrorType::Unauthorized, "Your API key was rejected."))
}

fn usd(micros: u64) -> String {
    micros_usd(micros)
}

/// `GET /v1/key`: the calling key's limits, spend, free requests left
/// today, and its workspace balance.
async fn key_view(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let key = match calling_key(&state, &headers) {
        Ok(key) => key,
        Err(error) => return api_error(&error),
    };
    let Some(book) = &state.inference_book else {
        return api_error(&book_trouble());
    };
    let day = today();
    let (limits, spend, free_used) = {
        let book = book.lock().await;
        (
            book.limits(&key.key_id),
            book.spend(&key.key_id, day),
            book.free_used(&key.tenant, day),
        )
    };
    let free = state
        .config
        .inference
        .as_ref()
        .and_then(|inference| inference.public.as_ref())
        .and_then(|public| public.free_tier.as_ref())
        .map(|free| {
            json!({
                "requests_per_day": free.requests_per_day,
                "used_today": free_used,
                "left_today": free.requests_per_day.saturating_sub(free_used),
                "models": free.models,
            })
        });
    let service = state
        .config
        .inference
        .as_ref()
        .is_some_and(|inference| inference.service_tenants.contains(&key.tenant));
    let public = Public {
        tenant: key.tenant.clone(),
        key_id: key.key_id.clone(),
        token: String::new(),
        scopes: key.scopes.clone(),
        workspace: headers
            .get("x-workspace-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    };
    let balance = match state.money_lock().await {
        Some(ledger) => {
            let workspace = public
                .workspace
                .clone()
                .or_else(|| {
                    tenancy::Accounts::open(&state.dir)
                        .ok()
                        .and_then(|accounts| accounts.store().ok())
                        .and_then(|store| {
                            store
                                .workspaces
                                .iter()
                                .find(|(_, ws)| ws.tenant == key.tenant)
                                .map(|(id, _)| id.clone())
                        })
                })
                .unwrap_or_else(|| key.tenant.clone());
            ledger.balance(&workspace).ok().map(|balance| {
                json!({
                    "workspace": workspace,
                    "currency": balance.currency,
                    "available": usd(balance.available),
                    "reserved": usd(balance.reserved),
                })
            })
        }
        None => None,
    };
    let remaining = limits.cap_micros().map(|(cap, period)| {
        json!({
            "period": period,
            "remaining_usd": usd(cap.saturating_sub(spend.in_period(period))),
        })
    });
    Json(json!({
        "key": {"id": key.key_id, "tenant": key.tenant, "service": service},
        "limits": limits,
        "spend": {
            "today_usd": usd(spend.day_micros),
            "this_month_usd": usd(spend.month_micros),
            "total_usd": usd(spend.total_micros),
        },
        "spend_cap": remaining,
        "free_tier": free,
        "balance": balance,
    }))
    .into_response()
}

/// `GET /v1/usage/{request_id}`: one request's tokens, cost, attempts,
/// and charge, to the tenant that made it.
async fn usage_view(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Response {
    let key = match calling_key(&state, &headers) {
        Ok(key) => key,
        Err(error) => return api_error(&error),
    };
    let missing = || {
        api_error(&ApiError::new(
            ErrorType::NotFound,
            format!("There's no request {request_id} on your account in the last day."),
        ))
    };
    let Some(meter) = &state.meter else {
        return missing();
    };
    let attempts = meter.request(&request_id);
    if attempts.is_empty()
        || attempts
            .iter()
            .any(|attempt| attempt.tenant.as_deref() != Some(key.tenant.as_str()))
    {
        return missing();
    }
    let answered = attempts
        .iter()
        .find(|attempt| attempt.outcome == inference::meter::Outcome::Ok)
        .or_else(|| attempts.last());
    let charge = match &state.inference_book {
        Some(book) => book.lock().await.charge(&request_id).cloned(),
        None => None,
    };
    let charge = charge.filter(|charge| charge.tenant == key.tenant);
    let tokens = answered.map(|attempt| attempt.tokens).unwrap_or_default();
    let cost = answered.and_then(|attempt| {
        Some(json!({
            "upstream_usd": usd(attempt.cost?),
            "margin_usd": usd(attempt.margin.unwrap_or(0)),
            "price_usd": usd(attempt.price.unwrap_or(0)),
        }))
    });
    let tried: Vec<Value> = attempts
        .iter()
        .map(|attempt| {
            json!({
                "attempt": attempt.attempt,
                "model": attempt.model,
                "upstream": attempt.upstream,
                "outcome": attempt.outcome,
                "error": attempt.error,
                "first_token_ms": attempt.first_token_ms,
                "total_ms": attempt.total_ms,
            })
        })
        .collect();
    Json(json!({
        "id": request_id,
        "object": "openagents.usage",
        "requested_model": answered.map(|attempt| attempt.requested_model.clone()),
        "model": answered.map(|attempt| attempt.model.clone()),
        "upstream": answered.map(|attempt| attempt.upstream.clone()),
        "api": answered.map(|attempt| attempt.api),
        "payer": if answered
            .and_then(|attempt| attempt.account.as_deref())
            .is_some_and(|account| account == inference::run::CALLER_KEY)
        {
            "mine"
        } else {
            "ours"
        },
        "tokens": tokens,
        "cost": cost,
        "charged": charge.map(|charge| json!({
            "free": charge.free,
            "usd": charge.micros.map(usd),
            "settlement": charge.settlement,
        })),
        "attempts": tried,
    }))
    .into_response()
}

async fn limits_read(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key)): Path<(String, String)>,
) -> Response {
    if let Err(response) = crate::accounts::key_context(&state, &headers, &workspace, &key) {
        return response;
    }
    let Some(book) = &state.inference_book else {
        return api_error(&book_trouble());
    };
    let limits = book.lock().await.limits(&key);
    Json(json!({"key": key, "limits": limits})).into_response()
}

/// `PUT .../keys/{key}/limits`: replaces the key's limits. Only a signed-in
/// member who may manage the key sets them, never the key itself.
async fn limits_write(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Response {
    if let Err(response) = crate::accounts::key_context(&state, &headers, &workspace, &key) {
        return response;
    }
    let limits: Limits = match serde_json::from_slice(&body) {
        Ok(limits) => limits,
        Err(why) => {
            return api_error(&ApiError::invalid_request(
                "body",
                format!("Those limits can't be read: {why}"),
            ));
        }
    };
    if let Err((param, message)) = limits.check() {
        return api_error(&ApiError::invalid_request(param, message));
    }
    let Some(book) = &state.inference_book else {
        return api_error(&book_trouble());
    };
    if book.lock().await.set_limits(&key, limits.clone()).is_err() {
        return api_error(&book_trouble());
    }
    Json(json!({"key": key, "limits": limits})).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_count_from_1970() {
        assert_eq!(month_of(0), 0);
        assert_eq!(month_of(31), 1); // 1970-02-01
        // 2026-10-09 is day 20,735.
        assert_eq!(month_of(20_735), (2026 - 1970) * 12 + 9);
    }

    #[test]
    fn limits_read_dollars_and_refuse_nonsense() {
        let limits: Limits = serde_json::from_value(json!({
            "spend_cap": {"usd": "5.00", "period": "month"},
            "max_price": {"output": "1.50"},
            "requests_per_minute": 10
        }))
        .unwrap();
        assert!(limits.check().is_ok());
        assert_eq!(limits.cap_micros(), Some((5_000_000, Period::Month)));
        assert_eq!(limits.price_limit().output, Some(1_500_000));
        let bad: Limits = serde_json::from_value(json!({"spend_cap": {"usd": "five"}})).unwrap();
        assert_eq!(bad.check().unwrap_err().0, "spend_cap.usd");
        assert!(serde_json::from_value::<Limits>(json!({"surprise": 1})).is_err());
    }

    #[test]
    fn the_book_counts_free_requests_spend_and_rate() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::open(dir.path()).unwrap();
        assert!(book.take_free("acme", 2, 10));
        assert!(book.take_free("acme", 2, 10));
        assert!(!book.take_free("acme", 2, 10));
        book.give_back_free("acme", 10);
        assert!(book.take_free("acme", 2, 10));
        assert!(book.take_free("acme", 2, 11), "a new day starts over");
        book.add_spend("k1", 1_000, 10);
        book.add_spend("k1", 500, 11);
        let spend = book.spend("k1", 11);
        assert_eq!((spend.day_micros, spend.total_micros), (500, 1_500));
        assert!(book.admit_rate("k1", 2, 0));
        assert!(book.admit_rate("k1", 2, 1));
        assert!(!book.admit_rate("k1", 2, 2));
        assert!(book.admit_rate("k1", 2, 60_001));
        let reopened = Book::open(dir.path()).unwrap();
        assert_eq!(reopened.spend("k1", 11).total_micros, 1_500);
        assert_eq!(reopened.free_used("acme", 11), 1);
    }
}
