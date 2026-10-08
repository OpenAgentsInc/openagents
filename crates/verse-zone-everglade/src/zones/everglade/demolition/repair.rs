//! Selective light repair on one worker, with bounded delivery and cancellation.

use crate::pbr::textured::{AlphaMode, InstancedFigure, TexturedScene, TexturedVertex};
use crate::pbr::textured_bake::{BakeLight, BakeSettings, SceneBaker};
use glam::{Mat3, Mat4, Vec3};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const BATCH_SAMPLES: usize = 128;
const BATCH_TIME: Duration = Duration::from_millis(2);
const DELIVERY_CAPACITY: usize = 2;

/// A static merged vertex, or one vertex in a stable chunk's source mesh.
/// Chunk indices flatten primitives in their source order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RepairTarget {
    Static(u32),
    Chunk { id: u64, vertex: u32 },
}

/// The complete currently affected target set, with shared source and rigid
/// snapshots. Index edits stay shared and are guarded by their captured revision.
/// The caller retains unfinished targets when replacing a request and removes
/// expired chunk IDs. Only the worker merges geometry and poses target vertices.
pub(crate) struct RepairRequest {
    pub revision: u64,
    /// Changes for structural edits or rigid membership changes. Moving targets
    /// may reuse the previous occluder snapshot; their own sample poses stay current.
    pub geometry_epoch: u64,
    pub scene: Arc<TexturedScene>,
    /// Individual source chunks, including settled chunks before renderer merging.
    pub instances: Option<InstancedFigure>,
    pub targets: Arc<[RepairTarget]>,
    pub light: BakeLight,
    pub settings: BakeSettings,
    pub key: u64,
}

/// Adjacent occupied texels only. Static patches become dynamic LightPatch spans;
/// chunk patches address the caller's stable per-vertex chunk light cache.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RepairPatch {
    Static {
        first: u32,
        lights: Vec<[u8; 4]>,
    },
    Chunk {
        id: u64,
        first: u32,
        lights: Vec<[u8; 4]>,
    },
}

#[derive(Debug)]
pub(crate) struct RepairBatch {
    pub generation: u64,
    pub revision: u64,
    pub geometry_epoch: u64,
    pub patches: Vec<RepairPatch>,
    pub processed: usize,
    pub skipped: usize,
    pub complete: bool,
    pub error: Option<String>,
}

#[derive(Default)]
struct Mailbox {
    pending: Option<Job>,
    stopped: bool,
}

#[derive(Default)]
struct Shared {
    generation: AtomicU64,
    cache_epoch: AtomicU64,
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}

#[derive(Clone)]
struct Stamp {
    generation: u64,
    cache_epoch: u64,
    edits_revision: u64,
    scene: Arc<TexturedScene>,
}

impl Stamp {
    fn current(&self, shared: &Shared) -> bool {
        self.generation == shared.generation.load(Ordering::Acquire)
            && self.edits_revision == self.scene.edits.revision()
    }
}

struct Job {
    stamp: Stamp,
    request: RepairRequest,
}

/// One detached worker and at most two queued batches. Neither polling nor
/// invalidation joins a worker or builds a scene hierarchy on the caller.
pub(crate) struct RepairQueue {
    shared: Arc<Shared>,
    receive: Receiver<RepairBatch>,
    current: Option<Stamp>,
}

impl RepairQueue {
    pub fn new() -> Result<Self, String> {
        #[cfg(target_arch = "wasm32")]
        {
            Err("Selective light repair requires a worker thread".into())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (queue, send) = Self::channel();
            let shared = queue.shared.clone();
            let _worker = std::thread::Builder::new()
                .name("verse-selective-light".into())
                .spawn(move || worker(shared, send))
                .map_err(|error| format!("Could not start selective light repair: {error}"))?;
            Ok(queue)
        }
    }

    fn channel() -> (Self, SyncSender<RepairBatch>) {
        let (send, receive) = std::sync::mpsc::sync_channel(DELIVERY_CAPACITY);
        (
            Self {
                shared: Arc::new(Shared::default()),
                receive,
                current: None,
            },
            send,
        )
    }

