//! Per-visit events on the Grid homepage (#12439).
//!
//! The page makes a random visit id when a tab opens `/` and keeps it in
//! that tab's `sessionStorage`, so it ends when the tab closes. Each event
//! the page sends carries that id, its number in the visit, a name from
//! [`EVENTS`], and a few details whose keys and values come from that
//! event's fixed list, or a duration the server turns into a bucket
//! ([`TIME`]). Nothing else is read or kept: no IP address, cookie,
//! account, key, message text, or free text, and the id is never joined to
//! any of them. A browser that sends `DNT: 1` or `Sec-GPC: 1` gets no id
//! and sends no events.
//!
//! Rows ([`VisitRow`]) are kept apart from the hourly counts, in
//! `visits/YYYY-MM-DD/HH/INSTANCE.json`, and deleted after
//! [`KEEP_DAYS`] days ([`prune`]).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::store::{self, Store};
use super::{bounded, date};

/// How long per-visit rows are kept, in days.
pub const KEEP_DAYS: u64 = 30;
/// The beacon the page sends per-visit events to.
pub const BEACON: &str = "/a/v";
/// The routes whose page sends per-visit events.
pub const ROUTES: [&str; 1] = ["/"];
/// The most events one beacon carries.
pub const MOST_PER_BEACON: usize = 50;
/// The largest beacon body, in bytes.
pub const MOST_BYTES: usize = 16 * 1024;
/// The most events kept for one visit in one hour; more are dropped.
const MOST_PER_VISIT: u32 = 400;
/// The most rows held in memory between flushes; more are dropped.
const MOST_HELD: usize = 100_000;
/// The highest event number a visit may send.
const MOST_NUMBER: u32 = 10_000;
/// The prefix the rows are stored under.
const PREFIX: &str = "visits/";
const SCHEMA: &str = "openagents.analytics.visits.v1";

/// The duration buckets a `time` detail becomes.
pub const TIME: [&str; 9] = [
    "<1s", "1-3s", "3-10s", "10-30s", "30s-1m", "1-3m", "3-10m", "10-30m", ">30m",
];

/// The bucket of a duration in milliseconds.
pub fn time_bucket(ms: u64) -> &'static str {
    match ms {
        0..1_000 => TIME[0],
        1_000..3_000 => TIME[1],
        3_000..10_000 => TIME[2],
        10_000..30_000 => TIME[3],
        30_000..60_000 => TIME[4],
        60_000..180_000 => TIME[5],
        180_000..600_000 => TIME[6],
        600_000..1_800_000 => TIME[7],
        _ => TIME[8],
    }
}

/// The places a Grid portal leads to, by the names the page's Verse
/// events give them (`verse-zone-load` and `verse-zone-unavailable`,
/// #12438), without a `verse-` prefix or `-v1` suffix; any other is
/// `other`.
pub const ZONES: [&str; 17] = [
    "grid",
    "lagrange1",
    "lagrange2",
    "everglade",
    "hosted",
    "physics-lab",
    "robot-lab",
    "water-lab",
    "coast",
    "crypt",
    "grove",
    "arena",
    "duel",
    "hand-duel",
    "meteor-stress-test",
    "meteor-showcase",
    "other",
];

/// The model families an answer is recorded as.
pub const FAMILIES: [&str; 10] = [
    "deepseek", "flint", "gemma", "qwen", "llama", "mistral", "gpt", "claude", "gemini", "other",
];

/// The family of a model's name, from [`FAMILIES`]: the first family whose
/// name the model's contains (`gemini-2.5-flash` is `gemini`,
/// `deepseek-v4.1-flash` is `deepseek`), with OpenAI's `o`-series as
/// `gpt`. Anything else is `other`; the name itself is never kept.
pub fn model_family(model: &str) -> &'static str {
    let model = model.trim().to_ascii_lowercase();
    let model = model.rsplit('/').next().unwrap_or_default();
    if let Some(family) = FAMILIES
        .iter()
        .take(FAMILIES.len() - 1)
        .find(|family| model.contains(*family))
    {
        return family;
    }
    let o_series = model.strip_prefix('o').is_some_and(|rest| {
        rest.starts_with(|c: char| c.is_ascii_digit())
            && rest.chars().take_while(char::is_ascii_digit).count() <= 1
    });
    if o_series || model.contains("openai") {
        "gpt"
    } else {
        "other"
    }
}

