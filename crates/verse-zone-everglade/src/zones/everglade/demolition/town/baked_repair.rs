//! Town repair requests and source geometry retained across renderer merging.

use super::vertex_lights::VertexLights;
use super::{Span, Town};
use crate::pbr::textured::{
    DynamicInstance, InstancedFigure, LightPatch, Primitive, TexturedMesh, TexturedScene,
};
use crate::pbr::textured_bake::{BakeLight, BakeSettings};
use crate::zones::everglade::demolition::repair::{
    RepairPatch, RepairQueue, RepairRequest, RepairTarget,
};
use glam::Vec3;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

type SourceKey = (usize, usize, usize, usize);

struct Sources {
    scene: Arc<TexturedScene>,
    meshes: Arc<Vec<TexturedMesh>>,
    indices: BTreeMap<SourceKey, usize>,
    vertices: usize,
}

impl Sources {
    fn new(kit: &TexturedScene) -> Self {
        // Copy material images once at opt-in. Requests clone these Arcs, and
        // only the worker combines local sources with the material scene.
        let mut scene = kit.clone();
        scene.meshes.clear();
        scene.placements.clear();
        Self {
            scene: Arc::new(scene),
            meshes: Arc::new(Vec::new()),
            indices: BTreeMap::new(),
            vertices: 0,
        }
    }

    fn prepare(&mut self, town: &Town) {
        let needed: BTreeMap<_, _> = town
            .pool
            .spans
            .iter()
            .map(|span| {
                let key = (span.shape, span.chunk, span.part, span.material);
                let vertices = &town.wreck.meshes[span.shape][span.chunk].parts[span.part].1;
                (key, vertices)
            })
            .collect();
        let added: usize = needed
            .iter()
            .filter(|(key, _)| !self.indices.contains_key(key))
            .map(|(_, vertices)| vertices.len())
            .sum();
        if self.vertices + added > town.pool.limit {
            self.meshes = Arc::new(Vec::new());
            self.indices.clear();
            self.vertices = 0;
        }
        for (key, vertices) in needed {
            if self.indices.contains_key(&key) {
                continue;
            }
            self.indices.insert(key, self.meshes.len());
            self.vertices += vertices.len();
            Arc::make_mut(&mut self.meshes).push(TexturedMesh {
                primitives: vec![Primitive {
                    material: key.3,
                    vertices: vertices.clone(),
                    indices: (0..vertices.len() as u32).collect(),
                }],
            });
        }
        debug_assert!(self.vertices <= town.pool.limit);
    }

    fn snapshot(&mut self, town: &Town) -> InstancedFigure {
        self.prepare(town);
        let live: BTreeMap<u64, DynamicInstance> = town
            .pool
            .instances
            .iter()
            .copied()
            .chain(town.pool.static_parts.iter().map(|part| part.instance))
            .map(|instance| (instance.id, instance))
            .collect();
        let mut instances = Vec::with_capacity(live.len());
        for span in &town.pool.spans {
            let Some(id) = instance_id(town, span) else {
                continue;
            };
            let Some(mut instance) = live.get(&id).copied() else {
                continue;
            };
            instance.mesh = self.indices[&(span.shape, span.chunk, span.part, span.material)];
            instances.push(instance);
        }
        InstancedFigure {
            scene: self.scene.clone(),
            instances: Arc::new(instances),
            vertex_lights: None,
            motion_epoch: town
                .pool
                .scene
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .motion_epoch(),
        }
    }
}

fn instance_id(town: &Town, span: &Span) -> Option<u64> {
    let piece = town.wreck.site.pieces().get(span.piece)?;
    let body = if piece.status == super::Status::Broken {
        let chunk = piece.chunks.get(span.chunk)?;
        if chunk.gone {
            return None;
        }
        chunk.body
    } else {
        piece.body
    };
    Some(
        (u64::from(body.0) << 32)
            | ((span.piece as u64) << 16)
            | ((span.chunk as u64) << 8)
            | span.part as u64,
    )
}

pub(super) struct Repair {
    queue: RepairQueue,
    pub lights: VertexLights,
    sources: Sources,
    receivers: super::receivers::Index,
    seen: Option<(u64, u64, u64)>,
    epoch: Option<Arc<()>>,
    minute: Option<u64>,
    revision: u64,
    geometry_epoch: u64,
    generation: u64,
    targets: Arc<[RepairTarget]>,
    membership: Vec<(u64, SourceKey)>,
    error: Option<String>,
}