    /// Replaces pending work and cancels the active generation between samples.
    /// Repeated caller revision values remain safe because generations never reset.
    pub fn request(&mut self, request: RepairRequest) -> u64 {
        let generation = self
            .shared
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        let stamp = Stamp {
            generation,
            cache_epoch: self.shared.cache_epoch.load(Ordering::Acquire),
            edits_revision: request.scene.edits.revision(),
            scene: request.scene.clone(),
        };
        let mut mailbox = self
            .shared
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        mailbox.pending = (!request.targets.is_empty()).then_some(Job {
            stamp: stamp.clone(),
            request,
        });
        self.current = Some(stamp);
        self.shared.wake.notify_one();
        generation
    }

    /// Call before restoring pristine light or reusing body IDs. Invalidates
    /// buffered deliveries and the cached hierarchy without waiting for a worker.
    pub fn invalidate(&mut self) {
        self.shared.generation.fetch_add(1, Ordering::AcqRel);
        self.shared.cache_epoch.fetch_add(1, Ordering::AcqRel);
        self.current = None;
        self.shared
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending = None;
        self.shared.wake.notify_one();
    }

    /// Consumes at most one batch, even when that batch is stale. Applying one
    /// result therefore touches at most 128 requested vertices on the caller.
    pub fn poll(&mut self) -> Option<RepairBatch> {
        let batch = self.receive.try_recv().ok()?;
        self.current
            .as_ref()
            .filter(|stamp| stamp.generation == batch.generation && stamp.current(&self.shared))
            .map(|_| batch)
    }
}

impl Drop for RepairQueue {
    fn drop(&mut self) {
        self.invalidate();
        self.shared
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopped = true;
        self.shared.wake.notify_one();
    }
}

fn next_job(shared: &Shared) -> Option<Job> {
    let mut mailbox = shared
        .mailbox
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    loop {
        if mailbox.stopped {
            return None;
        }
        if let Some(job) = mailbox.pending.take() {
            return Some(job);
        }
        mailbox = shared
            .wake
            .wait(mailbox)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
}

#[derive(Default)]
struct CachedBaker {
    entry: Option<(CacheKey, SceneBaker)>,
}

struct CacheKey {
    scene: Arc<TexturedScene>,
    rigid_scene: Option<Arc<TexturedScene>>,
    edits_revision: u64,
    geometry_epoch: u64,
    cache_epoch: u64,
    settings: BakeSettings,
}

impl CacheKey {
    fn of(job: &Job) -> Self {
        Self {
            scene: job.request.scene.clone(),
            rigid_scene: job
                .request
                .instances
                .as_ref()
                .map(|frame| frame.scene.clone()),
            edits_revision: job.stamp.edits_revision,
            geometry_epoch: job.request.geometry_epoch,
            cache_epoch: job.stamp.cache_epoch,
            settings: job.request.settings,
        }
    }

    fn matches(&self, next: &Self) -> bool {
        Arc::ptr_eq(&self.scene, &next.scene)
            && match (&self.rigid_scene, &next.rigid_scene) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && self.edits_revision == next.edits_revision
            && self.geometry_epoch == next.geometry_epoch
            && self.cache_epoch == next.cache_epoch
            && self.settings == next.settings
    }
}

impl CachedBaker {
    fn prepare(&mut self, job: &Job) -> Result<bool, String> {
        self.prepare_with(job, |request| {
            SceneBaker::for_repair(
                &request.scene,
                request.instances.as_ref(),
                request.light,
                request.settings,
                request.key,
            )
        })
    }

    fn prepare_with(
        &mut self,
        job: &Job,
        build: impl FnOnce(&RepairRequest) -> Result<SceneBaker, String>,
    ) -> Result<bool, String> {
        if job.request.scene.edits.revision() != job.stamp.edits_revision {
            return Ok(false);
        }
        let key = CacheKey::of(job);
        if let Some((old, baker)) = &mut self.entry
            && old.matches(&key)
        {
            baker.set_light(job.request.light);
            return Ok(true);
        }
        let baker = build(&job.request)?;
        // Index edits are shared with the runtime. A race during merge/BVH
        // construction cannot be cached or delivered under the earlier epoch.
        if job.request.scene.edits.revision() != job.stamp.edits_revision {
            self.entry = None;
            return Ok(false);
        }
        self.entry = Some((key, baker));
        Ok(true)
    }
}

struct ChunkPose {
    mesh: usize,
    transform: Mat4,
    normals: Mat3,
}

struct ChunkLookup<'a> {
    frame: Option<&'a InstancedFigure>,
    poses: BTreeMap<u64, ChunkPose>,
}

