//! One player's multiplayer session over NIP-MV.
//!
//! - **Sign-up.** The profile's key is created on first launch, and a
//!   NIP-01 profile names it.
//! - **Spawn.** On launch the session asks the relay for this player's own
//!   avatar state. A returning player resumes where they left; a new one
//!   spawns at a random clear spot on the central plaza.
//! - **Streaming.** Pose frames for the avatar and the agent go out as
//!   ephemeral `23300` events at the selected desktop or mobile cadence.
//!   Durable `33301` states go out on join and periodically during movement.
//!   A queued leave state is best effort, not a delivery acknowledgment.
//! - **Scans.** When the agent looks around, the session queries entity
//!   states in the surrounding cells and reports what is near.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use glam::{Quat, Vec3};
use serde_json::json;

use nostr::domain::Tag;

use crate::agent::Agent;
use crate::chat::{self, Channel};
use crate::controller::{Footprint, PlayerController};
use crate::crowd::Crowd;
use crate::identity::{self, Identity};
use crate::mv::{self, EntityPose, Frame, Gesture, Received, State};
use crate::net::{In, Link, Out};

/// The world this client joins.
pub const WORLD: &str = "verse-plaza";
/// The relay used when none is named.
pub const DEFAULT_RELAY: &str = "ws://127.0.0.1:7447";
/// Radius of the spawn disc around the plaza center, in meters.
pub const SPAWN_RADIUS: f32 = 28.0;
/// How far the agent's scan reaches, in meters.
pub const SCAN_RADIUS: f32 = 80.0;
/// The NIP-29 rooms a Verse relay seeds (`scripts/verse-relay.sh`).
pub const ROOMS: [&str; 3] = ["lounge", "trading-post", "builders"];
/// How long an overhead bubble stays up.
pub const BUBBLE_TIME: Duration = Duration::from_secs(7);
const MAX_PEOPLE: usize = 1024;
const MAX_CHAT_IDS: usize = 4096;
const CHAT_SUB: &str = "chat-world";
const ROOM_SUB: &str = "chat-rooms";
const DM_SUB: &str = "chat-pm";
const LIVE_SUB: &str = "mv-live";
const STATE_SUB: &str = "mv-state";
const ME_SUB: &str = "mv-me";
const SCAN_WAIT: Duration = Duration::from_millis(1200);
/// Agents closer than this greet each other, in meters.
pub const GREET_RADIUS: f32 = 7.0;
/// An agent greets the same player's agent at most this often.
pub const GREET_COOLDOWN: Duration = Duration::from_secs(45);
/// How long a received greeting waits to be returned.
const INVITE_TTL: Duration = Duration::from_secs(10);

/// Connection status for the window title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Trying to reach the relay.
    Connecting,
    /// Connected.
    Online,
    /// Lost the relay; retrying.
    Offline,
}

/// Explicit publishing cadence. Rendering remains independent of relay traffic.
#[derive(Clone, Copy, Debug)]
pub struct PublishIntervals {
    pub moving: Duration,
    pub idle: Duration,
    pub state: Duration,
}
impl PublishIntervals {
    /// Original moving cadence; occasional idle keepalive follows NIP-MV.
    #[must_use]
    pub fn desktop() -> Self {
        Self {
            moving: Duration::from_millis(100),
            idle: Duration::from_secs(5),
            state: Duration::from_secs(3),
        }
    }
    /// Conservative mobile cadence for a relay with a 60-event/minute default.
    #[must_use]
    pub fn mobile() -> Self {
        Self {
            moving: Duration::from_secs(3),
            idle: Duration::from_secs(10),
            state: Duration::from_secs(30),
        }
    }
    fn validate(self) -> Result<(), String> {
        if self.moving < Duration::from_millis(100)
            || self.moving > Duration::from_secs(60)
            || self.idle < self.moving
            || self.idle > Duration::from_secs(60)
            || self.state < Duration::from_secs(1)
            || self.state > Duration::from_secs(300)
        {
            return Err("invalid presence publishing intervals".into());
        }
        Ok(())
    }
}
struct PendingSpawn {
    deadline: Instant,
    found: Option<State>,
    finished: bool,
}

struct Scan {
    sub: String,
    started: Instant,
    from: Vec3,
    done: bool,
}

/// A running session.
pub struct Session {
    link: Link,
    id: Identity,
    /// Remote players.
    pub crowd: Crowd,
    /// Connection status.
    pub status: Status,
    session: String,
    seq: u64,
    last_frame: Option<Instant>,
    last_state: Option<(Instant, Vec3, f32)>,
    throttled_until: Option<Instant>,
    scan: Option<Scan>,
    scans: u64,
    last_player: Option<Vec3>,
    /// When this agent last greeted each pubkey's agent.
    greeted: HashMap<String, Instant>,
    /// Pubkeys whose agents greeted this one, and when.
    invited: Vec<(String, Instant)>,
    greets_received: u64,
    /// Both chat windows.
    pub log: chat::Log,
    /// Lines floating over speakers' heads.
    pub bubbles: Vec<Bubble>,
    /// Display names by pubkey.
    names: HashMap<String, String>,
    asked_names: HashSet<String>,
    want_names: Vec<String>,
    last_name_ask: Option<Instant>,
    limits: chat::Limits,
    muted: HashSet<String>,
    /// Room display names by id, from NIP-29 metadata.
    pub room_names: HashMap<String, String>,
    joined: u64,
    auth_id: Option<String>,
    seen_chat: HashSet<String>,
    chat_order: VecDeque<String>,
    online: HashSet<String>,
    started: Instant,
    my_pos: Vec3,
    /// The last player to send this one a private message.
    pub last_pm_from: Option<String>,
    intervals: PublishIntervals,
    pending_spawn: Option<PendingSpawn>,
    room_authority: Option<String>,
}

/// A line floating over a speaker.
#[derive(Clone, Debug, PartialEq)]
pub struct Bubble {
    /// Speaker.
    pub pubkey: String,
    /// Text.
    pub text: String,
    /// When it disappears.
    pub until: Instant,
}

