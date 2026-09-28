//! Walk-in portals: arches the player enters by walking (or flying) through
//! the opening, with no button. The OpenAgents app's bare world ("the Grid")
//! has one, to Lagrange 1, and Lagrange 1 entered from it returns through
//! its own arch the same way. Coder's plaza keeps its tap-and-button arches.
//!
//! A gate is an arch at `at`, turned `yaw` about the vertical. Its local +Z
//! points through the opening, away from the side the player approaches:
//! the side the arch's lettering reads from.

use glam::{Quat, Vec3};

use super::ZoneId;
use crate::mesh::Mesh;

/// The Grid's portal in the ball's layout frame: to the side opposite the
/// dominoes and short of the stack, so the ball, the stack, and the
/// dominoes all stand clear of the arch and its approach. Side, forward, m.
pub const GRID_PORTAL_AT: [f64; 2] = [-9.0, 11.0];
/// Half the arch's clear opening between its pillars, m.
pub const OPENING_HALF: f32 = 1.5;
/// How far either side of the arch's plane counts as inside it, m.
const DEPTH: f32 = 0.6;
/// Feet heights relative to the arch's base that pass through the opening.
const FEET: [f32; 2] = [-1.0, 3.2];
/// Where the player stands after coming back through: in front of the arch,
/// facing away from it, m.
pub const RETURN_DISTANCE: f32 = 3.5;
/// Seconds after a crossing before a gate admits another.
pub const COOLDOWN: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub at: Vec3,
    pub yaw: f32,
}

