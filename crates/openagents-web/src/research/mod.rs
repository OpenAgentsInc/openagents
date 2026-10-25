//! The playtest research stream (Kiki beta launch, 2026-10-24).
//!
//! A separate, disclosed stream beside the cookieless site counts
//! ([`crate::analytics`]) and the campaign-link records
//! ([`crate::attribution`]); neither of those reads anything here.
//!
//! On the launch funnel's pages ([`funnel_page`]: `/`, `/new-lander…`,
//! `/playtest…`, the survey, and the house and Verse pages) the site adds a
//! small first-party script, `/static/r.js` ([`inject`]). It reports, in
//! batches sent with `navigator.sendBeacon` to [`BEACON`]: page views,
//! clicks (the target's `data-aethyr-id`, id, role, and up to 60 characters
//! of its visible text, never an input's value), route changes, visibility
//! and time on the page, scroll depth, form submits (field names only),
//! script errors (the message and the script file, no stack), engine and
//! bridge states the page exposes, and the viewport and device class.
//!
//! Each event carries the playtest session's id (the `oa_playtest` cookie
//! that `/playtest` sets) or a random id the tab keeps in
//! `sessionStorage`. When the visitor is signed in, the server joins the
//! account id. It never stores an IP address or the user agent: only a
//! coarse class such as `mobile/ios/safari`.
//!
//! Opting out (the playtest intro's toggle, `DNT: 1`, or `Sec-GPC: 1`):
//! the script sends no id, label, or property, and the server keeps only a
//! count per day, kind, and path ([`Count`]). Server steps for an
//! opted-out session are counted the same way.
//!
//! Events are written append-only, one new object per flush, under
//! `research/events/DATE/` in the analytics store, and deleted after the
//! retention period ([`RETENTION_ENV`], 180 days by default). The owner's
//! view is [`DASHBOARD`] ([`dashboard`]), behind the analytics dashboard's
//! admin gate.

pub mod dashboard;
pub mod funnel;
pub mod ingest;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::analytics::{Client, Store, Traffic, store};

/// Where the script sends batches.
pub const BEACON: &str = "/a/r";
/// The capture script.
pub const SCRIPT: &str = "/static/r.js";
/// The owner's funnel and insights view.
pub const DASHBOARD: &str = "/admin/research";
/// The CSV export.
pub const EXPORT: &str = "/admin/research.csv";
/// The cookie `/playtest` sets with the playtest session's id.
pub const SESSION_COOKIE: &str = "oa_playtest";
/// Days to keep events, when set (1 to 400).
pub const RETENTION_ENV: &str = "OPENAGENTS_WEB_RESEARCH_RETENTION_DAYS";
/// The default retention, in days.
pub const RETENTION_DAYS: u64 = 180;

/// The most events one session may send in a [`WINDOW`].
pub const MOST_PER_SESSION: usize = 600;
/// The most events this process admits in a [`WINDOW`], from everyone.
pub const MOST_PER_WINDOW: usize = 60_000;
/// The rate window.
pub const WINDOW: Duration = Duration::from_secs(600);
/// The most events held between flushes; more are dropped.
const MOST_HELD: usize = 200_000;
/// The most sessions whose rate or account is remembered.
const MOST_SESSIONS: usize = 100_000;
/// The most events kept in the read cache.
const MOST_CACHED: usize = 3_000_000;
const FLUSH_EVERY: Duration = Duration::from_secs(30);
const PRUNE_EVERY: Duration = Duration::from_secs(3600);
const EVENTS_PREFIX: &str = "research/events/";
const COUNTS_PREFIX: &str = "research/counts/";
const EVENTS_SCHEMA: &str = "openagents.research.events.v1";
const COUNTS_SCHEMA: &str = "openagents.research.counts.v1";

/// One recorded event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Unix milliseconds, by the server's clock.
    pub at: u64,
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// One of [`ingest::KINDS`].
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    /// What was clicked, or a milestone's or step's name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, String>,
    /// `mobile/ios/safari` and the like; empty for server steps.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub campaign: String,
}

impl Event {
    pub fn day(&self) -> u64 {
        self.at / 86_400_000
    }
}

