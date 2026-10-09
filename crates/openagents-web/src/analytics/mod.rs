//! First-party, cookieless analytics for the website (#11153).
//!
//! What is counted, per hour, as aggregate numbers only ([`Row`]):
//!
//! - page views by route template (`/chat/{id}`, never the address
//!   itself), split into people, agents and bots ([`classify::Traffic`]);
//! - for people's page views: the referring site's name and the device
//!   class (mobile, tablet, desktop);
//! - every request's status code and a latency bucket, by route template;
//! - a fixed list of product events ([`EVENTS`]): some the server already
//!   knows (a chat sent, an answer shown, a sign-in), and a few clicks the
//!   page's own script (`/static/a.js`) reports to `POST /a` with
//!   `navigator.sendBeacon`.
//!
//! What is never read or kept: IP addresses (`X-Forwarded-For` is not
//! read), cookies, account ids, message text, query strings, full referrer
//! addresses, or any identifier of a person or browser. Nothing is set in
//! the browser. There is no visitor count: no row can tell two visits by
//! one person from visits by two people.
//!
//! `DNT: 1` or `Sec-GPC: 1`: the page view is counted (route, traffic
//! class, status, latency) and nothing else: no referrer, no device, no
//! event.
//!
//! Counts are kept in memory and written every minute to the store
//! ([`store`]); the dashboard is `/admin/analytics` ([`dashboard`]).

mod classify;
mod dashboard;
pub mod store;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};

use crate::App;
pub use classify::{Client, Device, Traffic};
pub use store::Store;

/// The beacon's path.
pub const BEACON: &str = "/a";
/// The beacon's script.
pub const SCRIPT: &str = "/static/a.js";
/// The owner's dashboard.
pub const DASHBOARD: &str = "/admin/analytics";

/// Every product event name a row may carry.
pub const EVENTS: [&str; 11] = [
    "chat_sent",
    "answer_shown",
    "answer_failed",
    "starter_clicked",
    "card_clicked",
    "download_clicked",
    "install_copied",
    "installer_fetched",
    "signin_started",
    "signin_completed",
    "api_key_created",
];

/// The events the page's script may send to [`BEACON`]; every other event
/// is the server's own.
pub const BEACON_EVENTS: [&str; 4] = [
    "starter_clicked",
    "card_clicked",
    "download_clicked",
    "install_copied",
];

/// The most distinct counters kept in memory; new ones past it are dropped.
const MAX_KEYS: usize = 20_000;
/// How often counts are written.
const FLUSH_EVERY: Duration = Duration::from_secs(60);
/// How often the daily rollups are remade.
const ROLLUP_EVERY: Duration = Duration::from_secs(3600);
const SCHEMA: &str = "openagents.analytics.rows.v1";

/// Which kind of count a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Series {
    /// `name` a route template, `detail` the traffic class.
    View,
    /// `name` a route template, `detail` a status code.
    Status,
    /// `name` a route template, `detail` a latency bucket.
    Latency,
    /// `name` a referring site, `detail` empty.
    Referrer,
    /// `name` a device class, `detail` empty.
    Device,
    /// `name` an [`EVENTS`] name, `detail` its fixed property or empty.
    Event,
}

/// What a count is of: an hour (hours since 1970 UTC), a series, a name
/// and a detail. Nothing else exists to be stored.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub hour: u64,
    pub series: Series,
    pub name: String,
    pub detail: String,
}

/// One stored count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub hour: u64,
    pub series: Series,
    pub name: String,
    pub detail: String,
    pub count: u64,
}

/// Whether a name or detail is one this module can produce: short, and
/// only lowercase letters, digits and `/ . _ - { } ( ) < >` (route
/// templates, site names, codes, buckets, event names). Rows read back
/// that fail this are ignored.
pub fn bounded(text: &str) -> bool {
    text.len() <= 96
        && text.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(
                    b,
                    b'/' | b'.' | b'_' | b'-' | b'{' | b'}' | b'(' | b')' | b'<' | b'>' | b'*'
                )
        })
}

impl Row {
    pub fn valid(&self) -> bool {
        bounded(&self.name)
            && bounded(&self.detail)
            && (self.series != Series::Event || EVENTS.contains(&self.name.as_str()))
    }
}

#[derive(Serialize, Deserialize)]
struct Batch {
    schema: String,
    rows: Vec<Row>,
}

