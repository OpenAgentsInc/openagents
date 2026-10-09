//! Broken chunks drawn as GPU instances (issue #10937).
//!
//! The town's pool ([`super::town`]) poses every vertex of every drawn
//! chunk on the CPU each frame, shades each one from the probe grid, and
//! uploads them all again. A meteor swarm leaves hundreds of chunks, over a
//! hundred thousand vertices, so that work cost several milliseconds a
//! frame. A [`Herd`] instead holds each chunk part once, in the chunk's own
//! space, as one mesh of a [`crate::pbr::textured::Instances`] scene the
//! renderer uploads once; a frame writes only each live chunk's transform
//! and the light of its vertices. Chunks of one shape draw together, one
//! instanced draw a part, after Unreal's instanced static meshes (an idea,
//! not code: `docs/research/unreal/AGENTS.md`).
//!
//! The light stays per vertex: the probe grid is blended once at each
//! chunk's center ([`AmbientProbes::at`]), and the chunk's vertices take
//! that light along their own normals, computed once for each distinct
//! normal of the part ([`QUANTUM`]) rather than once a vertex. Settled
//! rubble merges into one world-space mesh a material ([`Herd::merged`]),
//! so a pile at rest draws in a few draws and is written once.
//! A broken chunk's darker shade is baked into its mesh's colors, as the
//! pool darkens it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3};

use super::chunks::ChunkMesh;
use crate::pbr::textured::{
    InstanceOf, Instances, Primitive, TexturedMesh, TexturedScene, TexturedVertex, UNBAKED,
};
use crate::pbr::textured_bake::AmbientProbes;

/// The step normals are rounded to when finding a part's distinct ones: a
/// sixteenth along each axis, under four degrees, which ambient light
/// barely changes over.
pub const QUANTUM: f32 = 1.0 / 16.0;

/// A part's distinct normals and which one each vertex takes.
#[derive(Clone, Debug, Default)]
struct Normals {
    unique: Vec<Vec3>,
    of: Vec<u16>,
}

/// The chunk parts of the looks a town's buildings break into, as one
/// mesh each.
pub struct Herd {
    scene: Arc<TexturedScene>,
    /// Each mesh by chunk shape, chunk, part, and pool material.
    ids: BTreeMap<(usize, usize, usize, usize), u32>,
    normals: Vec<Normals>,
    /// The looks, shape and paint, whose parts have their meshes here.
    looks: BTreeSet<(usize, usize)>,
}

impl Herd {
    /// The meshes of `looks` (chunk shape and paint) cut as `meshes`, in
    /// the pool materials `materials` gives each pack material and paint,
    /// over `kit`'s images and materials, darkened by `shade`.
    pub fn new(
        kit: &TexturedScene,
        meshes: &[Vec<ChunkMesh>],
        materials: &BTreeMap<(u16, usize), usize>,
        looks: impl IntoIterator<Item = (usize, usize)>,
        shade: f32,
    ) -> Self {
        let mut scene = kit.clone();
        scene.meshes.clear();
        scene.placements.clear();
        let mut ids = BTreeMap::new();
        let mut normals = Vec::new();
        let mut covered = BTreeSet::new();
        for (shape, paint) in looks.into_iter().collect::<BTreeSet<_>>() {
            let Some(chunks) = meshes.get(shape) else {
                continue;
            };
            if chunks.is_empty() {
                continue;
            }
            for (chunk, mesh) in chunks.iter().enumerate() {
                for (part, (source, vertices)) in mesh.parts.iter().enumerate() {
                    // A part without a pool material draws nowhere, as in
                    // the pool.
                    let Some(&material) = materials.get(&(*source, paint)) else {
                        continue;
                    };
                    let key = (shape, chunk, part, material);
                    if ids.contains_key(&key) || vertices.len() < 3 {
                        continue;
                    }
                    let vertices: Vec<_> = vertices
                        .iter()
                        .map(|v| {
                            let mut v = *v;
                            for c in &mut v.color[..3] {
                                *c = (f32::from(*c) * shade) as u8;
                            }
                            v
                        })
                        .collect();
                    normals.push(distinct(&vertices));
                    let count = (vertices.len() / 3 * 3) as u32;
                    ids.insert(key, scene.meshes.len() as u32);
                    scene.meshes.push(TexturedMesh {
                        primitives: vec![Primitive {
                            vertices,
                            indices: (0..count).collect(),
                            material,
                        }],
                    });
                }
            }
            covered.insert((shape, paint));
        }
        Self {
            scene: Arc::new(scene),
            ids,
            normals,
            looks: covered,
        }
    }

