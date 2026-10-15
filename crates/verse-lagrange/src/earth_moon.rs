//! The circular restricted three-body problem for the Earth and the Moon, in
//! the rotating frame, with a halo orbit about Earth–Moon L2 and daily
//! station-keeping. Lagrange 2's factory flies here.
//!
//! Units are normalized: one length unit is the Earth–Moon distance
//! (384,400 km), one time unit is `1 / n` (about 4.34 days), and the combined
//! mass is one. The Earth sits at `(-mu, 0, 0)` and the Moon at
//! `(1 - mu, 0, 0)`; x points from the Earth toward the Moon, z is the
//! normal of the Moon's orbit, and y completes the right-handed frame (the
//! Moon's direction of motion). L2 is on +x beyond the Moon.
//!
//! The gravitational parameters are JPL DE440's; see
//! `docs/spacecraft-design/astrodynamics.md` for the derived constants this
//! module's tests check.

use std::sync::OnceLock;

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::orbit::{
    self, Collinear, CollinearPoint, GM_MOON, LIGHT_SPEED, MOON_RADIUS, SYNODIC_MONTH, State,
};

/// Earth gravitational parameter, m^3/s^2 (JPL DE440: 398,600.435507 km^3/s^2).
/// The Sun–Earth model keeps WGS 84's [`orbit::GM_EARTH`].
pub const GM_EARTH_DE440: f64 = 3.986_004_355_07e14;
/// Earth–Moon distance, m: the CR3BP's circular separation (the Moon's
/// conventional mean distance).
pub const DISTANCE: f64 = 3.844e8;
/// Solar radiation pressure acceleration on the factory, m/s^2: about
/// 4.56e-6 N/m^2 at 1 AU, times 1.1 for arrays that mostly absorb, on
/// 20,000 m^2 of arrays held face-on, over a 2,000 t factory. It pushes away
/// from the Sun, which turns once per synodic month in this frame.
pub const SRP_ACCEL: f64 = 5.0e-8;
/// Sun direction at mission time zero, radians from +x toward +y. Near zero
/// the Sun shines from beyond L2 onto the Moon's far side.
pub const SUN_PHASE0: f64 = 0.5;
/// One-sigma navigation knowledge error: position, m.
pub const NAV_POSITION_SIGMA: f64 = 1_000.0;
/// One-sigma navigation knowledge error: velocity, m/s.
pub const NAV_VELOCITY_SIGMA: f64 = 0.001;
/// One-sigma burn magnitude error, as a fraction.
pub const BURN_MAGNITUDE_SIGMA: f64 = 0.02;
/// How far downstream a station-keeping burn targets the reference halo, s.
pub const TARGET_HORIZON: f64 = 2.0 * 86_400.0;

/// Mass ratio of the Moon to the Earth plus the Moon (DE440).
#[must_use]
pub fn mu() -> f64 {
    GM_MOON / (GM_EARTH_DE440 + GM_MOON)
}

/// Mean motion of the rotating frame, rad/s.
#[must_use]
pub fn mean_motion() -> f64 {
    ((GM_EARTH_DE440 + GM_MOON) / DISTANCE.powi(3)).sqrt()
}

/// Seconds in one normalized time unit.
#[must_use]
pub fn time_unit() -> f64 {
    1.0 / mean_motion()
}

/// Meters per second in one normalized velocity unit.
#[must_use]
pub fn velocity_unit() -> f64 {
    DISTANCE * mean_motion()
}

/// Meters per second squared in one normalized acceleration unit.
#[must_use]
pub fn acceleration_unit() -> f64 {
    DISTANCE * mean_motion() * mean_motion()
}

/// Earth–Moon L2.
#[must_use]
pub fn l2() -> Collinear {
    Collinear::new(mu(), CollinearPoint::L2)
}

/// Earth–Moon L1.
#[must_use]
pub fn l1() -> Collinear {
    Collinear::new(mu(), CollinearPoint::L1)
}