/// Rows from a stored batch; anything malformed or unbounded is dropped.
fn decode(bytes: &[u8]) -> Vec<Row> {
    serde_json::from_slice::<Batch>(bytes)
        .ok()
        .filter(|batch| batch.schema == SCHEMA)
        .map(|batch| batch.rows.into_iter().filter(Row::valid).collect())
        .unwrap_or_default()
}

fn encode(rows: Vec<Row>) -> Vec<u8> {
    serde_json::to_vec(&Batch {
        schema: SCHEMA.to_owned(),
        rows,
    })
    .unwrap_or_default()
}

pub type Counts = BTreeMap<Key, u64>;

fn add(counts: &mut Counts, rows: impl IntoIterator<Item = Row>) {
    for row in rows {
        *counts
            .entry(Key {
                hour: row.hour,
                series: row.series,
                name: row.name,
                detail: row.detail,
            })
            .or_default() += row.count;
    }
}

fn rows(counts: impl IntoIterator<Item = (Key, u64)>) -> Vec<Row> {
    counts
        .into_iter()
        .map(|(key, count)| Row {
            hour: key.hour,
            series: key.series,
            name: key.name,
            detail: key.detail,
            count,
        })
        .collect()
}

/// The current hour, in hours since 1970 UTC.
pub fn hour_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 3600)
}

/// `YYYY-MM-DD` of a day (days since 1970).
pub fn date(day: u64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = day as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn raw_prefix(day: u64) -> String {
    format!("raw/{}/", date(day))
}

/// The latency bucket of a response's time to its headers.
pub fn latency(elapsed: Duration) -> &'static str {
    match elapsed.as_millis() {
        0..100 => "<100ms",
        100..300 => "100-300ms",
        300..1000 => "300ms-1s",
        1000..3000 => "1-3s",
        _ => ">3s",
    }
}

/// Requests that are not counted at all: assets, health checks, the
/// beacon, the dashboard, and live streams (whose latency means nothing).
fn ignored(template: &str) -> bool {
    template.starts_with("/static/")
        || template.ends_with("/events")
        || matches!(
            template,
            "/favicon.svg" | "/favicon.ico" | "/health" | "/theme" | "/account/avatar"
        )
        || template == BEACON
        || template == DASHBOARD
}

/// The name a request is counted under: its route template, a docs page's
/// own address (the docs are a fixed, public set), or `(not found)`.
pub fn route_name(template: Option<&str>, path: &str, status: u16) -> String {
    match template {
        Some("/docs/{slug}") if status == 200 => {
            let slug = path.strip_prefix("/docs/").unwrap_or_default();
            if !slug.is_empty()
                && slug.len() <= 64
                && slug
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                format!("/docs/{slug}")
            } else {
                "/docs/{slug}".to_owned()
            }
        }
        Some(template) => {
            let template = template.to_ascii_lowercase();
            if bounded(&template) {
                template
            } else {
                "(other)".to_owned()
            }
        }
        None if status == 404 => "(not found)".to_owned(),
        None => "(other)".to_owned(),
    }
}

/// One finished request, as the middleware saw it.
pub struct Seen<'a> {
    pub method: &'a Method,
    pub path: &'a str,
    pub template: Option<&'a str>,
    pub status: u16,
    pub elapsed: Duration,
    pub client: &'a Client,
    /// The response is a page (HTML, or Markdown for an agent).
    pub page: bool,
    /// The response starts a signed-in session.
    pub signed_in: bool,
}

