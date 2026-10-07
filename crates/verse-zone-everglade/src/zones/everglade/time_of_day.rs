//! Everglade's time of day: the town clock (`town_clock`) turned into the
//! stage's sky, haze, and light (`docs/verse/generative-agents.md`, phase
//! C1).
//!
//! The Sun rises at 06:00 in the east (+x is west here, where it sets),
//! stands 45 degrees up at noon a little east of south (-z), and sets at
//! 18:00. At 10:30 it stands where Everglade's fixed late-morning Sun stood.
//! After sunset the key light becomes the Moon: the Sun's direction
//! mirrored above the horizon, so the key never jumps, in a cool, dim light
//! that the stage's exposure opens up to, so the night stays playable and
//! the lamps glow against it.
//!
//! Every value but the Sun's direction comes from keyframes through the
//! day, interpolated linearly, so the light changes without a pop. The
//! state changes in steps of [`STEP_SECONDS`] of town time: each change
//! rebuilds the sky's light and redraws the cached shadows, so a step every
//! 10 real seconds, at a quarter of a degree of the Sun's travel a minute,
//! is too small to see.
//!
//! The ambient light bake keeps the late-morning key: its result is a
//! multiplier of the frame's sky light, which this module dims.

use glam::Vec3;
use town_clock::TownTime;

use super::ATMOSPHERE;
use crate::pbr::{Daylight, Key};

/// Town seconds between changes of the light: four town minutes.
pub const STEP_SECONDS: f64 = 240.0;

/// How far the Sun's daily circle tilts from the zenith, radians: its noon
/// elevation is the complement, 45 degrees.
const TILT: f32 = std::f32::consts::FRAC_PI_4;
/// The horizontal direction of the noon Sun, and of the setting Sun.
const NOON: Vec3 = Vec3::new(-0.1127, 0.0, -0.9936);
const SUNSET: Vec3 = Vec3::new(0.9936, 0.0, -0.1127);
/// Cloud cover by day and at night. The sky draws a cloud's lit side near
/// white under any light, so the night sky clears instead of showing bright
/// clouds against the dark.
const DAY_CLOUDS: f32 = 0.38;
const NIGHT_CLOUDS: f32 = 0.04;
/// The lowest the key light stands, radians: a moonrise or sunset key at
/// the horizon would shade the ground at a grazing angle.
const LOWEST_KEY: f32 = 0.03;

/// One keyframe of the day's light.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    hour: f32,
    zenith: [f32; 3],
    horizon: [f32; 3],
    /// The Sun's tint, or the Moon's: its disc, glow, and sunlit clouds.
    tint: [f32; 3],
    glow: f32,
    /// Key, rim, sky, and ground lux.
    lux: [f32; 4],
    ev100: f32,
    key_color: [f32; 3],
    rim_color: [f32; 3],
    /// How strongly the haze glows toward the key.
    haze_glow: f32,
    /// 0 in daylight to 1 at night.
    night: f32,
}

const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const MOON: [f32; 3] = [0.62, 0.74, 1.0];
const MOON_RIM: [f32; 3] = [0.5, 0.6, 1.0];
const COOL_RIM: [f32; 3] = [0.55, 0.65, 1.0];

const NIGHT: Frame = Frame {
    hour: 0.0,
    zenith: [0.008, 0.014, 0.04],
    horizon: [0.035, 0.045, 0.08],
    tint: [0.55, 0.6, 0.7],
    glow: 0.0,
    lux: [40.0, 10.0, 24.0, 8.0],
    ev100: 6.3,
    key_color: MOON,
    rim_color: MOON_RIM,
    haze_glow: 0.15,
    night: 1.0,
};

/// The late-morning light Everglade had before it had a clock.
const DAY: Frame = Frame {
    hour: 10.5,
    zenith: [0.10, 0.30, 0.73],
    horizon: ATMOSPHERE.color,
    tint: [1.0, 0.8, 0.54],
    glow: 0.0,
    lux: [4_000.0, 900.0, 1_200.0, 450.0],
    ev100: 10.0,
    key_color: WHITE,
    rim_color: WHITE,
    haze_glow: 0.4,
    night: 0.0,
};

