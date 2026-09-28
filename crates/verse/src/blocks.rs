//! The bare world's blocks: a stack of lacquered cubes and an arc of
//! dominoes, in the ball's physics world, which the player and the ball
//! knock over.
//!
//! Every block is a dynamic box on the shared [`physics`] crate: oriented
//! box contacts with the ground and with each other, gravity, friction,
//! restitution, and island sleep are the crate's own, stepped at the ball's
//! fixed rate. A stack placed at rest sleeps at once and stands until
//! something touches it; the player's kinematic capsule and the ball wake it.
//!
//! The blocks are laid out in the frame the ball was placed in: the spawn's
//! forward axis and the side axis to its right as seen from above, so a new
//! world shows the ball straight ahead, the stack beyond it to one side, and
//! the dominoes beyond it to the other. Like the ball they are local to the
//! device and are not networked.

use std::sync::OnceLock;

use glam::{DQuat, DVec3, Mat4, Quat, Vec3};
use physics::{Body, BodyId, Collider, Material, Shape, World};

use crate::pbr::{LitVertex, Material as Surface};

/// Edge of one cube, m.
pub const CUBE: f64 = 0.8;
/// Mass of one cube, kg: a hollow lacquered box, light enough for the ball
/// to knock out of the stack.
pub const CUBE_MASS: f64 = 5.0;
/// The stack's columns across, deep, and its layers.
pub const STACK: [usize; 3] = [2, 2, 4];
/// Horizontal gap between neighbouring columns, m, so resting columns do
/// not rub.
const GAP: f64 = 0.01;
/// The stack's center in the layout frame: side, forward, m.
pub const STACK_AT: [f64; 2] = [-5.0, 18.0];

/// Domino size: thickness along the row, height, width across the row, m.
pub const DOMINO: DVec3 = DVec3::new(0.22, 1.5, 0.8);
/// Mass of one domino, kg.
pub const DOMINO_MASS: f64 = 8.0;
/// How many dominoes stand in the arc.
pub const DOMINOES: usize = 10;
/// Distance between neighbouring dominoes along the arc, m: under half
/// their height, so each falling domino reaches the next.
pub const SPACING: f64 = 0.7;
/// Radius of the domino arc, m.
const ARC: f64 = 4.0;
/// Where the arc starts in the layout frame, heading forward and curving
/// toward the side axis: side, forward, m.
pub const DOMINOES_AT: [f64; 2] = [2.5, 16.0];

/// Lacquered wood on wood.
const FRICTION: f64 = 0.5;
/// Hard blocks return little of their closing speed.
const RESTITUTION: f64 = 0.1;
/// Spin about a contact normal meets a small torsional limit, m.
const TORSIONAL: f64 = 0.01;

/// White and charcoal lacquer, as on the ball.
const WHITE: [f32; 3] = [0.82, 0.82, 0.80];
const CHARCOAL: [f32; 3] = [0.035, 0.035, 0.04];

/// A block's resting pose, where it returns when reset.
#[derive(Clone, Copy, Debug)]
struct Home {
    pos: DVec3,
    orientation: DQuat,
}

/// The stack of cubes and the arc of dominoes.
pub struct Blocks {
    cubes: Vec<BodyId>,
    dominoes: Vec<BodyId>,
    homes: Vec<Home>,
}

/// A flat frame on the ground: an origin and unit forward and side axes.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub origin: DVec3,
    pub forward: DVec3,
    pub side: DVec3,
}

impl Layout {
    /// The frame of a player standing at `origin` facing along `forward`.
    #[must_use]
    pub fn new(origin: DVec3, forward: DVec3) -> Self {
        let forward = DVec3::new(forward.x, 0.0, forward.z)
            .try_normalize()
            .unwrap_or(DVec3::Z);
        Self {
            origin: DVec3::new(origin.x, 0.0, origin.z),
            forward,
            side: DVec3::new(forward.z, 0.0, -forward.x),
        }
    }

    /// The world point `side` right and `ahead` forward of the origin, at
    /// height `y`.
    #[must_use]
    pub fn at(&self, side: f64, ahead: f64, y: f64) -> DVec3 {
        self.origin + self.side * side + self.forward * ahead + DVec3::Y * y
    }

