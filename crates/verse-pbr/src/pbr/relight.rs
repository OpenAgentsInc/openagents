//! Relighting what destruction removes from a baked static scene.
//!
//! A scene's baked light ([`super::textured_bake`]) holds each vertex's sky
//! visibility and one bounce of sunlight under the geometry that stood when
//! it was baked. When a zone hides triangles through the scene's
//! [`IndexEdits`], such as the walls and roof spans a meteor broke, that
//! light goes stale: the ground keeps the shade of a roof that fell, and a
//! floor keeps the darkness of the room around it. The broken pieces' own
//! chunks, lit from the probe grid, keep the grid's old darkness too.
//!
//! A [`Relight`] follows the scene's edits. When they change, it finds the
//! hidden triangles (every index of the triangle the same), and on a worker
//! thread recomputes the light of every vertex and probe within [`REACH`] of
//! them, tracing the same hierarchy the bake traces with the hidden
//! triangles passed through ([`super::bake::Bvh::trace_masked`]). What stands
//! near a gap so keeps the occlusion of what still stands around it, a
//! newly exposed interior stays as dark as its remaining walls make it, and
//! what is gone casts no shade. Everything else keeps its baked light. When
//! nothing is hidden any more, as after a zone restores its buildings, the
//! baked light returns unchanged.
//!
//! The light channel goes to the renderer through the scene's
//! [`super::textured::BakedVertices`], whole, as a bake's does; the probes
//! go back to the zone from [`Relight::poll`]. One job runs at a time; edits
//! that land while it runs start the next when it finishes. Browsers have
//! no threads, so there the baked light stays as it was.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use glam::{IVec3, Vec3};

use super::textured::{IndexEdits, TexturedScene};
use super::textured_bake::{AmbientProbes, BakeGeometry, BakeLight, BakeSettings, SceneBaker};

/// How far from a hidden triangle a vertex or probe has its light
/// recomputed, m: a story and a half, past the ground a house's walls and
/// eaves shade at golden hour.
pub const REACH: f32 = 6.0;
/// The side of the cells that find what lies within [`REACH`], m.
const CELL: f32 = 2.0;
/// The most cells one hidden triangle marks; a larger one marks its
/// corners' cells only.
const MAX_TRIANGLE_CELLS: usize = 512;

/// The triangles of a scene's merged indices that `edits` hide, as their
/// first index over three, in order: every triangle whose three indices are
/// the same.
#[must_use]
pub fn hidden_triangles(edits: &IndexEdits) -> Vec<u32> {
    let (ranges, _) = edits.since(0);
    let mut hidden = Vec::new();
    for (first, indices) in ranges {
        let base = first / 3;
        for (t, tri) in indices.chunks_exact(3).enumerate() {
            if tri[0] == tri[1] && tri[1] == tri[2] {
                hidden.push(base + t as u32);
            }
        }
    }
    hidden.sort_unstable();
    hidden.dedup();
    hidden
}

/// A scene's bake held for relighting: its traced geometry, the merged
/// triangle each occluder came from, and the baked light to start from.
pub struct Relighter {
    baker: SceneBaker,
    /// Each occluder's merged triangle.
    triangles: Vec<u32>,
    /// The merged vertices at each occluder's corners.
    corners: Vec<[u32; 3]>,
    /// The light to start from: what the bake delivered, or what replaced
    /// it since ([`Self::rebase`]).
    base: std::sync::RwLock<(Arc<Vec<[u8; 4]>>, AmbientProbes)>,
}

/// A relit scene: the light channel of every merged vertex, the probes,
/// and how much was recomputed.
#[derive(Clone, Debug, PartialEq)]
pub struct Relit {
    pub lights: Vec<[u8; 4]>,
    pub probes: AmbientProbes,
    /// Triangles hidden, vertices and probes recomputed.
    pub hidden: usize,
    pub vertices: usize,
    pub probes_relit: usize,
    /// How long the relight took on its worker, ms.
    pub ms: f32,
}

/// What one relight recomputed, and how long it took.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RelightStats {
    pub hidden: usize,
    pub vertices: usize,
    pub probes: usize,
    pub ms: f32,
}