/// Whether the line from the ship to the Earth's center clears the Moon's
/// disc: the Earth is in view (and in radio contact).
#[must_use]
pub fn earth_visible(mu: f64, state: &State) -> bool {
    let earth = DVec3::new(-mu, 0.0, 0.0);
    let moon = DVec3::new(1.0 - mu, 0.0, 0.0);
    let d = earth - state.pos;
    let t = ((moon - state.pos).dot(d) / d.length_squared()).clamp(0.0, 1.0);
    let closest = state.pos + d * t;
    closest.distance(moon) * DISTANCE > MOON_RADIUS
}

/// A 6x6 state transition matrix over (x, y, z, vx, vy, vz).
pub type Stm = [[f64; 6]; 6];

fn identity() -> Stm {
    let mut m = [[0.0; 6]; 6];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

/// Second derivatives of the effective potential.
fn hessian(mu: f64, p: DVec3) -> [[f64; 3]; 3] {
    let d1 = p - DVec3::new(-mu, 0.0, 0.0);
    let d2 = p - DVec3::new(1.0 - mu, 0.0, 0.0);
    let (r1, r2) = (d1.length(), d2.length());
    let (a, b) = ((1.0 - mu) / r1.powi(3), mu / r2.powi(3));
    let (a5, b5) = (3.0 * (1.0 - mu) / r1.powi(5), 3.0 * mu / r2.powi(5));
    let d1 = d1.to_array();
    let d2 = d2.to_array();
    let mut h = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let delta = if i == j { 1.0 } else { 0.0 };
            h[i][j] = -(a + b) * delta + a5 * d1[i] * d1[j] + b5 * d2[i] * d2[j];
        }
        if i < 2 {
            h[i][i] += 1.0;
        }
    }
    h
}

fn stm_rate(mu: f64, pos: DVec3, phi: &Stm) -> Stm {
    let h = hessian(mu, pos);
    let mut out = [[0.0; 6]; 6];
    for j in 0..6 {
        for i in 0..3 {
            out[i][j] = phi[i + 3][j];
            let mut v = 0.0;
            for (k, hk) in h[i].iter().enumerate() {
                v += hk * phi[k][j];
            }
            v += match i {
                0 => 2.0 * phi[4][j],
                1 => -2.0 * phi[3][j],
                _ => 0.0,
            };
            out[i + 3][j] = v;
        }
    }
    out
}

fn axpy(phi: &Stm, k: &Stm, t: f64) -> Stm {
    let mut out = *phi;
    for i in 0..6 {
        for j in 0..6 {
            out[i][j] += k[i][j] * t;
        }
    }
    out
}

/// One RK4 step of the state and its state transition matrix.
#[must_use]
pub fn rk4_stm(mu: f64, s: &State, phi: &Stm, h: f64) -> (State, Stm) {
    let f = |s: &State| (s.vel, orbit::acceleration(mu, s));
    let add = |s: &State, k: (DVec3, DVec3), t: f64| State {
        pos: s.pos + k.0 * t,
        vel: s.vel + k.1 * t,
    };
    let k1 = f(s);
    let p1 = stm_rate(mu, s.pos, phi);
    let s2 = add(s, k1, h / 2.0);
    let k2 = f(&s2);
    let p2 = stm_rate(mu, s2.pos, &axpy(phi, &p1, h / 2.0));
    let s3 = add(s, k2, h / 2.0);
    let k3 = f(&s3);
    let p3 = stm_rate(mu, s3.pos, &axpy(phi, &p2, h / 2.0));
    let s4 = add(s, k3, h);
    let k4 = f(&s4);
    let p4 = stm_rate(mu, s4.pos, &axpy(phi, &p3, h));
    let state = State {
        pos: s.pos + (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0) * (h / 6.0),
        vel: s.vel + (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1) * (h / 6.0),
    };
    let mut out = *phi;
    for i in 0..6 {
        for j in 0..6 {
            out[i][j] += (p1[i][j] + 2.0 * p2[i][j] + 2.0 * p3[i][j] + p4[i][j]) * (h / 6.0);
        }
    }
    (state, out)
}

/// Integration step for halo work, normalized: about 12.5 minutes.
const HALO_STEP: f64 = 0.002;

/// Propagate a state and its STM by `t`, normalized.
#[must_use]
pub fn propagate_stm(mu: f64, s: &State, t: f64) -> (State, Stm) {
    let mut state = *s;
    let mut phi = identity();
    let mut left = t;
    while left > 0.0 {
        let h = left.min(HALO_STEP);
        (state, phi) = rk4_stm(mu, &state, &phi, h);
        left -= h;
    }
    (state, phi)
}

