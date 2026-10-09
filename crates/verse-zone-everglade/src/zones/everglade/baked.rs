//! The town's offline-baked light at run time: which baked sun the town
//! shows, and how brightly its baked lamps burn
//! ([`crate::zones::everglade_pack::kit_bake`]).
//!
//! The sky layer stays as baked. The sun's bounce blends the two baked sun
//! directions either side of the key light, weighted by how near each is
//! ([`Layers::sun_weights`]), and scales with how strong the key is against
//! the sky. As the hours pass the town combines the layers again off the
//! main thread whenever the blend or the strength moves a step
//! ([`STEP`]), so the light changes in steps too small to see rather than
//! jumping from one baked sun to the next (#10907). The lamp layer fades in
//! from dusk to full night. What destruction opens is relit over the
//! combined light ([`crate::pbr::relight`]).

use std::sync::Arc;

use crate::pbr::textured::TexturedScene;
use crate::pbr::textured_bake::{AmbientProbes, LayerChoice};
use crate::zones::everglade_pack::kit_bake;
use verse_pbr::pbr::baked_layers::Layers;

use super::time_of_day;

/// The layers offered for the town, combined for `light`.
pub(super) fn choice(light: &time_of_day::Light) -> Option<LayerChoice> {
    let layers = kit_bake::offered()?;
    Some(LayerChoice {
        compatibility: Some(kit_bake::compatibility()),
        sun: layers.nearest_sun(light.key_dir),
        ratio: layers.sun_ratio(light.key_lux, light.sky_lux),
        layers,
    })
}

/// The finest change of a sun's weight, or of the sun's strength against
/// the sky, that combines the layers again: a sixty-fourth.
pub const STEP: f32 = 1.0 / 64.0;

/// What the layers are combined for: the suns, their weights, and the
/// sun's strength, each held to [`STEP`]s.
#[derive(Clone, Debug, PartialEq)]
struct Mix {
    suns: Vec<(usize, f32)>,
    ratio: f32,
}

impl Mix {
    fn of(layers: &Layers, light: &time_of_day::Light) -> Self {
        let held = |v: f32| (v / STEP).round() * STEP;
        let mut suns: Vec<(usize, f32)> = layers
            .sun_weights(light.key_dir)
            .into_iter()
            .map(|(k, w)| (k, held(w)))
            .filter(|&(_, w)| w > 0.0)
            .collect();
        suns.sort_by_key(|&(k, _)| k);
        Self {
            suns,
            ratio: held(layers.sun_ratio(light.key_lux, light.sky_lux)),
        }
    }
}

/// The layers combined again: the light channel, one a merged vertex, and
/// the probes.
pub(super) struct Combined {
    pub(super) lights: Vec<[u8; 4]>,
    pub(super) probes: AmbientProbes,
}

