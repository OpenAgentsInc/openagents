//! The town's offline-baked light at run time: which baked sun the town
//! shows, and how brightly its baked lamps burn
//! ([`crate::zones::everglade_pack::kit_bake`]).
//!
//! The sky layer stays as baked. The sun's bounce comes from the baked sun
//! direction nearest the key light, scaled by how strong the key is against
//! the sky; when the key moves nearer another baked direction, the town
//! combines the layers again off the main thread and delivers the new light
//! channel and probes. The lamp layer fades in from dusk to full night.
//! Blending between the baked suns as the hours pass is the next phase's
//! work (#10907).

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
        sun: layers.nearest_sun(light.key_dir),
        ratio: layers.sun_ratio(light.key_lux, light.sky_lux),
        layers,
    })
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
    scene: Arc<TexturedScene>,
    sun: Option<usize>,
    /// Whether the bake job delivered the layers, rather than baking.
    pub(super) active: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<std::sync::mpsc::Receiver<(Option<usize>, AmbientProbes)>>,
}

impl BakedLight {
    pub(super) fn new(choice: &LayerChoice, scene: Arc<TexturedScene>) -> Self {
        Self {
            layers: choice.layers.clone(),
            scene,
            sun: choice.sun,
            active: false,
            #[cfg(not(target_arch = "wasm32"))]
            pending: None,
        }
    }

    /// Follows the key light: new probes once a recombination for another
    /// baked sun finishes.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn update(&mut self, light: &time_of_day::Light) -> Option<AmbientProbes> {
        if !self.active {
            return None;
        }
        if let Some(pending) = &self.pending {
            return match pending.try_recv() {
                Ok((sun, probes)) => {
                    self.sun = sun;
                    self.pending = None;
                    Some(probes)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    None
                }
            };
        }
        let sun = self.layers.nearest_sun(light.key_dir);
        if sun == self.sun {
            return None;
        }
        let ratio = self.layers.sun_ratio(light.key_lux, light.sky_lux);
        let (layers, scene) = (self.layers.clone(), self.scene.clone());
        let (send, receive) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("verse-light-layers".into())
            .spawn(move || {
                scene.baked.deliver_lights(layers.lights(sun, ratio));
                let _ = send.send((sun, layers.probes(sun, ratio)));
            });
        if spawned.is_ok() {
            self.pending = Some(receive);
        } else {
            // Without a thread, keep the sun the town shows.
            self.sun = sun;
        }
        None
    }

    /// Browsers load no baked layers.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn update(&mut self, _: &time_of_day::Light) -> Option<AmbientProbes> {
        None
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
}
