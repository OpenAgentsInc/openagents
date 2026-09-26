//! The player's agent: a floating 3D spade that follows the player.
//!
//! The agent hangs in the air near the player's shoulder. It chases a
//! target point on a slightly underdamped spring, so it trails behind when
//! the player moves and overshoots a little when the player stops. The
//! target wanders, and the body bobs and wobbles on incommensurate
//! frequencies, so the motion never settles into an exact repeat.
//!
//! On top of that motion the agent plays [`Emote`]s. After it chases a
//! running player and catches up, it asks for a scan of its surroundings
//! ([`Agent::take_scan`]); the game queries the relay and hands back what
//! is nearby ([`Agent::look_around`]), and the agent looks toward it, left
//! and then right. While it idles
//! beside a still player, it now and then spins, looks up and down, or
//! does a barrel roll. When it meets another player's agent, it greets it
//! ([`Agent::greet`]).

use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::avatar::dim;
use crate::controller::PlayerController;
use crate::mesh::Mesh;

/// Height of the spade's center above the player's feet, in meters.
pub const HOVER: f32 = 2.2;
/// Where the agent sits relative to the player: meters to the player's
/// right, and meters behind.
pub const OFFSET: Vec2 = Vec2::new(0.9, 1.1);
/// Spring stiffness toward the target, per second squared.
const STIFFNESS: f32 = 9.0;
/// Damping as a fraction of critical. Below one, the agent overshoots.
const DAMPING: f32 = 0.72;
/// The spade's height in meters, tip to stem.
pub const SIZE: f32 = 0.75;
/// Half the spade's thickness, in meters.
const DEPTH: f32 = 0.05;
/// Player speed, in meters per second, that counts as being chased.
const CHASE_SPEED: f32 = 5.0;
/// Seconds of chasing before a catch-up earns a look around.
const CHASE_TIME: f32 = 0.6;
/// Shortest and longest wait between idle emotes, in seconds.
const IDLE_WAIT: (f32, f32) = (5.0, 12.0);

/// A short gesture the agent plays over its ordinary motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emote {
    /// Look left, then right, then ahead. Played after catching up.
    LookAround,
    /// One full turn around the vertical axis.
    Spin,
    /// Tip back to look up, then forward to look down.
    LookUpDown,
    /// One full roll around the facing axis, with a small lift.
    BarrelRoll,
    /// Turn to face another agent, bow twice, and hop. Played when two
    /// agents meet.
    Greet,
}

impl Emote {
    /// The emotes the agent picks from while idle.
    pub const IDLE: [Emote; 3] = [Emote::Spin, Emote::LookUpDown, Emote::BarrelRoll];

    /// Length in seconds.
    #[must_use]
    pub fn duration(self) -> f32 {
        match self {
            Emote::LookAround => 2.4,
            Emote::Spin => 1.2,
            Emote::LookUpDown => 1.8,
            Emote::BarrelRoll => 1.1,
            Emote::Greet => 2.2,
        }
    }