/// An aggregate count kept when the visitor opted out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub day: u64,
    pub kind: String,
    pub path: String,
    pub count: u64,
}

#[derive(Serialize, Deserialize)]
struct EventsFile {
    schema: String,
    events: Vec<Event>,
}

#[derive(Serialize, Deserialize)]
struct CountsFile {
    schema: String,
    counts: Vec<Count>,
}

/// Why a batch was not admitted.
#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    /// Kept (or counted), with how many events were refused.
    Kept { refused: usize },
    /// The session or the process is over its rate.
    Limited,
}

#[derive(Default)]
struct Held {
    events: Vec<Event>,
    counts: BTreeMap<(u64, String, String), u64>,
    /// Admission times per session, for the rate limit.
    rates: HashMap<String, VecDeque<u64>>,
    /// Admission times for everyone.
    global: VecDeque<(u64, usize)>,
    global_total: usize,
    /// Sessions that opted out while this process ran.
    opted_out: HashSet<String>,
    /// Sessions whose account is known (or known to be none).
    accounts: HashMap<String, Option<String>>,
}

/// The stream's buffer, store and read cache.
pub struct Research {
    store: Option<Store>,
    instance: String,
    retention_days: u64,
    held: Mutex<Held>,
    sequence: Mutex<u64>,
    cache: tokio::sync::Mutex<HashMap<String, Arc<Vec<Event>>>>,
    flushing: tokio::sync::Mutex<()>,
}

