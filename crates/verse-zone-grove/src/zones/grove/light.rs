//! The Grove's dusk and the light its fires and spells give.
//!
//! The Grove has its own sky, apart from Everglade's afternoon: a low Sun
//! in the west-north-west, a few degrees over the tree line, rakes long
//! shadows across the field in warm light against a blue-violet sky and
//! cool fill. The sky glows toward the Sun, with violet cloud undersides,
//! fiery edges, and crepuscular rays, and a low haze glows around the Sun
//! along the tree line. The grade is cinematic: more contrast, cool
//! shadows against warm highlights, and a vignette.
//!
//! Its fires burn as flickering point lights ([`crate::pbr::Lamp`]): the
//! campfire, the braziers, the torches, the lanterns in the oak, the altar's
//! candles, and the rune stones ([`super::layout::fires`]). The big spells
//! light the field for as long as they burn: Meteor Swarm's meteors and
//! explosions, the Thunderbolt's and Call Lightning's flashes, which also
//! brighten the sky, Fire Breath along its cone, the bolts in flight, the
//! fiery landings, Wall of Fire, Moonbeam's and Sunbeam's columns, and
//! Shapechange's flash. The stage carries at most [`MAX_LAMPS`]: the spells
//! first, brightest first, then the fires nearest the eye. A quality tier
//! shades only the first of them (`pbr::gpu::lamp_budget`).

use super::Grove;
use super::draw::Effect;
use super::kit::Spell;
use super::layout::{Fire, FireKind};
use super::shape::Form;
use crate::controller::PlayerController;
use crate::pbr::{Daylight, Grade, Key, Lamp, MAX_LAMPS, Neon};
use crate::zones::Atmosphere;
use crate::zones::everglade::Everglade;
use crate::zones::everglade::demolition::meteor::Glow;
use glam::Vec3;
use verse_engine::lighting::HeightFog;

/// Unit direction toward the Sun: west-north-west, about 8 degrees up.
#[must_use]
pub fn sun() -> Vec3 {
    Vec3::new(-0.78, 0.14, 0.61).normalize()
}

/// The dusk haze at the horizon, which is also the fog's color: a dusty
/// mauve the sky warms toward the Sun.
pub const HAZE: [f32; 3] = [0.58, 0.44, 0.42];

/// The Grove's air: dusk haze that lies low and glows strongly toward the
/// low Sun, and closes over the forest past the tree ring.
pub const ATMOSPHERE: Atmosphere = Atmosphere {
    color: HAZE,
    fog_start: 35.0,
    fog_end: 170.0,
    height_fog: Some(HeightFog {
        density: 0.0065,
        base: 0.0,
        falloff: 0.1,
        start: 30.0,
        max_opacity: 0.9,
        sun_strength: 0.9,
        sun_exponent: 8.0,
    }),
};

/// The Sun's warm light and the sky's cool fill, linear.
const SUN_COLOR: [f32; 3] = [1.0, 0.66, 0.38];
const FILL_COLOR: [f32; 3] = [0.52, 0.62, 1.0];

/// The dusk light: a warm low Sun that casts long shadows, a cool rim from
/// the sky opposite, and a dim sky fill.
#[must_use]
pub fn key() -> Key {
    Key {
        dir: sun(),
        illuminance: 2_500.0,
        angular_radius: 0.03,
        rim_dir: Vec3::new(0.55, 0.5, -0.67).normalize(),
        rim_illuminance: 520.0,
        rim_angular_radius: 0.2,
        sky: 400.0,
        ground: 140.0,
        ev100: 10.0,
        shadow_center: Vec3::ZERO,
        shadow_half: 75.0,
        // The low Sun's shadows run long: past the stones and the oak.
        shadow_distance: Some(130.0),
        cache_far_shadows: true,
    }
}