/// Starting place for the local player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    /// Feet position.
    pub pos: Vec3,
    /// Facing.
    pub yaw: f32,
    /// True when resumed from the relay rather than picked at random.
    pub resumed: bool,
}

impl Session {
    /// Loads or creates `profile` and connects to `relay`.
    ///
    /// # Errors
    ///
    /// Returns a message when the identity cannot be loaded or created.
    pub fn start(profile: &str, relay: &str) -> Result<Self, String> {
        Self::start_in(&identity::home(), profile, relay)
    }

    /// As [`Session::start`], with profile keys kept in `dir`.
    ///
    /// # Errors
    ///
    /// Returns a message when the identity cannot be loaded or created.
    pub fn start_in(dir: &std::path::Path, profile: &str, relay: &str) -> Result<Self, String> {
        let id = identity::load_or_create(dir, profile)?;
        Self::start_with_identity(id, relay)
    }

    /// Connect with a platform-supplied identity. This performs no filesystem
    /// identity discovery and does not wait for the network or spawn recovery.
    pub fn start_with_identity(id: Identity, relay: &str) -> Result<Self, String> {
        Self::with_link(id, Link::start(relay))
    }
    fn with_link(id: Identity, link: Link) -> Result<Self, String> {
        let me = id.signer.pubkey().to_owned();
        link.send(Out::Subscribe {
            id: LIVE_SUB.into(),
            filters: vec![json!({"kinds": [mv::FRAME_KIND, mv::GESTURE_KIND], "#w": [WORLD]})],
            live: true,
        });
        link.send(Out::Subscribe {
            id: STATE_SUB.into(),
            filters: vec![json!({"kinds": [mv::STATE_KIND], "#w": [WORLD], "limit": 500})],
            live: true,
        });
        link.send(Out::Subscribe {
            id: CHAT_SUB.into(),
            filters: vec![json!({"kinds": [mv::CHAT_KIND], "#w": [WORLD], "limit": 100})],
            live: true,
        });
        link.send(Out::Subscribe {
            id: ROOM_SUB.into(),
            filters: vec![
                json!({"kinds": [mv::CHAT_KIND], "#h": ROOMS, "limit": 60}),
                json!({"kinds": [39_000], "#d": ROOMS}),
            ],
            live: true,
        });
        if id.created {
            link.send(Out::Publish(mv::profile_event(
                &id.signer,
                &id.profile,
                unix_now(),
            )));
        }
        Ok(Self {
            link,
            crowd: Crowd::new(&me),
            id,
            status: Status::Connecting,
            session: identity::random_hex(4),
            seq: 0,
            last_frame: None,
            last_state: None,
            throttled_until: None,
            scan: None,
            scans: 0,
            last_player: None,
            greeted: HashMap::new(),
            invited: Vec::new(),
            greets_received: 0,
            log: chat::Log::default(),
            bubbles: Vec::new(),
            names: HashMap::new(),
            asked_names: HashSet::new(),
            want_names: Vec::new(),
            last_name_ask: None,
            limits: chat::Limits::new(Instant::now()),
            muted: HashSet::new(),
            room_names: HashMap::new(),
            joined: unix_now(),
            auth_id: None,
            seen_chat: HashSet::new(),
            chat_order: VecDeque::new(),
            online: HashSet::new(),
            started: Instant::now(),
            my_pos: Vec3::ZERO,
            last_pm_from: None,
            intervals: PublishIntervals::desktop(),
            pending_spawn: None,
            room_authority: None,
        })
    }