impl Relighter {
    /// Reads `scene` as a bake under `light` and `settings` would, starting
    /// from the light the bake delivered, `base_lights`, one a merged
    /// vertex, and its probes.
    ///
    /// # Errors
    ///
    /// Returns the scene's validation error, or a mismatch between the
    /// scene's vertices and `base_lights`.
    pub fn new(
        scene: &TexturedScene,
        light: BakeLight,
        settings: BakeSettings,
        base_lights: Arc<Vec<[u8; 4]>>,
        base_probes: AmbientProbes,
    ) -> Result<Self, String> {
        let geometry = BakeGeometry::new(scene)?;
        if geometry.vertices.len() != base_lights.len() {
            return Err(format!(
                "baked light for {} vertices, scene has {}",
                base_lights.len(),
                geometry.vertices.len()
            ));
        }
        let triangles = geometry.triangles.clone();
        let corners = geometry.corners.clone();
        Ok(Self {
            baker: SceneBaker::from_geometry(geometry, light, settings, 0),
            triangles,
            corners,
            base: std::sync::RwLock::new((base_lights, base_probes)),
        })
    }

    /// Replaces the light to start from, as when offline-baked layers are
    /// combined again for another hour (#10907): the next relight lays the
    /// recomputed light over it.
    pub fn rebase(&self, lights: Arc<Vec<[u8; 4]>>, probes: AmbientProbes) {
        if lights.len() == self.baker.vertex_count() {
            *self
                .base
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = (lights, probes);
        }
    }

    fn base_now(&self) -> (Arc<Vec<[u8; 4]>>, AmbientProbes) {
        self.base
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The baked light and probes, as nothing hidden leaves them.
    #[must_use]
    pub fn base(&self) -> Relit {
        let (lights, probes) = self.base_now();
        Self::from_base(&lights, probes)
    }

    fn from_base(lights: &Arc<Vec<[u8; 4]>>, probes: AmbientProbes) -> Relit {
        Relit {
            lights: lights.as_ref().clone(),
            probes,
            hidden: 0,
            vertices: 0,
            probes_relit: 0,
            ms: 0.0,
        }
    }

    /// The scene's light with the merged triangles `hidden` (sorted, as
    /// [`hidden_triangles`] gives them) gone, or `None` once `cancel` is
    /// set.
    #[must_use]
    pub fn relight(&self, hidden: &[u32], threads: usize, cancel: &AtomicBool) -> Option<Relit> {
        let (base_lights, base_probes) = self.base_now();
        let base = || Self::from_base(&base_lights, base_probes.clone());
        if hidden.is_empty() {
            return Some(base());
        }
        let skip: Vec<bool> = self
            .triangles
            .iter()
            .map(|t| hidden.binary_search(t).is_ok())
            .collect();
        let mut gone = vec![false; base_lights.len()];
        let mut marked: HashSet<IVec3> = HashSet::new();
        for (o, _) in skip.iter().enumerate().filter(|(_, s)| **s) {
            let corners = self.corners[o].map(|i| self.baker.vertex_position(i as usize));
            for &i in &self.corners[o] {
                gone[i as usize] = true;
            }
            mark(&mut marked, corners);
        }
        if marked.is_empty() {
            // Only far levels of detail were hidden: nothing occludes less.
            return Some(Relit {
                hidden: hidden.len(),
                ..base()
            });
        }
        let near = dilate(&marked, (REACH / CELL).ceil() as i32);
        let vertices: Vec<usize> = (0..self.baker.vertex_count())
            .filter(|&i| !gone[i] && near.contains(&cell(self.baker.vertex_position(i))))
            .collect();
        let probes: Vec<usize> = (0..self.baker.probe_count())
            .filter(|&j| near.contains(&cell(self.baker.probe_point(j))))
            .collect();
        let skip = Some(skip.as_slice());
        let lights = run(vertices.len(), threads, cancel, |k| {
            self.baker.bake_vertex(vertices[k], skip)
        })?;
        let traced = run(probes.len(), threads, cancel, |k| {
            self.baker.bake_probe(probes[k], skip)
        })?;
        let mut out = base();
        for (&i, light) in vertices.iter().zip(lights) {
            out.lights[i] = light;
        }
        let mut relit = 0;
        for (&j, (probe, valid)) in probes.iter().zip(traced) {
            // A probe still buried in what stands keeps its dilated light.
            if valid && let Some(slot) = out.probes.grid.data.get_mut(j) {
                *slot = probe;
                relit += 1;
            }
        }
        // A new grid content, never zero and never a studio key's.
        let digest = hidden.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &t| {
            (h ^ u64::from(t)).wrapping_mul(0x0100_0000_01b3)
        });
        out.probes.grid.version = (base_probes.grid.version ^ (digest << 1)) | 1;
        out.hidden = hidden.len();
        out.vertices = vertices.len();
        out.probes_relit = relit;
        Some(out)
    }
}

