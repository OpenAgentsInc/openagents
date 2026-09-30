//! Watching the Grid without playing in it: the desktop app's backdrop.
//!
//! A [`Spectator`] subscribes to a world's NIP-MV pose frames and entity
//! states and nothing else, and has no way to publish: it holds no
//! identity, and its only messages to the relay are `REQ`s and, when the
//! relay asks for NIP-42 authentication, an `AUTH` signed by a fresh key
//! kept in memory for that one connection. It is never a player: nobody
//! sees it, counts it, or can link it to anyone.
//!
//! An [`Overlook`] is the Grid seen from above: the bare world with nobody
//! playing in it here ([`WorldRuntime::unoccupied`]), other players'
//! avatars as the phones draw them, the shared ball and blocks where their
//! owners report them, and a camera that sways slowly over the plaza, the
//! ball, the blocks, and the Gym. With nobody online the Grid is empty;
//! nothing is invented.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use glam::{Mat4, Vec3};
use nostr::domain::{RelaySigner, Tag};
use serde_json::json;

use crate::crowd::{Crowd, Shown};
use crate::identity::{self, Identity};
use crate::mesh::Mesh;
use crate::mv;
use crate::net::{In, Link, Out};
use crate::render::View;
use crate::runtime::WorldRuntime;
use crate::session::{BARE_WORLD, BodyIn, Status, apply_bodies, split_bodies};

const LIVE_SUB: &str = "mv-live";
const STATE_SUB: &str = "mv-state";
/// How far in the past other players are drawn: the phones' moving interval
/// plus the margin they use, so avatars walk between poses.
pub const DELAY: Duration = Duration::from_millis(3_300);
/// The point the camera circles: between the spawn, the ball, the blocks,
/// and the Gym's doorway.
pub const CENTER: Vec3 = Vec3::new(0.0, 0.0, 10.0);
/// The camera's distance from [`CENTER`] along the ground, m.
pub const ORBIT_RADIUS: f32 = 40.0;
/// The camera's height, m.
pub const HEIGHT: f32 = 58.0;
/// One slow sway of the camera, there and back, s.
pub const SWAY_PERIOD: f32 = 240.0;
/// How far the camera sways to either side of its place behind the spawn,
/// radians. It never swings round behind the Gym, so the picture keeps its
/// composition: the Gym at the top, the plaza below it.
pub const SWAY: f32 = 0.45;
/// Vertical field of view, radians.
const FOV_Y: f32 = 0.9;

/// A subscribe-only view of one world's presence. It has no publish method
/// and signs nothing but a relay's NIP-42 challenge.
pub struct Spectator {
    link: Link,
    world: &'static str,
    /// Other players, interpolated as the phones draw them.
    pub crowd: Crowd,
    /// Subscription status.
    pub status: Status,
    /// Answers NIP-42 challenges only. A fresh key for each spectator, never
    /// stored or shown, so no connection links to another or to a player.
    auth_signer: RelaySigner,
    auth_id: Option<String>,
    auth_accepted: bool,
    live_ready: bool,
    state_ready: bool,
    bodies_in: Vec<BodyIn>,
}

impl Spectator {
    /// Subscribes to `world`'s pose frames and entity states on `relay`.
    /// Like a session, it does not wait for the network.
    ///
    /// # Errors
    ///
    /// Returns a message when `world` is not a valid NIP-MV world identifier.
    pub fn start(relay: &str, world: &'static str) -> Result<Self, String> {
        if world.is_empty() || world.len() > 128 {
            return Err("invalid world identifier".into());
        }
        let key = Identity::from_secret("spectator", identity::random_secret())?;
        let link = Link::start(relay);
        link.send(Out::Subscribe {
            id: LIVE_SUB.into(),
            filters: vec![json!({"kinds": [mv::FRAME_KIND], "#w": [world]})],
            live: true,
        });
        link.send(Out::Subscribe {
            id: STATE_SUB.into(),
            filters: vec![json!({"kinds": [mv::STATE_KIND], "#w": [world], "limit": 500})],
            live: true,
        });
        let mut crowd = Crowd::new("");
        crowd.set_delay(DELAY);
        crowd.set_live_only(true);
        Ok(Self {
            link,
            world,
            crowd,
            status: Status::Connecting,
            auth_signer: key.signer,
            auth_id: None,
            auth_accepted: false,
            live_ready: false,
            state_ready: false,
            bodies_in: Vec::new(),
        })
    }

