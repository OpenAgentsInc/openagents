//! The coastal shell: procedural ground, the clipmap ocean, and movement.

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use physics::water::Water as _;
use verse_core::world::World;
use verse_pbr::mesh::Mesh;
use verse_pbr::pbr::textured::{
    Primitive, TexturedMaterial, TexturedMesh, TexturedScene, TexturedVertex,
};
use verse_water_spells::sea::{self, Hour};
use verse_world::social::{
    controller::{InputState, PlayerController},
    solids::Solids,
};
use verse_world::water::{Breath, BreathEvent, Medium, Stroke, medium, pace};

use crate::{HALF_EXTENT, SPAWN, ground, water};

/// The terrain is procedural and uses no licensed content or light bake.
pub fn world() -> Result<World, String> {
    let mut scene = TexturedScene::default();
    let material = scene.add_material(TexturedMaterial {
        roughness: 0.94,
        ..TexturedMaterial::default()
    });
    // The base grid stays under Low's triangle budget. Refine the narrow
    // estuary and pool shelf so their drawn beds follow collision depths.
    let cell = 2.0 * HALF_EXTENT / 256.0;
    for tz in 0..8 {
        for tx in 0..8 {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            for z in 0..32 {
                for x in 0..32 {
                    let origin = Vec2::new(
                        -HALF_EXTENT + (tx * 32 + x) as f32 * cell,
                        -HALF_EXTENT + (tz * 32 + z) as f32 * cell,
                    );
                    let center = origin + Vec2::splat(cell * 0.5);
                    let fine = refined(center);
                    let neighbors = [Vec2::NEG_X, Vec2::Y, Vec2::X, Vec2::NEG_Y]
                        .map(|offset| refined(center + offset * cell));
                    if !fine && neighbors.iter().any(|fine| *fine) {
                        // Match refined neighbors at every shared edge sample.
                        // A fan avoids cracks without refining the whole coast.
                        let first = vertices.len() as u32;
                        vertices.push(vertex(center.x, center.y));
                        let corners = [Vec2::ZERO, Vec2::Y, Vec2::ONE, Vec2::X];
                        for edge in 0..4 {
                            let steps = if neighbors[edge] { 4 } else { 1 };
                            for step in 0..steps {
                                let uv = corners[edge]
                                    .lerp(corners[(edge + 1) % 4], step as f32 / steps as f32);
                                let p = origin + uv * cell;
                                vertices.push(vertex(p.x, p.y));
                            }
                        }
                        let count = vertices.len() as u32 - first - 1;
                        for i in 0..count {
                            indices.extend_from_slice(&[
                                first,
                                first + 1 + i,
                                first + 1 + (i + 1) % count,
                            ]);
                        }
                        continue;
                    }
                    let divisions = if fine { 4 } else { 1 };
                    let first = vertices.len() as u32;
                    for j in 0..=divisions {
                        for i in 0..=divisions {
                            let p =
                                origin + Vec2::new(i as f32, j as f32) * (cell / divisions as f32);
                            vertices.push(vertex(p.x, p.y));
                        }
                    }
                    for j in 0..divisions {
                        for i in 0..divisions {
                            let a = first + j * (divisions + 1) + i;
                            let row = divisions + 1;
                            indices.extend_from_slice(&[
                                a,
                                a + row,
                                a + 1,
                                a + 1,
                                a + row,
                                a + row + 1,
                            ]);
                        }
                    }
                }
            }
            let mesh = scene.add_mesh(TexturedMesh {
                primitives: vec![Primitive {
                    vertices,
                    indices,
                    material,
                }],
            });
            scene.place(mesh, Mat4::IDENTITY);
        }
    }
    // Continue the terrain to the ocean's horizon so refraction has a bed.
    // The inner edge uses the same samples as the playable mesh.
    for side in 0..4 {
        let mut vertices = Vec::with_capacity(514);
        for i in 0..=256 {
            let t = -HALF_EXTENT + i as f32 * (2.0 * HALF_EXTENT / 256.0);
            let (inner, outer) = match side {
                0 => ([t, -HALF_EXTENT], [t * 8.0, -4800.0]),
                1 => ([HALF_EXTENT, t], [4800.0, t * 8.0]),
                2 => ([-t, HALF_EXTENT], [-t * 8.0, 4800.0]),
                _ => ([-HALF_EXTENT, -t], [-4800.0, -t * 8.0]),
            };
            vertices.push(vertex(inner[0], inner[1]));
            vertices.push(vertex(outer[0], outer[1]));
        }
        let mut indices = Vec::with_capacity(256 * 6);
        for i in 0..256u32 {
            let a = 2 * i;
            indices.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
        }
        let mesh = scene.add_mesh(TexturedMesh {
            primitives: vec![Primitive {
                vertices,
                indices,
                material,
            }],
        });
        scene.place(mesh, Mat4::IDENTITY);
    }
    let blockers = crate::kit::append(&mut scene)?;
    scene.validate()?;
    let fields = water::fields()?;
    let surface = crate::surface::surface(Arc::new(fields.optical));
    surface.validate()?;
    Ok(World {
        mesh: Mesh {
            textured: Some(Arc::new(scene)),
            water: Some(Arc::new(surface)),
            ..Mesh::default()
        },
        blockers,
    })
}