/// What a detail's value may be.
#[derive(Clone, Copy, Debug)]
pub enum Values {
    /// One of these words.
    One(&'static [&'static str]),
    /// A duration in milliseconds, kept as its [`TIME`] bucket.
    Time,
}

/// One event the page may send, and the details it may carry.
#[derive(Debug)]
pub struct Spec {
    pub name: &'static str,
    pub details: &'static [(&'static str, Values)],
}

const GPU: (&str, Values) = ("gpu", Values::One(&["webgpu", "webgl"]));
const TOOK: (&str, Values) = ("time", Values::Time);
const ZONE: (&str, Values) = ("zone", Values::One(&ZONES));
/// A Pylon's kind, as the Pylon Bay draws it; never its key or name.
const KIND: (&str, Values) = (
    "kind",
    Values::One(&["mac", "gpu", "cpu", "secure", "head", "part"]),
);
/// The terminal's model choice.
const MODEL: (&str, Values) = ("model", Values::One(&["deepseek", "flint", "any"]));

/// Every per-visit event, in the order of a visit.
pub const EVENTS: &[Spec] = &[
    // Arrival.
    Spec {
        name: "page_view",
        details: &[],
    },
    Spec {
        name: "verse_load_start",
        details: &[GPU],
    },
    Spec {
        name: "verse_first_frame",
        details: &[GPU, TOOK],
    },
    Spec {
        name: "verse_ready",
        details: &[GPU, TOOK],
    },
    Spec {
        name: "verse_load_failed",
        details: &[
            GPU,
            (
                "reason",
                Values::One(&["download", "start", "gpu", "stopped", "other"]),
            ),
        ],
    },
    // Joining.
    Spec {
        name: "overview_watched",
        details: &[TOOK],
    },
    Spec {
        name: "joined",
        details: &[("by", Values::One(&["click", "keys", "touch"]))],
    },
    Spec {
        name: "first_person",
        details: &[],
    },
    Spec {
        name: "zoom_used",
        details: &[("by", Values::One(&["wheel", "keys", "pinch"]))],
    },
    // Getting around.
    Spec {
        name: "terminal_reached",
        details: &[TOOK],
    },
    Spec {
        name: "pylon_bay_entered",
        details: &[],
    },
    Spec {
        name: "pylon_hovered",
        details: &[KIND],
    },
    Spec {
        name: "pylon_opened",
        details: &[KIND, ("from", Values::One(&["world", "list"]))],
    },
    // A portal to a zone, and whether this browser could open it (`no`:
    // the page said it's only in the desktop app).
    Spec {
        name: "portal_entered",
        details: &[ZONE, ("opened", Values::One(&["yes", "no"]))],
    },
    Spec {
        name: "zone_load_start",
        details: &[ZONE],
    },
    Spec {
        name: "zone_ready",
        details: &[ZONE, TOOK],
    },
    Spec {
        name: "zone_failed",
        details: &[ZONE, ("reason", Values::One(&["load", "return"]))],
    },
    Spec {
        name: "plaza_returned",
        details: &[("from", Values::One(&ZONES))],
    },
    // The help desk near the spawn (#12443): reached it, opened its chat,
    // and sent a message there. Never what anyone typed.
    Spec {
        name: "help_desk_reached",
        details: &[],
    },
    Spec {
        name: "help_desk_opened",
        details: &[],
    },
    Spec {
        name: "help_desk_message",
        details: &[("nth", Values::One(&["first", "later"]))],
    },
    // The Grid Foreman (#12446): came within talking range, opened its chat,
    // and asked it something. Never what anyone typed.
    Spec {
        name: "foreman_reached",
        details: &[],
    },
    Spec {
        name: "foreman_opened",
        details: &[],
    },
    Spec {
        name: "foreman_message",
        details: &[("nth", Values::One(&["first", "later"]))],
    },
    // Chat at the terminal.
    Spec {
        name: "terminal_opened",
        details: &[],
    },
    Spec {
        name: "terminal_failed",
        details: &[],
    },
    Spec {
        name: "model_chosen",
        details: &[MODEL],
    },
    Spec {
        name: "message_sent",
        details: &[MODEL, ("nth", Values::One(&["first", "later"]))],
    },
    Spec {
        name: "answered",
        details: &[
            ("source", Values::One(&["pylon", "group", "hosted"])),
            ("family", Values::One(&FAMILIES)),
            TOOK,
        ],
    },
    Spec {
        name: "answer_failed",
        details: &[(
            "why",
            Values::One(&[
                "busy",
                "unavailable",
                "refused",
                "sign_in",
                "error",
                "lost",
                "stopped",
            ]),
        )],
    },
    // Game Lab in the Grid (#12433): opened (or asked to sign in), a
    // change sent and checked, a version saved, played, compared, closed.
    Spec {
        name: "game_lab_opened",
        details: &[("result", Values::One(&["open", "sign_in", "failed"]))],
    },
    Spec {
        name: "game_lab_check",
        details: &[],
    },
    Spec {
        name: "game_lab_version_saved",
        details: &[],
    },
    Spec {
        name: "game_lab_played",
        details: &[("from", Values::One(&["chat", "card"]))],
    },
    Spec {
        name: "game_lab_compared",
        details: &[],
    },
    Spec {
        name: "game_lab_closed",
        details: &[TOOK],
    },
    // Account.
    Spec {
        name: "login_clicked",
        details: &[],
    },
    Spec {
        name: "signup_clicked",
        details: &[],
    },
    Spec {
        name: "signin_completed",
        details: &[],
    },
    // Engagement: the time the page was in view so far.
    Spec {
        name: "heartbeat",
        details: &[TOOK],
    },
    Spec {
        name: "tab_hidden",
        details: &[TOOK],
    },
    Spec {
        name: "tab_left",
        details: &[TOOK],
    },
];

