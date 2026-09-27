//! The circular restricted three-body problem for the Sun and the Earth–Moon
//! barycenter, in the rotating frame, with station-keeping about L1.
//!
//! Units are normalized: one length unit is the Sun–barycenter distance (1 AU),
//! one time unit is `1 / n` (about 58.1 days), and the combined mass is one.
//! The Sun sits at `(-mu, 0, 0)` and the Earth–Moon barycenter at `(1 - mu, 0, 0)`.
//! The x axis points from the Sun toward the Earth, z is the ecliptic normal.

use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Sun gravitational parameter, m^3/s^2 (IAU 2015 nominal).
pub const GM_SUN: f64 = 1.327_124_400_18e20;
/// Earth gravitational parameter, m^3/s^2 (WGS 84).
pub const GM_EARTH: f64 = 3.986_004_418e14;
/// Moon gravitational parameter, m^3/s^2 (DE440).
pub const GM_MOON: f64 = 4.904_869_5e12;
/// Astronomical unit, m. The CR3BP treats it as the circular separation.
pub const AU: f64 = 1.495_978_707e11;
/// Mean Earth radius, m.
pub const EARTH_RADIUS: f64 = 6.371_0e6;
/// Nominal solar radius, m.
pub const SUN_RADIUS: f64 = 6.957e8;
/// Mean lunar radius, m.
pub const MOON_RADIUS: f64 = 1.737_4e6;
/// Mean Earth–Moon distance, m.
pub const MOON_DISTANCE: f64 = 3.844e8;
/// Lunar inclination to the ecliptic, radians.
pub const MOON_INCLINATION: f64 = 5.145 * std::f64::consts::PI / 180.0;
/// Synodic month, s. In the Sun–Earth rotating frame the Moon returns to the
/// same phase after one synodic month, not one sidereal month.
pub const SYNODIC_MONTH: f64 = 29.530_589 * 86_400.0;
/// Speed of light, m/s.
pub const LIGHT_SPEED: f64 = 299_792_458.0;

/// Mass ratio of the Earth–Moon system to the Sun plus Earth–Moon system.
#[must_use]
pub fn mu() -> f64 {
    (GM_EARTH + GM_MOON) / (GM_SUN + GM_EARTH + GM_MOON)
}

/// Mean motion of the rotating frame, rad/s. One sidereal year is `2 pi / n`.
#[must_use]
pub fn mean_motion() -> f64 {
    ((GM_SUN + GM_EARTH + GM_MOON) / AU.powi(3)).sqrt()
}

/// Seconds in one normalized time unit.
#[must_use]
pub fn time_unit() -> f64 {
    1.0 / mean_motion()
}

/// A normalized rotating-frame state: position and velocity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub pos: DVec3,
    pub vel: DVec3,
}

/// Constants of the collinear point L1 and its linearized dynamics.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct L1 {
    pub mu: f64,
    /// Distance from the Earth–Moon barycenter to L1, normalized.
    pub gamma: f64,
    /// The L1 x coordinate, `1 - mu - gamma`.
    pub x: f64,
    /// Richardson's `c2`: the second-order Legendre coefficient at L1.
    pub c2: f64,
    /// Real in-plane eigenvalue: the unstable (and stable) rate.
    pub lambda: f64,
    /// In-plane oscillation frequency (planar Lyapunov).
    pub omega_p: f64,
    /// Out-of-plane oscillation frequency.
    pub omega_v: f64,
    /// In-plane amplitude ratio `Ay / Ax` of the linear oscillation.
    pub kappa: f64,
    /// Left eigenvector of the unstable mode over (x, y, vx, vy), normalized
    /// so its product with the right eigenvector is one.
    unstable_left: [f64; 4],
}

