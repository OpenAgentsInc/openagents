//! Everglade's weather (`docs/verse/water.md`, Weather, phase W9).
//!
//! - [`Sky`]: the zone's [`Schedule`] from its temperate [`CLIMATE`] and
//!   [`SEED`], sampled each frame at the world tick
//!   (`physics::water::tick_at`) with the spells' overlays, the ground the
//!   rain left, and the bounded rise of the ponds and the run. A pin or a
//!   fixed tick stands in for captures and tests ([`Sky::pin`],
//!   [`Sky::fix`], `VERSE_WEATHER`).
//! - [`weather_stage`]: the weather on a lit stage: cloud cover, a dimmer,
//!   grayer key and sky under it, thicker fog in fog and rain, and
//!   lightning's flash, stepped so the sky light rebakes rarely.
//! - [`Rainfall`]: the rain's effects around the camera, within the tier's
//!   budgets (`verse_pbr::water::rain::control`): wind-tilted streaks,
//!   splash-back where drops hit hard ground, drips from the eaves while
//!   the ground is wet, and drops on the water as small impacts in the
//!   ripple field.
//!
//! The Water Lab reuses all three with its own climate.

use glam::{DVec2, Vec2, Vec3};
use physics::water::weather::{Climate, Ground, Overlay, Schedule, State, Weather};
use verse_engine::quality::Tier;
use verse_pbr::pbr::Neon;
use verse_pbr::water::Source;
use verse_pbr::water::rain::{self, Rain};

use crate::fx::{Handle, Particles, Spawn};

/// Everglade's climate: temperate and mostly clear.
pub const CLIMATE: Climate = Climate::TEMPERATE;
/// Everglade's weather seed.
pub const SEED: u64 = 0xE7E5_61AD;

/// A zone's weather this frame, from its schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct Sky {
    pub schedule: Schedule,
    /// A world tick to sample in place of the clock's, for captures and
    /// tests.
    pub fixed: Option<u64>,
    /// Spell weather over the schedule this frame, and the tick it is
    /// timed by when the zone's spells keep their own clock.
    pub overlays: Vec<Overlay>,
    pub overlay_tick: Option<u64>,
    /// The tick last sampled and what it gave.
    pub tick: u64,
    pub weather: Weather,
    pub ground: Ground,
    /// The ponds' and streams' rise, m, and their currents' gain.
    pub rise: f64,
    pub flow_gain: f64,
}

impl Sky {
    /// The schedule, pinned by `VERSE_WEATHER` (a state's name) when set:
    /// the development control for captures.
    #[must_use]
    pub fn new(climate: Climate, seed: u64) -> Self {
        let mut schedule = Schedule::new(climate, seed);
        if let Some(state) = std::env::var("VERSE_WEATHER")
            .ok()
            .and_then(|v| State::parse(&v))
        {
            schedule = schedule.pinned(state);
        }
        Self {
            schedule,
            fixed: None,
            overlays: Vec::new(),
            overlay_tick: None,
            tick: 0,
            weather: Weather::CALM,
            ground: Ground::default(),
            rise: 0.0,
            flow_gain: 1.0,
        }
    }

    /// Everglade's.
    #[must_use]
    pub fn everglade() -> Self {
        Self::new(CLIMATE, SEED)
    }

    /// Pins `state` for all time.
    pub fn pin(&mut self, state: State) {
        self.set_schedule(self.schedule.clone().pinned(state));
    }

    /// Replaces the schedule, such as one with a capture's pins.
    pub fn set_schedule(&mut self, schedule: Schedule) {
        self.schedule = schedule;
        // The ground and the rise are read again at the next sample.
        self.tick = 0;
    }

    /// Samples at `tick` instead of the clock from now on, or the clock
    /// again with `None`.
    pub fn fix(&mut self, tick: Option<u64>) {
        self.fixed = tick;
        self.tick = 0;
    }

    /// The world tick now: the fixed one, or the clock's.
    #[must_use]
    pub fn now(&self) -> u64 {
        self.fixed.unwrap_or_else(|| {
            let ms = web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            physics::water::tick_at(ms)
        })
    }

    /// Samples the weather at the eye `(x, z)` now.
    pub fn update(&mut self, eye: Vec2) {
        let tick = self.now();
        self.sample(tick, eye);
    }