    /// The pose at `u`, from 0 to 1: extra yaw, pitch, roll in radians and
    /// extra height in meters. Every emote starts and ends at rest.
    #[must_use]
    pub fn pose(self, u: f32) -> Pose {
        use std::f32::consts::TAU;
        let u = u.clamp(0.0, 1.0);
        match self {
            Emote::LookAround => Pose {
                yaw: 0.85
                    * keys(
                        u,
                        &[
                            (0.0, 0.0),
                            (0.2, 1.0),
                            (0.4, 1.0),
                            (0.62, -1.0),
                            (0.8, -1.0),
                            (1.0, 0.0),
                        ],
                    ),
                ..Pose::REST
            },
            Emote::Spin => Pose {
                yaw: TAU * ease(u),
                lift: 0.12 * (u * std::f32::consts::PI).sin(),
                ..Pose::REST
            },
            Emote::LookUpDown => Pose {
                pitch: 0.55
                    * keys(
                        u,
                        &[
                            (0.0, 0.0),
                            (0.25, -1.0),
                            (0.45, -1.0),
                            (0.7, 1.0),
                            (0.85, 1.0),
                            (1.0, 0.0),
                        ],
                    ),
                ..Pose::REST
            },
            Emote::BarrelRoll => Pose {
                roll: TAU * ease(u),
                lift: 0.25 * (u * std::f32::consts::PI).sin(),
                ..Pose::REST
            },
            // The turn toward the other agent is added in `Agent::pose`,
            // which knows where it is.
            Emote::Greet => Pose {
                pitch: 0.5
                    * keys(
                        u,
                        &[
                            (0.0, 0.0),
                            (0.25, 0.0),
                            (0.35, 1.0),
                            (0.45, 0.0),
                            (0.55, 1.0),
                            (0.67, 0.0),
                            (1.0, 0.0),
                        ],
                    ),
                lift: keys(
                    u,
                    &[
                        (0.0, 0.0),
                        (0.12, 0.2),
                        (0.24, 0.04),
                        (0.75, 0.04),
                        (1.0, 0.0),
                    ],
                ),
                ..Pose::REST
            },
        }
    }
}

/// An emote's offset from the agent's ordinary pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Extra turn around the vertical axis, in radians. Positive is left.
    pub yaw: f32,
    /// Extra tip around the side axis, in radians. Negative looks up.
    pub pitch: f32,
    /// Extra roll around the facing axis, in radians.
    pub roll: f32,
    /// Extra height, in meters.
    pub lift: f32,
}

impl Pose {
    /// No offset.
    pub const REST: Pose = Pose {
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        lift: 0.0,
    };
}

/// Smoothstep between keyframes `(u, value)` sorted by `u`.
fn keys(u: f32, frames: &[(f32, f32)]) -> f32 {
    for pair in frames.windows(2) {
        let ((u0, a), (u1, b)) = (pair[0], pair[1]);
        if u <= u1 {
            let t = ((u - u0) / (u1 - u0)).clamp(0.0, 1.0);
            return a + (b - a) * ease(t);
        }
    }
    frames.last().map_or(0.0, |f| f.1)
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// The emote playing now, and how far into it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    emote: Emote,
    elapsed: f32,
    /// Look-around glance angles, left then right, in radians.
    glance: [f32; 2],
    /// What a greeting faces.
    toward: Option<Vec3>,
}

/// How much of the turn toward a greeted agent applies at `u`.
fn greet_turn(u: f32) -> f32 {
    keys(
        u.clamp(0.0, 1.0),
        &[(0.0, 0.0), (0.15, 1.0), (0.85, 1.0), (1.0, 0.0)],
    )
}

/// The look-around glances when nothing nearby is known.
pub const DEFAULT_GLANCE: [f32; 2] = [0.85, -0.85];

/// Look-around pose at `u` with custom glance angles.
fn look_pose(u: f32, glance: [f32; 2]) -> Pose {
    let u = u.clamp(0.0, 1.0);
    let (a, b) = (glance[0], glance[1]);
    Pose {
        yaw: keys(
            u,
            &[
                (0.0, 0.0),
                (0.2, a),
                (0.4, a),
                (0.62, b),
                (0.8, b),
                (1.0, 0.0),
            ],
        ),
        ..Pose::REST
    }
}

/// The agent's motion state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agent {
    /// Center of the spade in world space.
    pub pos: Vec3,
    vel: Vec3,
    /// Heading in radians, the same convention as the player's yaw.
    pub yaw: f32,
    time: f32,
    playing: Option<Playing>,
    chased: f32,
    owed_look: bool,
    scan_wanted: bool,
    scanning: bool,
    idle_wait: f32,
    rng: u64,
}

