//! Transient point lights shared by impacts and spell bursts.

use glam::Vec3;
use verse_pbr::pbr::{Lamp, MAX_FLASH_CANDIDATES};

const CAPACITY: usize = MAX_FLASH_CANDIDATES;
const FLASH_END: f32 = 0.15;
const FIREBALL_END: f32 = 0.65;
const LIFETIME: f32 = 4.0;
const SMOLDER: f32 = 0.035;
const ORANGE: [f32; 3] = [1.0, 0.18, 0.025];

#[derive(Clone, Copy, Debug)]
struct FlashLight {
    lamp: Lamp,
    age: f32,
}

impl FlashLight {
    fn lamp(self) -> Lamp {
        let (strength, warmth) = if self.age <= FLASH_END {
            (1.0 - 0.2 * self.age / FLASH_END, 0.0)
        } else if self.age <= FIREBALL_END {
            let t = (self.age - FLASH_END) / (FIREBALL_END - FLASH_END);
            (0.8 + (SMOLDER - 0.8) * t, t)
        } else {
            let t = (self.age - FIREBALL_END) / (LIFETIME - FIREBALL_END);
            (SMOLDER * (1.0 - t).max(0.0), 1.0)
        };
        Lamp {
            color: std::array::from_fn(|i| {
                self.lamp.color[i] * (1.0 - warmth) + ORANGE[i] * warmth
            }),
            intensity: self.lamp.intensity * strength,
            ..self.lamp
        }
    }
}

/// A bounded pool whose lights flash, fade into orange, and smolder for four seconds.
#[derive(Debug, Default)]
pub struct FlashLights {
    lights: Vec<FlashLight>,
}

impl FlashLights {
    /// Starts a light at peak intensity. Invalid or dark lights are ignored.
    ///
    /// At capacity, the weakest current light leaves first, with the oldest
    /// light leaving on a tie. A weaker new light does not replace it.
    pub fn spawn(&mut self, at: Vec3, color: [f32; 3], peak: f32, range: f32) {
        let lamp = Lamp {
            position: at,
            color,
            intensity: peak,
            range,
        };
        if !lamp.lit() || brightness(lamp) <= 0.0 {
            return;
        }
        let light = FlashLight { lamp, age: 0.0 };
        if self.lights.len() < CAPACITY {
            self.lights.push(light);
            return;
        }
        let (weakest, old) = self
            .lights
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                brightness(a.lamp())
                    .total_cmp(&brightness(b.lamp()))
                    .then_with(|| b.age.total_cmp(&a.age))
            })
            .expect("a full flash-light pool has entries");
        if brightness(lamp) >= brightness(old.lamp()) {
            self.lights.remove(weakest);
            self.lights.push(light);
        }
    }

    /// Advances every light and removes expired lights. Invalid time steps are ignored.
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        for light in &mut self.lights {
            light.age += dt;
        }
        self.lights.retain(|light| light.age < LIFETIME);
    }

    /// Removes every light, including its remaining lifetime.
    pub fn clear(&mut self) {
        self.lights.clear();
    }

    /// Returns every live candidate in descending order of brightness at the camera.
    /// The renderer applies visibility and tier limits after this step.
    #[must_use]
    pub fn lamps(&self, eye: Vec3) -> [Lamp; MAX_FLASH_CANDIDATES] {
        Self::select(self.lights.iter().map(|light| light.lamp()), eye)
    }

    /// Selects a bounded camera-ranked candidate set from several light sources.
    ///
    /// Priority is linear-color luminance times intensity over squared camera
    /// distance, bounded at one square meter. Camera distance affects priority
    /// without excluding lights whose range does not reach the camera.
    #[must_use]
    pub fn select(
        lamps: impl IntoIterator<Item = Lamp>,
        eye: Vec3,
    ) -> [Lamp; MAX_FLASH_CANDIDATES] {
        let mut selected = [Lamp::OFF; MAX_FLASH_CANDIDATES];
        if !eye.is_finite() {
            return selected;
        }
        let mut priorities = [0.0_f64; MAX_FLASH_CANDIDATES];
        for lamp in lamps {
            if !lamp.lit() {
                continue;
            }
            let distance_squared = (lamp.position.as_dvec3() - eye.as_dvec3()).length_squared();
            let priority = brightness(lamp) / distance_squared.max(1.0);
            if let Some(index) = priorities.iter().position(|current| priority > *current) {
                for slot in (index + 1..MAX_FLASH_CANDIDATES).rev() {
                    selected[slot] = selected[slot - 1];
                    priorities[slot] = priorities[slot - 1];
                }
                selected[index] = lamp;
                priorities[index] = priority;
            }
        }
        selected
    }

    /// Returns the number of live lights, including dim smoldering lights.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lights.len()
    }

    /// Whether the pool contains no live lights.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lights.is_empty()
    }
}