/// The Grove's stage at `time`, s, before its lamps: the dusk sky, its
/// haze, the warm key against the cool fill, and the cinematic grade.
#[must_use]
pub fn stage(time: f32) -> Neon {
    let air = ATMOSPHERE;
    Neon {
        field: air.color,
        fog_start: air.fog_start,
        fog_end: air.fog_end,
        line_gain: 1.0,
        line_width: 1.4,
        bloom: 0.07,
        vignette: 0.42,
        time,
        key: Some(key()),
        daylight: Some(Daylight {
            zenith: [0.05, 0.07, 0.25],
            horizon: air.color,
            sun: [1.0, 0.56, 0.24],
            clouds: 0.46,
            ground: [0.1, 0.1, 0.06],
            glow: 0.9,
        }),
        height_fog: air.height_fog,
        grade: Grade {
            exposure: -0.1,
            saturation: 1.06,
            contrast: 1.16,
            shadows: Vec3::new(0.9, 0.99, 1.12),
            highlights: Vec3::new(1.08, 1.0, 0.88),
            ..Grade::STAGE
        },
        key_color: SUN_COLOR,
        rim_color: FILL_COLOR,
        ..Neon::plaza(time)
    }
}

/// A brief light where a spell landed or burst.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flash {
    pub at: Vec3,
    pub start: f32,
    /// How long it takes to fade, s.
    pub life: f32,
    pub color: [f32; 3],
    /// Candela at its peak.
    pub peak: f32,
    pub range: f32,
    /// Whether it crackles like lightning rather than fading smoothly.
    pub crackle: bool,
}

impl Flash {
    /// Its intensity `age` s after it began: a fast rise and a fade.
    fn intensity(&self, age: f32) -> f32 {
        if !(0.0..self.life).contains(&age) {
            return 0.0;
        }
        let rise = (age / 0.03).min(1.0);
        let fade = 1.0 - age / self.life;
        let mut k = rise * fade * fade;
        if self.crackle {
            // Restrikes: the stroke's light stutters a few times.
            k *= 0.45 + 0.55 * (age * 62.0).sin().abs();
        }
        self.peak * k
    }
}

/// The most flashes kept at once, oldest dropped first.
pub const MAX_FLASHES: usize = 16;

/// The light a particle effect's burst gives, by the effect's name: its
/// color, peak candela, range, how long it fades, and whether it
/// crackles. `None` for effects that give no light.
fn burst_light(name: &str) -> Option<([f32; 3], f32, f32, f32, bool)> {
    const FIRE: [f32; 3] = [1.0, 0.5, 0.16];
    const BOLT: [f32; 3] = [0.7, 0.8, 1.0];
    Some(match name {
        "grove_flame_hit" => (FIRE, 150_000.0, 13.0, 0.5, false),
        "grove_fire_cone" => (FIRE, 125_000.0, 12.0, 0.6, false),
        "grove_sunburst" => ([1.0, 0.9, 0.62], 550_000.0, 22.0, 0.9, false),
        "grove_sunbeam" => ([1.0, 0.86, 0.55], 300_000.0, 16.0, 0.8, false),
        "grove_lightning" | "grove_lightning_line" => (BOLT, 400_000.0, 22.0, 0.35, true),
        "grove_star_hit" => ([0.62, 0.72, 1.0], 45_000.0, 7.0, 0.4, false),
        "grove_heal" => ([0.55, 1.0, 0.6], 35_000.0, 6.0, 0.8, false),
        "grove_ice_shatter" | "grove_cold_cone" => ([0.6, 0.85, 1.0], 50_000.0, 7.0, 0.4, false),
        "grove_necrotic" | "grove_poison_puff" | "grove_acid" => {
            ([0.55, 0.95, 0.3], 22_500.0, 5.0, 0.5, false)
        }
        "grove_cast_burst" => ([0.6, 1.0, 0.55], 22_500.0, 5.0, 0.4, false),
        _ => return None,
    })
}

/// The steady light of a fire among the placements: color, candela, and
/// range.
fn fire_light(kind: FireKind) -> ([f32; 3], f32, f32) {
    match kind {
        FireKind::Campfire => ([1.0, 0.52, 0.2], 22_000.0, 14.0),
        FireKind::Brazier => ([1.0, 0.55, 0.22], 13_000.0, 11.0),
        FireKind::Torch => ([1.0, 0.6, 0.26], 7_000.0, 9.0),
        FireKind::Lantern => ([1.0, 0.72, 0.4], 4_000.0, 7.0),
        FireKind::Candles => ([1.0, 0.7, 0.38], 3_000.0, 5.0),
        FireKind::Runes => ([0.35, 1.0, 0.62], 1_800.0, 4.5),
    }
}

/// A lamp of `color` at `at`.
fn lamp(at: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Lamp {
    Lamp {
        position: at,
        color,
        intensity,
        range,
    }
}

/// The light the Grove gives this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lights {
    /// The lamps, most important first, at most [`MAX_LAMPS`].
    pub lamps: Vec<Lamp>,
    /// How brightly lightning lights the sky, 0 to 1.
    pub sky_flash: f32,
}