/// `work` for each index below `count`, on `threads` workers where the
/// target has threads.
fn run<T: Send>(
    count: usize,
    threads: usize,
    cancel: &AtomicBool,
    work: impl Fn(usize) -> T + Sync,
) -> Option<Vec<T>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        super::textured_bake::parallel(count, threads.max(1), cancel, work)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = threads;
        let out: Vec<T> = (0..count).map(work).collect();
        (!cancel.load(Ordering::Relaxed)).then_some(out)
    }
}

/// The cell holding `p`.
fn cell(p: Vec3) -> IVec3 {
    (p / CELL).floor().as_ivec3()
}

/// Marks the cells a triangle's bounds cover, or its corners' cells when
/// its bounds cover too many.
fn mark(marked: &mut HashSet<IVec3>, corners: [Vec3; 3]) {
    if !corners.iter().all(|c| c.is_finite()) {
        return;
    }
    let lo = cell(corners[0].min(corners[1]).min(corners[2]));
    let hi = cell(corners[0].max(corners[1]).max(corners[2]));
    let size = (hi - lo + IVec3::ONE).as_i64vec3();
    if (size.x * size.y * size.z) as usize > MAX_TRIANGLE_CELLS {
        marked.extend(corners.map(cell));
        return;
    }
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                marked.insert(IVec3::new(x, y, z));
            }
        }
    }
}

/// Every cell within `r` cells of a marked one along each axis.
fn dilate(marked: &HashSet<IVec3>, r: i32) -> HashSet<IVec3> {
    let mut out = HashSet::with_capacity(marked.len() * 8);
    for c in marked {
        for x in -r..=r {
            for y in -r..=r {
                for z in -r..=r {
                    out.insert(*c + IVec3::new(x, y, z));
                }
            }
        }
    }
    out
}

/// What a [`Relight`] is doing.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
enum State {
    /// Reading the scene into a [`Relighter`] off the main thread.
    #[cfg(not(target_arch = "wasm32"))]
    Building(std::sync::mpsc::Receiver<Option<Relighter>>),
    Ready(Arc<Relighter>),
    Failed,
}

/// A scene's baked light following its edits: see the module docs.
pub struct Relight {
    scene: Arc<TexturedScene>,
    state: State,
    /// The edits' revision the light shows or is being relit for.
    seen: u64,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<std::sync::mpsc::Receiver<Option<Relit>>>,
    cancel: Arc<AtomicBool>,
    /// What the last relight recomputed.
    last: Option<RelightStats>,
    /// A new light to start from, waiting for the relighter to be built.
    rebased: Option<(Arc<Vec<[u8; 4]>>, AmbientProbes)>,
}

impl Relight {
    /// Follows `scene`'s edits from the light a bake under `light` and
    /// `settings` delivered, `base_lights`, and its probes. The scene is
    /// read into a [`Relighter`] on a worker thread.
    #[must_use]
    pub fn start(
        scene: Arc<TexturedScene>,
        light: BakeLight,
        settings: BakeSettings,
        base_lights: Arc<Vec<[u8; 4]>>,
        base_probes: AmbientProbes,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let state = Self::build(&scene, light, settings, base_lights, base_probes);
        Self {
            scene,
            state,
            seen: 0,
            #[cfg(not(target_arch = "wasm32"))]
            pending: None,
            cancel,
            last: None,
            rebased: None,
        }
    }