impl L1 {
    /// Solve the collinear quintic for L1 by Newton iteration.
    #[must_use]
    pub fn new(mu: f64) -> Self {
        // gamma^5 - (3 - mu) gamma^4 + (3 - 2 mu) gamma^3 - mu gamma^2 + 2 mu gamma - mu = 0
        let mut gamma = (mu / 3.0).cbrt();
        for _ in 0..50 {
            let f = gamma.powi(5) - (3.0 - mu) * gamma.powi(4) + (3.0 - 2.0 * mu) * gamma.powi(3)
                - mu * gamma.powi(2)
                + 2.0 * mu * gamma
                - mu;
            let df = 5.0 * gamma.powi(4) - 4.0 * (3.0 - mu) * gamma.powi(3)
                + 3.0 * (3.0 - 2.0 * mu) * gamma.powi(2)
                - 2.0 * mu * gamma
                + 2.0 * mu;
            let step = f / df;
            gamma -= step;
            if step.abs() < 1e-16 {
                break;
            }
        }
        let c2 = (mu + (1.0 - mu) * gamma.powi(3) / (1.0 - gamma).powi(3)) / gamma.powi(3);
        let root = (9.0 * c2 * c2 - 8.0 * c2).sqrt();
        let lambda = ((c2 - 2.0 + root) / 2.0).sqrt();
        let omega_p = ((2.0 - c2 + root) / 2.0).sqrt();
        let omega_v = c2.sqrt();
        let kappa = (omega_p * omega_p + 1.0 + 2.0 * c2) / (2.0 * omega_p);
        // Right unstable eigenvector over (x, y, vx, vy): (1, tau, lambda, lambda tau).
        let a = 1.0 + 2.0 * c2;
        let b = 1.0 - c2;
        let tau = (lambda * lambda - a) / (2.0 * lambda);
        // Left eigenvector from w^T A = lambda w^T with w_vx = 1.
        let w1 = a / lambda;
        let w4 = (w1 - lambda) / 2.0;
        let w2 = b * w4 / lambda;
        let w = [w1, w2, 1.0, w4];
        let dot = w[0] + w[1] * tau + w[2] * lambda + w[3] * lambda * tau;
        Self {
            mu,
            gamma,
            x: 1.0 - mu - gamma,
            c2,
            lambda,
            omega_p,
            omega_v,
            kappa,
            unstable_left: w.map(|v| v / dot),
        }
    }

    /// The Sun–Earth L1 point of this crate's constants.
    #[must_use]
    pub fn sun_earth() -> Self {
        Self::new(mu())
    }

    #[must_use]
    pub fn point(&self) -> DVec3 {
        DVec3::new(self.x, 0.0, 0.0)
    }

    /// The linear unstable-mode coordinate of a state relative to L1.
    /// A Lissajous orbit of the linearized system has zero here.
    #[must_use]
    pub fn unstable_component(&self, state: &State) -> f64 {
        let d = state.pos - self.point();
        let w = self.unstable_left;
        w[0] * d.x + w[1] * d.y + w[2] * state.vel.x + w[3] * state.vel.y
    }

    /// The smallest in-plane velocity change that cancels the unstable mode.
    #[must_use]
    pub fn cancel_unstable(&self, state: &State) -> DVec3 {
        let alpha = self.unstable_component(state);
        let w = self.unstable_left;
        let norm = w[2] * w[2] + w[3] * w[3];
        DVec3::new(-alpha * w[2] / norm, -alpha * w[3] / norm, 0.0)
    }

    /// A linear Lissajous state about L1 with in-plane amplitude `ax` and
    /// out-of-plane amplitude `az`, both normalized.
    #[must_use]
    pub fn lissajous(&self, ax: f64, az: f64, phase: f64, vertical_phase: f64) -> State {
        let (s, c) = phase.sin_cos();
        let (sv, cv) = vertical_phase.sin_cos();
        State {
            pos: DVec3::new(self.x - ax * c, self.kappa * ax * s, az * sv),
            vel: DVec3::new(
                ax * self.omega_p * s,
                self.kappa * ax * self.omega_p * c,
                az * self.omega_v * cv,
            ),
        }
    }
}