/// The spec of event `name`.
pub fn spec(name: &str) -> Option<&'static Spec> {
    EVENTS.iter().find(|spec| spec.name == name)
}

/// The name a per-visit event is counted under in the hourly totals,
/// which last longer than the rows: `grid.joined`.
pub fn counted_name(event: &str) -> String {
    format!("grid.{event}")
}

/// Whether `name` is a [`counted_name`].
pub fn is_counted_name(name: &str) -> bool {
    name.strip_prefix("grid.")
        .is_some_and(|e| spec(e).is_some())
}

/// Whether `id` is a visit id the page makes: 16 lowercase hex digits.
pub fn valid_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Why an event was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    UnknownEvent,
    UnknownDetail,
    BadValue,
}

/// The stored form of one event's details: each key and value checked
/// against `name`'s [`Spec`], durations turned into buckets.
pub fn admit(
    name: &str,
    detail: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<(&'static str, BTreeMap<String, String>), Refused> {
    let spec = spec(name).ok_or(Refused::UnknownEvent)?;
    let mut out = BTreeMap::new();
    for (key, value) in detail.into_iter().flatten() {
        let (key, allowed) = spec
            .details
            .iter()
            .find(|(k, _)| k == key)
            .ok_or(Refused::UnknownDetail)?;
        let kept = match (allowed, value) {
            (Values::One(words), serde_json::Value::String(word)) => words
                .iter()
                .find(|w| **w == word.as_str())
                .copied()
                .ok_or(Refused::BadValue)?,
            (Values::Time, serde_json::Value::Number(ms)) => ms
                .as_f64()
                .filter(|ms| ms.is_finite() && *ms >= 0.0)
                .map(|ms| time_bucket(ms as u64))
                .ok_or(Refused::BadValue)?,
            (Values::Time, serde_json::Value::String(word)) => TIME
                .iter()
                .find(|w| **w == word.as_str())
                .copied()
                .ok_or(Refused::BadValue)?,
            _ => return Err(Refused::BadValue),
        };
        out.insert((*key).to_owned(), kept.to_owned());
    }
    Ok((spec.name, out))
}

/// One stored event of one visit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisitRow {
    /// The page's random id for this visit.
    pub visit: String,
    /// When the server took it, in whole minutes since 1970 UTC.
    pub minute: u64,
    /// Its number in the visit, from the page: the order of events.
    pub n: u32,
    /// The route template of the page that sent it.
    pub route: String,
    /// An [`EVENTS`] name.
    pub event: String,
    /// Its checked details.
    #[serde(default)]
    pub detail: BTreeMap<String, String>,
}

