use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::GatewayLimits;

const WINDOW: Duration = Duration::from_secs(60);
const OBSERVER_WINDOW: Duration = Duration::from_secs(1);
const POSE_WINDOW: Duration = Duration::from_secs(1);
/// How long a key that published a pose-lane event counts as present in its
/// world: three of NIP-MV's five-second idle keepalives.
pub const WORLD_PRESENCE: Duration = Duration::from_secs(15);
/// Most worlds the pose lane tracks at once.
const MAX_WORLDS: usize = 4_096;
const CLEANUP_INTERVAL: Duration = Duration::from_secs(10);
const MAX_RATE_KEYS: usize = 100_000;

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<State>>,
    limits: GatewayLimits,
}

struct State {
    connections: HashMap<IpAddr, usize>,
    event_ip: HashMap<IpAddr, Counter>,
    event_pubkey: HashMap<String, Counter>,
    gift_wrap_recipient: HashMap<String, Counter>,
    observer_ip: HashMap<IpAddr, Counter>,
    observer_agent: HashMap<String, Counter>,
    pose_ip: HashMap<IpAddr, Counter>,
    pose_pubkey: HashMap<String, Counter>,
    worlds: HashMap<String, World>,
    req_ip: HashMap<IpAddr, Counter>,
    media_ip: HashMap<IpAddr, Counter>,
    media_pubkey: HashMap<String, Counter>,
    last_cleanup: Instant,
}

/// One world's pose lane: who published lately, and this second's frames.
struct World {
    present: HashMap<String, Instant>,
    frames: Counter,
}

/// Why the pose lane refused an event for its world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldRefusal {
    /// The world already holds its population cap of other keys.
    Full,
    /// The world spent this second's pose-lane budget.
    Budget,
}

struct Counter {
    started: Instant,
    count: u32,
}

pub struct ConnectionPermit {
    limiter: RateLimiter,
    ip: IpAddr,
}

impl RateLimiter {
    pub fn new(limits: GatewayLimits) -> Self {
        Self {
            inner: Arc::new(Mutex::new(State {
                connections: HashMap::new(),
                event_ip: HashMap::new(),
                event_pubkey: HashMap::new(),
                gift_wrap_recipient: HashMap::new(),
                observer_ip: HashMap::new(),
                observer_agent: HashMap::new(),
                pose_ip: HashMap::new(),
                pose_pubkey: HashMap::new(),
                worlds: HashMap::new(),
                req_ip: HashMap::new(),
                media_ip: HashMap::new(),
                media_pubkey: HashMap::new(),
                last_cleanup: Instant::now(),
            })),
            limits,
        }
    }

    pub fn connect(&self, ip: IpAddr) -> Option<ConnectionPermit> {
        let mut state = self.inner.lock().ok()?;
        let count = state.connections.entry(ip).or_default();
        if *count >= self.limits.max_connections_per_ip {
            return None;
        }
        *count += 1;
        Some(ConnectionPermit {
            limiter: self.clone(),
            ip,
        })
    }

