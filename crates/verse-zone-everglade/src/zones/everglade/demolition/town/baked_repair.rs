//! Town repair requests and source geometry retained across renderer merging.

use super::vertex_lights::VertexLights;
use super::{Span, Town};
use crate::pbr::textured::{
    DynamicInstance, InstancedFigure, LightPatch, Primitive, TexturedMesh, TexturedScene, UNBAKED,
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
            .filter(|(key, _)| !self.indices.contains_key(*key))
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

/// Valid worker deliveries and applied texels, counted across all generations.
/// Repeated repair of the same vertex counts again. Sweep progress can mix
/// clock revisions; completion requires a full sweep at the latest clock light.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct BakedRepairDiagnostics {
    pub enabled: bool,
    pub active: bool,
    pub generation: u64,
    pub geometry_epoch: u64,
    pub clock_minute: Option<u64>,
    pub clock_revision: u64,
    pub delivered_clock_minute: Option<u64>,
    pub delivered_clock_revision: u64,
    pub delivered_sun_direction: [f32; 3],
    pub delivered_sun_lux: f32,
    pub delivered_sky_lux: f32,
    pub delivered_ground_lux: f32,
    pub sun_direction: [f32; 3],
    pub sun_lux: f32,
    pub sky_lux: f32,
    pub ground_lux: f32,
    pub requests: u64,
    pub geometry_requests: u64,
    pub clock_requests: u64,
    pub invalidations: u64,
    pub delivered_batches: u64,
    pub delivered_vertices: u64,
    pub processed_vertices: u64,
    pub skipped_vertices: u64,
    pub applied_batches: u64,
    pub applied_static_vertices: u64,
    pub applied_chunk_vertices: u64,
    pub rejected_chunk_vertices: u64,
    pub completed_generations: u64,
    pub completed_geometry_generations: u64,
    pub completed_clock_generations: u64,
    pub last_completed_generation: u64,
    pub last_completed_clock_revision: u64,
    pub last_completed_clock_minute: Option<u64>,
    pub completed_sweeps: u64,
    pub current_sweep: u64,
    pub current_mixed_clock: bool,
    pub current_geometry: bool,
    pub current_targets: usize,
    pub current_processed: usize,
    pub current_skipped: usize,
    pub current_applied_vertices: usize,
    pub current_backlog: usize,
    pub current_complete: bool,
    pub error_count: u64,
    pub error: Option<String>,
}