/// The day's keyframes, in order of hour from midnight.
const FRAMES: [Frame; 13] = [
    NIGHT,
    Frame {
        hour: 4.5,
        zenith: [0.02, 0.03, 0.08],
        horizon: [0.08, 0.08, 0.12],
        tint: [0.4, 0.45, 0.55],
        lux: [40.0, 10.0, 24.0, 8.0],
        ev100: 6.3,
        ..NIGHT
    },
    // First light: the Sun just under the horizon, the Moon gone.
    Frame {
        hour: 5.5,
        zenith: [0.06, 0.08, 0.2],
        horizon: [0.32, 0.24, 0.27],
        tint: [0.35, 0.2, 0.15],
        glow: 0.5,
        lux: [0.0, 30.0, 80.0, 25.0],
        ev100: 7.3,
        key_color: [1.0, 0.6, 0.4],
        rim_color: COOL_RIM,
        haze_glow: 0.6,
        night: 0.8,
    },
    // Sunrise.
    Frame {
        hour: 6.0,
        zenith: [0.15, 0.2, 0.4],
        horizon: [0.6, 0.42, 0.38],
        tint: [1.0, 0.55, 0.3],
        glow: 0.9,
        lux: [0.0, 120.0, 200.0, 70.0],
        ev100: 8.4,
        key_color: [1.0, 0.6, 0.4],
        rim_color: COOL_RIM,
        haze_glow: 0.9,
        night: 0.5,
    },
    Frame {
        hour: 6.5,
        zenith: [0.13, 0.24, 0.52],
        horizon: [0.66, 0.52, 0.44],
        tint: [1.0, 0.64, 0.38],
        glow: 0.6,
        lux: [600.0, 300.0, 380.0, 140.0],
        ev100: 9.0,
        key_color: [1.0, 0.7, 0.5],
        rim_color: [0.65, 0.74, 1.0],
        haze_glow: 0.75,
        night: 0.2,
    },
    Frame {
        hour: 7.0,
        zenith: [0.12, 0.28, 0.62],
        horizon: [0.70, 0.60, 0.50],
        tint: [1.0, 0.72, 0.45],
        glow: 0.35,
        lux: [1_800.0, 500.0, 700.0, 260.0],
        ev100: 9.5,
        key_color: [1.0, 0.8, 0.62],
        rim_color: [0.75, 0.82, 1.0],
        haze_glow: 0.6,
        night: 0.0,
    },
    DAY,
    Frame { hour: 13.5, ..DAY },
    Frame {
        hour: 16.0,
        horizon: [0.74, 0.64, 0.48],
        tint: [1.0, 0.78, 0.5],
        glow: 0.05,
        lux: [3_500.0, 800.0, 1_100.0, 420.0],
        key_color: [1.0, 0.92, 0.8],
        ..DAY
    },
    Frame {
        hour: 17.0,
        zenith: [0.08, 0.2, 0.5],
        horizon: [0.66, 0.52, 0.44],
        tint: [1.0, 0.66, 0.38],
        glow: 0.4,
        lux: [2_400.0, 520.0, 600.0, 220.0],
        ev100: 9.6,
        key_color: [1.0, 0.75, 0.5],
        rim_color: COOL_RIM,
        haze_glow: 0.8,
        night: 0.0,
    },
    // Sunset.
    Frame {
        hour: 18.0,
        zenith: [0.06, 0.08, 0.26],
        horizon: [0.58, 0.40, 0.38],
        tint: [1.0, 0.5, 0.25],
        glow: 1.0,
        lux: [0.0, 180.0, 240.0, 90.0],
        ev100: 8.5,
        key_color: [1.0, 0.6, 0.4],
        rim_color: COOL_RIM,
        haze_glow: 0.9,
        night: 0.5,
    },
    // Last light, and the Moon rising.
    Frame {
        hour: 18.75,
        zenith: [0.03, 0.04, 0.12],
        horizon: [0.25, 0.18, 0.22],
        tint: [0.4, 0.22, 0.2],
        glow: 0.5,
        lux: [8.0, 30.0, 80.0, 25.0],
        ev100: 7.3,
        key_color: MOON,
        rim_color: MOON_RIM,
        haze_glow: 0.4,
        night: 0.85,
    },
    Frame {
        hour: 20.0,
        ..NIGHT
    },
];

/// The stage's light at one moment of the town day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// The hour this light is for, after the step.
    pub hours: f32,
    /// Unit direction toward the Sun, below the horizon at night.
    pub sun: Vec3,
    /// Unit direction toward the key light: the Sun by day, the Moon by
    /// night.
    pub key_dir: Vec3,
    pub key_lux: f32,
    pub rim_lux: f32,
    pub sky_lux: f32,
    pub ground_lux: f32,
    pub ev100: f32,
    pub zenith: [f32; 3],
    pub horizon: [f32; 3],
    pub tint: [f32; 3],
    pub glow: f32,
    pub clouds: f32,
    pub key_color: [f32; 3],
    pub rim_color: [f32; 3],
    pub haze_glow: f32,
    /// 0 in daylight to 1 at night: how far the lamps stand out.
    pub night: f32,
}