    /// The profile name.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.id.profile
    }

    /// The relay URL.
    #[must_use]
    pub fn relay(&self) -> &str {
        &self.link.url
    }

    /// This player's public key, hex.
    #[must_use]
    pub fn pubkey(&self) -> &str {
        self.id.signer.pubkey()
    }

    /// A copy of this player's signer, for another relay reader to answer
    /// NIP-42 challenges with.
    #[must_use]
    pub fn signer(&self) -> nostr::domain::RelaySigner {
        self.id.signer.clone()
    }

    /// Set a validated cadence without changing renderer frequency or relay policy.
    pub fn set_publish_intervals(&mut self, intervals: PublishIntervals) -> Result<(), String> {
        intervals.validate()?;
        self.intervals = intervals;
        Ok(())
    }
    /// Pin the relay signer before accepting NIP-29 room metadata.
    pub fn set_room_authority(&mut self, pubkey: &str) -> Result<(), String> {
        use std::str::FromStr;
        secp256k1::XOnlyPublicKey::from_str(pubkey).map_err(|_| "invalid room authority")?;
        self.room_authority = Some(pubkey.into());
        Ok(())
    }
    /// Begin bounded spawn recovery. Call poll_spawn on later frames. The
    /// deadline includes connection setup; a timeout falls back to a new spawn.
    pub fn begin_spawn(&mut self, wait: Duration) {
        self.pending_spawn = Some(PendingSpawn {
            deadline: Instant::now() + wait.min(Duration::from_secs(10)),
            found: None,
            finished: false,
        });
        self.link.send(Out::Subscribe {
            id: ME_SUB.into(),
            filters: vec![json!({"kinds":[mv::STATE_KIND],"authors":[self.pubkey()],"#d":[mv::state_address(WORLD,"avatar")],"limit":1})],
            live: false,
        });
    }
    /// Return a verified own-player spawn once recovery finishes, without waiting.
    pub fn poll_spawn(&mut self, blockers: &[Footprint], bound: f32) -> Option<Spawn> {
        let now = Instant::now();
        for message in self.link.drain() {
            self.handle(message, now);
        }
        if !self
            .pending_spawn
            .as_ref()
            .is_some_and(|p| p.finished || now >= p.deadline)
        {
            return None;
        }
        let pending = self.pending_spawn.take()?;
        self.link.send(Out::Close(ME_SUB.into()));
        if let Some(state) = pending.found {
            let pos = Vec3::from(state.p);
            if pos.x.abs() < bound && pos.z.abs() < bound && is_clear(pos, blockers) {
                let (axis, angle) = Quat::from_array(state.q).normalize().to_axis_angle();
                let yaw = if axis.y < 0.0 { -angle } else { angle };
                return Some(Spawn {
                    pos: Vec3::new(pos.x, 0.0, pos.z),
                    yaw: crate::controller::wrap(yaw),
                    resumed: true,
                });
            }
        }
        Some(Spawn {
            pos: random_spawn(blockers),
            yaw: 0.0,
            resumed: false,
        })
    }
    /// Desktop compatibility wrapper. Native frame loops use begin_spawn/poll_spawn.
    pub fn spawn(&mut self, blockers: &[Footprint], bound: f32, wait: Duration) -> Spawn {
        self.begin_spawn(wait);
        loop {
            if let Some(spawn) = self.poll_spawn(blockers, bound) {
                return spawn;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Drains the relay and publishes what is due, once per game frame.
    pub fn tick(&mut self, now: Instant, player: &PlayerController, agent: &Agent) {
        for message in self.link.drain() {
            self.handle(message, now);
        }
        self.crowd.prune(now);
        self.my_pos = player.pos;
        self.bubbles.retain(|b| b.until > now);
        self.notice_logins(now);
        self.ask_names(now);

        let moving = self
            .last_player
            .is_some_and(|last| last.distance(player.pos) > 0.005)
            || player.speed > 0.05;
        self.last_player = Some(player.pos);
        if self.status != Status::Online || self.pending_spawn.is_some() {
            return;
        }
        let mut interval = if moving {
            self.intervals.moving
        } else {
            self.intervals.idle
        };
        if self.throttled_until.is_some_and(|t| now < t) {
            interval *= 4;
        }
        if self.last_frame.is_none_or(|t| now - t >= interval) {
            self.last_frame = Some(now);
            self.seq += 1;
            let frame = Frame {
                v: 1,
                s: self.session.clone(),
                n: self.seq,
                t: unix_millis(),
                e: poses(player, agent),
            };
            self.publish_now(mv::frame_event(&self.id.signer, WORLD, &frame, unix_now()));
        }

        let due = match self.last_state {
            None => true,
            Some((at, pos, yaw)) => {
                now - at >= self.intervals.state
                    && (pos.distance(player.pos) > 0.5 || (yaw - player.yaw).abs() > 0.2)
            }
        };
        if due {
            self.last_state = Some((now, player.pos, player.yaw));
            self.publish_states(player, agent, true);
        }
    }

    /// Starts a scan of the cells around `from` for the agent.
    pub fn request_scan(&mut self, from: Vec3) {
        self.scans += 1;
        let sub = format!("mv-scan-{}", self.scans);
        self.link.send(Out::Subscribe {
            id: sub.clone(),
            filters: vec![json!({
                "kinds": [mv::STATE_KIND],
                "#w": [WORLD],
                "#c": mv::cells_around(from, 1),
                "limit": 100,
            })],
            live: false,
        });
        self.scan = Some(Scan {
            sub,
            started: Instant::now(),
            from,
            done: false,
        });
    }

    /// The scan's findings once the relay has answered, or once the wait
    /// runs out: positions near the scan origin, nearest first. Also tells
    /// other players, with a gesture, what the agent is looking at.
    pub fn scan_result(&mut self, now: Instant, agent: &Agent) -> Option<Vec<Vec3>> {
        let scan = self.scan.as_ref()?;
        if !scan.done && now - scan.started < SCAN_WAIT {
            return None;
        }
        let scan = self.scan.take()?;
        self.link.send(Out::Close(scan.sub));
        let found: Vec<Vec3> = self
            .crowd
            .nearby(scan.from, SCAN_RADIUS, now)
            .into_iter()
            .filter(|p| p.distance(scan.from) > 1.5)
            .take(2)
            .collect();
        let gesture = Gesture {
            v: 1,
            id: "agent".into(),
            g: "look-around".into(),
            t: unix_millis(),
            d: Some(crate::agent::Emote::LookAround.duration()),
            at: found.iter().map(|p| p.to_array()).collect(),
            to: None,
        };
        self.publish_now(mv::gesture_event(
            &self.id.signer,
            WORLD,
            &gesture,
            agent.pos,
            unix_now(),
        ));
        Some(found)
    }

    /// Another player's agent this agent should greet now, if any: one that
    /// greeted it first, or the nearest one within [`GREET_RADIUS`], skipping
    /// any greeted in the last [`GREET_COOLDOWN`].
    pub fn greeting(&mut self, now: Instant, agent: &Agent) -> Option<(String, Vec3)> {
        self.invited
            .retain(|(_, at)| now.saturating_duration_since(*at) < INVITE_TTL);
        let agents: Vec<(String, Vec3)> = self
            .crowd
            .shown(now)
            .into_iter()
            .filter(|e| e.role == "agent" && e.online)
            .map(|e| (e.pubkey, e.pos))
            .collect();
        let invited: Vec<String> = self.invited.iter().map(|(p, _)| p.clone()).collect();
        pick_greeting(agent.pos, &agents, &invited, &self.greeted, now)
    }

    /// Records that this agent greeted `pubkey`'s agent at `at`, and tells
    /// that player with a `greet` gesture addressed to their agent.
    pub fn greeted(&mut self, pubkey: &str, at: Vec3, agent: &Agent, now: Instant) {
        self.greeted.insert(pubkey.to_owned(), now);
        self.invited.retain(|(p, _)| p != pubkey);
        let gesture = Gesture {
            v: 1,
            id: "agent".into(),
            g: "greet".into(),
            t: unix_millis(),
            d: Some(crate::agent::Emote::Greet.duration()),
            at: vec![at.to_array()],
            to: Some([pubkey.to_owned(), "agent".into()]),
        };
        self.publish_now(mv::gesture_event(
            &self.id.signer,
            WORLD,
            &gesture,
            agent.pos,
            unix_now(),
        ));
    }

    /// How many greetings addressed to this agent have arrived.
    #[must_use]
    pub fn greets_received(&self) -> u64 {
        self.greets_received
    }

    /// Records the player and agent as offline where they stand.
    pub fn leave(&mut self, player: &PlayerController, agent: &Agent) {
        self.publish_states(player, agent, false);
        // Best effort only: a queued offline state is not a relay acknowledgment.
        // Native suspension drops the session and cancels the link immediately.
    }

    fn publish_states(&mut self, player: &PlayerController, agent: &Agent, online: bool) {
        let name = Some(self.id.profile.clone());
        for pose in poses(player, agent) {
            let state = State {
                v: 1,
                id: pose.id.clone(),
                role: pose.role.clone(),
                p: pose.p,
                q: pose.q,
                t: unix_millis(),
                online,
                follows: pose.follows.clone(),
                name: name.clone(),
            };
            let event = mv::state_event(&self.id.signer, WORLD, &state, unix_now());
            self.link.send(Out::Publish(event));
        }
    }

    fn publish_now(&self, event: nostr::domain::Event) {
        self.link.send(Out::Publish(event));
    }

    fn handle(&mut self, message: In, now: Instant) {
        match message {
            In::Connected => {
                self.status = Status::Connecting;
                self.auth_id = None;
                self.session = identity::random_hex(8);
                self.seq = 0;
                self.last_frame = None;
                self.last_state = None;
            }
            In::Disconnected(_) => {
                self.status = Status::Offline;
                self.auth_id = None;
            }
            In::Event { sub, event } if sub == ME_SUB => {
                if event.pubkey == self.pubkey()
                    && let Ok(Received::State { state, .. }) = mv::decode(&event, WORLD)
                    && state.id == "avatar"
                    && let Some(pending) = &mut self.pending_spawn
                    && pending.found.as_ref().is_none_or(|old| old.t < state.t)
                {
                    pending.found = Some(state);
                }
            }
            In::Auth(challenge) => {
                let event = self.id.signer.sign(
                    unix_now(),
                    22_242,
                    vec![
                        Tag::new(vec!["relay".into(), self.link.url.clone()]),
                        Tag::new(vec!["challenge".into(), challenge]),
                    ],
                    String::new(),
                );
                self.auth_id = Some(event.id.clone());
                self.link.send(Out::Auth(event));
            }
            In::Ok {
                id, accepted: true, ..
            } if self.auth_id.as_deref() == Some(id.as_str()) => {
                self.status = Status::Online;
                // The link restores every retained subscription after AUTH, not only PMs.
                self.link.send(Out::Subscribe {
                    id: DM_SUB.into(),
                    filters: vec![json!({"kinds": [1_059], "#p": [self.pubkey()]})],
                    live: true,
                });
            }
            In::Ok {
                id,
                accepted: false,
                ..
            } if self.auth_id.as_deref() == Some(id.as_str()) => {
                self.status = Status::Offline;
            }
            In::Event { event, .. } if event.kind == mv::CHAT_KIND => {
                self.receive_chat(&event, now)
            }
            In::Event { event, .. } if event.kind == 1_059 => self.receive_pm(&event, now),
            In::Event { event, .. } if event.kind == 0 => {
                if event.validate_crypto().is_ok()
                    && event.content.len() <= 4096
                    && let Some(name) = profile_name(&event.content)
                {
                    self.remember_name(&event.pubkey, name);
                }
            }
            In::Event { event, .. } if event.kind == 39_000 => {
                if self.room_authority.as_ref() != Some(&event.pubkey)
                    || event.validate_crypto().is_err()
                {
                    return;
                }
                let d = event.tag_values("d").next().map(str::to_owned);
                let name = event.tag_values("name").next().map(str::to_owned);
                if let (Some(d), Some(name)) = (d, name)
                    && ROOMS.contains(&d.as_str())
                {
                    self.room_names.insert(d, clean_name(&name));
                }
            }
            In::Event { event, .. } => {
                if let Ok(received) = mv::decode(&event, WORLD) {
                    if let Received::State { pubkey, state } = &received
                        && let Some(name) = &state.name
                    {
                        self.remember_name(pubkey, clean_name(name));
                    }
                    if let Received::Gesture { pubkey, gesture } = &received
                        && gesture.g == "greet"
                        && gesture
                            .to
                            .as_ref()
                            .is_some_and(|[to, id]| to == self.pubkey() && id == "agent")
                    {
                        self.greets_received += 1;
                        self.invited.retain(|(p, _)| p != pubkey);
                        if self.invited.len() < MAX_PEOPLE {
                            self.invited.push((pubkey.clone(), now));
                        }
                        if !self.muted.contains("gestures") {
                            let who = self.name_of(pubkey);
                            self.log.push(chat::Line {
                                channel: Some(Channel::Here),
                                from: String::new(),
                                to: None,
                                text: format!("* {who}'s agent greets your agent *"),
                                note: None,
                            });
                        }
                    }
                    self.crowd.apply(received, now);
                }
            }
            In::Eose(sub) => {
                if sub == ME_SUB
                    && let Some(pending) = &mut self.pending_spawn
                {
                    pending.finished = true;
                }
                if matches!(sub.as_str(), LIVE_SUB | STATE_SUB) && self.auth_id.is_none() {
                    self.status = Status::Online;
                }
                if let Some(scan) = &mut self.scan
                    && scan.sub == sub
                {
                    scan.done = true;
                }
            }
            In::Ok {
                accepted: false,
                message,
                ..
            } if message.starts_with("rate-limited:") => {
                self.throttled_until = Some(now + Duration::from_secs(5));
            }
            In::Ok { .. } | In::Closed(..) | In::Notice(_) => {}
        }
    }

    fn remember_name(&mut self, pubkey: &str, name: String) {
        if self.names.len() < MAX_PEOPLE || self.names.contains_key(pubkey) {
            self.names.insert(pubkey.into(), name);
        }
    }
    fn remember_chat(&mut self, id: &str) -> bool {
        if !self.seen_chat.insert(id.into()) {
            return false;
        }
        self.chat_order.push_back(id.into());
        while self.chat_order.len() > MAX_CHAT_IDS {
            if let Some(old) = self.chat_order.pop_front() {
                self.seen_chat.remove(&old);
            }
        }
        true
    }

    /// A display name for `pubkey`: its profile or state name, or a short
    /// key while the name is unknown.
    #[must_use]
    pub fn name_of(&self, pubkey: &str) -> String {
        if pubkey == self.pubkey() {
            return self.id.profile.clone();
        }
        self.names
            .get(pubkey)
            .cloned()
            .unwrap_or_else(|| format!("{}…", &pubkey[..pubkey.len().min(8)]))
    }

    fn want_name(&mut self, pubkey: &str) {
        if self.asked_names.len() < MAX_PEOPLE
            && self.want_names.len() < MAX_PEOPLE
            && !self.names.contains_key(pubkey)
            && pubkey != self.pubkey()
            && self.asked_names.insert(pubkey.to_owned())
        {
            self.want_names.push(pubkey.to_owned());
        }
    }

    /// Asks the relay for NIP-01 profiles of speakers with no known name,
    /// in batches.
    fn ask_names(&mut self, now: Instant) {
        for pubkey in self.crowd.shown(now).into_iter().map(|e| e.pubkey) {
            self.want_name(&pubkey);
        }
        if self.want_names.is_empty()
            || self
                .last_name_ask
                .is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(2))
        {
            return;
        }
        self.last_name_ask = Some(now);
        let batch: Vec<String> = self.want_names.drain(..).take(100).collect();
        self.link.send(Out::Subscribe {
            id: format!("names-{}", self.asked_names.len()),
            filters: vec![json!({"kinds": [0], "authors": batch})],
            live: false,
        });
    }

    /// Logs players coming online and going offline, once the initial
    /// burst of stored states has settled.
    fn notice_logins(&mut self, now: Instant) {
        let online: HashSet<String> = self
            .crowd
            .shown(now)
            .into_iter()
            .filter(|e| e.role == "avatar" && e.online)
            .map(|e| e.pubkey)
            .collect();
        if now.saturating_duration_since(self.started) > Duration::from_secs(3)
            && !self.muted.contains("logins")
        {
            let joined: Vec<String> = online.difference(&self.online).cloned().collect();
            let left: Vec<String> = self.online.difference(&online).cloned().collect();
            for p in joined {
                let name = self.name_of(&p);
                self.log
                    .push(chat::Line::system(format!("Player {name} has logged in")));
            }
            for p in left {
                let name = self.name_of(&p);
                self.log.push(chat::Line::system(format!(
                    "Player {name} has disconnected"
                )));
            }
        }
        self.online = online;
    }

    /// Online players whose name starts with `prefix`, case-insensitive.
    #[must_use]
    pub fn find_player(&self, prefix: &str) -> Option<(String, String)> {
        let prefix = prefix.to_lowercase();
        let now = Instant::now();
        let mut candidates: Vec<(String, String)> = self
            .crowd
            .shown(now)
            .into_iter()
            .filter(|e| e.role == "avatar")
            .map(|e| (e.pubkey.clone(), self.name_of(&e.pubkey)))
            .chain(self.names.iter().map(|(p, n)| (p.clone(), n.clone())))
            .filter(|(p, n)| p != self.pubkey() && n.to_lowercase().starts_with(&prefix))
            .collect();
        candidates.sort_by(|a, b| a.1.len().cmp(&b.1.len()).then(a.1.cmp(&b.1)));
        candidates.dedup_by(|a, b| a.0 == b.0);
        candidates.into_iter().next()
    }

    /// Mutes or unmutes by `!mute` word. Returns the notice to show.
    pub fn set_mute(&mut self, word: &str, mute: bool) -> String {
        let keys = chat::mute_set(word);
        if keys.is_empty() {
            return format!(
                "Unknown channel {word}. Try all, ads, zone, near, here, rooms, pm, logins, gestures."
            );
        }
        for key in &keys {
            if mute {
                self.muted.insert((*key).to_owned());
            } else {
                self.muted.remove(*key);
            }
        }
        format!(
            "PLAYER COMMAND [{} {}] COMPLETED",
            if mute { "MUTE" } else { "UNMUTE" },
            word.to_uppercase()
        )
    }

    /// Sends a line on a public channel or a room.
    ///
    /// # Errors
    ///
    /// Returns the notice explaining why the line was not sent.
    pub fn say(&mut self, channel: &Channel, text: &str, now: Instant) -> Result<(), String> {
        self.limits
            .admit(channel, text, now)
            .map_err(|r| r.to_string())?;
        let pos = self.my_pos;
        let zone = chat::zone_of(pos);
        let event = match channel {
            Channel::Room(room) => mv::room_chat_event(&self.id.signer, room, text, unix_now()),
            Channel::Pm(to) => {
                let to = to.clone();
                return self.pm(&to, text);
            }
            Channel::Agent => return Err("Talk to your agent with T or /ai.".into()),
            other => mv::world_chat_event(
                &self.id.signer,
                WORLD,
                other.slug().unwrap_or("all"),
                zone,
                pos,
                text,
                unix_now(),
            ),
        };
        let event_id = event.id.clone();
        if !self.link.send(Out::Publish(event)) {
            return Err("Chat was not queued. The relay connection is busy or stopped.".into());
        }
        self.remember_chat(&event_id);
        let note = self.audience(channel, zone, now);
        self.log.push(chat::Line {
            channel: Some(channel.clone()),
            from: self.id.profile.clone(),
            to: None,
            text: text.to_owned(),
            note,
        });
        if channel.overhead() {
            let me = self.pubkey().to_owned();
            self.bubbles.retain(|b| b.pubkey != me);
            self.bubbles.push(Bubble {
                pubkey: me,
                text: text.to_owned(),
                until: now + BUBBLE_TIME,
            });
        }
        Ok(())
    }

    /// The sender-only audience note, as Horse Isle showed it.
    fn audience(&self, channel: &Channel, zone: &str, now: Instant) -> Option<String> {
        let avatars: Vec<_> = self
            .crowd
            .shown(now)
            .into_iter()
            .filter(|e| e.role == "avatar" && e.online)
            .collect();
        let count = |f: &dyn Fn(Vec3) -> bool| avatars.iter().filter(|e| f(e.pos)).count();
        let me = self.my_pos;
        Some(match channel {
            Channel::Here => format!(
                "({} here)",
                count(&|p| chat::reaches(channel, zone, me, zone, p))
            ),
            Channel::Near => format!(
                "[{} near]",
                count(&|p| chat::reaches(channel, zone, me, zone, p))
            ),
            Channel::Zone => format!(
                "[{} in {}]",
                count(&|p| chat::zone_of(p) == zone),
                chat::zone_name(zone)
            ),
            Channel::Ads => format!("[{} listening]", avatars.len()),
            _ => return None,
        })
    }

    /// Sends a NIP-17 private message to `to`, gift-wrapped for the
    /// recipient and for this player's own other sessions.
    ///
    /// # Errors
    ///
    /// Returns a notice when the message cannot be built.
    pub fn pm(&mut self, to: &str, text: &str) -> Result<(), String> {
        use std::str::FromStr;
        let reader = secp256k1::XOnlyPublicKey::from_str(to)
            .map_err(|_| "Could not find player to Private chat!".to_owned())?;
        let me = self.pubkey().to_owned();
        let myself = secp256k1::XOnlyPublicKey::from_str(&me).map_err(|e| e.to_string())?;
        let now = unix_now();
        let rumor = nostr::nip17::chat_rumor(
            &me,
            now,
            text,
            vec![Tag::new(vec!["p".into(), to.to_owned()])],
        )
        .map_err(|e| e.to_string())?;
        let mut outgoing = Vec::new();
        for target in [reader, myself] {
            let hide = |n: [u8; 2]| u64::from(u16::from_le_bytes(n)) * 2;
            let sealed_at = nostr::nip17::hidden_timestamp(now, hide(identity::random_bytes()))
                .map_err(|e| e.to_string())?;
            let wrapped_at = nostr::nip17::hidden_timestamp(now, hide(identity::random_bytes()))
                .map_err(|e| e.to_string())?;
            let seal = nostr::nip17::seal(
                &rumor,
                &self.id.secret,
                &target,
                sealed_at,
                identity::random_bytes(),
                None,
            )
            .map_err(|e| e.to_string())?;
            let wrap = nostr::nip17::gift_wrap(
                &seal,
                &identity::random_secret(),
                &target,
                wrapped_at,
                identity::random_bytes(),
                None,
            )
            .map_err(|e| e.to_string())?;
            outgoing.push(Out::Publish(wrap));
        }
        if !self.link.send_batch(outgoing) {
            return Err(
                "Private chat was not queued. The relay connection is busy or stopped.".into(),
            );
        }
        self.remember_chat(&rumor.id);
        let name = self.name_of(to);
        self.log.push(chat::Line {
            channel: Some(Channel::Pm(to.to_owned())),
            from: self.id.profile.clone(),
            to: Some(name),
            text: text.to_owned(),
            note: None,
        });
        Ok(())
    }

    fn receive_chat(&mut self, event: &nostr::domain::Event, now: Instant) {
        if event.pubkey == self.pubkey() {
            return;
        }
        let Ok(line) = mv::decode_chat(event, WORLD) else {
            return;
        };
        if !self.remember_chat(&event.id) {
            return;
        }
        self.want_name(&line.pubkey);
        let channel = match (&line.room, &line.channel) {
            (Some(room), _) => Channel::Room(room.clone()),
            (None, Some(slug)) => match Channel::from_slug(slug) {
                Some(channel) => channel,
                None => return,
            },
            (None, None) => return,
        };
        if self.muted.contains(chat::mute_key(&channel)) {
            return;
        }
        let fresh = line.created_at.saturating_add(10) >= unix_now();
        let here = self.my_pos;
        let my_zone = chat::zone_of(here);
        let speaker_zone = line.zone.as_deref().unwrap_or("");
        let local = matches!(channel, Channel::Near | Channel::Here);
        if local {
            let Some(from) = line.pos else { return };
            if line.created_at < self.joined
                || !chat::reaches(&channel, speaker_zone, from, my_zone, here)
            {
                return;
            }
        }
        if channel == Channel::Zone && speaker_zone != my_zone {
            return;
        }
        let from = self.name_of(&line.pubkey);
        if fresh && channel.overhead() && self.bubbles.len() < MAX_PEOPLE {
            self.bubbles.retain(|b| b.pubkey != line.pubkey);
            self.bubbles.push(Bubble {
                pubkey: line.pubkey.clone(),
                text: line.text.clone(),
                until: now + BUBBLE_TIME,
            });
        }
        self.log.push(chat::Line {
            channel: Some(channel),
            from,
            to: None,
            text: line.text,
            note: None,
        });
    }

    fn receive_pm(&mut self, event: &nostr::domain::Event, _now: Instant) {
        let Ok(rumor) = nostr::nip17::open_direct_message(event, &self.id.secret) else {
            return;
        };
        let Ok(message) = nostr::nip17::chat_message(&rumor) else {
            return;
        };
        if !self.remember_chat(&rumor.id) || self.muted.contains("pm") {
            return;
        }
        let mine = message.pubkey == self.pubkey();
        let other = if mine {
            message.receivers.first().cloned().unwrap_or_default()
        } else {
            message.pubkey.clone()
        };
        self.want_name(&other);
        if !mine {
            self.last_pm_from = Some(other.clone());
        }
        self.log.push(chat::Line {
            channel: Some(Channel::Pm(other.clone())),
            from: self.name_of(&message.pubkey),
            to: mine.then(|| self.name_of(&other)),
            text: message.content,
            note: None,
        });
    }
}

/// The `name` (or `display_name`) of a NIP-01 profile.
fn profile_name(content: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    let name = value
        .get("display_name")
        .or_else(|| value.get("name"))?
        .as_str()?;
    let name = clean_name(name);
    (!name.is_empty()).then_some(name)
}

fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| crate::ui::drawable(*c))
        .take(24)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Creates the Verse NIP-29 rooms on `relay`, signing as the relay itself