impl Default for Research {
    fn default() -> Self {
        Self::new(None, RETENTION_DAYS)
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Research {
    pub fn new(store: Option<Store>, retention_days: u64) -> Self {
        let bytes: [u8; 8] = secp256k1::rand::random();
        Self {
            store,
            instance: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            retention_days: retention_days.clamp(1, 400),
            held: Mutex::new(Held::default()),
            sequence: Mutex::new(0),
            cache: tokio::sync::Mutex::new(HashMap::new()),
            flushing: tokio::sync::Mutex::new(()),
        }
    }

    /// From the environment: the analytics store (bucket or directory) and
    /// [`RETENTION_ENV`].
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
        let days = var(RETENTION_ENV)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(RETENTION_DAYS);
        Ok(Self::new(store, days))
    }

    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    pub fn retention_days(&self) -> u64 {
        self.retention_days
    }

    /// Whether `session` may add `events` more at `now`; records them if so.
    fn rate(held: &mut Held, session: Option<&str>, events: usize, now: u64) -> bool {
        let window = WINDOW.as_millis() as u64;
        while held
            .global
            .front()
            .is_some_and(|(at, _)| at + window <= now)
        {
            if let Some((_, n)) = held.global.pop_front() {
                held.global_total -= n;
            }
        }
        if held.global_total + events > MOST_PER_WINDOW {
            return false;
        }
        if let Some(session) = session {
            if !held.rates.contains_key(session) && held.rates.len() >= MOST_SESSIONS {
                held.rates
                    .retain(|_, times| times.back().is_some_and(|t| t + window > now));
                if held.rates.len() >= MOST_SESSIONS {
                    return false;
                }
            }
            let times = held.rates.entry(session.to_owned()).or_default();
            while times.front().is_some_and(|t| t + window <= now) {
                times.pop_front();
            }
            if times.len() + events > MOST_PER_SESSION {
                return false;
            }
            times.extend(std::iter::repeat_n(now, events));
        }
        held.global.push_back((now, events));
        held.global_total += events;
        true
    }

    fn count(held: &mut Held, day: u64, kind: &str, path: &str) {
        let key = (day, kind.to_owned(), path.to_owned());
        if let Some(count) = held.counts.get_mut(&key) {
            *count += 1;
        } else if held.counts.len() < MOST_HELD {
            held.counts.insert(key, 1);
        }
    }

    /// Admits a checked batch. `quiet` is the browser's `DNT` or `Sec-GPC`;
    /// `device` the coarse class; `account` the signed-in account.
    pub fn admit(
        &self,
        mut batch: ingest::Batch,
        quiet: bool,
        device: &str,
        account: Option<String>,
        now: u64,
    ) -> Admission {
        let Ok(mut held) = self.held.lock() else {
            return Admission::Limited;
        };
        let refused = batch.refused;
        if let Some(session) = &batch.session
            && batch.opt_out
        {
            if held.opted_out.len() < MOST_SESSIONS {
                held.opted_out.insert(session.clone());
            }
            held.events.retain(|e| &e.session != session);
            held.accounts.remove(session);
        }
        let aggregate = quiet
            || batch.opt_out
            || batch
                .session
                .as_ref()
                .is_none_or(|s| held.opted_out.contains(s));
        if !Self::rate(&mut held, batch.session.as_deref(), batch.events.len(), now) {
            return Admission::Limited;
        }
        if aggregate {
            for event in &batch.events {
                Self::count(&mut held, event.day(), &event.kind, &event.path);
            }
            return Admission::Kept { refused };
        }
        let session = batch.session.take().unwrap_or_default();
        for mut event in batch.events {
            if held.events.len() >= MOST_HELD {
                break;
            }
            event.session = session.clone();
            event.account = account.clone();
            event.device = device.to_owned();
            event.source = batch.source.clone();
            event.campaign = batch.campaign.clone();
            held.events.push(event);
        }
        Admission::Kept { refused }
    }

    /// Records a playtest step from the server (`playtest::events`).
    pub fn step(&self, session: &str, step: &str, props: &[(&str, &str)], now: u64) {
        if !ingest::valid_name(step) {
            return;
        }
        let Ok(mut held) = self.held.lock() else {
            return;
        };
        if !ingest::valid_session(session) || held.opted_out.contains(session) {
            Self::count(&mut held, now / 86_400_000, "step", "");
            return;
        }
        if !Self::rate(&mut held, Some(session), 1, now) || held.events.len() >= MOST_HELD {
            return;
        }
        let props = props
            .iter()
            .take(ingest::MOST_PROPS)
            .filter(|(k, _)| ingest::valid_name(k))
            .map(|(k, v)| ((*k).to_owned(), ingest::scrub(v, ingest::MOST_VALUE)))
            .collect();
        let account = held.accounts.get(session).cloned().flatten();
        held.events.push(Event {
            at: now,
            session: session.to_owned(),
            account,
            kind: "step".into(),
            path: String::new(),
            label: step.to_owned(),
            props,
            device: String::new(),
            source: String::new(),
            campaign: String::new(),
        });
    }

    /// The account a session was joined to, when this process knows it.
    fn known_account(&self, session: &str) -> Option<Option<String>> {
        self.held
            .lock()
            .ok()
            .and_then(|held| held.accounts.get(session).cloned())
    }

    fn remember_account(&self, session: &str, account: Option<String>) {
        if let Ok(mut held) = self.held.lock() {
            if held.accounts.len() >= MOST_SESSIONS {
                held.accounts.clear();
            }
            held.accounts.insert(session.to_owned(), account);
        }
    }

    /// The events and counts held and not yet written.
    pub fn snapshot(&self) -> (Vec<Event>, Vec<Count>) {
        self.held
            .lock()
            .map(|held| (held.events.clone(), counts_of(&held.counts)))
            .unwrap_or_default()
    }

    /// Writes what is held as new objects (never rewriting one), then lets
    /// it go.
    pub async fn flush(&self) -> Result<(), store::Error> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        let _one = self.flushing.lock().await;
        let (events, counts) = match self.held.lock() {
            Ok(mut held) => (
                std::mem::take(&mut held.events),
                std::mem::take(&mut held.counts),
            ),
            Err(_) => return Ok(()),
        };
        let mut by_day: BTreeMap<u64, Vec<Event>> = BTreeMap::new();
        for event in events {
            by_day.entry(event.day()).or_default().push(event);
        }
        let mut counts_by_day: BTreeMap<u64, Vec<Count>> = BTreeMap::new();
        for count in counts_of(&counts) {
            counts_by_day.entry(count.day).or_default().push(count);
        }
        let mut failed = None;
        for (day, events) in by_day {
            let name = format!(
                "{EVENTS_PREFIX}{}/{}.json",
                crate::analytics::date(day),
                self.next_name()
            );
            let bytes = serde_json::to_vec(&EventsFile {
                schema: EVENTS_SCHEMA.into(),
                events: events.clone(),
            })
            .unwrap_or_default();
            if let Err(error) = store.put(&name, bytes).await {
                // Put them back for the next flush.
                if let Ok(mut held) = self.held.lock() {
                    held.events.extend(events);
                }
                failed = Some(error);
            }
        }
        for (day, counts) in counts_by_day {
            let name = format!(
                "{COUNTS_PREFIX}{}/{}.json",
                crate::analytics::date(day),
                self.next_name()
            );
            let bytes = serde_json::to_vec(&CountsFile {
                schema: COUNTS_SCHEMA.into(),
                counts: counts.clone(),
            })
            .unwrap_or_default();
            if let Err(error) = store.put(&name, bytes).await {
                if let Ok(mut held) = self.held.lock() {
                    for count in counts {
                        *held
                            .counts
                            .entry((count.day, count.kind, count.path))
                            .or_default() += count.count;
                    }
                }
                failed = Some(error);
            }
        }
        failed.map_or(Ok(()), Err)
    }