/// Propagate a state by `t`, normalized, in the pure CR3BP.
#[must_use]
pub fn propagate(mu: f64, s: &State, t: f64) -> State {
    let mut state = *s;
    let mut left = t;
    while left > 0.0 {
        let h = left.min(HALO_STEP);
        state = orbit::rk4(mu, &state, h);
        left -= h;
    }
    state
}

/// Integrate from a state on the x–z plane to its next crossing of it.
/// Returns the elapsed time, the state there, and the STM.
fn half_revolution(mu: f64, s: &State) -> Option<(f64, State, Stm)> {
    let mut state = *s;
    let mut phi = identity();
    let mut t = 0.0;
    loop {
        let (next, next_phi) = rk4_stm(mu, &state, &phi, HALO_STEP);
        if t > 0.1 && next.pos.y * state.pos.y <= 0.0 && state.pos.y != 0.0 {
            // Refine the step that crosses y = 0 by Newton on its length.
            let mut h = HALO_STEP * state.pos.y / (state.pos.y - next.pos.y);
            for _ in 0..8 {
                let (try_state, _) = rk4_stm(mu, &state, &phi, h);
                h -= try_state.pos.y / try_state.vel.y;
            }
            let (end, end_phi) = rk4_stm(mu, &state, &phi, h);
            return Some((t + h, end, end_phi));
        }
        state = next;
        phi = next_phi;
        t += HALO_STEP;
        if t > 20.0 || !state.pos.is_finite() {
            return None;
        }
    }
}

/// A periodic halo orbit about Earth–Moon L2, symmetric about the x–z plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Halo {
    pub mu: f64,
    /// The state at the x–z plane crossing nearest the Moon, normalized:
    /// `(x0, 0, z0)` moving with `(0, vy0, 0)`.
    pub initial: State,
    /// Period, normalized.
    pub period: f64,
}

impl Halo {
    /// Differential correction: hold `z0` and adjust `x0` and `vy0` until the
    /// next x–z crossing has `vx = vz = 0`. Returns `None` if Newton's
    /// iteration does not converge.
    #[must_use]
    pub fn correct(mu: f64, x0: f64, z0: f64, vy0: f64) -> Option<Self> {
        let (mut x0, mut vy0) = (x0, vy0);
        for _ in 0..40 {
            let start = State {
                pos: DVec3::new(x0, 0.0, z0),
                vel: DVec3::new(0.0, vy0, 0.0),
            };
            let (half, end, phi) = half_revolution(mu, &start)?;
            let (vx, vz) = (end.vel.x, end.vel.z);
            if vx.abs() < 1e-12 && vz.abs() < 1e-12 {
                return Some(Self {
                    mu,
                    initial: start,
                    period: 2.0 * half,
                });
            }
            let a = orbit::acceleration(mu, &end);
            let vy = end.vel.y;
            // Rows vx and vz, columns x0 and vy0, with the crossing time free.
            let m00 = phi[3][0] - a.x / vy * phi[1][0];
            let m01 = phi[3][4] - a.x / vy * phi[1][4];
            let m10 = phi[5][0] - a.z / vy * phi[1][0];
            let m11 = phi[5][4] - a.z / vy * phi[1][4];
            let det = m00 * m11 - m01 * m10;
            if det.abs() < 1e-300 {
                return None;
            }
            let dx = (-vx * m11 + vz * m01) / det;
            let dv = (-vz * m00 + vx * m10) / det;
            // Damp large steps so the iteration stays on this family.
            let scale = (0.01 / dx.abs().max(dv.abs())).min(1.0);
            x0 += dx * scale;
            vy0 += dv * scale;
        }
        None
    }

