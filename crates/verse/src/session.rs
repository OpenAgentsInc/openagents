//! One player's multiplayer session over NIP-MV.
//!
//! - **Sign-up.** The profile's key is created on first launch, and a
//!   NIP-01 profile names it.
//! - **Spawn.** On launch the session asks the relay for this player's own
//!   avatar state. A returning player resumes where they left; a new one
//!   spawns at a random clear spot on the central plaza.
//! - **Streaming.** Pose frames for the avatar and the agent go out as
//!   ephemeral `23300` events, 10 per second while moving and 4 while
//!   still. Durable `33301` states go out on join, every few seconds of
//!   movement, and on leave.
//! - **Scans.** When the agent looks around, the session queries entity
//!   states in the surrounding cells and reports what is near.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use glam::{Quat, Vec3};
use serde_json::json;

use crate::agent::Agent;
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
        let link = Link::start(relay);
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

    /// Asks the relay where this player left their avatar, waiting up to
    /// `wait`. Falls back to a random clear spot on the plaza.
    pub fn spawn(&mut self, blockers: &[Footprint], bound: f32, wait: Duration) -> Spawn {
        let address = mv::state_address(WORLD, "avatar");
        self.link.send(Out::Subscribe {
            id: ME_SUB.into(),
            filters: vec![json!({
                "kinds": [mv::STATE_KIND],
                "authors": [self.pubkey()],
                "#d": [address],
                "limit": 1,
            })],
            live: false,
        });
        let deadline = Instant::now() + wait;
        let mut found: Option<State> = None;
        'wait: while Instant::now() < deadline {
            for message in self.link.drain() {
                match message {
                    In::Connected => self.status = Status::Online,
                    In::Event { sub, event } if sub == ME_SUB => {
                        if let Ok(Received::State { state, .. }) = mv::decode(&event, WORLD)
                            && state.id == "avatar"
                            && found.as_ref().is_none_or(|f| state.t > f.t)
                        {
                            found = Some(state);
                        }
                    }
                    In::Eose(sub) if sub == ME_SUB => break 'wait,
                    other => self.handle(other, Instant::now()),
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.link.send(Out::Close(ME_SUB.into()));
        if let Some(state) = found {
            let pos = Vec3::from(state.p);
            let clear = pos.x.abs() < bound && pos.z.abs() < bound && is_clear(pos, blockers);
            if clear {
                let (axis, angle) = Quat::from_array(state.q).normalize().to_axis_angle();
                let yaw = if axis.y < 0.0 { -angle } else { angle };
                return Spawn {
                    pos: Vec3::new(pos.x, 0.0, pos.z),
                    yaw: crate::controller::wrap(yaw),
                    resumed: true,
                };
            }
        }
        Spawn {
            pos: random_spawn(blockers),
            yaw: random_unit() * std::f32::consts::TAU - std::f32::consts::PI,
            resumed: false,
        }
    }

    /// Drains the relay and publishes what is due, once per game frame.
    pub fn tick(&mut self, now: Instant, player: &PlayerController, agent: &Agent) {
        for message in self.link.drain() {
            self.handle(message, now);
        }
        self.crowd.prune(now);

        let moving = self
            .last_player
            .is_some_and(|last| last.distance(player.pos) > 0.005)
            || player.speed > 0.05;
        self.last_player = Some(player.pos);
        let mut interval = if moving {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(250)
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
                now - at >= Duration::from_secs(3)
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
        // Give the link thread a moment to flush before the process exits.
        std::thread::sleep(Duration::from_millis(250));
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
            In::Connected => self.status = Status::Online,
            In::Disconnected(_) => self.status = Status::Offline,
            In::Event { event, .. } => {
                if let Ok(received) = mv::decode(&event, WORLD) {
                    if let Received::Gesture { pubkey, gesture } = &received
                        && gesture.g == "greet"
                        && gesture
                            .to
                            .as_ref()
                            .is_some_and(|[to, id]| to == self.pubkey() && id == "agent")
                    {
                        self.greets_received += 1;
                        self.invited.retain(|(p, _)| p != pubkey);
                        self.invited.push((pubkey.clone(), now));
                    }
                    self.crowd.apply(received, now);
                }
            }
            In::Eose(sub) => {
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