fn refined(center: Vec2) -> bool {
    crate::terrain::segment(
        center,
        crate::terrain::ESTUARY_MOUTH,
        crate::terrain::ESTUARY_GATE,
    )
    .0 < 18.0
        || center.distance(crate::terrain::POOL_SHELF) < 35.0
}

fn vertex(x: f32, z: f32) -> TexturedVertex {
    let h = ground(x, z);
    let normal = Vec3::new(
        ground(x - 0.5, z) - ground(x + 0.5, z),
        1.0,
        ground(x, z - 0.5) - ground(x, z + 0.5),
    )
    .normalize();
    let mut v = TexturedVertex::new(Vec3::new(x, h, z), normal, [x * 0.1, z * 0.1]);
    v.color = if normal.y < 0.7 {
        [91, 88, 80, 255]
    } else if h < 3.0 {
        [176, 155, 111, 255]
    } else {
        [91, 112, 67, 255]
    };
    v
}

/// Mutable character state and a shared Unix tick. The host supplies the
/// tick; a capture supplies a fixed tick instead of a process-local clock.
pub struct Coast {
    pub tick: u64,
    pub water: water::CoastalWater,
    solids: Solids,
    breath: Breath,
    pub medium: Medium,
}

impl Coast {
    pub fn new(tick: u64) -> Result<Self, String> {
        Ok(Self {
            tick,
            water: water::CoastalWater::new("calm")?,
            solids: crate::kit::solids()?,
            breath: Breath::new(2),
            medium: Medium::Ground,
        })
    }