    pub fn event_from_ip(&self, ip: IpAddr) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_ip(&mut state.event_ip, ip, self.limits.events_per_minute_ip)
    }

    pub fn event_from_pubkey(&self, pubkey: &str) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_string(
            &mut state.event_pubkey,
            pubkey,
            self.limits.events_per_minute_pubkey,
        )
    }

    pub fn gift_wrap_for_recipient(&self, recipient: &str) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_string(
            &mut state.gift_wrap_recipient,
            recipient,
            self.limits.gift_wraps_per_minute_recipient,
        )
    }

    pub fn req_from_ip(&self, ip: IpAddr) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_ip(&mut state.req_ip, ip, self.limits.req_per_minute_ip)
    }

    pub fn observer_from_ip(&self, ip: IpAddr) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_ip_for(
            &mut state.observer_ip,
            ip,
            self.limits.observer_events_per_second_ip,
            OBSERVER_WINDOW,
        )
    }

    pub fn observer_from_agent(&self, agent_pubkey: &str) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_string_for(
            &mut state.observer_agent,
            agent_pubkey,
            self.limits.observer_events_per_second_agent,
            OBSERVER_WINDOW,
        )
    }

    /// Whether `ip` may publish one more pose-lane event this second.
    pub fn pose_from_ip(&self, ip: IpAddr) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_ip_for(
            &mut state.pose_ip,
            ip,
            self.limits.pose_events_per_second_ip,
            POSE_WINDOW,
        )
    }

    /// Whether `pubkey` may publish one more pose-lane event this second.
    pub fn pose_from_pubkey(&self, pubkey: &str) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_string_for(
            &mut state.pose_pubkey,
            pubkey,
            self.limits.pose_events_per_second_pubkey,
            POSE_WINDOW,
        )
    }

    /// Whether `pubkey` may publish one more pose-lane event in `world`: a
    /// key not already present is refused while the world holds
    /// `world_population_cap` others, and the world as a whole may carry
    /// `world_pose_events_per_second`. A cap or budget of zero is no limit.
    pub fn pose_in_world(&self, world: &str, pubkey: &str) -> Result<(), WorldRefusal> {
        self.pose_in_world_at(world, pubkey, Instant::now())
    }

    fn pose_in_world_at(
        &self,
        world: &str,
        pubkey: &str,
        now: Instant,
    ) -> Result<(), WorldRefusal> {
        let Ok(mut state) = self.inner.lock() else {
            return Err(WorldRefusal::Budget);
        };
        state.cleanup();
        if !state.worlds.contains_key(world) && state.worlds.len() >= MAX_WORLDS {
            return Err(WorldRefusal::Full);
        }
        let entry = state
            .worlds
            .entry(world.to_owned())
            .or_insert_with(|| World {
                present: HashMap::new(),
                frames: Counter {
                    started: now,
                    count: 0,
                },
            });
        entry
            .present
            .retain(|_, seen| now.saturating_duration_since(*seen) < WORLD_PRESENCE);
        let cap = self.limits.world_population_cap as usize;
        if cap > 0 && !entry.present.contains_key(pubkey) && entry.present.len() >= cap {
            return Err(WorldRefusal::Full);
        }
        let budget = self.limits.world_pose_events_per_second;
        if now.saturating_duration_since(entry.frames.started) >= POSE_WINDOW {
            entry.frames = Counter {
                started: now,
                count: 0,
            };
        }
        if budget > 0 && entry.frames.count >= budget {
            return Err(WorldRefusal::Budget);
        }
        entry.frames.count += 1;
        entry.present.insert(pubkey.to_owned(), now);
        Ok(())
    }

    /// Keys present in `world` now: those that published a pose-lane event
    /// within [`WORLD_PRESENCE`].
    #[cfg(test)]
    pub fn world_population(&self, world: &str) -> usize {
        let Ok(state) = self.inner.lock() else {
            return 0;
        };
        let now = Instant::now();
        state.worlds.get(world).map_or(0, |entry| {
            entry
                .present
                .values()
                .filter(|seen| now.saturating_duration_since(**seen) < WORLD_PRESENCE)
                .count()
        })
    }

    pub fn media_from_ip(&self, ip: IpAddr) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_ip(&mut state.media_ip, ip, self.limits.media_per_minute_ip)
    }

    pub fn media_from_pubkey(&self, pubkey: &str) -> bool {
        let Ok(mut state) = self.inner.lock() else {
            return false;
        };
        state.cleanup();
        allow_string(
            &mut state.media_pubkey,
            pubkey,
            self.limits.media_per_minute_pubkey,
        )
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let Ok(mut state) = self.limiter.inner.lock() else {
            return;
        };
        if let Some(count) = state.connections.get_mut(&self.ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.connections.remove(&self.ip);
            }
        }
    }
}

fn allow_ip(map: &mut HashMap<IpAddr, Counter>, key: IpAddr, limit: u32) -> bool {
    allow_ip_for(map, key, limit, WINDOW)
}

fn allow_ip_for(
    map: &mut HashMap<IpAddr, Counter>,
    key: IpAddr,
    limit: u32,
    window: Duration,
) -> bool {
    if !map.contains_key(&key) && map.len() >= MAX_RATE_KEYS {
        return false;
    }
    allow_counter(map.entry(key).or_insert_with(new_counter), limit, window)
}

fn allow_string(map: &mut HashMap<String, Counter>, key: &str, limit: u32) -> bool {
    allow_string_for(map, key, limit, WINDOW)
}

