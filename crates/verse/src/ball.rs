//! The bare world's ball: one large rubber ball on the shared
//! [`physics`] crate, which the player pushes by walking into it.
//!
//! The ground is a static box, the player a kinematic capsule that follows
//! the shared [`PlayerController`], and the ball a dynamic sphere. Contacts,
//! friction, restitution, and island sleep are the physics crate's own. The
//! crate has no rolling resistance or air, so this module adds both as the
//! bare world's rules: a rolling-resistance moment from the measured ground
//! reaction, and quadratic drag. The ball therefore slides, spins up, rolls,
//! and comes to rest on its own.
//!
//! Steps are fixed at [`DT`] with at most [`MAX_STEPS`] per frame, as in
//! Lagrange 1, and the ball is drawn between its last two poses. The ball is
//! drawn with physical materials under a studio [`Key`] light: a lacquered
//! octant pattern, so its rotation reads. Nothing is drawn on the floor
//! under it: the grid's lines alone ground it.

use std::sync::OnceLock;
use std::time::Duration;

use glam::{DQuat, DVec3, Mat4, Quat, Vec3};
use physics::{Body, BodyId, BodyKind, Collider, FixedStep, Material, Shape, Uniform, World};

use crate::controller::{PlayerController, RADIUS as PLAYER_RADIUS};
use crate::mesh::Mesh;
use crate::pbr::{Key, LitVertex, Material as Surface};

/// Fixed step length, s: Lagrange 1's `PHYSICS_DT`.
pub const DT: f64 = 1.0 / 120.0;
/// Most steps one frame runs (0.1 s), as in Lagrange 1.
pub const MAX_STEPS: u32 = 12;
/// Standard gravity, m/s².
pub const G: f64 = 9.81;

/// Ball radius, m: a 2.4 m ball, taller than the player.
pub const RADIUS: f64 = 1.2;
/// Ball mass, kg: a thick rubber shell.
pub const MASS: f64 = 40.0;
/// Rubber on a hard floor.
pub const FRICTION: f64 = 0.8;
/// A heavy rubber ball returns a third of its closing speed.
pub const RESTITUTION: f64 = 0.35;
/// Spin about the contact normal meets a small torsional limit, m.
pub const TORSIONAL: f64 = 0.02;
/// Rolling-resistance coefficient: the resisting moment is this times the
/// ground reaction times the radius. A soft ball on a hard floor.
pub const ROLLING: f64 = 0.1;
/// Drag coefficient of a sphere at these Reynolds numbers.
pub const DRAG: f64 = 0.47;
/// Air density, kg/m³.
pub const AIR: f64 = 1.2;
/// How far ahead of the player the ball starts, m.
pub const AHEAD: f64 = 7.0;
/// Where the ball starts in a new world: resting on the ground [`AHEAD`] of
/// the spawn, which faces +Z.
pub const START: DVec3 = DVec3::new(
    crate::world::SPAWN.x as f64,
    RADIUS,
    crate::world::SPAWN.z as f64 + AHEAD,
);

/// The player's capsule: the controller's radius, 1.8 m tall.
const PLAYER_HEIGHT: f64 = 1.8;
/// The fastest the player body is carried, m/s; a larger step is a teleport.
const PLAYER_MAX_SPEED: f64 = 20.0;
/// How far around the ball its shadow region reaches, m.
const POOL: f32 = 6.0;
/// The largest half extent of one shadow region over the ball and the
/// blocks, m; beyond it the shadow follows the ball alone.
const SHARED_SHADOW: f32 = 24.0;

/// The bare world's ball and the physics world it rolls in, which also
/// holds the stack of cubes and the dominoes ([`crate::blocks`]).
pub struct Ball {
    world: World,
    clock: FixedStep,
    ball: BodyId,
    blocks: crate::blocks::Blocks,
    player: BodyId,
    ground: BodyId,
    /// Where the ball rests when reset.
    start: DVec3,
    /// Wall-clock time of the last frame's steps.
    pub step_time: Duration,
    /// Steps the last frame ran.
    pub steps: u32,
}

