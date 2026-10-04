//! Wizard Woods rendering over the retained Atlantis simulation.
//!
//! The zone draws on the physical path: the terrain, the ruins, and the
//! characters are lit triangles under a key light from the direction the
//! original renderer baked its shading from, on a neon stage whose field and
//! fog are the zone's dark green air. They take the sun's shadow cascades,
//! the ambient probes, and the shared output pass. Health bars, spell orbs,
//! and sparks stay display-colored faces, as the original drew them. The
//! simulation and combat do not read any of this.

use super::assets::LoadedAssets;
use crate::{
    mesh::{Mesh, Vertex},
    pbr::{Key, LitVertex, Material, Neon},
    world::World,
};
use glam::{Mat4, Vec3};
use verse_ruins::{Simulation, Snapshot, Spell, scene::Terrain};

/// Toward the light the original renderer baked into the terrain's colors.
const SUN: Vec3 = Vec3::new(-0.3, 0.8, 0.4);
/// The terrain's albedo: the original's moss green.
const GROUND: [f32; 3] = [0.12, 0.23, 0.065];
/// The ruins' albedo: the original's weathered stone.
const STONE: [f32; 3] = [0.28, 0.3, 0.24];
/// Character colors are display colors from the pack; as albedo they stay
/// below this, so no surface returns more light than it receives.
const MAX_ALBEDO: f32 = 0.9;

pub(crate) struct Ruins {
    pub assets: LoadedAssets,
    pub simulation: Simulation,
    pub snapshot: Snapshot,
    particles: Vec<Particle>,
    rendered: Mesh,
    controller: verse_ruins::Controller,
}
struct Particle {
    pos: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    color: [f32; 3],
}

impl Ruins {
    pub fn new(assets: LoadedAssets) -> Result<Self, String> {
        let simulation = Simulation::new(Self::spawn().to_array())?;
        let snapshot = simulation.snapshot();
        let mut ruins = Self {
            assets,
            simulation,
            snapshot,
            particles: Vec::new(),
            rendered: Mesh::default(),
            controller: verse_ruins::Controller::new(Self::spawn()),
        };
        ruins.rendered = ruins.build_dynamic(&crate::controller::PlayerController::new(
            Self::spawn(),
            0.0,
        ));
        Ok(ruins)
    }
    pub fn spawn() -> Vec3 {
        Vec3::new(0.0, Terrain::bundled().height(0.0, 0.0), 0.0)
    }
    /// The terrain as lit triangles with the heightfield's smooth normals.
    /// The key light and the ambient probes shade it, where the original
    /// baked a Lambert term from [`SUN`] into its colors.
    pub fn world(&self) -> World {
        let mut world = World::default();
        let terrain = Terrain::bundled();
        let corner = |x: usize, z: usize| {
            lit_vertex(
                Vec3::from(terrain.vertex(x, z)),
                Vec3::from(terrain.normal(x, z)),
                GROUND,
            )
        };
        for z in 0..terrain.size() - 1 {
            for x in 0..terrain.size() - 1 {
                let [a, b, c, d] = [
                    corner(x, z),
                    corner(x + 1, z),
                    corner(x + 1, z + 1),
                    corner(x, z + 1),
                ];
                world.mesh.lit.extend([a, b, c, a, c, d]);
            }
        }
        world
    }

    /// The zone's physical stage: the dark green field and fog of the
    /// original, and a key light from the original's baked light direction,
    /// whose shadow follows the camera to the fog.
    pub fn stage(time: f32) -> Neon {
        let air = super::atmosphere(super::ZoneId::Ruins);
        Neon {
            field: air.color,
            fog_start: air.fog_start,
            fog_end: air.fog_end,
            line_gain: 1.0,
            line_width: 1.4,
            bloom: 0.04,
            vignette: 0.15,
            time,
            key: Some(Self::key()),
            daylight: None,
            height_fog: air.height_fog,
        }
    }

    /// Light levels that keep the original's brightness: a moss-green slope
    /// facing the sun reads as it did under the baked Lambert term, and a
    /// slope facing away keeps about the original's ambient share.
    fn key() -> Key {
        Key {
            dir: SUN.normalize(),
            illuminance: 3_000.0,
            angular_radius: 0.03,
            rim_dir: Vec3::new(0.4, 0.35, -0.6).normalize(),
            rim_illuminance: 600.0,
            rim_angular_radius: 0.1,
            sky: 1_300.0,
            ground: 300.0,
            ev100: 10.0,
            shadow_center: Vec3::ZERO,
            shadow_half: 150.0,
            // The fog closes at 82 m.
            shadow_distance: Some(80.0),
            // Destructible ruins and roaming undead are frame geometry.
            cache_far_shadows: false,
        }
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
            &verse_ruins::MovementInput {
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
        let mut mesh = Mesh {
            neon: Some(Self::stage(self.snapshot.elapsed)),
            ..Mesh::default()
        };
        for chunk in self.simulation.ruins() {
            // Whole triangles only, so a short index list cannot shift every
            // later triangle's corners.
            for triangle in chunk.indices.chunks_exact(3) {
                let corners: Vec<_> = triangle
                    .iter()
                    .filter_map(|&index| {
                        let i = index as usize;
                        let pos = *chunk.positions.get(i)?;
                        let normal = chunk.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                        Some(lit_vertex(Vec3::from(pos), Vec3::from(normal), STONE))
                    })
                    .collect();
                if corners.len() == 3 {
                    mesh.lit.extend(corners);
                }
            }
        }
        let elapsed = self.snapshot.elapsed;
        if self.snapshot.player.hp > 0 {
            lit_model(
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
            lit_model(
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
/// One lit vertex of a rough, non-metallic surface.
fn lit_vertex(pos: Vec3, normal: Vec3, color: [f32; 3]) -> LitVertex {
    let normal = normal.normalize_or(Vec3::Y);
    // Any unit vector across the normal: these surfaces are not brushed.
    let across = if normal.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Z
    };
    let tangent = (across - normal * across.dot(normal)).normalize_or(Vec3::X);
    LitVertex {
        pos: pos.to_array(),
        normal: normal.to_array(),
        tangent: tangent.to_array(),
        local: pos.to_array(),
        color: color.map(|c| c.clamp(0.0, MAX_ALBEDO)),
        params: [0.0, 0.85, Material::WhitePaint.code(), 1.0],
    }
}

/// A character model's faces as lit triangles with flat normals, its vertex
/// colors as albedo; its lines stay display-colored.
fn lit_model(target: &mut Mesh, source: &Mesh, transform: Mat4) {
    for triangle in source.faces.chunks_exact(3) {
        let corners = [0, 1, 2].map(|k| transform.transform_point3(Vec3::from(triangle[k].pos)));
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or(Vec3::Y);
        for (k, corner) in corners.into_iter().enumerate() {
            target
                .lit
                .push(lit_vertex(corner, normal, triangle[k].color));
        }
    }
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
