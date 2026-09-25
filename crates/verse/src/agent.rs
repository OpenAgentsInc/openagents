//! The player's agent: a floating 3D spade that follows the player.
//!
//! The agent hangs in the air near the player's shoulder. It chases a
//! target point on a slightly underdamped spring, so it trails behind when
//! the player moves and overshoots a little when the player stops. The
//! target wanders, and the body bobs and wobbles on incommensurate
//! frequencies, so the motion never settles into an exact repeat.

use coder_terminal::Intensity;
use glam::{Mat4, Quat, Vec2, Vec3};

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

/// The agent's motion state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agent {
    /// Center of the spade in world space.
    pub pos: Vec3,
    vel: Vec3,
    /// Heading in radians, the same convention as the player's yaw.
    pub yaw: f32,
    time: f32,
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
        }
    }

    /// Advances the agent by `dt` seconds toward its place beside `player`.
    pub fn update(&mut self, player: &PlayerController, dt: f32) {
        self.time += dt;
        let goal = target(player, self.time);
        let damping = 2.0 * STIFFNESS.sqrt() * DAMPING;
        let accel = (goal - self.pos) * STIFFNESS - self.vel * damping;
        self.vel += accel * dt;
        self.pos += self.vel * dt;

        // Turn toward the player's heading, lazily.
        let delta = crate::controller::wrap(player.yaw - self.yaw);
        self.yaw = crate::controller::wrap(self.yaw + delta * (1.0 - 0.08f32.powf(dt)));
    }

    /// The rendered transform: position plus bob, heading plus wobble.
    #[must_use]
    pub fn transform(&self) -> Mat4 {
        let t = self.time;
        let bob = (t * 1.9).sin() * 0.07 + (t * 0.73).sin() * 0.04;
        let lean = self.vel.length().min(6.0) * 0.05;
        let wobble = Quat::from_rotation_y(self.yaw + (t * 0.61).sin() * 0.35)
            * Quat::from_rotation_x((t * 1.37).sin() * 0.12 + lean)
            * Quat::from_rotation_z((t * 0.97).sin() * 0.10);
        Mat4::from_rotation_translation(wobble, self.pos + Vec3::Y * bob)
    }

    /// The agent's geometry for this frame: the spade and its ground ring.
    #[must_use]
    pub fn mesh(&self) -> Mesh {
        let mut mesh = spade(self.transform());
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
    let wander = Vec3::new(
        (t * 0.37).sin() * 0.35 + (t * 0.83).sin() * 0.12,
        (t * 0.53).sin() * 0.15,
        (t * 0.29).cos() * 0.30,
    );
    player.pos + right * OFFSET.x - fwd * OFFSET.y + Vec3::Y * HOVER + wander
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

/// Builds the spade under `transform`: near-black faces front, back, and
/// sides, a bright front and back outline, and quieter edges joining them.
fn spade(transform: Mat4) -> Mesh {
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
        mesh.line(at(p, front), at(q, front), Intensity::Full);
        mesh.line(at(p, back), at(q, back), Intensity::ThreeQuarters);
        if i % 6 == 0 {
            mesh.line(at(p, front), at(p, back), Intensity::Half);
        }
    }

    // A small core line down the face, like a highlight.
    mesh.line(
        at(Vec2::new(0.0, 0.34), front * 1.01),
        at(Vec2::new(0.0, 0.02), front * 1.01),
        Intensity::ThreeQuarters,
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
    fn the_spade_is_its_size_and_amber() {
        let pc = PlayerController::new(Vec3::ZERO, 0.0);
        let agent = Agent::new(&pc);
        let mesh = spade(Mat4::IDENTITY);
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