fn brightness(lamp: Lamp) -> f64 {
    (f64::from(lamp.color[0]) * 0.2126
        + f64::from(lamp.color[1]) * 0.7152
        + f64::from(lamp.color[2]) * 0.0722)
        * f64::from(lamp.intensity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_fades_to_orange_then_smolders_and_expires() {
        let mut lights = FlashLights::default();
        lights.spawn(Vec3::ZERO, [1.0; 3], 1_000.0, 12.0);
        assert_eq!(lights.lamps(Vec3::ZERO)[0].intensity, 1_000.0);
        lights.tick(FLASH_END);
        let flash = lights.lamps(Vec3::ZERO)[0];
        assert_eq!(flash.color, [1.0; 3]);
        assert!((flash.intensity - 800.0).abs() < 0.001);
        lights.tick(FIREBALL_END - FLASH_END);
        let fireball = lights.lamps(Vec3::ZERO)[0];
        assert_eq!(fireball.color, ORANGE);
        assert!((fireball.intensity - 35.0).abs() < 0.001);
        lights.tick(2.0);
        let smolder = lights.lamps(Vec3::ZERO)[0];
        assert!(smolder.intensity > 0.0 && smolder.intensity < fireball.intensity);
        assert_eq!(smolder.range, 12.0);
        lights.tick(LIFETIME - FIREBALL_END - 2.0);
        assert!(lights.is_empty());
        assert_eq!(lights.lamps(Vec3::ZERO), [Lamp::OFF; MAX_FLASH_CANDIDATES]);
    }

    #[test]
    fn invalid_lights_and_time_steps_do_not_enter_or_age_the_pool() {
        let mut lights = FlashLights::default();
        for (at, color, intensity, range) in [
            (Vec3::splat(f32::NAN), [1.0; 3], 10.0, 4.0),
            (Vec3::ZERO, [f32::INFINITY; 3], 10.0, 4.0),
            (Vec3::ZERO, [-1.0; 3], 10.0, 4.0),
            (Vec3::ZERO, [0.0; 3], 10.0, 4.0),
            (Vec3::ZERO, [1.0; 3], f32::NAN, 4.0),
            (Vec3::ZERO, [1.0; 3], -10.0, 4.0),
            (Vec3::ZERO, [1.0; 3], 10.0, 0.0),
            (Vec3::ZERO, [1.0; 3], 10.0, f32::INFINITY),
        ] {
            lights.spawn(at, color, intensity, range);
        }
        assert!(lights.is_empty());
        lights.spawn(Vec3::ZERO, [1.0; 3], 10.0, 4.0);
        for dt in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
            lights.tick(dt);
        }
        assert_eq!(lights.lamps(Vec3::ZERO)[0].intensity, 10.0);
        assert_eq!(
            lights.lamps(Vec3::splat(f32::NAN)),
            [Lamp::OFF; MAX_FLASH_CANDIDATES]
        );
        lights.clear();
        assert!(lights.is_empty());
    }

    #[test]
    fn overflow_keeps_stronger_lights_and_replaces_the_oldest_tie() {
        let mut lights = FlashLights::default();
        for i in 0..CAPACITY {
            lights.spawn(Vec3::new(i as f32, 0.0, 0.0), [1.0; 3], 100.0, 4.0);
        }
        lights.spawn(Vec3::splat(100.0), [1.0; 3], 1.0, 4.0);
        assert_eq!(lights.len(), CAPACITY);
        assert!(
            lights
                .lights
                .iter()
                .all(|light| light.lamp.intensity == 100.0)
        );
        lights.spawn(Vec3::splat(200.0), [1.0; 3], 100.0, 4.0);
        assert_eq!(lights.len(), CAPACITY);
        assert!(
            lights
                .lights
                .iter()
                .all(|light| light.lamp.position != Vec3::ZERO)
        );
        lights.tick(1.0);
        lights.spawn(Vec3::ZERO, [1.0; 3], 100.0, 4.0);
        assert_eq!(lights.len(), CAPACITY);
        assert_eq!(lights.lamps(Vec3::ZERO)[0].position, Vec3::ZERO);
        assert_eq!(lights.lamps(Vec3::ZERO)[0].intensity, 100.0);
    }

    #[test]
    fn selection_ranks_luminance_and_distance_and_preserves_twelve_candidates() {
        let mut lights = FlashLights::default();
        lights.spawn(Vec3::new(10.0, 0.0, 0.0), [1.0; 3], 100.0, 4.0);
        lights.spawn(Vec3::X, [1.0; 3], 10.0, 4.0);
        assert_eq!(lights.lamps(Vec3::ZERO)[0].position, Vec3::X);
        lights.clear();
        for i in 0..12 {
            lights.spawn(Vec3::new(i as f32, 1.0, 0.0), [1.0; 3], 100.0, 4.0);
        }
        let selected = lights.lamps(Vec3::ZERO);
        assert_eq!(selected.iter().filter(|lamp| lamp.lit()).count(), 12);
        for (i, lamp) in selected[..12].iter().enumerate() {
            assert_eq!(lamp.position, Vec3::new(i as f32, 1.0, 0.0));
            assert!(
                selected[..i]
                    .iter()
                    .all(|previous| previous.position != lamp.position)
            );
        }
        assert!(selected[12..].iter().all(|lamp| *lamp == Lamp::OFF));
        let red = Lamp {
            color: [1.0, 0.0, 0.0],
            ..selected[0]
        };
        let green = Lamp {
            color: [0.0, 1.0, 0.0],
            ..red
        };
        let merged = FlashLights::select([red, Lamp::OFF, green], Vec3::ZERO);
        assert_eq!(merged[0], green);
        assert_eq!(merged[1], red);
        assert_eq!(merged[2], Lamp::OFF);
    }

    #[test]
    fn full_pool_reaches_the_renderer_and_merged_sources_remain_bounded() {
        let mut lights = FlashLights::default();
        for i in 0..CAPACITY {
            lights.spawn(Vec3::new(i as f32, 1.0, 0.0), [1.0; 3], 100.0, 4.0);
        }
        let candidates = lights.lamps(Vec3::ZERO);
        assert_eq!(
            candidates.iter().filter(|lamp| lamp.lit()).count(),
            CAPACITY
        );
        for (i, lamp) in candidates.iter().enumerate() {
            assert_eq!(lamp.position, Vec3::new(i as f32, 1.0, 0.0));
        }
        let extra = (CAPACITY..CAPACITY + 4).map(|i| Lamp {
            position: Vec3::new(i as f32, 1.0, 0.0),
            color: [1.0; 3],
            intensity: 100.0,
            range: 4.0,
        });
        assert_eq!(
            FlashLights::select(candidates.into_iter().chain(extra), Vec3::ZERO),
            candidates
        );
    }
}