impl Light {
    /// The light at `time`, held to the step it falls in.
    #[must_use]
    pub fn at(time: TownTime) -> Self {
        let stepped = (time.second / STEP_SECONDS).floor() * STEP_SECONDS;
        Self::at_hours((stepped / 3_600.0) as f32)
    }

    /// The light at exactly `hours` into the day.
    #[must_use]
    pub fn at_hours(hours: f32) -> Self {
        let h = if hours.is_finite() {
            hours.rem_euclid(24.0)
        } else {
            DAY.hour
        };
        let f = blend(h);
        let sun = sun(h);
        let mut key_dir = Vec3::new(sun.x, sun.y.abs(), sun.z);
        if key_dir.y < LOWEST_KEY.sin() {
            let flat = Vec3::new(key_dir.x, 0.0, key_dir.z).normalize_or(SUNSET);
            key_dir = flat * LOWEST_KEY.cos() + Vec3::Y * LOWEST_KEY.sin();
        }
        Self {
            hours: h,
            sun,
            key_dir,
            key_lux: f.lux[0],
            rim_lux: f.lux[1],
            sky_lux: f.lux[2],
            ground_lux: f.lux[3],
            ev100: f.ev100,
            zenith: f.zenith,
            horizon: f.horizon,
            tint: f.tint,
            glow: f.glow,
            clouds: DAY_CLOUDS + (NIGHT_CLOUDS - DAY_CLOUDS) * f.night,
            key_color: f.key_color,
            rim_color: f.rim_color,
            haze_glow: f.haze_glow,
            night: f.night,
        }
    }

    /// Whether the town's lamps are lit: from dusk to dawn. The world tree
    /// reads this for its lamps' state.
    #[must_use]
    pub fn lamps_lit(&self) -> bool {
        self.night >= 0.25
    }

    /// The key light: the late-morning key's shape, rim, and shadows, under
    /// this light's direction and levels.
    #[must_use]
    pub fn key(&self, base: Key) -> Key {
        Key {
            dir: self.key_dir,
            illuminance: self.key_lux,
            rim_illuminance: self.rim_lux,
            sky: self.sky_lux,
            ground: self.ground_lux,
            ev100: self.ev100,
            ..base
        }
    }

    /// The daylight sky: this light's colors and cloud cover over the base
    /// sky's ground.
    #[must_use]
    pub fn daylight(&self, base: Daylight) -> Daylight {
        Daylight {
            zenith: self.zenith,
            horizon: self.horizon,
            sun: self.tint,
            glow: self.glow,
            clouds: self.clouds,
            ..base
        }
    }
}

/// Unit direction toward the Sun at `hours`: a circle tilted [`TILT`] from
/// the zenith toward [`NOON`], crossing the horizon at 06:00 and 18:00.
fn sun(hours: f32) -> Vec3 {
    let angle = (hours - 12.0) / 24.0 * std::f32::consts::TAU;
    let high = Vec3::Y * TILT.cos() + NOON * TILT.sin();
    (SUNSET * angle.sin() + high * angle.cos()).normalize()
}

