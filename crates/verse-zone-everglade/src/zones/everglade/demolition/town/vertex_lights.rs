//! Repaired chunk light, retained independently of rigid geometry and poses.

use super::rubble::LightMember;
use crate::pbr::textured::{InstancedFigure, TexturedMesh, TexturedScene, VertexLightStream};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const MAX_PATCH_SAMPLES: usize = 128;

struct Entry {
    mesh: usize,
    fallback: [u8; 4],
    texels: Vec<[u8; 4]>,
    repaired: Vec<bool>,
    has_repair: bool,
}

impl Entry {
    fn new(mesh: usize, count: usize, fallback: [u8; 4]) -> Self {
        Self {
            mesh,
            fallback,
            texels: vec![fallback; count],
            repaired: vec![false; count],
            has_repair: false,
        }
    }

    fn overlay(&self, destination: &mut [[u8; 4]]) {
        for ((destination, light), &repaired) in
            destination.iter_mut().zip(&self.texels).zip(&self.repaired)
        {
            if repaired {
                *destination = *light;
            }
        }
    }
}

struct FrameCache {
    generation: u64,
    scene: Arc<TexturedScene>,
    epoch: Arc<()>,
    membership: Vec<(u64, usize, [u8; 4])>,
    stream: Option<VertexLightStream>,
}

#[derive(Default)]
pub(super) struct VertexLights {
    entries: BTreeMap<u64, Entry>,
    sample_generation: u64,
    active: bool,
    data_generation: u64,
    epoch: Option<Arc<()>>,
    frame: Option<FrameCache>,
}

impl VertexLights {
    /// Tracks individual chunks, including settled members absent from the
    /// renderer's aggregate instance list. Shade their ambient before this call.
    /// Use the generation returned by the repair queue, rather than a site revision.
    pub fn begin(&mut self, generation: u64, snapshot: &InstancedFigure) -> bool {
        self.begin_with_sources(generation, snapshot, &[])
    }

    /// Separate source meshes follow the shared scene's mesh indices. This
    /// avoids copying the scene's images to append evicted settled sources.
    pub fn begin_with_sources(
        &mut self,
        generation: u64,
        snapshot: &InstancedFigure,
        sources: &[TexturedMesh],
    ) -> bool {
        if generation <= self.sample_generation {
            return false;
        }
        self.sample_generation = generation;
        self.active = true;
        let mut changed = false;
        if self
            .epoch
            .as_ref()
            .is_none_or(|epoch| !Arc::ptr_eq(epoch, &snapshot.motion_epoch))
        {
            changed = !self.entries.is_empty();
            self.entries.clear();
            self.epoch = Some(snapshot.motion_epoch.clone());
        }
        let mut ids = BTreeSet::new();
        for instance in snapshot.instances.iter() {
            let count = if instance.mesh < snapshot.scene.meshes.len() {
                vertex_count(&snapshot.scene, instance.mesh)
            } else {
                sources
                    .get(instance.mesh - snapshot.scene.meshes.len())
                    .map(|mesh| {
                        mesh.primitives
                            .iter()
                            .map(|primitive| primitive.vertices.len())
                            .sum()
                    })
            };
            let Some(count) = count else {
                continue;
            };
            ids.insert(instance.id);
            let entry = self.entries.entry(instance.id).or_insert_with(|| {
                changed = true;
                Entry::new(instance.mesh, count, instance.light)
            });
            if entry.mesh != instance.mesh || entry.texels.len() != count {
                *entry = Entry::new(instance.mesh, count, instance.light);
                changed = true;
            } else if entry.fallback != instance.light {
                entry.fallback = instance.light;
                for (texel, &repaired) in entry.texels.iter_mut().zip(&entry.repaired) {
                    if !repaired {
                        *texel = instance.light;
                    }
                }
                changed = true;
            }
        }
        let previous = self.entries.len();
        self.entries.retain(|id, _| ids.contains(id));
        changed |= previous != self.entries.len();
        if changed {
            self.changed();
        }
        true
    }

    /// Applies one bounded chunk patch. Reject stale generations and invalid
    /// ranges atomically, so an expired or reused ID cannot receive old light.
    pub fn apply(&mut self, generation: u64, id: u64, first: u32, lights: &[[u8; 4]]) -> bool {
        if !self.active || generation != self.sample_generation || lights.len() > MAX_PATCH_SAMPLES
        {
            return false;
        }
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        let first = first as usize;
        let Some(end) = first.checked_add(lights.len()) else {
            return false;
        };
        if end > entry.texels.len() {
            return false;
        }
        let mut changed = false;
        for ((texel, repaired), light) in entry.texels[first..end]
            .iter_mut()
            .zip(&mut entry.repaired[first..end])
            .zip(lights)
        {
            changed |= *texel != *light || !*repaired;
            *texel = *light;
            *repaired = true;
        }
        entry.has_repair |= !lights.is_empty();
        if changed {
            self.changed();
        }
        true
    }