impl Agent {
    /// An agent already at rest beside `player`.
    #[must_use]
    pub fn new(player: &PlayerController) -> Self {
        Self {
            pos: target(player, 0.0),
            vel: Vec3::ZERO,
            yaw: player.yaw,
            time: 0.0,
            playing: None,
            chased: 0.0,
            owed_look: false,
            scan_wanted: false,
            scanning: false,
            idle_wait: IDLE_WAIT.0,
            rng: 0x5eed_a6e7,
        }
    }

    /// An agent at rest at `pos`, heading `yaw`, with no player: a replay's
    /// ghost.
    #[must_use]
    pub fn at(pos: Vec3, yaw: f32) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            yaw,
            time: 0.0,
            playing: None,
            chased: 0.0,
            owed_look: false,
            scan_wanted: false,
            scanning: false,
            idle_wait: IDLE_WAIT.0,
            rng: 0x5eed_9057,
        }
    }

    /// The emote playing now, if any.
    #[must_use]
    pub fn emote(&self) -> Option<Emote> {
        self.playing.map(|p| p.emote)
    }

    /// The current emote pose, or rest.
    #[must_use]
    pub fn pose(&self) -> Pose {
        self.playing.map_or(Pose::REST, |p| {
            let u = p.elapsed / p.emote.duration();
            match (p.emote, p.toward) {
                (Emote::LookAround, _) => look_pose(u, p.glance),
                (Emote::Greet, Some(toward)) => {
                    let d = toward - self.pos;
                    let face = crate::controller::wrap(d.x.atan2(d.z) - self.yaw);
                    Pose {
                        yaw: face * greet_turn(u),
                        ..Emote::Greet.pose(u)
                    }
                }
                (emote, _) => emote.pose(u),
            }
        })
    }

    /// Greets the agent at `toward`: turns to face it, bows twice, and
    /// hops. Returns false, doing nothing, while the agent is scanning,
    /// looking around, or already greeting; idle emotes give way.
    pub fn greet(&mut self, toward: Vec3) -> bool {
        let busy =
            self.scanning || matches!(self.emote(), Some(Emote::LookAround) | Some(Emote::Greet));
        if busy {
            return false;
        }
        self.playing = Some(Playing {
            emote: Emote::Greet,
            elapsed: 0.0,
            glance: DEFAULT_GLANCE,
            toward: Some(toward),
        });
        true
    }

    /// True once when the agent has caught up after a chase and wants to
    /// assess its surroundings. The caller answers with
    /// [`Agent::look_around`].
    pub fn take_scan(&mut self) -> bool {
        std::mem::take(&mut self.scan_wanted)
    }

    /// True between a scan request and its answer.
    #[must_use]
    pub fn scanning(&self) -> bool {
        self.scanning
    }

    /// Plays the look-around toward `targets`, world positions the scan
    /// found, nearest first. With none, it glances a default left and right.
    pub fn look_around(&mut self, targets: &[Vec3]) {
        self.scanning = false;
        let mut angles: Vec<f32> = targets
            .iter()
            .take(2)
            .map(|t| {
                let d = *t - self.pos;
                let heading = d.x.atan2(d.z);
                crate::controller::wrap(heading - self.yaw).clamp(-1.4, 1.4)
            })
            .collect();
        angles.sort_by(|a, b| b.total_cmp(a));
        let glance = match angles.as_slice() {
            [a, b] if (a - b).abs() > 0.2 => [*a, *b],
            [a, ..] if *a >= 0.0 => [*a, DEFAULT_GLANCE[1]],
            [a, ..] => [DEFAULT_GLANCE[0], *a],
            [] => DEFAULT_GLANCE,
        };
        self.playing = Some(Playing {
            emote: Emote::LookAround,
            elapsed: 0.0,
            glance,
            toward: None,
        });
    }

    /// Advances the agent by `dt` seconds toward its place beside `player`.
    pub fn update(&mut self, player: &PlayerController, dt: f32) {
        self.time += dt;
        let goal = target(player, self.time);
        self.spring(goal, dt);

        // Turn toward the player's heading, lazily.
        let delta = crate::controller::wrap(player.yaw - self.yaw);
        self.yaw = crate::controller::wrap(self.yaw + delta * (1.0 - 0.08f32.powf(dt)));

        self.emotes(player, goal, dt);
    }

    /// Advances the agent by `dt` seconds toward `goal`, a point a replay
    /// moves between places, on the same spring and with the same drift
    /// and bob as when it follows the player. It turns to face where it is
    /// going, and plays no idle emotes.
    pub fn visit(&mut self, goal: Vec3, dt: f32) {
        self.time += dt;
        let goal = goal + wander(self.time);
        self.spring(goal, dt);
        let ahead = (goal - self.pos).with_y(0.0);
        if ahead.length() > 0.6 {
            let delta = crate::controller::wrap(ahead.x.atan2(ahead.z) - self.yaw);
            self.yaw = crate::controller::wrap(self.yaw + delta * (1.0 - 0.02f32.powf(dt)));
        }
        self.advance_emote(dt);
    }

    /// One full spin: what an agent does when its replayed run passes.
    pub fn celebrate(&mut self) {
        self.play(Emote::Spin);
    }

    fn spring(&mut self, goal: Vec3, dt: f32) {
        let damping = 2.0 * STIFFNESS.sqrt() * DAMPING;
        let accel = (goal - self.pos) * STIFFNESS - self.vel * damping;
        self.vel += accel * dt;
        self.pos += self.vel * dt;
    }

    fn advance_emote(&mut self, dt: f32) {
        if let Some(playing) = &mut self.playing {
            playing.elapsed += dt;
            if playing.elapsed >= playing.emote.duration() {
                self.playing = None;
            }
        }
    }

    fn emotes(&mut self, player: &PlayerController, goal: Vec3, dt: f32) {
        self.advance_emote(dt);

        if player.speed > CHASE_SPEED {
            self.chased += dt;
            if self.chased > CHASE_TIME {
                self.owed_look = true;
            }
        } else {
            self.chased = 0.0;
        }

        let still = player.speed < 0.1 && !player.airborne();
        let caught_up = self.pos.distance(goal) < 0.45 && self.vel.length() < 0.6;
        if !still || !caught_up {
            return;
        }
        if self.owed_look && !self.scanning {
            self.owed_look = false;
            self.scan_wanted = true;
            self.scanning = true;
            return;
        }
        if self.playing.is_none() && !self.scanning {
            self.idle_wait -= dt;
            if self.idle_wait <= 0.0 {
                let pick = Emote::IDLE[(self.random() * 3.0) as usize % 3];
                self.play(pick);
                self.idle_wait = IDLE_WAIT.0 + self.random() * (IDLE_WAIT.1 - IDLE_WAIT.0);
            }
        }
    }

    fn play(&mut self, emote: Emote) {
        self.playing = Some(Playing {
            emote,
            elapsed: 0.0,
            glance: DEFAULT_GLANCE,
            toward: None,
        });
    }

    /// A uniform value in `[0, 1)` from a xorshift generator.
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// The rendered transform: position plus bob, heading plus wobble.
    #[must_use]
    pub fn transform(&self) -> Mat4 {
        let t = self.time;
        let bob = (t * 1.9).sin() * 0.07 + (t * 0.73).sin() * 0.04;
        let lean = self.vel.length().min(6.0) * 0.05;
        let pose = self.pose();
        let wobble = Quat::from_rotation_y(self.yaw + (t * 0.61).sin() * 0.35 + pose.yaw)
            * Quat::from_rotation_x((t * 1.37).sin() * 0.12 + lean + pose.pitch)
            * Quat::from_rotation_z((t * 0.97).sin() * 0.10 + pose.roll);
        Mat4::from_rotation_translation(wobble, self.pos + Vec3::Y * (bob + pose.lift))
    }

    /// The agent's geometry for this frame: the spade and its ground ring.
    #[must_use]
    pub fn mesh(&self) -> Mesh {
        self.mesh_at(Intensity::Full)
    }

    /// The agent's geometry with its front edge at `bright`. A replay's
    /// ghost is one step down the ladder from the player's agent.
    #[must_use]
    pub fn mesh_at(&self, bright: Intensity) -> Mesh {
        let mut mesh = spade(self.transform(), bright);
        let ground = Vec3::new(self.pos.x, 0.02, self.pos.z);
        let pulse = 0.28 + (self.time * 1.9).sin() * 0.03;
        mesh.ring(ground, pulse, 24, Intensity::Quarter);
        mesh
    }
}