/// The counters one request adds.
pub fn keys(seen: &Seen<'_>) -> Vec<(Series, String, String)> {
    let mut out = Vec::new();
    if seen.template.is_some_and(ignored) {
        return out;
    }
    let route = route_name(seen.template, seen.path, seen.status);
    let client = seen.client;
    let get = seen.method == Method::GET;
    if get && seen.page && (200..300).contains(&seen.status) && !client.fragment {
        out.push((
            Series::View,
            route.clone(),
            client.traffic.name().to_owned(),
        ));
        if !client.quiet && client.traffic == Traffic::Human {
            let site = client
                .referrer
                .clone()
                .unwrap_or_else(|| "(direct)".to_owned());
            out.push((Series::Referrer, site, String::new()));
            out.push((
                Series::Device,
                client.device.name().to_owned(),
                String::new(),
            ));
        }
    }
    out.push((Series::Status, route.clone(), seen.status.to_string()));
    out.push((
        Series::Latency,
        route.clone(),
        latency(seen.elapsed).to_owned(),
    ));
    if !client.quiet && client.traffic != Traffic::Bot {
        let ok = seen.status < 400;
        let template = seen.template.unwrap_or_default();
        let event = match (seen.method.as_str(), template) {
            ("POST", "/chat") if ok => Some(("chat_sent", "new")),
            ("POST", "/chat/{id}") if ok => Some(("chat_sent", "follow-up")),
            ("GET", "/auth/github") if ok => Some(("signin_started", "")),
            ("GET", "/auth/github/callback") if seen.signed_in => Some(("signin_completed", "")),
            ("POST", crate::api_keys::KEYS) if (200..300).contains(&seen.status) => {
                Some(("api_key_created", ""))
            }
            ("GET", "/cli/install.sh") if ok => Some(("installer_fetched", "shell")),
            ("GET", "/cli/install.ps1") if ok => Some(("installer_fetched", "powershell")),
            _ => None,
        };
        if let Some((name, detail)) = event {
            out.push((Series::Event, name.to_owned(), detail.to_owned()));
        }
    }
    out
}

/// The counters, the store they're written to, and the dashboard's key.
pub struct Analytics {
    counts: Mutex<Counts>,
    store: Option<Store>,
    /// This process's name in raw object names: random, per process.
    instance: String,
    /// SHA-256 of the dashboard key; no dashboard without one.
    key: Option<[u8; 32]>,
    flushing: tokio::sync::Mutex<()>,
}

impl Default for Analytics {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl Analytics {
    pub fn new(store: Option<Store>, key: Option<&str>) -> Self {
        let bytes: [u8; 8] = secp256k1::rand::random();
        Self {
            counts: Mutex::new(Counts::new()),
            store,
            instance: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            key: key
                .filter(|key| !key.trim().is_empty())
                .map(|key| sha(key.trim())),
            flushing: tokio::sync::Mutex::new(()),
        }
    }

    /// From the environment: `OPENAGENTS_WEB_ANALYTICS_BUCKET` (a private
    /// bucket) or `OPENAGENTS_WEB_ANALYTICS_DIR` (a directory), and
    /// `OPENAGENTS_WEB_ANALYTICS_KEY`, the dashboard's key. Without a store
    /// the counts stay in memory.
    pub fn from_env() -> Result<Self, store::Error> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let store = match (
            var("OPENAGENTS_WEB_ANALYTICS_BUCKET"),
            var("OPENAGENTS_WEB_ANALYTICS_DIR"),
        ) {
            (Some(bucket), _) => Some(Store::gcs(bucket.trim().to_owned())?),
            (None, Some(directory)) => Some(Store::disk(directory.into())),
            (None, None) => None,
        };
        Ok(Self::new(
            store,
            var("OPENAGENTS_WEB_ANALYTICS_KEY").as_deref(),
        ))
    }

    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    fn count(&self, hour: u64, series: Series, name: String, detail: String) {
        let Ok(mut counts) = self.counts.lock() else {
            return;
        };
        let key = Key {
            hour,
            series,
            name,
            detail,
        };
        if let Some(count) = counts.get_mut(&key) {
            *count += 1;
        } else if counts.len() < MAX_KEYS {
            counts.insert(key, 1);
        }
    }

    /// Counts one finished request.
    pub fn observe(&self, seen: &Seen<'_>) {
        let hour = hour_now();
        for (series, name, detail) in keys(seen) {
            self.count(hour, series, name, detail);
        }
    }

    /// Counts a server event that isn't tied to a request (an answer
    /// finishing). `name` must be in [`EVENTS`].
    pub fn event(&self, name: &'static str, detail: &'static str) {
        if EVENTS.contains(&name) && bounded(detail) {
            self.count(
                hour_now(),
                Series::Event,
                name.to_owned(),
                detail.to_owned(),
            );
        }
    }

    /// Everything counted in memory and not yet dropped, as rows.
    pub fn snapshot(&self) -> Vec<Row> {
        self.counts
            .lock()
            .map(|counts| rows(counts.clone()))
            .unwrap_or_default()
    }