    fn next_name(&self) -> String {
        let mut sequence = self.sequence.lock().unwrap_or_else(|e| e.into_inner());
        *sequence += 1;
        format!("{}-{:08}", self.instance, *sequence)
    }

    /// Deletes stored events and counts older than the retention period.
    pub async fn prune(&self, today: u64) -> Result<usize, store::Error> {
        let Some(store) = &self.store else {
            return Ok(0);
        };
        let first = today.saturating_sub(self.retention_days - 1);
        let mut deleted = 0;
        for prefix in [EVENTS_PREFIX, COUNTS_PREFIX] {
            for name in store.list(prefix).await? {
                let day = name
                    .strip_prefix(prefix)
                    .and_then(|rest| rest.split('/').next())
                    .and_then(day_of);
                if day.is_some_and(|day| day < first) {
                    store.delete(&name).await?;
                    self.cache.lock().await.remove(&name);
                    deleted += 1;
                }
            }
        }
        Ok(deleted)
    }

    /// The events and counts of the `days` days ending today: what is
    /// stored and what this process holds.
    pub async fn load(&self, days: u64) -> Result<(Vec<Event>, Vec<Count>), store::Error> {
        let today = now_ms() / 86_400_000;
        let first = today.saturating_sub(days.clamp(1, self.retention_days) - 1);
        let Some(store) = &self.store else {
            let (mut events, mut counts) = self.snapshot();
            events.retain(|e| e.day() >= first);
            counts.retain(|c| c.day >= first);
            return Ok((events, counts));
        };
        self.flush().await?;
        let mut events = Vec::new();
        let mut counts = Vec::new();
        for day in first..=today {
            let date = crate::analytics::date(day);
            for name in store.list(&format!("{EVENTS_PREFIX}{date}/")).await? {
                let cached = self.cache.lock().await.get(&name).cloned();
                let file = match cached {
                    Some(file) => file,
                    None => {
                        let file = Arc::new(
                            store
                                .get(&name)
                                .await?
                                .and_then(|bytes| serde_json::from_slice::<EventsFile>(&bytes).ok())
                                .filter(|f| f.schema == EVENTS_SCHEMA)
                                .map(|f| f.events)
                                .unwrap_or_default(),
                        );
                        let mut cache = self.cache.lock().await;
                        if cache.values().map(|f| f.len()).sum::<usize>() > MOST_CACHED {
                            cache.clear();
                        }
                        cache.insert(name, file.clone());
                        file
                    }
                };
                events.extend(file.iter().cloned());
            }
            for name in store.list(&format!("{COUNTS_PREFIX}{date}/")).await? {
                if let Some(file) = store
                    .get(&name)
                    .await?
                    .and_then(|bytes| serde_json::from_slice::<CountsFile>(&bytes).ok())
                    .filter(|f| f.schema == COUNTS_SCHEMA)
                {
                    counts.extend(file.counts);
                }
            }
        }
        let (held_events, held_counts) = self.snapshot();
        events.extend(held_events);
        counts.extend(held_counts);
        events.sort_by(|a, b| (a.at, &a.session).cmp(&(b.at, &b.session)));
        Ok((events, counts))
    }