    /// Whether the parts of a look's chunks have their meshes here.
    #[must_use]
    pub fn covers(&self, shape: usize, paint: usize) -> bool {
        self.looks.contains(&(shape, paint))
    }

    /// The mesh of a chunk part in a pool material, if there is one.
    #[must_use]
    pub fn mesh(&self, shape: usize, chunk: usize, part: usize, material: usize) -> Option<u32> {
        self.ids.get(&(shape, chunk, part, material)).copied()
    }

    /// How many meshes it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.normals.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.normals.is_empty()
    }

    /// This frame's instances: each of `copies`, a mesh and its transform,
    /// lit by `probes` at its center along each of its distinct normals,
    /// or left unlit without them.
    #[must_use]
    pub fn frame(
        &self,
        copies: impl IntoIterator<Item = (u32, Mat4)>,
        probes: Option<&AmbientProbes>,
    ) -> Instances {
        let mut records = Vec::new();
        let mut lights = Vec::new();
        let mut table = Vec::new();
        for (mesh, transform) in copies {
            let Some(normals) = self.normals.get(mesh as usize) else {
                continue;
            };
            records.push(InstanceOf { mesh, transform });
            match probes {
                Some(probes) => {
                    let local = probes.at(transform.w_axis.truncate());
                    let turn = Mat3::from_mat4(transform);
                    table.clear();
                    table.extend(
                        normals
                            .unique
                            .iter()
                            .map(|&n| local.encoded((turn * n).normalize_or(Vec3::Y))),
                    );
                    lights.extend(normals.of.iter().map(|&k| table[usize::from(k)]));
                }
                None => lights.extend(std::iter::repeat_n(UNBAKED, normals.of.len())),
            }
        }
        Instances {
            scene: self.scene.clone(),
            records: Arc::new(records),
            lights: Arc::new(lights),
        }
    }
}

impl Herd {
    /// `copies` merged into one mesh a material, in world space, with
    /// their light from `probes` baked in: rubble at rest, drawn in a few
    /// draws and written to the GPU once rather than every frame.
    #[must_use]
    pub fn merged(
        &self,
        copies: impl IntoIterator<Item = (u32, Mat4)>,
        probes: Option<&AmbientProbes>,
    ) -> Instances {
        let mut by_material: BTreeMap<usize, (Vec<TexturedVertex>, Vec<[u8; 4]>)> = BTreeMap::new();
        let mut table = Vec::new();
        for (mesh, transform) in copies {
            let (Some(normals), Some(source)) = (
                self.normals.get(mesh as usize),
                self.scene.meshes.get(mesh as usize),
            ) else {
                continue;
            };
            let Some(primitive) = source.primitives.first() else {
                continue;
            };
            let local = probes.map(|p| p.at(transform.w_axis.truncate()));
            let turn = Mat3::from_mat4(transform);
            table.clear();
            table.extend(normals.unique.iter().map(|&n| match &local {
                Some(local) => local.encoded((turn * n).normalize_or(Vec3::Y)),
                None => UNBAKED,
            }));
            let (vertices, lights) = by_material.entry(primitive.material).or_default();
            let count = primitive.indices.len().min(primitive.vertices.len());
            for (v, &k) in primitive.vertices[..count].iter().zip(&normals.of) {
                vertices.push(TexturedVertex {
                    pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
                    normal: (turn * Vec3::from(v.normal))
                        .normalize_or(Vec3::Y)
                        .to_array(),
                    ..*v
                });
                lights.push(table[usize::from(k)]);
            }
        }
        let mut scene = TexturedScene {
            images: self.scene.images.clone(),
            materials: self.scene.materials.clone(),
            ..TexturedScene::default()
        };
        let mut records = Vec::new();
        let mut all = Vec::new();
        for (material, (vertices, lights)) in by_material {
            let count = (vertices.len() / 3 * 3) as u32;
            records.push(InstanceOf {
                mesh: scene.meshes.len() as u32,
                transform: Mat4::IDENTITY,
            });
            all.extend_from_slice(&lights);
            scene.meshes.push(TexturedMesh {
                primitives: vec![Primitive {
                    vertices,
                    indices: (0..count).collect(),
                    material,
                }],
            });
        }
        Instances {
            scene: Arc::new(scene),
            records: Arc::new(records),
            lights: Arc::new(all),
        }
    }
}

