//! Bounded transport admission and work budgets, shared across reconnects.
use super::super::wire::Body;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::IpAddr,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::Notify;

pub const CONNECTIONS: usize = 128;
pub const PRE_AUTH: usize = 32;
pub const PER_IP: usize = 8;
const PRINCIPALS: usize = 128;
const RETAIN: Duration = Duration::from_secs(120);

/// Aggregate counters contain neither principal keys nor network addresses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub capacity_refusals: u64,
    pub pre_auth_refusals: u64,
    pub ip_refusals: u64,
    pub handshake_rate_refusals: u64,
    pub principal_capacity_refusals: u64,
    pub principal_work_refusals: u64,
    pub aggregate_work_refusals: u64,
    pub projections: u64,
    pub commands: u64,
    pub pending_peak: usize,
    pub active_peak: usize,
    pub pending: usize,
    pub active: usize,
    pub principal_records: usize,
    pub completed: u64,
    pub handshake_timeouts: u64,
    pub handshake_refusals: u64,
    pub authentication_timeouts: u64,
    pub authentication_refusals: u64,
    pub grant_refusals: u64,
    pub retired_connections: u64,
    pub frame_failures: u64,
    pub read_timeouts: u64,
    pub write_failures: u64,
    pub request_rate_failures: u64,
    pub host_failures: u64,
    pub transport_closures: u64,
    pub cancelled_workers: u64,
}
struct Bucket {
    tokens: f64,
    capacity: f64,
    rate: f64,
    at: Instant,
}
impl Bucket {
    fn new(capacity: u32, rate: u32, now: Instant) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            rate: rate as f64,
            at: now,
        }
    }
    fn ready(&mut self, cost: u32, now: Instant) -> bool {
        let now = now.max(self.at);
        self.tokens = (self.tokens
            + now.saturating_duration_since(self.at).as_secs_f64() * self.rate)
            .min(self.capacity);
        self.at = now;
        self.tokens >= cost as f64
    }
    fn charge(&mut self, cost: u32) {
        self.tokens -= cost as f64;
    }
}
struct Principal {
    stop: Arc<Stop>,
    commands: Bucket,
    projections: Bucket,
    used: Instant,
}
#[derive(Default)]
struct Stop {
    retired: AtomicBool,
    wake: Notify,
}
struct Activity {
    authenticated: bool,
    requests: u64,
    received: u64,
    sent: u64,
    used: Instant,
    delivered: Option<(u64, Instant)>,
    refusals: u64,
}
struct State {
    serial: u64,
    clients: BTreeMap<u64, Activity>,
    stats: Stats,
    ips: BTreeMap<IpAddr, usize>,
    principals: BTreeMap<[u8; 32], Principal>,
    handshakes: Bucket,
    commands: Bucket,
    projections: Bucket,
}
#[derive(Clone)]
pub(in crate::service) struct Limits(Arc<Mutex<State>>);
impl Limits {
    /// The shared state. A panic elsewhere while it was held must not take
    /// every later connection (or a `Slot` drop) down with it, so a poisoned
    /// lock is recovered: every update here leaves the counters consistent.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn new() -> Self {
        let now = Instant::now();
        Self(Arc::new(Mutex::new(State {
            serial: 0,
            clients: BTreeMap::new(),
            stats: Stats::default(),
            ips: BTreeMap::new(),
            principals: BTreeMap::new(),
            handshakes: Bucket::new(64, 128, now),
            commands: Bucket::new(256, 16384, now),
            projections: Bucket::new(2048, 65536, now),
        })))
    }
    /// Before any handshake allocation, reserve bounded pending and IP capacity.
    pub fn open(&self, ip: IpAddr) -> Result<Slot, &'static str> {
        let ip = match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
            _ => ip,
        };
        let mut s = self.lock();
        if s.stats.active + s.stats.pending >= CONNECTIONS {
            s.stats.capacity_refusals += 1;
            return Err("Chamber connection budget exceeded");
        }
        if s.stats.pending >= PRE_AUTH {
            s.stats.pre_auth_refusals += 1;
            return Err("Chamber pre-authentication budget exceeded");
        }
        if s.ips.get(&ip).copied().unwrap_or(0) >= PER_IP {
            s.stats.ip_refusals += 1;
            return Err("Chamber pending IP budget exceeded");
        }
        if !s.handshakes.ready(1, Instant::now()) {
            s.stats.handshake_rate_refusals += 1;
            return Err("Chamber handshake rate exceeded");
        }
        if s.serial == u64::MAX {
            return Err("Chamber connection identity exhausted");
        }
        s.handshakes.charge(1);
        s.stats.pending += 1;
        s.stats.pending_peak = s.stats.pending_peak.max(s.stats.pending);
        *s.ips.entry(ip).or_default() += 1;
        s.serial = s
            .serial
            .checked_add(1)
            .ok_or("Chamber connection identity exhausted")?;
        let serial = s.serial;
        s.clients.insert(
            serial,
            Activity {
                authenticated: false,
                requests: 0,
                received: 0,
                sent: 0,
                used: Instant::now(),
                delivered: None,
                refusals: 0,
            },
        );
        Ok(Slot {
            serial,
            limits: self.clone(),
            ip,
            principal: None,
            stop: None,
        })
    }
    pub fn stats(&self) -> Stats {
        let s = self.lock();
        let mut out = s.stats.clone();
        out.principal_records = s.principals.len();
        out
    }
    pub fn clients(&self, tick: u64) -> Vec<super::super::operator::Client> {
        let s = self.lock();
        let now = Instant::now();
        s.clients
            .iter()
            .map(|(&connection, a)| super::super::operator::Client {
                connection,
                authenticated: a.authenticated,
                requests: a.requests,
                received_payload_bytes: a.received,
                sent_payload_bytes: a.sent,
                idle_ms: super::super::operator::millis(now.saturating_duration_since(a.used)),
                delivered_tick: a.delivered.map(|(tick, _)| tick),
                delivered_tick_lag: a
                    .delivered
                    .map(|(delivered, _)| tick.saturating_sub(delivered)),
                last_delivery_ms_ago: a.delivered.map(|(_, at)| {
                    super::super::operator::millis(now.saturating_duration_since(at))
                }),
                work_refusals: a.refusals,
            })
            .collect()
    }
    pub fn cancelled(&self) {
        self.lock().stats.cancelled_workers += 1;
    }
    pub fn finish(&self, result: &Result<(), String>) {
        let mut s = self.lock();
        let stats = &mut s.stats;
        stats.completed += 1;
        let Err(error) = result else {
            return;
        };
        if error.contains("handshake timed out") || error.contains("upgrade timed out") {
            stats.handshake_timeouts += 1;
        } else if error.contains("TLS handshake refused")
            || error.starts_with("Chamber channel refused")
        {
            stats.handshake_refusals += 1;
        } else if error == "Chamber authentication timed out" {
            stats.authentication_timeouts += 1;
        } else if error == "Chamber connection is not authenticated"
            || error.contains("principal budget")
        {
            stats.authentication_refusals += 1;
        } else if error.starts_with("Chamber grant refused")
            || error.contains("only its own device")
        {
            stats.grant_refusals += 1;
        } else if error == "Chamber connection was superseded" {
            stats.retired_connections += 1;
        } else if error.contains("byte budget")
            || error.contains("Malformed chamber request")
            || error.contains("Unsupported chamber version")
            || error.contains("payload incomplete")
        {
            stats.frame_failures += 1;
        } else if error == "Chamber read timed out" {
            stats.read_timeouts += 1;
        } else if error.contains("write") || error.contains("flush") {
            stats.write_failures += 1;
        } else if error.contains("request rate") {
            stats.request_rate_failures += 1;
        } else if error.contains("host stopped") {
            stats.host_failures += 1;
        } else {
            stats.transport_closures += 1;
        }
    }
}
pub(in crate::service) struct Slot {
    serial: u64,
    limits: Limits,
    ip: IpAddr,
    principal: Option<[u8; 32]>,
    stop: Option<Arc<Stop>>,
}
fn release_pending(s: &mut State, ip: IpAddr) {
    s.stats.pending = s.stats.pending.saturating_sub(1);
    if let Some(count) = s.ips.get_mut(&ip) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            s.ips.remove(&ip);
        }
    }
}
impl Slot {
    pub fn received(&self, bytes: usize) {
        let mut s = self.limits.lock();
        let Some(a) = s.clients.get_mut(&self.serial) else {
            return;
        };
        a.requests = a.requests.saturating_add(1);
        a.received = a.received.saturating_add(bytes as u64);
        a.used = Instant::now();
    }
    pub fn delivered(&self, bytes: usize, tick: Option<u64>) {
        let mut s = self.limits.lock();
        let Some(a) = s.clients.get_mut(&self.serial) else {
            return;
        };
        a.sent = a.sent.saturating_add(bytes as u64);
        a.used = Instant::now();
        if let Some(tick) = tick {
            a.delivered = Some((tick, a.used));
        }
    }
    /// Called only after signature verification and world-right admission.
    pub fn authenticate(&mut self, key: [u8; 32]) -> Result<(), String> {
        let mut s = self.limits.lock();
        let now = Instant::now();
        s.principals.retain(|_, p| {
            Arc::strong_count(&p.stop) > 1 || now.saturating_duration_since(p.used) < RETAIN
        });
        if !s.principals.contains_key(&key) && s.principals.len() >= PRINCIPALS {
            s.stats.principal_capacity_refusals += 1;
            return Err("Chamber principal budget exceeded".into());
        }
        let stop = Arc::new(Stop::default());
        if let Some(previous) = s.principals.get_mut(&key) {
            previous.stop.retired.store(true, Ordering::Release);
            previous.stop.wake.notify_one();
            previous.stop = stop.clone();
            previous.used = now;
        } else {
            s.principals.insert(
                key,
                Principal {
                    stop: stop.clone(),
                    commands: Bucket::new(120, 120, now),
                    projections: Bucket::new(512, 512, now),
                    used: now,
                },
            );
        }
        release_pending(&mut s, self.ip);
        s.stats.active += 1;
        s.stats.active_peak = s.stats.active_peak.max(s.stats.active);
        if let Some(a) = s.clients.get_mut(&self.serial) {
            a.authenticated = true;
        }
        self.principal = Some(key);
        self.stop = Some(stop);
        Ok(())
    }
    pub fn retired(&self) -> bool {
        self.stop
            .as_ref()
            .is_some_and(|s| s.retired.load(Ordering::Acquire))
    }
    pub async fn cancelled(&self) {
        match &self.stop {
            Some(stop) => {
                if stop.retired.load(Ordering::Acquire) {
                    return;
                }
                stop.wake.notified().await;
            }
            None => std::future::pending().await,
        }
    }
    /// One request per connection is in flight. Supersession closes the previous
    /// connection; shared principal buckets do not refill on reconnection.
    pub fn request(&self, body: &Body) -> bool {
        self.request_at(body, Instant::now())
    }
    fn request_at(&self, body: &Body, now: Instant) -> bool {
        let Some(key) = self.principal else {
            return matches!(body, Body::Authenticate { .. });
        };
        if self.retired() {
            return false;
        }
        let (projection, cost) = match body {
            Body::Snapshot {} => (true, 32),
            Body::Replicate { .. } => (true, 8),
            Body::Safety {} => (true, 32),
            Body::Inventory {} | Body::Account {} | Body::Services { .. } => (true, 8),
            Body::Events { .. } => (true, 4),
            _ => (false, 1),
        };
        let mut s = self.limits.lock();
        let Some(p) = s.principals.get_mut(&key) else {
            return false;
        };
        p.used = p.used.max(now);
        let local = if projection {
            &mut p.projections
        } else {
            &mut p.commands
        };
        if !local.ready(cost, now) {
            s.stats.principal_work_refusals += 1;
            if let Some(a) = s.clients.get_mut(&self.serial) {
                a.refusals += 1;
            }
            return false;
        }
        let global = if projection {
            &mut s.projections
        } else {
            &mut s.commands
        };
        if !global.ready(cost, now) {
            s.stats.aggregate_work_refusals += 1;
            if let Some(a) = s.clients.get_mut(&self.serial) {
                a.refusals += 1;
            }
            return false;
        }
        global.charge(cost);
        if let Some(p) = s.principals.get_mut(&key) {
            (if projection {
                &mut p.projections
            } else {
                &mut p.commands
            })
            .charge(cost);
        }
        if projection {
            s.stats.projections += 1;
        } else {
            s.stats.commands += 1;
        }
        true
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        let mut s = self.limits.lock();
        s.clients.remove(&self.serial);
        if self.principal.is_some() {
            s.stats.active = s.stats.active.saturating_sub(1);
        } else {
            release_pending(&mut s, self.ip);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ip(n: u8) -> IpAddr {
        [127, 0, 0, n].into()
    }
    #[test]
    fn pending_partitions_and_drop_leave_admitted_capacity() {
        let limits = Limits::new();
        let mut admitted = limits.open(ip(1)).unwrap();
        admitted.authenticate([1; 32]).unwrap();
        let slots: Vec<_> = (0..PRE_AUTH)
            .map(|n| limits.open(ip((n / PER_IP + 1) as u8)).unwrap())
            .collect();
        assert!(limits.open(ip(99)).is_err());
        assert!(admitted.request(&Body::Snapshot {}));
        assert_eq!(limits.stats().pre_auth_refusals, 1);
        drop(slots);
        drop(admitted);
        assert_eq!((limits.stats().pending, limits.stats().active), (0, 0));
        assert!(limits.lock().ips.is_empty());
    }
    #[test]
    fn mapped_ipv4_connections_share_the_pending_ip_limit() {
        let limits = Limits::new();
        let slots: Vec<_> = (0..PER_IP).map(|_| limits.open(ip(1)).unwrap()).collect();
        let mapped: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert!(limits.open(mapped).is_err());
        assert_eq!(limits.stats().ip_refusals, 1);
        drop(slots);
        assert!(limits.open(mapped).is_ok());
    }

    #[tokio::test]
    async fn reconnect_retires_old_connection_without_refilling_work() {
        let limits = Limits::new();
        let mut first = limits.open(ip(1)).unwrap();
        first.authenticate([1; 32]).unwrap();
        let now = Instant::now();
        for _ in 0..16 {
            assert!(first.request_at(&Body::Snapshot {}, now));
        }
        assert!(!first.request_at(&Body::Snapshot {}, now));
        let mut next = limits.open(ip(1)).unwrap();
        next.authenticate([1; 32]).unwrap();
        tokio::time::timeout(Duration::from_millis(50), first.cancelled())
            .await
            .unwrap();
        assert!(first.retired());
        assert!(!next.request_at(&Body::Snapshot {}, now));
        // Projection overload leaves the separate command budget available.
        assert!(next.request_at(
            &Body::Respawn {
                life: super::super::super::wire::Life {
                    instance: 1,
                    actor: 1,
                    generation: 1
                }
            },
            now
        ));
        assert!(next.request_at(&Body::Snapshot {}, now + Duration::from_secs(1)));
    }
    #[test]
    fn aggregate_projection_limit_preserves_other_principals_commands() {
        let limits = Limits::new();
        let now = Instant::now();
        let mut slots = vec![];
        for n in 1..=5 {
            let mut s = limits.open(ip(n)).unwrap();
            s.authenticate([n; 32]).unwrap();
            slots.push(s);
        }
        for slot in &slots[..4] {
            for _ in 0..16 {
                assert!(slot.request_at(&Body::Snapshot {}, now));
            }
        }
        assert!(!slots[4].request_at(&Body::Snapshot {}, now));
        assert_eq!(limits.stats().aggregate_work_refusals, 1);
        assert!(slots[4].request_at(
            &Body::Respawn {
                life: super::super::super::wire::Life {
                    instance: 1,
                    actor: 1,
                    generation: 1
                }
            },
            now
        ));
    }
    #[test]
    fn a_poisoned_lock_still_admits_and_drops_slots() {
        let limits = Limits::new();
        let mut held = limits.open(ip(1)).unwrap();
        held.authenticate([1; 32]).unwrap();
        let poisoner = limits.clone();
        let panicked = std::thread::spawn(move || {
            let _guard = poisoner.0.lock().unwrap();
            panic!("poison the admission lock");
        })
        .join();
        assert!(panicked.is_err());
        assert!(limits.0.is_poisoned());
        // New connections are still admitted, authenticated and served.
        let mut next = limits.open(ip(2)).unwrap();
        next.authenticate([2; 32]).unwrap();
        next.received(10);
        next.delivered(10, Some(1));
        assert!(next.request(&Body::Snapshot {}));
        assert_eq!(limits.stats().active, 2);
        // Dropping slots after poisoning neither panics nor underflows.
        drop(next);
        drop(held);
        let pending = limits.open(ip(3)).unwrap();
        drop(pending);
        let stats = limits.stats();
        assert_eq!((stats.pending, stats.active), (0, 0));
        assert!(limits.lock().ips.is_empty());
    }
}