impl Grove {
    /// Records the light of the effect `name` bursting at `at`.
    pub(super) fn flash(&mut self, name: &str, at: Vec3) {
        if let Some((color, peak, range, life, crackle)) = burst_light(name) {
            self.add_flash(Flash {
                at: at + Vec3::Y * 0.6,
                start: self.time,
                life,
                color,
                peak,
                range,
                crackle,
            });
        }
    }

    pub(super) fn add_flash(&mut self, flash: Flash) {
        if self.flashes.len() >= MAX_FLASHES {
            self.flashes.remove(0);
        }
        self.flashes.push(flash);
    }

    /// The light of a strike landing on the field or the tower: Meteor
    /// Swarm's explosions and the Thunderbolt's blast.
    pub(super) fn impact_flash(&mut self, at: Vec3, normal: Vec3, lightning: bool) {
        let at = at + normal * 1.2;
        let flash = if lightning {
            Flash {
                at,
                start: self.time,
                life: 0.45,
                color: [0.6, 0.72, 1.0],
                peak: 900_000.0,
                range: 34.0,
                crackle: true,
            }
        } else {
            Flash {
                at,
                start: self.time,
                life: 0.9,
                color: [1.0, 0.5, 0.17],
                peak: 420_000.0,
                range: 26.0,
                crackle: false,
            }
        };
        self.add_flash(flash);
    }

    /// Drops the flashes that have faded.
    pub(super) fn tick_flashes(&mut self) {
        let now = self.time;
        self.flashes.retain(|f| now - f.start < f.life);
    }

