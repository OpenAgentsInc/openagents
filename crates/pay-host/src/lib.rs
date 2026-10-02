//! Public payment flow projection. Private source identifiers never enter the wire schema.
mod ingest;
mod usage;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
    routing::get,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

/// Exact sats on the wire, stored internally as integer millisatoshis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sats(u64);

impl Sats {
    pub const fn from_msat(msat: u64) -> Self {
        Self(msat)
    }

    pub const fn msat(self) -> u64 {
        self.0
    }

    fn checked_add(self, other: Self) -> Result<Self, Error> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| "Public amount overflow".into())
    }

    fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl Serialize for Sats {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = if self.0.is_multiple_of(1000) {
            (self.0 / 1000).to_string()
        } else {
            format!("{}.{:03}", self.0 / 1000, self.0 % 1000)
                .trim_end_matches('0')
                .to_owned()
        };
        let number: serde_json::Number = text.parse().map_err(serde::ser::Error::custom)?;
        number.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Sats {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        exact_msat(&number.to_string())
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom("Expected nonnegative exact millisatoshis"))
    }
}

fn exact_msat(text: &str) -> Option<u64> {
    if text.starts_with('-') {
        return None;
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0);
    }
    let scale = exponent
        .checked_add(3)?
        .checked_sub(fraction.len() as i64)?;
    if scale >= 0 {
        let scale = u32::try_from(scale).ok()?;
        if digits.len().checked_add(scale as usize)? > 20 {
            return None;
        }
        digits
            .parse::<u64>()
            .ok()?
            .checked_mul(10u64.checked_pow(scale)?)
    } else {
        let places = usize::try_from(scale.checked_neg()?).ok()?;
        let keep = digits.len().checked_sub(places)?;
        if !digits[keep..].bytes().all(|b| b == b'0') {
            return None;
        }
        digits[..keep].parse().ok()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Call,
    Payment,
    Share,
    Payout,
    Bonus,
    Run,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Plugin,
    HostedResource,
    Coder,
    Route,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rail {
    Lightning,
    Balance,
    Spark,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Author,
    Resource,
    Openagents,
    Bonus,
    LspFee,
    Provider,
}

/// The `openagents.flow-event.v1` wire object. Optional fields are absent, not null.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowEvent {
    pub v: u8,
    pub seq: u64,
    pub at: i64,
    #[serde(rename = "type")]
    pub kind: EventType,
    pub resource: Resource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    pub node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_sats: Option<Sats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rail: Option<Rail>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub split: BTreeMap<Role, Sats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
}

/// Trusted ingestion boundary for ledger changes, usage records, and run cost records.
/// `source` is the stable private record key; replaying it writes nothing.
/// Amounts must be exact sats; producers must not round msat values.
pub struct SourceRecord {
    pub source: String,
    pub at: i64,
    pub kind: EventType,
    pub resource: Resource,
    pub plugin: Option<String>,
    pub node: String,
    pub amount_sats: Option<Sats>,
    pub rail: Option<Rail>,
    pub split: BTreeMap<Role, Sats>,
    pub author_identity: Option<String>,
    /// Set only from verified plugin publication metadata, not payment metadata.
    pub published_author_npub: Option<String>,
    pub payer_identity: Option<String>,
}

pub struct Store {
    db: Connection,
    salt: [u8; 32],
}
pub type SharedStore = Arc<Mutex<Store>>;
type Error = Box<dyn std::error::Error + Send + Sync>;