    /// The Lagrange 2 factory's halo: a southern Earth–Moon L2 halo with a
    /// z amplitude near 30,000 km, found once by continuation in `z0` from
    /// the planar Lyapunov family's vertical bifurcation and cached.
    #[must_use]
    pub fn earth_moon() -> &'static Self {
        static HALO: OnceLock<Halo> = OnceLock::new();
        HALO.get_or_init(|| Self::family(mu(), FACTORY_Z0).expect("the L2 halo converges"))
    }

    /// A member of the L2 halo family with `z0` at the Moon-side crossing.
    /// Continuation in `z0` from a small halo, each member seeding the next.
    #[must_use]
    pub fn family(mu: f64, z0: f64) -> Option<Self> {
        let (mut x0, mut vy0) = HALO_SEED;
        let steps = 12;
        let mut halo = None;
        for i in 1..=steps {
            let z = HALO_SEED_Z0 + (z0 - HALO_SEED_Z0) * f64::from(i) / f64::from(steps);
            let h = Self::correct(mu, x0, z, vy0)?;
            x0 = h.initial.pos.x;
            vy0 = h.initial.vel.y;
            halo = Some(h);
        }
        halo
    }

    /// The state at normalized time `t` along the orbit, from the cached
    /// samples of [`Self::earth_moon`] when this is that halo.
    #[must_use]
    pub fn state_at(&self, t: f64) -> State {
        let t = t.rem_euclid(self.period);
        if std::ptr::eq(self, Self::earth_moon()) || self == Self::earth_moon() {
            let table = reference_table();
            let i = ((t / TABLE_STEP) as usize).min(table.len() - 1);
            return propagate(self.mu, &table[i], t - i as f64 * TABLE_STEP);
        }
        propagate(self.mu, &self.initial, t)
    }

    /// Extents of the orbit about L2 in x, y, z, km (half widths).
    #[must_use]
    pub fn amplitudes_km(&self) -> [f64; 3] {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        let mut s = self.initial;
        let n = (self.period / HALO_STEP).ceil() as usize;
        for _ in 0..n {
            lo = lo.min(s.pos);
            hi = hi.max(s.pos);
            s = orbit::rk4(self.mu, &s, self.period / n as f64);
        }
        ((hi - lo) * 0.5 * DISTANCE / 1_000.0).to_array()
    }
}

/// The factory halo's `z0` at the Moon-side crossing, normalized: southern
/// (negative z). Chosen so the period is near 14.8 days.
const FACTORY_Z0: f64 = -0.03;
/// A small southern halo near the bifurcation, the continuation's start:
/// `(x0, vy0)` at `z0 = HALO_SEED_Z0`.
const HALO_SEED_Z0: f64 = -0.005;
const HALO_SEED: (f64, f64) = (1.120_2, 0.176_6);

/// Reference samples spacing, normalized.
const TABLE_STEP: f64 = 0.01;

fn reference_table() -> &'static [State] {
    static TABLE: OnceLock<Vec<State>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let halo = Halo::earth_moon();
        let n = (halo.period / TABLE_STEP).ceil() as usize + 1;
        let mut out = Vec::with_capacity(n);
        let mut s = halo.initial;
        for _ in 0..n {
            out.push(s);
            s = propagate(halo.mu, &s, TABLE_STEP);
        }
        out
    })
}

