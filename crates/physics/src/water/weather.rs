//! Weather: a deterministic schedule for each zone (`docs/verse/water.md`,
//! Weather).
//!
//! Weather is a pure function of a zone's [`Climate`], its seed, and the
//! world tick ([`super::tick_at`]), so every client, a late joiner, a
//! replay, and an offline session see the same sky without any stream of
//! their own, as they already see the same waves.
//!
//! - [`Schedule`]: world time splits into [`BLOCK_SECONDS`] blocks; a hash
//!   of the seed and the block index draws each block's [`State`] from the
//!   climate's weights, and states blend over [`BLEND_SECONDS`] at block
//!   boundaries. Seeded smooth noise varies the rain and the wind inside a
//!   block. [`Schedule::sample`] is the [`Weather`] at a tick.
//! - Pins: a development control that overrides the schedule from a tick
//!   on, for captures and tests ([`Schedule::pin`]). Players have no
//!   weather control except spells.
//! - [`Overlay`]: spell weather (Create Water's rain, Sleet Storm, Fog
//!   Cloud, Call Lightning's cloud, Storm of Vengeance) as bounded local
//!   areas over the schedule, each with a start tick and a duration, as
//!   host events. They never change the schedule.
//! - [`Schedule::ground`]: how wet the ground is and how full its puddles
//!   are, integrated from the schedule's rain over the last hour on a grid
//!   fixed to the world tick, so it agrees across clients too.
//! - [`Schedule::rise`]: rising water, the schedule's mean rain over the
//!   last 30 minutes times [`MAX_RISE`], so it is bounded and needs no
//!   event; [`Schedule::flow_gain`] strengthens currents with it.
//!
//! Nothing here renders; the SRD's rules on weather live in
//! `verse_world::spells::water::weather`.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::event::TICK_HZ;

/// A schedule block's length, s.
pub const BLOCK_SECONDS: u64 = 600;
/// How long one state blends into the next at a block boundary, s.
pub const BLEND_SECONDS: u64 = 60;
/// The rain and wind vary smoothly through knots this far apart, s.
pub const KNOT_SECONDS: u64 = 90;
/// Lightning in a storm: at most one strike in each window this long, s.
pub const STRIKE_SECONDS: u64 = 6;
/// Rising water reads the rain over this long, s.
pub const RISE_SECONDS: u64 = 1_800;
/// The most rain raises a pond or a stream, m.
pub const MAX_RISE: f64 = 0.15;
/// How much more a fully risen stream's current runs (ours).
pub const MAX_FLOW_GAIN: f64 = 0.6;
/// The ground's wetness and puddles integrate over this long, s, on a grid
/// of [`GROUND_STEP`] s fixed to the world tick.
pub const GROUND_SECONDS: u64 = 3_600;
pub const GROUND_STEP: u64 = 30;
/// Rain at or above this is heavy rain: SRD 5.2.1 Heavy Precipitation.
pub const HEAVY_RAIN: f64 = 0.5;
/// Wind at or above this, m/s (20 mph), is SRD 5.2.1 Strong Wind.
pub const STRONG_WIND: f64 = 9.0;

const BLOCK_TICKS: u64 = BLOCK_SECONDS * TICK_HZ;
const BLEND_TICKS: u64 = BLEND_SECONDS * TICK_HZ;
const KNOT_TICKS: u64 = KNOT_SECONDS * TICK_HZ;
const STRIKE_TICKS: u64 = STRIKE_SECONDS * TICK_HZ;

/// One of the schedule's weathers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum State {
    Clear,
    Overcast,
    Fog,
    Rain,
    Storm,
}

impl State {
    pub const ALL: [Self; 5] = [
        Self::Clear,
        Self::Overcast,
        Self::Fog,
        Self::Rain,
        Self::Storm,
    ];

