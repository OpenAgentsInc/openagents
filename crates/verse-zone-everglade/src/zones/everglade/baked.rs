//! The town's immutable light layers, interpolated on the GPU as its clock
//! moves, and selective dynamic lighting after a support graph changes.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::pbr::textured::{LightPatch, TexturedScene};
use crate::pbr::textured_bake::{AmbientProbes, LayerChoice};
use crate::zones::everglade_pack::kit_bake;
use verse_pbr::pbr::baked_layers::Layers;

use super::time_of_day;

/// The sky and exposure keep their existing cadence; sun weights follow
/// the exact clock between those updates.
pub(super) fn clock_light(mut light: time_of_day::Light, time: town_clock::TownTime,
    pinned: bool) -> time_of_day::Light {
    if !pinned {
        let exact = time_of_day::Light::at_hours((time.second / 3600.0) as f32);
        light.key_dir = exact.key_dir;
        light.sun = exact.sun;
    }
    light
}

pub(super) fn choice(light: &time_of_day::Light) -> Option<LayerChoice> {
    let layers = kit_bake::offered()?;
    Some(LayerChoice {
        sun: layers.sun_blend(light.sun),
        ratio: sun_ratio(&layers, light),
        layers,
    })
}

fn sun_ratio(layers: &Layers, light: &time_of_day::Light) -> f32 {
    // The baked suns are daylight. Moonlight keeps the stage's direct key
    // and readable sky floor, without reviving a daytime bounce at night.
    if light.sun.y <= 0.0 { return 0.0; }
    layers.sun_ratio(light.key_lux, light.sky_lux)
}

/// The existing dusk/dawn schedule, with a smooth fade around lamp ignition.
#[must_use]
pub fn lamp_intensity(light: &time_of_day::Light) -> f32 {
    let t = ((light.night - 0.15) / 0.3).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) struct BakedLight {
    layers: Arc<Layers>,
    scene: Arc<TexturedScene>,
    pub(super) active: bool,
    hour: f32,
    revision: u64,
    dynamic: BTreeSet<u32>,
    #[cfg(not(target_arch = "wasm32"))]
    repair: RepairQueue,
}