    /// The lamps and sky flash for this frame, seen from `eye`.
    #[must_use]
    pub fn lights(&self, glade: &Everglade, player: &PlayerController, eye: Vec3) -> Lights {
        let now = self.time;
        let mut spells: Vec<Lamp> = Vec::new();
        let mut sky_flash: f32 = 0.0;
        for flash in &self.flashes {
            let intensity = flash.intensity(now - flash.start);
            if intensity > 0.0 {
                spells.push(lamp(flash.at, flash.color, intensity, flash.range));
                if flash.crackle {
                    sky_flash = sky_flash.max(intensity / 900_000.0);
                }
            }
        }
        // Meteor Swarm and the Thunderbolt.
        if let Some(swarm) = glade.town().map(|town| town.swarm()) {
            for glow in swarm.glows() {
                match glow {
                    Glow::Gathering {
                        at,
                        strike,
                        progress,
                    } => {
                        let color = match strike {
                            crate::zones::everglade::demolition::meteor::Strike::Meteors => {
                                [1.0, 0.45, 0.14]
                            }
                            crate::zones::everglade::demolition::meteor::Strike::Lightning
                            | crate::zones::everglade::demolition::meteor::Strike::MegaLightning => {
                                [0.6, 0.72, 1.0]
                            }
                        };
                        spells.push(lamp(at, color, 6_000.0 + 30_000.0 * progress, 9.0));
                    }
                    Glow::Meteor { at } => {
                        let flicker = 0.85 + 0.15 * (now * 41.0 + at.x).sin();
                        spells.push(lamp(at, [1.0, 0.56, 0.2], 260_000.0 * flicker, 30.0));
                    }
                    Glow::Bolt { at, sky, age } => {
                        let life = crate::zones::everglade::demolition::meteor::Swarm::bolt_life();
                        let fade = (1.0 - age / life).clamp(0.0, 1.0);
                        let stutter = 0.6 + 0.4 * (age * 55.0).sin().abs();
                        let k = fade * stutter;
                        // The stroke lights the whole field from above, and
                        // its foot glares on what it strikes.
                        spells.push(lamp(
                            at.lerp(sky, 0.35),
                            [0.55, 0.68, 1.0],
                            1.6e7 * k,
                            110.0,
                        ));
                        spells.push(lamp(at, [0.6, 0.72, 1.0], 700_000.0 * k, 26.0));
                        sky_flash = sky_flash.max(k);
                    }
                }
            }
        }
        for effect in &self.effects {
            match *effect {
                Effect::Bolt {
                    from,
                    target,
                    start,
                    flight,
                    spell,
                    ..
                } => {
                    let Some(d) = self.dummies.get(target) else {
                        continue;
                    };
                    let k = ((now - start) / flight.max(1e-3)).clamp(0.0, 1.0);
                    let at = from.lerp(d.center(), k);
                    let (color, intensity) = match spell {
                        Spell::Fireball => ([1.0, 0.5, 0.15], 150_000.0),
                        Spell::FireBolt | Spell::ProduceFlame => ([1.0, 0.55, 0.2], 65_000.0),
                        Spell::RayOfFrost | Spell::IceKnife => ([0.6, 0.85, 1.0], 14_000.0),
                        Spell::StarryWisp => ([0.65, 0.75, 1.0], 12_000.0),
                        Spell::RayOfSickness => ([0.55, 0.95, 0.3], 9_000.0),
                        _ => ([1.0, 0.7, 0.4], 10_000.0),
                    };
                    spells.push(lamp(at, color, intensity, 11.0));
                }
                Effect::Strike { from, to, start } => {
                    let age = now - start;
                    let k =
                        (1.0 - age / 0.35).clamp(0.0, 1.0) * (0.4 + 0.6 * (age * 60.0).sin().abs());
                    spells.push(lamp(from.lerp(to, 0.6), [0.7, 0.8, 1.0], 2.5e6 * k, 60.0));
                    sky_flash = sky_flash.max(0.7 * k);
                }
                Effect::Shift { at, start } | Effect::Rest { at, start } => {
                    let k = (1.0 - (now - start) / 1.2).clamp(0.0, 1.0);
                    spells.push(lamp(at + Vec3::Y, [0.5, 1.0, 0.55], 12_000.0 * k, 6.0));
                }
                _ => {}
            }
        }
        for aura in &self.auras {
            match aura.spell {
                Spell::WallOfFire => {
                    let across = if aura.wall { aura.across } else { Vec3::X };
                    for (i, t) in [-0.66_f32, 0.0, 0.66].into_iter().enumerate() {
                        let at = aura.center + across * aura.reach * t + Vec3::Y * 1.4;
                        let seed = i as u32 + 7;
                        spells.push(
                            lamp(at, [1.0, 0.48, 0.15], 90_000.0, 14.0).flickering(now * 1.6, seed),
                        );
                    }
                }
                Spell::Moonbeam => {
                    for y in [0.6, 3.0] {
                        spells.push(lamp(
                            aura.center + Vec3::Y * y,
                            [0.62, 0.76, 1.0],
                            60_000.0,
                            10.0,
                        ));
                    }
                }
                Spell::FireStorm => {
                    spells.push(
                        lamp(
                            aura.center + Vec3::Y * 2.0,
                            [1.0, 0.45, 0.14],
                            80_000.0,
                            16.0,
                        )
                        .flickering(now * 1.6, 11),
                    );
                }
                Spell::CallLightning | Spell::StormOfVengeance => {
                    // The storm cloud's faint inner glow.
                    let k = 0.5 + 0.5 * (now * 3.1).sin();
                    spells.push(lamp(
                        aura.center + Vec3::Y * 12.0,
                        [0.5, 0.6, 1.0],
                        40_000.0 * k,
                        24.0,
                    ));
                }
                _ => {}
            }
        }
        // Fire Breath: a warm light rolling along the cone.
        if let Some(breath) = self.breath {
            let age = now - breath.start;
            if (super::dragon::BREATH_START..super::dragon::BREATH_END).contains(&age)
                && self.form() == Some(Form::Dragon)
            {
                let swell = ((age - super::dragon::BREATH_START) / 0.25).min(1.0);
                let mouth = self.mouth(player);
                let along = (player.forward() - Vec3::Y * 0.22).normalize();
                for (i, d) in [1.5_f32, 4.5, 8.0].into_iter().enumerate() {
                    let at = mouth + along * d * swell.max(0.2);
                    let peak = [90_000.0, 140_000.0, 110_000.0][i];
                    spells.push(
                        lamp(at, [1.0, 0.5, 0.16], peak * swell, 12.0)
                            .flickering(now * 1.8, i as u32 + 3),
                    );
                }
            }
        }
        // Shapechange's flash at the swap, and the green light gathering
        // before it.
        if let Some(morph) = &self.morph {
            let age = now - morph.start;
            let to_swap = age - morph.swap;
            let k = if to_swap < 0.0 {
                (age / morph.swap.max(1e-3)).clamp(0.0, 1.0) * 0.25
            } else {
                (1.0 - to_swap / 0.6).clamp(0.0, 1.0)
            };
            if k > 0.0 {
                spells.push(lamp(
                    self.feet + Vec3::Y * 1.8,
                    [0.62, 1.0, 0.6],
                    300_000.0 * k,
                    20.0,
                ));
            }
        }
        // The burning dummies.
        for (n, &(i, _)) in self.burns.iter().enumerate() {
            if let Some(d) = self.dummies.get(i) {
                spells.push(
                    lamp(d.center(), [1.0, 0.5, 0.17], 14_000.0, 7.0)
                        .flickering(now, n as u32 + 21),
                );
            }
        }
        spells.retain(Lamp::lit);
        spells.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
        spells.truncate(MAX_LAMPS);
        // The fires, nearest the eye first.
        let mut fires: Vec<(f32, Lamp)> = self
            .fires
            .iter()
            .enumerate()
            .map(|(i, fire)| {
                let (color, intensity, range) = fire_light(fire.kind);
                let lamp = lamp(fire.at, color, intensity, range);
                let lamp = if fire.kind == FireKind::Runes {
                    // A slow pulse rather than a flicker.
                    Lamp {
                        intensity: intensity * (0.75 + 0.25 * (now * 1.3 + i as f32).sin()),
                        ..lamp
                    }
                } else {
                    lamp.flickering(now, i as u32)
                };
                (fire.at.distance_squared(eye), lamp)
            })
            .collect();
        fires.sort_by(|a, b| a.0.total_cmp(&b.0));
        let room = MAX_LAMPS - spells.len();
        let mut lamps = spells;
        lamps.extend(fires.into_iter().take(room).map(|(_, l)| l));
        Lights {
            lamps,
            sky_flash: sky_flash.clamp(0.0, 1.0),
        }
    }