    /// Samples the weather at `eye` and `tick`. The ground and the rise are
    /// the slow part; they move little within a second, so they are read
    /// once a second of world time.
    pub fn sample(&mut self, tick: u64, eye: Vec2) {
        let p = DVec2::new(f64::from(eye.x), f64::from(eye.y));
        self.weather = self.schedule.sample(tick);
        let timed = self.overlay_tick.unwrap_or(tick);
        self.schedule
            .lay(&mut self.weather, p, timed, &self.overlays);
        let second = physics::water::TICK_HZ;
        if tick / second != self.tick / second || self.tick == 0 {
            self.ground = self.schedule.ground(tick);
            self.rise = self.schedule.rise(tick);
            self.flow_gain = self.schedule.flow_gain(tick);
        }
        // Spell rain wets the ground under it at once.
        self.ground.wet = self.ground.wet.max(self.weather.rain * 0.8);
        self.tick = tick;
    }

    /// The rain the frame's water and ground draw.
    #[must_use]
    pub fn rain(&self) -> Rain {
        Rain::from_weather(&self.weather, &self.ground)
    }

    /// One line for a HUD: the state, the rain, and the wind.
    #[must_use]
    pub fn caption(&self) -> String {
        let w = &self.weather;
        let pinned = if self.schedule.pinned_at(self.tick).is_some() {
            " (pinned)"
        } else {
            ""
        };
        let mut line = format!(
            "Weather: {}{pinned}, wind {:.0} m/s",
            name(w.state),
            w.wind_speed()
        );
        if w.heavy_precipitation() {
            line.push_str(", Heavy Precipitation");
        }
        if w.strong_wind() {
            line.push_str(", Strong Wind");
        }
        line
    }
}

/// A state's name for people.
#[must_use]
pub fn name(state: State) -> &'static str {
    match state {
        State::Clear => "Clear",
        State::Overcast => "Overcast",
        State::Fog => "Fog",
        State::Rain => "Rain",
        State::Storm => "Storm",
    }
}

/// `v` held to steps of 1/8, so the sky light rebakes only when the
/// weather changes by a step.
fn stepped(v: f64) -> f32 {
    ((v.clamp(0.0, 1.0) * 8.0).round() / 8.0) as f32
}

/// The weather on `neon`, a lit stage: cloud cover over the sky, the key
/// and the sky's light dimmed and grayed under it, fog and rain thickening
/// the height fog, and lightning's flash.
pub fn weather_stage(neon: &mut Neon, weather: &Weather) {
    let cloud = stepped(weather.cloud);
    let fog = stepped(weather.fog);
    let rain = stepped(weather.rain);
    // Clear skies keep the stage's own look.
    let over = ((cloud - 0.15) / 0.85).clamp(0.0, 1.0);
    let gray = |c: [f32; 3], k: f32| {
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        [
            c[0] + (l * 0.9 - c[0]) * k,
            c[1] + (l * 0.9 - c[1]) * k,
            c[2] + (l * 0.95 - c[2]) * k,
        ]
    };
    if let Some(day) = neon.daylight.as_mut() {
        day.clouds = day.clouds + (1.0 - day.clouds) * over;
        day.zenith = gray(day.zenith, over * 0.85);
        day.horizon = gray(day.horizon, over * 0.7);
        day.glow *= 1.0 - over;
    }
    if let Some(key) = neon.key.as_mut() {
        key.illuminance *= 1.0 - 0.9 * over;
        key.rim_illuminance *= 1.0 - 0.5 * over;
        key.sky *= 1.0 - 0.35 * over;
    }
    neon.field = gray(neon.field, over * 0.7);
    if let Some(height) = neon.height_fog.as_mut() {
        height.density *= 1.0 + 6.0 * fog + 1.5 * rain;
        height.max_opacity = (height.max_opacity + 0.4 * fog + 0.15 * rain).min(1.0);
        height.sun_strength *= 1.0 - over;
    }
    neon.fog_end *= 1.0 - 0.55 * fog - 0.2 * rain;
    neon.fog_start *= 1.0 - 0.6 * fog;
    neon.sky_flash = neon.sky_flash.max(weather.lightning as f32);
}

/// The rain's effects around the camera.
pub struct Rainfall {
    pub particles: Particles,
    control: rain::Control,
    /// The streak emitters running now.
    streaks: Vec<Handle>,
    /// Drops owed to splash-back and to the eaves.
    splash_owed: f32,
    drip_owed: f32,
    rng: u32,
    /// Drops on the water this frame, for the ripple field.
    pub sources: Vec<Source>,
    /// Effects skipped for the budget.
    pub skipped: u32,
}