impl Default for Ball {
    fn default() -> Self {
        Self::new()
    }
}

impl Ball {
    /// The ball at rest at [`START`], the ground, the world's edges, and a
    /// player capsule at the spawn.
    #[must_use]
    pub fn new() -> Self {
        let mut world = World::new(DT);
        let half = f64::from(crate::world::HALF);
        let fixed = |world: &mut World, pos: DVec3, half: DVec3| {
            let id = world.add(Body::new(1.0, DVec3::ONE, pos).with_kind(BodyKind::Static));
            world.add_collider(
                Collider::new(id, Shape::Cuboid { half }).with_material(Material {
                    friction: 0.8,
                    torsional: TORSIONAL,
                    restitution: 0.0,
                }),
            );
            id
        };
        let ground = fixed(
            &mut world,
            DVec3::new(0.0, -1.0, 0.0),
            DVec3::new(half, 1.0, half),
        );
        // Invisible walls at the grid's edge keep the ball in the world.
        for (pos, extent) in [
            (DVec3::new(half + 1.0, 5.0, 0.0), DVec3::new(1.0, 5.0, half)),
            (
                DVec3::new(-half - 1.0, 5.0, 0.0),
                DVec3::new(1.0, 5.0, half),
            ),
            (DVec3::new(0.0, 5.0, half + 1.0), DVec3::new(half, 5.0, 1.0)),
            (
                DVec3::new(0.0, 5.0, -half - 1.0),
                DVec3::new(half, 5.0, 1.0),
            ),
        ] {
            fixed(&mut world, pos, extent);
        }
        // A thin spherical shell: I = 2/3 m r².
        let ball = world.add(Body::new(
            MASS,
            DVec3::splat(2.0 / 3.0 * MASS * RADIUS * RADIUS),
            START,
        ));
        world.add_collider(
            Collider::new(ball, Shape::Sphere { radius: RADIUS }).with_material(Material {
                friction: FRICTION,
                torsional: TORSIONAL,
                restitution: RESTITUTION,
            }),
        );
        let blocks = crate::blocks::Blocks::new(
            &mut world,
            &crate::blocks::Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z),
        );
        let spawn = capsule_center(crate::world::SPAWN);
        let player = world.add(Body::new(1.0, DVec3::ONE, spawn).with_kind(BodyKind::Kinematic));
        let r = f64::from(PLAYER_RADIUS);
        world.add_collider(
            Collider::new(
                player,
                Shape::Capsule {
                    radius: r,
                    half_length: PLAYER_HEIGHT / 2.0 - r,
                },
            )
            // The capsule's axis is its collider z; stand it up.
            .at(
                DVec3::ZERO,
                DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
            )
            .with_material(Material {
                friction: 0.3,
                torsional: 0.0,
                restitution: 0.0,
            }),
        );
        Self {
            world,
            clock: FixedStep::new(DT, MAX_STEPS),
            ball,
            blocks,
            player,
            ground,
            start: START,
            step_time: Duration::ZERO,
            steps: 0,
        }
    }

    /// The ball's body.
    #[must_use]
    pub fn body(&self) -> &Body {
        &self.world[self.ball]
    }

    #[cfg(test)]
    pub(crate) fn ball_id(&self) -> BodyId {
        self.ball
    }

    #[cfg(test)]
    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// The stack of cubes and the dominoes.
    #[must_use]
    pub fn blocks(&self) -> &crate::blocks::Blocks {
        &self.blocks
    }

    /// The physics world, for inspection.
    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The ball's pose between its last two steps, for drawing.
    #[must_use]
    pub fn pose(&self) -> (Vec3, Quat) {
        let (pos, orientation) = self.body().interpolated(self.clock.alpha());
        (pos.as_vec3(), orientation.as_quat())
    }

    /// Advances the ball by `dt` seconds of frame time. The player moved from
    /// feet position `from` to `player.pos` this frame; its capsule sweeps
    /// that path and pushes the ball, and the player is then kept out of the
    /// ball, which is heavier than a step can walk through.
    pub fn advance(&mut self, from: Vec3, player: &mut PlayerController, dt: f32) {
        let started = std::time::Instant::now();
        let dt = f64::from(dt);
        let start = capsule_center(from);
        let end = capsule_center(player.pos);
        let velocity = if dt > 0.0 {
            (end - start) / dt
        } else {
            DVec3::ZERO
        };
        if velocity.is_finite() && velocity.length() <= PLAYER_MAX_SPEED {
            let body = &mut self.world[self.player];
            body.pos = start;
            body.prev_pos = start;
            body.vel = velocity;
        } else {
            // A spawn or a zone change moves the player without walking:
            // place it clear of the ball, so nothing is struck.
            self.keep_out(player);
            let end = capsule_center(player.pos);
            let body = &mut self.world[self.player];
            body.pos = end;
            body.prev_pos = end;
            body.vel = DVec3::ZERO;
        }
        self.steps = self.clock.advance(dt);
        let gravity = Uniform(DVec3::new(0.0, -G, 0.0));
        for _ in 0..self.steps {
            self.apply_resistance();
            self.world.step(&gravity);
        }
        self.world[self.player].vel = DVec3::ZERO;
        let body = self.world[self.ball];
        if !body.pos.is_finite() || body.pos.y < -RADIUS {
            self.reset();
        }
        self.blocks.recover(&mut self.world);
        self.keep_out(player);
        self.step_time = started.elapsed();
    }

    /// Puts the ball back at rest where it started.
    pub fn reset(&mut self) {
        let start = self.start;
        self.place(start);
    }

    /// Puts the ball at rest in front of a player standing at `feet` and
    /// facing `yaw`, as far ahead as [`START`] is from the world's spawn.
    /// Later resets return it there.
    pub fn place_ahead(&mut self, feet: Vec3, yaw: f32) {
        let half = f64::from(crate::world::HALF) - RADIUS - 1.0;
        let feet_at = feet.as_dvec3();
        let forward = crate::controller::forward(yaw).as_dvec3();
        let clamp = |p: DVec3| DVec3::new(p.x.clamp(-half, half), RADIUS, p.z.clamp(-half, half));
        let mut start = clamp(feet_at + forward * AHEAD);
        // Facing the world's edge, the ball goes behind instead.
        let clear = RADIUS + f64::from(PLAYER_RADIUS) + 1.0;
        if DVec3::new(start.x - feet_at.x, 0.0, start.z - feet_at.z).length() < clear {
            start = clamp(feet_at - forward * AHEAD);
        }
        self.start = start;
        self.place(start);
        // The blocks stand beyond the ball, in the direction it was placed.
        let layout = crate::blocks::Layout::new(feet_at, start - feet_at);
        let layout = crate::blocks::Layout {
            origin: layout.origin + inside(&layout),
            ..layout
        };
        self.blocks.place(&mut self.world, &layout);
        let player = capsule_center(feet);
        let body = &mut self.world[self.player];
        body.pos = player;
        body.prev_pos = player;
        body.vel = DVec3::ZERO;
    }

    fn place(&mut self, start: DVec3) {
        let body = &mut self.world[self.ball];
        body.pos = start;
        body.prev_pos = start;
        body.vel = DVec3::ZERO;
        body.omega = DVec3::ZERO;
        body.orientation = DQuat::IDENTITY;
        body.prev_orientation = DQuat::IDENTITY;
        body.wake();
    }

    /// Rolling resistance and air drag for the next step. The rolling
    /// moment opposes the ball's spin about horizontal axes and is bounded
    /// by the ground reaction the last step measured, and by the spin
    /// itself, so it stops a ball without turning it backward.
    fn apply_resistance(&mut self) {
        let dt = self.world.dt;
        let ball = self.ball;
        let ground = self.ground;
        let reaction: f64 = self
            .world
            .contacts
            .iter()
            .filter(|c| {
                (c.body_a == ball && c.body_b == ground) || (c.body_a == ground && c.body_b == ball)
            })
            .map(|c| c.impulse.dot(c.normal).abs() / dt)
            .sum();
        let body = &mut self.world[ball];
        if body.sleeping {
            return;
        }
        let spin = body.omega_world();
        let rolling = DVec3::new(spin.x, 0.0, spin.z);
        let rate = rolling.length();
        if reaction > 0.0 && rate > 0.0 {
            let inertia = body.inertia.x;
            let moment = (ROLLING * reaction * RADIUS).min(inertia * rate / dt);
            body.apply_torque(-rolling / rate * moment);
        }
        let speed = body.vel.length();
        if speed > 0.0 {
            let area = std::f64::consts::PI * RADIUS * RADIUS;
            body.apply_force(-body.vel * (0.5 * AIR * DRAG * area * speed));
        }
    }

    /// Moves the player out of the ball along the ground.
    fn keep_out(&self, player: &mut PlayerController) {
        let center = self.body().pos;
        let feet = player.pos.as_dvec3();
        let r = f64::from(PLAYER_RADIUS);
        let low = feet.y + r;
        let high = feet.y + PLAYER_HEIGHT - r;
        let dy = center.y - center.y.clamp(low, high);
        let reach = RADIUS + r;
        if dy.abs() >= reach {
            return;
        }
        let clear = (reach * reach - dy * dy).sqrt();
        let away = DVec3::new(feet.x - center.x, 0.0, feet.z - center.z);
        let distance = away.length();
        if distance >= clear {
            return;
        }
        let direction = if distance > 1e-6 {
            away / distance
        } else {
            -player.forward().as_dvec3()
        };
        let out = center + direction * clear;
        player.pos.x = out.x as f32;
        player.pos.z = out.z as f32;
    }

    /// Adds the ball and the blocks, and the studio light that shades them,
    /// to `mesh`. No floor is drawn under them.
    pub fn draw(&self, mesh: &mut Mesh) {
        let (pos, orientation) = self.pose();
        let transform = Mat4::from_rotation_translation(orientation, pos);
        mesh.lit.extend(
            sphere()
                .iter()
                .map(|v| place(v, &transform, &Mat4::from_quat(orientation))),
        );
        self.blocks
            .draw(&self.world, self.clock.alpha(), &mut mesh.lit);
        if let Some(neon) = &mut mesh.neon {
            let mut light = key(Vec3::new(pos.x, RADIUS as f32 * 0.5, pos.z));
            // One shadow region over the ball and the blocks while they
            // share the stage; a ball rolled far away keeps its own.
            let (mut low, mut high) = (pos - Vec3::splat(POOL), pos + Vec3::splat(POOL));
            for (center, radius) in [self.blocks.stack_pool(), self.blocks.domino_pool()] {
                low = low.min(center - Vec3::splat(radius));
                high = high.max(center + Vec3::splat(radius));
            }
            let half = ((high - low) * Vec3::new(1.0, 0.0, 1.0)).max_element() / 2.0 + 1.0;
            if half <= SHARED_SHADOW {
                light.shadow_center =
                    Vec3::new((low.x + high.x) / 2.0, 1.0, (low.z + high.z) / 2.0);
                light.shadow_half = half;
            }
            neon.key = Some(light);
        }
    }
}

