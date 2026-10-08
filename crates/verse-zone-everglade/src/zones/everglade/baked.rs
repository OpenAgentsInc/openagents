//! The town's immutable light layers, interpolated on the GPU as its clock
//! moves, and selective dynamic lighting after a support graph changes.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::pbr::textured::{LightPatch, TexturedScene};
use crate::pbr::textured_bake::{AmbientProbes, LayerChoice};
use crate::zones::everglade_pack::kit_bake;
use verse_pbr::pbr::baked_layers::Layers;

use super::time_of_day;

pub(super) fn choice(light: &time_of_day::Light) -> Option<LayerChoice> {
    let layers = kit_bake::offered()?;
    Some(LayerChoice {
        sun: layers.sun_blend(light.key_dir),
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
    repair: Option<Repair>,
}

#[cfg(not(target_arch = "wasm32"))]
struct Repair {
    cancel: Arc<std::sync::atomic::AtomicBool>,
    receive: std::sync::mpsc::Receiver<Vec<LightPatch>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Repair {
    fn drop(&mut self) {
        self.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
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
        if self.active && revision != self.revision {
            self.damage(revision, town.lighting_vertices(), light);
        }
    }
    pub(super) fn new(choice: &LayerChoice, scene: Arc<TexturedScene>) -> Self {
        Self {
            layers: choice.layers.clone(), scene, active: false,
            hour: f32::NAN, revision: u64::MAX, dynamic: BTreeSet::new(),
            #[cfg(not(target_arch = "wasm32"))]
            repair: None,
        }
    }

    /// Only four scalars change each frame; vertex light is never recombined.
    pub(super) fn sun(&self, light: &time_of_day::Light) -> [f32; 4] {
        if !self.active { return [0.0; 4]; }
        self.layers.sun_blend(light.key_dir)
            .uniform(sun_ratio(&self.layers, light))
    }

    pub(super) fn update(&mut self, light: &time_of_day::Light) -> Option<AmbientProbes> {
        if !self.active { return None; }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(repair) = &self.repair {
            // At most one two-millisecond batch reaches the renderer per tick.
            if let Ok(patches) = repair.receive.try_recv() {
                self.scene.baked.deliver_patches(patches);
            }
        }
        if self.hour == light.hours { return None; }
        self.hour = light.hours;
        Some(self.layers.blended_probes(self.layers.sun_blend(light.key_dir),
            sun_ratio(&self.layers, light)))
    }

    /// Invalidates stale shadows immediately; a worker repairs only the changed
    /// pieces and support neighbors, in two-millisecond batches.
    pub(super) fn damage(&mut self, revision: u64, dynamic: BTreeSet<u32>, light: &time_of_day::Light) {
        if !self.active || revision == self.revision { return; }
        self.revision = revision;
        #[cfg(not(target_arch = "wasm32"))]
        { self.repair = None; }
        let restored = self.dynamic.difference(&dynamic).copied();
        self.scene.baked.deliver_patches(patches(restored, false,
            |i| self.layers.sky.get(i as usize).copied().unwrap_or([0; 4])));
        self.scene.baked.deliver_patches(patches(dynamic.iter().copied(), true, |_| [0; 4]));
        self.dynamic = dynamic;
        #[cfg(not(target_arch = "wasm32"))]
        if !self.dynamic.is_empty() {
            use crate::pbr::textured_bake::{BakeLight, BakeSettings, SceneBaker};
            use std::sync::atomic::{AtomicBool, Ordering};
            let scene = self.scene.clone();
            let vertices = self.dynamic.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            let stopped = cancel.clone();
            let (send, receive) = std::sync::mpsc::sync_channel(1);
            let bake_light = BakeLight::from_key(&light.key(super::Everglade::afternoon()));
            let spawned = std::thread::Builder::new().name("verse-damaged-light".into()).spawn(move || {
                let settings = BakeSettings::new(glam::Vec3::ZERO, glam::Vec3::ONE, 1.0);
                let Ok(baker) = SceneBaker::new(&scene, bake_light, settings, revision) else { return; };
                let mut vertices = vertices.into_iter().peekable();
                while vertices.peek().is_some() && !stopped.load(Ordering::Relaxed) {
                    let start = std::time::Instant::now();
                    let mut batch = Vec::new();
                    while let Some(index) = vertices.next() {
                        if let Some(light) = baker.vertex_light(index as usize) {
                            batch.push((index, light));
                        }
                        if start.elapsed() >= std::time::Duration::from_millis(2) { break; }
                    }
                    let mut cursor = batch.iter();
                    let edits = patches(batch.iter().map(|(i, _)| *i), true,
                        |_| cursor.next().map_or([0; 4], |(_, light)| *light));
                    if stopped.load(Ordering::Relaxed) || send.send(edits).is_err() { return; }
                    std::thread::sleep(std::time::Duration::from_millis(16));
                }
            });
            if spawned.is_ok() { self.repair = Some(Repair { cancel, receive }); }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = light;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            layers.blended_probes(layers.sun_blend(light.key_dir), sun_ratio(&layers, light)).grid.data[0][0]
        };
        let mut last = sample(&time_of_day::Light::at_hours(0.0));
        let mut pairs = BTreeSet::new();
        for second in (1..=86400).step_by(10) {
            let light = time_of_day::Light::at_hours(second as f32 / 3600.0);
            let now = sample(&light);
            assert!((now - last).abs() < 0.03, "second {second}: {last} to {now}");
            if light.sun.y > 0.0 {
                let blend = layers.sun_blend(light.key_dir);
                pairs.insert((blend.first, blend.second));
            }
            last = now;
        }
        assert!(pairs.len() >= 6, "the clock crosses all adjacent layer pairs");
        assert_eq!(sun_ratio(&layers, &time_of_day::Light::at_hours(0.0)), 0.0);
    }
}