    /// Clear on restoration or structural ID reuse. Keep the generation floor
    /// even when a restored site starts its own revisions again at zero.
    pub fn reset(&mut self) {
        self.active = false;
        self.entries.clear();
        self.epoch = None;
        self.changed();
    }

    /// Ordinary frames scan instance membership and clone the retained stream.
    /// A repair or aggregate change copies only light texels; geometry stays shared.
    pub fn stream(
        &mut self,
        frame: &InstancedFigure,
        members: impl IntoIterator<Item = LightMember>,
    ) -> Option<VertexLightStream> {
        if self
            .epoch
            .as_ref()
            .is_some_and(|epoch| !Arc::ptr_eq(epoch, &frame.motion_epoch))
        {
            return None;
        }
        if let Some(cached) = &self.frame
            && cached.generation == self.data_generation
            && Arc::ptr_eq(&cached.scene, &frame.scene)
            && Arc::ptr_eq(&cached.epoch, &frame.motion_epoch)
            && cached.membership.len() == frame.instances.len()
            && cached.membership.iter().zip(frame.instances.iter()).all(
                |(&(id, mesh, light), instance)| {
                    (id, mesh, light) == (instance.id, instance.mesh, instance.light)
                },
            )
        {
            return cached.stream.clone();
        }
        let membership = frame
            .instances
            .iter()
            .map(|instance| (instance.id, instance.mesh, instance.light))
            .collect();
        let mut aggregates: BTreeMap<usize, Vec<LightMember>> = BTreeMap::new();
        for member in members {
            aggregates.entry(member.mesh).or_default().push(member);
        }
        let mut texels = Vec::new();
        let mut ranges = BTreeMap::new();
        for instance in frame.instances.iter() {
            let Some(count) = vertex_count(&frame.scene, instance.mesh) else {
                continue;
            };
            let Some(end) = texels.len().checked_add(count) else {
                return None;
            };
            if end > u32::MAX as usize {
                return None;
            }
            let base = texels.len() as u32;
            if let Some(members) = aggregates.get(&instance.mesh) {
                // Match the actual aggregate mesh, not its provisional MAX ID.
                let repaired: Vec<_> = members
                    .iter()
                    .filter_map(|member| {
                        self.entries
                            .get(&member.id)
                            .filter(|entry| {
                                entry.has_repair
                                    && member.vertices.start <= member.vertices.end
                                    && member.vertices.end <= count
                                    && entry.texels.len() == member.vertices.len()
                            })
                            .map(|entry| (member, entry))
                    })
                    .collect();
                if repaired.is_empty() {
                    continue;
                }
                texels.resize(end, instance.light);
                for (member, entry) in repaired {
                    let start = base as usize + member.vertices.start;
                    entry.overlay(&mut texels[start..start + entry.texels.len()]);
                }
            } else if let Some(entry) = self
                .entries
                .get(&instance.id)
                .filter(|entry| entry.has_repair && entry.texels.len() == count)
            {
                texels.resize(end, instance.light);
                entry.overlay(&mut texels[base as usize..end]);
            } else {
                continue;
            }
            ranges.insert(instance.id, base);
        }
        let stream = (!ranges.is_empty()).then(|| VertexLightStream {
            texels: Arc::new(texels),
            ranges: Arc::new(ranges),
        });
        self.frame = Some(FrameCache {
            generation: self.data_generation,
            scene: frame.scene.clone(),
            epoch: frame.motion_epoch.clone(),
            membership,
            stream: stream.clone(),
        });
        stream
    }

    fn changed(&mut self) {
        self.data_generation = self
            .data_generation
            .checked_add(1)
            .expect("light generation overflow");
        self.frame = None;
    }
}

fn vertex_count(scene: &TexturedScene, mesh: usize) -> Option<usize> {
    scene
        .meshes
        .get(mesh)?
        .primitives
        .iter()
        .try_fold(0usize, |count, primitive| {
            count.checked_add(primitive.vertices.len())
        })
}

#[cfg(test)]
mod tests {
    use super::super::rubble::{Cache, Part};
    use super::*;
    use crate::pbr::textured::{
        DynamicInstance, Primitive, TexturedMaterial, TexturedMesh, TexturedVertex,
    };
    use glam::{Mat4, Vec3};

