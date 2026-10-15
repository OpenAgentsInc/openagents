//! Affiliate attribution (#11698): `/go/{code}` and `/r/{token}[/{series}]`.
//!
//! This is a separate, disclosed feature beside the cookieless site
//! analytics ([`crate::analytics`]), which still set nothing in the browser
//! and keep no identifier. Here, and only here:
//!
//! - `GET /go/{code}` follows one of our own campaign links, from the
//!   checked-in list in [`codes`] (`{channel}-{series}`, each with a fixed
//!   destination). `GET /r/{token}` and `/r/{token}/{series}` follow an
//!   affiliate's referral link: the token the referral system issued
//!   (`rfr_…`, `tenancy::accounts::referrals`). An unknown code or a
//!   malformed token goes to `/` and is counted only as `unknown`; what
//!   was typed is never kept.
//! - A person's click (never an agent's or a crawler's,
//!   [`crate::analytics::Traffic`]) sets one first-party cookie, `oa_ref`:
//!   signed, `HttpOnly`, `SameSite=Lax`, for 30 days, holding the link
//!   (code, or the affiliate's token), the series, a random click id and
//!   the time. A newer click replaces it: the last click wins.
//! - Each click is recorded on the server ([`Click`]): its random id, the
//!   code (or, for an affiliate, a short hash of the token, never the
//!   token), series, time, destination and traffic class. No address,
//!   device, browser detail or account.
//! - `Sec-GPC: 1` or `DNT: 1`: no cookie and no click record; the click is
//!   only counted in a per-code total. Background fetches (prefetch, link
//!   previews) set nothing either.
//! - Consent: where the law asks for it, the cookie is set only after the
//!   visitor says yes on a small page ([`consent`]). Which visitors that is
//!   comes from a country header named by `OPENAGENTS_WEB_COUNTRY_HEADER`
//!   (for a Google load balancer, a custom request header such as
//!   `X-Client-Geo-Location: {client_region}`). openagents.com is a Cloud
//!   Run domain mapping, which sends no country header, so with none
//!   configured, or none on the request, every visitor is asked.
//! - At sign-up ([`crate::auth`]), the cookie's click goes with the GitHub
//!   sign-in to the account service, which records it with the new account
//!   through the referral system (and, for an affiliate, proposes the
//!   attribution under the published policy). The cookie is then cleared.
//! - `/admin/attribution` ([`scoreboard`]) shows clicks and sign-ups per
//!   code, affiliate and series, per week, to site admins.
//!
//! Sales are joined to the account's attribution by the gateway's Stripe
//! webhook (`gateway::conversions`).

pub(crate) mod codes;
mod consent;
pub(crate) mod cookie;
pub(crate) mod scoreboard;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};

use crate::App;
use crate::analytics::{Client, Store, Traffic};
pub use cookie::Ticket;

/// The cookie's name.
pub const COOKIE: &str = "oa_ref";
/// How long a click is remembered: 30 days.
pub const WINDOW: u64 = 30 * 24 * 3600;
/// The owner's scoreboard.
pub const SCOREBOARD: &str = "/admin/attribution";
/// Where an affiliate link lands.
pub const AFFILIATE_DESTINATION: &str = "/";
/// The environment variable naming the request header that carries the
/// visitor's country, when a load balancer adds one.
pub const COUNTRY_HEADER_ENV: &str = "OPENAGENTS_WEB_COUNTRY_HEADER";

/// The most click records one process keeps for a day; past it, clicks are
/// only counted in totals.
const MAX_CLICKS_PER_DAY: usize = 20_000;
const FLUSH_EVERY: Duration = Duration::from_secs(60);
const SCHEMA: &str = "openagents.attribution.clicks.v1";

/// Countries whose visitors are asked before the cookie is set: the
/// European Economic Area, the United Kingdom and Switzerland.
pub const CONSENT_COUNTRIES: [&str; 32] = [
    "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE", "IT", "LV",
    "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE", "IS", "LI", "NO", "GB", "CH",
];

/// Whether paths under it belong to this module.
pub(crate) fn owns(path: &str) -> bool {
    path.starts_with("/go/") || path.starts_with("/r/") || path == SCOREBOARD
}

/// Which kind of link a click followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    Go,
    Affiliate,
}

impl Link {
    pub fn name(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::Affiliate => "affiliate",
        }
    }
}