impl Repair {
    fn new(queue: RepairQueue, town: &Town) -> Self {
        Self {
            queue,
            lights: VertexLights::default(),
            sources: Sources::new(&town.pool.kit),
            receivers: super::receivers::Index::all_directions(
                &town.relight_vertices,
                &town.wreck.buildings,
            ),
            seen: None,
            epoch: None,
            minute: None,
            revision: 0,
            geometry_epoch: 0,
            generation: 0,
            targets: Arc::from([]),
            membership: Vec::new(),
            error: None,
        }
    }

    fn invalidate(&mut self) {
        self.queue.invalidate();
        self.lights.reset();
        self.seen = None;
        self.epoch = None;
        self.minute = None;
        self.targets = Arc::from([]);
        self.membership.clear();
        self.error = None;
    }

    fn request(&mut self, town: &Town, light: BakeLight, minute: u64) {
        let seen = (
            town.wreck.revision,
            town.wreck.site.revision(),
            town.world.edits.revision(),
        );
        let epoch = town
            .pool
            .scene
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .motion_epoch();
        let members = || {
            town.pool.spans.iter().filter_map(|span| {
                instance_id(town, span)
                    .map(|id| (id, (span.shape, span.chunk, span.part, span.material)))
            })
        };
        let geometry = self.seen != Some(seen)
            || !members().eq(self.membership.iter().copied())
            || self
                .epoch
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, &epoch));
        if !geometry && self.minute == Some(minute) {
            return;
        }
        let snapshot = self.sources.snapshot(town);
        if geometry {
            self.geometry_epoch = self.geometry_epoch.wrapping_add(1);
            self.lights.reset();
            // Invalidate first so a buffered old result cannot restore a stale
            // ground shadow while the immediate fallback remains visible.
            self.queue.invalidate();
            town.world.baked.clear_repairs();
            let affected: std::collections::BTreeSet<_> = town
                .wreck
                .refs
                .iter()
                .enumerate()
                .filter(|(piece, _)| town.wreck.site.relight(*piece))
                .map(|(_, &(building, _))| building)
                .collect();
            let mut targets: Vec<_> = self
                .receivers
                .affected(affected)
                .into_iter()
                .map(RepairTarget::Static)
                .collect();
            let affected_ids: std::collections::BTreeSet<_> = town
                .pool
                .spans
                .iter()
                .filter(|span| town.wreck.site.relight(span.piece))
                .filter_map(|span| instance_id(town, span))
                .collect();
            for instance in snapshot
                .instances
                .iter()
                .filter(|instance| affected_ids.contains(&instance.id))
            {
                let count: usize = self.sources.meshes[instance.mesh]
                    .primitives
                    .iter()
                    .map(|primitive| primitive.vertices.len())
                    .sum();
                targets.extend((0..count as u32).map(|vertex| RepairTarget::Chunk {
                    id: instance.id,
                    vertex,
                }));
            }
            targets.sort_unstable();
            targets.dedup();
            self.targets = targets.into();
            self.membership = members().collect();
        }
        self.revision = self.revision.wrapping_add(1);
        let generation = self.queue.request(RepairRequest {
            revision: self.revision,
            geometry_epoch: self.geometry_epoch,
            scene: town.world.clone(),
            instances: Some(snapshot.clone()),
            extra_sources: self.sources.meshes.clone(),
            targets: self.targets.clone(),
            light,
            settings: BakeSettings::new(Vec3::ZERO, Vec3::ONE, 1.0),
            key: super::SEED,
        });
        self.generation = generation;
        self.lights
            .begin_with_sources(generation, &snapshot, &self.sources.meshes);
        self.seen = Some(seen);
        self.epoch = Some(epoch);
        self.minute = Some(minute);
        self.error = None;
    }

    fn poll(&mut self, town: &Town) {
        let Some(batch) = self.queue.poll() else {
            return;
        };
        if batch.revision != self.revision || batch.geometry_epoch != self.geometry_epoch {
            return;
        }
        self.error = batch.error;
        let mut patches = Vec::new();
        for patch in batch.patches {
            match patch {
                RepairPatch::Static { first, lights } => patches.push(LightPatch {
                    first,
                    lights,
                    dynamic: true,
                }),
                RepairPatch::Chunk { id, first, lights } => {
                    self.lights.apply(batch.generation, id, first, &lights);
                }
            }
        }
        if !patches.is_empty() {
            town.world.baked.deliver_patches(patches);
        }
    }
}