    /// Its name, as `--weather` takes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Overcast => "overcast",
            Self::Fog => "fog",
            Self::Rain => "rain",
            Self::Storm => "storm",
        }
    }

    /// The state named `name`, ignoring case.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(name.trim()))
    }

    /// The next state in [`Self::ALL`], wrapping.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    /// What the state looks like at its steadiest.
    const fn profile(self) -> Profile {
        match self {
            Self::Clear => Profile {
                rain: 0.0,
                cloud: 0.15,
                fog: 0.0,
                wind: 0.35,
                storm: 0.0,
            },
            Self::Overcast => Profile {
                rain: 0.0,
                cloud: 0.85,
                fog: 0.1,
                wind: 0.55,
                storm: 0.0,
            },
            Self::Fog => Profile {
                rain: 0.0,
                cloud: 0.7,
                fog: 1.0,
                wind: 0.1,
                storm: 0.0,
            },
            Self::Rain => Profile {
                rain: 0.55,
                cloud: 0.95,
                fog: 0.25,
                wind: 0.6,
                storm: 0.0,
            },
            // A storm's wind runs past the climate's usual range.
            Self::Storm => Profile {
                rain: 1.0,
                cloud: 1.0,
                fog: 0.3,
                wind: 1.4,
                storm: 1.0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    rain: f64,
    cloud: f64,
    fog: f64,
    wind: f64,
    storm: f64,
}

impl Profile {
    fn mix(a: Self, b: Self, t: f64) -> Self {
        let m = |x: f64, y: f64| x + (y - x) * t;
        Self {
            rain: m(a.rain, b.rain),
            cloud: m(a.cloud, b.cloud),
            fog: m(a.fog, b.fog),
            wind: m(a.wind, b.wind),
            storm: m(a.storm, b.storm),
        }
    }
}

/// A zone's climate: how often each [`State`] comes, in [`State::ALL`]'s
/// order, and the range its wind usually blows in, m/s. A zone with no
/// weather (indoors, the Grid, Lagrange 1) has no climate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Climate {
    pub weights: [f64; 5],
    pub wind: [f64; 2],
}

impl Climate {
    /// Temperate and mostly clear, as Everglade's glade.
    pub const TEMPERATE: Self = Self {
        weights: [0.55, 0.2, 0.08, 0.13, 0.04],
        wind: [0.5, 7.0],
    };
    /// Wetter and windier, as a coast.
    pub const COASTAL: Self = Self {
        weights: [0.32, 0.25, 0.1, 0.2, 0.13],
        wind: [2.0, 12.0],
    };

    /// Whether the weights are finite, non-negative, and not all zero, and
    /// the wind's range is finite and ordered.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.weights.iter().all(|w| w.is_finite() && *w >= 0.0)
            && self.weights.iter().sum::<f64>() > 0.0
            && self.wind.iter().all(|w| w.is_finite() && *w >= 0.0)
            && self.wind[0] <= self.wind[1]
    }

    /// The state a uniform draw `u` in [0, 1) picks by the weights.
    #[must_use]
    pub fn draw(&self, u: f64) -> State {
        let total: f64 = self.weights.iter().sum();
        let mut left = u * total;
        for (state, w) in State::ALL.into_iter().zip(self.weights) {
            if left < w {
                return state;
            }
            left -= w;
        }
        State::ALL
            .into_iter()
            .zip(self.weights)
            .rev()
            .find(|(_, w)| *w > 0.0)
            .map_or(State::Clear, |(s, _)| s)
    }
}

/// The weather at one place and tick.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    /// The state that holds now: the block's, or during a blend the one
    /// more than halfway in.
    pub state: State,
    /// Rain, 0 (none) to 1 (a storm's downpour); [`HEAVY_RAIN`] and over is
    /// heavy rain.
    pub rain: f64,
    /// The wind, m/s, in x and z: where it blows toward.
    pub wind: DVec2,
    /// Fog, 0 to 1.
    pub fog: f64,
    /// Cloud cover, 0 to 1.
    pub cloud: f64,
    /// Lightning's flash now, 0 to 1.
    pub lightning: f64,
}

impl Weather {
    /// A clear, still sky.
    pub const CALM: Self = Self {
        state: State::Clear,
        rain: 0.0,
        wind: DVec2::ZERO,
        fog: 0.0,
        cloud: 0.0,
        lightning: 0.0,
    };

    /// The wind's speed, m/s.
    #[must_use]
    pub fn wind_speed(&self) -> f64 {
        self.wind.length()
    }

    /// Heavy rain or a storm: SRD 5.2.1 Heavy Precipitation.
    #[must_use]
    pub fn heavy_precipitation(&self) -> bool {
        self.state == State::Storm || self.rain >= HEAVY_RAIN
    }

    /// Wind of [`STRONG_WIND`] or more: SRD 5.2.1 Strong Wind.
    #[must_use]
    pub fn strong_wind(&self) -> bool {
        self.wind_speed() >= STRONG_WIND
    }
}