/// Why a click was only counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Counted {
    /// An unknown code or a malformed affiliate token.
    Unknown,
    /// `Sec-GPC: 1` or `DNT: 1`.
    Quiet,
    /// Not a page visit: a prefetch, a link preview, a subresource.
    Background,
    /// Shown the consent page.
    Asked,
    /// Said no on the consent page.
    Declined,
    /// Past the day's record limit.
    Overflow,
}

impl Counted {
    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown code",
            Self::Quiet => "do not track",
            Self::Background => "not a visit",
            Self::Asked => "asked",
            Self::Declined => "said no",
            Self::Overflow => "over the limit",
        }
    }
}

/// One recorded click.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Click {
    /// `clk_` and 32 hex digits, random; also in the cookie.
    pub id: String,
    /// Unix seconds.
    pub at: u64,
    pub link: Link,
    /// The campaign code, or `aff-` and 16 hex digits of the affiliate
    /// token's SHA-256 (the referral system's link key starts the same).
    pub key: String,
    pub series: Option<String>,
    pub destination: String,
    /// `human`, `agent` or `bot`.
    pub traffic: String,
    /// Whether this click can be credited: a person's, with the cookie set.
    pub credited: bool,
}

/// A sign-up that came through a click.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signup {
    pub at: u64,
    pub click: String,
    pub link: Link,
    pub key: String,
    pub series: Option<String>,
}

/// A count of clicks that left no record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Total {
    pub day: u64,
    pub link: Link,
    pub key: String,
    pub counted: Counted,
    pub count: u64,
}

/// One day's records.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Day {
    pub clicks: Vec<Click>,
    pub signups: Vec<Signup>,
    pub totals: Vec<Total>,
}

impl Day {
    fn add_total(&mut self, day: u64, link: Link, key: &str, counted: Counted, count: u64) {
        if let Some(total) = self
            .totals
            .iter_mut()
            .find(|t| t.link == link && t.key == key && t.counted == counted)
        {
            total.count += count;
        } else {
            self.totals.push(Total {
                day,
                link,
                key: key.to_owned(),
                counted,
                count,
            });
        }
    }

    /// Adds `other`'s records, keeping one of each click and sign-up.
    pub fn merge(&mut self, other: Day) {
        for click in other.clicks {
            if !self.clicks.iter().any(|c| c.id == click.id) {
                self.clicks.push(click);
            }
        }
        for signup in other.signups {
            if !self.signups.iter().any(|s| s.click == signup.click) {
                self.signups.push(signup);
            }
        }
        for total in other.totals {
            self.add_total(
                total.day,
                total.link,
                &total.key,
                total.counted,
                total.count,
            );
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Batch {
    schema: String,
    day: Day,
}

fn decode(bytes: &[u8]) -> Option<Day> {
    serde_json::from_slice::<Batch>(bytes)
        .ok()
        .filter(|b| b.schema == SCHEMA)
        .map(|b| b.day)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The cookie key, the records, and where they're written.
pub struct Attribution {
    key: [u8; 32],
    store: Option<Store>,
    instance: String,
    country_header: Option<String>,
    days: Mutex<BTreeMap<u64, Day>>,
    flushing: tokio::sync::Mutex<()>,
}

impl Default for Attribution {
    fn default() -> Self {
        Self::new(&secp256k1::rand::random(), None, None)
    }
}

impl Attribution {
    /// `salt` is the server's secret (`OPENAGENTS_WEB_ASK_SALT`); the
    /// cookie key is derived from it for this use only.
    pub fn new(salt: &[u8; 32], store: Option<Store>, country_header: Option<String>) -> Self {
        let bytes: [u8; 8] = secp256k1::rand::random();
        Self {
            key: cookie::derive_key(salt),
            store,
            instance: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            country_header: country_header
                .map(|h| h.trim().to_ascii_lowercase())
                .filter(|h| !h.is_empty()),
            days: Mutex::new(BTreeMap::new()),
            flushing: tokio::sync::Mutex::new(()),
        }
    }

    /// From the environment: the analytics store (bucket or directory),
    /// and [`COUNTRY_HEADER_ENV`].
    pub fn from_env(salt: &[u8; 32]) -> Result<Self, crate::analytics::store::Error> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let store = match (
            var("OPENAGENTS_WEB_ANALYTICS_BUCKET"),
            var("OPENAGENTS_WEB_ANALYTICS_DIR"),
        ) {
            (Some(bucket), _) => Some(Store::gcs(bucket.trim().to_owned())?),
            (None, Some(directory)) => Some(Store::disk(directory.into())),
            (None, None) => None,
        };
        Ok(Self::new(salt, store, var(COUNTRY_HEADER_ENV)))
    }

    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    /// Whether this visitor is asked before the cookie is set: always,
    /// unless a configured country header names a country outside
    /// [`CONSENT_COUNTRIES`].
    pub fn needs_consent(&self, headers: &HeaderMap) -> bool {
        let Some(name) = &self.country_header else {
            return true;
        };
        let country = headers
            .get(name.as_str())
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().get(..2).unwrap_or_default().to_ascii_uppercase())
            .filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase()));
        match country {
            Some(country) => CONSENT_COUNTRIES.contains(&country.as_str()),
            None => true,
        }
    }