    /// Writes this process's counts for each hour it holds, then forgets
    /// hours that ended over an hour ago.
    pub async fn flush(&self) -> Result<(), store::Error> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        let _one = self.flushing.lock().await;
        let held = self.counts.lock().map(|c| c.clone()).unwrap_or_default();
        let mut by_hour: BTreeMap<u64, Vec<Row>> = BTreeMap::new();
        for row in rows(held) {
            by_hour.entry(row.hour).or_default().push(row);
        }
        let now = hour_now();
        let mut written = Vec::new();
        for (hour, rows) in by_hour {
            let name = format!(
                "raw/{}/{:02}/{}.json",
                date(hour / 24),
                hour % 24,
                self.instance
            );
            store.put(&name, encode(rows)).await?;
            written.push(hour);
        }
        if let Ok(mut counts) = self.counts.lock() {
            counts.retain(|key, _| !(written.contains(&key.hour) && key.hour + 1 < now));
        }
        Ok(())
    }

    /// Writes counts every minute and remakes the last two days' rollups
    /// every hour, on the current Tokio runtime.
    pub fn spawn(self: &Arc<Self>) {
        if self.store.is_none() {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let mut flush = tokio::time::interval(FLUSH_EVERY);
            flush.tick().await;
            let mut rolled = Instant::now() - ROLLUP_EVERY + Duration::from_secs(120);
            loop {
                flush.tick().await;
                if let Err(error) = me.flush().await {
                    eprintln!("analytics: {error}");
                }
                if rolled.elapsed() >= ROLLUP_EVERY {
                    rolled = Instant::now();
                    let today = hour_now() / 24;
                    for day in [today - 1, today - 2] {
                        if let Some(store) = &me.store
                            && let Err(error) = rollup(store, day).await
                        {
                            eprintln!("analytics: rollup {}: {error}", date(day));
                        }
                    }
                }
            }
        });
    }

    /// Whether `offered` is the dashboard key.
    fn admits(&self, offered: &str) -> bool {
        let Some(key) = &self.key else {
            return false;
        };
        let offered = sha(offered.trim());
        offered
            .iter()
            .zip(key.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }

    /// The counts for the `days` days ending today: the daily rollups for
    /// older days and the raw files for today and yesterday, plus what this
    /// process holds and has not written.
    pub async fn load(&self, days: u64) -> Result<Counts, store::Error> {
        let mut counts = Counts::new();
        let today = hour_now() / 24;
        let first = today.saturating_sub(days.saturating_sub(1));
        let Some(store) = &self.store else {
            add(
                &mut counts,
                self.snapshot().into_iter().filter(|r| r.hour / 24 >= first),
            );
            return Ok(counts);
        };
        self.flush().await?;
        for day in first..=today {
            let rows = if day + 1 >= today {
                read_raw(store, day).await?
            } else if let Some(bytes) = store.get(&format!("daily/{}.json", date(day))).await? {
                decode(&bytes)
            } else {
                read_raw(store, day).await?
            };
            add(&mut counts, rows);
        }
        Ok(counts)
    }
}

fn sha(text: &str) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes()).into()
}

/// The sum of a day's raw files.
async fn read_raw(store: &Store, day: u64) -> Result<Vec<Row>, store::Error> {
    use futures_util::stream::{self, StreamExt};
    let names = store.list(&raw_prefix(day)).await?;
    let batches: Vec<_> = stream::iter(names)
        .map(|name| async move { store.get(&name).await })
        .buffer_unordered(16)
        .collect()
        .await;
    let mut counts = Counts::new();
    for batch in batches {
        if let Some(bytes) = batch? {
            add(&mut counts, decode(&bytes));
        }
    }
    Ok(rows(counts))
}

/// Remakes `daily/DAY.json` from the day's raw files (kept 30 days), and
/// returns its rows.
pub async fn rollup(store: &Store, day: u64) -> Result<Vec<Row>, store::Error> {
    let rows = read_raw(store, day).await?;
    if !rows.is_empty() {
        store
            .put(&format!("daily/{}.json", date(day)), encode(rows.clone()))
            .await?;
    }
    Ok(rows)
}

/// The route template of the matched route, set on the response by
/// [`mark`] for [`observe`].
#[derive(Clone)]
struct Template(String);

/// Inside routing: records which route matched.
pub(crate) async fn mark(matched: Option<MatchedPath>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    if let Some(matched) = matched {
        response
            .extensions_mut()
            .insert(Template(matched.as_str().to_owned()));
    }
    response
}