impl<'a> ChunkLookup<'a> {
    fn new(frame: Option<&'a InstancedFigure>) -> Self {
        let poses = frame
            .into_iter()
            .flat_map(|frame| frame.instances.iter())
            .map(|instance| {
                (
                    instance.id,
                    ChunkPose {
                        mesh: instance.mesh,
                        transform: instance.current,
                        normals: Mat3::from_mat4(instance.current).inverse().transpose(),
                    },
                )
            })
            .collect();
        Self { frame, poses }
    }

    fn vertex(&self, id: u64, vertex: u32) -> Option<(TexturedVertex, bool)> {
        let frame = self.frame?;
        let pose = self.poses.get(&id)?;
        let mesh = frame.scene.meshes.get(pose.mesh)?;
        let mut index = vertex as usize;
        for primitive in &mesh.primitives {
            if index >= primitive.vertices.len() {
                index -= primitive.vertices.len();
                continue;
            }
            let mut vertex = primitive.vertices[index];
            vertex.pos = pose
                .transform
                .transform_point3(vertex.pos.into())
                .to_array();
            vertex.normal = (pose.normals * Vec3::from(vertex.normal))
                .normalize_or(Vec3::Y)
                .to_array();
            let foliage = matches!(
                frame.scene.materials.get(primitive.material)?.alpha,
                AlphaMode::Mask { .. }
            );
            return Some((vertex, foliage));
        }
        None
    }
}

fn append_patch(patches: &mut Vec<RepairPatch>, target: RepairTarget, light: [u8; 4]) {
    match (patches.last_mut(), target) {
        (Some(RepairPatch::Static { first, lights }), RepairTarget::Static(index))
            if first.checked_add(lights.len() as u32) == Some(index) =>
        {
            lights.push(light)
        }
        (
            Some(RepairPatch::Chunk { id, first, lights }),
            RepairTarget::Chunk { id: next, vertex },
        ) if *id == next && first.checked_add(lights.len() as u32) == Some(vertex) => {
            lights.push(light)
        }
        (_, RepairTarget::Static(first)) => patches.push(RepairPatch::Static {
            first,
            lights: vec![light],
        }),
        (_, RepairTarget::Chunk { id, vertex }) => patches.push(RepairPatch::Chunk {
            id,
            first: vertex,
            lights: vec![light],
        }),
    }
}

fn sample_batch(
    job: &Job,
    cursor: &mut usize,
    mut elapsed: impl FnMut() -> Duration,
    current: impl Fn() -> bool,
    mut sample: impl FnMut(RepairTarget) -> Option<[u8; 4]>,
) -> Option<RepairBatch> {
    let mut batch = RepairBatch {
        generation: job.stamp.generation,
        revision: job.request.revision,
        geometry_epoch: job.request.geometry_epoch,
        patches: Vec::new(),
        processed: 0,
        skipped: 0,
        complete: false,
        error: None,
    };
    while *cursor < job.request.targets.len() && batch.processed < BATCH_SAMPLES {
        if !current() {
            return None;
        }
        // A single vertex trace is not preemptible; stop at the time boundary
        // before starting another one, rather than promising a hard deadline.
        if elapsed() >= BATCH_TIME {
            break;
        }
        let target = job.request.targets[*cursor];
        match sample(target) {
            Some(light) => append_patch(&mut batch.patches, target, light),
            None => batch.skipped += 1,
        }
        *cursor += 1;
        batch.processed += 1;
    }
    if !current() {
        return None;
    }
    batch.complete = *cursor == job.request.targets.len();
    Some(batch)
}