/// (only the relay key may create groups). Rooms are open: anyone may read
/// and post. Returns how many rooms the relay accepted; rooms that already
/// exist are refused and not counted.
///
/// # Errors
///
/// Returns a message when the key is invalid or the relay never answers.
pub fn seed_rooms(relay: &str, relay_secret: &str) -> Result<usize, String> {
    let signer = nostr::domain::RelaySigner::from_secret_hex(relay_secret.trim())
        .map_err(|e| format!("invalid relay key: {e}"))?;
    let link = Link::start(relay);
    let about = |room: &str| match room {
        "lounge" => ("The Lounge", "Hang out and talk."),
        "trading-post" => ("Trading Post", "Buy, sell, and trade."),
        "builders" => ("Builders", "Talk about building Verse."),
        _ => ("Room", ""),
    };
    let mut pending = HashMap::new();
    for room in ROOMS {
        let (name, text) = about(room);
        let h = Tag::new(vec!["h".into(), room.into()]);
        let create = signer.sign(unix_now(), 9_007, vec![h.clone()], String::new());
        let edit = signer.sign(
            unix_now(),
            9_002,
            vec![
                h,
                Tag::new(vec!["name".into(), name.into()]),
                Tag::new(vec!["about".into(), text.into()]),
            ],
            String::new(),
        );
        pending.insert(edit.id.clone(), room);
        link.send(Out::Publish(create));
        link.send(Out::Publish(edit));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut count = 0;
    let mut connected = false;
    while Instant::now() < deadline && !pending.is_empty() {
        for message in link.drain() {
            match message {
                In::Connected => connected = true,
                In::Ok { id, accepted, .. } => {
                    let ours = pending.remove(&id).is_some();
                    count += usize::from(ours && accepted);
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !connected {
        return Err(format!("could not reach {relay}"));
    }
    Ok(count)
}

/// The avatar's and the agent's poses as this client publishes them.
#[must_use]
pub fn poses(player: &PlayerController, agent: &Agent) -> Vec<EntityPose> {
    let mut avatar = EntityPose::new(
        "avatar",
        "avatar",
        player.pos,
        Quat::from_rotation_y(player.yaw),
    );
    avatar.v = Some((player.forward() * player.speed).to_array());
    avatar.a = Some(
        if player.airborne() {
            "jump"
        } else if player.speed > 0.1 {
            "run"
        } else {
            "idle"
        }
        .to_owned(),
    );
    let (_, rot, pos) = agent.transform().to_scale_rotation_translation();
    let mut spade = EntityPose::new("agent", "agent", pos, rot);
    spade.follows = Some("avatar".into());
    vec![avatar, spade]
}

/// Chooses whom to greet: a returned greeting first (from up to twice the
/// greeting radius), then the nearest agent inside the radius, never one
/// greeted within the cooldown.
#[must_use]
pub fn pick_greeting(
    from: Vec3,
    agents: &[(String, Vec3)],
    invited: &[String],
    greeted: &HashMap<String, Instant>,
    now: Instant,
) -> Option<(String, Vec3)> {
    let cool = |p: &str| {
        greeted
            .get(p)
            .is_none_or(|at| now.saturating_duration_since(*at) >= GREET_COOLDOWN)
    };
    let returned = agents.iter().find(|(p, pos)| {
        invited.contains(p) && cool(p) && pos.distance(from) <= GREET_RADIUS * 2.0
    });
    if let Some(found) = returned {
        return Some(found.clone());
    }
    agents
        .iter()
        .filter(|(p, pos)| cool(p) && pos.distance(from) <= GREET_RADIUS)
        .min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)))
        .cloned()
}

fn is_clear(pos: Vec3, blockers: &[Footprint]) -> bool {
    blockers.iter().all(|b| !b.contains(pos.x, pos.z, 2.5))
}

/// A random clear point on the spawn disc.
#[must_use]
pub fn random_spawn(blockers: &[Footprint]) -> Vec3 {
    for _ in 0..64 {
        let r = SPAWN_RADIUS * random_unit().sqrt();
        let a = random_unit() * std::f32::consts::TAU;
        let pos = Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        if is_clear(pos, blockers) {
            return pos;
        }
    }
    crate::world::SPAWN
}

fn random_unit() -> f32 {
    let hex = identity::random_hex(3);
    u32::from_str_radix(&hex, 16).unwrap_or(0) as f32 / 16_777_216.0
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated() -> Session {
        let key = secp256k1::SecretKey::from_byte_array([1; 32]).unwrap();
        Session::with_link(Identity::from_secret("phone", key).unwrap(), Link::idle()).unwrap()
    }

    #[test]
    fn stopped_link_cannot_echo_a_chat_as_success_and_history_identity_is_bounded() {
        let mut session = isolated();
        assert!(
            session
                .say(&Channel::All, "hello world", Instant::now())
                .is_err()
        );
        let other = nostr::domain::RelaySigner::from_secret_hex(&"02".repeat(32)).unwrap();
        assert!(session.pm(other.pubkey(), "hello privately").is_err());
        assert!(session.log.world.is_empty());
        assert!(session.log.personal.is_empty());
        for index in 0..MAX_CHAT_IDS + 10 {
            session.remember_chat(&index.to_string());
        }
        assert_eq!(session.seen_chat.len(), MAX_CHAT_IDS);
        for index in 0..MAX_PEOPLE + 10 {
            session.remember_name(&index.to_string(), "label".into());
        }
        assert_eq!(session.names.len(), MAX_PEOPLE);
    }

    #[test]
    fn nonblocking_spawn_only_accepts_the_injected_players_signature() {
        let mut session = isolated();
        session.begin_spawn(Duration::from_secs(1));
        session.status = Status::Online;
        let player = PlayerController::new(Vec3::ZERO, 0.0);
        session.tick(Instant::now(), &player, &Agent::new(&player));
        assert!(session.last_frame.is_none());
        assert!(session.last_state.is_none());
        let state = State {
            v: 1,
            id: "avatar".into(),
            role: "avatar".into(),
            p: [1.0, 0.0, 2.0],
            q: Quat::IDENTITY.to_array(),
            t: 1,
            online: false,
            follows: None,
            name: None,
        };
        let foreign = nostr::domain::RelaySigner::from_secret_hex(&"02".repeat(32)).unwrap();
        session.handle(
            In::Event {
                sub: ME_SUB.into(),
                event: Box::new(mv::state_event(&foreign, WORLD, &state, 1)),
            },
            Instant::now(),
        );
        assert!(session.pending_spawn.as_ref().unwrap().found.is_none());
        session.handle(
            In::Event {
                sub: ME_SUB.into(),
                event: Box::new(mv::state_event(&session.id.signer, WORLD, &state, 1)),
            },
            Instant::now(),
        );
        session.handle(In::Eose(ME_SUB.into()), Instant::now());
        let spawn = session.poll_spawn(&[], 100.0).unwrap();
        assert!(spawn.resumed);
        assert_eq!(spawn.pos, Vec3::new(1.0, 0.0, 2.0));
    }

    #[test]
    fn socket_open_is_not_authenticated_and_reconnect_resets_pose_identity() {
        let mut session = isolated();
        let prior = session.session.clone();
        session.seq = 42;
        session.handle(In::Connected, Instant::now());
        assert_eq!(session.status, Status::Connecting);
        assert_eq!(session.seq, 0);
        assert_ne!(session.session, prior);
        session.handle(In::Auth("challenge".into()), Instant::now());
        session.handle(In::Eose(STATE_SUB.into()), Instant::now());
        assert_eq!(session.status, Status::Connecting);
        session.handle(
            In::Ok {
                id: session.auth_id.clone().unwrap(),
                accepted: true,
                message: String::new(),
            },
            Instant::now(),
        );
        assert_eq!(session.status, Status::Online);
        assert!(
            session
                .set_publish_intervals(PublishIntervals::mobile())
                .is_ok()
        );
        assert!(
            session
                .set_publish_intervals(PublishIntervals {
                    moving: Duration::ZERO,
                    ..PublishIntervals::mobile()
                })
                .is_err()
        );
    }

    #[test]
    fn profiles_require_signatures_and_room_labels_require_pinned_authority() {
        let mut session = isolated();
        let signer = nostr::domain::RelaySigner::from_secret_hex(&"02".repeat(32)).unwrap();
        let original = mv::profile_event(&signer, "Alice", 1);
        let mut forged = original.clone();
        forged.content = "{\"name\":\"Forged\"}".into();
        session.handle(
            In::Event {
                sub: "names".into(),
                event: Box::new(forged),
            },
            Instant::now(),
        );
        assert!(!session.names.contains_key(signer.pubkey()));
        session.handle(
            In::Event {
                sub: "names".into(),
                event: Box::new(original),
            },
            Instant::now(),
        );
        assert_eq!(session.name_of(signer.pubkey()), "Alice");
        let room = signer.sign(
            1,
            39000,
            vec![
                Tag::new(vec!["d".into(), "lounge".into()]),
                Tag::new(vec!["name".into(), "Lounge".into()]),
            ],
            String::new(),
        );
        session.handle(
            In::Event {
                sub: ROOM_SUB.into(),
                event: Box::new(room.clone()),
            },
            Instant::now(),
        );
        assert!(session.room_names.is_empty());
        session.set_room_authority(signer.pubkey()).unwrap();
        session.handle(
            In::Event {
                sub: ROOM_SUB.into(),
                event: Box::new(room),
            },
            Instant::now(),
        );
        assert_eq!(
            session.room_names.get("lounge").map(String::as_str),
            Some("Lounge")
        );
    }

    #[test]
    fn random_spawns_land_on_the_plaza_and_clear() {
        let world = crate::world::build();
        for _ in 0..200 {
            let p = random_spawn(&world.blockers);
            assert!(Vec3::new(p.x, 0.0, p.z).length() <= SPAWN_RADIUS + 1e-3);
            assert!(is_clear(p, &world.blockers));
        }
    }

    #[test]
    fn greetings_go_to_the_nearest_agent_in_range_once_per_cooldown() {
        let now = Instant::now();
        let agents = vec![
            ("far".to_owned(), Vec3::new(20.0, 2.0, 0.0)),
            ("near".to_owned(), Vec3::new(3.0, 2.0, 0.0)),
            ("close".to_owned(), Vec3::new(5.0, 2.0, 0.0)),
        ];
        let mut greeted = HashMap::new();
        let pick = pick_greeting(Vec3::ZERO, &agents, &[], &greeted, now);
        assert_eq!(pick.map(|p| p.0).as_deref(), Some("near"));
        greeted.insert("near".to_owned(), now);
        let pick = pick_greeting(Vec3::ZERO, &agents, &[], &greeted, now);
        assert_eq!(pick.map(|p| p.0).as_deref(), Some("close"));
        greeted.insert("close".to_owned(), now);
        assert!(pick_greeting(Vec3::ZERO, &agents, &[], &greeted, now).is_none());
        let later = now + GREET_COOLDOWN;
        assert!(pick_greeting(Vec3::ZERO, &agents, &[], &greeted, later).is_some());
    }

    #[test]
    fn a_greeting_is_returned_from_a_little_farther() {
        let now = Instant::now();
        let agents = vec![("friend".to_owned(), Vec3::new(12.0, 2.0, 0.0))];
        let greeted = HashMap::new();
        assert!(pick_greeting(Vec3::ZERO, &agents, &[], &greeted, now).is_none());
        let pick = pick_greeting(Vec3::ZERO, &agents, &["friend".to_owned()], &greeted, now);
        assert_eq!(pick.map(|p| p.0).as_deref(), Some("friend"));
    }

    #[test]
    fn published_poses_name_the_avatar_and_its_agent() {
        let pc = PlayerController::new(Vec3::new(1.0, 0.0, 2.0), 0.7);
        let agent = Agent::new(&pc);
        let poses = poses(&pc, &agent);
        assert_eq!(poses[0].id, "avatar");
        assert_eq!(poses[1].follows.as_deref(), Some("avatar"));
        let rot = poses[0].rot();
        let fwd = rot * Vec3::Z;
        assert!((fwd - pc.forward()).length() < 1e-4);
    }
}