/// The studio light around the ball at `center`: a warm-white key high on
/// the spawn side, a cooler rim behind, and a dim sky.
#[must_use]
pub fn key(center: Vec3) -> Key {
    Key {
        dir: Vec3::new(0.7, 0.62, -0.35).normalize(),
        illuminance: 4_200.0,
        angular_radius: 0.035,
        rim_dir: Vec3::new(-0.35, 0.3, 0.9).normalize(),
        rim_illuminance: 2_600.0,
        rim_angular_radius: 0.12,
        sky: 260.0,
        ground: 40.0,
        ev100: 10.0,
        shadow_center: center,
        shadow_half: POOL + 1.0,
    }
}

/// The shift that keeps the blocks laid out in `layout` inside the world's
/// walls.
fn inside(layout: &crate::blocks::Layout) -> DVec3 {
    let limit = f64::from(crate::world::HALF) - 4.0;
    let mut low = DVec3::splat(f64::MAX);
    let mut high = DVec3::splat(f64::MIN);
    for (side, ahead) in [(-8.0, 12.0), (-8.0, 24.0), (12.0, 12.0), (12.0, 24.0)] {
        let p = layout.at(side, ahead, 0.0);
        low = low.min(p);
        high = high.max(p);
    }
    let shift = |low: f64, high: f64| {
        if low < -limit {
            -limit - low
        } else if high > limit {
            limit - high
        } else {
            0.0
        }
    };
    DVec3::new(shift(low.x, high.x), 0.0, shift(low.z, high.z))
}