/// A part's distinct normals, rounded to [`QUANTUM`].
fn distinct(vertices: &[crate::pbr::textured::TexturedVertex]) -> Normals {
    let mut seen: BTreeMap<[i32; 3], u16> = BTreeMap::new();
    let mut out = Normals::default();
    for v in vertices {
        let n = Vec3::from(v.normal).normalize_or(Vec3::Y);
        let key = (n / QUANTUM).round().as_ivec3().to_array();
        let next = out.unique.len();
        let k = *seen.entry(key).or_insert_with(|| {
            out.unique.push(n);
            next as u16
        });
        out.of.push(k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbr::ProbeGrid;
    use crate::pbr::textured::{TexturedMaterial, TexturedVertex};
    use crate::pbr::textured_bake::{BakeLight, decode};

    fn cube_part() -> Vec<TexturedVertex> {
        // Two triangles facing up and two facing east.
        let up = Vec3::Y;
        let east = Vec3::X;
        let v = |p: [f32; 3], n: Vec3| {
            let mut v = TexturedVertex::new(Vec3::from(p), n, [0.0, 0.0]);
            v.color = [200, 100, 50, 255];
            v
        };
        vec![
            v([0.0, 0.5, 0.0], up),
            v([1.0, 0.5, 0.0], up),
            v([1.0, 0.5, 1.0], up),
            v([0.0, 0.5, 0.0], up),
            v([1.0, 0.5, 1.0], up),
            v([0.0, 0.5, 1.0], up),
            v([0.5, 0.0, 0.0], east),
            v([0.5, 1.0, 0.0], east),
            v([0.5, 1.0, 1.0], east),
        ]
    }

    fn herd() -> Herd {
        let mut kit = TexturedScene::default();
        kit.add_material(TexturedMaterial::default());
        let meshes = vec![vec![ChunkMesh {
            cuboid: None,
            parts: vec![(0, cube_part())],
        }]];
        let materials = BTreeMap::from([((0u16, 0usize), 0usize)]);
        Herd::new(&kit, &meshes, &materials, [(0, 0), (0, 0)], 0.8)
    }

    #[test]
    fn each_part_is_one_mesh_with_its_shade_and_distinct_normals() {
        let herd = herd();
        assert_eq!(herd.len(), 1);
        assert!(herd.covers(0, 0) && !herd.covers(0, 1));
        assert_eq!(herd.mesh(0, 0, 0, 0), Some(0));
        assert_eq!(herd.normals[0].unique.len(), 2);
        let v = herd.scene.meshes[0].primitives[0].vertices[0];
        assert_eq!(&v.color[..3], &[160, 80, 40]);
    }

    #[test]
    fn a_frame_writes_transforms_and_light_along_turned_normals() {
        let herd = herd();
        // A grid lit from above only: up-facing surfaces get the sky.
        let light = BakeLight {
            sun_dir: Vec3::Y,
            sun_illuminance: 0.0,
            sky: 1_000.0,
            ground: 0.0,
        };
        let mut data = vec![[0.0f32; 12]; 8];
        for probe in &mut data {
            for c in 0..3 {
                // Irradiance 500 + 500 n.y: the open sky's.
                probe[c * 4] = 500.0;
                probe[c * 4 + 2] = 500.0;
            }
        }
        let probes = AmbientProbes {
            grid: ProbeGrid {
                origin: Vec3::splat(-10.0),
                cell: 20.0,
                dims: [2, 2, 2],
                data,
                version: 1,
            },
            light,
        };
        let turned = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0))
            * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let frame = herd.frame(
            [(0, Mat4::IDENTITY), (0, turned), (9, turned)],
            Some(&probes),
        );
        frame.validate().unwrap();
        assert_eq!(frame.records.len(), 2, "an unknown mesh draws nothing");
        assert_eq!(frame.lights.len(), 18);
        assert_eq!(frame.records[1].transform, turned);
        // Up-facing vertices see the open sky everywhere.
        assert!((decode(frame.lights[0]).0.x - 1.0).abs() < 0.05);
        assert!(
            herd.frame([(0, turned)], None)
                .lights
                .iter()
                .all(|l| *l == UNBAKED)
        );
    }
}