/// Particles alive in one streak emitter at most: its rate times its life.
const STREAK_PEAK: usize = 57;
/// Splash-backs a second in the heaviest rain, within 7 m of the eye.
const SPLASH_RATE: f32 = 45.0;
/// Drips a second from the eaves in reach while the ground is soaked.
const DRIP_RATE: f32 = 6.0;
/// How far from the eye splash-back and drips are made, m.
const NEAR: f32 = 7.0;
const EAVE_REACH: f32 = 14.0;

impl Rainfall {
    #[must_use]
    pub fn new(tier: Tier) -> Self {
        Self {
            particles: Particles::new(0x0052_A1F0),
            control: rain::control(tier),
            streaks: Vec::new(),
            splash_owed: 0.0,
            drip_owed: 0.0,
            rng: 0x9E37_79B9,
            sources: Vec::new(),
            skipped: 0,
        }
    }

    /// Sets the tier's budgets.
    pub fn set_tier(&mut self, tier: Tier) {
        self.control = rain::control(tier);
    }

    /// The tier's rain particle budget.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.control.particles
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / 16_777_216.0
    }

    /// Advances `dt` s with the eye at `eye` looking along `forward` under
    /// `weather` over `ground`. `land` is the hard ground's height at
    /// `(x, z)`, none over water; `water` the water's surface there, none on
    /// land; `eaves` the points rain drips from.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        dt: f32,
        eye: Vec3,
        forward: Vec3,
        weather: &Weather,
        ground: &Ground,
        land: impl Fn(f32, f32) -> Option<f32>,
        water: impl Fn(f32, f32) -> Option<f32>,
        eaves: &[[f32; 3]],
    ) {
        self.tick_covered(
            dt,
            eye,
            forward,
            weather,
            ground,
            land,
            water,
            eaves,
            |_| true,
        );
    }

    /// Advances rain with a live sky-exposure query for splash and ripple
    /// impacts. The CPU sprite fallback uses the same query when drawing.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_covered(
        &mut self,
        dt: f32,
        eye: Vec3,
        forward: Vec3,
        weather: &Weather,
        ground: &Ground,
        land: impl Fn(f32, f32) -> Option<f32>,
        water: impl Fn(f32, f32) -> Option<f32>,
        eaves: &[[f32; 3]],
        open: impl Fn(Vec3) -> bool,
    ) {
        self.sources.clear();
        let rain = weather.rain as f32;
        // Streak emitters: as many as the rain and the budget allow, one
        // ahead of the eye and the rest on a ring around it.
        let most = (self.control.particles * 4 / 5 / STREAK_PEAK).max(1);
        let want = if rain > 0.02 {
            ((rain * most as f32).ceil() as usize).clamp(1, most)
        } else {
            0
        };
        while self.streaks.len() > want {
            if let Some(h) = self.streaks.pop() {
                self.particles.stop(h);
            }
        }
        let wind = Vec3::new(weather.wind.x as f32, 0.0, weather.wind.y as f32);
        // Drops fall at about 9.5 m/s and drift with the wind: the effect's
        // axis points back up along their fall.
        let axis = (Vec3::Y * 9.5 - wind).normalize_or(Vec3::Y);
        let ahead = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::NEG_Z);
        let place = |k: usize| {
            let center = eye + ahead * 3.0 + Vec3::Y * 7.0 + wind * 0.7;
            if k == 0 {
                center
            } else {
                let angle = k as f32 / (most.max(2) - 1) as f32 * std::f32::consts::TAU;
                center + Vec3::new(angle.sin(), 0.0, angle.cos()) * 9.0
            }
        };
        while self.streaks.len() < want {
            let k = self.streaks.len();
            let Some(h) = self
                .particles
                .start("weather_rain", Spawn::at(place(k)).along(axis))
            else {
                break;
            };
            self.streaks.push(h);
        }
        for (k, h) in self.streaks.clone().into_iter().enumerate() {
            self.particles.place(h, place(k), Vec3::ZERO);
        }
        // Splash-back on hard ground and drops on the water near the eye.
        self.splash_owed += SPLASH_RATE * rain * dt;
        let mut sources = 0;
        while self.splash_owed >= 1.0 {
            self.splash_owed -= 1.0;
            let a = self.random() * std::f32::consts::TAU;
            let r = NEAR * self.random().sqrt();
            let (x, z) = (eye.x + a.sin() * r, eye.z + a.cos() * r);
            if let Some(y) = water(x, z) {
                if !open(Vec3::new(x, y, z)) {
                    continue;
                }
                if sources < self.control.sources {
                    self.sources
                        .push(Source::impact(Vec2::new(x, z), 0.12, 0.004 * (0.5 + rain)));
                    sources += 1;
                }
            } else if let Some(y) = land(x, z) {
                if !open(Vec3::new(x, y, z)) {
                    continue;
                }
                if self.particles.len() + 3 <= self.control.particles {
                    self.particles
                        .start("weather_splash", Spawn::at(Vec3::new(x, y + 0.01, z)));
                } else {
                    self.skipped += 1;
                }
            }
        }
        // Drips from the eaves while the ground is wet.
        let soaked = ((ground.wet as f32 - 0.3) / 0.7).clamp(0.0, 1.0);
        self.drip_owed += DRIP_RATE * soaked * dt;
        if self.drip_owed >= 1.0 {
            let near: Vec<[f32; 3]> = eaves
                .iter()
                .filter(|p| Vec2::new(p[0] - eye.x, p[2] - eye.z).length() < EAVE_REACH)
                .copied()
                .collect();
            while self.drip_owed >= 1.0 {
                self.drip_owed -= 1.0;
                if near.is_empty() {
                    continue;
                }
                let k = ((self.random() * near.len() as f32) as usize).min(near.len() - 1);
                let [x, y, z] = near[k];
                if self.particles.len() + 6 <= self.control.particles {
                    self.particles
                        .start("water_drips", Spawn::at(Vec3::new(x, y, z)).scaled(0.5));
                } else {
                    self.skipped += 1;
                }
            }
        }
        let floor = |x: f32, z: f32| land(x, z).or_else(|| water(x, z)).unwrap_or(-1e3);
        self.particles.tick(dt, floor);
    }

    /// Draws only sky-exposed rain. Test the streak's bottom as well as
    /// its center so its elongated sprite cannot extend through a roof.
    /// Near a sheltered eye, fade the remaining outdoor rain over 2 m.
    pub fn draw_covered(
        &self,
        out: &mut Vec<crate::fx::Sprite>,
        eye: Vec3,
        open: impl Fn(Vec3) -> bool,
    ) {
        let first = out.len();
        self.particles.draw(out);
        let sheltered = !open(eye);
        let mut kept = first;
        for i in first..out.len() {
            let mut sprite = out[i];
            let p = sprite.at;
            let end = p + sprite.tail;
            let bottom = p.min(end) - Vec3::Y * sprite.half;
            if !open(bottom) {
                continue;
            }
            if sheltered {
                sprite.alpha *= (eye.distance(p) / 2.0).clamp(0.0, 1.0);
            }
            out[kept] = sprite;
            kept += 1;
        }
        out.truncate(kept);
    }

    /// Draws the particles.
    pub fn draw(&self, out: &mut Vec<crate::fx::Sprite>) {
        self.particles.draw(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_overrides_the_schedule_and_rain_wets_the_frame() {
        let mut sky = Sky::everglade();
        sky.pin(State::Rain);
        sky.fix(Some(1_000_000_000));
        sky.update(Vec2::ZERO);
        assert_eq!(sky.weather.state, State::Rain);
        assert!(sky.rain().rain > 0.3 && sky.rain().wet > 0.9);
        assert!(sky.rise > 0.05 && sky.rise <= physics::water::weather::MAX_RISE);
        sky.pin(State::Clear);
        sky.update(Vec2::ZERO);
        assert_eq!(sky.rain().rain, 0.0);
        assert!(sky.caption().starts_with("Weather: Clear (pinned)"));
    }

    #[test]
    fn the_stage_darkens_under_cloud_and_thickens_in_fog() {
        let base = super::super::Everglade::glade_stage(
            0.0,
            &super::super::time_of_day::Light::at_hours(10.5),
        );
        let mut stormy = base;
        let mut sky = Sky::everglade();
        sky.pin(State::Storm);
        sky.fix(Some(77));
        sky.update(Vec2::ZERO);
        weather_stage(&mut stormy, &sky.weather);
        assert!(stormy.key.unwrap().illuminance < base.key.unwrap().illuminance * 0.4);
        assert!(stormy.fog_end < base.fog_end);
        let mut clear = base;
        weather_stage(&mut clear, &Weather::CALM);
        assert_eq!(clear.key, base.key);
    }

    #[test]
    fn known_roofs_cull_streaks_and_covered_impacts_on_every_tier() {
        use verse_world::social::solids::{Roof, Solids};
        let mut cover = Solids::over(|_, _| 0.0);
        cover.add_roof(Roof {
            center: [0.0; 2],
            across: [1.0, 0.0],
            half: [40.0; 2],
            eave: 20.0,
            ridge: 20.0,
        });
        let eye = Vec3::Y * 1.6;
        let storm = Weather {
            rain: 1.0,
            ..Weather::CALM
        };
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let mut fall = Rainfall::new(tier);
            for _ in 0..120 {
                fall.tick_covered(
                    1.0 / 60.0,
                    eye,
                    Vec3::NEG_Z,
                    &storm,
                    &Ground::default(),
                    |_, _| Some(0.0),
                    |_, _| None,
                    &[],
                    |p| cover.rain_open(p),
                );
            }
            let mut outside = Vec::new();
            fall.draw(&mut outside);
            assert!(!outside.is_empty(), "outdoor rain is present on {tier:?}");
            let mut inside = Vec::new();
            fall.draw_covered(&mut inside, eye, |p| cover.rain_open(p));
            assert!(inside.is_empty(), "streaks below the roof: {tier:?}");
            let mut open_draw = Vec::new();
            fall.draw_covered(&mut open_draw, eye, |_| true);
            assert_eq!(open_draw, outside, "outdoors is unchanged");
            for _ in 0..60 {
                fall.tick_covered(
                    1.0 / 60.0,
                    eye,
                    Vec3::NEG_Z,
                    &storm,
                    &Ground::default(),
                    |_, _| None,
                    |_, _| Some(0.0),
                    &[],
                    |p| cover.rain_open(p),
                );
                assert!(
                    fall.sources.is_empty(),
                    "covered water has no new rain impacts"
                );
            }
        }
        // With rain streaks disabled, all remaining particles would be splashes.
        let mut fall = Rainfall::new(Tier::High);
        fall.control.particles = 4;
        for _ in 0..60 {
            fall.tick_covered(
                1.0 / 60.0,
                eye,
                Vec3::NEG_Z,
                &storm,
                &Ground::default(),
                |_, _| Some(0.0),
                |_, _| None,
                &[],
                |p| cover.rain_open(p),
            );
        }
        assert!(
            fall.particles.len() <= STREAK_PEAK,
            "covered land adds no splash emitters"
        );
    }

    #[test]
    fn rainfall_keeps_to_the_tier_budget() {
        let storm = Weather {
            state: State::Storm,
            rain: 1.0,
            ..Weather::CALM
        };
        let soaked = Ground {
            wet: 1.0,
            puddles: 1.0,
        };
        let eaves = [[2.0, 4.0, 2.0], [-2.0, 4.0, 1.0]];
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let mut fall = Rainfall::new(tier);
            let mut most = 0;
            let mut sources = 0;
            for _ in 0..240 {
                fall.tick(
                    1.0 / 60.0,
                    Vec3::new(0.0, 1.6, 0.0),
                    Vec3::NEG_Z,
                    &storm,
                    &soaked,
                    |x, _| (x < 0.0).then_some(0.0),
                    |x, _| (x >= 0.0).then_some(0.0),
                    &eaves,
                );
                most = most.max(fall.particles.len());
                sources = sources.max(fall.sources.len());
            }
            assert!(most > 0, "{tier:?}");
            assert!(most <= fall.budget() + 8, "{tier:?}: {most}");
            assert!(sources > 0 && sources <= rain::control(tier).sources);
        }
        // No rain, no streaks.
        let mut dry = Rainfall::new(Tier::High);
        for _ in 0..120 {
            dry.tick(
                1.0 / 60.0,
                Vec3::ZERO,
                Vec3::NEG_Z,
                &Weather::CALM,
                &Ground::default(),
                |_, _| Some(0.0),
                |_, _| None,
                &[],
            );
        }
        assert!(dry.particles.is_empty());
    }
}