    /// Flushes every 30 seconds and prunes every hour.
    pub fn spawn(self: &Arc<Self>) {
        if self.store.is_none() {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(FLUSH_EVERY);
            tick.tick().await;
            let mut pruned = std::time::Instant::now() - PRUNE_EVERY + Duration::from_secs(300);
            loop {
                tick.tick().await;
                if let Err(error) = me.flush().await {
                    eprintln!("research: {error}");
                }
                if pruned.elapsed() >= PRUNE_EVERY {
                    pruned = std::time::Instant::now();
                    if let Err(error) = me.prune(now_ms() / 86_400_000).await {
                        eprintln!("research: deleting old events: {error}");
                    }
                }
            }
        });
    }
}

fn counts_of(counts: &BTreeMap<(u64, String, String), u64>) -> Vec<Count> {
    counts
        .iter()
        .map(|((day, kind, path), count)| Count {
            day: *day,
            kind: kind.clone(),
            path: path.clone(),
            count: *count,
        })
        .collect()
}

/// The day number of a `YYYY-MM-DD` folder.
fn day_of(date: &str) -> Option<u64> {
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u64::try_from(era * 146_097 + doe - 719_468).ok()
}

/// The `playtest::events` sink: steps go to the research stream.
struct Sink(Arc<Research>);

impl playtest::events::Sink for Sink {
    fn record(&self, session_id: &str, step: &str, props: &[(&str, &str)]) {
        self.0.step(session_id, step, props, now_ms());
    }
}

/// Installs the research stream as the process's playtest step sink.
pub fn install_sink(research: &Arc<Research>) -> bool {
    playtest::events::install(Box::new(Sink(research.clone())))
}

/// Whether this module answers `path`.
pub(crate) fn owns(path: &str) -> bool {
    matches!(path, BEACON | SCRIPT | DASHBOARD | EXPORT)
}

/// Whether a page at `path` belongs to the launch funnel. A file under a
/// funnel prefix (its last segment has an extension, like
/// `/verse/hands/left.glb`) is not a page.
pub fn funnel_page(path: &str) -> bool {
    const PREFIXES: [&str; 9] = [
        "/beta",
        "/new-lander",
        "/playtest",
        "/survey",
        "/house",
        "/verse",
        "/kiki",
        "/waitlist",
        "/signup",
    ];
    if path.rsplit('/').next().is_some_and(|name| name.contains('.')) {
        return false;
    }
    path == "/"
        || PREFIXES.iter().any(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '-']))
        })
}

/// The script tag the funnel's pages get.
const TAG: &str = "<script src=\"/static/r.js\" defer></script>";
/// The largest page the tag is added to.
const MOST_PAGE: usize = 4 * 1024 * 1024;

/// Adds the capture script to a funnel page, and lets the page's policy
/// load it from this site and send to this site.
pub(crate) async fn inject(mut request: Request, next: Next) -> Response {
    let wanted = request.method() == Method::GET && funnel_page(request.uri().path());
    if wanted {
        // Some pages answer with a body compressed ahead of time; ask for
        // the plain one. The site's outer compression layer has already
        // read this header and compresses the finished page.
        request.headers_mut().remove(header::ACCEPT_ENCODING);
    }
    let response = next.run(request).await;
    if !wanted || response.status() != StatusCode::OK {
        return response;
    }
    let headers = response.headers();
    let html = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !html || headers.contains_key(header::CONTENT_ENCODING) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MOST_PAGE).await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "The page couldn't be loaded.",
        )
            .into_response();
    };
    let page = add_tag(&bytes);
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.remove(header::ETAG);
    let policy = parts
        .headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or(crate::SITE_POLICY)
        .to_owned();
    if let Ok(value) = HeaderValue::from_str(&allow_self(&policy)) {
        parts.headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    Response::from_parts(parts, Body::from(page))
}