/// The capsule's center for a player whose feet are at `feet`.
fn capsule_center(feet: Vec3) -> DVec3 {
    feet.as_dvec3() + DVec3::Y * (PLAYER_HEIGHT / 2.0)
}

fn place(v: &LitVertex, t: &Mat4, rotation: &Mat4) -> LitVertex {
    LitVertex {
        pos: t.transform_point3(Vec3::from(v.pos)).to_array(),
        normal: rotation.transform_vector3(Vec3::from(v.normal)).to_array(),
        tangent: rotation.transform_vector3(Vec3::from(v.tangent)).to_array(),
        ..*v
    }
}

/// The ball in object space, built once: an octahedron's eight faces
/// subdivided and pushed onto the sphere, alternating white and charcoal
/// lacquer by octant, so any rotation shows.
fn sphere() -> &'static [LitVertex] {
    static SPHERE: OnceLock<Vec<LitVertex>> = OnceLock::new();
    SPHERE.get_or_init(|| {
        const N: i32 = 12;
        let radius = RADIUS as f32;
        let (_, metallic, roughness) = Surface::Lacquer.parameters();
        let code = Surface::Lacquer.code();
        let mut out = Vec::with_capacity(8 * (N * N) as usize * 3);
        for octant in 0..8 {
            let sx = if octant & 1 == 0 { 1.0 } else { -1.0 };
            let sy = if octant & 2 == 0 { 1.0 } else { -1.0 };
            let sz = if octant & 4 == 0 { 1.0 } else { -1.0 };
            let (a, b, c) = (Vec3::X * sx, Vec3::Y * sy, Vec3::Z * sz);
            let color = if sx * sy * sz > 0.0 {
                [0.82, 0.82, 0.80]
            } else {
                [0.035, 0.035, 0.04]
            };
            let point = |i: i32, j: i32| {
                let k = N - i - j;
                (a * i as f32 + b * j as f32 + c * k as f32).normalize()
            };
            let vertex = |n: Vec3| {
                let tangent = Vec3::Y.cross(n).try_normalize().unwrap_or(Vec3::X);
                LitVertex {
                    pos: (n * radius).to_array(),
                    normal: n.to_array(),
                    tangent: tangent.to_array(),
                    local: (n * radius).to_array(),
                    color,
                    params: [metallic, roughness, code, 1.0],
                }
            };
            for i in 0..N {
                for j in 0..N - i {
                    out.extend([point(i, j), point(i + 1, j), point(i, j + 1)].map(vertex));
                    if i + j + 1 < N {
                        out.extend(
                            [point(i + 1, j), point(i + 1, j + 1), point(i, j + 1)].map(vertex),
                        );
                    }
                }
            }
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::InputState;

    const FRAME: f32 = 1.0 / 60.0;

    fn walk(ball: &mut Ball, player: &mut PlayerController, frames: usize) {
        let input = InputState {
            forward: true,
            ..InputState::default()
        };
        for _ in 0..frames {
            let from = player.pos;
            player.update(&input, FRAME, &[], crate::world::HALF);
            ball.advance(from, player, FRAME);
        }
    }

    fn idle(ball: &mut Ball, player: &mut PlayerController, frames: usize) {
        for _ in 0..frames {
            let from = player.pos;
            player.update(&InputState::default(), FRAME, &[], crate::world::HALF);
            ball.advance(from, player, FRAME);
        }
    }

    #[test]
    fn the_ball_rests_on_the_ground_until_touched() {
        let mut ball = Ball::new();
        let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
        idle(&mut ball, &mut player, 120);
        let body = ball.body();
        assert!((body.pos - START).length() < 0.01, "{:?}", body.pos);
        assert!(body.sleeping, "a ball at rest sleeps");
    }

    #[test]
    fn walking_into_the_ball_pushes_it_rolling_and_it_comes_to_rest() {
        let mut ball = Ball::new();
        let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
        // Walk straight at the ball for two seconds.
        walk(&mut ball, &mut player, 120);
        let pushed = *ball.body();
        assert!(
            pushed.pos.z > START.z + 1.0,
            "the ball moved ahead: {:?}",
            pushed.pos
        );
        assert!(pushed.pos.x.abs() < 0.05);
        // The player never walked through the ball.
        assert!(f64::from(player.pos.z) < pushed.pos.z);
        // It rolls: spin about the axis across its motion, v = ω × r.
        let mut rolling = false;
        let mut max_height: f64 = 0.0;
        let mut min_height = f64::MAX;
        let mut distance = 0.0;
        let mut last = pushed.pos;
        let mut rest = None;
        for frame in 0..(60 * 30) {
            idle(&mut ball, &mut player, 1);
            let body = ball.body();
            let spin = body.omega_world();
            if body.vel.z > 0.5 && spin.x > 0.0 {
                let slip = body.vel.z - spin.x * RADIUS;
                rolling |= slip.abs() < 0.05 * body.vel.z;
            }
            max_height = max_height.max(body.pos.y);
            min_height = min_height.min(body.pos.y);
            distance += (body.pos - last).length();
            last = body.pos;
            if body.sleeping && rest.is_none() {
                rest = Some(frame);
            }
        }
        assert!(rolling, "the ball rolls without slipping");
        // It stays on the ground: no hop, no sinking.
        assert!(max_height < RADIUS + 0.05, "{max_height}");
        assert!(min_height > RADIUS - 0.05, "{min_height}");
        assert!(distance > 2.0, "{distance}");
        // From a 6.4 m/s push it rolls about 22 m and sleeps within 15 s.
        assert!(rest.is_some_and(|frame| frame < 60 * 20), "{rest:?}");
        let body = ball.body();
        assert!(body.sleeping, "{:?} {:?}", body.vel, body.omega);
        assert!(body.vel.length() < 0.01);
        assert!((body.pos.y - RADIUS).abs() < 0.02);
        // Rest is a function of the push alone: it rolled a sensible way.
        assert!(body.pos.z - START.z < 40.0, "{:?}", body.pos);
    }

    #[test]
    fn a_push_from_the_side_rolls_it_sideways() {
        let mut ball = Ball::new();
        // Stand west of the ball, facing east (+X is yaw π/2).
        let side = Vec3::new(START.x as f32 - 6.0, 0.0, START.z as f32);
        let mut player = PlayerController::new(side, std::f32::consts::FRAC_PI_2);
        walk(&mut ball, &mut player, 90);
        let body = ball.body();
        assert!(body.pos.x > START.x + 0.5, "{:?}", body.pos);
        assert!((body.pos.z - START.z).abs() < 0.1);
        idle(&mut ball, &mut player, 30);
        // Rolling toward +X spins it about -Z.
        assert!(ball.body().omega_world().z < 0.0);
    }

    #[test]
    fn a_teleport_does_not_fling_the_ball() {
        let mut ball = Ball::new();
        let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
        let from = player.pos;
        // Land right beside the ball in one frame.
        player.pos = Vec3::new(START.x as f32 + 1.4, 0.0, START.z as f32);
        ball.advance(from, &mut player, FRAME);
        idle(&mut ball, &mut player, 60);
        assert!(ball.body().vel.length() < 0.5, "{:?}", ball.body().vel);
        // The player was moved out of the ball.
        let offset = player.pos - ball.body().pos.as_vec3();
        assert!(offset.x.hypot(offset.z) >= (RADIUS as f32 + PLAYER_RADIUS) * 0.99);
    }

    #[test]
    fn a_restored_spawn_finds_the_ball_ahead() {
        let mut ball = Ball::new();
        let feet = Vec3::new(19.6, 0.0, 40.9);
        ball.place_ahead(feet, std::f32::consts::FRAC_PI_2);
        let body = *ball.body();
        assert!((body.pos - DVec3::new(26.6, RADIUS, 40.9)).length() < 1e-3);
        assert!(body.vel == DVec3::ZERO && body.omega == DVec3::ZERO);
        // Near the world's edge it stays inside the walls.
        let edge = crate::world::HALF - 1.0;
        ball.place_ahead(Vec3::new(edge, 0.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!(ball.body().pos.x < f64::from(crate::world::HALF) - RADIUS);
        let mut player = PlayerController::new(Vec3::new(edge, 0.0, 0.0), 0.0);
        idle(&mut ball, &mut player, 60);
        assert!(ball.body().vel.length() < 0.1);
    }

    #[test]
    fn the_step_stays_within_lagrange_budget() {
        let mut ball = Ball::new();
        let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
        walk(&mut ball, &mut player, 120);
        let mut worst = Duration::ZERO;
        let mut total = Duration::ZERO;
        let mut steps = 0;
        for _ in 0..240 {
            let from = player.pos;
            player.update(&InputState::default(), FRAME, &[], crate::world::HALF);
            ball.advance(from, &mut player, FRAME);
            worst = worst.max(ball.world().stats.total);
            total += ball.world().stats.total * ball.steps;
            steps += ball.steps;
        }
        eprintln!(
            "ball step: mean {:?}, worst {:?} over {steps} steps",
            total / steps.max(1),
            worst
        );
        // Lagrange 1 runs up to 12 steps a frame; one step here is far less
        // than a millisecond even in a debug build.
        assert!(worst < Duration::from_millis(5));
    }

    #[test]
    fn the_ball_draws_lit_with_a_studio_key() {
        let ball = Ball::new();
        let mut mesh = Mesh {
            neon: Some(crate::pbr::Neon::neutral(0.0)),
            ..Mesh::default()
        };
        ball.draw(&mut mesh);
        assert!(!mesh.lit.is_empty());
        assert!(mesh.lit.iter().all(|v| v.pos.iter().all(|x| x.is_finite())));
        // Both lacquers appear, and every ball vertex lies on the sphere.
        let ball_vertices = sphere().len();
        let (white, dark): (Vec<&LitVertex>, Vec<&LitVertex>) = mesh.lit[..ball_vertices]
            .iter()
            .partition(|v| v.color[0] > 0.5);
        assert!(!white.is_empty() && !dark.is_empty());
        for v in &mesh.lit[..ball_vertices] {
            let d = (Vec3::from(v.pos) - START.as_vec3()).length();
            assert!((d - RADIUS as f32).abs() < 1e-3);
        }
        // No floor is drawn under the ball or the blocks: no stage disc,
        // and nothing below the ground.
        assert!(
            mesh.lit
                .iter()
                .all(|v| v.params[2] != Surface::Stage.code())
        );
        assert!(mesh.lit.iter().all(|v| v.pos[1] > -1e-3));
        let key = mesh.neon.and_then(|n| n.key).expect("a studio key");
        assert!(key.dir.y > 0.5 && key.illuminance > 0.0);
        // The shadow region covers the ball and the blocks beside it.
        let reach = (key.shadow_center - START.as_vec3()) * Vec3::new(1.0, 0.0, 1.0);
        assert!(reach.abs().max_element() + RADIUS as f32 <= key.shadow_half);
        let (stack, stack_r) = ball.blocks().stack_pool();
        let stack_reach = (key.shadow_center - stack) * Vec3::new(1.0, 0.0, 1.0);
        assert!(stack_reach.abs().max_element() + stack_r <= key.shadow_half + 1e-3);
    }
}