/// How the rain has left the ground: both 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    /// How wet exposed surfaces are: darker, glossier, and streaked.
    pub wet: f64,
    /// How full the puddles in low ground are.
    pub puddles: f64,
}

/// A kind of spell weather.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayKind {
    /// Create Water's rain.
    Rain,
    /// Sleet Storm: sleet as heavy as a storm's rain.
    Sleet,
    /// Fog Cloud.
    Fog,
    /// Call Lightning's storm cloud.
    Cloud,
    /// Storm of Vengeance: a storm under the cloud, strong wind included.
    Storm,
}

/// Spell weather: a bounded local area over the schedule, as a host event
/// with a start tick and a duration.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    pub kind: OverlayKind,
    /// Its middle (x, z) and its radius, m.
    pub center: DVec2,
    pub radius: f64,
    pub start: u64,
    /// Ticks it lasts.
    pub duration: u64,
}

impl Overlay {
    /// How much of it applies at `p` and `tick`: 1 inside, easing to 0 over
    /// its last meter and its first and last 2 s.
    #[must_use]
    pub fn weight(&self, p: DVec2, tick: u64) -> f64 {
        if tick < self.start || tick >= self.start.saturating_add(self.duration) {
            return 0.0;
        }
        let edge = TICK_HZ as f64 * 2.0;
        let age = (tick - self.start) as f64;
        let left = (self.start + self.duration - tick) as f64;
        let time = (age / edge).min(left / edge).min(1.0);
        let d = p.distance(self.center);
        let space = (self.radius - d).clamp(0.0, 1.0);
        smooth(time) * smooth(space)
    }
}

/// A zone's weather over time ([`self`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub climate: Climate,
    pub seed: u64,
    /// Development pins: from each tick on, the state is pinned, in
    /// ascending order of tick.
    pins: Vec<(u64, State)>,
}

impl Schedule {
    #[must_use]
    pub fn new(climate: Climate, seed: u64) -> Self {
        Self {
            climate,
            seed,
            pins: Vec::new(),
        }
    }

    /// This schedule with `state` pinned for all time.
    #[must_use]
    pub fn pinned(mut self, state: State) -> Self {
        self.pins = vec![(0, state)];
        self
    }

    /// Pins `state` from `tick` on, replacing pins at or after it; it blends
    /// in over [`BLEND_SECONDS`] as a block does.
    pub fn pin(&mut self, tick: u64, state: State) {
        self.pins.retain(|(t, _)| *t < tick);
        self.pins.push((tick, state));
    }

    /// Drops every pin: the schedule alone again.
    pub fn unpin(&mut self) {
        self.pins.clear();
    }

    /// The pins, in order.
    #[must_use]
    pub fn pins(&self) -> &[(u64, State)] {
        &self.pins
    }

    /// The pinned state at `tick`, if one is.
    #[must_use]
    pub fn pinned_at(&self, tick: u64) -> Option<State> {
        self.pins
            .iter()
            .rev()
            .find(|(t, _)| *t <= tick)
            .map(|(_, s)| *s)
    }

    /// Block `index`'s state by the climate, pins aside.
    #[must_use]
    pub fn block(&self, index: u64) -> State {
        self.climate.draw(unit(hash(self.seed, index, 0x0B10_C4ED)))
    }

    /// The state before `tick`'s transition, the state after, and how far
    /// the blend from one to the other has run, 0 to 1 (eased).
    #[must_use]
    pub fn states(&self, tick: u64) -> (State, State, f64) {
        let index = self.pins.iter().rposition(|(t, _)| *t <= tick);
        if let Some(i) = index {
            let (start, state) = self.pins[i];
            let before = if i > 0 {
                self.pins[i - 1].1
            } else if start == 0 {
                state
            } else {
                let (b, a, t) = self.scheduled(start - 1);
                if t < 0.5 { b } else { a }
            };
            let t = (tick - start) as f64 / BLEND_TICKS as f64;
            return (before, state, smooth(t.min(1.0)));
        }
        self.scheduled(tick)
    }

    fn scheduled(&self, tick: u64) -> (State, State, f64) {
        let block = tick / BLOCK_TICKS;
        let into = tick % BLOCK_TICKS;
        let now = self.block(block);
        let before = if block == 0 {
            now
        } else {
            self.block(block - 1)
        };
        let t = into as f64 / BLEND_TICKS as f64;
        (before, now, smooth(t.min(1.0)))
    }