/// The page with [`TAG`] before `</head>` (or `</body>`, or at the end),
/// unless it already has it.
pub fn add_tag(page: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(page);
    if text.contains(SCRIPT) {
        return page.to_vec();
    }
    let at = text
        .find("</head>")
        .or_else(|| text.rfind("</body>"))
        .unwrap_or(text.len());
    let mut out = String::with_capacity(text.len() + TAG.len());
    out.push_str(&text[..at]);
    out.push_str(TAG);
    out.push_str(&text[at..]);
    out.into_bytes()
}

/// `policy` with `'self'` allowed for scripts and connections: added to
/// `script-src` and `connect-src`, or those directives made from
/// `default-src` plus `'self'`. Nothing else changes.
pub fn allow_self(policy: &str) -> String {
    let mut directives: Vec<(String, Vec<String>)> = policy
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut words = d.split_whitespace().map(str::to_owned);
            let name = words.next().unwrap_or_default().to_ascii_lowercase();
            (name, words.collect())
        })
        .collect();
    let default: Vec<String> = directives
        .iter()
        .find(|(name, _)| name == "default-src")
        .map(|(_, values)| values.clone())
        .unwrap_or_default();
    for wanted in ["script-src", "connect-src"] {
        match directives.iter_mut().find(|(name, _)| name == wanted) {
            Some((_, values)) => {
                values.retain(|v| v != "'none'");
                if !values.iter().any(|v| v == "'self'") {
                    values.push("'self'".into());
                }
            }
            None => {
                let mut values: Vec<String> =
                    default.iter().filter(|v| *v != "'none'").cloned().collect();
                if !values.iter().any(|v| v == "'self'") {
                    values.push("'self'".into());
                }
                directives.push((wanted.to_owned(), values));
            }
        }
    }
    directives
        .into_iter()
        .map(|(name, values)| {
            if values.is_empty() {
                name
            } else {
                format!("{name} {}", values.join(" "))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(BEACON, post(beacon))
        .route(SCRIPT, get(script))
        .route(EXPORT, get(dashboard::export))
        .route(DASHBOARD, get(dashboard::show))
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../../static/r.js"),
    )
        .into_response()
}

/// The coarse class of the browser that sent `headers`.
pub fn device_class(headers: &HeaderMap) -> String {
    let client = Client::from_headers(headers);
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    format!(
        "{}/{}/{}",
        client.device.name(),
        ingest::system(agent),
        ingest::browser(agent)
    )
}

/// The session id in the `oa_playtest` cookie, if it holds a valid one.
pub fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| ingest::valid_session(value))
}

/// `POST /a/r`: a batch from the capture script. `204` when kept or
/// counted (or dropped for a bot), `400` when malformed or any event was
/// refused (the rest are kept), `413` when too large, `429` over the rate.
async fn beacon(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let research = &app.config.research;
    let now = now_ms();
    let client = Client::from_headers(&headers);
    let status = match ingest::parse(&body, now) {
        Err(ingest::Refusal::TooLarge) => StatusCode::PAYLOAD_TOO_LARGE,
        Err(ingest::Refusal::Malformed) => StatusCode::BAD_REQUEST,
        Ok(_) if client.traffic != Traffic::Human => StatusCode::NO_CONTENT,
        Ok(batch) => {
            let account = match batch.session.as_deref() {
                Some(session) if !client.quiet && !batch.opt_out => {
                    account_for(&app, &headers, session).await
                }
                _ => None,
            };
            match research.admit(batch, client.quiet, &device_class(&headers), account, now) {
                Admission::Limited => StatusCode::TOO_MANY_REQUESTS,
                Admission::Kept { refused: 0 } => StatusCode::NO_CONTENT,
                Admission::Kept { .. } => StatusCode::BAD_REQUEST,
            }
        }
    };
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// The signed-in account for `session`: asked of the account service once
/// per session, then remembered.
async fn account_for(app: &App, headers: &HeaderMap, session: &str) -> Option<String> {
    let research = &app.config.research;
    if !crate::account::has_session(headers) {
        return research.known_account(session).flatten();
    }
    if let Some(Some(known)) = research.known_account(session) {
        return Some(known);
    }
    let service = app.config.cloud.as_deref()?;
    let account = service
        .authenticate(headers)
        .await
        .ok()
        .map(|viewer| viewer.account_id);
    research.remember_account(session, account.clone());
    account
}