impl Town {
    /// Enables immediate ambient fallback and selective vertex repair on one worker.
    ///
    /// # Errors
    /// Returns a scene validation error or a worker startup error.
    pub fn enable_baked_repair(&mut self, light: BakeLight) -> Result<(), String> {
        if self.baked_repair.is_some() {
            return Ok(());
        }
        let queue = RepairQueue::new()?;
        self.invalidate_baked_repair();
        self.set_destruction_relighting(Some(light.sun_dir))?;
        self.pose();
        self.baked_repair = Some(Mutex::new(Repair::new(queue, self)));
        Ok(())
    }

    /// Whether selective repair has an active worker.
    #[must_use]
    pub fn baked_repair_enabled(&self) -> bool {
        self.baked_repair.is_some()
    }

    /// Requests changed geometry or a new clock minute, then applies at most one
    /// bounded batch. Moving occluders use the last requested pose snapshot;
    /// the renderer's dynamic shadows follow their current poses each frame.
    pub fn poll_baked_repair(&mut self, light: BakeLight, minute: u64) {
        if let Some(repair) = &self.baked_repair {
            let mut repair = repair
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            repair.request(self, light, minute);
            repair.poll(self);
        }
    }

    /// The latest repair worker error, if a request failed.
    #[must_use]
    pub fn baked_repair_error(&self) -> Option<String> {
        self.baked_repair.as_ref().and_then(|repair| {
            repair
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .error
                .clone()
        })
    }