    /// The world it watches.
    #[must_use]
    pub fn world(&self) -> &'static str {
        self.world
    }

    /// Drains the relay, moves others' players, and applies the shared
    /// bodies' reports to `bodies`. It never claims a body.
    pub fn tick(&mut self, now: Instant, bodies: Option<&mut crate::ball::Ball>) {
        for message in self.link.drain() {
            self.handle(message, now);
        }
        let arrived = std::mem::take(&mut self.bodies_in);
        if let Some(bodies) = bodies {
            apply_bodies(arrived, bodies);
        }
        self.crowd.prune(now);
    }

    fn handle(&mut self, message: In, now: Instant) {
        match message {
            In::Connected => {
                self.status = Status::Connecting;
                self.auth_id = None;
                self.auth_accepted = false;
                self.live_ready = false;
                self.state_ready = false;
            }
            In::Disconnected(_) => {
                self.status = Status::Offline;
                self.auth_id = None;
                self.auth_accepted = false;
                self.live_ready = false;
                self.state_ready = false;
            }
            In::Auth(challenge) => {
                self.status = Status::Connecting;
                self.auth_accepted = false;
                let event = self.auth_signer.sign(
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
                self.auth_accepted = true;
                self.update_status();
            }
            In::Ok {
                id,
                accepted: false,
                ..
            } if self.auth_id.as_deref() == Some(id.as_str()) => {
                self.status = Status::Offline;
            }
            In::Event { event, .. } => {
                if !matches!(event.kind, mv::FRAME_KIND | mv::STATE_KIND) {
                    return;
                }
                if let Ok(received) = mv::decode(&event, self.world)
                    && let Some(received) = split_bodies(received, &mut self.bodies_in)
                    && matches!(
                        received,
                        mv::Received::Frame { .. } | mv::Received::State { .. }
                    )
                {
                    self.crowd.apply(received, now);
                }
            }
            In::Eose(sub) => {
                if sub == LIVE_SUB {
                    self.live_ready = true;
                }
                if sub == STATE_SUB {
                    self.state_ready = true;
                }
                self.update_status();
            }
            In::Closed(sub, reason) if matches!(sub.as_str(), LIVE_SUB | STATE_SUB) => {
                if sub == LIVE_SUB {
                    self.live_ready = false;
                }
                if sub == STATE_SUB {
                    self.state_ready = false;
                }
                self.status = if reason.starts_with("auth-required:") {
                    Status::Connecting
                } else {
                    Status::Offline
                };
            }
            In::Ok { .. } | In::Closed(..) | In::Notice(_) => {}
        }
    }

    fn update_status(&mut self) {
        if self.live_ready && self.state_ready && (self.auth_id.is_none() || self.auth_accepted) {
            self.status = Status::Online;
        }
    }
}

/// The Grid from above, as the desktop app's backdrop shows it.
pub struct Overlook {
    /// The bare world with nobody playing here.
    pub world: WorldRuntime,
    relay: Option<String>,
    spectator: Option<Spectator>,
    last: Option<Instant>,
}

impl Overlook {
    /// Watches the Grid's players on `relay`.
    #[must_use]
    pub fn new(relay: &str) -> Self {
        let mut overlook = Self::offline();
        overlook.relay = Some(relay.to_owned());
        overlook.resume();
        overlook
    }

    /// The empty Grid, with no relay: captures and tests.
    #[must_use]
    pub fn offline() -> Self {
        Self {
            world: WorldRuntime::unoccupied(),
            relay: None,
            spectator: None,
            last: None,
        }
    }

    /// Closes the relay connection and forgets the players: nothing more is
    /// received until [`Self::resume`].
    pub fn pause(&mut self) {
        self.spectator = None;
        self.last = None;
    }

    /// Opens the relay connection again, if there is a relay and it is closed.
    pub fn resume(&mut self) {
        if self.spectator.is_none()
            && let Some(relay) = &self.relay
        {
            self.spectator = Spectator::start(relay, BARE_WORLD).ok();
        }
        self.last = None;
    }

    /// Whether a relay connection is open.
    #[must_use]
    pub fn connected(&self) -> bool {
        self.spectator.is_some()
    }

    /// The relay connection's status, when one is open.
    #[must_use]
    pub fn status(&self) -> Option<Status> {
        self.spectator.as_ref().map(|spectator| spectator.status)
    }