    /// The rotation that takes local +Z to `heading` (a direction in the
    /// layout's side/forward plane) about the vertical.
    fn yaw(&self, heading: [f64; 2]) -> DQuat {
        let dir = self.side * heading[0] + self.forward * heading[1];
        DQuat::from_rotation_y(dir.x.atan2(dir.z))
    }
}

fn material() -> Material {
    Material {
        friction: FRICTION,
        torsional: TORSIONAL,
        restitution: RESTITUTION,
    }
}

/// The stack's resting poses in `layout`: columns across and deep, layers up,
/// bottom layer first.
fn stack_homes(layout: &Layout) -> Vec<Home> {
    let [across, deep, layers] = STACK;
    let pitch = CUBE + GAP;
    let orientation = layout.yaw([0.0, 1.0]);
    let mut out = Vec::with_capacity(across * deep * layers);
    for layer in 0..layers {
        for i in 0..across {
            for j in 0..deep {
                let side = STACK_AT[0] + (i as f64 - (across as f64 - 1.0) / 2.0) * pitch;
                let ahead = STACK_AT[1] + (j as f64 - (deep as f64 - 1.0) / 2.0) * pitch;
                let y = CUBE / 2.0 + CUBE * layer as f64;
                out.push(Home {
                    pos: layout.at(side, ahead, y),
                    orientation,
                });
            }
        }
    }
    out
}

/// The dominoes' resting poses in `layout`, from the first, which faces the
/// spawn, to the last.
fn domino_homes(layout: &Layout) -> Vec<Home> {
    // The arc starts heading forward and turns toward the side axis about a
    // center `ARC` to the side of its start.
    let center = [DOMINOES_AT[0] + ARC, DOMINOES_AT[1]];
    (0..DOMINOES)
        .map(|n| {
            let theta = SPACING * n as f64 / ARC;
            let side = center[0] - ARC * theta.cos();
            let ahead = center[1] + ARC * theta.sin();
            // Tangent: its thickness lies along the row.
            let heading = [theta.sin(), theta.cos()];
            Home {
                pos: layout.at(side, ahead, DOMINO.y / 2.0),
                orientation: layout.yaw(heading),
            }
        })
        .collect()
}

/// The domino's half extents in its body frame: thickness along local +Z,
/// height along +Y, width along +X.
fn domino_half() -> DVec3 {
    DVec3::new(DOMINO.z, DOMINO.y, DOMINO.x) / 2.0
}

impl Blocks {
    /// The stack and the dominoes at rest in `layout`, added to `world`.
    pub fn new(world: &mut World, layout: &Layout) -> Self {
        let cube_half = DVec3::splat(CUBE / 2.0);
        let mut homes = stack_homes(layout);
        let cubes = homes
            .iter()
            .map(|home| add_box(world, CUBE_MASS, cube_half, home.pos, home.orientation))
            .collect();
        let domino_homes = domino_homes(layout);
        let dominoes = domino_homes
            .iter()
            .map(|home| {
                add_box(
                    world,
                    DOMINO_MASS,
                    domino_half(),
                    home.pos,
                    home.orientation,
                )
            })
            .collect();
        homes.extend(domino_homes);
        Self {
            cubes,
            dominoes,
            homes,
        }
    }

    /// The cubes, bottom layer first.
    #[must_use]
    pub fn cubes(&self) -> &[BodyId] {
        &self.cubes
    }

    /// The dominoes, from the first in the row to the last.
    #[must_use]
    pub fn dominoes(&self) -> &[BodyId] {
        &self.dominoes
    }

    fn bodies(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.cubes.iter().chain(&self.dominoes).copied()
    }

    /// Where the stack stands and how far its shadow region reaches.
    #[must_use]
    pub fn stack_pool(&self) -> (Vec3, f32) {
        pool_around(&self.homes[..self.cubes.len()], 3.0)
    }

    /// Where the dominoes stand and how far their shadow region reaches.
    #[must_use]
    pub fn domino_pool(&self) -> (Vec3, f32) {
        pool_around(&self.homes[self.cubes.len()..], 1.6)
    }

    /// Stands every block at rest again in `layout`, which later resets
    /// return to.
    pub fn place(&mut self, world: &mut World, layout: &Layout) {
        let mut homes = stack_homes(layout);
        homes.extend(domino_homes(layout));
        self.homes = homes;
        let ids: Vec<BodyId> = self.bodies().collect();
        for (id, home) in ids.into_iter().zip(self.homes.clone()) {
            rest(&mut world[id], home);
        }
    }

