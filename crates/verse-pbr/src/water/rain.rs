//! Rain on the water and the ground (`docs/verse/water.md`, phase W9).
//!
//! A zone samples its weather (`physics::water::weather`) and hands the
//! frame a [`Rain`]: how hard it rains, how wet exposed surfaces are, and
//! how full the puddles stand. Both renderers draw rain ripples on every
//! water surface from it (`water_rain_slope` in `water.wgsl`, the rain in
//! the water uniform's `look.z`); the physical renderer also wets lit
//! surfaces (`pbr/photo.wgsl`, the frame's `weather` row), after Lagarde's
//! "Water drop" series (2012–2013): porous surfaces darken, every wet
//! surface turns glossier, flat ground pools into puddles whose rain
//! ripples ride on them, and on High rain streaks run down vertical faces.
//! A wet character darkens too (the frame's `weather_figure` row,
//! [`Drying`]). Each tier draws what [`control`] says from uniforms alone, with no
//! texture, so Low's last free sampler slot stays free.

use verse_engine::quality::Tier;

/// The weather on a frame's water and ground, each 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rain {
    /// Rain falling now: ripples on the water and in puddles.
    pub rain: f32,
    /// How wet exposed surfaces are.
    pub wet: f32,
    /// How full the puddles in low ground are.
    pub puddles: f32,
    /// A wet character: its feet (x, y, z) and how wet it is, 0 to 1. One
    /// who swam, stood in the rain, or walked through mist darkens for a
    /// while ([`Drying`]).
    pub figure: [f32; 4],
}

impl Rain {
    /// From a weather sample and the ground it left.
    #[must_use]
    pub fn from_weather(
        weather: &physics::water::Weather,
        ground: &physics::water::Ground,
    ) -> Self {
        Self {
            rain: weather.rain as f32,
            wet: ground.wet as f32,
            puddles: ground.puddles as f32,
            figure: [0.0; 4],
        }
    }

    /// Whether every value is in 0 to 1.
    #[must_use]
    pub fn valid(&self) -> bool {
        [self.rain, self.wet, self.puddles, self.figure[3]]
            .iter()
            .all(|v| (0.0..=1.0).contains(v))
            && self.figure.iter().all(|v| v.is_finite())
    }

    /// The physical renderer's frame row for `tier`: wetness, puddles, rain,
    /// and 1 where the tier streaks vertical faces. Low draws no puddles,
    /// Medium no streaks.
    #[must_use]
    pub fn row(&self, tier: Tier) -> [f32; 4] {
        let c = control(tier);
        [
            self.wet,
            if c.puddles { self.puddles } else { 0.0 },
            self.rain,
            if c.streaks { 1.0 } else { 0.0 },
        ]
    }
}

/// How wet a character is: soaked at once in water, wetting in rain or
/// mist, and drying over [`Drying::DRY`] s after (ours).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drying {
    /// 0 (dry) to 1 (soaked).
    pub wet: f32,
}

impl Drying {
    /// Seconds from soaked to dry.
    pub const DRY: f32 = 180.0;
    /// Seconds of the heaviest rain to soak through.
    pub const SOAK: f32 = 40.0;

    /// Advances `dt` s: `immersed` in water soaks it; `rain` (0 to 1, or a
    /// mist's share) wets it toward that much; otherwise it dries.
    pub fn tick(&mut self, dt: f32, immersed: bool, rain: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if immersed {
            self.wet = 1.0;
        } else {
            // Rain holds it at least as wet as it falls; past that it dries.
            let floor = if rain > 0.02 { rain.min(1.0) } else { 0.0 };
            if self.wet < floor {
                self.wet = (self.wet + rain * dt / Self::SOAK).min(floor);
            } else {
                self.wet = (self.wet - dt / Self::DRY).max(floor);
            }
        }
    }
}

/// What a tier draws of the rain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    /// Puddles in low ground, with rain ripples on them.
    pub puddles: bool,
    /// Streaks of wet on vertical faces.
    pub streaks: bool,
    /// Rain streak particles alive at once around the camera, within the
    /// sprite budget beside the water effects' own.
    pub particles: usize,
    /// Drops a frame written into the ripple field as small impacts, within
    /// the tier's ripple sources.
    pub sources: usize,
}

/// The rain each tier draws (`docs/verse/water.md`, Budgets per tier). On
/// every tier: rain ripples on the water, streak particles, and darker,
/// glossier wet surfaces.
#[must_use]
pub fn control(tier: Tier) -> Control {
    match tier {
        Tier::Low => Control {
            puddles: false,
            streaks: false,
            particles: 64,
            sources: 2,
        },
        Tier::Medium => Control {
            puddles: true,
            streaks: false,
            particles: 320,
            sources: 4,
        },
        Tier::High => Control {
            puddles: true,
            streaks: true,
            particles: 640,
            sources: 8,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_draw_less_and_keep_within_their_budgets() {
        // Rain particles fit beside the water effects (64, 256, 512) in
        // the sprite budget (160, 768, 1,536), and rain's ripple sources
        // leave most of the tier's (8, 16, 32) to movers.
        for (tier, water_fx, sprites, sources) in [
            (Tier::Low, 64, 160, 8),
            (Tier::Medium, 256, 768, 16),
            (Tier::High, 512, 1_536, 32),
        ] {
            let c = control(tier);
            assert!(water_fx + c.particles <= sprites, "{tier:?}");
            assert!(c.sources * 4 <= sources, "{tier:?}");
        }
        let rain = Rain {
            rain: 1.0,
            wet: 0.8,
            puddles: 0.6,
            figure: [0.0; 4],
        };
        assert_eq!(rain.row(Tier::Low), [0.8, 0.0, 1.0, 0.0]);
        assert_eq!(rain.row(Tier::Medium), [0.8, 0.6, 1.0, 0.0]);
        assert_eq!(rain.row(Tier::High), [0.8, 0.6, 1.0, 1.0]);
        assert!(rain.valid());
        assert!(!Rain { rain: 2.0, ..rain }.valid());
    }

    #[test]
    fn characters_soak_in_water_and_dry_after() {
        let mut d = Drying::default();
        d.tick(0.1, true, 0.0);
        assert_eq!(d.wet, 1.0);
        d.tick(Drying::DRY / 2.0, false, 0.0);
        assert!((d.wet - 0.5).abs() < 1e-5);
        d.tick(Drying::DRY, false, 0.0);
        assert_eq!(d.wet, 0.0);
        // Rain wets it only as far as it rains.
        for _ in 0..600 {
            d.tick(0.5, false, 0.6);
        }
        assert!((d.wet - 0.6).abs() < 1e-5);
    }
}