/// Where the agent wants to be at time `t`: behind the player's right
/// shoulder, drifting slowly so the distance is never exact.
fn target(player: &PlayerController, t: f32) -> Vec3 {
    let fwd = player.forward();
    let right = fwd.cross(Vec3::Y);
    player.pos + right * OFFSET.x - fwd * OFFSET.y + Vec3::Y * HOVER + wander(t)
}

/// The slow drift of the point the agent chases at time `t`.
fn wander(t: f32) -> Vec3 {
    Vec3::new(
        (t * 0.37).sin() * 0.35 + (t * 0.83).sin() * 0.12,
        (t * 0.53).sin() * 0.15,
        (t * 0.29).cos() * 0.30,
    )
}

/// The spade's outline in the unit plane: tip up, stem down, centered on
/// the origin, one unit tall. The body is an inverted heart curve; the stem
/// is a flared foot joined to it where the curve dips.
fn outline() -> (Vec<Vec2>, Vec<Vec2>, [Vec2; 4]) {
    let heart = |t: f32| {
        let x = 16.0 * t.sin().powi(3);
        let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
        Vec2::new(x, -y)
    };
    let scale = |p: Vec2| (p - Vec2::new(0.0, -3.0)) / 36.0;
    let segments = 72;
    let body: Vec<Vec2> = (0..segments)
        .map(|i| scale(heart(i as f32 / segments as f32 * std::f32::consts::TAU)))
        .collect();

    // Leave the dip where the stem joins out of the drawn outline.
    let join = 0.42_f32;
    let arc: Vec<Vec2> = (0..=64)
        .map(|i| {
            let t = join + (std::f32::consts::TAU - 2.0 * join) * i as f32 / 64.0;
            scale(heart(t))
        })
        .collect();
    let (a, b) = (arc[0], arc[arc.len() - 1]);
    let foot = -0.5;
    let stem = [a, Vec2::new(0.16, foot), Vec2::new(-0.16, foot), b];
    (body, arc, stem)
}