pub(super) struct Repair {
    queue: RepairQueue,
    pub lights: VertexLights,
    sources: Sources,
    receivers: super::receivers::Index,
    affected_buildings: Option<std::collections::BTreeSet<usize>>,
    ground: Arc<Vec<u32>>,
    published_fallback: Option<(Arc<Vec<u32>>, Arc<Vec<(u32, [u8; 4])>>)>,
    seen: Option<(u64, u64, u64)>,
    epoch: Option<Arc<()>>,
    minute: Option<u64>,
    revision: u64,
    clock_revision: u64,
    geometry_completed: bool,
    geometry_epoch: u64,
    generation: u64,
    targets: Arc<[RepairTarget]>,
    membership: Vec<(u64, SourceKey, bool)>,
    error: Option<String>,
    diagnostics: BakedRepairDiagnostics,
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
            affected_buildings: None,
            ground: Arc::new(Vec::new()),
            published_fallback: None,
            seen: None,
            epoch: None,
            minute: None,
            revision: 0,
            clock_revision: 0,
            geometry_completed: false,
            geometry_epoch: 0,
            generation: 0,
            targets: Arc::from([]),
            membership: Vec::new(),
            error: None,
            diagnostics: BakedRepairDiagnostics {
                enabled: true,
                ..Default::default()
            },
        }
    }

    fn invalidate(&mut self) {
        self.queue.invalidate();
        self.lights.reset();
        self.diagnostics.invalidations = self.diagnostics.invalidations.saturating_add(1);
        self.diagnostics.active = false;
        self.diagnostics.current_targets = 0;
        self.diagnostics.current_processed = 0;
        self.diagnostics.current_skipped = 0;
        self.diagnostics.current_applied_vertices = 0;
        self.diagnostics.current_backlog = 0;
        self.diagnostics.current_complete = false;
        self.diagnostics.current_sweep = 0;
        self.diagnostics.current_mixed_clock = false;
        self.geometry_completed = false;
        self.diagnostics.error = None;
        self.affected_buildings = None;
        self.ground = Arc::new(Vec::new());
        self.published_fallback = None;
        self.seen = None;
        self.epoch = None;
        self.minute = None;
        self.targets = Arc::from([]);
        self.membership.clear();
        self.error = None;
    }

    fn refresh_ground(&mut self, town: &Town) {
        let affected = town.wreck.relight_buildings();
        if self.affected_buildings.as_ref() == Some(&affected) {
            return;
        }
        self.ground = Arc::new(self.receivers.affected(affected.iter().copied()));
        self.affected_buildings = Some(affected);
    }

    fn publish_fallback(&mut self, town: &Town) {
        let sampled = town
            .relight_fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if self
            .published_fallback
            .as_ref()
            .is_some_and(|(ground, old)| {
                Arc::ptr_eq(ground, &self.ground) && Arc::ptr_eq(old, &sampled)
            })
        {
            return;
        }
        // Both inputs are sorted. Sampled local openness wins overlaps;
        // conservative receivers otherwise bypass pristine sun and lamps
        // with neutral ambient until a selective worker patch arrives.
        let mut fallback = Vec::with_capacity(self.ground.len() + sampled.len());
        let mut ground = self.ground.iter().copied().peekable();
        for &(index, light) in sampled.iter() {
            while ground.peek().is_some_and(|&next| next < index) {
                fallback.push((ground.next().unwrap(), UNBAKED));
            }
            if ground.peek() == Some(&index) {
                ground.next();
            }
            fallback.push((index, light));
        }
        fallback.extend(ground.map(|index| (index, UNBAKED)));
        town.world.baked.set_fallback(fallback);
        self.published_fallback = Some((self.ground.clone(), sampled));
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
                instance_id(town, span).map(|id| {
                    let piece = &town.wreck.site.pieces()[span.piece];
                    let settled = piece.status == super::Status::Broken
                        && piece.chunks[span.chunk].settled.is_some();
                    (
                        id,
                        (span.shape, span.chunk, span.part, span.material),
                        settled,
                    )
                })
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
            self.diagnostics.invalidations = self.diagnostics.invalidations.saturating_add(1);
            town.world.baked.clear_repairs();
            self.refresh_ground(town);
            self.publish_fallback(town);
            let mut targets: Vec<_> = self
                .ground
                .iter()
                .copied()
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
        self.clock_revision = self.clock_revision.wrapping_add(1);
        let generation = if geometry {
            self.revision = self.revision.wrapping_add(1);
            self.geometry_completed = false;
            self.queue.request(RepairRequest {
                revision: self.revision,
                geometry_epoch: self.geometry_epoch,
                clock_revision: self.clock_revision,
                clock_minute: minute,
                scene: town.world.clone(),
                instances: Some(snapshot.clone()),
                extra_sources: self.sources.meshes.clone(),
                targets: self.targets.clone(),
                light,
                settings: BakeSettings::new(Vec3::ZERO, Vec3::ONE, 1.0),
                key: super::SEED,
            })
        } else {
            self.queue
                .update_light(self.clock_revision, minute, light, Some(snapshot.clone()));
            self.generation
        };
        self.generation = generation;
        let diagnostics = &mut self.diagnostics;
        diagnostics.active = true;
        diagnostics.generation = generation;
        diagnostics.geometry_epoch = self.geometry_epoch;
        diagnostics.clock_minute = Some(minute);
        diagnostics.clock_revision = self.clock_revision;
        diagnostics.sun_direction = light.sun_dir.to_array();
        diagnostics.sun_lux = light.sun_illuminance;
        diagnostics.sky_lux = light.sky;
        diagnostics.ground_lux = light.ground;
        diagnostics.requests = diagnostics.requests.saturating_add(1);
        if geometry {
            diagnostics.geometry_requests = diagnostics.geometry_requests.saturating_add(1);
        } else {
            diagnostics.clock_requests = diagnostics.clock_requests.saturating_add(1);
        }
        if geometry {
            diagnostics.current_geometry = true;
            diagnostics.current_targets = self.targets.len();
            diagnostics.current_sweep = 0;
            diagnostics.current_mixed_clock = false;
            diagnostics.current_processed = 0;
            diagnostics.current_skipped = 0;
            diagnostics.current_applied_vertices = 0;
            diagnostics.current_backlog = self.targets.len();
            diagnostics.error = None;
            self.lights
                .begin_with_sources(generation, &snapshot, &self.sources.meshes);
            self.error = None;
        }
        diagnostics.current_complete = self.targets.is_empty();
        self.seen = Some(seen);
        self.epoch = Some(epoch);
        self.minute = Some(minute);
    }

    fn poll(&mut self, town: &Town) {
        let Some(batch) = self.queue.poll() else {
            return;
        };
        if batch.revision != self.revision || batch.geometry_epoch != self.geometry_epoch {
            return;
        }
        self.error = batch.error;
        let diagnostics = &mut self.diagnostics;
        if batch.sweep != diagnostics.current_sweep {
            diagnostics.current_sweep = batch.sweep;
            diagnostics.current_geometry = !self.geometry_completed;
            diagnostics.current_processed = 0;
            diagnostics.current_skipped = 0;
            diagnostics.current_applied_vertices = 0;
            diagnostics.current_backlog = diagnostics.current_targets;
        }
        diagnostics.current_mixed_clock = !batch.stable_clock;
        diagnostics.delivered_clock_revision = batch.clock_revision;
        diagnostics.delivered_clock_minute = Some(batch.clock_minute);
        diagnostics.delivered_sun_direction = batch.light.sun_dir.to_array();
        diagnostics.delivered_sun_lux = batch.light.sun_illuminance;
        diagnostics.delivered_sky_lux = batch.light.sky;
        diagnostics.delivered_ground_lux = batch.light.ground;
        diagnostics.delivered_batches = diagnostics.delivered_batches.saturating_add(1);
        diagnostics.processed_vertices = diagnostics
            .processed_vertices
            .saturating_add(batch.processed as u64);
        diagnostics.skipped_vertices = diagnostics
            .skipped_vertices
            .saturating_add(batch.skipped as u64);
        diagnostics.current_processed = diagnostics
            .current_processed
            .saturating_add(batch.processed);
        diagnostics.current_skipped = diagnostics.current_skipped.saturating_add(batch.skipped);
        diagnostics.current_backlog = diagnostics
            .current_targets
            .saturating_sub(diagnostics.current_processed);
        diagnostics.error = self.error.clone();
        if self.error.is_some() {
            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
        }
        let mut delivered = 0;
        let mut applied = 0;
        let mut patches = Vec::new();
        for patch in batch.patches {
            match patch {
                RepairPatch::Static { first, lights } => {
                    delivered += lights.len();
                    applied += lights.len();
                    diagnostics.applied_static_vertices = diagnostics
                        .applied_static_vertices
                        .saturating_add(lights.len() as u64);
                    patches.push(LightPatch {
                        first,
                        lights,
                        dynamic: true,
                    });
                }
                RepairPatch::Chunk { id, first, lights } => {
                    delivered += lights.len();
                    if self.lights.apply(batch.generation, id, first, &lights) {
                        applied += lights.len();
                        diagnostics.applied_chunk_vertices = diagnostics
                            .applied_chunk_vertices
                            .saturating_add(lights.len() as u64);
                    } else {
                        diagnostics.rejected_chunk_vertices = diagnostics
                            .rejected_chunk_vertices
                            .saturating_add(lights.len() as u64);
                    }
                }
            }
        }
        if !patches.is_empty() {
            town.world.baked.deliver_patches(patches);
        }
        diagnostics.delivered_vertices = diagnostics
            .delivered_vertices
            .saturating_add(delivered as u64);
        diagnostics.current_applied_vertices =
            diagnostics.current_applied_vertices.saturating_add(applied);
        if applied > 0 {
            diagnostics.applied_batches = diagnostics.applied_batches.saturating_add(1);
        }
        let sweep_complete = batch.complete
            && self.error.is_none()
            && diagnostics.current_processed == diagnostics.current_targets
            && diagnostics
                .current_applied_vertices
                .saturating_add(diagnostics.current_skipped)
                == diagnostics.current_processed;
        if sweep_complete && diagnostics.current_targets > 0 {
            diagnostics.completed_sweeps = diagnostics.completed_sweeps.saturating_add(1);
        }
        diagnostics.current_complete = sweep_complete
            && diagnostics.current_skipped == 0
            && diagnostics.current_applied_vertices == diagnostics.current_targets
            && batch.stable_clock
            && batch.clock_revision == self.clock_revision;
        if diagnostics.current_complete
            && diagnostics.current_targets > 0
            && diagnostics.last_completed_clock_revision != batch.clock_revision
        {
            diagnostics.completed_generations = diagnostics.completed_generations.saturating_add(1);
            diagnostics.last_completed_generation = batch.generation;
            diagnostics.last_completed_clock_revision = batch.clock_revision;
            diagnostics.last_completed_clock_minute = Some(batch.clock_minute);
            if !self.geometry_completed {
                diagnostics.completed_geometry_generations =
                    diagnostics.completed_geometry_generations.saturating_add(1);
                self.geometry_completed = true;
            } else {
                diagnostics.completed_clock_generations =
                    diagnostics.completed_clock_generations.saturating_add(1);
            }
        }
    }
}