/// Around routing: counts the finished request.
pub(crate) async fn observe(State(app): State<App>, request: Request, next: Next) -> Response {
    let started = Instant::now();
    let client = Client::from_headers(request.headers());
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    let elapsed = started.elapsed();
    let kind = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let page = kind.starts_with("text/html") || kind.starts_with("text/markdown");
    let signed_in = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| {
            v.split(';')
                .next()
                .and_then(|pair| pair.trim().split_once('='))
                .is_some_and(|(name, value)| name == "oa_cloud_session" && !value.is_empty())
        });
    let template = response
        .extensions()
        .get::<Template>()
        .map(|t| t.0.as_str());
    app.config.analytics.observe(&Seen {
        method: &method,
        path: &path,
        template,
        status: response.status().as_u16(),
        elapsed,
        client: &client,
        page,
        signed_in,
    });
    response
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(BEACON, post(beacon))
        .route(SCRIPT, get(script))
        .route(DASHBOARD, get(dashboard::show).post(dashboard::sign_in))
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../../static/a.js"),
    )
        .into_response()
}

struct Beacon {
    e: String,
    d: String,
}

/// What a beacon's event and value are counted as, or `None` when the
/// pair isn't one the page sends. Values are matched against the site's
/// own fixed lists; the value sent is never stored.
pub fn beacon_event(event: &str, value: &str) -> Option<(&'static str, String)> {
    let value = value.trim();
    match event {
        "starter_clicked" => openagents_chat::suggestions::SUGGESTIONS
            .iter()
            .find(|s| s.label == value || s.id == value)
            .filter(|s| bounded(s.id))
            .map(|s| ("starter_clicked", s.id.to_owned())),
        "card_clicked" => openagents_chat::home_cards::HOME_CARDS
            .iter()
            .find(|card| card.href == value || card.id == value)
            .filter(|card| bounded(card.id))
            .map(|card| ("card_clicked", card.id.to_owned())),
        "download_clicked" => download_platform(value).map(|p| ("download_clicked", p)),
        "install_copied" => {
            matches!(value, "shell" | "powershell").then(|| ("install_copied", value.to_owned()))
        }
        _ => None,
    }
}

/// `desktop-macos`, `coder-linux-x86_64`, ...: the platform of one of the
/// download page's own links.
fn download_platform(href: &str) -> Option<String> {
    use crate::pages::download::{CODER_BASE, CODER_PLATFORMS, DOWNLOADS};
    if DOWNLOADS.iter().any(|d| d.url == Some(href)) {
        let os = ["macos", "linux", "windows"]
            .into_iter()
            .find(|os| href.contains(&format!("/desktop/{os}/")))?;
        return Some(format!("desktop-{os}"));
    }
    let file = href.strip_prefix(CODER_BASE)?.rsplit('/').next()?;
    CODER_PLATFORMS
        .iter()
        .map(|(_, slug)| *slug)
        .filter(|slug| {
            file.ends_with(&format!("-{slug}.tar.gz")) || file.ends_with(&format!("-{slug}.zip"))
        })
        .max_by_key(|slug| slug.len())
        .map(|slug| format!("coder-{slug}"))
}

/// `POST /a`: one click from the page's script. Answers `204` whether or
/// not it was counted, sets nothing, and keeps nothing of the request
/// but the event's name and fixed value.
async fn beacon(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let mut response = if body.len() > 1024 {
        StatusCode::PAYLOAD_TOO_LARGE.into_response()
    } else {
        let client = Client::from_headers(&headers);
        let parsed: Option<Beacon> = parse_beacon(&body);
        match parsed.and_then(|b| beacon_event(&b.e, &b.d)) {
            None => StatusCode::BAD_REQUEST.into_response(),
            Some(_) if client.quiet || client.traffic == Traffic::Bot => {
                StatusCode::NO_CONTENT.into_response()
            }
            Some((name, detail)) => {
                app.config
                    .analytics
                    .count(hour_now(), Series::Event, name.to_owned(), detail);
                StatusCode::NO_CONTENT.into_response()
            }
        }
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A small `application/x-www-form-urlencoded` (or `text/plain` holding
/// the same) body.
fn parse_beacon(body: &[u8]) -> Option<Beacon> {
    let mut event = None;
    let mut value = String::new();
    for (name, v) in url::form_urlencoded::parse(body) {
        match name.as_ref() {
            "e" => event = Some(v.into_owned()),
            "d" => value = v.into_owned(),
            _ => {}
        }
    }
    Some(Beacon {
        e: event?,
        d: value,
    })
}
