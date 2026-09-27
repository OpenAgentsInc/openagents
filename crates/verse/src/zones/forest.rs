//! The first mobile-sized rendition of Wizard Woods, using the original models.

use super::{assets::LoadedAssets, rules};
use crate::{
    controller::Footprint,
    mesh::{Mesh, Vertex},
    world::World,
};
use glam::{Mat4, Vec3};

pub(crate) struct Forest {
    pub assets: LoadedAssets,
    pub elapsed: f32,
    pub encounter: Option<rules::Encounter>,
    pub dice: rules::SeededDice,
    pub flash: f32,
}
impl Forest {
    pub fn new(assets: LoadedAssets) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self {
            assets,
            elapsed: 0.0,
            encounter: None,
            dice: rules::SeededDice::new(seed),
            flash: 0.0,
        }
    }
    pub fn world(&self) -> World {
        let mut world = World::default();
        // A flat arena is an explicit first physics profile, with a seeded
        // woodland around it rather than pretending to import terrain physics.
        for z in -16..16 {
            for x in -16..16 {
                let n = ((x * 71_i32 + z * 131).unsigned_abs() % 11) as f32 / 10.0;
                let color = [0.055 + n * 0.024, 0.10 + n * 0.035, 0.035 + n * 0.018];
                quad(
                    &mut world.mesh,
                    [
                        Vec3::new(x as f32 * 4.0, -0.02, z as f32 * 4.0),
                        Vec3::new(x as f32 * 4.0 + 4.0, -0.02, z as f32 * 4.0),
                        Vec3::new(x as f32 * 4.0 + 4.0, -0.02, z as f32 * 4.0 + 4.0),
                        Vec3::new(x as f32 * 4.0, -0.02, z as f32 * 4.0 + 4.0),
                    ],
                    color,
                );
            }
        }
        for ring in 0..3 {
            let count = 12 + ring * 4;
            for i in 0..count {
                let angle = (i as f32 + ring as f32 * 0.37) / count as f32 * std::f32::consts::TAU;
                let radius = 29.0 + ring as f32 * 12.0 + (i as f32 * 2.1).sin() * 2.0;
                let at = Vec3::new(angle.sin() * radius, 0.0, angle.cos() * radius);
                let scale = 1.0 + ((i * 7 + ring * 3) % 9) as f32 * 0.08;
                transformed(
                    &mut world.mesh,
                    &self.assets.tree,
                    Mat4::from_translation(at)
                        * Mat4::from_rotation_y(angle)
                        * Mat4::from_scale(Vec3::splat(scale)),
                );
                world.blockers.push(Footprint {
                    min: [at.x - 0.65, at.z - 0.65],
                    max: [at.x + 0.65, at.z + 0.65],
                });
            }
        }
        // A path and a stone-bordered encounter circle keep the entry readable.
        quad(
            &mut world.mesh,
            [
                Vec3::new(-1.6, 0.005, 21.0),
                Vec3::new(1.6, 0.005, 21.0),
                Vec3::new(2.4, 0.005, 2.0),
                Vec3::new(-2.4, 0.005, 2.0),
            ],
            [0.17, 0.125, 0.07],
        );
        for i in 0..64 {
            let a = i as f32 * std::f32::consts::TAU / 64.0;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 64.0;
            quad(
                &mut world.mesh,
                [
                    Vec3::new(a.cos() * 10.8, 0.01, a.sin() * 10.8),
                    Vec3::new(b.cos() * 10.8, 0.01, b.sin() * 10.8),
                    Vec3::new(b.cos() * 11.0, 0.01, b.sin() * 11.0),
                    Vec3::new(a.cos() * 11.0, 0.01, a.sin() * 11.0),
                ],
                [0.24, 0.27, 0.23],
            );
        }
        world
    }
    pub fn dynamic(&self, player: &crate::controller::PlayerController) -> Mesh {
        let mut mesh = Mesh::default();
        transformed(
            &mut mesh,
            self.assets.wizard.sample(self.elapsed),
            Mat4::from_translation(player.pos) * Mat4::from_rotation_y(player.yaw),
        );
        for (i, at) in [
            Vec3::new(-7.0, 0.0, 2.0),
            Vec3::new(7.0, 0.0, 2.0),
            Vec3::new(-7.0, 0.0, -7.0),
        ]
        .into_iter()
        .enumerate()
        {
            transformed(
                &mut mesh,
                self.assets.wizard.sample(self.elapsed + i as f32 * 0.7),
                Mat4::from_translation(at) * Mat4::from_rotation_y(std::f32::consts::PI),
            );
        }
        // Background zombies are observers, not a second unsynchronized combat engine.
        for (i, at) in [
            Vec3::new(-15.0, 0.0, -17.0),
            Vec3::new(13.0, 0.0, -18.0),
            Vec3::new(18.0, 0.0, 7.0),
        ]
        .into_iter()
        .enumerate()
        {
            transformed(
                &mut mesh,
                self.assets.zombie.sample(self.elapsed + i as f32),
                Mat4::from_translation(at),
            );
        }
        let (at, alive) = if let Some(encounter) = &self.encounter {
            let s = encounter.snapshot();
            (
                Vec3::new(s.zombie.position[0], 0.0, s.zombie.position[1]),
                s.zombie.hp > 0,
            )
        } else {
            (Vec3::new(0.0, 0.0, -7.62), true)
        };
        if alive {
            transformed(
                &mut mesh,
                self.assets.zombie.sample(self.elapsed),
                Mat4::from_translation(at),
            );
        }
        if self.flash > 0.0 {
            let start = player.pos + Vec3::Y * 1.1;
            let end = at + Vec3::Y;
            for offset in [-0.06, 0.0, 0.06] {
                for p in [start + Vec3::X * offset, end + Vec3::X * offset] {
                    mesh.lines.push(Vertex {
                        pos: p.to_array(),
                        color: [1.0, 0.3, 0.04],
                        fog: 1.0,
                    });
                }
            }
        }
        mesh
    }
}
pub(crate) fn transformed(target: &mut Mesh, source: &Mesh, transform: Mat4) {
    target.faces.extend(source.faces.iter().map(|v| Vertex {
        pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
        ..*v
    }));
    target.lines.extend(source.lines.iter().map(|v| Vertex {
        pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
        ..*v
    }));
}
fn quad(mesh: &mut Mesh, points: [Vec3; 4], color: [f32; 3]) {
    let [a, b, c, d] = points.map(|p| Vertex {
        pos: p.to_array(),
        color,
        fog: 1.0,
    });
    mesh.faces.extend([a, b, c, a, c, d]);
}
