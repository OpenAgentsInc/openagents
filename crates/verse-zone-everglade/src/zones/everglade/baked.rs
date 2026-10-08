//! Immutable baked sunlight blended on the GPU as the town clock advances.
//! The sky multiplier follows the frame's sky brightness, and lamps fade
//! through the existing dusk and dawn schedule.

use std::sync::Arc;

use crate::pbr::textured::TexturedScene;
use crate::pbr::textured_bake::{AmbientProbes, LayerChoice};
use crate::zones::everglade_pack::kit_bake;
use verse_pbr::pbr::baked_layers::Layers;

use super::time_of_day;

/// Baked sun, sky brightness, and lamp fades follow the exact clock.
/// The expensive sky's shape and character probes keep their existing cadence.
pub(super) fn clock_light(
    light: time_of_day::Light,
    time: town_clock::TownTime,
    pinned: bool,
) -> time_of_day::Light {
    if pinned {
        light
    } else {
        time_of_day::Light::at_hours((time.second / 3600.0) as f32)
    }
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
    // Night keeps the stage's moonlight and readable sky floor, without
    // reviving the daytime bake's sunlight.
    if light.sun.y <= 0.0 {
        return 0.0;
    }
    layers.sun_ratio(light.key_lux, light.sky_lux)
}

/// The existing dusk/dawn schedule with a smooth fade around lamp ignition.
#[must_use]
pub fn lamp_intensity(light: &time_of_day::Light) -> f32 {
    let t = ((light.night - 0.15) / 0.3).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) struct BakedLight {
    layers: Arc<Layers>,
    pub(super) scene: Arc<TexturedScene>,
    pub(super) active: bool,
    hour: f32,
}

impl BakedLight {
    pub(super) fn new(choice: &LayerChoice, scene: Arc<TexturedScene>) -> Self {
        Self {
            layers: choice.layers.clone(),
            scene,
            active: false,
            hour: f32::NAN,
        }
    }

    /// Four scalars change each frame; static vertex layers stay immutable.
    pub(super) fn sun(&self, light: &time_of_day::Light) -> [f32; 4] {
        if !self.active {
            return [0.0; 4];
        }
        self.layers
            .sun_blend(light.sun)
            .uniform(sun_ratio(&self.layers, light))
    }

    /// Character probes follow the sky's existing cadence. This combines a
    /// small probe grid without recombining the town's vertex layers.
    pub(super) fn update(&mut self, light: &time_of_day::Light) -> Option<AmbientProbes> {
        if !self.active || self.hour == light.hours {
            return None;
        }
        self.hour = light.hours;
        Some(self.layers.blended_probes(
            self.layers.sun_blend(light.sun),
            sun_ratio(&self.layers, light),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_clock_weights_move_between_sky_steps() {
        let before = town_clock::TownTime::at_hour(0, 9.0);
        let after = town_clock::TownTime::at_hour(0, 9.0 + 3.0 / 60.0);
        let stepped = time_of_day::Light::at(before);
        assert_eq!(stepped, time_of_day::Light::at(after));
        let a = clock_light(stepped, before, false);
        let b = clock_light(stepped, after, false);
        assert_ne!(a.sun, b.sun);
        assert!(a.sun.angle_between(b.sun) < 0.02);
        assert_eq!(clock_light(stepped, after, true), stepped);
        let before = town_clock::TownTime::at_hour(0, 17.0 + 28.0 / 60.0);
        let after = town_clock::TownTime::at_hour(0, 17.0 + 31.0 / 60.0);
        let stepped = time_of_day::Light::at(before);
        assert_eq!(stepped, time_of_day::Light::at(after));
        let a = clock_light(stepped, before, false);
        let b = clock_light(stepped, after, false);
        assert_ne!(a.sky_lux, b.sky_lux);
        assert_ne!(lamp_intensity(&a), lamp_intensity(&b));
    }

    #[test]
    fn lamp_fade_is_continuous_through_dusk_and_dawn() {
        assert_eq!(lamp_intensity(&time_of_day::Light::at_hours(12.0)), 0.0);
        assert!(lamp_intensity(&time_of_day::Light::at_hours(23.0)) > 0.99);
        let mut previous = lamp_intensity(&time_of_day::Light::at_hours(0.0));
        for minute in 1..=1440 {
            let intensity = lamp_intensity(&time_of_day::Light::at_hours(minute as f32 / 60.0));
            assert!((intensity - previous).abs() < 0.06, "minute {minute}");
            previous = intensity;
        }
    }

    #[test]
    fn exact_sky_keeps_the_running_stages_tenth_of_noon_floor() {
        let noon = time_of_day::Light::at_hours(12.0);
        let exposed = |key: &time_of_day::Light, sky: &time_of_day::Light| {
            (key.key_lux * key.key_dir.y + sky.sky_lux) * crate::pbr::exposure(key.ev100)
        };
        let noon_level = exposed(&noon, &noon);
        for minute in 0..1440 {
            let time = town_clock::TownTime::at_hour(0, minute as f64 / 60.0);
            let stepped = time_of_day::Light::at(time);
            let exact = clock_light(stepped, time, false);
            let ratio = exposed(&stepped, &exact) / noon_level;
            assert!(ratio >= 0.1, "{ratio} at minute {minute}");
        }
    }

    #[test]
    fn town_clock_blends_every_sun_pair_without_daytime_bounce_at_night() {
        use verse_pbr::pbr::baked_layers::{Reference, SunLayer};
        let mut layers = Layers {
            bake_key: "synthetic".into(),
            scene: "synthetic".into(),
            reference: Reference {
                sun: 4000.0,
                sky: 1200.0,
                ground: 450.0,
            },
            sky: vec![[100, 100, 100, 255]],
            sky_probes: vec![[1.0; 12]],
            suns: Vec::new(),
            lamps: Vec::new(),
            probe_origin: [0.0; 3],
            probe_cell: 1.0,
            probe_dims: [1; 3],
        };
        for (i, hour) in [8.0, 12.0, 15.5, 17.5].into_iter().enumerate() {
            layers.suns.push(SunLayer {
                dir: time_of_day::Light::at_hours(hour).sun.to_array(),
                vertices: vec![[100, 100, 100, 255]],
                probes: vec![[i as f32; 12]],
            });
        }
        let sample = |light: &time_of_day::Light| {
            layers
                .blended_probes(layers.sun_blend(light.sun), sun_ratio(&layers, light))
                .grid
                .data[0][0]
        };
        let mut previous = sample(&time_of_day::Light::at_hours(0.0));
        let mut pairs = std::collections::BTreeSet::new();
        for second in (1..=86400).step_by(10) {
            let light = time_of_day::Light::at_hours(second as f32 / 3600.0);
            let now = sample(&light);
            assert!(
                (now - previous).abs() < 0.03,
                "second {second}: {previous} to {now}"
            );
            if light.sun.y > 0.0 {
                let blend = layers.sun_blend(light.sun);
                pairs.insert((blend.first, blend.second));
            }
            previous = now;
        }
        assert!(pairs.len() >= 5);
        assert_eq!(sun_ratio(&layers, &time_of_day::Light::at_hours(0.0)), 0.0);
    }
}