impl VisitRow {
    /// Whether a row read back is one this module could have written.
    pub fn valid(&self) -> bool {
        valid_id(&self.visit)
            && self.n <= MOST_NUMBER
            && ROUTES.contains(&self.route.as_str())
            && spec(&self.event).is_some_and(|spec| {
                self.detail.iter().all(|(key, value)| {
                    bounded(value)
                        && spec.details.iter().any(|(k, allowed)| {
                            k == key
                                && match allowed {
                                    Values::One(words) => words.contains(&value.as_str()),
                                    Values::Time => TIME.contains(&value.as_str()),
                                }
                        })
                })
            })
    }

    pub fn day(&self) -> u64 {
        self.minute / (24 * 60)
    }

    /// One detail's value, or `""`.
    pub fn get(&self, key: &str) -> &str {
        self.detail.get(key).map_or("", String::as_str)
    }
}

/// A beacon's events, admitted: the visit, the route, and each event's
/// number, name and details. Events that don't pass [`admit`] are left
/// out and counted in `refused`.
#[derive(Debug, Default)]
pub struct Admitted {
    pub visit: String,
    pub route: &'static str,
    pub events: Vec<(u32, &'static str, BTreeMap<String, String>)>,
    pub refused: usize,
}

/// Reads a beacon body: `{"v": ID, "r": ROUTE, "e": [[NAME, N, {DETAIL}],
/// ...]}`. `None` when the body itself isn't one.
pub fn parse(body: &[u8]) -> Option<Admitted> {
    if body.len() > MOST_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let visit = value.get("v")?.as_str()?;
    if !valid_id(visit) {
        return None;
    }
    let route = value.get("r")?.as_str()?;
    let route = *ROUTES.iter().find(|r| **r == route)?;
    let events = value.get("e")?.as_array()?;
    if events.is_empty() || events.len() > MOST_PER_BEACON {
        return None;
    }
    let mut out = Admitted {
        visit: visit.to_owned(),
        route,
        ..Admitted::default()
    };
    for event in events {
        let parts = event.as_array().map(Vec::as_slice).unwrap_or_default();
        let admitted = match parts {
            [name, n, rest @ ..] if rest.len() <= 1 => {
                let n = n
                    .as_u64()
                    .filter(|n| *n <= u64::from(MOST_NUMBER))
                    .map(|n| n as u32);
                let detail = match rest.first() {
                    None | Some(serde_json::Value::Null) => Ok(None),
                    Some(serde_json::Value::Object(map)) => Ok(Some(map)),
                    Some(_) => Err(Refused::BadValue),
                };
                match (name.as_str(), n, detail) {
                    (Some(name), Some(n), Ok(detail)) => admit(name, detail).ok().map(|a| (n, a)),
                    _ => None,
                }
            }
            _ => None,
        };
        match admitted {
            Some((n, (name, detail))) => out.events.push((n, name, detail)),
            None => out.refused += 1,
        }
    }
    Some(out)
}

/// The rows held in memory until they're written.
#[derive(Default)]
pub struct Buffer {
    rows: Vec<VisitRow>,
    /// Events kept per visit and hour, for [`MOST_PER_VISIT`].
    per_visit: HashMap<(String, u64), u32>,
}

impl Buffer {
    /// Keeps `admitted`'s events at `minute`. Returns how many were kept.
    pub fn add(&mut self, admitted: &Admitted, minute: u64) -> usize {
        let hour = minute / 60;
        let mut kept = 0;
        for (n, name, detail) in &admitted.events {
            if self.rows.len() >= MOST_HELD {
                break;
            }
            let count = self
                .per_visit
                .entry((admitted.visit.clone(), hour))
                .or_default();
            if *count >= MOST_PER_VISIT {
                break;
            }
            *count += 1;
            self.rows.push(VisitRow {
                visit: admitted.visit.clone(),
                minute,
                n: *n,
                route: admitted.route.to_owned(),
                event: (*name).to_owned(),
                detail: detail.clone(),
            });
            kept += 1;
        }
        kept
    }