/// The keyframes blended at `hours`, `0.0..24.0`.
fn blend(hours: f32) -> Frame {
    let next = FRAMES
        .iter()
        .position(|f| f.hour > hours)
        .unwrap_or(FRAMES.len());
    let a = FRAMES[(next + FRAMES.len() - 1) % FRAMES.len()];
    let (b, b_hour) = match FRAMES.get(next) {
        Some(b) => (*b, b.hour),
        None => (FRAMES[0], FRAMES[0].hour + 24.0),
    };
    let t = ((hours - a.hour) / (b_hour - a.hour)).clamp(0.0, 1.0);
    let mix = |x: f32, y: f32| x + (y - x) * t;
    let mix3 = |x: [f32; 3], y: [f32; 3]| [mix(x[0], y[0]), mix(x[1], y[1]), mix(x[2], y[2])];
    Frame {
        hour: hours,
        zenith: mix3(a.zenith, b.zenith),
        horizon: mix3(a.horizon, b.horizon),
        tint: mix3(a.tint, b.tint),
        glow: mix(a.glow, b.glow),
        lux: [
            mix(a.lux[0], b.lux[0]),
            mix(a.lux[1], b.lux[1]),
            mix(a.lux[2], b.lux[2]),
            mix(a.lux[3], b.lux[3]),
        ],
        ev100: mix(a.ev100, b.ev100),
        key_color: mix3(a.key_color, b.key_color),
        rim_color: mix3(a.rim_color, b.rim_color),
        haze_glow: mix(a.haze_glow, b.haze_glow),
        night: mix(a.night, b.night),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exposed(light: &Light, lux: f32) -> f32 {
        lux * crate::pbr::exposure(light.ev100)
    }

    #[test]
    fn keyframes_run_in_order_through_the_day() {
        assert!(FRAMES.windows(2).all(|w| w[0].hour < w[1].hour));
        assert_eq!(FRAMES[0].hour, 0.0);
        assert!(FRAMES.last().is_some_and(|f| f.hour < 24.0));
    }

    #[test]
    fn late_morning_is_the_old_fixed_light() {
        let light = Light::at_hours(10.5);
        let old = Vec3::new(-0.42, 0.6, -0.56).normalize();
        assert!(
            light.key_dir.angle_between(old) < 0.01,
            "{:?}",
            light.key_dir
        );
        assert_eq!(light.key_lux, 4_000.0);
        assert_eq!(light.horizon, ATMOSPHERE.color);
    }

    #[test]
    fn the_sun_rises_and_sets_on_the_clock() {
        assert!(sun(6.0).y.abs() < 1e-5);
        assert!(sun(18.0).y.abs() < 1e-5);
        assert!(sun(12.0).y > 0.7);
        assert!(sun(0.0).y < -0.7);
        assert!(sun(9.0).x < 0.0 && sun(15.0).x > 0.0, "east to west");
    }

    #[test]
    fn the_light_changes_without_a_pop() {
        // What a level patch of ground receives, as the stage exposes it.
        let level = |l: &Light| exposed(l, l.key_lux * l.key_dir.y + l.sky_lux);
        let step = (STEP_SECONDS / 60.0) as u32;
        let mut last = Light::at_hours(0.0);
        for minute in (step..=24 * 60).step_by(step as usize) {
            let light = Light::at_hours(minute as f32 / 60.0);
            let (a, b) = (level(&last), level(&light));
            assert!((a - b).abs() <= 0.1 * a.max(b), "{minute}: {a} to {b}");
            assert!(light.key_dir.angle_between(last.key_dir) < 0.03, "{minute}");
            for (x, y) in light.zenith.iter().zip(&last.zenith) {
                assert!((x - y).abs() < 0.03, "{minute}");
            }
            last = light;
        }
    }

    #[test]
    fn every_light_is_a_valid_stage() {
        for minute in 0..24 * 60 {
            let light = Light::at_hours(minute as f32 / 60.0);
            let day = light.daylight(Daylight {
                zenith: [0.0; 3],
                horizon: [0.0; 3],
                sun: [0.0; 3],
                clouds: 0.38,
                ground: [0.1, 0.11, 0.07],
                glow: 0.0,
            });
            assert!(day.valid(), "{minute}");
            assert!(light.key_dir.y >= LOWEST_KEY.sin() - 1e-4, "{minute}");
            assert!(light.key_dir.is_normalized());
        }
    }

    #[test]
    fn night_is_dim_but_playable_and_lamps_light() {
        let noon = Light::at_hours(12.0);
        let night = Light::at_hours(0.0);
        let level = |l: &Light| exposed(l, l.key_lux + l.sky_lux);
        let ratio = level(&night) / level(&noon);
        assert!((0.08..0.4).contains(&ratio), "{ratio}");
        // A lamp's candela is scaled by the same exposure: it stands out
        // about ten times more at night.
        assert!(crate::pbr::exposure(night.ev100) > 8.0 * crate::pbr::exposure(noon.ev100));
        assert!(night.lamps_lit() && !noon.lamps_lit());
        assert!(Light::at_hours(19.0).lamps_lit() && Light::at_hours(5.0).lamps_lit());
    }

    #[test]
    fn the_darkest_moment_stays_readable() {
        // The running clock passes through every hour, so no hour may go
        // black: what a level patch of ground shows, as the stage exposes
        // it, never falls below a tenth of noon's.
        let level = |l: &Light| exposed(l, l.key_lux * l.key_dir.y + l.sky_lux);
        let noon = level(&Light::at_hours(12.0));
        let (darkest, at) = (0..24 * 60)
            .map(|m| {
                let light = Light::at_hours(m as f32 / 60.0);
                (level(&light) / noon, m)
            })
            .fold((f32::MAX, 0), |a, b| if b.0 < a.0 { b } else { a });
        assert!(darkest >= 0.1, "{darkest} at minute {at}");
    }

    #[test]
    fn the_light_holds_through_a_step() {
        let a = Light::at(TownTime::at_hour(0, 9.0));
        let b = Light::at(TownTime::at_hour(0, 9.0 + 3.0 / 60.0));
        assert_eq!(a, b);
        assert_ne!(a, Light::at(TownTime::at_hour(0, 9.0 + 4.0 / 60.0)));
    }
}
