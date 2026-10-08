//! The Pylon Field's relay source: verified beacons and receipts, projected
//! into NIP-PYLON `pylon` world states. Verse polls a [`RelayField`] and
//! draws what it returns; nothing else feeds a pylon's glow.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nostr::domain::Event;
use nostr::pylon::{
    self, AggregateInputs, BEACON_KIND, BEACON_MARKER, BeaconBook, Freshness, MAX_FUTURE_SKEW_SECS,
    MAX_WINDOW_SECS, Outcome, POOL_KIND, POOL_MARKER, PoolAggregate, PoolPolicy, PylonState,
    RECEIPT_KIND, RECEIPT_MARKER, freshness, parse_aggregate, parse_beacon, parse_receipt, project,
    verify_aggregate,
};
use serde_json::json;

use crate::client::beacons;
use crate::identity::Identity;
use crate::now;
use crate::relay::{self, LIFETIME};

/// How far back receipts count toward a pylon's job total.
pub const RECEIPT_WINDOW_SECS: u64 = 24 * 3_600;

/// A relay-backed source of pylon states for one pool.
pub struct RelayField {
    pub relay: String,
    /// Only pylons whose beacon asks to join this pool; `None` keeps all.
    pub pool: Option<String>,
    reader: Identity,
}

impl RelayField {
    /// A field over `relay`, reading as `reader` (any key; the relay needs
    /// one for NIP-42).
    #[must_use]
    pub fn new(relay: &str, pool: Option<&str>, reader: Identity) -> Self {
        Self {
            relay: relay.into(),
            pool: pool.map(str::to_string),
            reader,
        }
    }

    /// Fetch, verify, and project. Stale beacons come back as `unknown`.
    /// At most 256 pylons, newest first, as the world projection allows.
    ///
    /// # Errors
    ///
    /// When the relay cannot be read.
    pub async fn poll(&self) -> Result<Vec<PylonState>, String> {
        let mut conn = relay::connect(&self.relay, &self.reader, LIFETIME).await?;
        let book = beacons(&mut conn, None).await?;
        let receipts = relay::query(
            &mut conn,
            "receipts",
            &[json!({
                "kinds": [RECEIPT_KIND],
                "#t": [RECEIPT_MARKER],
                "since": now().saturating_sub(RECEIPT_WINDOW_SECS),
                "limit": 5_000,
            })],
        )
        .await?;
        let _ = conn.close().await;

        let mut seen = BTreeSet::new();
        let mut jobs: BTreeMap<String, u64> = BTreeMap::new();
        for event in &receipts {
            let Ok(receipt) = parse_receipt(event, None) else {
                continue;
            };
            if receipt.outcome == Outcome::Accepted
                && !book.self_dealt(&receipt.buyer, &receipt.address())
                && seen.insert((receipt.buyer.clone(), receipt.request.clone()))
            {
                *jobs.entry(receipt.address()).or_default() += 1;
            }
        }
        let at = now();
        let mut states: Vec<(u64, PylonState)> = book
            .iter()
            .filter(|(_, b)| self.pool.as_ref().is_none_or(|p| b.pools.contains(p)))
            .map(|(_, b)| {
                (
                    b.observed_at,
                    project(b, at, jobs.get(&b.address()).copied().unwrap_or(0)),
                )
            })
            .collect();
        states.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(states.into_iter().take(256).map(|(_, s)| s).collect())
    }
}

/// Whether a pylon state should glow: fresh and serving at least one job.
#[must_use]
pub fn glowing(state: &PylonState) -> bool {
    state.status == "online" && state.busy > 0
}

/// Re-exported so Verse needs only this crate for the state type.
pub use pylon::PylonState as State;

/// The most pylons a live field holds.
pub const MAX_LIVE_PYLONS: usize = 256;
/// The most receipts a live field remembers for its job counts.
pub const MAX_LIVE_RECEIPTS: usize = 16_384;
/// The most raw receipts it keeps to recompute an aggregate.
pub const MAX_LIVE_RECENT: usize = 4_096;
/// The window the pool's rate counts receipts in when no aggregate is
/// valid, s.
pub const RATE_SECS: u64 = 60;
/// How long a reconnect waits after a failed connection at most, s.
pub const MAX_BACKOFF_SECS: u64 = 60;

/// One verified pylon as a live field shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pylon {
    /// The world state at the reader's clock: `unknown` when stale.
    pub state: PylonState,
    /// `class.memory_gb` from the beacon.
    pub memory_gb: u32,
    /// When the pylon sampled itself, Unix seconds.
    pub observed_at: u64,
}

/// A receipt as a live field remembers it.
#[derive(Debug, Clone)]
struct Counted {
    finished_at: u64,
    address: String,
    accepted: bool,
}

/// The newest pool aggregate and whether it recomputed.
#[derive(Debug, Clone)]
struct Held {
    event: Event,
    body: PoolAggregate,
    /// `None` until a recomputation was tried.
    verified: Option<bool>,
}