    pub fn move_player(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        pitch: f32,
        dt: f32,
    ) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.1);
        let p = player.pos;
        let sample = self.water.sample(p.x as f64, p.z as f64, self.tick);
        self.medium = medium::classify(
            sample.map(|s| s.height),
            ground(p.x, p.z) as f64,
            p.y as f64,
        );
        // Match Everglade's look-to-dive dead zone for ordinary forward strokes.
        let steered = pitch > 0.5 || pitch < -0.1;
        let speed = pace(self.medium, false, false, self.breath.levels());
        player.set_pace(if self.medium.afloat() && steered {
            speed * pitch.cos()
        } else {
            speed
        });
        if let Some(sample) = sample.filter(|_| self.medium.afloat()) {
            let mut swim = *input;
            swim.jump = false;
            self.solids.step(player, &swim, dt, HALF_EXTENT, false);
            let moved = player.pos;
            let current = sample.flow.as_vec3() * dt;
            player.pos.x = (moved.x + current.x).clamp(-HALF_EXTENT, HALF_EXTENT);
            player.pos.z = (moved.z + current.z).clamp(-HALF_EXTENT, HALF_EXTENT);
            let bed = ground(player.pos.x, player.pos.z) as f64;
            let surface = self
                .water
                .sample(player.pos.x as f64, player.pos.z as f64, self.tick)
                .map_or(sample.height, |s| s.height);
            let vertical = if input.jump {
                2.0
            } else if input.forward && steered {
                -2.0 * f64::from(pitch.sin())
            } else {
                0.0
            };
            let feet = medium::swim_height(
                p.y as f64,
                surface,
                bed,
                Stroke {
                    vertical,
                    held: false,
                },
                dt as f64,
            )
            .max(bed) as f32;
            player.set_surface_height(feet);
            player.hold_altitude(feet);
        } else {
            self.solids.step(player, input, dt, HALF_EXTENT, true);
        }
        let p = player.pos;
        let surface = self.water.sample(p.x as f64, p.z as f64, self.tick);
        self.medium = medium::classify(
            surface.map(|s| s.height),
            ground(p.x, p.z) as f64,
            p.y as f64,
        );
        if self
            .breath
            .tick(dt, self.medium == Medium::Diving, false)
            .contains(&BreathEvent::Defeated)
        {
            player.pos = SPAWN;
            player.set_surface_height(SPAWN.y);
            player.hold_altitude(SPAWN.y);
            self.medium = Medium::Ground;
        }
    }

    pub fn mesh(&self, eye: Vec3) -> Mesh {
        let mut water = crate::surface::frame(self.tick);
        if let Some(sample) = self.water.sample(eye.x as f64, eye.z as f64, self.tick) {
            water.eye = Some(verse_pbr::water::under::EyeSurface {
                body: sample.body.0 as usize,
                height: sample.height as f32,
                slope: [
                    (-sample.normal.x / sample.normal.y) as f32,
                    (-sample.normal.z / sample.normal.y) as f32,
                ],
            });
        }
        let mut stage = sea::stage(water.time, Hour::Noon, water);
        stage.fog_start = 600.0;
        stage.fog_end = 2000.0;
        stage.height_fog = None;
        Mesh {
            neon: Some(stage),
            ..Mesh::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_uses_coast_bounds_and_swimming_follows_current() {
        let mut live = Coast::new(water::TIDE_PERIOD / 4).unwrap();
        let mut player = PlayerController::new(SPAWN, 0.0);
        player.pos = Vec3::new(500.0, ground(500.0, -500.0), -500.0);
        player.set_surface_height(player.pos.y);
        live.move_player(&mut player, &InputState::default(), 0.0, 0.05);
        assert!(
            player.pos.x > 490.0,
            "a coast step must not clamp to town bounds"
        );
        player.pos = Vec3::new(590.0, 0.0, 590.0);
        player.set_surface_height(player.pos.y);
        let before = player.pos;
        for _ in 0..120 {
            live.move_player(&mut player, &InputState::default(), 0.0, 1.0 / 60.0);
        }
        assert!(player.pos.x < before.x && player.pos.z < before.z);
        assert!(live.medium.afloat());
        assert!(
            player.pos.y > -3.0,
            "a swimmer must not sink to the 40 m bed"
        );
        let mut dive = InputState::default();
        dive.forward = true;
        for _ in 0..180 {
            live.move_player(&mut player, &dive, 0.3, 1.0 / 60.0);
        }
        assert_ne!(
            live.medium,
            Medium::Diving,
            "ordinary forward strokes stay afloat"
        );
        for _ in 0..180 {
            live.move_player(&mut player, &dive, 1.0, 1.0 / 60.0);
        }
        assert_eq!(live.medium, Medium::Diving);
        assert!(live.breath.left() < live.breath.limit());
    }

    #[test]
    fn procedural_world_validates_within_the_low_terrain_budget() {
        let world = world().unwrap();
        let scene = world.mesh.textured.as_ref().unwrap();
        scene.validate().unwrap();
        let triangles: usize = scene
            .meshes
            .iter()
            .flat_map(|m| &m.primitives)
            .map(|p| p.indices.len() / 3)
            .sum();
        assert!(triangles <= 250_000, "{triangles}");
        // Reserve High's entire water allowance even for the Low budget.
        let textures: u64 = scene.images.iter().map(|i| i.rgba.len() as u64).sum();
        assert!(scene.gpu_bytes() + textures + 64 * 1024 * 1024 < 160 * 1024 * 1024);
        let mut edges = std::collections::HashMap::<([u32; 3], [u32; 3]), usize>::new();
        for primitive in scene
            .meshes
            .iter()
            .take(68)
            .flat_map(|mesh| &mesh.primitives)
        {
            for triangle in primitive.indices.chunks_exact(3) {
                for edge in 0..3 {
                    let a = primitive.vertices[triangle[edge] as usize].pos;
                    let b = primitive.vertices[triangle[(edge + 1) % 3] as usize].pos;
                    let mut pair = [a.map(f32::to_bits), b.map(f32::to_bits)];
                    pair.sort();
                    *edges.entry((pair[0], pair[1])).or_default() += 1;
                }
            }
        }
        for ((a, b), count) in edges {
            let inside = |p: [u32; 3]| {
                f32::from_bits(p[0]).abs() < HALF_EXTENT && f32::from_bits(p[2]).abs() < HALF_EXTENT
            };
            if inside(a) && inside(b) {
                assert_eq!(count, 2, "an interior terrain edge must have two faces");
            }
        }
        let water = world.mesh.water.as_ref().unwrap();
        water.validate().unwrap();
        assert!(water.ocean.as_ref().unwrap().field.is_some());
    }
}