    /// Starts from `lights` and `probes` from now on, as when the light the
    /// bake delivered is combined again for another hour, and lays the
    /// relit light over them again ([`Relighter::rebase`]). Until the next
    /// relight lands, nothing is delivered: the caller's light stays.
    pub fn rebase(&mut self, lights: Arc<Vec<[u8; 4]>>, probes: AmbientProbes) {
        match &self.state {
            State::Ready(relighter) => {
                relighter.rebase(lights, probes);
                // Run again whatever the edits' revision.
                self.seen = u64::MAX;
            }
            State::Failed => {
                self.scene.baked.deliver_lights(lights.as_ref().clone());
            }
            #[cfg(not(target_arch = "wasm32"))]
            State::Building(_) => self.rebased = Some((lights, probes)),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build(
        scene: &Arc<TexturedScene>,
        light: BakeLight,
        settings: BakeSettings,
        base_lights: Arc<Vec<[u8; 4]>>,
        base_probes: AmbientProbes,
    ) -> State {
        let (send, receive) = std::sync::mpsc::channel();
        let scene = scene.clone();
        let spawned = std::thread::Builder::new()
            .name("verse-relight-build".into())
            .spawn(move || {
                let built = Relighter::new(&scene, light, settings, base_lights, base_probes)
                    .map_err(|error| eprintln!("verse: relighting unavailable: {error}"))
                    .ok();
                let _ = send.send(built);
            });
        match spawned {
            Ok(_) => State::Building(receive),
            Err(_) => State::Failed,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn build(
        _: &Arc<TexturedScene>,
        _: BakeLight,
        _: BakeSettings,
        _: Arc<Vec<[u8; 4]>>,
        _: AmbientProbes,
    ) -> State {
        State::Failed
    }

    /// Whether the relighter is built and no relight is running or due.
    #[must_use]
    pub fn settled(&self) -> bool {
        match &self.state {
            State::Ready(_) => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.pending.is_some() {
                    return false;
                }
                self.seen == self.scene.edits.revision()
            }
            State::Failed => true,
            #[cfg(not(target_arch = "wasm32"))]
            State::Building(_) => false,
        }
    }

    /// Triangles hidden, vertices and probes recomputed by the last
    /// relight to land.
    #[must_use]
    pub fn last(&self) -> Option<RelightStats> {
        self.last
    }

    /// Advances the relight: takes a finished one, delivering its light
    /// channel to the scene and returning its probes, and starts the next
    /// when the scene's edits changed.
    pub fn poll(&mut self) -> Option<AmbientProbes> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let State::Building(receive) = &self.state {
                self.state = match receive.try_recv() {
                    Ok(Some(relighter)) => {
                        if let Some((lights, probes)) = self.rebased.take() {
                            relighter.rebase(lights, probes);
                            self.seen = u64::MAX;
                        }
                        State::Ready(Arc::new(relighter))
                    }
                    Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => State::Failed,
                    Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                };
            }
            let State::Ready(relighter) = &self.state else {
                return None;
            };
            let mut landed = None;
            if let Some(pending) = &self.pending {
                match pending.try_recv() {
                    Ok(relit) => {
                        self.pending = None;
                        if let Some(relit) = relit {
                            self.last = Some(RelightStats {
                                hidden: relit.hidden,
                                vertices: relit.vertices,
                                probes: relit.probes_relit,
                                ms: relit.ms,
                            });
                            self.scene.baked.deliver_lights(relit.lights);
                            landed = Some(relit.probes);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pending = None,
                }
            }
            let revision = self.scene.edits.revision();
            if revision != self.seen {
                self.seen = revision;
                let relighter = relighter.clone();
                let edits = self.scene.edits.clone();
                let cancel = self.cancel.clone();
                let (send, receive) = std::sync::mpsc::channel();
                let threads = std::thread::available_parallelism()
                    .map_or(1, std::num::NonZeroUsize::get)
                    .clamp(1, 6);
                let spawned = std::thread::Builder::new()
                    .name("verse-relight".into())
                    .spawn(move || {
                        let started = std::time::Instant::now();
                        let hidden = hidden_triangles(&edits);
                        let relit = relighter.relight(&hidden, threads, &cancel).map(|mut r| {
                            r.ms = started.elapsed().as_secs_f32() * 1000.0;
                            r
                        });
                        let _ = send.send(relit);
                    });
                if spawned.is_ok() {
                    self.pending = Some(receive);
                }
            }
            landed
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    /// Waits for the relighter and any relight due, for offline captures,
    /// and returns the last probes to land.
    pub fn settle(&mut self) -> Option<AmbientProbes> {
        let mut probes = None;
        for _ in 0..30_000 {
            if let Some(landed) = self.poll() {
                probes = Some(landed);
            }
            if self.settled() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        probes
    }
}

impl Drop for Relight {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbr::textured::{Primitive, TexturedMaterial, TexturedMesh, TexturedVertex};
    use crate::pbr::textured_bake::{SceneBaker, decode};
    use glam::Mat4;

    const LIGHT: BakeLight = BakeLight {
        sun_dir: Vec3::new(0.0, 0.8, 0.6),
        sun_illuminance: 4_000.0,
        sky: 1_200.0,
        ground: 450.0,
    };

    fn settings() -> BakeSettings {
        BakeSettings {
            vertex_rays: 96,
            probe_rays: 128,
            reach: 50.0,
            probe_min: Vec3::new(-4.0, 0.5, -4.0),
            probe_max: Vec3::new(4.0, 1.5, 4.0),
            probe_cell: 1.0,
        }
    }

    /// A grid of quads `n` by `n` across `half` m around `center`, facing
    /// `normal`, so vertices lie under and beside what stands.
    fn grid(center: Vec3, u: Vec3, v: Vec3, normal: Vec3, n: u32) -> TexturedMesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                let a = i as f32 / n as f32 * 2.0 - 1.0;
                let b = j as f32 / n as f32 * 2.0 - 1.0;
                vertices.push(TexturedVertex::new(
                    center + u * a + v * b,
                    normal,
                    [0.0, 0.0],
                ));
            }
        }
        for j in 0..n {
            for i in 0..n {
                let k = j * (n + 1) + i;
                indices.extend([k, k + 1, k + n + 2, k, k + n + 2, k + n + 1]);
            }
        }
        TexturedMesh {
            primitives: vec![Primitive {
                vertices,
                indices,
                material: 0,
            }],
        }
    }

    /// Ground 40 m across and a roof 2 m over its middle, the roof last.
    fn shelter() -> TexturedScene {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial {
            base_color: [0.4, 0.4, 0.4, 1.0],
            ..TexturedMaterial::default()
        });
        let ground = scene.add_mesh(grid(
            Vec3::ZERO,
            Vec3::X * 20.0,
            Vec3::Z * 20.0,
            Vec3::Y,
            40,
        ));
        scene.place(ground, Mat4::IDENTITY);
        let roof = scene.add_mesh(grid(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::X * 3.0,
            Vec3::Z * 3.0,
            -Vec3::Y,
            2,
        ));
        scene.place(roof, Mat4::IDENTITY);
        scene
    }

    fn baked(scene: &TexturedScene) -> (Arc<Vec<[u8; 4]>>, AmbientProbes) {
        let bake = SceneBaker::new(scene, LIGHT, settings(), 7)
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        (
            Arc::new(bake.vertices.iter().map(|v| v.light).collect()),
            bake.probes,
        )
    }

    /// Hides the roof's triangles through the scene's edits.
    fn hide_roof(scene: &TexturedScene) {
        let ranges = scene.index_ranges();
        for range in &ranges[1] {
            scene
                .edits
                .write(range.first, vec![range.base; range.count as usize]);
        }
    }

    /// The light channel at the merged vertex nearest `p` facing up.
    fn at(scene: &TexturedScene, lights: &[[u8; 4]], p: Vec3) -> f32 {
        let merged = scene.merge().unwrap();
        let (i, _) = merged
            .vertices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.normal[1] > 0.5)
            .min_by(|a, b| {
                Vec3::from(a.1.pos)
                    .distance(p)
                    .total_cmp(&Vec3::from(b.1.pos).distance(p))
            })
            .unwrap();
        decode(lights[i]).1
    }

    #[test]
    fn hidden_triangles_are_the_degenerate_ones() {
        let scene = shelter();
        assert!(hidden_triangles(&scene.edits).is_empty());
        hide_roof(&scene);
        let hidden = hidden_triangles(&scene.edits);
        // The roof's two by two quads.
        assert_eq!(hidden.len(), 8);
        let range = scene.index_ranges()[1][0];
        assert_eq!(hidden[0], range.first / 3);
    }

    #[test]
    fn the_ground_under_a_fallen_roof_sees_the_sky_again_and_restores() {
        let scene = shelter();
        let (lights, probes) = baked(&scene);
        let under = Vec3::new(0.0, 0.0, 0.0);
        let away = Vec3::new(15.0, 0.0, 15.0);
        assert!(
            at(&scene, &lights, under) < 0.4,
            "the roof shades the ground"
        );
        let relighter =
            Relighter::new(&scene, LIGHT, settings(), lights.clone(), probes.clone()).unwrap();
        hide_roof(&scene);
        let hidden = hidden_triangles(&scene.edits);
        let relit = relighter
            .relight(&hidden, 2, &AtomicBool::new(false))
            .unwrap();
        // No floating shade where the roof was; far ground untouched.
        assert!(at(&scene, &relit.lights, under) > 0.95);
        assert_eq!(
            at(&scene, &relit.lights, away),
            at(&scene, &lights, away),
            "light beyond the reach stays baked"
        );
        assert!(relit.vertices > 0 && relit.vertices < lights.len());
        // The probes under the roof brighten too, so rubble lit by them does.
        let n = Vec3::Y;
        let p = Vec3::new(0.0, 1.0, 0.0);
        assert!(relit.probes.irradiance(p, n).x > probes.irradiance(p, n).x * 1.3);
        assert_ne!(relit.probes.grid.version, probes.grid.version);
        // Nothing hidden: the baked light exactly.
        let restored = relighter.relight(&[], 2, &AtomicBool::new(false)).unwrap();
        assert_eq!(restored.lights, *lights);
        assert_eq!(restored.probes, probes);
    }

    #[test]
    fn a_relight_follows_the_edits_and_delivers_to_the_renderer() {
        let scene = Arc::new(shelter());
        let (lights, probes) = baked(&scene);
        let mut relight = Relight::start(scene.clone(), LIGHT, settings(), lights, probes);
        relight.settle();
        assert!(scene.baked.take().is_none(), "nothing hidden, nothing new");
        hide_roof(&scene);
        assert!(!relight.settled());
        assert!(relight.settle().is_some());
        let delivered = scene.baked.take().unwrap();
        assert!(at(&scene, &delivered, Vec3::ZERO) > 0.95);
        assert_eq!(relight.last().map(|l| l.hidden), Some(8));
        // Showing the roof again brings its shade back.
        for range in &scene.index_ranges()[1] {
            scene
                .edits
                .write(range.first, scene.range_indices(1, range));
        }
        assert!(relight.settle().is_some());
        let restored = scene.baked.take().unwrap();
        assert!(at(&scene, &restored, Vec3::ZERO) < 0.4);
    }

    #[test]
    fn a_new_base_keeps_the_gap_relit_and_returns_whole_on_restore() {
        let scene = Arc::new(shelter());
        let (lights, probes) = baked(&scene);
        let mut relight = Relight::start(
            scene.clone(),
            LIGHT,
            settings(),
            lights.clone(),
            probes.clone(),
        );
        relight.settle();
        hide_roof(&scene);
        relight.settle();
        scene.baked.take();
        // Another hour's light, darker everywhere: far ground takes it, and
        // the ground under the fallen roof stays relit, open to the sky.
        let darker: Vec<[u8; 4]> = lights
            .iter()
            .map(|l| [l[0] / 2, l[1] / 2, l[2] / 2, l[3]])
            .collect();
        let darker = Arc::new(darker);
        relight.rebase(darker.clone(), probes.clone());
        assert!(!relight.settled());
        relight.settle();
        let delivered = scene.baked.take().unwrap();
        let away = Vec3::new(15.0, 0.0, 15.0);
        assert_eq!(at(&scene, &delivered, away), at(&scene, &darker, away));
        assert!(at(&scene, &delivered, Vec3::ZERO) > 0.95);
        // Restored, the new light exactly.
        for range in &scene.index_ranges()[1] {
            scene
                .edits
                .write(range.first, scene.range_indices(1, range));
        }
        relight.settle();
        assert_eq!(scene.baked.take().unwrap(), *darker);
    }

    #[test]
    fn a_cancelled_relight_returns_nothing() {
        let scene = shelter();
        let (lights, probes) = baked(&scene);
        let relighter = Relighter::new(&scene, LIGHT, settings(), lights, probes).unwrap();
        hide_roof(&scene);
        let hidden = hidden_triangles(&scene.edits);
        assert!(
            relighter
                .relight(&hidden, 2, &AtomicBool::new(true))
                .is_none()
        );
    }
}
