//! The player's avatar: a boxy line figure that swings its limbs as it runs.

use coder_terminal::Intensity;
use glam::{Mat4, Quat, Vec3};

use crate::controller::PlayerController;
use crate::mesh::Mesh;

/// Leg-swing cycles per meter travelled.
const STRIDE: f32 = 0.55;

/// The avatar's walk cycle, advanced by distance rather than time so the
/// feet keep pace with the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gait {
    phase: f32,
    swing: f32,
}

impl Gait {
    /// Advances the cycle for a character moving at `speed` meters per
    /// second for `dt` seconds.
    pub fn advance(&mut self, speed: f32, airborne: bool, dt: f32) {
        self.phase =
            (self.phase + speed * dt * STRIDE * std::f32::consts::TAU) % std::f32::consts::TAU;
        let target = if speed > 0.1 && !airborne { 0.75 } else { 0.0 };
        self.swing += (target - self.swing) * (1.0 - 0.001f32.powf(dt));
    }
}

/// Builds the local player's avatar for this frame.
#[must_use]
pub fn mesh(pc: &PlayerController, gait: &Gait) -> Mesh {
    figure(pc.pos, Quat::from_rotation_y(pc.yaw), gait, Intensity::Full)
}

/// Builds an avatar at `pos` facing `rot`, its edges drawn at `bright`
/// and its ground marks one step dimmer.
#[must_use]
pub fn figure(pos: Vec3, rot: Quat, gait: &Gait, bright: Intensity) -> Mesh {
    let mut mesh = Mesh::default();
    let root = Mat4::from_rotation_translation(rot, pos);
    let swing = gait.phase.sin() * gait.swing;

    let limb = |pivot: Vec3, size: Vec3, angle: f32| {
        root * Mat4::from_translation(pivot)
            * Mat4::from_rotation_x(angle)
            * Mat4::from_translation(Vec3::new(0.0, -size.y / 2.0, 0.0))
            * Mat4::from_scale(size)
    };
    let part =
        |center: Vec3, size: Vec3| root * Mat4::from_translation(center) * Mat4::from_scale(size);

    let leg = Vec3::new(0.2, 0.92, 0.22);
    mesh.cube(limb(Vec3::new(0.14, 0.92, 0.0), leg, swing), bright);
    mesh.cube(limb(Vec3::new(-0.14, 0.92, 0.0), leg, -swing), bright);
    mesh.cube(
        part(Vec3::new(0.0, 1.27, 0.0), Vec3::new(0.54, 0.7, 0.3)),
        bright,
    );
    let arm = Vec3::new(0.15, 0.68, 0.17);
    mesh.cube(limb(Vec3::new(0.36, 1.58, 0.0), arm, -swing), bright);
    mesh.cube(limb(Vec3::new(-0.36, 1.58, 0.0), arm, swing), bright);
    mesh.cube(
        part(Vec3::new(0.0, 1.78, 0.0), Vec3::new(0.28, 0.3, 0.28)),
        bright,
    );

    let visor = |x: f32| root.transform_point3(Vec3::new(x, 1.8, 0.145));
    mesh.line(visor(-0.1), visor(0.1), bright);

    let ground = Vec3::new(pos.x, 0.02, pos.z);
    let ahead = rot * Vec3::Z;
    let ahead = Vec3::new(ahead.x, 0.0, ahead.z).normalize_or(Vec3::Z);
    mesh.ring(ground, 0.7, 32, dim(bright));
    mesh.line(ground + ahead * 0.7, ground + ahead * 1.1, dim(bright));
    mesh
}

/// One step dimmer on the amber ladder, bottoming out at a quarter.
#[must_use]
pub fn dim(step: Intensity) -> Intensity {
    match step {
        Intensity::Full => Intensity::ThreeQuarters,
        Intensity::ThreeQuarters => Intensity::Half,
        Intensity::Half | Intensity::Quarter => Intensity::Quarter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_avatar_stands_on_its_feet() {
        let pc = PlayerController::new(Vec3::new(3.0, 0.0, 4.0), 0.0);
        let mesh = mesh(&pc, &Gait::default());
        let low = mesh.lines.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
        let high = mesh.lines.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(low.abs() < 0.03, "feet at {low}");
        assert!((1.8..2.0).contains(&high), "head at {high}");
    }

    #[test]
    fn a_standing_gait_does_not_swing() {
        let mut gait = Gait::default();
        gait.advance(0.0, false, 1.0);
        assert_eq!(gait.swing, 0.0);
        gait.advance(6.0, false, 1.0);
        assert!(gait.swing > 0.7);
    }
}