    /// Returns any block that left the world, or whose state is no longer
    /// finite, to its home.
    pub fn recover(&self, world: &mut World) {
        for (id, home) in self.bodies().zip(&self.homes) {
            let body = &world[id];
            let lost = !body.pos.is_finite()
                || !body.vel.is_finite()
                || !body.orientation.is_finite()
                || body.pos.y < -1.0;
            if lost {
                rest(&mut world[id], *home);
            }
        }
    }

    /// The blocks between their last two steps, lit and lacquered.
    pub fn draw(&self, world: &World, alpha: f64, out: &mut Vec<LitVertex>) {
        for (n, id) in self.cubes.iter().enumerate() {
            let [across, deep, _] = STACK;
            let (i, j, layer) = (n % deep, (n / deep) % across, n / (across * deep));
            let color = if (i + j + layer) % 2 == 0 {
                WHITE
            } else {
                CHARCOAL
            };
            draw_body(&world[*id], alpha, cube(), color, out);
        }
        for id in &self.dominoes {
            draw_body(&world[*id], alpha, domino(), WHITE, out);
        }
    }
}

/// A dynamic box added to `world` at rest.
fn add_box(world: &mut World, mass: f64, half: DVec3, pos: DVec3, orientation: DQuat) -> BodyId {
    let mut body = Body::new(mass, Body::box_inertia(mass, half * 2.0), pos);
    body.orientation = orientation;
    body.prev_orientation = orientation;
    let id = world.add(body);
    world.add_collider(Collider::new(id, Shape::Cuboid { half }).with_material(material()));
    id
}

fn rest(body: &mut Body, home: Home) {
    body.pos = home.pos;
    body.prev_pos = home.pos;
    body.orientation = home.orientation;
    body.prev_orientation = home.orientation;
    body.vel = DVec3::ZERO;
    body.omega = DVec3::ZERO;
    body.wake();
}

/// The center of `homes` on the floor and a radius that reaches `margin`
/// past the farthest.
fn pool_around(homes: &[Home], margin: f64) -> (Vec3, f32) {
    let sum: DVec3 = homes.iter().map(|h| h.pos).sum();
    let center = sum / homes.len().max(1) as f64;
    let center = DVec3::new(center.x, 0.0, center.z);
    let reach = homes
        .iter()
        .map(|h| DVec3::new(h.pos.x - center.x, 0.0, h.pos.z - center.z).length())
        .fold(0.0, f64::max);
    (center.as_vec3(), (reach + margin) as f32)
}

/// One solid in object space: a triangle list with a flag per vertex for
/// the accent color.
struct Solid {
    vertices: Vec<LitVertex>,
    accent: Vec<bool>,
}

fn draw_body(body: &Body, alpha: f64, solid: &Solid, color: [f32; 3], out: &mut Vec<LitVertex>) {
    let (pos, orientation) = body.interpolated(alpha);
    let (pos, orientation): (Vec3, Quat) = (pos.as_vec3(), orientation.as_quat());
    let transform = Mat4::from_rotation_translation(orientation, pos);
    let rotation = Mat4::from_quat(orientation);
    out.extend(solid.vertices.iter().zip(&solid.accent).map(|(v, accent)| {
        let accent_color = if color == WHITE { CHARCOAL } else { WHITE };
        LitVertex {
            pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
            normal: rotation.transform_vector3(Vec3::from(v.normal)).to_array(),
            tangent: rotation.transform_vector3(Vec3::from(v.tangent)).to_array(),
            color: if *accent { accent_color } else { color },
            ..*v
        }
    }));
}

/// Adds the six faces of a box with half extents `half` centered at
/// `center` to `out`, each flat-shaded.
fn push_box(out: &mut Solid, center: Vec3, half: Vec3, accent: bool) {
    let (_, metallic, roughness) = Surface::Lacquer.parameters();
    let code = Surface::Lacquer.code();
    for axis in 0..3 {
        for sign in [1.0_f32, -1.0] {
            let mut n = Vec3::ZERO;
            n[axis] = sign;
            let u_axis = (axis + 1) % 3;
            let v_axis = (axis + 2) % 3;
            let mut u = Vec3::ZERO;
            u[u_axis] = half[u_axis];
            let mut v = Vec3::ZERO;
            v[v_axis] = half[v_axis];
            // Counter-clockwise seen from outside.
            if sign < 0.0 {
                std::mem::swap(&mut u, &mut v);
            }
            let face = center + n * half[axis];
            let corners = [face - u - v, face + u - v, face + u + v, face - u + v];
            let vertex = |p: Vec3| LitVertex {
                pos: p.to_array(),
                normal: n.to_array(),
                tangent: u.normalize().to_array(),
                local: p.to_array(),
                color: WHITE,
                params: [metallic, roughness, code, 1.0],
            };
            for k in [0, 1, 2, 0, 2, 3] {
                out.vertices.push(vertex(corners[k]));
                out.accent.push(accent);
            }
        }
    }
}