#[cfg(not(target_arch = "wasm32"))]
struct Repair {
    cancel: Arc<std::sync::atomic::AtomicBool>,
    receive: std::sync::mpsc::Receiver<Vec<LightPatch>>,
    worker: std::thread::JoinHandle<()>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Repair {
    fn drop(&mut self) {
        self.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct RepairRequest {
    revision: u64,
    vertices: BTreeSet<u32>,
    light: crate::pbr::textured_bake::BakeLight,
}

/// One worker owns the scene hierarchy; newer requests replace only the
/// pending request until that worker exits. Polling never joins a thread.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct RepairQueue {
    active: Option<Repair>,
    pending: Option<RepairRequest>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RepairQueue {
    fn request(&mut self, request: RepairRequest) {
        if let Some(active) = &self.active {
            active.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.pending = (!request.vertices.is_empty()).then_some(request);
    }

    fn poll(&mut self, mut start: impl FnMut(RepairRequest) -> Option<Repair>) -> Vec<LightPatch> {
        let mut delivered = Vec::new();
        if let Some(active) = &self.active {
            if !active.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                delivered = active.receive.try_recv().unwrap_or_default();
            }
            if active.worker.is_finished() {
                self.active = None;
            }
        }
        if self.active.is_none() && let Some(request) = self.pending.take() {
            self.active = start(request);
        }
        delivered
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_repair(scene: Arc<TexturedScene>, request: RepairRequest) -> Option<Repair> {
    use crate::pbr::textured_bake::{BakeSettings, SceneBaker};
    use std::sync::atomic::{AtomicBool, Ordering};
    let cancel = Arc::new(AtomicBool::new(false));
    let stopped = cancel.clone();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::Builder::new().name("verse-damaged-light".into()).spawn(move || {
        if stopped.load(Ordering::Relaxed) { return; }
        let settings = BakeSettings::new(glam::Vec3::ZERO, glam::Vec3::ONE, 1.0);
        let Ok(baker) = SceneBaker::new(&scene, request.light, settings, request.revision) else { return; };
        let mut vertices = request.vertices.into_iter().peekable();
        while vertices.peek().is_some() && !stopped.load(Ordering::Relaxed) {
            let start = std::time::Instant::now();
            let mut batch = Vec::new();
            while let Some(index) = vertices.next() {
                if let Some(light) = baker.vertex_light(index as usize) { batch.push((index, light)); }
                if start.elapsed() >= std::time::Duration::from_millis(2) { break; }
            }
            let mut cursor = batch.iter();
            let mut edits = patches(batch.iter().map(|(i, _)| *i), true,
                |_| cursor.next().map_or([0; 4], |(_, light)| *light));
            loop {
                if stopped.load(Ordering::Relaxed) { return; }
                match send.try_send(edits) {
                    Ok(()) => break,
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return,
                    Err(std::sync::mpsc::TrySendError::Full(pending)) => edits = pending,
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }).ok()?;
    Some(Repair { cancel, receive, worker })
}

/// Coalesces adjacent vertices so damage uploads change only occupied rows.
fn patches(indices: impl IntoIterator<Item = u32>, dynamic: bool,
    mut light: impl FnMut(u32) -> [u8; 4]) -> Vec<LightPatch> {
    let mut patches: Vec<LightPatch> = Vec::new();
    for index in indices {
        if let Some(last) = patches.last_mut()
            && last.first + last.lights.len() as u32 == index {
            last.lights.push(light(index));
        } else {
            patches.push(LightPatch { first: index, lights: vec![light(index)], dynamic });
        }
    }
    patches
}

impl BakedLight {
    pub(super) fn damage_town(&mut self, town: &super::demolition::town::Town,
        light: &time_of_day::Light) {
        let revision = self.scene.edits.revision();
        if self.active && (revision != self.revision || (!self.dynamic.is_empty() && self.hour != light.hours)) {
            self.damage(revision, town.lighting_vertices(), light);
        }
    }
    pub(super) fn new(choice: &LayerChoice, scene: Arc<TexturedScene>) -> Self {
        Self {
            layers: choice.layers.clone(), scene, active: false,
            hour: f32::NAN, revision: u64::MAX, dynamic: BTreeSet::new(),
            #[cfg(not(target_arch = "wasm32"))]
            repair: RepairQueue::default(),
        }
    }

    /// Only four scalars change each frame; vertex light is never recombined.
    pub(super) fn sun(&self, light: &time_of_day::Light) -> [f32; 4] {
        if !self.active { return [0.0; 4]; }
        self.layers.sun_blend(light.sun)
            .uniform(sun_ratio(&self.layers, light))
    }

    pub(super) fn update(&mut self, light: &time_of_day::Light) -> Option<AmbientProbes> {
        if !self.active { return None; }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let scene = self.scene.clone();
            // At most one two-millisecond batch reaches the renderer per tick.
            let patches = self.repair.poll(|request| start_repair(scene.clone(), request));
            self.scene.baked.deliver_patches(patches);
        }
        if self.hour == light.hours { return None; }
        self.hour = light.hours;
        Some(self.layers.blended_probes(self.layers.sun_blend(light.sun),
            sun_ratio(&self.layers, light)))
    }

    /// Invalidates stale shadows immediately; a worker repairs only the changed
    /// pieces and support neighbors, in two-millisecond batches.
    pub(super) fn damage(&mut self, revision: u64, dynamic: BTreeSet<u32>, light: &time_of_day::Light) {
        if !self.active || (revision == self.revision && self.hour == light.hours) { return; }
        self.revision = revision;
        let restored = self.dynamic.difference(&dynamic).copied();
        self.scene.baked.deliver_patches(patches(restored, false,
            |i| self.layers.sky.get(i as usize).copied().unwrap_or([0; 4])));
        self.scene.baked.deliver_patches(patches(dynamic.difference(&self.dynamic).copied(), true, |_| [0; 4]));
        self.dynamic = dynamic;
        #[cfg(not(target_arch = "wasm32"))]
        self.repair.request(RepairRequest {
            revision, vertices: self.dynamic.clone(),
            light: crate::pbr::textured_bake::BakeLight::from_key(&light.key(super::Everglade::afternoon())),
        });
        #[cfg(target_arch = "wasm32")]
        let _ = light;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn rapid_damage_and_clock_changes_keep_one_worker_and_deliver_the_latest_request() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(AtomicBool::new(false));
        let mut started = Vec::new();
        let mut start = |request: RepairRequest| {
            started.push((request.revision, request.light.sun_illuminance));
            let cancel = Arc::new(AtomicBool::new(false));
            let (send, receive) = std::sync::mpsc::channel();
            let (active, maximum, gate) = (active.clone(), maximum.clone(), gate.clone());
            let worker = std::thread::spawn(move || {
                maximum.fetch_max(active.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                // Simulate hierarchy construction that cannot stop mid-build.
                while !gate.load(Ordering::SeqCst) { std::thread::yield_now(); }
                let _ = send.send(vec![LightPatch { first: request.revision as u32,
                    lights: vec![[request.light.sun_illuminance as u8, 0, 0, 255]], dynamic: true }]);
                active.fetch_sub(1, Ordering::SeqCst);
            });
            Some(Repair { cancel, receive, worker })
        };
        let mut queue = RepairQueue::default();
        let mut light = crate::pbr::textured_bake::BakeLight::from_key(&time_of_day::Light::at_hours(12.0).key(super::super::Everglade::afternoon()));
        queue.request(RepairRequest { revision: 1, vertices: BTreeSet::from([1]), light });
        assert!(queue.poll(&mut start).is_empty());
        for revision in 2..=20 {
            queue.request(RepairRequest { revision, vertices: BTreeSet::from([revision as u32]), light });
            assert!(queue.poll(&mut start).is_empty());
        }
        light.sun_illuminance = 99.0;
        queue.request(RepairRequest { revision: 20, vertices: BTreeSet::from([20]), light });
        gate.store(true, Ordering::SeqCst);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let delivered = loop {
            let delivered = queue.poll(&mut start);
            if !delivered.is_empty() { break delivered; }
            assert!(std::time::Instant::now() < deadline, "latest repair finishes");
            std::thread::yield_now();
        };
        assert_eq!(delivered[0].first, 20);
        assert_eq!(delivered[0].lights[0][0], 99);
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
        assert_eq!(started.len(), 2);
        assert_eq!(started[1], (20, 99.0));
    }

    #[test]
    fn production_clock_adapter_moves_sun_weights_between_sky_steps() {
        let before = town_clock::TownTime::at_hour(0, 9.0);
        let after = town_clock::TownTime::at_hour(0, 9.0 + 3.0 / 60.0);
        let stepped = time_of_day::Light::at(before);
        assert_eq!(stepped, time_of_day::Light::at(after));
        let a = clock_light(stepped, before, false);
        let b = clock_light(stepped, after, false);
        assert_ne!(a.key_dir, b.key_dir);
        assert!(a.key_dir.angle_between(b.key_dir) < 0.02);
        assert_eq!(clock_light(stepped, after, true), stepped);
    }

    #[test]
    fn baked_lamps_burn_at_night_and_rest_by_day() {
        assert_eq!(lamp_intensity(&time_of_day::Light::at_hours(12.0)), 0.0);
        assert!(lamp_intensity(&time_of_day::Light::at_hours(23.0)) > 0.99);
        let dusk = time_of_day::Light::at_hours(19.0);
        assert!(dusk.lamps_lit() && lamp_intensity(&dusk) > 0.0);
    }

    #[test]
    fn damage_patches_coalesce_only_adjacent_vertices() {
        let edited = patches([2, 3, 8], true, |_| [0; 4]);
        assert_eq!(edited.len(), 2);
        assert_eq!(edited[0].first, 2);
        assert_eq!(edited[0].lights.len(), 2);
        assert_eq!(edited[1].first, 8);
        assert!(edited.iter().all(|p| p.dynamic));
    }

    #[test]
    fn restore_recovers_original_sky_and_clears_the_dynamic_mask() {
        let layers = Arc::new(Layers {
            bake_key: "synthetic".into(), scene: "synthetic".into(),
            reference: verse_pbr::pbr::baked_layers::Reference { sun: 4000.0, sky: 1200.0, ground: 450.0 },
            sky: vec![[90, 80, 70, 200]; 8], sky_probes: Vec::new(), suns: Vec::new(), lamps: Vec::new(),
            probe_origin: [0.0; 3], probe_cell: 1.0, probe_dims: [0; 3],
        });
        let scene = Arc::new(TexturedScene::default());
        let mut baked = BakedLight::new(&LayerChoice { layers, sun: Default::default(), ratio: 0.0 }, scene.clone());
        baked.active = true;
        baked.dynamic = BTreeSet::from([2, 3]);
        baked.damage(1, BTreeSet::from([2, 3]), &time_of_day::Light::at_hours(12.0));
        assert!(scene.baked.take_patches().is_empty(), "queued repairs retain their current lighting");
        baked.damage(1, BTreeSet::new(), &time_of_day::Light::at_hours(12.0));
        let edits = scene.baked.take_patches();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].first, 2);
        assert_eq!(edits[0].lights, vec![[90, 80, 70, 200]; 2]);
        assert!(!edits[0].dynamic);
    }

    #[test]
    fn lamp_fade_is_continuous_through_dusk_and_dawn() {
        let mut previous = lamp_intensity(&time_of_day::Light::at_hours(0.0));
        for minute in 1..=1440 {
            let intensity = lamp_intensity(&time_of_day::Light::at_hours(minute as f32 / 60.0));
            assert!((intensity - previous).abs() < 0.06, "minute {minute}");
            previous = intensity;
        }
    }

    #[test]
    fn town_clock_blends_continuously_across_every_sun_pair() {
        use verse_pbr::pbr::baked_layers::{Reference, SunLayer};
        let mut layers = Layers {
            bake_key: "synthetic".into(), scene: "synthetic".into(),
            reference: Reference { sun: 4000.0, sky: 1200.0, ground: 450.0 },
            sky: vec![[100, 100, 100, 255]], sky_probes: vec![[1.0; 12]],
            suns: Vec::new(), lamps: Vec::new(), probe_origin: [0.0; 3], probe_cell: 1.0,
            probe_dims: [1; 3],
        };
        for (i, hour) in [8.0, 12.0, 15.5, 17.5].into_iter().enumerate() {
            layers.suns.push(SunLayer { dir: time_of_day::Light::at_hours(hour).sun.to_array(),
                vertices: vec![[100, 100, 100, 255]], probes: vec![[i as f32; 12]] });
        }
        let sample = |light: &time_of_day::Light| {
            layers.blended_probes(layers.sun_blend(light.sun), sun_ratio(&layers, light)).grid.data[0][0]
        };
        let mut last = sample(&time_of_day::Light::at_hours(0.0));
        let mut pairs = BTreeSet::new();
        for second in (1..=86400).step_by(10) {
            let light = time_of_day::Light::at_hours(second as f32 / 3600.0);
            let now = sample(&light);
            assert!((now - last).abs() < 0.03, "second {second}: {last} to {now}");
            if light.sun.y > 0.0 {
                let blend = layers.sun_blend(light.sun);
                pairs.insert((blend.first, blend.second));
            }
            last = now;
        }
        assert!(pairs.len() >= 5, "the clock crosses all adjacent layer pairs and both endpoints");
        assert_eq!(sun_ratio(&layers, &time_of_day::Light::at_hours(0.0)), 0.0);
    }
}