    fn with_day(&self, at: u64, change: impl FnOnce(u64, &mut Day)) {
        if let Ok(mut days) = self.days.lock() {
            let day = at / 86_400;
            change(day, days.entry(day).or_default());
        }
    }

    /// Counts a click that leaves no record.
    pub fn count(&self, link: Link, key: &str, counted: Counted) {
        self.with_day(now(), |day, records| {
            records.add_total(day, link, key, counted, 1);
        });
    }

    /// Records a click; past the day's limit it is only counted.
    pub fn record(&self, click: Click) {
        self.with_day(click.at, |day, records| {
            if records.clicks.len() < MAX_CLICKS_PER_DAY {
                records.clicks.push(click);
            } else {
                records.add_total(day, click.link, &click.key, Counted::Overflow, 1);
            }
        });
    }

    /// Records a sign-up through `ticket`'s click.
    pub fn signed_up(&self, ticket: &Ticket) {
        let at = now();
        self.with_day(at, |_, records| {
            if records.signups.len() < MAX_CLICKS_PER_DAY
                && !records.signups.iter().any(|s| s.click == ticket.click)
            {
                records.signups.push(Signup {
                    at,
                    click: ticket.click.clone(),
                    link: ticket.link(),
                    key: ticket.key(),
                    series: ticket.series.clone(),
                });
            }
        });
    }

    /// The records held in memory.
    pub fn snapshot(&self) -> BTreeMap<u64, Day> {
        self.days.lock().map(|d| d.clone()).unwrap_or_default()
    }

    /// Writes each held day whole, then forgets days before yesterday.
    pub async fn flush(&self) -> Result<(), crate::analytics::store::Error> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        let _one = self.flushing.lock().await;
        let held = self.snapshot();
        for (day, records) in &held {
            let name = format!(
                "attribution/raw/{}/{}.json",
                crate::analytics::date(*day),
                self.instance
            );
            let bytes = serde_json::to_vec(&Batch {
                schema: SCHEMA.to_owned(),
                day: records.clone(),
            })
            .unwrap_or_default();
            store.put(&name, bytes).await?;
        }
        let today = now() / 86_400;
        if let Ok(mut days) = self.days.lock() {
            days.retain(|day, _| day + 1 >= today);
        }
        Ok(())
    }

    /// Writes the records every minute, on the current Tokio runtime.
    pub fn spawn(self: &Arc<Self>) {
        if self.store.is_none() {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let mut flush = tokio::time::interval(FLUSH_EVERY);
            flush.tick().await;
            loop {
                flush.tick().await;
                if let Err(error) = me.flush().await {
                    eprintln!("attribution: {error}");
                }
            }
        });
    }

    /// The records of the `days` days ending today: what is stored, or
    /// what this process holds when nothing is.
    pub async fn load(
        &self,
        days: u64,
    ) -> Result<BTreeMap<u64, Day>, crate::analytics::store::Error> {
        let today = now() / 86_400;
        let first = today.saturating_sub(days.saturating_sub(1));
        let Some(store) = &self.store else {
            let mut held = self.snapshot();
            held.retain(|day, _| *day >= first);
            return Ok(held);
        };
        self.flush().await?;
        let mut out = BTreeMap::new();
        for day in first..=today {
            let names = store
                .list(&format!("attribution/raw/{}/", crate::analytics::date(day)))
                .await?;
            let mut merged = Day::default();
            for name in names {
                if let Some(records) = store.get(&name).await?.as_deref().and_then(decode) {
                    merged.merge(records);
                }
            }
            if merged != Day::default() {
                out.insert(day, merged);
            }
        }
        Ok(out)
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/go/{code}", get(go).post(go_consent))
        .route("/r/{token}", get(affiliate).post(affiliate_consent))
        .route(
            "/r/{token}/{series}",
            get(affiliate_series).post(affiliate_series_consent),
        )
        .route(SCOREBOARD, get(scoreboard::show).fallback(crate::not_found))
}