impl Gate {
    /// The Grid's portal, placed in the frame the ball and blocks were laid
    /// out in and turned to face that frame's origin (the spawn), so it
    /// reads face-on from where the player starts.
    #[must_use]
    pub fn grid(layout: &crate::blocks::Layout) -> Self {
        let limit = f64::from(crate::world::HALF) - 6.0;
        let point = layout.at(GRID_PORTAL_AT[0], GRID_PORTAL_AT[1], 0.0);
        let point = glam::DVec3::new(
            point.x.clamp(-limit, limit),
            0.0,
            point.z.clamp(-limit, limit),
        );
        let away = (point - layout.origin).as_vec3();
        let away = Vec3::new(away.x, 0.0, away.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        Self {
            at: point.as_vec3(),
            yaw: away.x.atan2(away.z),
        }
    }

    /// A zone's axis-aligned return arch.
    #[must_use]
    pub fn fixed(at: Vec3) -> Self {
        Self { at, yaw: 0.0 }
    }

    fn rotation(self) -> Quat {
        Quat::from_rotation_y(self.yaw)
    }

    /// A world point in the arch's frame.
    #[must_use]
    pub fn local(self, point: Vec3) -> Vec3 {
        self.rotation().inverse() * (point - self.at)
    }

    /// Whether feet moving from `from` to `to` pass through the opening:
    /// inside its width and height, and either crossing its plane or
    /// standing within it.
    #[must_use]
    pub fn crossed(self, from: Vec3, to: Vec3) -> bool {
        let (a, b) = (self.local(from), self.local(to));
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let inside = |p: Vec3| p.x.abs() <= OPENING_HALF && (FEET[0]..=FEET[1]).contains(&p.y);
        if b.z.abs() <= DEPTH {
            return inside(b);
        }
        // A fast step can jump the band: check where it crossed the plane.
        if a.z.signum() != b.z.signum() {
            let t = a.z / (a.z - b.z);
            return inside(a.lerp(b, t));
        }
        false
    }

    /// Where the player stands after coming back through the arch, and the
    /// yaw that faces away from it.
    #[must_use]
    pub fn front(self) -> (Vec3, f32) {
        let back = self.rotation() * Vec3::NEG_Z;
        let pos = self.at + back * RETURN_DISTANCE;
        (Vec3::new(pos.x, self.at.y, pos.z), back.x.atan2(back.z))
    }

    /// The arch, lettered for `destination`, in the neutral palette.
    #[must_use]
    pub fn mesh(self, zone: ZoneId, sign: &str, elapsed: f32) -> Mesh {
        let mut mesh = Mesh::default();
        super::arch(&mut mesh, zone, sign, Vec3::ZERO, elapsed);
        mesh.neutralize();
        let transform = glam::Mat4::from_rotation_translation(self.rotation(), self.at);
        for vertex in mesh.lines.iter_mut().chain(mesh.faces.iter_mut()) {
            vertex.pos = transform
                .transform_point3(Vec3::from_array(vertex.pos))
                .to_array();
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn layout() -> crate::blocks::Layout {
        crate::blocks::Layout::new(crate::world::SPAWN.as_dvec3(), DVec3::Z)
    }

    #[test]
    fn the_opening_admits_feet_passing_through_and_nothing_beside_it() {
        let gate = Gate::grid(&layout());
        let world =
            |x: f32, y: f32, z: f32| gate.at + Quat::from_rotation_y(gate.yaw) * Vec3::new(x, y, z);
        assert!(gate.crossed(world(0.0, 0.0, -2.0), world(0.0, 0.0, -0.4)));
        assert!(gate.crossed(world(1.2, 0.0, -1.0), world(1.2, 0.0, 1.0)));
        // A long step over the plane still counts where it crossed.
        assert!(gate.crossed(world(0.0, 0.0, -3.0), world(0.5, 0.0, 3.0)));
        // Walking past the pillars, approaching, or leaping over does not.
        assert!(!gate.crossed(world(2.5, 0.0, -1.0), world(2.5, 0.0, 1.0)));
        assert!(!gate.crossed(world(0.0, 0.0, -3.0), world(0.0, 0.0, -1.0)));
        assert!(!gate.crossed(world(0.0, 4.0, -1.0), world(0.0, 4.0, 1.0)));
        assert!(!gate.crossed(world(0.0, 0.0, -1.0), Vec3::NAN));
    }

    #[test]
    fn the_return_stands_in_front_facing_away_and_outside_the_opening() {
        for gate in [
            Gate::grid(&layout()),
            Gate::fixed(Vec3::new(-5.0, 4.4, 22.5)),
        ] {
            let (pos, yaw) = gate.front();
            assert!(!gate.crossed(pos, pos));
            assert!((pos.distance(gate.at) - RETURN_DISTANCE).abs() < 1e-4);
            assert!(gate.local(pos).z < 0.0);
            // Walking forward from there leads away from the arch.
            let ahead = pos + crate::controller::forward(yaw) * 2.0;
            assert!(ahead.distance(gate.at) > pos.distance(gate.at));
        }
    }

    #[test]
    fn the_grid_portal_faces_the_spawn_clear_of_the_ball_and_blocks() {
        let layout = layout();
        let gate = Gate::grid(&layout);
        // The spawn is on the lettered side, and the arch faces it.
        let spawn = crate::world::SPAWN;
        assert!(gate.local(spawn).z < -8.0);
        assert!(gate.local(spawn).x.abs() < 0.01);
        // Every block and the ball, with the pools of light under them.
        let ball = crate::ball::Ball::new();
        let world = ball.world();
        let blocks = ball.blocks();
        let ball_at = ball.body().pos.as_vec3();
        assert!(ball_at.with_y(0.0).distance(gate.at) - crate::ball::RADIUS as f32 > 6.0);
        for &id in blocks.cubes().iter().chain(blocks.dominoes()) {
            let p = world[id].pos.as_vec3().with_y(0.0);
            assert!(
                p.distance(gate.at) > 5.0,
                "{p} is near the arch at {}",
                gate.at
            );
        }
        for (center, radius) in [blocks.stack_pool(), blocks.domino_pool()] {
            assert!(center.with_y(0.0).distance(gate.at) - radius > 1.0);
        }
        // Near the world's edge the arch stays inside the walls.
        let edge = crate::blocks::Layout::new(
            DVec3::new(f64::from(crate::world::HALF) - 1.0, 0.0, 0.0),
            DVec3::X,
        );
        let gate = Gate::grid(&edge);
        assert!(gate.at.x.abs() < crate::world::HALF - 5.0);
        assert!(gate.at.z.abs() < crate::world::HALF - 5.0);
    }

    #[test]
    fn the_arch_is_neutral() {
        let mesh = Gate::grid(&layout()).mesh(ZoneId::Plaza, "LAGRANGE 1", 0.0);
        assert!(!mesh.lines.is_empty());
        assert!(
            mesh.lines
                .iter()
                .chain(&mesh.faces)
                .all(|v| v.color[0] == v.color[1] && v.color[1] == v.color[2])
        );
    }
}