    /// The weather over the whole zone at `tick`.
    #[must_use]
    pub fn sample(&self, tick: u64) -> Weather {
        let (before, after, t) = self.states(tick);
        let p = Profile::mix(before.profile(), after.profile(), t);
        let state = if t < 0.5 { before } else { after };
        let rain_var = 0.7 + 0.6 * self.noise(tick, 1);
        let wind_var = 0.75 + 0.5 * self.noise(tick, 2);
        let [low, high] = self.climate.wind;
        let speed = (low + (high - low) * p.wind) * wind_var;
        // The wind veers slowly about the seed's prevailing direction.
        let heading = unit(hash(self.seed, 0, 0x00DE_C1DE)) * std::f64::consts::TAU
            + (self.noise(tick, 3) - 0.5) * 1.6;
        let mut weather = Weather {
            state,
            rain: (p.rain * rain_var).clamp(0.0, 1.0),
            wind: DVec2::new(heading.sin(), heading.cos()) * speed,
            fog: p.fog,
            cloud: p.cloud,
            lightning: p.storm * self.flash(tick),
        };
        disperse(&mut weather);
        weather
    }

    /// The weather at `p` and `tick` with the spells' `overlays` laid over
    /// the schedule.
    #[must_use]
    pub fn sample_at(&self, p: DVec2, tick: u64, overlays: &[Overlay]) -> Weather {
        let mut w = self.sample(tick);
        self.lay(&mut w, p, tick, overlays);
        w
    }

    /// Lays the spells' `overlays` over `w` at `p`, timed by `tick` (a zone
    /// whose spells keep their own clock passes that clock's tick).
    pub fn lay(&self, w: &mut Weather, p: DVec2, tick: u64, overlays: &[Overlay]) {
        overlay(w, p, tick, overlays, |t| self.flash(t));
    }

    /// Only the rain at `tick`, as [`Self::sample`] has it.
    #[must_use]
    pub fn rain(&self, tick: u64) -> f64 {
        let (before, after, t) = self.states(tick);
        let p = Profile::mix(before.profile(), after.profile(), t);
        (p.rain * (0.7 + 0.6 * self.noise(tick, 1))).clamp(0.0, 1.0)
    }

    /// The ground at `tick`: wetness rises within a minute of heavy rain
    /// and dries over 15 minutes; puddles fill over 8 minutes of heavy rain
    /// and dry over 25 (ours). Integrated over the last
    /// [`GROUND_SECONDS`] on a [`GROUND_STEP`] grid fixed to the world
    /// tick, starting dry.
    #[must_use]
    pub fn ground(&self, tick: u64) -> Ground {
        const WET_FILL: f64 = 45.0;
        const WET_DRY: f64 = 900.0;
        const PUDDLE_FILL: f64 = 480.0;
        const PUDDLE_DRY: f64 = 1_500.0;
        let step = GROUND_STEP * TICK_HZ;
        let last = tick / step * step;
        let first = last.saturating_sub(GROUND_SECONDS * TICK_HZ);
        let mut g = Ground::default();
        let advance = |g: &mut Ground, rain: f64, dt: f64| {
            if rain > 0.02 {
                g.wet = (g.wet + rain * dt / WET_FILL).min(1.0);
                g.puddles = (g.puddles + rain * dt / PUDDLE_FILL).min(1.0);
            } else {
                g.wet = (g.wet - dt / WET_DRY).max(0.0);
                g.puddles = (g.puddles - dt / PUDDLE_DRY).max(0.0);
            }
        };
        let mut at = first;
        while at < last {
            advance(&mut g, self.rain(at), GROUND_STEP as f64);
            at += step;
        }
        let rest = (tick - last) as f64 / TICK_HZ as f64;
        if rest > 0.0 {
            advance(&mut g, self.rain(last), rest);
        }
        // A downpour wets the ground now, whatever the grid has.
        g.wet = g.wet.max(self.rain(tick).min(1.0) * 0.8);
        g
    }