    pub fn rows(&self) -> &[VisitRow] {
        &self.rows
    }

    /// Forgets the rows of `hours` that ended before `now_hour - 1`.
    pub fn forget(&mut self, hours: &[u64], now_hour: u64) {
        let gone = |hour: u64| hours.contains(&hour) && hour + 1 < now_hour;
        self.rows.retain(|row| !gone(row.minute / 60));
        self.per_visit.retain(|(_, hour), _| !gone(*hour));
    }
}

#[derive(Serialize, Deserialize)]
struct Batch {
    schema: String,
    rows: Vec<VisitRow>,
}

pub fn encode(rows: Vec<VisitRow>) -> Vec<u8> {
    serde_json::to_vec(&Batch {
        schema: SCHEMA.to_owned(),
        rows,
    })
    .unwrap_or_default()
}

/// Rows from a stored batch; anything malformed is dropped.
pub fn decode(bytes: &[u8]) -> Vec<VisitRow> {
    serde_json::from_slice::<Batch>(bytes)
        .ok()
        .filter(|batch| batch.schema == SCHEMA)
        .map(|batch| batch.rows.into_iter().filter(VisitRow::valid).collect())
        .unwrap_or_default()
}

/// Where one instance's rows for one hour are written.
pub fn object_name(hour: u64, instance: &str) -> String {
    format!(
        "{PREFIX}{}/{:02}/{instance}.json",
        date(hour / 24),
        hour % 24
    )
}

fn day_prefix(day: u64) -> String {
    format!("{PREFIX}{}/", date(day))
}

/// Writes each hour's rows in `rows` for `instance`, and returns the hours
/// written.
pub async fn write(
    store: &Store,
    rows: &[VisitRow],
    instance: &str,
) -> Result<Vec<u64>, store::Error> {
    let mut by_hour: BTreeMap<u64, Vec<VisitRow>> = BTreeMap::new();
    for row in rows {
        by_hour
            .entry(row.minute / 60)
            .or_default()
            .push(row.clone());
    }
    let mut written = Vec::new();
    for (hour, rows) in by_hour {
        store
            .put(&object_name(hour, instance), encode(rows))
            .await?;
        written.push(hour);
    }
    Ok(written)
}

/// Every stored row of `day`.
pub async fn read_day(store: &Store, day: u64) -> Result<Vec<VisitRow>, store::Error> {
    use futures_util::stream::{self, StreamExt};
    let names = store.list(&day_prefix(day)).await?;
    let batches: Vec<_> = stream::iter(names)
        .map(|name| async move { store.get(&name).await })
        .buffer_unordered(16)
        .collect()
        .await;
    let mut rows = Vec::new();
    for batch in batches {
        if let Some(bytes) = batch? {
            rows.extend(decode(&bytes));
        }
    }
    Ok(rows)
}

/// The first day still kept on `today`.
pub fn first_kept(today: u64) -> u64 {
    today.saturating_sub(KEEP_DAYS - 1)
}

/// Whether a row is still kept on `today`.
pub fn kept(row: &VisitRow, today: u64) -> bool {
    row.day() >= first_kept(today) && row.day() <= today
}

/// Deletes the rows of the days that passed [`KEEP_DAYS`]: the 30 days
/// before the first day kept, so a missed run is caught up. Returns how
/// many objects were deleted.
pub async fn prune(store: &Store, today: u64) -> Result<usize, store::Error> {
    let first = first_kept(today);
    let mut deleted = 0;
    for day in first.saturating_sub(KEEP_DAYS)..first {
        for name in store.list(&day_prefix(day)).await? {
            store.delete(&name).await?;
            deleted += 1;
        }
    }
    Ok(deleted)
}