fn send_batch(
    shared: &Shared,
    stamp: &Stamp,
    send: &SyncSender<RepairBatch>,
    mut batch: RepairBatch,
) -> bool {
    loop {
        if !stamp.current(shared) {
            return false;
        }
        match send.try_send(batch) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(pending)) => batch = pending,
        }
        // Backpressure never spins and a replacement request wakes the wait.
        let mailbox = shared
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(shared.wake.wait_timeout(mailbox, Duration::from_millis(1)));
    }
}

fn worker(shared: Arc<Shared>, send: SyncSender<RepairBatch>) {
    let mut cache = CachedBaker::default();
    while let Some(job) = next_job(&shared) {
        if !job.stamp.current(&shared) {
            continue;
        }
        match cache.prepare(&job) {
            Ok(false) => continue,
            Err(error) => {
                let batch = RepairBatch {
                    generation: job.stamp.generation,
                    revision: job.request.revision,
                    geometry_epoch: job.request.geometry_epoch,
                    patches: Vec::new(),
                    processed: 0,
                    skipped: 0,
                    complete: true,
                    error: Some(error),
                };
                send_batch(&shared, &job.stamp, &send, batch);
                continue;
            }
            Ok(true) => {}
        }
        if !job.stamp.current(&shared) {
            continue;
        }
        let lookup = ChunkLookup::new(job.request.instances.as_ref());
        let Some((_, baker)) = &cache.entry else {
            continue;
        };
        let mut cursor = 0;
        while cursor < job.request.targets.len() {
            let started = Instant::now();
            let Some(batch) = sample_batch(
                &job,
                &mut cursor,
                || started.elapsed(),
                || job.stamp.current(&shared),
                |target| match target {
                    RepairTarget::Static(index) => baker.vertex_light(index as usize),
                    RepairTarget::Chunk { id, vertex } => lookup
                        .vertex(id, vertex)
                        .map(|(vertex, foliage)| baker.surface_light(&vertex, foliage)),
                },
            ) else {
                break;
            };
            if !send_batch(&shared, &job.stamp, &send, batch) {
                break;
            }
            std::thread::yield_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbr::textured::{DynamicInstance, Primitive, TexturedMaterial, TexturedMesh};
    use std::cell::Cell;

    fn request(revision: u64, targets: &[RepairTarget]) -> RepairRequest {
        RepairRequest {
            revision,
            geometry_epoch: 1,
            scene: Arc::new(TexturedScene::default()),
            instances: None,
            targets: Arc::from(targets),
            light: BakeLight {
                sun_dir: Vec3::Y,
                sun_illuminance: 10.0,
                sky: 10.0,
                ground: 2.0,
            },
            settings: BakeSettings::new(Vec3::ZERO, Vec3::ONE, 1.0),
            key: 1,
        }
    }

    fn take(queue: &RepairQueue) -> Job {
        queue.shared.mailbox.lock().unwrap().pending.take().unwrap()
    }

    fn batch(job: &Job) -> RepairBatch {
        sample_batch(job, &mut 0, || Duration::ZERO, || true, |_| Some([7; 4])).unwrap()
    }

    #[test]
    fn newest_complete_request_replaces_pending_work_without_starting_more_workers() {
        let (mut queue, _) = RepairQueue::channel();
        for revision in 1..=100 {
            queue.request(request(revision, &[RepairTarget::Static(revision as u32)]));
        }
        let job = take(&queue);
        assert_eq!(job.request.revision, 100);
        assert_eq!(&*job.request.targets, &[RepairTarget::Static(100)]);
        assert!(job.stamp.current(&queue.shared));
        queue.request(request(
            101,
            &[RepairTarget::Static(100), RepairTarget::Static(101)],
        ));
        assert!(
            !job.stamp.current(&queue.shared),
            "active sampling is cancelled too"
        );
        assert_eq!(take(&queue).request.targets.len(), 2);
    }

    #[test]
    fn stale_deliveries_and_restore_reused_revisions_never_escape_one_batch_poll() {
        let (mut queue, send) = RepairQueue::channel();
        queue.request(request(1, &[RepairTarget::Static(1)]));
        let old = take(&queue);
        send.try_send(batch(&old)).unwrap();
        queue.invalidate();
        queue.request(request(1, &[RepairTarget::Static(2)]));
        let new = take(&queue);
        assert_ne!(old.stamp.generation, new.stamp.generation);
        assert_ne!(old.stamp.cache_epoch, new.stamp.cache_epoch);
        send.try_send(batch(&new)).unwrap();
        assert!(
            queue.poll().is_none(),
            "one poll only discards the stale batch"
        );
        assert_eq!(
            queue.poll().unwrap().patches,
            vec![RepairPatch::Static {
                first: 2,
                lights: vec![[7; 4]]
            }]
        );
        send.try_send(batch(&new)).unwrap();
        new.request.scene.edits.write(0, vec![0, 0, 0]);
        assert!(
            queue.poll().is_none(),
            "an unpublished index revision also invalidates delivery"
        );
    }

    #[test]
    fn channel_and_sampling_limits_keep_sparse_delivery_bounded_and_cancelled() {
        let (mut queue, send) = RepairQueue::channel();
        let targets: Vec<_> = (0..300).map(|i| RepairTarget::Static(i * 2)).collect();
        queue.request(request(1, &targets));
        let job = take(&queue);
        let mut cursor = 0;
        let first = sample_batch(
            &job,
            &mut cursor,
            || Duration::ZERO,
            || true,
            |_| Some([9; 4]),
        )
        .unwrap();
        assert_eq!(first.processed, 128);
        assert_eq!(first.patches.len(), 128, "sparse gaps never gain texels");
        assert!(!first.complete);
        send.try_send(first).unwrap();
        send.try_send(batch(&job)).unwrap();
        assert!(matches!(
            send.try_send(batch(&job)),
            Err(TrySendError::Full(_))
        ));
        let elapsed = Cell::new(Duration::ZERO);
        let limited = sample_batch(
            &job,
            &mut cursor,
            || elapsed.get(),
            || true,
            |_| {
                elapsed.set(elapsed.get() + Duration::from_millis(1));
                Some([8; 4])
            },
        )
        .unwrap();
        assert_eq!(limited.processed, 2);
        let cancelled = Cell::new(false);
        assert!(
            sample_batch(
                &job,
                &mut cursor,
                || Duration::ZERO,
                || !cancelled.get(),
                |_| {
                    cancelled.set(true);
                    Some([6; 4])
                }
            )
            .is_none(),
            "a cancellation during one trace discards its entire unpublished batch"
        );
    }

    #[test]
    fn light_only_requests_reuse_hierarchy_but_edits_epochs_settings_and_restore_do_not() {
        let (mut queue, _) = RepairQueue::channel();
        let first = request(1, &[RepairTarget::Static(0)]);
        let scene = first.scene.clone();
        queue.request(first);
        let mut cache = CachedBaker::default();
        let builds = Cell::new(0);
        let build = |request: &RepairRequest| {
            builds.set(builds.get() + 1);
            SceneBaker::for_repair(
                &request.scene,
                request.instances.as_ref(),
                request.light,
                request.settings,
                request.key,
            )
        };
        assert!(cache.prepare_with(&take(&queue), build).unwrap());
        let mut next = request(2, &[RepairTarget::Static(0)]);
        next.scene = scene.clone();
        next.light.sky = 3.0;
        next.key = 2;
        queue.request(next);
        assert!(cache.prepare_with(&take(&queue), build).unwrap());
        assert_eq!(
            builds.get(),
            1,
            "light and bake key changes retain the hierarchy"
        );
        let mut geometry_epoch = 1;
        let mut settings = request(0, &[]).settings;
        for change in 0..4 {
            let mut next = request(3 + change, &[RepairTarget::Static(0)]);
            next.scene = scene.clone();
            match change {
                0 => geometry_epoch += 1,
                1 => scene.edits.write(0, Vec::new()),
                2 => settings.vertex_rays += 1,
                _ => queue.invalidate(),
            }
            next.geometry_epoch = geometry_epoch;
            next.settings = settings;
            queue.request(next);
            assert!(cache.prepare_with(&take(&queue), build).unwrap());
        }
        assert_eq!(builds.get(), 5);
        let mut next = request(8, &[RepairTarget::Static(0)]);
        next.scene = Arc::new(scene.as_ref().clone());
        next.geometry_epoch = geometry_epoch;
        next.settings = settings;
        let new_scene = next.scene.clone();
        queue.request(next);
        assert!(cache.prepare_with(&take(&queue), build).unwrap());
        assert_eq!(
            builds.get(),
            6,
            "a different world source cannot reuse the hierarchy"
        );
        let rigid = InstancedFigure {
            scene: Arc::new(TexturedScene::default()),
            instances: Arc::new(Vec::new()),
        };
        let mut next = request(9, &[RepairTarget::Static(0)]);
        next.scene = new_scene.clone();
        next.geometry_epoch = geometry_epoch;
        next.settings = settings;
        next.instances = Some(rigid.clone());
        queue.request(next);
        assert!(cache.prepare_with(&take(&queue), build).unwrap());
        assert_eq!(
            builds.get(),
            7,
            "a rigid geometry source joins the hierarchy"
        );
        let mut next = request(10, &[RepairTarget::Static(0)]);
        next.scene = new_scene;
        next.geometry_epoch = geometry_epoch;
        next.settings = settings;
        next.instances = Some(rigid);
        next.light.sky = 4.0;
        queue.request(next);
        assert!(cache.prepare_with(&take(&queue), build).unwrap());
        assert_eq!(
            builds.get(),
            7,
            "unchanged rigid source identities are reusable"
        );
        let mut next = request(10, &[RepairTarget::Static(0)]);
        next.scene = scene.clone();
        queue.request(next);
        let job = take(&queue);
        cache.entry = None;
        assert!(
            !cache
                .prepare_with(&job, |request| {
                    let baker = build(request)?;
                    request.scene.edits.write(0, Vec::new());
                    Ok(baker)
                })
                .unwrap(),
            "an edit during hierarchy construction rejects the result"
        );
        assert!(cache.entry.is_none());
    }

    #[test]
    fn stable_chunk_lookup_transforms_only_selected_local_vertices_across_primitives() {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial::default());
        scene.add_material(TexturedMaterial {
            alpha: AlphaMode::Mask { cutoff: 0.5 },
            ..Default::default()
        });
        scene.add_mesh(TexturedMesh {
            primitives: vec![
                Primitive {
                    material: 0,
                    vertices: vec![TexturedVertex::new(Vec3::ZERO, Vec3::Y, [0.2; 2])],
                    indices: Vec::new(),
                },
                Primitive {
                    material: 1,
                    vertices: vec![TexturedVertex::new(
                        Vec3::X,
                        Vec3::new(1.0, 1.0, 0.0).normalize(),
                        [0.8; 2],
                    )],
                    indices: Vec::new(),
                },
            ],
        });
        let transform = Mat4::from_translation(Vec3::new(4.0, 2.0, 3.0))
            * Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
        let frame = InstancedFigure {
            scene: Arc::new(scene),
            instances: Arc::new(vec![DynamicInstance {
                id: 99,
                mesh: 0,
                current: transform,
                previous: transform,
                color: [1.0; 4],
                light: [0; 4],
                settled: true,
            }]),
        };
        let lookup = ChunkLookup::new(Some(&frame));
        let (vertex, foliage) = lookup.vertex(99, 1).unwrap();
        assert_eq!(vertex.pos, [6.0, 2.0, 3.0]);
        assert_eq!(vertex.uv, [0.8; 2]);
        assert!((Vec3::from(vertex.normal) - Vec3::new(0.5, 1.0, 0.0).normalize()).length() < 1e-6);
        assert!(foliage);
        assert!(!lookup.vertex(99, 0).unwrap().1);
        assert!(lookup.vertex(99, 2).is_none());
        assert!(lookup.vertex(98, 0).is_none());
    }
}