fn allow_string_for(
    map: &mut HashMap<String, Counter>,
    key: &str,
    limit: u32,
    window: Duration,
) -> bool {
    if !map.contains_key(key) && map.len() >= MAX_RATE_KEYS {
        return false;
    }
    allow_counter(
        map.entry(key.to_owned()).or_insert_with(new_counter),
        limit,
        window,
    )
}

impl State {
    fn cleanup(&mut self) {
        if self.last_cleanup.elapsed() < CLEANUP_INTERVAL {
            return;
        }
        let now = Instant::now();
        self.event_ip
            .retain(|_, counter| now.duration_since(counter.started) < WINDOW);
        self.event_pubkey
            .retain(|_, counter| now.duration_since(counter.started) < WINDOW);
        self.gift_wrap_recipient
            .retain(|_, counter| now.duration_since(counter.started) < WINDOW);
        self.observer_ip
            .retain(|_, counter| now.duration_since(counter.started) < OBSERVER_WINDOW);
        self.observer_agent
            .retain(|_, counter| now.duration_since(counter.started) < OBSERVER_WINDOW);
        self.pose_ip
            .retain(|_, counter| now.duration_since(counter.started) < POSE_WINDOW);
        self.pose_pubkey
            .retain(|_, counter| now.duration_since(counter.started) < POSE_WINDOW);
        self.worlds.retain(|_, world| {
            world
                .present
                .retain(|_, seen| now.saturating_duration_since(*seen) < WORLD_PRESENCE);
            !world.present.is_empty()
        });
        self.req_ip
            .retain(|_, counter| now.duration_since(counter.started) < WINDOW);
        self.media_ip
            .retain(|_, counter| now.duration_since(counter.started) < WINDOW);
        self.media_pubkey
            .retain(|_, counter| now.duration_since(counter.started) < WINDOW);
        self.last_cleanup = now;
    }
}

fn new_counter() -> Counter {
    Counter {
        started: Instant::now(),
        count: 0,
    }
}