/// Builds the spade under `transform`, its front edge at `bright`: near-black faces front, back, and
/// sides, a bright front and back outline, and quieter edges joining them.
pub fn spade(transform: Mat4, bright: Intensity) -> Mesh {
    let mut mesh = Mesh::default();
    let (body, arc, stem) = outline();
    let at = |p: Vec2, z: f32| transform.transform_point3(Vec3::new(p.x, p.y, z) * SIZE);
    let center = Vec2::new(0.0, 0.0);

    for z in [DEPTH / SIZE, -DEPTH / SIZE] {
        for i in 0..body.len() {
            let (p, q) = (body[i], body[(i + 1) % body.len()]);
            mesh.quad([at(center, z), at(p, z), at(q, z), at(q, z)]);
        }
        mesh.quad(stem.map(|p| at(p, z)));
    }

    let mut rim: Vec<Vec2> = arc;
    rim.extend_from_slice(&[stem[2], stem[1]]);
    let (front, back) = (DEPTH / SIZE, -DEPTH / SIZE);
    for i in 0..rim.len() {
        let (p, q) = (rim[i], rim[(i + 1) % rim.len()]);
        mesh.quad([at(p, front), at(q, front), at(q, back), at(p, back)]);
        mesh.line(at(p, front), at(q, front), bright);
        mesh.line(at(p, back), at(q, back), dim(bright));
        if i % 6 == 0 {
            mesh.line(at(p, front), at(p, back), dim(dim(bright)));
        }
    }

    // A small core line down the face, like a highlight.
    mesh.line(
        at(Vec2::new(0.0, 0.34), front * 1.01),
        at(Vec2::new(0.0, 0.02), front * 1.01),
        dim(bright),
    );
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::InputState;
    use crate::palette;

    #[test]
    fn the_agent_starts_beside_the_player() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let agent = Agent::new(&pc);
        let d = agent.pos - pc.pos;
        assert!((d.y - HOVER).abs() < 0.3);
        assert!(Vec2::new(d.x, d.z).length() > 1.0);
        assert!(d.z < 0.0, "behind a +Z-facing player");
    }

    #[test]
    fn the_agent_lags_then_catches_up() {
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        let run = InputState {
            forward: true,
            ..Default::default()
        };
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            pc.update(&run, dt, &[], 1000.0);
            agent.update(&pc, dt);
        }
        let lag = agent.pos.distance(target(&pc, agent.time));
        assert!(lag > 0.3, "a spring trails a moving player, lag {lag}");
        for _ in 0..300 {
            agent.update(&pc, dt);
        }
        let rest = agent.pos.distance(target(&pc, agent.time));
        assert!(
            rest < 0.2,
            "and settles once the player stops, off by {rest}"
        );
    }

    #[test]
    fn catching_up_after_a_run_earns_a_look_around() {
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        let run = InputState {
            forward: true,
            ..Default::default()
        };
        let dt = 1.0 / 60.0;
        for _ in 0..90 {
            pc.update(&run, dt, &[], 1000.0);
            agent.update(&pc, dt);
            assert_eq!(agent.emote(), None, "no emote mid-chase");
        }
        let mut seen = false;
        let mut scans = 0;
        for _ in 0..240 {
            pc.update(&InputState::default(), dt, &[], 1000.0);
            agent.update(&pc, dt);
            if agent.take_scan() {
                scans += 1;
                agent.look_around(&[agent.pos + Vec3::new(5.0, 0.0, 5.0)]);
            }
            seen |= agent.emote() == Some(Emote::LookAround);
        }
        assert_eq!(scans, 1, "one scan per catch-up");
        assert!(seen, "the agent looks around once it catches up");
    }

    #[test]
    fn a_visiting_agent_flies_to_its_goal_and_faces_it() {
        let mut agent = Agent::at(Vec3::new(0.0, HOVER, 0.0), 0.0);
        let goal = Vec3::new(20.0, HOVER, 0.0);
        for _ in 0..(60 * 6) {
            agent.visit(goal, 1.0 / 60.0);
        }
        assert!(agent.pos.distance(goal) < 0.8, "at {:?}", agent.pos);
        assert_eq!(agent.emote(), None, "no idle emotes on a visit");
        let mut agent = Agent::at(Vec3::new(0.0, HOVER, 0.0), 0.0);
        for _ in 0..20 {
            agent.visit(goal, 1.0 / 60.0);
        }
        assert!(agent.yaw > 0.3, "turns toward +X, yaw {}", agent.yaw);
    }

    #[test]
    fn the_ghost_is_one_step_down_the_ladder() {
        let agent = Agent::at(Vec3::ZERO, 0.0);
        let full = palette::amber(Intensity::Full);
        let three = palette::amber(Intensity::ThreeQuarters);
        assert!(agent.mesh().lines.iter().any(|v| v.color == full));
        let ghost = agent.mesh_at(Intensity::ThreeQuarters);
        assert!(!ghost.lines.iter().any(|v| v.color == full));
        assert!(ghost.lines.iter().any(|v| v.color == three));
    }

    #[test]
    fn a_short_step_earns_no_look_around() {
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        let run = InputState {
            forward: true,
            ..Default::default()
        };
        let dt = 1.0 / 60.0;
        for _ in 0..12 {
            pc.update(&run, dt, &[], 1000.0);
            agent.update(&pc, dt);
        }
        for _ in 0..180 {
            pc.update(&InputState::default(), dt, &[], 1000.0);
            agent.update(&pc, dt);
            assert_ne!(agent.emote(), Some(Emote::LookAround));
        }
    }

    #[test]
    fn an_idle_agent_plays_every_idle_emote() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        let mut seen = Vec::new();
        for _ in 0..(60 * 240) {
            agent.update(&pc, 1.0 / 60.0);
            if let Some(e) = agent.emote()
                && !seen.contains(&e)
            {
                seen.push(e);
            }
        }
        for e in Emote::IDLE {
            assert!(seen.contains(&e), "{e:?} never played");
        }
    }

    #[test]
    fn every_emote_starts_and_ends_at_rest() {
        use std::f32::consts::TAU;
        for e in [
            Emote::LookAround,
            Emote::Spin,
            Emote::LookUpDown,
            Emote::BarrelRoll,
            Emote::Greet,
        ] {
            for u in [0.0, 1.0] {
                let p = e.pose(u);
                let turn = |a: f32| (a.rem_euclid(TAU)).min(TAU - a.rem_euclid(TAU));
                assert!(turn(p.yaw) < 1e-4 && turn(p.pitch) < 1e-4 && turn(p.roll) < 1e-4);
                assert!(p.lift.abs() < 1e-4, "{e:?} at {u}");
            }
        }
    }

    #[test]
    fn a_greeting_faces_the_other_agent_and_bows() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        // Heading +Z: a friend due +X is a quarter turn to the left.
        let friend = agent.pos + Vec3::new(5.0, 0.0, 0.0);
        assert!(agent.greet(friend));
        for _ in 0..30 {
            agent.update(&pc, 1.0 / 60.0);
        }
        let pose = agent.pose();
        assert!(
            (pose.yaw - std::f32::consts::FRAC_PI_2).abs() < 0.15,
            "yaw {}",
            pose.yaw
        );
        let bow = Emote::Greet.pose(0.35).pitch;
        assert!(bow > 0.4, "bows forward");
        assert!(!agent.greet(friend), "one greeting at a time");
    }

    #[test]
    fn a_scanning_agent_does_not_greet() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        agent.scanning = true;
        assert!(!agent.greet(Vec3::X));
    }

    #[test]
    fn the_look_around_faces_what_the_scan_found() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let mut agent = Agent::new(&pc);
        // Heading +Z: +X is to the agent's left.
        let left = agent.pos + Vec3::new(10.0, 0.0, 10.0);
        let right = agent.pos + Vec3::new(-10.0, 0.0, 3.0);
        agent.look_around(&[right, left]);
        let playing = agent.playing.expect("playing");
        assert!((playing.glance[0] - std::f32::consts::FRAC_PI_4).abs() < 0.05);
        assert!(playing.glance[1] < -1.0);
    }

    #[test]
    fn the_look_around_goes_left_then_right() {
        let left = Emote::LookAround.pose(0.3).yaw;
        let right = Emote::LookAround.pose(0.7).yaw;
        assert!(left > 0.8 && right < -0.8);
    }

    #[test]
    fn the_spade_is_its_size_and_amber() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let agent = Agent::new(&pc);
        let mesh = spade(Mat4::IDENTITY, Intensity::Full);
        let ys: Vec<f32> = mesh.lines.iter().map(|v| v.pos[1]).collect();
        let height = ys.iter().copied().fold(f32::MIN, f32::max)
            - ys.iter().copied().fold(f32::MAX, f32::min);
        assert!((height - SIZE).abs() < SIZE * 0.12, "height {height}");
        let mut allowed: Vec<[f32; 3]> =
            Intensity::ALL.iter().map(|&s| palette::amber(s)).collect();
        allowed.push(palette::field());
        for v in agent.mesh().lines.iter().chain(&agent.mesh().faces) {
            assert!(allowed.contains(&v.color));
        }
    }
}