/// Acceleration of the full nonlinear CR3BP in the rotating frame.
#[must_use]
pub fn acceleration(mu: f64, state: &State) -> DVec3 {
    let p = state.pos;
    let r1 = (p - DVec3::new(-mu, 0.0, 0.0)).length();
    let r2 = (p - DVec3::new(1.0 - mu, 0.0, 0.0)).length();
    let k1 = (1.0 - mu) / r1.powi(3);
    let k2 = mu / r2.powi(3);
    DVec3::new(
        2.0 * state.vel.y + p.x - k1 * (p.x + mu) - k2 * (p.x - 1.0 + mu),
        -2.0 * state.vel.x + p.y - k1 * p.y - k2 * p.y,
        -k1 * p.z - k2 * p.z,
    )
}

/// The Jacobi integral `2 U - v^2`, conserved without control.
#[must_use]
pub fn jacobi(mu: f64, state: &State) -> f64 {
    let p = state.pos;
    let r1 = (p - DVec3::new(-mu, 0.0, 0.0)).length();
    let r2 = (p - DVec3::new(1.0 - mu, 0.0, 0.0)).length();
    let u = 0.5 * (p.x * p.x + p.y * p.y) + (1.0 - mu) / r1 + mu / r2;
    2.0 * u - state.vel.length_squared()
}

/// One classical Runge–Kutta step.
#[must_use]
pub fn rk4(mu: f64, s: &State, h: f64) -> State {
    let f = |s: &State| (s.vel, acceleration(mu, s));
    let add = |s: &State, k: (DVec3, DVec3), t: f64| State {
        pos: s.pos + k.0 * t,
        vel: s.vel + k.1 * t,
    };
    let k1 = f(s);
    let k2 = f(&add(s, k1, h / 2.0));
    let k3 = f(&add(s, k2, h / 2.0));
    let k4 = f(&add(s, k3, h));
    State {
        pos: s.pos + (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0) * (h / 6.0),
        vel: s.vel + (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1) * (h / 6.0),
    }
}

/// Maximum integration step, normalized: about 2.8 hours.
pub const MAX_STEP: f64 = 0.002;

/// A station on a controlled Lissajous orbit about Sun–Earth L1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationOrbit {
    pub l1: L1,
    pub state: State,
    /// Elapsed mission time, s.
    pub mission_seconds: f64,
    /// Seconds between station-keeping evaluations.
    pub keeping_interval: f64,
    /// Burns smaller than this, m/s, are skipped (thruster minimum impulse).
    pub deadband: f64,
    /// Cumulative station-keeping delta-v, m/s.
    pub keeping_dv: f64,
    /// Number of executed station-keeping burns.
    pub burns: u32,
    /// Mission time of the last executed burn, s.
    pub last_burn: Option<f64>,
    /// Velocity change of the last executed burn, rotating-frame axes, m/s.
    #[serde(default)]
    pub last_dv: DVec3,
    next_keeping: f64,
}

/// Displayed numbers for one orbital instant, in SI units.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct OrbitSnapshot {
    pub mission_days: f64,
    /// Station offset from L1 in the rotating frame, km (x sunward-negative).
    pub offset_km: [f64; 3],
    pub earth_distance_km: f64,
    pub sun_distance_km: f64,
    pub l1_earth_distance_km: f64,
    /// Sun–Earth–vehicle angle seen from Earth, degrees.
    pub sev_degrees: f64,
    /// One-way light time to Earth, s.
    pub light_seconds: f64,
    pub keeping_dv_m_s: f64,
    pub burns: u32,
    /// Speed relative to the rotating frame, m/s.
    pub frame_speed_m_s: f64,
    /// e-folding time of the uncontrolled instability, days.
    pub instability_days: f64,
    /// Linear in-plane period, days.
    pub lissajous_days: f64,
}