fn allow_counter(counter: &mut Counter, limit: u32, window: Duration) -> bool {
    if counter.started.elapsed() >= window {
        *counter = new_counter();
    }
    if counter.count >= limit {
        return false;
    }
    counter.count += 1;
    true
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use crate::gateway::GatewayLimits;

    use std::time::{Duration, Instant};

    use super::{RateLimiter, WORLD_PRESENCE, WorldRefusal};

    #[test]
    fn rate_and_connection_limits_fail_closed_and_permits_release() {
        let limits = GatewayLimits {
            max_connections_per_ip: 1,
            events_per_minute_ip: 1,
            events_per_minute_pubkey: 1,
            gift_wraps_per_minute_recipient: 1,
            observer_events_per_second_ip: 1,
            observer_events_per_second_agent: 1,
            req_per_minute_ip: 1,
            media_per_minute_ip: 1,
            media_per_minute_pubkey: 1,
            ..GatewayLimits::default()
        };
        let limiter = RateLimiter::new(limits);
        let ip = "127.0.0.1".parse::<IpAddr>().unwrap();
        let permit = limiter.connect(ip).unwrap();
        assert!(limiter.connect(ip).is_none());
        drop(permit);
        assert!(limiter.connect(ip).is_some());
        assert!(limiter.event_from_ip(ip));
        assert!(!limiter.event_from_ip(ip));
        assert!(limiter.event_from_pubkey("a"));
        assert!(!limiter.event_from_pubkey("a"));
        assert!(limiter.gift_wrap_for_recipient("recipient"));
        assert!(!limiter.gift_wrap_for_recipient("recipient"));
        assert!(limiter.observer_from_ip(ip));
        assert!(!limiter.observer_from_ip(ip));
        assert!(limiter.observer_from_agent("agent"));
        assert!(!limiter.observer_from_agent("agent"));
        assert!(limiter.req_from_ip(ip));
        assert!(!limiter.req_from_ip(ip));
        assert!(limiter.media_from_ip(ip));
        assert!(!limiter.media_from_ip(ip));
        assert!(limiter.media_from_pubkey("a"));
        assert!(!limiter.media_from_pubkey("a"));
    }

    #[test]
    fn the_pose_lane_is_counted_apart_from_the_minute_budget() {
        let limits = GatewayLimits {
            events_per_minute_ip: 1,
            events_per_minute_pubkey: 1,
            pose_events_per_second_ip: 1,
            pose_events_per_second_pubkey: 2,
            ..GatewayLimits::default()
        };
        let limiter = RateLimiter::new(limits);
        let ip = "127.0.0.3".parse::<IpAddr>().unwrap();
        assert!(limiter.event_from_ip(ip));
        assert!(!limiter.event_from_ip(ip));
        assert!(limiter.event_from_pubkey("walker"));
        assert!(!limiter.event_from_pubkey("walker"));
        // A spent minute budget leaves the pose lane open, and the other way
        // round.
        assert!(limiter.pose_from_pubkey("walker"));
        assert!(limiter.pose_from_pubkey("walker"));
        assert!(!limiter.pose_from_pubkey("walker"));
        assert!(limiter.pose_from_pubkey("other"));
        assert!(limiter.pose_from_ip(ip));
        assert!(!limiter.pose_from_ip(ip));
    }

    #[test]
    fn gift_wrap_rate_dimensions_are_bounded_without_inferring_an_inner_sender() {
        let limits = GatewayLimits {
            events_per_minute_ip: 1,
            events_per_minute_pubkey: 1,
            gift_wraps_per_minute_recipient: 1,
            ..GatewayLimits::default()
        };
        let discovery = RateLimiter::new(limits.clone());
        let ip = "127.0.0.2".parse::<IpAddr>().unwrap();
        assert!(discovery.event_from_ip(ip));
        assert!(!discovery.event_from_ip(ip));
        assert!(discovery.event_from_pubkey("discovery-author"));
        assert!(!discovery.event_from_pubkey("discovery-author"));

        let gift_wrap = RateLimiter::new(limits);
        assert!(gift_wrap.event_from_ip(ip));
        assert!(gift_wrap.event_from_pubkey("outer-wrapper"));
        assert!(gift_wrap.gift_wrap_for_recipient("recipient"));
        assert!(!gift_wrap.gift_wrap_for_recipient("recipient"));
    }

    #[test]
    fn a_full_world_refuses_a_new_key_until_a_present_one_goes_quiet() {
        let limits = GatewayLimits {
            world_population_cap: 2,
            world_pose_events_per_second: 0,
            ..GatewayLimits::default()
        };
        let limiter = RateLimiter::new(limits);
        let start = Instant::now();
        assert_eq!(limiter.pose_in_world_at("verse-bare", "a", start), Ok(()));
        assert_eq!(limiter.pose_in_world_at("verse-bare", "b", start), Ok(()));
        assert_eq!(
            limiter.pose_in_world_at("verse-bare", "c", start),
            Err(WorldRefusal::Full)
        );
        // Present keys keep publishing, and another world has its own cap.
        assert_eq!(limiter.pose_in_world_at("verse-bare", "a", start), Ok(()));
        assert_eq!(limiter.pose_in_world_at("other", "c", start), Ok(()));
        // Once `b` has been quiet for the presence window, `c` takes its
        // place while `a` keeps its own.
        let later = start + WORLD_PRESENCE - Duration::from_secs(1);
        assert_eq!(limiter.pose_in_world_at("verse-bare", "a", later), Ok(()));
        let after = start + WORLD_PRESENCE;
        assert_eq!(limiter.pose_in_world_at("verse-bare", "c", after), Ok(()));
        assert_eq!(
            limiter.pose_in_world_at("verse-bare", "b", after),
            Err(WorldRefusal::Full)
        );
    }

    #[test]
    fn a_world_spends_its_frame_budget_each_second() {
        let limits = GatewayLimits {
            world_population_cap: 0,
            world_pose_events_per_second: 3,
            ..GatewayLimits::default()
        };
        let limiter = RateLimiter::new(limits);
        let start = Instant::now();
        for key in ["a", "b", "c"] {
            assert_eq!(limiter.pose_in_world_at("w", key, start), Ok(()));
        }
        assert_eq!(
            limiter.pose_in_world_at("w", "a", start),
            Err(WorldRefusal::Budget)
        );
        assert_eq!(limiter.pose_in_world_at("elsewhere", "a", start), Ok(()));
        let next = start + Duration::from_secs(1);
        assert_eq!(limiter.pose_in_world_at("w", "a", next), Ok(()));
    }
}