/// What a live subscription has seen, verified and bounded: the newest
/// beacon per pylon, receipts for the last 24 hours, and the pool's newest
/// aggregate. Every record passes `nostr::pylon` validation before it
/// counts; anything else is dropped.
#[derive(Debug, Clone, Default)]
pub struct Live {
    pool: Option<String>,
    book: BeaconBook,
    pylons: usize,
    receipts: BTreeMap<(String, String), Counted>,
    /// Raw receipts of the last hour, for recomputing an aggregate.
    recent: Vec<Event>,
    aggregate: Option<Held>,
    /// The newest receipt's `created_at`, where a reconnect resumes.
    newest_receipt: u64,
    /// Whether the subscription has caught up with the relay.
    pub synced: bool,
    /// The last connection error, until the next catch-up.
    pub error: Option<String>,
}

impl Live {
    /// An empty field for `pool`, or for every pool.
    #[must_use]
    pub fn new(pool: Option<&str>) -> Self {
        Self {
            pool: pool.map(str::to_string),
            ..Self::default()
        }
    }

    /// Offers one event the relay sent. Returns whether it counted: a
    /// beacon, receipt, or aggregate that verifies, isn't from the future,
    /// and fits the bounds.
    pub fn offer(&mut self, event: Event, now: u64) -> bool {
        match event.kind {
            BEACON_KIND => {
                let Ok(beacon) = parse_beacon(&event) else {
                    return false;
                };
                if freshness(&beacon, now) == Freshness::Future {
                    return false;
                }
                let known = self.book.get(&beacon.address()).is_some();
                if !known && self.pylons >= MAX_LIVE_PYLONS {
                    return false;
                }
                let taken = self.book.offer(event, beacon);
                if taken && !known {
                    self.pylons += 1;
                }
                taken
            }
            RECEIPT_KIND => {
                let Ok(receipt) = parse_receipt(&event, None) else {
                    return false;
                };
                if receipt.finished_at > now + MAX_FUTURE_SKEW_SECS
                    || receipt.finished_at + RECEIPT_WINDOW_SECS < now
                {
                    return false;
                }
                let key = (receipt.buyer.clone(), receipt.request.clone());
                if self.receipts.contains_key(&key) || self.receipts.len() >= MAX_LIVE_RECEIPTS {
                    return false;
                }
                self.newest_receipt = self.newest_receipt.max(event.created_at);
                self.receipts.insert(
                    key,
                    Counted {
                        finished_at: receipt.finished_at,
                        address: receipt.address(),
                        accepted: receipt.outcome == Outcome::Accepted,
                    },
                );
                if receipt.finished_at + MAX_WINDOW_SECS >= now
                    && self.recent.len() < MAX_LIVE_RECENT
                {
                    self.recent.push(event);
                }
                true
            }
            POOL_KIND => {
                let Ok(body) = parse_aggregate(&event) else {
                    return false;
                };
                if self.pool.as_ref().is_some_and(|p| p != &body.pool)
                    || body.generated_at > now + MAX_FUTURE_SKEW_SECS
                    || self
                        .aggregate
                        .as_ref()
                        .is_some_and(|held| held.body.generated_at >= body.generated_at)
                {
                    return false;
                }
                self.aggregate = Some(Held {
                    event,
                    body,
                    verified: None,
                });
                true
            }
            _ => false,
        }
    }

    /// Drops what has aged out at `now`, and recomputes a new aggregate
    /// once from the beacons and receipts held, under the open policy with
    /// the aggregate's slice count. An aggregate counted under another
    /// policy, or from events no longer on the relay, stays unverified.
    pub fn settle(&mut self, now: u64) {
        self.receipts
            .retain(|_, r| r.finished_at + RECEIPT_WINDOW_SECS >= now);
        self.recent.retain(|event| {
            parse_receipt(event, None).is_ok_and(|r| r.finished_at + MAX_WINDOW_SECS >= now)
        });
        if let Some(held) = self.aggregate.as_mut()
            && held.verified.is_none()
        {
            let beacons: Vec<Event> = self.book.iter().map(|(event, _)| event.clone()).collect();
            let slices = u32::try_from(held.body.rate.len()).unwrap_or(u32::MAX);
            let policy = PoolPolicy::open(&held.body.pool, slices);
            let inputs = AggregateInputs {
                beacons: &beacons,
                receipts: &self.recent,
            };
            held.verified = Some(verify_aggregate(&held.event, &policy, &inputs).is_ok());
        }
    }

    /// Each pylon in the pool at `now`, newest sample first.
    #[must_use]
    pub fn pylons(&self, now: u64) -> Vec<Pylon> {
        let mut jobs: BTreeMap<&str, u64> = BTreeMap::new();
        for ((buyer, _), counted) in &self.receipts {
            if counted.accepted
                && counted.finished_at + RECEIPT_WINDOW_SECS >= now
                && !self.book.self_dealt(buyer, &counted.address)
            {
                *jobs.entry(counted.address.as_str()).or_default() += 1;
            }
        }
        let mut out: Vec<Pylon> = self
            .book
            .iter()
            .filter(|(_, b)| self.pool.as_ref().is_none_or(|p| b.pools.contains(p)))
            .map(|(_, b)| {
                let address = b.address();
                Pylon {
                    state: project(b, now, jobs.get(address.as_str()).copied().unwrap_or(0)),
                    memory_gb: b.class.memory_gb,
                    observed_at: b.observed_at,
                }
            })
            .collect();
        out.sort_by(|a, b| b.observed_at.cmp(&a.observed_at));
        out
    }