    /// The Grove's stage at `time` with this frame's lights.
    #[must_use]
    pub fn lit_stage(&self, lights: &Lights, time: f32) -> Neon {
        let mut neon = stage(time);
        for (slot, lamp) in neon.lamps.iter_mut().zip(&lights.lamps) {
            *slot = *lamp;
        }
        neon.sky_flash = lights.sky_flash;
        neon
    }
}

/// The light-giving placements, as the Grove keeps them.
#[must_use]
pub(crate) fn fires() -> Vec<Fire> {
    super::layout::fires()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dusk_is_valid_and_distinct_from_everglade() {
        assert!(ATMOSPHERE.validate().is_ok());
        let neon = stage(0.0);
        let day = neon.daylight.unwrap();
        assert!(day.valid());
        assert!(day.glow > 0.5);
        assert!(neon.grade.validate().is_ok());
        // A low Sun: under 15 degrees over the horizon.
        assert!(sun().y > 0.0 && sun().y.asin().to_degrees() < 15.0);
        // Warm light against cool fill.
        assert!(neon.key_color[0] > neon.key_color[2]);
        assert!(neon.rim_color[2] > neon.rim_color[0]);
        assert_ne!(ATMOSPHERE, crate::zones::everglade::ATMOSPHERE);
    }

    #[test]
    fn flashes_rise_fast_and_fade_out() {
        let flash = Flash {
            at: Vec3::ZERO,
            start: 0.0,
            life: 0.5,
            color: [1.0; 3],
            peak: 1000.0,
            range: 5.0,
            crackle: false,
        };
        assert_eq!(flash.intensity(-0.1), 0.0);
        assert!(flash.intensity(0.03) > 800.0);
        assert!(flash.intensity(0.3) < flash.intensity(0.1));
        assert_eq!(flash.intensity(0.5), 0.0);
        for name in ["grove_flame_hit", "grove_sunburst", "grove_lightning"] {
            assert!(burst_light(name).is_some(), "{name}");
        }
        assert!(burst_light("grove_vines").is_none());
    }

    #[test]
    fn every_fire_gives_a_lamp_in_range() {
        let fires = fires();
        assert!(fires.iter().any(|f| f.kind == FireKind::Campfire));
        assert!(fires.iter().filter(|f| f.kind == FireKind::Runes).count() >= 3);
        for fire in fires {
            let (color, intensity, range) = fire_light(fire.kind);
            assert!(lamp(fire.at, color, intensity, range).lit());
        }
    }
}