/// splitmix64: a small deterministic generator for navigation and burn
/// errors, so a recorded mission replays exactly.
fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A standard normal sample for draw `index` of evaluation `n`.
fn gaussian(seed: u64, n: u64, index: u64) -> f64 {
    let a = splitmix(seed ^ n.wrapping_mul(0x1000_0000_01B3) ^ (index << 1));
    let b = splitmix(a ^ 0xA5A5_A5A5);
    let u1 = ((a >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let u2 = (b >> 11) as f64 / (1u64 << 53) as f64;
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// A station on the factory halo about Earth–Moon L2, kept there by daily
/// targeting burns against solar radiation pressure and its own navigation
/// and burn errors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HaloOrbit {
    pub l2: Collinear,
    pub halo: Halo,
    pub state: State,
    /// Elapsed mission time, s.
    pub mission_seconds: f64,
    /// Seconds between station-keeping evaluations: about daily, since
    /// errors grow e-fold in about two days here.
    pub keeping_interval: f64,
    /// Burns smaller than this, m/s, are skipped.
    pub deadband: f64,
    /// Solar radiation pressure acceleration, m/s^2.
    pub srp_accel: f64,
    /// Seed for navigation and burn errors.
    pub seed: u64,
    /// Cumulative station-keeping delta-v, m/s.
    pub keeping_dv: f64,
    /// Number of executed station-keeping burns.
    pub burns: u32,
    /// Station-keeping evaluations made, burned or not.
    pub evaluations: u64,
    /// Mission time of the last executed burn, s.
    pub last_burn: Option<f64>,
    /// Velocity change of the last executed burn, rotating-frame axes, m/s.
    pub last_dv: DVec3,
    next_keeping: f64,
}

/// Displayed numbers for one orbital instant, in SI units.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct HaloSnapshot {
    pub mission_days: f64,
    /// Station offset from L2 in the rotating frame, km (x away from the Moon).
    pub offset_km: [f64; 3],
    pub moon_distance_km: f64,
    pub earth_distance_km: f64,
    /// One-way light time to the Earth, s.
    pub light_seconds: f64,
    /// Whether the Moon leaves the Earth in view.
    pub earth_visible: bool,
    pub keeping_dv_m_s: f64,
    pub burns: u32,
    /// e-folding time of the uncontrolled instability, days.
    pub instability_days: f64,
    pub halo_period_days: f64,
    /// Distance from the reference halo at this time, km.
    pub deviation_km: f64,
    /// Speed relative to the rotating frame, m/s.
    pub frame_speed_m_s: f64,
}

impl HaloOrbit {
    #[must_use]
    pub fn new() -> Self {
        let halo = *Halo::earth_moon();
        Self {
            l2: l2(),
            halo,
            state: halo.initial,
            mission_seconds: 0.0,
            keeping_interval: 86_400.0,
            deadband: 0.002,
            srp_accel: SRP_ACCEL,
            seed: 0x4c32,
            keeping_dv: 0.0,
            burns: 0,
            evaluations: 0,
            last_burn: None,
            last_dv: DVec3::ZERO,
            next_keeping: 86_400.0,
        }
    }

    /// Unit direction from the station to the Sun, rotating frame. The Sun
    /// turns once per synodic month, backward (toward -y from +x); the
    /// Moon's 5° inclination to the ecliptic is ignored.
    #[must_use]
    pub fn sun_direction(&self) -> DVec3 {
        sun_direction_at(self.mission_seconds)
    }

    /// Station-to-Earth vector, m, rotating frame.
    #[must_use]
    pub fn earth_vector(&self) -> DVec3 {
        (DVec3::new(-self.l2.mu, 0.0, 0.0) - self.state.pos) * DISTANCE
    }

    /// Station-to-Moon vector, m, rotating frame.
    #[must_use]
    pub fn moon_vector(&self) -> DVec3 {
        (DVec3::new(1.0 - self.l2.mu, 0.0, 0.0) - self.state.pos) * DISTANCE
    }

    /// The reference halo's state now.
    #[must_use]
    pub fn reference(&self) -> State {
        self.halo.state_at(self.mission_seconds / time_unit())
    }

    /// Add a rotating-frame velocity change, m/s, and stop station-keeping:
    /// the ship's test burn departing the halo.
    pub fn apply_burn(&mut self, dv_m_s: DVec3) {
        self.state.vel += dv_m_s / velocity_unit();
        self.keeping_interval = f64::INFINITY;
        self.next_keeping = f64::INFINITY;
    }

    fn step(&mut self, h: f64) {
        let unit = time_unit();
        let t0 = self.mission_seconds;
        let mu = self.l2.mu;
        let srp = self.srp_accel / acceleration_unit();
        let f = |s: &State, dt: f64| {
            let push = -sun_direction_at(t0 + dt * unit) * srp;
            (s.vel, orbit::acceleration(mu, s) + push)
        };
        let add = |s: &State, k: (DVec3, DVec3), t: f64| State {
            pos: s.pos + k.0 * t,
            vel: s.vel + k.1 * t,
        };
        let s = self.state;
        let k1 = f(&s, 0.0);
        let k2 = f(&add(&s, k1, h / 2.0), h / 2.0);
        let k3 = f(&add(&s, k2, h / 2.0), h / 2.0);
        let k4 = f(&add(&s, k3, h), h);
        self.state = State {
            pos: s.pos + (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0) * (h / 6.0),
            vel: s.vel + (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1) * (h / 6.0),
        };
        self.mission_seconds += h * unit;
    }

    /// Advance by `seconds` of mission time. Returns the number of burns.
    pub fn advance(&mut self, seconds: f64) -> u32 {
        if !seconds.is_finite() || seconds <= 0.0 {
            return 0;
        }
        let unit = time_unit();
        let mut remaining = seconds.min(365.25 * 86_400.0) / unit;
        let mut burns = 0;
        while remaining > 0.0 {
            let until = ((self.next_keeping - self.mission_seconds) / unit).max(0.0);
            let h = remaining
                .min(HALO_STEP)
                .min(if until > 0.0 { until } else { HALO_STEP });
            if h > 0.0 {
                self.step(h);
                remaining -= h;
            }
            if self.mission_seconds + 1e-6 >= self.next_keeping {
                self.next_keeping = self.mission_seconds + self.keeping_interval;
                if self.keep() {
                    burns += 1;
                }
            }
        }
        burns
    }

    /// One station-keeping evaluation: estimate the state with navigation
    /// error, predict it [`TARGET_HORIZON`] ahead in the CR3BP, and burn the
    /// velocity change that puts it on the reference halo's position there.
    fn keep(&mut self) -> bool {
        let n = self.evaluations;
        self.evaluations += 1;
        let g = |i| gaussian(self.seed, n, i);
        let nav_p = DVec3::new(g(0), g(1), g(2)) * (NAV_POSITION_SIGMA / DISTANCE);
        let nav_v = DVec3::new(g(3), g(4), g(5)) * (NAV_VELOCITY_SIGMA / velocity_unit());
        let estimate = State {
            pos: self.state.pos + nav_p,
            vel: self.state.vel + nav_v,
        };
        let unit = time_unit();
        let horizon = TARGET_HORIZON / unit;
        let (predicted, phi) = propagate_stm(self.l2.mu, &estimate, horizon);
        let target = self
            .halo
            .state_at((self.mission_seconds + TARGET_HORIZON) / unit)
            .pos;
        let miss = (target - predicted.pos).to_array();
        // Solve phi_rv dv = miss.
        let m = glam::DMat3::from_cols(
            DVec3::new(phi[0][3], phi[1][3], phi[2][3]),
            DVec3::new(phi[0][4], phi[1][4], phi[2][4]),
            DVec3::new(phi[0][5], phi[1][5], phi[2][5]),
        );
        let dv = m.inverse() * DVec3::from_array(miss);
        let si = dv.length() * velocity_unit();
        if !si.is_finite() || si < self.deadband {
            return false;
        }
        let executed = dv * (1.0 + BURN_MAGNITUDE_SIGMA * g(6));
        self.state.vel += executed;
        self.last_dv = executed * velocity_unit();
        self.keeping_dv += executed.length() * velocity_unit();
        self.burns += 1;
        self.last_burn = Some(self.mission_seconds);
        true
    }

    #[must_use]
    pub fn snapshot(&self) -> HaloSnapshot {
        let offset = (self.state.pos - self.l2.point()) * DISTANCE / 1_000.0;
        let earth = self.earth_vector();
        let unit = time_unit();
        HaloSnapshot {
            mission_days: self.mission_seconds / 86_400.0,
            offset_km: offset.to_array(),
            moon_distance_km: self.moon_vector().length() / 1_000.0,
            earth_distance_km: earth.length() / 1_000.0,
            light_seconds: earth.length() / LIGHT_SPEED,
            earth_visible: earth_visible(self.l2.mu, &self.state),
            keeping_dv_m_s: self.keeping_dv,
            burns: self.burns,
            instability_days: unit / self.l2.lambda / 86_400.0,
            halo_period_days: self.halo.period * unit / 86_400.0,
            deviation_km: self.reference().pos.distance(self.state.pos) * DISTANCE / 1_000.0,
            frame_speed_m_s: self.state.vel.length() * velocity_unit(),
        }
    }
}

impl Default for HaloOrbit {
    fn default() -> Self {
        Self::new()
    }
}

/// Unit direction to the Sun at mission time `seconds`, rotating frame.
#[must_use]
pub fn sun_direction_at(seconds: f64) -> DVec3 {
    let theta = SUN_PHASE0 - std::f64::consts::TAU * seconds / SYNODIC_MONTH;
    DVec3::new(theta.cos(), theta.sin(), 0.0)
}