    /// The pool's newest aggregate while it is valid at `now`, and whether
    /// it recomputed.
    #[must_use]
    pub fn aggregate(&self, now: u64) -> Option<(&PoolAggregate, bool)> {
        self.aggregate
            .as_ref()
            .filter(|held| held.body.valid_until >= now)
            .map(|held| (&held.body, held.verified == Some(true)))
    }

    /// Accepted jobs a minute across the pool at `now`: from a valid
    /// aggregate's newest rate slice when there is one, else from the
    /// receipts that finished in the last [`RATE_SECS`].
    #[must_use]
    pub fn rate(&self, now: u64) -> u32 {
        if let Some((aggregate, _)) = self.aggregate(now) {
            let slices = aggregate.rate.len().max(1) as u64;
            let slice = (aggregate.window.to - aggregate.window.from) / slices;
            let last = aggregate.rate.last().copied().unwrap_or(0);
            return u32::try_from((last * 60).div_ceil(slice.max(1))).unwrap_or(u32::MAX);
        }
        let recent = self
            .receipts
            .iter()
            .filter(|((buyer, _), r)| {
                r.accepted
                    && r.finished_at + RATE_SECS >= now
                    && r.finished_at <= now
                    && !self.book.self_dealt(buyer, &r.address)
            })
            .count();
        u32::try_from(recent).unwrap_or(u32::MAX)
    }

    /// Where a reconnect resumes the receipt query, at `now`.
    fn receipts_since(&self, now: u64) -> u64 {
        let floor = now.saturating_sub(RECEIPT_WINDOW_SECS);
        if self.newest_receipt == 0 {
            floor
        } else {
            self.newest_receipt.saturating_sub(120).max(floor)
        }
    }
}

impl RelayField {
    /// Subscribes to the pool's beacons, receipts, and aggregates and feeds
    /// `live` until `stop` is set, reconnecting when a connection ends: at
    /// once after a connection that caught up, else after a backoff that
    /// doubles from 2 seconds to [`MAX_BACKOFF_SECS`]. Checks `stop` at
    /// least twice a second.
    pub async fn watch(&self, live: &Mutex<Live>, stop: &AtomicBool) {
        let mut backoff = 2;
        while !stop.load(Ordering::Relaxed) {
            match self.session(live, stop).await {
                Ok(()) => backoff = 2,
                Err(error) => {
                    {
                        let mut l = lock(live);
                        l.synced = false;
                        l.error = Some(error);
                    }
                    for _ in 0..backoff * 2 {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
                }
            }
        }
    }

    /// One connection's subscription: `Ok` when it caught up before it
    /// ended, or when `stop` was set.
    async fn session(&self, live: &Mutex<Live>, stop: &AtomicBool) -> Result<(), String> {
        let mut conn = relay::connect(&self.relay, &self.reader, LIFETIME).await?;
        let since = lock(live).receipts_since(now());
        let mut aggregates = json!({
            "kinds": [POOL_KIND],
            "#t": [POOL_MARKER],
            "limit": 8,
        });
        if let Some(pool) = &self.pool {
            aggregates["#d"] = json!([pool]);
        }
        conn.send(json!([
            "REQ",
            "field",
            {"kinds": [BEACON_KIND], "#t": [BEACON_MARKER], "limit": 500},
            {"kinds": [RECEIPT_KIND], "#t": [RECEIPT_MARKER], "since": since, "limit": 2_000},
            aggregates,
        ]))
        .await?;
        let mut synced = false;
        loop {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let frame = match tokio::time::timeout(Duration::from_millis(500), conn.next()).await {
                Err(_) => continue,
                // The connection's lifetime or frame budget ran out.
                Ok(Err(error)) => return if synced { Ok(()) } else { Err(error) },
                Ok(Ok(value)) => relay::Frame::parse(value),
            };
            match frame {
                relay::Frame::Event { sub, event } if sub == "field" => {
                    let mut l = lock(live);
                    let at = now();
                    if l.offer(*event, at) && synced {
                        l.settle(at);
                    }
                }
                relay::Frame::Eose(sub) if sub == "field" => {
                    synced = true;
                    let mut l = lock(live);
                    l.synced = true;
                    l.error = None;
                    l.settle(now());
                }
                relay::Frame::Closed { sub, message } if sub == "field" => {
                    return Err(format!("relay closed the field: {message}"));
                }
                _ => {}
            }
        }
    }
}

/// `live`, whether or not a holder panicked.
fn lock(live: &Mutex<Live>) -> std::sync::MutexGuard<'_, Live> {
    live.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