impl Store {
    pub fn open(path: impl AsRef<std::path::Path>, salt: [u8; 32]) -> Result<Self, Error> {
        Self::from_connection(Connection::open(path)?, salt)
    }
    pub fn from_connection(db: Connection, salt: [u8; 32]) -> Result<Self, Error> {
        db.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS flow_event(seq INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT UNIQUE NOT NULL, event TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS flow_plugin(plugin TEXT PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS flow_identity(id INTEGER PRIMARY KEY CHECK(id=1), salt_digest TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS flow_state(id INTEGER PRIMARY KEY CHECK(id=1), reconciliation TEXT NOT NULL);
            INSERT OR IGNORE INTO flow_state VALUES(1, 'unknown');")?;
        let salt_digest = format!("{:x}", Sha256::digest(salt));
        db.execute(
            "INSERT OR IGNORE INTO flow_identity VALUES(1,?)",
            [&salt_digest],
        )?;
        let existing: String = db.query_row(
            "SELECT salt_digest FROM flow_identity WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if existing != salt_digest {
            return Err("Flow salt does not match the existing projection".into());
        }
        Ok(Self { db, salt })
    }
    fn alias(&self, prefix: &str, identity: &str, day: Option<i64>) -> String {
        let mut hash = Sha256::new();
        hash.update(self.salt);
        hash.update(prefix.as_bytes());
        if let Some(day) = day {
            hash.update(day.to_be_bytes());
        }
        hash.update(identity.as_bytes());
        let bytes = hash.finalize();
        format!(
            "{prefix}-{}",
            bytes[..8]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
    }
    pub fn record(&mut self, record: SourceRecord) -> Result<FlowEvent, Error> {
        if let Some(json) = self
            .db
            .query_row(
                "SELECT event FROM flow_event WHERE source=?1",
                [&record.source],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok(serde_json::from_str(&json)?);
        }
        if record.kind == EventType::Call
            && (record.amount_sats.is_some() || !record.split.is_empty())
        {
            return Err("Call events must not carry an amount".into());
        }
        if record.at < 0
            || record.plugin.as_deref().is_some_and(|s| !safe_id(s))
            || !(committed_nodes().contains(&record.node)
                || record
                    .plugin
                    .as_ref()
                    .is_some_and(|plugin| record.node == format!("plugin:{plugin}")))
        {
            return Err("Invalid public route identifier or timestamp".into());
        }
        if let Some(npub) = &record.published_author_npub {
            if record.plugin.is_none() || nostr::nip19::decode_npub(npub).is_err() {
                return Err("Invalid published author npub".into());
            }
        }
        let author = record.published_author_npub.or_else(|| {
            record
                .author_identity
                .as_deref()
                .map(|id| self.alias("author", id, None))
        });
        let payer = record
            .payer_identity
            .as_deref()
            .map(|id| self.alias("caller", id, Some(record.at / 86_400_000)));
        let mut event = FlowEvent {
            v: 1,
            seq: 0,
            at: record.at,
            kind: record.kind,
            resource: record.resource,
            plugin: record.plugin,
            node: record.node,
            amount_sats: record.amount_sats,
            rail: record.rail,
            split: record.split,
            author,
            payer,
        };
        let tx = self.db.transaction()?;
        tx.execute(
            "INSERT INTO flow_event(source,event) VALUES(?1,'')",
            [&record.source],
        )?;
        event.seq = tx.last_insert_rowid() as u64;
        tx.execute(
            "UPDATE flow_event SET event=?1 WHERE seq=?2",
            params![serde_json::to_string(&event)?, event.seq],
        )?;
        if let Some(plugin) = &event.plugin {
            tx.execute("INSERT OR IGNORE INTO flow_plugin VALUES(?)", [plugin])?;
        }
        tx.commit()?;
        Ok(event)
    }
    /// Read one bounded page after the cursor. Continue from the last sequence.
    pub fn since(&self, seq: u64) -> Result<Vec<FlowEvent>, Error> {
        let seq = i64::try_from(seq).map_err(|_| "Invalid flow cursor")?;
        let mut stmt = self
            .db
            .prepare("SELECT event FROM flow_event WHERE seq>?1 ORDER BY seq LIMIT 500")?;
        let rows = stmt.query_map([seq], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    fn recent(&self) -> Result<Vec<FlowEvent>, Error> {
        let mut stmt = self.db.prepare("SELECT event FROM (SELECT seq,event FROM flow_event ORDER BY seq DESC LIMIT 500) ORDER BY seq")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    fn topology_map(&self) -> Result<openagents_chat_app::route_map::Map, Error> {
        use openagents_chat_app::route_map::{Edge, EdgeKind, Health, Kind, Map, Node};
        let mut map = Map::committed();
        let parent = map
            .nodes
            .iter()
            .position(|node| node.id == "coder")
            .ok_or("Committed route map has no plugin hub")?;
        let mut stmt = self
            .db
            .prepare("SELECT plugin FROM flow_plugin ORDER BY plugin")?;
        for plugin in stmt.query_map([], |row| row.get::<_, String>(0))? {
            let plugin = plugin?;
            if !safe_id(&plugin) {
                return Err("Invalid stored plugin identifier".into());
            }
            let id = format!("plugin:{plugin}");
            if map.nodes.iter().any(|node| node.id == id) {
                continue;
            }
            let index = map.nodes.len();
            map.nodes.push(Node {
                id,
                kind: Kind::Plugin,
                label: plugin,
                line: "A plugin with recorded payment flow.".into(),
                parent: Some(parent),
                depth: map.nodes[parent].depth + 1,
                weight: 0.3,
                health: Health::Unmeasured,
                stage: None,
                family: map.nodes[parent].family.clone(),
                gaps: Vec::new(),
                local: None,
                showcase: false,
            });
            map.edges.push(Edge {
                from: parent,
                to: index,
                kind: EdgeKind::Admits,
            });
        }
        Ok(map)
    }
    pub fn set_reconciliation(&self, state: Reconciliation) -> Result<(), Error> {
        self.db.execute(
            "UPDATE flow_state SET reconciliation=?1 WHERE id=1",
            [match state {
                Reconciliation::Unknown => "unknown",
                Reconciliation::Ok => "ok",
                Reconciliation::Drift => "drift",
            }],
        )?;
        Ok(())
    }
    pub fn stats(&self, now: i64) -> Result<Stats, Error> {
        let mut stats = Stats {
            totals: Totals::default(),
            per_plugin: BTreeMap::new(),
            per_author: BTreeMap::new(),
            series_24h: series(now, 24, 3_600_000),
            series_30d: series(now, 30, 86_400_000),
            reconciliation: self.db.query_row(
                "SELECT reconciliation FROM flow_state",
                [],
                |r| r.get(0),
            )?,
        };
        let mut stmt = self
            .db
            .prepare("SELECT event FROM flow_event ORDER BY seq")?;
        for json in stmt.query_map([], |row| row.get::<_, String>(0))? {
            let event: FlowEvent = serde_json::from_str(&json?)?;
            stats.totals.add(&event, false)?;
            let treasury_only =
                event.kind == EventType::Payout && recipient_amount(&event)? == Sats::default();
            if let Some(plugin) = &event.plugin
                && !treasury_only
            {
                stats
                    .per_plugin
                    .entry(plugin.clone())
                    .or_default()
                    .add(&event, true)?;
            }
            if let Some(author) = &event.author
                && !treasury_only
            {
                stats
                    .per_author
                    .entry(self.canonical_author(author, event.plugin.as_deref())?)
                    .or_default()
                    .add(&event, true)?;
            }
            for points in [&mut stats.series_24h, &mut stats.series_30d] {
                for point in points.iter_mut() {
                    if event.at >= point.at
                        && event.at < point.at + point.width_ms
                        && event.at <= now
                    {
                        point.totals.add(&event, false)?;
                    }
                }
            }
        }
        Ok(stats)
    }
}
fn safe_id(s: &str) -> bool {
    const PRIVATE_PREFIXES: &[&str] = &[
        "npub1",
        "nsec1",
        "nprofile1",
        "lnbc",
        "lntb",
        "lnbcrt",
        "lnurl",
        "spark1",
        "bc1",
        "tb1",
        "bcrt1",
    ];
    !s.is_empty()
        && s.len() <= 160
        && s.split('/').all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                && !PRIVATE_PREFIXES
                    .iter()
                    .any(|prefix| part.to_ascii_lowercase().starts_with(prefix))
        })
        && !s
            .as_bytes()
            .windows(64)
            .any(|w| w.iter().all(u8::is_ascii_hexdigit))
}
fn committed_nodes() -> &'static BTreeSet<String> {
    static NODES: OnceLock<BTreeSet<String>> = OnceLock::new();
    NODES.get_or_init(|| {
        openagents_chat_app::route_map::Map::committed()
            .nodes
            .into_iter()
            .map(|node| node.id)
            .collect()
    })
}
#[derive(Clone, Copy)]
pub enum Reconciliation {
    Unknown,
    Ok,
    Drift,
}
#[derive(Default, Serialize)]
pub struct Totals {
    pub received_sats: Sats,
    pub paid_out_sats: Sats,
    pub pending_accruals_sats: Sats,
    pub calls: u64,
    pub earnings_sats: Sats,
}
impl Totals {
    fn add(&mut self, e: &FlowEvent, breakdown: bool) -> Result<(), Error> {
        let amount = e.amount_sats.unwrap_or_default();
        match e.kind {
            EventType::Call => {
                self.calls = self.calls.checked_add(1).ok_or("Call count overflow")?
            }
            EventType::Payment => self.received_sats = self.received_sats.checked_add(amount)?,
            EventType::Share | EventType::Bonus => {
                self.earnings_sats = self.earnings_sats.checked_add(recipient_amount(e)?)?;
            }
            EventType::Payout => {
                let paid = if breakdown {
                    recipient_amount(e)?
                } else {
                    amount
                };
                self.paid_out_sats = self.paid_out_sats.checked_add(paid)?;
            }
            EventType::Run => {}
        }
        let accrued = if breakdown {
            self.earnings_sats
        } else {
            self.received_sats
        };
        self.pending_accruals_sats = accrued.saturating_sub(self.paid_out_sats);
        Ok(())
    }
}
fn recipient_amount(event: &FlowEvent) -> Result<Sats, Error> {
    event
        .split
        .iter()
        .filter(|(role, _)| matches!(role, Role::Author | Role::Resource | Role::Bonus))
        .try_fold(Sats::default(), |sum, (_, amount)| sum.checked_add(*amount))
}
#[derive(Serialize)]
pub struct SeriesPoint {
    pub at: i64,
    pub width_ms: i64,
    pub totals: Totals,
}
fn series(now: i64, count: i64, width: i64) -> Vec<SeriesPoint> {
    let end = now.div_euclid(width) * width;
    (0..count)
        .map(|i| SeriesPoint {
            at: end - (count - 1 - i) * width,
            width_ms: width,
            totals: Totals::default(),
        })
        .collect()
}
#[derive(Serialize)]
pub struct Stats {
    pub totals: Totals,
    pub per_plugin: BTreeMap<String, Totals>,
    pub per_author: BTreeMap<String, Totals>,
    pub series_24h: Vec<SeriesPoint>,
    pub series_30d: Vec<SeriesPoint>,
    pub reconciliation: String,
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn internal(_: impl std::fmt::Display) -> StatusCode {
    StatusCode::INTERNAL_SERVER_ERROR
}

pub fn router(store: SharedStore) -> Router {
    Router::new()
        .route("/flow/stream", get(stream))
        .route("/flow/snapshot", get(snapshot))
        .route("/stats", get(stats))
        .with_state(store)
}
async fn stats(State(store): State<SharedStore>) -> Result<Json<Stats>, StatusCode> {
    Ok(Json(
        store
            .lock()
            .map_err(internal)?
            .stats(now())
            .map_err(internal)?,
    ))
}
async fn snapshot(State(store): State<SharedStore>) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = store.lock().map_err(internal)?;
    let events = store.recent().map_err(internal)?;
    let map = store.topology_map().map_err(internal)?;
    let layout = openagents_chat_app::route_map::layout::Layout::of(&map);
    let topology:Vec<_>=map.nodes.iter().enumerate().map(|(i,n)| serde_json::json!({"id":n.id,"kind":n.kind,"parent":n.parent.map(|p| &map.nodes[p].id),"position":{"x":layout.positions[i].x,"y":layout.positions[i].y}})).collect();
    Ok(Json(
        serde_json::json!({"events":events,"totals":store.stats(now()).map_err(internal)?.totals,"topology":topology}),
    ))
}
async fn stream(
    State(store): State<SharedStore>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, std::io::Error>>>, StatusCode> {
    let cursor = match headers.get("last-event-id") {
        Some(id) => id
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|seq| *seq <= i64::MAX as u64)
            .ok_or(StatusCode::BAD_REQUEST)?,
        None => 0,
    };
    let events = futures_util::stream::unfold(
        (store, cursor, std::collections::VecDeque::new()),
        |(store, mut cursor, mut queue)| async move {
            loop {
                if let Some(event) = queue.pop_front() {
                    let event: FlowEvent = event;
                    cursor = event.seq;
                    let wire = Event::default()
                        .id(cursor.to_string())
                        .data(serde_json::to_string(&event).expect("Public event is serializable"));
                    return Some((Ok(wire), (store, cursor, queue)));
                }
                let result = store
                    .lock()
                    .map_err(|_| "Store lock failed".to_owned())
                    .and_then(|s| s.since(cursor).map_err(|e| e.to_string()));
                match result {
                    Ok(events) => queue.extend(events),
                    Err(_) => {
                        return Some((
                            Err(std::io::Error::other("Flow store unavailable")),
                            (store, cursor, queue),
                        ));
                    }
                }
                if queue.is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        },
    );
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

#[cfg(test)]
mod tests;