    const AMBIENT: [u8; 4] = [71, 83, 109, 201];
    const REPAIRED: [u8; 4] = [5, 9, 13, 177];

    fn triangle() -> Vec<TexturedVertex> {
        [Vec3::ZERO, Vec3::X, Vec3::Y]
            .into_iter()
            .map(|point| TexturedVertex::new(point, Vec3::Z, [0.0; 2]))
            .collect()
    }

    fn scene(primitives: usize) -> TexturedScene {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial::default());
        scene.add_mesh(TexturedMesh {
            primitives: (0..primitives)
                .map(|_| Primitive {
                    vertices: triangle(),
                    indices: vec![0, 1, 2],
                    material: 0,
                })
                .collect(),
        });
        scene
    }

    fn instance(id: u64) -> DynamicInstance {
        DynamicInstance {
            id,
            mesh: 0,
            current: Mat4::IDENTITY,
            previous: Mat4::IDENTITY,
            color: [1.0; 4],
            light: AMBIENT,
            settled: false,
        }
    }

    fn snapshot(
        scene: Arc<TexturedScene>,
        instances: Vec<DynamicInstance>,
        epoch: Arc<()>,
    ) -> InstancedFigure {
        InstancedFigure {
            scene,
            instances: Arc::new(instances),
            vertex_lights: None,
            motion_epoch: epoch,
        }
    }

    #[test]
    fn partial_moving_patch_keeps_exact_ambient_across_primitive_boundaries_and_rejects_bad_ranges()
    {
        let frame = snapshot(Arc::new(scene(2)), vec![instance(7)], Arc::new(()));
        let mut lights = VertexLights::default();
        assert!(lights.begin(1, &frame));
        assert!(lights.stream(&frame, []).is_none());
        assert!(lights.apply(1, 7, 2, &[REPAIRED; 2]));
        let first = lights.stream(&frame, []).unwrap();
        assert_eq!(first.ranges[&7], 0);
        assert_eq!(
            *first.texels,
            [AMBIENT, AMBIENT, REPAIRED, REPAIRED, AMBIENT, AMBIENT]
        );
        for (generation, id, offset, patch) in [
            (0, 7, 0, vec![REPAIRED]),
            (1, 8, 0, vec![REPAIRED]),
            (1, 7, 6, vec![REPAIRED]),
            (1, 7, u32::MAX, vec![REPAIRED]),
            (1, 7, 0, vec![REPAIRED; MAX_PATCH_SAMPLES + 1]),
        ] {
            assert!(!lights.apply(generation, id, offset, &patch));
        }
        assert!(lights.apply(1, 7, 2, &[REPAIRED; 2]));
        let unchanged = lights.stream(&frame, []).unwrap();
        assert!(Arc::ptr_eq(&first.texels, &unchanged.texels));
        assert!(Arc::ptr_eq(&first.ranges, &unchanged.ranges));
        let mut lit_frame = frame;
        lit_frame.vertex_lights = Some(first);
        lit_frame.validate().unwrap();
    }

    #[test]
    fn generations_expiry_mesh_changes_and_restore_cannot_revive_reused_ids() {
        let source = Arc::new(scene(1));
        let epoch = Arc::new(());
        let frame = snapshot(
            source.clone(),
            vec![instance(4), instance(5)],
            epoch.clone(),
        );
        let mut lights = VertexLights::default();
        assert!(lights.begin(1, &frame));
        assert!(lights.apply(1, 5, 0, &[REPAIRED]));
        let expired = snapshot(source.clone(), vec![instance(4)], epoch.clone());
        assert!(lights.begin(2, &expired));
        assert!(!lights.apply(1, 5, 0, &[REPAIRED]));
        assert!(!lights.apply(2, 5, 0, &[REPAIRED]));
        assert!(lights.stream(&expired, []).is_none());
        assert!(lights.begin(3, &frame));
        assert!(lights.stream(&frame, []).is_none());
        assert!(lights.apply(3, 5, 0, &[REPAIRED]));
        lights.reset();
        assert!(!lights.apply(3, 5, 0, &[REPAIRED]));
        assert!(!lights.begin(3, &frame));
        assert!(lights.begin(4, &frame));
        assert!(lights.stream(&frame, []).is_none());
        assert!(lights.apply(4, 5, 0, &[REPAIRED]));
        let resized = snapshot(Arc::new(scene(2)), vec![instance(5)], epoch);
        assert!(lights.begin(5, &resized));
        assert!(lights.stream(&resized, []).is_none());
        assert!(lights.apply(5, 5, 5, &[REPAIRED]));
        let reused = snapshot(source, vec![instance(5)], Arc::new(()));
        assert!(lights.stream(&reused, []).is_none());
        assert!(lights.begin(6, &reused));
        assert!(lights.stream(&reused, []).is_none());
    }

    #[test]
    fn poses_and_noop_generations_reuse_arcs_but_fallback_changes_only_unrepaired_texels() {
        let frame = snapshot(Arc::new(scene(1)), vec![instance(8)], Arc::new(()));
        let mut lights = VertexLights::default();
        lights.begin(1, &frame);
        lights.apply(1, 8, 1, &[REPAIRED]);
        let first = lights.stream(&frame, []).unwrap();
        let mut moving = frame.clone();
        Arc::make_mut(&mut moving.instances)[0].current = Mat4::from_translation(Vec3::X);
        lights.begin(2, &moving);
        let second = lights.stream(&moving, []).unwrap();
        assert!(Arc::ptr_eq(&first.texels, &second.texels));
        let fallback = [17, 29, 31, 193];
        Arc::make_mut(&mut moving.instances)[0].light = fallback;
        let delivered = lights.stream(&moving, []).unwrap();
        assert_eq!(*delivered.texels, [fallback, REPAIRED, fallback]);
        lights.begin(3, &moving);
        let updated = lights.stream(&moving, []).unwrap();
        assert_eq!(*updated.texels, [fallback, REPAIRED, fallback]);
        assert!(!Arc::ptr_eq(&first.texels, &updated.texels));
        assert!(Arc::ptr_eq(&frame.scene, &moving.scene));
    }

    #[test]
    fn merged_members_map_actual_aggregate_ids_and_light_updates_never_rebuild_or_pose_geometry() {
        let vertices = triangle();
        let parts = [1, 2].map(|id| Part {
            instance: DynamicInstance {
                settled: true,
                ..instance(id)
            },
            source: [0, 0, 0],
            material: 0,
            vertices: &vertices,
        });
        let mut cache = Cache::new(scene(1));
        let frame = cache.frame(vec![instance(u64::MAX)], &parts);
        let stats = cache.stats;
        let aggregate = frame
            .instances
            .iter()
            .find(|instance| instance.mesh == 1)
            .unwrap();
        assert_eq!(aggregate.id, u64::MAX - 1);
        let individuals = snapshot(
            Arc::new(scene(1)),
            vec![instance(1), instance(2), instance(u64::MAX)],
            frame.motion_epoch.clone(),
        );
        let mut lights = VertexLights::default();
        lights.begin(1, &individuals);
        lights.apply(1, 1, 1, &[REPAIRED]);
        lights.apply(1, u64::MAX, 2, &[REPAIRED]);
        let first = lights.stream(&frame, cache.light_members()).unwrap();
        let base = first.ranges[&aggregate.id] as usize;
        assert_eq!(
            first.texels[base..base + 6],
            [AMBIENT, REPAIRED, AMBIENT, AMBIENT, AMBIENT, AMBIENT]
        );
        let moving_base = first.ranges[&u64::MAX] as usize;
        assert_eq!(
            first.texels[moving_base..moving_base + 3],
            [AMBIENT, AMBIENT, REPAIRED]
        );
        let mut lit = frame.clone();
        lit.vertex_lights = Some(first.clone());
        lit.validate().unwrap();
        let repeated = cache.frame(vec![instance(u64::MAX)], &parts);
        let unchanged = lights.stream(&repeated, cache.light_members()).unwrap();
        assert!(Arc::ptr_eq(&first.texels, &unchanged.texels));
        assert!(Arc::ptr_eq(&first.ranges, &unchanged.ranges));
        lights.apply(1, 2, 0, &[REPAIRED]);
        let updated = lights.stream(&repeated, cache.light_members()).unwrap();
        assert!(!Arc::ptr_eq(&first.texels, &updated.texels));
        assert_eq!(updated.texels[base + 3], REPAIRED);
        assert!(Arc::ptr_eq(&frame.scene, &repeated.scene));
        assert_eq!(cache.stats, stats);
        cache.add_source(TexturedMesh {
            primitives: vec![Primitive {
                vertices: vertices.clone(),
                indices: vec![0, 1, 2],
                material: 0,
            }],
        });
        let shifted = cache.frame(vec![instance(u64::MAX)], &parts);
        let mapped = lights.stream(&shifted, cache.light_members()).unwrap();
        let aggregate = shifted
            .instances
            .iter()
            .find(|instance| instance.mesh == 2)
            .unwrap();
        assert_eq!(
            mapped.texels[mapped.ranges[&aggregate.id] as usize + 1],
            REPAIRED
        );
        assert_eq!(cache.stats.transformed_vertices, stats.transformed_vertices);
    }
}