/// Where a click goes and what it is.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub link: Link,
    /// The campaign code (`/go`).
    pub code: Option<String>,
    /// The affiliate's token (`/r`); kept only in the visitor's cookie.
    pub token: Option<String>,
    pub series: Option<String>,
    pub destination: String,
}

impl Target {
    fn key(&self) -> String {
        match (&self.code, &self.token) {
            (Some(code), _) => code.clone(),
            (None, Some(token)) => cookie::affiliate_key(token),
            (None, None) => "unknown".into(),
        }
    }

    /// The consent form's address: the link itself.
    fn path(&self) -> String {
        match (&self.code, &self.token, &self.series) {
            (Some(code), _, _) => format!("/go/{code}"),
            (None, Some(token), Some(series)) => format!("/r/{token}/{series}"),
            (None, Some(token), None) => format!("/r/{token}"),
            (None, None, _) => "/".into(),
        }
    }
}

fn go_target(code: &str) -> Option<Target> {
    let entry = codes::find(code)?;
    Some(Target {
        link: Link::Go,
        code: Some(entry.code.to_owned()),
        token: None,
        series: Some(entry.series().to_owned()),
        destination: entry.destination.to_owned(),
    })
}

/// An affiliate token: `rfr_` and 64 lowercase hex digits.
pub(crate) fn affiliate_token(token: &str) -> bool {
    token.strip_prefix("rfr_").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn affiliate_target(token: &str, series: Option<&str>) -> Option<Target> {
    affiliate_token(token).then(|| Target {
        link: Link::Affiliate,
        code: None,
        token: Some(token.to_owned()),
        // A series that isn't a plain word is ignored, never kept.
        series: series
            .filter(|s| tenancy::accounts::referrals::campaign_word(s))
            .map(str::to_owned),
        destination: AFFILIATE_DESTINATION.to_owned(),
    })
}

async fn go(State(app): State<App>, headers: HeaderMap, Path(code): Path<String>) -> Response {
    match go_target(&code) {
        Some(target) => land(&app, &headers, &target),
        None => unknown(&app, Link::Go),
    }
}

async fn affiliate(
    State(app): State<App>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Response {
    match affiliate_target(&token, None) {
        Some(target) => land(&app, &headers, &target),
        None => unknown(&app, Link::Affiliate),
    }
}

async fn affiliate_series(
    State(app): State<App>,
    headers: HeaderMap,
    Path((token, series)): Path<(String, String)>,
) -> Response {
    match affiliate_target(&token, Some(&series)) {
        Some(target) => land(&app, &headers, &target),
        None => unknown(&app, Link::Affiliate),
    }
}

#[derive(Deserialize)]
struct Choice {
    choice: String,
}

async fn go_consent(
    State(app): State<App>,
    headers: HeaderMap,
    Path(code): Path<String>,
    form: Result<Form<Choice>, FormRejection>,
) -> Response {
    match go_target(&code) {
        Some(target) => decide(&app, &headers, &target, form.ok()),
        None => unknown(&app, Link::Go),
    }
}

async fn affiliate_consent(
    State(app): State<App>,
    headers: HeaderMap,
    Path(token): Path<String>,
    form: Result<Form<Choice>, FormRejection>,
) -> Response {
    match affiliate_target(&token, None) {
        Some(target) => decide(&app, &headers, &target, form.ok()),
        None => unknown(&app, Link::Affiliate),
    }
}

async fn affiliate_series_consent(
    State(app): State<App>,
    headers: HeaderMap,
    Path((token, series)): Path<(String, String)>,
    form: Result<Form<Choice>, FormRejection>,
) -> Response {
    match affiliate_target(&token, Some(&series)) {
        Some(target) => decide(&app, &headers, &target, form.ok()),
        None => unknown(&app, Link::Affiliate),
    }
}

/// A redirect that is never cached or sent on as a referrer.
fn redirect(to: &str) -> Response {
    let mut response = Redirect::to(to).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    response
}

fn unknown(app: &App, link: Link) -> Response {
    app.config
        .attribution
        .count(link, "unknown", Counted::Unknown);
    redirect("/")
}

/// A navigation the visitor made, or one a browser marks as something
/// else (a prefetch, a preview, a subresource): those set nothing.
fn navigation(headers: &HeaderMap) -> bool {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let mode_ok = text("sec-fetch-mode").is_none_or(|m| m == "navigate");
    let purpose = text("sec-purpose")
        .or_else(|| text("purpose"))
        .is_some_and(|p| p.contains("prefetch") || p.contains("prerender"));
    mode_ok && !purpose
}

/// What to do with a click before any choice: count, ask, or remember.
fn land(app: &App, headers: &HeaderMap, target: &Target) -> Response {
    let attribution = &app.config.attribution;
    let client = Client::from_headers(headers);
    let key = target.key();
    if client.quiet {
        attribution.count(target.link, &key, Counted::Quiet);
        return redirect(&target.destination);
    }
    if client.traffic != Traffic::Human {
        attribution.record(new_click(target, &client, false));
        return redirect(&target.destination);
    }
    if !navigation(headers) {
        attribution.count(target.link, &key, Counted::Background);
        return redirect(&target.destination);
    }
    let agreed_before = cookie::read(headers, &attribution.key, now())
        .is_some_and(|t| t.basis == cookie::Basis::Consent);
    let ask = attribution.needs_consent(headers);
    if ask && !agreed_before {
        attribution.count(target.link, &key, Counted::Asked);
        return consent::page(headers, target);
    }
    let basis = if ask || agreed_before {
        cookie::Basis::Consent
    } else {
        cookie::Basis::Notice
    };
    remember(app, &client, target, basis)
}

/// The consent page's answer.
fn decide(app: &App, headers: &HeaderMap, target: &Target, form: Option<Form<Choice>>) -> Response {
    let attribution = &app.config.attribution;
    let client = Client::from_headers(headers);
    let key = target.key();
    let allowed = form.is_some_and(|f| f.choice == "allow");
    if !crate::attribution::same_site(headers) || client.traffic != Traffic::Human {
        return redirect(&target.destination);
    }
    if client.quiet {
        attribution.count(target.link, &key, Counted::Quiet);
        return redirect(&target.destination);
    }
    if !allowed {
        attribution.count(target.link, &key, Counted::Declined);
        return redirect(&target.destination);
    }
    remember(app, &client, target, cookie::Basis::Consent)
}

fn new_click(target: &Target, client: &Client, credited: bool) -> Click {
    Click {
        id: cookie::click_id(),
        at: now(),
        link: target.link,
        key: target.key(),
        series: target.series.clone(),
        destination: target.destination.clone(),
        traffic: client.traffic.name().to_owned(),
        credited,
    }
}

/// Records the click, sets the cookie (replacing any earlier one) and goes on.
fn remember(app: &App, client: &Client, target: &Target, basis: cookie::Basis) -> Response {
    let attribution = &app.config.attribution;
    let click = new_click(target, client, true);
    let ticket = Ticket {
        code: target.code.clone(),
        token: target.token.clone(),
        series: target.series.clone(),
        click: click.id.clone(),
        at: click.at,
        basis,
    };
    attribution.record(click);
    let mut response = redirect(&target.destination);
    if let Ok(value) = HeaderValue::from_str(&cookie::set(
        &ticket,
        &attribution.key,
        app.config.secure_cookies,
    )) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

/// A form post must come from this site's own page.
pub(crate) fn same_site(headers: &HeaderMap) -> bool {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    match text("sec-fetch-site") {
        Some(site) => site == "same-origin",
        None => match (text("origin"), text("host")) {
            (Some(origin), Some(host)) => {
                origin
                    .strip_prefix("http://")
                    .or_else(|| origin.strip_prefix("https://"))
                    == Some(host)
            }
            _ => false,
        },
    }
}

/// The signed-in step of a sign-up: the click this browser holds, if any
/// and still within its window.
pub(crate) fn ticket(app: &App, headers: &HeaderMap) -> Option<Ticket> {
    cookie::read(headers, &app.config.attribution.key, now())
}

/// Whether the browser sent an `oa_ref` cookie at all.
pub(crate) fn has_cookie(headers: &HeaderMap) -> bool {
    cookie::raw(headers).is_some()
}

/// The `Set-Cookie` that clears `oa_ref`.
pub(crate) fn clear(secure: bool) -> HeaderValue {
    HeaderValue::from_str(&cookie::clear(secure)).expect("a static cookie is valid")
}