    pub(super) fn invalidate_baked_repair(&self) {
        if let Some(repair) = &self.baked_repair {
            repair
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .invalidate();
            self.world.baked.clear_repairs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        Building, ChunkMesh, Debris, Draft, Lifted, Link, Look, Matter, Role, TownPiece,
    };
    use super::*;
    use crate::pbr::textured::{TexturedMaterial, TexturedVertex};
    use crate::zones::everglade::demolition::repair::RepairBatch;
    use crate::zones::everglade::demolition::site::{Cuboid, Site};
    use crate::zones::everglade_pack::ZonePack;
    use glam::{DVec3, Mat4};
    use std::sync::mpsc::SyncSender;

    fn light() -> BakeLight {
        BakeLight {
            sun_dir: Vec3::Y,
            sun_illuminance: 100.0,
            sky: 20.0,
            ground: 2.0,
        }
    }

    fn triangle(normal: Vec3) -> Vec<TexturedVertex> {
        [Vec3::ZERO, Vec3::X, Vec3::Z]
            .map(|point| TexturedVertex::new(point, normal, [0.0; 2]))
            .to_vec()
    }

    fn fixture() -> (Town, SyncSender<RepairBatch>) {
        fixture_chunks(1)
    }

    fn fixture_chunks(count: usize) -> (Town, SyncSender<RepairBatch>) {
        let pack = ZonePack {
            textures: Vec::new(),
            materials: Vec::new(),
            models: Vec::new(),
            character: None,
            forms: Vec::new(),
        };
        let mut world = TexturedScene::default();
        world.add_material(TexturedMaterial::default());
        world.add_mesh(TexturedMesh {
            primitives: vec![Primitive {
                material: 0,
                vertices: triangle(Vec3::Y),
                indices: vec![0, 1, 2],
            }],
        });
        world.place(0, Mat4::IDENTITY);
        world.baked.deliver_lights(vec![[100; 4]; 3]);
        let mut town = Town::standalone(&pack, &[], Arc::new(world)).unwrap();
        let cuboid = Cuboid::between(DVec3::splat(-0.5), DVec3::splat(0.5));
        let draft = Draft {
            building: 0,
            role: Role::Block { level: 0 },
            matter: Matter::Brick,
            placement: Mat4::from_translation(Vec3::Y),
            models: Vec::new(),
            colliders: vec![cuboid],
            origin: Vec3::ZERO,
            mass: 2.0,
            hit_points: 10,
        };
        let cuboids: Vec<_> = (0..count)
            .map(|i| {
                Cuboid::between(
                    DVec3::new(-0.5 + i as f64 / count as f64, -0.5, -0.5),
                    DVec3::new(-0.5 + (i + 1) as f64 / count as f64, 0.5, 0.5),
                )
            })
            .collect();
        let mut spec = draft.spec(cuboids.clone());
        spec.link.footing = true;
        town.wreck.site = Site::new(vec![spec], 1);
        town.wreck.site.set_ground(180.0, Vec::new());
        town.wreck.refs = vec![(0, 0)];
        town.wreck.lifted = vec![Lifted {
            building: 0,
            since: 0.0,
            hit: 0.0,
        }];
        town.wreck.debris = Debris::TOWN;
        town.wreck.buildings = vec![Building {
            rect: ([0.0; 2], [1.0; 2]),
            stories: 1,
            valid: true,
            carved: Vec::new(),
            ready: true,
            base: 0.0,
            top: 2.0,
            blocks: Vec::new(),
            roofs: Vec::new(),
            pieces: vec![TownPiece {
                draft,
                placements: Vec::new(),
                look: Look { shape: 0, paint: 0 },
                roof: None,
                link: Link {
                    footing: true,
                    ..Default::default()
                },
                carve: None,
                cut: true,
            }],
        }];
        town.wreck.cuboids = vec![cuboids.clone()];
        town.wreck.meshes = vec![
            cuboids
                .into_iter()
                .map(|cuboid| ChunkMesh {
                    cuboid: Some(cuboid),
                    parts: vec![(0, triangle(Vec3::Y))],
                })
                .collect(),
        ];
        let mut kit = TexturedScene::default();
        kit.add_material(TexturedMaterial::default());
        town.pool = super::super::Pool::new(kit);
        town.materials.insert((0, 0), 0);
        town.set_destruction_relighting(Some(Vec3::Y)).unwrap();
        let (queue, send) = RepairQueue::channel();
        town.baked_repair = Some(Mutex::new(Repair::new(queue, &town)));
        assert!(town.wreck.site.damage(0, 10, DVec3::Y, DVec3::ZERO));
        town.sync();
        town.pose();
        (town, send)
    }

    fn response(town: &Town, lights: [u8; 4]) -> RepairBatch {
        let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
        RepairBatch {
            generation: repair.generation,
            revision: repair.revision,
            geometry_epoch: repair.geometry_epoch,
            patches: vec![
                RepairPatch::Static {
                    first: 0,
                    lights: vec![lights; 3],
                },
                RepairPatch::Chunk {
                    id: town.pool.instances[0].id,
                    first: 0,
                    lights: vec![lights; 3],
                },
            ],
            processed: 6,
            skipped: 0,
            complete: true,
            error: None,
        }
    }

    #[test]
    fn clock_requests_reuse_sources_and_keep_complete_targets_and_repaired_light() {
        let (mut town, send) = fixture();
        town.poll_baked_repair(light(), 10);
        let (source, scene, epoch, generation, targets) = {
            let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
            assert_eq!(
                repair.targets.len(),
                6,
                "ground and all affected chunk vertices"
            );
            (
                repair.sources.meshes.clone(),
                repair.sources.scene.clone(),
                repair.geometry_epoch,
                repair.generation,
                repair.targets.clone(),
            )
        };
        send.try_send(response(&town, [7, 9, 11, 177])).unwrap();
        town.poll_baked_repair(light(), 10);
        assert_eq!(
            town.world.baked.take_patches()[0].lights,
            [[7, 9, 11, 177]; 3]
        );
        let repaired = town.instances(None).unwrap().vertex_lights.unwrap();
        assert_eq!(&*repaired.texels, &[[7, 9, 11, 177]; 3]);
        let mut next_light = light();
        next_light.sky = 30.0;
        town.poll_baked_repair(next_light, 10);
        assert_eq!(
            town.baked_repair
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .generation,
            generation
        );
        town.poll_baked_repair(next_light, 11);
        let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
        assert!(Arc::ptr_eq(&source, &repair.sources.meshes));
        assert!(Arc::ptr_eq(&scene, &repair.sources.scene));
        assert!(Arc::ptr_eq(&targets, &repair.targets));
        assert_eq!(repair.geometry_epoch, epoch);
        assert!(repair.generation > generation);
        drop(repair);
        let kept = town.instances(None).unwrap().vertex_lights.unwrap();
        assert!(
            Arc::ptr_eq(&repaired.texels, &kept.texels),
            "clock begin changes no rendered texels"
        );
    }

    #[test]
    fn evicted_settled_sources_keep_individual_light_and_restore_rejects_buffered_results() {
        let (mut town, send) = fixture();
        town.poll_baked_repair(light(), 1);
        send.try_send(response(&town, [17; 4])).unwrap();
        town.poll_baked_repair(light(), 1);
        let stale = response(&town, [99; 4]);
        let mut instance = town.pool.instances.remove(0);
        instance.mesh = usize::MAX;
        instance.settled = true;
        town.pool.spans[0].mesh = usize::MAX;
        town.pool.static_parts.push(super::super::SettledPart {
            instance,
            source: [0, 0, 0],
            material: 0,
        });
        town.pool
            .scene
            .lock()
            .unwrap()
            .reset_geometry(town.pool.kit.clone());
        town.pool.meshes.clear();
        town.poll_baked_repair(light(), 2);
        let merged = town.instances(None).unwrap();
        assert_eq!(&*merged.vertex_lights.unwrap().texels, &[[17; 4]; 3]);
        let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
        let snapshot_meshes = repair.sources.meshes.clone();
        assert_eq!(
            snapshot_meshes.len(),
            1,
            "source survives renderer eviction"
        );
        assert_eq!(repair.targets.len(), 6);
        drop(repair);
        send.try_send(stale).unwrap();
        town.restore();
        assert!(town.world.baked.take_patches().is_empty());
        town.poll_baked_repair(light(), 2);
        assert!(town.instances(None).is_none());
        assert!(
            town.baked_repair
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .targets
                .is_empty()
        );
        assert_eq!(
            town.world.baked.take().unwrap(),
            [[100; 4]; 3],
            "pristine light returns"
        );
    }

    #[test]
    fn structural_retain_resets_identity_before_reused_chunk_ids_receive_any_light() {
        let (mut town, send) = fixture();
        town.poll_baked_repair(light(), 1);
        let stale = response(&town, [99; 4]);
        let epoch = town.instances(None).unwrap().motion_epoch;
        let generation = stale.generation;
        town.wreck.site.retain(|_| true);
        town.wreck.revision += 1;
        town.sync();
        town.pose();
        let retained = town.instances(None).unwrap();
        assert!(!Arc::ptr_eq(&epoch, &retained.motion_epoch));
        assert!(retained.vertex_lights.is_none());
        send.try_send(stale).unwrap();
        town.poll_baked_repair(light(), 1);
        assert!(town.world.baked.take_patches().is_empty());
        assert!(town.instances(None).unwrap().vertex_lights.is_none());
        let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
        assert!(repair.generation > generation);
        assert_eq!(
            repair.targets.len(),
            6,
            "retained changed pieces remain repair targets"
        );
    }
    #[test]
    fn partial_retirement_rebuilds_targets_even_without_a_site_revision_or_identity_change() {
        let (mut town, send) = fixture_chunks(2);
        town.poll_baked_repair(light(), 1);
        let stale = response(&town, [99; 4]);
        let removed = town.pool.instances[0].id;
        let epoch = town.instances(None).unwrap().motion_epoch;
        let site_revision = town.wreck.site.revision();
        let geometry_epoch = stale.geometry_epoch;
        town.wreck.site.retire_chunks(&[(0, 0)]);
        town.sync();
        town.pose();
        assert_eq!(
            town.wreck.site.revision(),
            site_revision,
            "retirement is a membership edit"
        );
        assert!(Arc::ptr_eq(
            &epoch,
            &town.instances(None).unwrap().motion_epoch
        ));
        send.try_send(stale).unwrap();
        town.poll_baked_repair(light(), 1);
        assert!(town.world.baked.take_patches().is_empty());
        let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
        assert!(repair.geometry_epoch > geometry_epoch);
        assert_eq!(
            repair.targets.len(),
            6,
            "ground and the remaining chunk stay requested"
        );
        assert!(
            !repair
                .targets
                .iter()
                .any(|target| matches!(target, RepairTarget::Chunk { id, .. } if *id == removed))
        );
        assert!(repair.sources.vertices <= town.pool.limit);
    }
}