    /// Receives what the relay sent and advances the shared bodies to `now`.
    /// Returns the frame's dt, in seconds.
    pub fn tick(&mut self, now: Instant) -> f32 {
        let dt = self.last.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32()
        });
        self.last = Some(now);
        if let Some(spectator) = &mut self.spectator {
            spectator.tick(now, self.world.ball.as_deref_mut());
        }
        self.world.tick_unoccupied(dt)
    }

    /// The players drawn at `now`: avatars of those online now.
    #[must_use]
    pub fn players(&self, now: Instant) -> Vec<Shown> {
        self.spectator.as_ref().map_or_else(Vec::new, |spectator| {
            spectator
                .crowd
                .shown(now)
                .into_iter()
                .filter(|shown| shown.role == "avatar" && shown.online)
                .collect()
        })
    }

    /// Whether anything in the Grid moves at `now`: a player is online, or
    /// the ball or a block is awake. An empty, settled Grid needs few frames.
    #[must_use]
    pub fn lively(&self, now: Instant) -> bool {
        !self.players(now).is_empty() || self.world.ball().is_some_and(|ball| !ball.at_rest())
    }

    /// The world's moving geometry at `now`: the ball, the blocks, the Gym's
    /// boards, and other players' avatars in the neutral palette, as the
    /// phones draw them. `dt` advances their walk cycles.
    pub fn mesh(&mut self, now: Instant, dt: f32) -> Mesh {
        let mut mesh = self.world.dynamic_mesh();
        if let Some(spectator) = &mut self.spectator {
            let mut others = spectator.crowd.mesh(now, dt);
            others.neutralize();
            others.lit.clear();
            others.glow.clear();
            others.neon = None;
            mesh.extend(&others);
        }
        mesh
    }

    /// The camera `seconds` into its slow sway, for a view `aspect` wide.
    #[must_use]
    pub fn view(aspect: f32, seconds: f32) -> View {
        let aspect = if aspect.is_finite() {
            aspect.clamp(0.1, 10.0)
        } else {
            1.0
        };
        let turn = if seconds.is_finite() {
            SWAY * ((seconds / SWAY_PERIOD).fract() * std::f32::consts::TAU).sin()
        } else {
            0.0
        };
        // Start behind the spawn, looking along it toward the Gym.
        let eye = CENTER
            + Vec3::new(
                turn.sin() * ORBIT_RADIUS,
                HEIGHT,
                -turn.cos() * ORBIT_RADIUS,
            );
        let view = Mat4::look_at_rh(eye, CENTER, Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y, aspect, 0.5, 600.0);
        View {
            view_proj: proj * view,
            eye,
        }
    }

    /// The Grid's fog pushed out for a camera this high, so the plaza stays
    /// clear and only the far grid fades.
    #[must_use]
    pub fn atmosphere(&self) -> crate::zones::Atmosphere {
        let mut atmosphere = self.world.atmosphere();
        atmosphere.fog_start = 45.0;
        atmosphere.fog_end = 160.0;
        atmosphere
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_looks_down_on_the_plaza_and_drifts() {
        let start = Overlook::view(0.8, 0.0);
        let later = Overlook::view(0.8, 60.0);
        assert!(start.eye.y > 30.0, "{}", start.eye);
        assert!(start.eye.distance(later.eye) > 1.0);
        assert!(start.eye.distance(Overlook::view(0.8, SWAY_PERIOD).eye) < 0.01);
        // The ball, the blocks, and the Gym's doorway are in frame all along.
        for seconds in [0.0, SWAY_PERIOD / 4.0, SWAY_PERIOD * 0.75] {
            let view = Overlook::view(0.8, seconds);
            for point in [
                crate::world::SPAWN,
                crate::ball::START.as_vec3(),
                Vec3::new(-5.0, 0.0, 8.0),
                Vec3::new(0.0, 0.0, 26.0),
            ] {
                let clip = view.view_proj * point.extend(1.0);
                let ndc = clip.truncate() / clip.w;
                assert!(
                    clip.w > 0.0 && ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0,
                    "{point} at {ndc}"
                );
            }
        }
    }

    #[test]
    fn an_offline_grid_is_empty_and_draws_no_avatar_of_its_own() {
        let mut overlook = Overlook::offline();
        let now = Instant::now();
        overlook.tick(now);
        overlook.tick(now + Duration::from_millis(33));
        assert!(overlook.players(now).is_empty());
        assert!(overlook.world.is_unoccupied());
        let own = crate::avatar::mesh(&overlook.world.player, &overlook.world.gait);
        let mesh = overlook.mesh(now, 0.033);
        // Only the ball, blocks, and boards: no local avatar's geometry.
        let with_avatar = WorldRuntime::bare().dynamic_mesh();
        assert!(!own.faces.is_empty());
        assert_eq!(mesh.faces.len() + own.faces.len(), with_avatar.faces.len());
    }

    #[test]
    fn an_unoccupied_world_leaves_the_bodies_at_rest() {
        let mut world = WorldRuntime::unoccupied();
        let before = world.ball().map(|ball| ball.pose().0).unwrap();
        for _ in 0..240 {
            world.tick_unoccupied(1.0 / 60.0);
        }
        let ball = world.ball().unwrap();
        assert!(ball.pose().0.distance(before) < 1e-3);
        assert!(!ball.has_outgoing(), "a spectator never claims a body");
        assert!(ball.at_rest(), "the settled Grid needs few frames");
    }
}