/// A cube in object space, built once.
fn cube() -> &'static Solid {
    static CUBE_SOLID: OnceLock<Solid> = OnceLock::new();
    CUBE_SOLID.get_or_init(|| {
        let mut solid = Solid {
            vertices: Vec::new(),
            accent: Vec::new(),
        };
        push_box(
            &mut solid,
            Vec3::ZERO,
            Vec3::splat(CUBE as f32 / 2.0),
            false,
        );
        solid
    })
}

/// A domino in object space, built once: a white slab with a charcoal bar
/// across the middle of each broad face.
fn domino() -> &'static Solid {
    static DOMINO_SOLID: OnceLock<Solid> = OnceLock::new();
    DOMINO_SOLID.get_or_init(|| {
        let half = domino_half().as_vec3();
        let mut solid = Solid {
            vertices: Vec::new(),
            accent: Vec::new(),
        };
        push_box(&mut solid, Vec3::ZERO, half, false);
        // The bar stands just proud of the faces, so it never fights them.
        let bar = Vec3::new(half.x * 0.8, 0.025, half.z + 0.003);
        push_box(&mut solid, Vec3::ZERO, bar, true);
        solid
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ball::Ball;
    use crate::controller::{InputState, PlayerController};

    const FRAME: f32 = 1.0 / 60.0;

    fn run(ball: &mut Ball, player: &mut PlayerController, frames: usize, forward: bool) {
        let input = InputState {
            forward,
            ..InputState::default()
        };
        for _ in 0..frames {
            let from = player.pos;
            player.update(&input, FRAME, &[], crate::world::HALF);
            ball.advance(from, player, FRAME);
        }
    }

    /// How far a body's up axis leans from vertical, radians.
    fn tilt(body: &Body) -> f64 {
        (body.orientation * DVec3::Y)
            .dot(DVec3::Y)
            .clamp(-1.0, 1.0)
            .acos()
    }

    fn poses(ball: &Ball, ids: &[BodyId]) -> Vec<(DVec3, DQuat)> {
        ids.iter()
            .map(|id| (ball.world()[*id].pos, ball.world()[*id].orientation))
            .collect()
    }

    /// Bodies whose top came down by at least `drop`, m, or that lean past
    /// `lean` radians.
    fn disturbed(ball: &Ball, ids: &[BodyId], homes: &[(DVec3, DQuat)], drop: f64) -> usize {
        ids.iter()
            .zip(homes)
            .filter(|(id, (pos, _))| pos.y - ball.world()[**id].pos.y > drop)
            .count()
    }

    #[test]
    fn the_stack_and_the_dominoes_stand_still_until_disturbed() {
        let mut ball = Ball::new();
        let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
        let cubes = ball.blocks().cubes().to_vec();
        let dominoes = ball.blocks().dominoes().to_vec();
        let start = poses(&ball, &cubes);
        let row = poses(&ball, &dominoes);
        // Eight seconds untouched.
        run(&mut ball, &mut player, 60 * 8, false);
        for (id, (pos, orientation)) in cubes.iter().zip(&start).chain(dominoes.iter().zip(&row)) {
            let body = &ball.world()[*id];
            assert!(
                (body.pos - *pos).length() < 0.005,
                "{:?} {:?}",
                body.pos,
                pos
            );
            assert!(body.orientation.angle_between(*orientation) < 0.005);
            assert!(body.sleeping, "a block at rest sleeps");
        }
        // Asleep, the demos cost nothing: only the ball world's awake bodies
        // step.
        assert_eq!(ball.world().stats.awake, 0);
    }

    #[test]
    fn walking_into_the_stack_topples_it_and_it_comes_to_rest() {
        let mut ball = Ball::new();
        let layout = Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z);
        let front = layout.at(STACK_AT[0], STACK_AT[1] - 4.0, 0.0).as_vec3();
        let mut player = PlayerController::new(front, 0.0);
        let cubes = ball.blocks().cubes().to_vec();
        let start = poses(&ball, &cubes);
        // Walk into the stack for three seconds, then watch it fall.
        run(&mut ball, &mut player, 180, true);
        let mut max_speed: f64 = 0.0;
        for _ in 0..(60 * 4) {
            run(&mut ball, &mut player, 1, false);
            for id in &cubes {
                max_speed = max_speed.max(ball.world()[*id].vel.length());
            }
        }
        // The upper layers came down and tumbled.
        let fallen = disturbed(&ball, &cubes, &start, CUBE * 0.9);
        assert!(fallen >= 6, "{fallen} cubes came down");
        let tumbled = cubes
            .iter()
            .filter(|id| {
                let t = tilt(&ball.world()[**id]);
                t > 0.3 && (t - std::f64::consts::FRAC_PI_2).abs() > 0.05
                    || ball.world()[**id].orientation.angle_between(start[0].1) > 0.3
            })
            .count();
        assert!(tumbled >= 4, "{tumbled} cubes turned over");
        // Falling from 3.2 m, nothing flies off.
        assert!(max_speed < 12.0, "{max_speed}");
        // The rubble settles on the floor and sleeps.
        run(&mut ball, &mut player, 60 * 12, false);
        for id in &cubes {
            let body = &ball.world()[*id];
            assert!(body.pos.is_finite());
            assert!(body.pos.y > CUBE * 0.35, "no cube sinks: {:?}", body.pos);
            assert!(body.pos.y < CUBE * 2.6, "{:?}", body.pos);
            assert!(body.vel.length() < 0.05, "{:?}", body.vel);
        }
        let asleep = cubes
            .iter()
            .filter(|id| ball.world()[**id].sleeping)
            .count();
        assert!(asleep >= cubes.len() - 2, "{asleep} cubes asleep");
    }

    #[test]
    fn the_ball_knocks_the_stack_over() {
        let mut ball = Ball::new();
        let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
        let cubes = ball.blocks().cubes().to_vec();
        let start = poses(&ball, &cubes);
        // Roll the ball at the middle of the stack's face.
        let center = start.iter().map(|(p, _)| *p).sum::<DVec3>() / start.len() as f64;
        let id = ball.ball_id();
        let body = &mut ball.world_mut()[id];
        body.pos = DVec3::new(center.x, RADIUS, center.z - 5.0);
        body.prev_pos = body.pos;
        body.vel = DVec3::Z * 6.0;
        body.omega = DVec3::X * (6.0 / RADIUS);
        body.wake();
        run(&mut ball, &mut player, 60 * 5, false);
        let fallen = disturbed(&ball, &cubes, &start, CUBE * 0.9);
        assert!(fallen >= 4, "{fallen} cubes came down");
    }

    use crate::ball::RADIUS;

    #[test]
    fn pushing_the_first_domino_topples_the_row() {
        let mut ball = Ball::new();
        let layout = Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z);
        let behind = layout
            .at(DOMINOES_AT[0], DOMINOES_AT[1] - 2.5, 0.0)
            .as_vec3();
        let mut player = PlayerController::new(behind, 0.0);
        let dominoes = ball.blocks().dominoes().to_vec();
        // A short walk into the first domino; the player coasts to a stop
        // among the first few.
        run(&mut ball, &mut player, 30, true);
        run(&mut ball, &mut player, 60 * 6, false);
        let down: Vec<f64> = dominoes.iter().map(|id| tilt(&ball.world()[*id])).collect();
        // Each one fell onto the next: every domino leans well over, the
        // last one flat on the floor.
        assert!(down.iter().all(|t| *t > 0.5), "{down:?}");
        assert!(down[DOMINOES - 1] > 1.3, "{down:?}");
        // Beyond the player's reach they fell where they stood, along the
        // row, not scattered.
        let homes = domino_homes(&layout);
        for (id, home) in dominoes.iter().zip(&homes).skip(4) {
            let d = ball.world()[*id].pos - home.pos;
            assert!(DVec3::new(d.x, 0.0, d.z).length() < DOMINO.y, "{d:?}");
        }
        assert!(
            f64::from(player.pos.z) < homes[4].pos.z - 1.0,
            "{:?}",
            player.pos
        );
        // And the fallen row comes to rest.
        run(&mut ball, &mut player, 60 * 4, false);
        assert!(dominoes.iter().all(|id| ball.world()[*id].sleeping));
    }

    #[test]
    fn a_restored_spawn_lays_the_blocks_out_ahead() {
        let mut ball = Ball::new();
        let feet = Vec3::new(19.6, 0.0, 40.9);
        ball.place_ahead(feet, std::f32::consts::FRAC_PI_2);
        let (stack, _) = ball.blocks().stack_pool();
        let (dominoes, _) = ball.blocks().domino_pool();
        // Facing +X, both stand ahead of the ball.
        assert!(stack.x > 26.6 + 5.0 && dominoes.x > 26.6 + 5.0);
        let mut player = PlayerController::new(feet, std::f32::consts::FRAC_PI_2);
        run(&mut ball, &mut player, 120, false);
        assert_eq!(ball.world().stats.awake, 0);
        // At the world's edge they stay inside the walls.
        let edge = crate::world::HALF - 1.0;
        ball.place_ahead(Vec3::new(edge, 0.0, edge), 0.7);
        let limit = f64::from(crate::world::HALF);
        for id in ball.blocks().cubes().iter().chain(ball.blocks().dominoes()) {
            let p = ball.world()[*id].pos;
            assert!(p.x.abs() < limit - 1.0 && p.z.abs() < limit - 1.0, "{p:?}");
        }
    }

    #[test]
    fn a_toppled_stack_steps_within_budget() {
        let mut ball = Ball::new();
        let layout = Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z);
        let front = layout.at(STACK_AT[0], STACK_AT[1] - 4.0, 0.0).as_vec3();
        let mut player = PlayerController::new(front, 0.0);
        run(&mut ball, &mut player, 150, true);
        let mut worst = std::time::Duration::ZERO;
        let mut total = std::time::Duration::ZERO;
        let mut steps = 0;
        let mut awake = 0;
        for _ in 0..120 {
            run(&mut ball, &mut player, 1, false);
            worst = worst.max(ball.world().stats.total);
            total += ball.world().stats.total * ball.steps;
            steps += ball.steps;
            awake = awake.max(ball.world().stats.awake);
        }
        let mean = total / steps.max(1);
        eprintln!("toppled stack: {awake} awake, mean step {mean:?}, worst {worst:?}");
        assert!(awake >= 8, "{awake}");
        // The tumbling stack in a debug build, which runs many times slower
        // than the release build a phone runs, even beside other tests.
        assert!(mean < std::time::Duration::from_millis(6), "{mean:?}");
    }

    #[test]
    fn the_default_layout_keeps_the_demos_apart_and_ahead() {
        let layout = Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z);
        let stack = stack_homes(&layout);
        let dominoes = domino_homes(&layout);
        assert_eq!(stack.len(), 16);
        assert_eq!(dominoes.len(), DOMINOES);
        // Resting columns do not touch; layers sit on each other.
        for (a, ha) in stack.iter().enumerate() {
            for hb in &stack[a + 1..] {
                let d = hb.pos - ha.pos;
                let flat = d.x.abs().max(d.z.abs());
                assert!(flat >= CUBE || flat < 1e-9 && d.y.abs() >= CUBE - 1e-9);
            }
        }
        // Neighbouring dominoes stand closer than their height and apart.
        for pair in dominoes.windows(2) {
            let d = (pair[1].pos - pair[0].pos).length();
            assert!(d > DOMINO.x + 0.2 && d < DOMINO.y * 0.6, "{d}");
        }
        let ball = crate::ball::START;
        let (stack_center, stack_r) = pool_around(&stack, 3.0);
        let (domino_center, domino_r) = pool_around(&dominoes, 1.6);
        let flat = |p: Vec3| Vec3::new(p.x, 0.0, p.z);
        let ball_at = flat(ball.as_vec3());
        // The three demos' regions do not overlap.
        assert!(stack_center.distance(ball_at) > stack_r + 6.0);
        assert!(domino_center.distance(ball_at) > domino_r + 6.0);
        assert!(stack_center.distance(domino_center) > stack_r + domino_r);
        // Both lie beyond the ball, on either side of the spawn's line.
        assert!(stack_center.z > ball_at.z && domino_center.z > ball_at.z);
        assert!(stack_center.x < -2.0 && domino_center.x > 2.0);
    }
}