impl Town {
    /// Enables immediate ambient fallback and selective vertex repair on one worker.
    ///
    /// # Errors
    /// Returns a scene validation error or a worker startup error. Worker
    /// startup failure retains immediate lighting for every sun direction.
    pub fn enable_baked_repair(&mut self, light: BakeLight) -> Result<(), String> {
        self.enable_baked_repair_with(light, RepairQueue::new)
    }

    fn enable_baked_repair_with(
        &mut self,
        light: BakeLight,
        start_worker: impl FnOnce() -> Result<RepairQueue, String>,
    ) -> Result<(), String> {
        if self.baked_repair.is_some() {
            return Ok(());
        }
        self.invalidate_baked_repair();
        self.set_destruction_relighting(Some(light.sun_dir))?;
        self.pose();
        let queue = match start_worker() {
            Ok(queue) => queue,
            Err(error) => {
                // Without selective repair, keep every possible former sun
                // shadow dynamic as the clock advances. Local openness is
                // independent of the sun; build this membership only once.
                self.relight_receivers = super::receivers::Index::all_directions(
                    &self.relight_vertices,
                    &self.wreck.buildings,
                );
                self.refresh_light();
                return Err(error);
            }
        };
        let mut repair = Repair::new(queue, self);
        repair.refresh_ground(self);
        repair.publish_fallback(self);
        self.baked_repair = Some(Mutex::new(repair));
        Ok(())
    }

