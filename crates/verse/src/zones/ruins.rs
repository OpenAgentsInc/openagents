//! Wizard Woods rendering over the retained Atlantis simulation.

use super::assets::LoadedAssets;
use crate::{
    mesh::{Mesh, Vertex},
    world::World,
};
use glam::{Mat4, Vec3};
use verse_atlantis::{Simulation, Snapshot, Spell, scene::Terrain};

pub(crate) struct Forest {
    pub assets: LoadedAssets,
    pub simulation: Simulation,
    pub snapshot: Snapshot,
    particles: Vec<Particle>,
    rendered: Mesh,
    controller: verse_atlantis::Controller,
}
struct Particle {
    pos: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    color: [f32; 3],
}

impl Forest {
    pub fn new(assets: LoadedAssets) -> Result<Self, String> {
        let simulation = Simulation::new(Self::spawn().to_array())?;
        let snapshot = simulation.snapshot();
        let mut forest = Self {
            assets,
            simulation,
            snapshot,
            particles: Vec::new(),
            rendered: Mesh::default(),
            controller: verse_atlantis::Controller::new(Self::spawn()),
        };
        forest.rendered = forest.build_dynamic(&crate::controller::PlayerController::new(
            Self::spawn(),
            0.0,
        ));
        Ok(forest)
    }
    pub fn spawn() -> Vec3 {
        Vec3::new(0.0, Terrain::bundled().height(0.0, 0.0), 0.0)
    }
    pub fn world(&self) -> World {
        let mut world = World::default();
        let terrain = Terrain::bundled();
        for z in 0..terrain.size() - 1 {
            for x in 0..terrain.size() - 1 {
                let normal = Vec3::from(terrain.normal(x, z));
                let light =
                    0.45 + normal.dot(Vec3::new(-0.3, 0.8, 0.4).normalize()).max(0.0) * 0.55;
                quad(
                    &mut world.mesh,
                    [
                        terrain.vertex(x, z),
                        terrain.vertex(x + 1, z),
                        terrain.vertex(x + 1, z + 1),
                        terrain.vertex(x, z + 1),
                    ]
                    .map(Vec3::from),
                    [0.12 * light, 0.23 * light, 0.065 * light],
                );
            }
        }
        world
    }
    pub fn move_player(
        &mut self,
        player: &mut crate::controller::PlayerController,
        input: &crate::controller::InputState,
        dt: f32,
    ) {
        if self.snapshot.player.hp <= 0 {
            player.speed = 0.0;
            return;
        }
        let source = &mut self.controller;
        source.pos = player.pos;
        source.yaw = player.yaw;
        source.set_ground_height(Terrain::bundled().height(source.pos.x, source.pos.z));
        let before = source.pos;
        source.update(
            &verse_atlantis::MovementInput {
                forward: input.forward,
                backward: input.backward,
                strafe_left: input.strafe_left || (input.mouse_look && input.left),
                strafe_right: input.strafe_right || (input.mouse_look && input.right),
                turn_left: input.left,
                turn_right: input.right,
                mouse_look: input.mouse_look,
                jump_pressed: input.jump,
                run: input.sprint,
                ..Default::default()
            },
            dt,
            player.forward(),
        );
        source.pos.x = source.pos.x.clamp(-149.55, 149.55);
        source.pos.z = source.pos.z.clamp(-149.55, 149.55);
        source.set_ground_height(Terrain::bundled().height(source.pos.x, source.pos.z));
        player.pos = source.pos;
        player.yaw = source.yaw;
        player.speed = if dt > 0.0 {
            (source.pos.x - before.x).hypot(source.pos.z - before.z) / dt
        } else {
            0.0
        };
        player.set_surface_height(Terrain::bundled().height(source.pos.x, source.pos.z));
    }
    pub fn tick(
        &mut self,
        dt: f32,
        player: &crate::controller::PlayerController,
    ) -> Result<(), String> {
        self.simulation
            .tick(dt, player.pos.to_array(), player.yaw)?;
        let next = self.simulation.snapshot();
        // The original renderer triggers the fireball burst when its replicated
        // projectile disappears. Damage remains entirely in the source ECS.
        let explosions: Vec<_> = self
            .snapshot
            .projectiles
            .iter()
            .filter(|p| p.kind == Spell::Fireball && !next.projectiles.iter().any(|n| n.id == p.id))
            .map(|p| Vec3::from(p.pos))
            .collect();
        for at in explosions {
            self.burst(at, true);
        }
        for hit in &next.effects {
            self.burst(Vec3::from(hit.pos), false);
        }
        self.snapshot = next;
        for p in &mut self.particles {
            p.age += dt;
            p.pos += p.velocity * dt;
        }
        self.particles.retain(|p| p.age < p.life);
        self.rendered = self.build_dynamic(player);
        Ok(())
    }
    fn burst(&mut self, at: Vec3, fireball: bool) {
        // Keep the source fireball's 42 particles, speed, lifetime, and color.
        // A fixed sequence replaces renderer RNG; it cannot affect combat.
        let count = if fireball { 42 } else { 8 };
        for i in 0..count {
            if self.particles.len() >= 2048 {
                break;
            }
            let a = (i as f32 * 2.399_963_1).rem_euclid(std::f32::consts::TAU);
            let r = 6.0 + ((i * 7 % 19) as f32 / 19.0) * 2.0;
            self.particles.push(Particle {
                pos: at,
                velocity: Vec3::new(a.cos() * r, 3.0 + (i % 5) as f32 * 0.4, a.sin() * r),
                age: 0.0,
                life: 0.28,
                color: if fireball {
                    [2.2, 1.0, 0.3]
                } else {
                    [1.3, 0.7, 2.3]
                },
            });
        }
    }
    /// Reuse the frame geometry for rendering and depth picking.
    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    fn build_dynamic(&self, player: &crate::controller::PlayerController) -> Mesh {
        let mut mesh = Mesh::default();
        for chunk in self.simulation.ruins() {
            for &index in &chunk.indices {
                let i = index as usize;
                if let Some(&pos) = chunk.positions.get(i) {
                    let normal = chunk.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                    let light = 0.4
                        + Vec3::from(normal)
                            .dot(Vec3::new(-0.3, 0.8, 0.4).normalize())
                            .max(0.0)
                            * 0.6;
                    mesh.faces.push(Vertex {
                        pos,
                        color: [0.28 * light, 0.3 * light, 0.24 * light],
                        fog: 1.0,
                    });
                }
            }
        }
        let elapsed = self.snapshot.elapsed;
        if self.snapshot.player.hp > 0 {
            transformed(
                &mut mesh,
                if player.speed > 0.1 {
                    self.assets.wizard.sample(elapsed)
                } else {
                    &self.assets.wizard_still
                },
                Mat4::from_translation(player.pos) * Mat4::from_rotation_y(player.yaw),
            );
        }
        for actor in &self.snapshot.actors {
            if !actor.alive || actor.faction == "player" {
                continue;
            }
            let pos = Vec3::new(
                actor.pos[0],
                Terrain::bundled().height(actor.pos[0], actor.pos[2]),
                actor.pos[2],
            );
            let undead = actor.faction == "undead";
            let model = if undead {
                self.assets
                    .zombie_walk
                    .sample(elapsed + actor.id as f32 * 0.17)
            } else {
                self.assets.wizard.sample(elapsed + actor.id as f32 * 0.17)
            };
            let scale = if actor.kind == "boss" { 1.3 } else { 1.0 };
            transformed(
                &mut mesh,
                model,
                Mat4::from_translation(pos)
                    * Mat4::from_rotation_y(actor.yaw)
                    * Mat4::from_scale(Vec3::splat(scale)),
            );
            // The bar is world geometry attached to the moving actor.
            let y = pos.y + 2.15 * scale;
            let width = (actor.hp as f32 / actor.max_hp.max(1) as f32).clamp(0.0, 1.0) * 0.9;
            quad(
                &mut mesh,
                [
                    Vec3::new(pos.x - 0.45, y, pos.z),
                    Vec3::new(pos.x - 0.45 + width, y, pos.z),
                    Vec3::new(pos.x - 0.45 + width, y + 0.08, pos.z),
                    Vec3::new(pos.x - 0.45, y + 0.08, pos.z),
                ],
                if undead {
                    [0.8, 0.08, 0.02]
                } else {
                    [0.1, 0.7, 0.3]
                },
            );
        }
        for projectile in &self.snapshot.projectiles {
            let pos = Vec3::from(projectile.pos);
            let vel = Vec3::from(projectile.vel).normalize_or_zero();
            let (radius, color) = match projectile.kind {
                Spell::Firebolt => (0.16, [2.0, 0.7, 0.1]),
                Spell::Fireball => (0.34, [2.2, 0.7, 0.2]),
                Spell::MagicMissile => (0.18, [1.3, 0.7, 2.3]),
            };
            orb(&mut mesh, pos, radius, color);
            for i in 1..7 {
                orb(
                    &mut mesh,
                    pos - vel * i as f32 * 0.12,
                    radius * (1.0 - i as f32 / 8.0),
                    color.map(|c| c * (1.0 - i as f32 / 9.0)),
                );
            }
        }
        for p in &self.particles {
            orb(&mut mesh, p.pos, 0.05 * (1.0 - p.age / p.life), p.color);
        }
        mesh
    }
}
fn orb(mesh: &mut Mesh, pos: Vec3, radius: f32, color: [f32; 3]) {
    let points = [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z];
    for i in 0..4 {
        for pole in [Vec3::Y, -Vec3::Y] {
            for offset in [pole, points[i], points[(i + 1) % 4]] {
                mesh.faces.push(Vertex {
                    pos: (pos + offset * radius).to_array(),
                    color,
                    fog: 0.0,
                });
            }
        }
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