/// How brightly the baked lamps burn under `light`: off by day, rising
/// through dusk while the lamps light ([`time_of_day::Light::lamps_lit`]),
/// and full by night.
#[must_use]
pub fn lamp_intensity(light: &time_of_day::Light) -> f32 {
    let t = ((light.night - 0.15) / 0.3).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A town lit by offline-baked layers.
pub(super) struct BakedLight {
    layers: Arc<Layers>,
    /// The scene the layers light.
    pub(super) scene: Arc<TexturedScene>,
    /// What the light the town shows was combined for, once it was.
    mix: Option<Mix>,
    /// Whether the bake job delivered the layers, rather than baking.
    pub(super) active: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<(Mix, std::sync::mpsc::Receiver<Combined>)>,
}

impl BakedLight {
    pub(super) fn new(choice: &LayerChoice, scene: Arc<TexturedScene>) -> Self {
        Self {
            layers: choice.layers.clone(),
            scene,
            mix: None,
            active: false,
            #[cfg(not(target_arch = "wasm32"))]
            pending: None,
        }
    }

    /// Follows the hour: the layers combined again once a combination for
    /// a new blend or strength finishes. The caller delivers the light.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn update(&mut self, light: &time_of_day::Light) -> Option<Combined> {
        if !self.active {
            return None;
        }
        if let Some((mix, pending)) = &self.pending {
            return match pending.try_recv() {
                Ok(combined) => {
                    self.mix = Some(mix.clone());
                    self.pending = None;
                    Some(combined)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    None
                }
            };
        }
        let mix = Mix::of(&self.layers, light);
        if self.mix.as_ref() == Some(&mix) {
            return None;
        }
        let layers = self.layers.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let job = mix.clone();
        let spawned = std::thread::Builder::new()
            .name("verse-light-layers".into())
            .spawn(move || {
                let _ = send.send(Combined {
                    lights: layers.lights_blend(&job.suns, job.ratio),
                    probes: layers.probes_blend(&job.suns, job.ratio),
                });
            });
        if spawned.is_ok() {
            self.pending = Some((mix, receive));
        } else {
            // Without a thread, keep the light the town shows.
            self.mix = Some(mix);
        }
        None
    }

    /// Browsers load no baked layers.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn update(&mut self, _: &time_of_day::Light) -> Option<Combined> {
        None
    }

    /// Whether the light the town shows is combined for `light`'s hour, or
    /// the town doesn't use its layers.
    pub(super) fn settled(&self, light: &time_of_day::Light) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending.is_some() {
            return false;
        }
        !self.active || self.mix.as_ref() == Some(&Mix::of(&self.layers, light))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blend_moves_in_small_steps_through_the_day() {
        // The pinned bake's four suns, at 8:00, 12:00, 15:30, and 17:30.
        let layers = Layers {
            bake_key: String::new(),
            scene: String::new(),
            reference: verse_pbr::pbr::baked_layers::Reference {
                sun: 100_000.0,
                sky: 20_000.0,
                ground: 5_000.0,
            },
            sky: Vec::new(),
            sky_probes: Vec::new(),
            suns: [8.0, 12.0, 15.5, 17.5]
                .map(|h| verse_pbr::pbr::baked_layers::SunLayer {
                    dir: time_of_day::Light::at_hours(h).key_dir.to_array(),
                    vertices: Vec::new(),
                    probes: Vec::new(),
                })
                .to_vec(),
            lamps: Vec::new(),
            probe_origin: [0.0; 3],
            probe_cell: 1.0,
            probe_dims: [0; 3],
        };
        let mut last: Option<Mix> = None;
        let mut changes = 0;
        for minute in (6 * 60)..(21 * 60) {
            let light = time_of_day::Light::at_hours(minute as f32 / 60.0);
            let mix = Mix::of(&layers, &light);
            assert!(mix.suns.len() <= 2);
            if let Some(last) = &last
                && *last != mix
            {
                changes += 1;
                // Each weight and the strength move at most a few steps a
                // minute.
                assert!(
                    (mix.ratio - last.ratio).abs() <= 4.0 * STEP,
                    "{last:?} {mix:?}"
                );
                let weight =
                    |m: &Mix, k: usize| m.suns.iter().find(|s| s.0 == k).map_or(0.0, |s| s.1);
                for k in 0..4 {
                    assert!(
                        (weight(&mix, k) - weight(last, k)).abs() <= 4.0 * STEP,
                        "{minute}: {last:?} {mix:?}"
                    );
                }
            }
            last = Some(mix);
        }
        assert!(changes > 20, "{changes} changes through the day");
    }

    #[test]
    fn baked_lamps_burn_at_night_and_rest_by_day() {
        assert_eq!(lamp_intensity(&time_of_day::Light::at_hours(12.0)), 0.0);
        assert!(lamp_intensity(&time_of_day::Light::at_hours(23.0)) > 0.99);
        let dusk = time_of_day::Light::at_hours(19.0);
        assert!(dusk.lamps_lit() && lamp_intensity(&dusk) > 0.0);
    }
}