impl StationOrbit {
    /// A small Lissajous orbit (about 48,000 km along-track and 50,000 km
    /// out of the ecliptic) started from linear theory. Larger orbits such as
    /// DSCOVR's need nonlinear targeting to stay cheap; this controller only
    /// cancels the linear unstable mode, so it uses a modest amplitude.
    #[must_use]
    pub fn new() -> Self {
        let l1 = L1::sun_earth();
        let km = 1_000.0 / AU;
        let state = l1.lissajous(15_000.0 * km, 50_000.0 * km, 0.4, 1.3);
        Self {
            l1,
            state,
            mission_seconds: 0.0,
            keeping_interval: 7.0 * 86_400.0,
            deadband: 0.000_5,
            keeping_dv: 0.0,
            burns: 0,
            last_burn: None,
            last_dv: DVec3::ZERO,
            next_keeping: 0.0,
        }
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
            let until_keeping = ((self.next_keeping - self.mission_seconds) / unit).max(0.0);
            let h = remaining.min(MAX_STEP).min(if until_keeping > 0.0 {
                until_keeping
            } else {
                MAX_STEP
            });
            if h > 0.0 {
                self.state = rk4(self.l1.mu, &self.state, h);
                remaining -= h;
                self.mission_seconds += h * unit;
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

    /// Evaluate and execute one station-keeping burn.
    fn keep(&mut self) -> bool {
        let dv = self.l1.cancel_unstable(&self.state);
        let si = dv.length() * AU / time_unit();
        if si < self.deadband {
            return false;
        }
        self.state.vel += dv;
        self.last_dv = dv * (AU / time_unit());
        self.keeping_dv += si;
        self.burns += 1;
        self.last_burn = Some(self.mission_seconds);
        true
    }

    /// Unit direction from the station to the Sun, rotating frame.
    #[must_use]
    pub fn sun_direction(&self) -> DVec3 {
        (DVec3::new(-self.l1.mu, 0.0, 0.0) - self.state.pos).normalize()
    }

    /// Station-to-Earth vector in meters, rotating frame. The Earth sits at the
    /// Earth–Moon barycenter offset by the Moon's reflex motion.
    #[must_use]
    pub fn earth_vector(&self) -> DVec3 {
        let barycenter = (DVec3::new(1.0 - self.l1.mu, 0.0, 0.0) - self.state.pos) * AU;
        barycenter - self.moon_offset() * (GM_MOON / (GM_EARTH + GM_MOON))
    }

    /// Station-to-Moon vector in meters, rotating frame.
    #[must_use]
    pub fn moon_vector(&self) -> DVec3 {
        let barycenter = (DVec3::new(1.0 - self.l1.mu, 0.0, 0.0) - self.state.pos) * AU;
        barycenter + self.moon_offset() * (GM_EARTH / (GM_EARTH + GM_MOON))
    }

    /// Earth-to-Moon vector, m. Phase zero is full moon (Moon beyond Earth).
    fn moon_offset(&self) -> DVec3 {
        let theta = std::f64::consts::TAU * self.mission_seconds / SYNODIC_MONTH + 0.9;
        DVec3::new(
            theta.cos(),
            theta.sin() * MOON_INCLINATION.cos(),
            theta.sin() * MOON_INCLINATION.sin(),
        ) * MOON_DISTANCE
    }

    #[must_use]
    pub fn snapshot(&self) -> OrbitSnapshot {
        let offset = (self.state.pos - self.l1.point()) * AU / 1_000.0;
        let earth = self.earth_vector();
        let sun = (DVec3::new(-self.l1.mu, 0.0, 0.0) - self.state.pos) * AU;
        // Angle at the Earth between the Sun and the vehicle.
        let earth_to_sun = sun - earth;
        let earth_to_vehicle = -earth;
        let sev = earth_to_sun.angle_between(earth_to_vehicle).to_degrees();
        OrbitSnapshot {
            mission_days: self.mission_seconds / 86_400.0,
            offset_km: offset.to_array(),
            earth_distance_km: earth.length() / 1_000.0,
            sun_distance_km: sun.length() / 1_000.0,
            l1_earth_distance_km: self.l1.gamma * AU / 1_000.0,
            sev_degrees: sev,
            light_seconds: earth.length() / LIGHT_SPEED,
            keeping_dv_m_s: self.keeping_dv,
            burns: self.burns,
            frame_speed_m_s: self.state.vel.length() * AU / time_unit(),
            instability_days: time_unit() / self.l1.lambda / 86_400.0,
            lissajous_days: std::f64::consts::TAU / self.l1.omega_p * time_unit() / 86_400.0,
        }
    }
}

impl Default for StationOrbit {
    fn default() -> Self {
        Self::new()
    }
}