    /// Whether selective repair has an active worker.
    #[must_use]
    pub fn baked_repair_enabled(&self) -> bool {
        self.baked_repair.is_some()
    }

    /// Replaces changed geometry or coalesces the new clock light, then applies
    /// at most one bounded batch. Clock changes retain the ordered target cursor.
    /// Moving occluders use the last requested pose snapshot;
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

    /// Fixed-size progress counters for selective repair, including pending work.
    #[must_use]
    pub fn baked_repair_diagnostics(&self) -> BakedRepairDiagnostics {
        self.baked_repair
            .as_ref()
            .map_or_else(BakedRepairDiagnostics::default, |repair| {
                repair
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .diagnostics
                    .clone()
            })
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

    pub(super) fn set_relight_fallback(&self, fallback: Vec<(u32, [u8; 4])>) {
        let sampled = {
            let mut cached = self
                .relight_fallback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if cached.as_ref() != &fallback {
                *cached = Arc::new(fallback);
            }
            cached.clone()
        };
        if let Some(repair) = &self.baked_repair {
            let mut repair = repair
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            repair.refresh_ground(self);
            repair.publish_fallback(self);
        } else {
            self.world.baked.set_fallback(sampled.as_ref().clone());
        }
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
        fixture_options(count, false)
    }

    fn fixture_options(count: usize, remote: bool) -> (Town, SyncSender<RepairBatch>) {
        let pack = ZonePack {
            textures: Vec::new(),
            materials: Vec::new(),
            models: Vec::new(),
            character: None,
            forms: Vec::new(),
        };
        let mut world = TexturedScene::default();
        world.add_material(TexturedMaterial::default());
        let mut vertices = triangle(Vec3::Y);
        if remote {
            vertices.extend(triangle(Vec3::Y).into_iter().map(|mut vertex| {
                vertex.pos[0] += 30.0;
                vertex
            }));
        }
        let count_vertices = vertices.len();
        world.add_mesh(TexturedMesh {
            primitives: vec![Primitive {
                material: 0,
                vertices,
                indices: (0..count_vertices as u32).collect(),
            }],
        });
        world.place(0, Mat4::IDENTITY);
        world.baked.deliver_lights(vec![[100; 4]; count_vertices]);
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
            walls: None,
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
            sweep: repair.diagnostics.current_sweep,
            clock_revision: repair.clock_revision,
            clock_minute: repair.minute.unwrap(),
            light: BakeLight {
                sun_dir: Vec3::from(repair.diagnostics.sun_direction),
                sun_illuminance: repair.diagnostics.sun_lux,
                sky: repair.diagnostics.sky_lux,
                ground: repair.diagnostics.ground_lux,
            },
            stable_clock: true,
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
    fn worker_startup_failure_keeps_clock_safe_fallback_until_restore() {
        let (mut town, send) = fixture_options(1, true);
        town.invalidate_baked_repair();
        town.baked_repair = None;
        drop(send);
        town.set_destruction_relighting(None).unwrap();
        town.pose();
        assert!(!town.destruction_relighting());
        assert!(town.world.baked.take_mask().unwrap().is_empty());
        assert_eq!(town.world.baked.take().unwrap(), [[100; 4]; 6]);
        let error = town
            .enable_baked_repair_with(light(), || Err("worker unavailable".into()))
            .unwrap_err();
        assert_eq!(error, "worker unavailable");
        assert!(town.destruction_relighting());
        assert!(!town.baked_repair_enabled());
        let diagnostics = town.baked_repair_diagnostics();
        assert!(!diagnostics.enabled && !diagnostics.current_complete);
        assert_eq!(town.world.baked.take_mask().unwrap(), [0, 1, 2, 3, 4, 5]);
        let fallback = town.relight_fallback.lock().unwrap().clone();
        assert_eq!(fallback.len(), 6);
        let current = town.world.baked.take().unwrap();
        for &(index, light) in fallback.iter() {
            assert_eq!(current[index as usize], light);
        }
        let chunks = town.instances(None).unwrap();
        assert!(chunks.vertex_lights.is_none());
        assert!(
            chunks
                .instances
                .iter()
                .all(|instance| instance.light[3] > 0)
        );

        // These remote receivers were outside the admission light's shadow.
        assert_eq!(
            super::super::receivers::Index::new(
                &town.relight_vertices,
                &town.wreck.buildings,
                Vec3::Y,
            )
            .affected([0]),
            [0, 1, 2]
        );
        let mut later = light();
        later.sun_dir = Vec3::new(-1.0, 0.01, 0.0).normalize();
        let later_receivers = super::super::receivers::Index::new(
            &town.relight_vertices,
            &town.wreck.buildings,
            later.sun_dir,
        )
        .affected([0]);
        assert!(later_receivers.contains(&3));
        town.poll_baked_repair(later, 2);
        assert!(Arc::ptr_eq(
            &fallback,
            &town.relight_fallback.lock().unwrap()
        ));
        town.world.baked.deliver_lights(vec![[44; 4]; 6]);
        town.world.baked.deliver_lamps(vec![[31; 4]; 6]);
        assert_eq!(town.world.baked.take().unwrap(), current);
        assert_eq!(town.world.baked.take_lamps().unwrap(), [[0; 4]; 6]);
        assert!(town.world.baked.take_patches().is_empty());

        town.restore();
        assert!(town.world.baked.take_mask().unwrap().is_empty());
        assert_eq!(town.world.baked.take().unwrap(), [[44; 4]; 6]);
        assert_eq!(town.world.baked.take_lamps().unwrap(), [[31; 4]; 6]);
        assert!(town.instances(None).is_none());
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
        assert_eq!(
            repair.generation, generation,
            "clock updates retain geometry admission"
        );
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
    #[test]
    fn conservative_ground_is_neutral_immediately_and_survives_late_bakes_until_repair_or_restore()
    {
        let (mut town, send) = fixture_options(1, true);
        // Sync has published the mask before any request or BVH build.
        assert_eq!(town.relight_receivers.affected([0]), [0, 1, 2]);
        assert_eq!(town.world.baked.take_mask().unwrap(), [0, 1, 2, 3, 4, 5]);
        let sampled = town.relight_fallback.lock().unwrap().clone();
        assert_eq!(
            sampled.len(),
            3,
            "remote ground requires no extra local samples"
        );
        let first = town.world.baked.take().unwrap();
        for &(index, light) in sampled.iter() {
            assert_eq!(first[index as usize], light);
        }
        assert_eq!(&first[3..], &[UNBAKED; 3]);
        town.world.baked.deliver_lights(vec![[24; 4]; 6]);
        town.world.baked.deliver_lamps(vec![[31; 4]; 6]);
        assert_eq!(&town.world.baked.take().unwrap()[3..], &[UNBAKED; 3]);
        assert_eq!(town.world.baked.take_lamps().unwrap(), [[0; 4]; 6]);
        town.poll_baked_repair(light(), 1);
        let ground = town
            .baked_repair
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .ground
            .clone();
        let mut batch = response(&town, [17; 4]);
        batch.patches = vec![RepairPatch::Static {
            first: 3,
            lights: vec![[17; 4]; 3],
        }];
        send.try_send(batch).unwrap();
        town.poll_baked_repair(light(), 1);
        assert!(town.world.baked.take_patches()[0].dynamic);
        town.world.baked.deliver_lights(vec![[44; 4]; 6]);
        let repaired = town.world.baked.take().unwrap();
        for &(index, light) in sampled.iter() {
            assert_eq!(repaired[index as usize], light);
        }
        assert_eq!(
            &repaired[3..],
            &[[17; 4]; 3],
            "worker repair wins the neutral fallback"
        );
        let mut low_sun = light();
        low_sun.sun_dir = Vec3::new(-1.0, 0.01, 0.0).normalize();
        town.poll_baked_repair(low_sun, 2);
        assert!(Arc::ptr_eq(
            &ground,
            &town.baked_repair.as_ref().unwrap().lock().unwrap().ground
        ));
        assert!(Arc::ptr_eq(
            &sampled,
            &town.relight_fallback.lock().unwrap()
        ));
        // Retain drops repaired light before compacted IDs can be reused.
        town.wreck.site.retain(|_| true);
        town.wreck.revision += 1;
        town.sync();
        town.pose();
        assert_eq!(&town.world.baked.take().unwrap()[3..], &[UNBAKED; 3]);
        town.restore();
        assert!(town.world.baked.take_mask().unwrap().is_empty());
        assert_eq!(town.world.baked.take().unwrap(), [[44; 4]; 6]);
        assert_eq!(town.world.baked.take_lamps().unwrap(), [[31; 4]; 6]);
    }
    #[test]
    fn final_settlement_refreshes_the_occluder_epoch_once_then_clock_reuses_it() {
        let (mut town, _) = fixture();
        town.wreck.site.set_debris_lifetime(60.0);
        town.poll_baked_repair(light(), 1);
        let initial_pose = town.pool.instances[0].current;
        let site_revision = town.wreck.site.revision();
        let (geometry_epoch, source, motion_epoch) = {
            let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
            (
                repair.geometry_epoch,
                repair.sources.meshes.clone(),
                repair.epoch.clone().unwrap(),
            )
        };
        for _ in 0..1200 {
            town.wreck.site.tick(1.0 / 60.0);
            if town.wreck.site.pieces()[0].chunks[0].settled.is_some() {
                break;
            }
        }
        let final_pose = town.wreck.site.pieces()[0].chunks[0]
            .settled
            .expect("the single grounded chunk must retire into static rubble");
        assert_ne!(initial_pose, final_pose);
        assert_eq!(town.wreck.site.revision(), site_revision);
        town.sync();
        town.pose();
        town.poll_baked_repair(light(), 1);
        let final_epoch = {
            let mut repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
            assert!(
                repair.geometry_epoch > geometry_epoch,
                "settlement invalidates a flying-rubble hierarchy"
            );
            assert!(Arc::ptr_eq(&motion_epoch, repair.epoch.as_ref().unwrap()));
            assert!(Arc::ptr_eq(&source, &repair.sources.meshes));
            let snapshot = repair.sources.snapshot(&town);
            assert_eq!(snapshot.instances[0].current, final_pose);
            assert!(snapshot.instances[0].settled);
            repair.geometry_epoch
        };
        town.poll_baked_repair(light(), 2);
        let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
        assert_eq!(repair.geometry_epoch, final_epoch);
        assert!(Arc::ptr_eq(&source, &repair.sources.meshes));
    }
    #[test]
    fn diagnostics_track_partial_completion_and_ignore_stale_or_empty_jobs() {
        let (mut town, send) = fixture();
        town.poll_baked_repair(light(), 1);
        let pending = town.baked_repair_diagnostics();
        assert!(pending.enabled && pending.active);
        assert_eq!(pending.current_targets, 6);
        assert_eq!(pending.current_backlog, 6);
        assert_eq!(pending.applied_batches, 0);
        assert!(!pending.current_complete);
        let mut partial = response(&town, [17; 4]);
        partial.complete = false;
        partial.processed = 4;
        partial.skipped = 1;
        let id = town.pool.instances[0].id;
        partial.patches = vec![
            RepairPatch::Static {
                first: 0,
                lights: vec![[17; 4]; 2],
            },
            RepairPatch::Chunk {
                id,
                first: 0,
                lights: vec![[17; 4]],
            },
        ];
        send.try_send(partial).unwrap();
        town.poll_baked_repair(light(), 1);
        let partial = town.baked_repair_diagnostics();
        assert_eq!(
            (partial.delivered_batches, partial.delivered_vertices),
            (1, 3)
        );
        assert_eq!(
            (
                partial.current_processed,
                partial.current_skipped,
                partial.current_backlog
            ),
            (4, 1, 2)
        );
        assert!(!partial.current_complete);
        let mut complete = response(&town, [19; 4]);
        complete.processed = 2;
        complete.patches = vec![RepairPatch::Chunk {
            id,
            first: 1,
            lights: vec![[19; 4]; 2],
        }];
        send.try_send(complete).unwrap();
        town.poll_baked_repair(light(), 1);
        let completed = town.baked_repair_diagnostics();
        assert_eq!(
            (
                completed.applied_static_vertices,
                completed.applied_chunk_vertices
            ),
            (2, 3)
        );
        assert_eq!(completed.completed_sweeps, 1);
        assert_eq!(completed.completed_generations, 0);
        assert_eq!(completed.current_backlog, 0);
        assert!(
            !completed.current_complete,
            "a skipped target is not a current texel"
        );
        let mut retry = response(&town, [23; 4]);
        retry.sweep = 1;
        send.try_send(retry).unwrap();
        town.poll_baked_repair(light(), 1);
        let completed = town.baked_repair_diagnostics();
        assert_eq!(
            (
                completed.completed_generations,
                completed.completed_geometry_generations
            ),
            (1, 1)
        );
        assert!(completed.current_complete);
        assert_eq!(completed.last_completed_generation, completed.generation);
        town.poll_baked_repair(light(), 2);
        let clock = town.baked_repair_diagnostics();
        assert_eq!(
            (
                clock.requests,
                clock.geometry_requests,
                clock.clock_requests
            ),
            (2, 1, 1)
        );
        assert_eq!(
            clock.delivered_batches, completed.delivered_batches,
            "a light update without delivery changes no applied counts"
        );
        assert_eq!(
            clock.current_backlog, 0,
            "the last delivered sweep remains recorded"
        );
        assert!(!clock.current_complete);
        town.restore();
        let restored = town.baked_repair_diagnostics();
        assert!(!restored.active);
        assert_eq!(
            restored.completed_generations, 1,
            "lifetime evidence survives reset"
        );
        town.poll_baked_repair(light(), 2);
        let empty = town.baked_repair_diagnostics();
        assert_eq!(empty.current_targets, 0);
        assert!(empty.current_complete);
        assert_eq!(
            empty.completed_generations, 1,
            "empty requests do not prove worker completion"
        );
    }

    #[test]
    fn mixed_clock_coverage_is_not_reported_as_converged_until_a_complete_stable_sweep() {
        let (mut town, send) = fixture();
        town.poll_baked_repair(light(), 1);
        let geometry = town.baked_repair_diagnostics().generation;
        let mut old = response(&town, [11; 4]);
        old.processed = 3;
        old.complete = false;
        old.patches
            .retain(|patch| matches!(patch, RepairPatch::Static { .. }));
        let old_revision = old.clock_revision;
        send.try_send(old).unwrap();
        let mut midnight = light();
        midnight.sky = 1.0;
        midnight.sun_illuminance = 0.0;
        town.poll_baked_repair(midnight, 2);
        let partial = town.baked_repair_diagnostics();
        assert_eq!(partial.generation, geometry);
        assert_eq!(
            partial.current_processed, 3,
            "buffered old-clock work is still geometry-valid"
        );
        assert_eq!(partial.delivered_clock_revision, old_revision);
        assert_eq!(partial.delivered_clock_minute, Some(1));
        assert_eq!(partial.clock_minute, Some(2));
        assert!(!partial.current_complete);

        let mut tail = response(&town, [22; 4]);
        tail.processed = 3;
        tail.stable_clock = false;
        tail.patches
            .retain(|patch| matches!(patch, RepairPatch::Chunk { .. }));
        send.try_send(tail).unwrap();
        town.poll_baked_repair(midnight, 2);
        let mixed = town.baked_repair_diagnostics();
        assert_eq!(mixed.current_processed, 6);
        assert_eq!(mixed.current_backlog, 0);
        assert_eq!(mixed.completed_sweeps, 1);
        assert!(mixed.current_mixed_clock);
        assert!(!mixed.current_complete);
        assert_eq!(mixed.completed_generations, 0);
        assert_eq!(mixed.delivered_sky_lux, midnight.sky);

        let mut stable = response(&town, [33; 4]);
        stable.sweep = 1;
        send.try_send(stable).unwrap();
        town.poll_baked_repair(midnight, 2);
        let converged = town.baked_repair_diagnostics();
        assert_eq!(converged.current_processed, converged.current_targets);
        assert_eq!(
            converged.current_applied_vertices,
            converged.current_targets
        );
        assert_eq!(converged.current_skipped, 0);
        assert!(converged.current_complete);
        assert!(!converged.current_mixed_clock);
        assert_eq!(converged.completed_geometry_generations, 1);
        assert_eq!(
            converged.last_completed_clock_revision,
            converged.clock_revision
        );
        assert_eq!(converged.last_completed_clock_minute, Some(2));

        town.poll_baked_repair(light(), 3);
        let mut noon = response(&town, [44; 4]);
        noon.sweep = 2;
        send.try_send(noon).unwrap();
        town.poll_baked_repair(light(), 3);
        let latest = town.baked_repair_diagnostics();
        assert_eq!(latest.generation, geometry);
        assert_eq!(latest.completed_geometry_generations, 1);
        assert_eq!(latest.completed_clock_generations, 1);
        assert_eq!(latest.last_completed_clock_minute, Some(3));
        assert!(latest.current_complete);
    }

    #[test]
    fn frozen_missing_buildings_keep_ground_fallback_and_repair_targets_until_restore() {
        let (mut town, _) = fixture();
        town.poll_baked_repair(light(), 1);
        let ground = town
            .baked_repair
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .ground
            .clone();
        assert!(!ground.is_empty());
        town.wreck.let_go(0);
        assert!(town.wreck.refs.is_empty());
        assert!(town.wreck.site.pieces().is_empty());
        assert!(town.wreck.frozen[&0].contains(&super::super::Frozen::Gone));
        town.sync();
        town.pose();
        assert_eq!(
            town.wreck.relight_buildings(),
            std::collections::BTreeSet::from([0])
        );
        assert!(
            !town.relight_fallback.lock().unwrap().is_empty(),
            "immediate ground fallback survives eviction"
        );
        town.poll_baked_repair(light(), 2);
        {
            let repair = town.baked_repair.as_ref().unwrap().lock().unwrap();
            assert_eq!(&*repair.ground, &*ground);
            assert_eq!(repair.targets.len(), ground.len());
            assert!(
                repair
                    .targets
                    .iter()
                    .all(|target| matches!(target, RepairTarget::Static(_)))
            );
        }
        town.restore();
        assert!(town.wreck.frozen.is_empty());
        assert!(town.wreck.relight_buildings().is_empty());
        assert!(town.relight_fallback.lock().unwrap().is_empty());
        town.poll_baked_repair(light(), 2);
        assert_eq!(town.baked_repair_diagnostics().current_targets, 0);
    }

    #[test]
    fn diagnostics_retain_errors_and_rejected_texels_across_replacement_requests() {
        let (mut town, send) = fixture();
        town.poll_baked_repair(light(), 1);
        let mut failed = response(&town, [99; 4]);
        failed.processed = 0;
        failed.patches.clear();
        failed.error = Some("Test hierarchy failure".into());
        send.try_send(failed).unwrap();
        town.poll_baked_repair(light(), 1);
        let failed = town.baked_repair_diagnostics();
        assert_eq!(failed.error.as_deref(), Some("Test hierarchy failure"));
        assert_eq!(failed.error_count, 1);
        assert!(!failed.current_complete);
        assert_eq!(failed.completed_generations, 0);
        town.poll_baked_repair(light(), 2);
        let replaced = town.baked_repair_diagnostics();
        assert_eq!(replaced.error.as_deref(), Some("Test hierarchy failure"));
        assert_eq!(replaced.error_count, 1);
        let mut rejected = response(&town, [99; 4]);
        rejected.skipped = 5;
        rejected.patches = vec![RepairPatch::Chunk {
            id: u64::MAX,
            first: 0,
            lights: vec![[99; 4]],
        }];
        send.try_send(rejected).unwrap();
        town.poll_baked_repair(light(), 2);
        let rejected = town.baked_repair_diagnostics();
        assert_eq!(rejected.delivered_vertices, 1);
        assert_eq!(rejected.rejected_chunk_vertices, 1);
        assert_eq!(rejected.applied_batches, 0);
        assert!(!rejected.current_complete);
    }
}
