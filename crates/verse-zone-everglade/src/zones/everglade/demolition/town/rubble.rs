//! Settled chunk geometry, grouped without changing its material or light.

use super::{
    AmbientProbes, DynamicInstance, InstancedFigure, Primitive, TexturedMesh, TexturedScene,
    TexturedVertex, UNBAKED,
};
use glam::{Mat3, Mat4, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

/// Settled geometry retained by the town's static batches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MergeStats {
    pub parts: usize,
    pub groups: usize,
    pub vertices: usize,
    /// Geometry transformations since the last reset; unchanged frames add none.
    pub transformed_vertices: u64,
    pub rebuilds: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Part<'a> {
    pub instance: DynamicInstance,
    pub source: [usize; 3],
    pub material: usize,
    pub vertices: &'a [TexturedVertex],
}

pub(super) fn shade(instance: &mut DynamicInstance, probes: Option<&AmbientProbes>) {
    if instance.light != UNBAKED {
        return;
    }
    let Some(probes) = probes else {
        return;
    };
    let mut center = [TexturedVertex::new(
        instance.current.w_axis.truncate(),
        instance
            .current
            .transform_vector3(Vec3::Y)
            .normalize_or(Vec3::Y),
        [0.0; 2],
    )];
    probes.shade(&mut center);
    instance.light = center[0].light;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Group {
    material: usize,
    light: [u8; 4],
    tint: [u32; 4],
}

impl Group {
    fn of(part: &Part<'_>) -> Self {
        Self {
            material: part.material,
            light: part.instance.light,
            tint: part.instance.color.map(f32::to_bits),
        }
    }
}

struct Member {
    source: [usize; 3],
    transform: Mat4,
    group: Group,
    mesh: usize,
    vertices: Range<usize>,
}

struct Batch {
    group: Group,
    mesh: usize,
}

pub(super) struct Cache {
    pub scene: Arc<TexturedScene>,
    motion_epoch: Arc<()>,
    sources: usize,
    members: BTreeMap<u64, Member>,
    batches: Vec<Batch>,
    dirty: bool,
    pub stats: MergeStats,
}

impl Cache {
    pub fn new(scene: TexturedScene) -> Self {
        let sources = scene.meshes.len();
        Self {
            scene: Arc::new(scene),
            motion_epoch: Arc::new(()),
            sources,
            members: BTreeMap::new(),
            batches: Vec::new(),
            dirty: false,
            stats: MergeStats::default(),
        }
    }

    pub fn held(&self) -> usize {
        vertices(&self.scene.meshes)
    }

    pub fn source_vertices(&self) -> usize {
        vertices(&self.scene.meshes[..self.sources])
    }

    pub fn add_source(&mut self, mesh: TexturedMesh) -> usize {
        let index = self.sources;
        Arc::make_mut(&mut self.scene).meshes.insert(index, mesh);
        self.sources += 1;
        for member in self.members.values_mut() {
            member.mesh += 1;
        }
        for batch in &mut self.batches {
            batch.mesh += 1;
        }
        index
    }

    pub fn reset_motion(&mut self) {
        self.motion_epoch = Arc::new(());
        let _ = Arc::make_mut(&mut self.scene);
        // Retain can reuse a former member's body and piece identity.
        self.members.clear();
        self.dirty = true;
    }

    pub fn reset_geometry(&mut self, scene: TexturedScene) {
        let epoch = self.motion_epoch.clone();
        *self = Self::new(scene);
        self.motion_epoch = epoch;
    }

    pub fn frame(&mut self, moving: Vec<DynamicInstance>, parts: &[Part<'_>]) -> InstancedFigure {
        let changed = self.dirty
            || self.members.len() != parts.len()
            || parts.iter().any(|part| {
                self.members.get(&part.instance.id).is_none_or(|old| {
                    old.source != part.source
                        || old.transform != part.instance.current
                        || old.group != Group::of(part)
                })
            });
        if changed {
            self.rebuild(parts);
        }
        let mut instances = moving;
        instances.reserve(self.batches.len());
        let reserved = u64::MAX.saturating_sub(self.batches.len() as u64);
        let mut ids = (!self.batches.is_empty() && instances.iter().any(|i| i.id >= reserved))
            .then(|| instances.iter().map(|i| i.id).collect::<BTreeSet<_>>());
        let mut id = u64::MAX;
        for batch in &self.batches {
            if let Some(ids) = &mut ids {
                while ids.contains(&id) {
                    id -= 1;
                }
                ids.insert(id);
            }
            instances.push(DynamicInstance {
                id,
                mesh: batch.mesh,
                current: Mat4::IDENTITY,
                previous: Mat4::IDENTITY,
                color: batch.group.tint.map(f32::from_bits),
                light: batch.group.light,
                settled: true,
            });
            id -= 1;
        }
        InstancedFigure {
            vertex_lights: None,
            scene: self.scene.clone(),
            instances: Arc::new(instances),
            motion_epoch: self.motion_epoch.clone(),
        }
    }

    fn rebuild(&mut self, parts: &[Part<'_>]) {
        let old_scene = self.scene.clone();
        let old_members = std::mem::take(&mut self.members);
        let mut groups: BTreeMap<Group, (Primitive, Vec<(u64, Member)>)> = BTreeMap::new();
        for part in parts {
            let group = Group::of(part);
            let (primitive, members) = groups.entry(group).or_insert_with(|| {
                (
                    Primitive {
                        material: group.material,
                        ..Default::default()
                    },
                    Vec::new(),
                )
            });
            let first = primitive.vertices.len();
            if let Some(old) = old_members
                .get(&part.instance.id)
                .filter(|old| old.source == part.source && old.transform == part.instance.current)
            {
                let previous = &old_scene.meshes[old.mesh].primitives[0];
                primitive
                    .vertices
                    .extend_from_slice(&previous.vertices[old.vertices.clone()]);
            } else {
                let transform = part.instance.current;
                let normals = Mat3::from_mat4(transform).inverse().transpose();
                primitive
                    .vertices
                    .extend(part.vertices.iter().map(|vertex| {
                        TexturedVertex {
                            pos: transform
                                .transform_point3(Vec3::from(vertex.pos))
                                .to_array(),
                            normal: (normals * Vec3::from(vertex.normal))
                                .normalize_or(Vec3::Y)
                                .to_array(),
                            ..*vertex
                        }
                    }));
                self.stats.transformed_vertices += part.vertices.len() as u64;
            }
            let mirrored = part.instance.current.determinant() < 0.0;
            for a in (first as u32..primitive.vertices.len() as u32).step_by(3) {
                primitive.indices.extend(if mirrored {
                    [a, a + 2, a + 1]
                } else {
                    [a, a + 1, a + 2]
                });
            }
            members.push((
                part.instance.id,
                Member {
                    source: part.source,
                    transform: part.instance.current,
                    group,
                    mesh: 0,
                    vertices: first..primitive.vertices.len(),
                },
            ));
        }
        // Copy the source prefix without also cloning the old aggregates.
        // Old and new geometry can overlap during a frame handoff, while
        // the cache keeps one current scene once the handoff completes.
        let mut scene = TexturedScene {
            images: old_scene.images.clone(),
            materials: old_scene.materials.clone(),
            meshes: old_scene.meshes[..self.sources].to_vec(),
            placements: old_scene.placements.clone(),
            switches: old_scene.switches.clone(),
            detail_groups: old_scene.detail_groups.clone(),
            baked: old_scene.baked.clone(),
            edits: old_scene.edits.clone(),
        };
        self.batches.clear();
        for (group, (primitive, members)) in groups {
            let mesh = scene.add_mesh(TexturedMesh {
                primitives: vec![primitive],
            });
            self.batches.push(Batch { group, mesh });
            for (id, mut member) in members {
                member.mesh = mesh;
                self.members.insert(id, member);
            }
        }
        self.dirty = false;
        self.stats.parts = parts.len();
        self.stats.groups = self.batches.len();
        self.stats.vertices = vertices(&scene.meshes[self.sources..]);
        self.stats.rebuilds += 1;
        self.scene = Arc::new(scene);
    }
}

fn vertices(meshes: &[TexturedMesh]) -> usize {
    meshes
        .iter()
        .flat_map(|m| &m.primitives)
        .map(|p| p.vertices.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbr::textured::{AlphaMode, TexturedMaterial};
    use glam::Quat;

    fn triangle() -> Vec<TexturedVertex> {
        [Vec3::ZERO, Vec3::X, Vec3::Y]
            .into_iter()
            .map(|pos| TexturedVertex {
                pos: pos.to_array(),
                normal: Vec3::Z.to_array(),
                uv: [pos.x * 0.7, pos.y * 0.9],
                color: [51, 127, 201, 230],
                light: [20, 30, 40, 200],
            })
            .collect()
    }

    fn scene() -> TexturedScene {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial::default());
        scene.add_material(TexturedMaterial {
            alpha: AlphaMode::Mask { cutoff: 0.5 },
            ..Default::default()
        });
        scene.add_material(TexturedMaterial {
            alpha: AlphaMode::Blend,
            ..Default::default()
        });
        scene
    }

    #[test]
    fn regroup_and_geometry_eviction_keep_motion_identity_but_structural_reset_replaces_it() {
        let vertices = triangle();
        let mut cache = Cache::new(scene());
        let first = cache.frame(Vec::new(), &[]);
        let mut parts = [part(instance(1, 1.0), 0, &vertices)];
        let merged = cache.frame(Vec::new(), &parts);
        assert!(!Arc::ptr_eq(&first.scene, &merged.scene));
        assert!(Arc::ptr_eq(&first.motion_epoch, &merged.motion_epoch));
        parts[0].instance.light = [7; 4];
        let relit = cache.frame(Vec::new(), &parts);
        assert!(!Arc::ptr_eq(&merged.scene, &relit.scene));
        assert!(Arc::ptr_eq(&merged.motion_epoch, &relit.motion_epoch));
        cache.reset_geometry(scene());
        let evicted = cache.frame(Vec::new(), &parts);
        assert!(Arc::ptr_eq(&relit.motion_epoch, &evicted.motion_epoch));
        cache.reset_motion();
        let retained = cache.frame(Vec::new(), &parts);
        assert!(!Arc::ptr_eq(&evicted.motion_epoch, &retained.motion_epoch));
        let new_town = Cache::new(scene()).frame(Vec::new(), &[]);
        assert!(!Arc::ptr_eq(&new_town.motion_epoch, &retained.motion_epoch));
    }

    fn instance(id: u64, x: f32) -> DynamicInstance {
        DynamicInstance {
            id,
            mesh: 0,
            current: Mat4::from_rotation_translation(
                Quat::from_rotation_y(0.3),
                Vec3::new(x, 0.4, 2.0),
            ),
            previous: Mat4::IDENTITY,
            color: [0.8, 0.7, 0.6, 1.0],
            light: [90, 100, 110, 210],
            settled: true,
        }
    }

    fn part<'a>(
        instance: DynamicInstance,
        material: usize,
        vertices: &'a [TexturedVertex],
    ) -> Part<'a> {
        Part {
            instance,
            source: [0, 0, 0],
            material,
            vertices,
        }
    }

    #[test]
    fn static_groups_preserve_geometry_uv_color_light_material_and_reflected_winding() {
        let vertices = triangle();
        let mut reflected = instance(3, 6.0);
        reflected.current *= Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        let parts = [
            part(instance(1, 1.0), 0, &vertices),
            part(instance(2, 3.0), 0, &vertices),
            part(reflected, 1, &vertices),
        ];
        let mut cache = Cache::new(scene());
        let frame = cache.frame(Vec::new(), &parts);
        frame.validate().unwrap();
        assert_eq!(frame.instances.len(), 2);
        assert_eq!(cache.stats.parts, 3);
        assert_eq!(cache.stats.transformed_vertices, 9);
        for part in &parts {
            let member = &cache.members[&part.instance.id];
            let primitive = &frame.scene.meshes[member.mesh].primitives[0];
            let record = frame
                .instances
                .iter()
                .find(|i| i.mesh == member.mesh)
                .unwrap();
            assert_eq!(primitive.material, part.material);
            assert_eq!(record.current, Mat4::IDENTITY);
            assert_eq!(record.previous, Mat4::IDENTITY);
            assert_eq!(
                record.color.map(f32::to_bits),
                part.instance.color.map(f32::to_bits)
            );
            assert_eq!(record.light, part.instance.light);
            let normals = Mat3::from_mat4(part.instance.current).inverse().transpose();
            for (actual, source) in primitive.vertices[member.vertices.clone()]
                .iter()
                .zip(&vertices)
            {
                assert!(
                    (Vec3::from(actual.pos)
                        - part.instance.current.transform_point3(source.pos.into()))
                    .length()
                        < 1e-6
                );
                assert!(
                    (Vec3::from(actual.normal) - (normals * Vec3::from(source.normal)).normalize())
                        .length()
                        < 1e-6
                );
                assert_eq!(actual.uv, source.uv);
                assert_eq!(
                    actual.color, source.color,
                    "tint remains an exact float instance factor"
                );
                assert_eq!(actual.light, source.light);
            }
        }
        assert_eq!(
            frame.scene.meshes[cache.members[&3].mesh].primitives[0].indices,
            [0, 2, 1]
        );
    }

    #[test]
    fn unchanged_static_frames_reuse_scene_and_light_regrouping_does_not_pose_again() {
        let vertices = triangle();
        let mut cache = Cache::new(scene());
        let mut parts = [
            part(instance(1, 1.0), 0, &vertices),
            part(instance(2, 2.0), 0, &vertices),
        ];
        let first = cache.frame(Vec::new(), &parts);
        let stats = cache.stats;
        let repeated = cache.frame(Vec::new(), &parts);
        assert!(Arc::ptr_eq(&first.scene, &repeated.scene));
        assert_eq!(cache.stats, stats);
        parts[1].instance.light = [120, 90, 80, 180];
        let relit = cache.frame(Vec::new(), &parts);
        assert_eq!(relit.instances.len(), 2);
        assert_eq!(
            cache.stats.transformed_vertices, 6,
            "light changes only regroup existing world vertices"
        );
        assert!(!Arc::ptr_eq(&first.scene, &relit.scene));
        let third = part(instance(3, 3.0), 0, &vertices);
        let next = cache.frame(Vec::new(), &[parts[0], parts[1], third]);
        assert_eq!(next.instances.len(), 2);
        assert_eq!(
            cache.stats.transformed_vertices, 9,
            "only the new member is transformed"
        );
    }

    #[test]
    fn wake_expiry_and_structural_reset_remove_cached_geometry_and_keep_live_ids_distinct() {
        let vertices = triangle();
        let mut cache = Cache::new(scene());
        cache.add_source(TexturedMesh {
            primitives: vec![Primitive {
                vertices: vertices.clone(),
                indices: vec![0, 1, 2],
                material: 0,
            }],
        });
        let parts = [
            part(instance(1, 1.0), 0, &vertices),
            part(instance(2, 2.0), 0, &vertices),
        ];
        let moving = DynamicInstance {
            id: u64::MAX,
            settled: false,
            ..instance(u64::MAX, 5.0)
        };
        let first = cache.frame(vec![moving], &parts);
        first.validate().unwrap();
        assert_eq!(first.instances[1].id, u64::MAX - 1);
        let wake = DynamicInstance {
            settled: false,
            ..instance(1, 1.0)
        };
        let woken = cache.frame(vec![moving, wake], &parts[1..]);
        woken.validate().unwrap();
        assert_eq!(cache.stats.parts, 1);
        assert_eq!(cache.stats.vertices, 3);
        assert_eq!(cache.stats.transformed_vertices, 6);
        let expired = cache.frame(vec![moving, wake], &[]);
        assert_eq!(
            expired.scene.meshes.len(),
            1,
            "expired aggregate suffix is removed"
        );
        assert_eq!(expired.instances.len(), 2);
        assert_eq!(cache.stats.parts, 0);
        cache.frame(vec![moving], &parts);
        cache.reset_motion();
        let restored = cache.frame(vec![moving], &[]);
        assert_eq!(
            restored.scene.meshes.len(),
            1,
            "reset cannot leave old static batches behind"
        );
        assert_eq!(restored.instances.len(), 1);
    }

    #[test]
    fn new_source_meshes_preserve_cached_aggregate_ranges() {
        let vertices = triangle();
        let mut cache = Cache::new(scene());
        let parts = [part(instance(1, 1.0), 0, &vertices)];
        let first = cache.frame(Vec::new(), &parts);
        let source = TexturedMesh {
            primitives: vec![Primitive {
                vertices: vertices.clone(),
                indices: vec![0, 1, 2],
                material: 0,
            }],
        };
        assert_eq!(cache.add_source(source), 0);
        let next = cache.frame(
            vec![DynamicInstance {
                settled: false,
                ..instance(9, 4.0)
            }],
            &parts,
        );
        next.validate().unwrap();
        assert_eq!(next.instances[1].mesh, 1);
        assert_eq!(next.scene.meshes[1], first.scene.meshes[0]);
        assert_eq!(cache.stats.transformed_vertices, 3);
    }

    #[test]
    fn settled_probe_sampling_matches_moving_chunks_and_keeps_destruction_override() {
        use crate::pbr::ProbeGrid;
        use crate::pbr::textured_bake::BakeLight;
        let probes = AmbientProbes {
            grid: ProbeGrid {
                origin: Vec3::ZERO,
                cell: 10.0,
                dims: [2; 3],
                data: vec![[3.0, 0.0, 0.0, 0.0, 6.0, 0.0, 0.0, 0.0, 9.0, 0.0, 0.0, 0.0]; 8],
                version: 1,
            },
            light: BakeLight {
                sun_dir: Vec3::Y,
                sun_illuminance: 10.0,
                sky: 10.0,
                ground: 2.0,
            },
        };
        let mut settled = DynamicInstance {
            light: UNBAKED,
            ..instance(1, 1.0)
        };
        let mut moving = DynamicInstance {
            settled: false,
            ..settled
        };
        shade(&mut settled, Some(&probes));
        shade(&mut moving, Some(&probes));
        assert_eq!(settled.light, moving.light);
        assert_ne!(settled.light, UNBAKED);
        let mut destruction = instance(2, 2.0);
        let original = destruction.light;
        shade(&mut destruction, Some(&probes));
        assert_eq!(destruction.light, original);
        let mut changed = probes;
        changed.grid.data.fill([12.0; 12]);
        settled.light = UNBAKED;
        shade(&mut settled, Some(&changed));
        assert_ne!(
            settled.light, moving.light,
            "a late probe delivery changes the group lookup"
        );
    }

    #[test]
    fn pool_keeps_blended_chunks_separate_and_evicts_unused_sources_before_static_budget_overflow()
    {
        use super::super::{ChunkMesh, Look, Pool};
        let vertices = triangle();
        let meshes = vec![vec![ChunkMesh {
            cuboid: None,
            parts: vec![
                (0, vertices.clone()),
                (1, vertices.clone()),
                (2, vertices.clone()),
            ],
        }]];
        let looks = [Look { shape: 0, paint: 0 }; 2];
        let materials = [((0, 0), 0), ((1, 0), 1), ((2, 0), 2)]
            .into_iter()
            .collect();
        let mut pool = Pool::new(scene());
        let epoch = pool.scene.lock().unwrap().motion_epoch.clone();
        let other = Pool::new(scene());
        assert!(!Arc::ptr_eq(
            &epoch,
            &other.scene.lock().unwrap().motion_epoch
        ));
        pool.limit = 18;
        pool.pack(
            vec![0, 1],
            &looks,
            &meshes,
            &materials,
            &|_, _| true,
            BTreeSet::new(),
        );
        assert_eq!(pool.held(), 9, "three shared source parts");
        pool.pack(
            vec![0, 1],
            &looks,
            &meshes,
            &materials,
            &|_, _| true,
            [(0, 0), (1, 0)].into_iter().collect(),
        );
        assert_eq!(pool.spans.len(), 6, "merging loses no visible parts");
        assert_eq!(
            pool.spans.iter().filter(|s| s.mesh == usize::MAX).count(),
            4
        );
        assert_eq!(pool.held(), 3, "only the blended source is still needed");
        assert!(Arc::ptr_eq(
            &epoch,
            &pool.scene.lock().unwrap().motion_epoch
        ));
        let parts: Vec<_> = pool
            .spans
            .iter()
            .filter(|s| s.mesh == usize::MAX)
            .map(|span| Part {
                instance: instance((span.piece * 3 + span.part) as u64, span.piece as f32),
                source: [span.shape, span.chunk, span.part],
                material: span.material,
                vertices: &meshes[span.shape][span.chunk].parts[span.part].1,
            })
            .collect();
        let blended: Vec<_> = pool
            .spans
            .iter()
            .filter(|s| s.mesh != usize::MAX)
            .map(|span| DynamicInstance {
                mesh: span.mesh,
                ..instance((span.piece * 3 + span.part) as u64, span.piece as f32)
            })
            .collect();
        let frame = pool.scene.lock().unwrap().frame(blended, &parts);
        frame.validate().unwrap();
        assert_eq!(
            frame.instances.len(),
            4,
            "two blended instances and two static material groups"
        );
        assert_eq!(
            frame
                .instances
                .iter()
                .filter(|i| frame.scene.meshes[i.mesh].primitives[0].material == 2)
                .count(),
            2
        );
        assert_eq!(pool.held(), 15);
        assert!(pool.held() <= pool.limit);
        pool.pack(
            vec![0, 1],
            &looks,
            &meshes,
            &materials,
            &|_, _| true,
            [(1, 0)].into_iter().collect(),
        );
        assert_eq!(
            pool.spans.len(),
            6,
            "wake preserves every part under the same budget"
        );
        assert_eq!(
            pool.spans.iter().filter(|s| s.mesh == usize::MAX).count(),
            2
        );
        pool.clear();
        assert!(!Arc::ptr_eq(
            &epoch,
            &pool.scene.lock().unwrap().motion_epoch
        ));
        assert_eq!(pool.held(), 0);
        assert!(pool.static_parts.is_empty() && pool.settled.is_empty());
    }
}