    /// How far rain has raised ponds and streams at `tick`, m: the mean rain
    /// over the last [`RISE_SECONDS`] times [`MAX_RISE`], so never more than
    /// [`MAX_RISE`]. The trapezoid rule over a grid fixed to the world tick,
    /// with the window's ends exact, so it moves smoothly.
    #[must_use]
    pub fn rise(&self, tick: u64) -> f64 {
        let window = RISE_SECONDS * TICK_HZ;
        let step = GROUND_STEP * TICK_HZ;
        let start = tick.saturating_sub(window);
        if tick == start {
            return 0.0;
        }
        let mut points = vec![start];
        let mut g = (start / step + 1) * step;
        while g < tick {
            points.push(g);
            g += step;
        }
        points.push(tick);
        let mut area = 0.0;
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            area += (self.rain(a) + self.rain(b)) * 0.5 * (b - a) as f64;
        }
        (area / window as f64 * MAX_RISE).clamp(0.0, MAX_RISE)
    }

    /// How much faster currents run at `tick` for the risen water: 1, up to
    /// 1 + [`MAX_FLOW_GAIN`] at the full rise.
    #[must_use]
    pub fn flow_gain(&self, tick: u64) -> f64 {
        1.0 + MAX_FLOW_GAIN * self.rise(tick) / MAX_RISE
    }

    /// Seeded smooth noise in [0, 1] on `channel`: cubic between knots
    /// [`KNOT_SECONDS`] apart.
    fn noise(&self, tick: u64, channel: u64) -> f64 {
        let knot = tick / KNOT_TICKS;
        let t = (tick % KNOT_TICKS) as f64 / KNOT_TICKS as f64;
        let a = unit(hash(self.seed, knot, channel));
        let b = unit(hash(self.seed, knot + 1, channel));
        a + (b - a) * smooth(t)
    }

    /// A storm's lightning at `tick`: in each [`STRIKE_SECONDS`] window a
    /// strike at a seeded moment, or none, flashing twice and fading.
    #[must_use]
    pub fn flash(&self, tick: u64) -> f64 {
        let window = tick / STRIKE_TICKS;
        let h = hash(self.seed, window, 0x0F1A_5400);
        if unit(h) > 0.45 {
            return 0.0;
        }
        let at = window * STRIKE_TICKS + (h >> 40) % (STRIKE_TICKS * 2 / 3);
        if tick < at {
            return 0.0;
        }
        let age = (tick - at) as f64 / TICK_HZ as f64;
        let first = (-age / 0.08).exp();
        let second = if age > 0.18 {
            0.7 * (-(age - 0.18) / 0.15).exp()
        } else {
            0.0
        };
        first.max(second).min(1.0)
    }
}

/// Lays `overlays` over `w` at `p` and `tick`; `flash` is the schedule's
/// lightning for a storm under the cloud.
fn overlay(w: &mut Weather, p: DVec2, tick: u64, overlays: &[Overlay], flash: impl Fn(u64) -> f64) {
    for o in overlays {
        let k = o.weight(p, tick);
        if k <= 0.0 {
            continue;
        }
        let lift = |v: &mut f64, to: f64| *v += (to - *v).max(0.0) * k;
        match o.kind {
            OverlayKind::Rain => {
                lift(&mut w.rain, 0.7);
                lift(&mut w.cloud, 0.9);
            }
            OverlayKind::Sleet => {
                lift(&mut w.rain, 1.0);
                lift(&mut w.fog, 0.4);
                lift(&mut w.cloud, 1.0);
            }
            OverlayKind::Fog => lift(&mut w.fog, 1.0),
            OverlayKind::Cloud => lift(&mut w.cloud, 1.0),
            OverlayKind::Storm => {
                lift(&mut w.rain, 1.0);
                lift(&mut w.cloud, 1.0);
                let speed = w.wind_speed();
                if speed < STRONG_WIND + 1.0 {
                    let dir = w.wind.try_normalize().unwrap_or(DVec2::X);
                    w.wind = dir * (speed + (STRONG_WIND + 1.0 - speed) * k);
                }
                w.lightning = w.lightning.max(flash(tick) * k);
                if k >= 0.5 {
                    w.state = State::Storm;
                }
            }
        }
        if o.kind != OverlayKind::Storm && o.kind != OverlayKind::Fog && w.rain >= HEAVY_RAIN {
            w.state = match w.state {
                State::Storm => State::Storm,
                _ => State::Rain,
            };
        }
    }
    disperse(w);
}

/// SRD 5.2.1 Strong Wind disperses fog.
fn disperse(w: &mut Weather) {
    if w.strong_wind() {
        w.fog *= 0.2;
    }
}

/// Smoothstep on [0, 1].
fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// SplitMix64 over the seed, an index, and a channel.
fn hash(seed: u64, index: u64, channel: u64) -> u64 {
    let mut z = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(index.wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add(channel.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The top 53 bits of `h` as a fraction in [0, 1).
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
#[path = "weather_tests.rs"]
mod tests;
