//! The Gerstner terms of a `physics::water::WaveSet` as the shader reads
//! them, and an `f32` mirror of the shader's displacement for what must sit
//! on the drawn surface.
//!
//! The physics evaluates each term's angle as `k D·p0 − 2π n / P + φ` from an
//! integer phase counter `n` ([`physics::water::Phases`]). [`Swell::angles`]
//! sums the last two parts in `f64` and wraps them into `[−π, π)`, so the
//! shader only adds `k D·p0` in `f32`: the same terms, and no precision lost
//! to a long-running clock.

use std::f64::consts::TAU;

use glam::{DVec2, Vec2, Vec3};
use physics::water::{MAX_WAVES, WaveSet};

/// One Gerstner term, fixed when the set is built.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Term {
    /// Direction of travel in the (x, z) plane, unit.
    pub dir: [f32; 2],
    /// Wavenumber, rad/m.
    pub k: f32,
    /// Amplitude, m.
    pub amplitude: f32,
    /// The horizontal factor `Q / (k A N)`.
    pub q: f32,
    /// Phase offset, rad.
    pub phase: f64,
    /// Angular frequency as a whole multiple of `2π / period`.
    pub harmonic: u64,
}

/// A body's swell: up to [`MAX_WAVES`] terms on the physics clock. It is
/// `Copy`, so a frame's water can carry it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swell {
    pub terms: [Term; MAX_WAVES],
    pub count: usize,
    /// The loop period, ticks.
    pub period: u64,
    /// One tick, s.
    pub tick: f64,
}

impl Default for Swell {
    fn default() -> Self {
        Self {
            terms: [Term::default(); MAX_WAVES],
            count: 0,
            period: 1,
            tick: 1.0 / 120.0,
        }
    }
}

impl Swell {
    /// The terms of `set`, exactly as `physics::water` evaluates them.
    #[must_use]
    pub fn from_set(set: &WaveSet) -> Self {
        let mut swell = Self {
            period: set.period.max(1),
            tick: set.tick,
            count: set.waves.len().min(MAX_WAVES),
            ..Self::default()
        };
        for (term, wave) in swell.terms.iter_mut().zip(&set.waves) {
            *term = Term {
                dir: [wave.direction.x as f32, wave.direction.y as f32],
                k: wave.wavenumber() as f32,
                amplitude: wave.amplitude as f32,
                q: set.q(wave) as f32,
                phase: wave.phase,
                harmonic: wave.harmonic,
            };
        }
        swell
    }

    /// Whether the swell ever moves the surface.
    #[must_use]
    pub fn is_calm(&self) -> bool {
        self.terms[..self.count].iter().all(|t| t.amplitude == 0.0)
    }

    /// Term `i`'s angular frequency, rad/s.
    #[must_use]
    pub fn omega(&self, i: usize) -> f64 {
        TAU * self.terms[i].harmonic as f64 / (self.period as f64 * self.tick)
    }

    /// Each term's angle less `k D·p0` at `time` s, wrapped into `[−π, π)`.
    /// At a whole tick this is `φ − 2π n / P` with `n` the physics' phase
    /// counter; between ticks the counter advances smoothly.
    #[must_use]
    pub fn angles(&self, time: f64) -> [f32; MAX_WAVES] {
        let ticks = (time / self.tick).max(0.0);
        let whole = ticks.floor();
        let fraction = ticks - whole;
        let tick = whole as u64;
        self.angles_at(tick, fraction)
    }

    /// As [`Swell::angles`], at `tick` plus `fraction` of a tick.
    #[must_use]
    pub fn angles_at(&self, tick: u64, fraction: f64) -> [f32; MAX_WAVES] {
        let mut out = [0.0; MAX_WAVES];
        let folded = u128::from(tick % self.period);
        let period = u128::from(self.period);
        for (angle, term) in out.iter_mut().zip(&self.terms[..self.count]) {
            let count = (u128::from(term.harmonic) * folded % period) as f64;
            let steps = count + term.harmonic as f64 * fraction;
            let a = term.phase - TAU * steps / self.period as f64;
            *angle = (a + std::f64::consts::PI).rem_euclid(TAU) as f32 - std::f32::consts::PI;
        }
        out
    }

    /// The displacement of rest point `p0` at angles `angles`, each
    /// amplitude scaled by `gain` and the shallow-water factor over `depth`
    /// m: x, height, and z, m. The shader's `water_gerstner`, in `f32`.
    #[must_use]
    pub fn displacement(&self, angles: &[f32; MAX_WAVES], p0: Vec2, depth: f32, gain: f32) -> Vec3 {
        let mut d = Vec3::ZERO;
        for (term, angle) in self.terms[..self.count].iter().zip(angles) {
            let dir = Vec2::from(term.dir);
            let amp = term.amplitude * gain * shoaling(term.k, depth);
            let theta = term.k * dir.dot(p0) + angle;
            let (s, c) = theta.sin_cos();
            d.x += term.q * amp * dir.x * c;
            d.z += term.q * amp * dir.y * c;
            d.y += amp * s;
        }
        d
    }

    /// The velocity of the water at rest point `p0`: the time derivative of
    /// [`Swell::displacement`], m/s.
    #[must_use]
    pub fn velocity(&self, angles: &[f32; MAX_WAVES], p0: Vec2, depth: f32, gain: f32) -> Vec3 {
        let mut v = Vec3::ZERO;
        for (i, (term, angle)) in self.terms[..self.count].iter().zip(angles).enumerate() {
            let dir = Vec2::from(term.dir);
            let amp = term.amplitude * gain * shoaling(term.k, depth);
            let omega = self.omega(i) as f32;
            let theta = term.k * dir.dot(p0) + angle;
            let (s, c) = theta.sin_cos();
            v.x += term.q * amp * dir.x * omega * s;
            v.z += term.q * amp * dir.y * omega * s;
            v.y -= amp * omega * c;
        }
        v
    }

    /// The uniform rows the shader reads for this swell at `angles`.
    #[must_use]
    pub fn rows(&self, angles: &[f32; MAX_WAVES]) -> [[f32; 4]; 2 * MAX_WAVES] {
        let mut rows = [[0.0; 4]; 2 * MAX_WAVES];
        for (i, term) in self.terms[..self.count].iter().enumerate() {
            rows[i * 2] = [term.dir[0], term.dir[1], term.k, term.amplitude];
            rows[i * 2 + 1] = [term.q, angles[i], self.omega(i) as f32, 0.0];
        }
        rows
    }
}

/// The share of a wave's height left over water `depth` deep, `√tanh(k d)`:
/// one in deep water, zero on dry land. A visual factor; the physics
/// surface is deep water.
#[must_use]
pub fn shoaling(k: f32, depth: f32) -> f32 {
    // As the shader does: tanh is one in f32 past 10.
    (k * depth).clamp(0.0, 10.0).tanh().sqrt()
}

/// A swell like a wind sea: `count` terms around `wind` (radians about +Y,
/// the direction the waves travel toward; 0 is +z), the longest `longest`
/// m and each shorter by a fixed ratio, turned a little off the wind to
/// alternate sides, amplitude `height` × 1.1% of the wavelength, and
/// steepness 0.75, on a clock of 120 ticks a second that loops each hour.
///
/// # Panics
///
/// Never for finite, positive `longest`.
#[must_use]
pub fn wind_sea(wind: f32, longest: f32, height: f32, count: usize) -> WaveSet {
    let count = count.min(MAX_WAVES);
    let mut wavelength = f64::from(longest.max(0.5));
    let golden = |i: u32| (f64::from(i) * 0.618_034).fract();
    let waves = (0..count)
        .map(|i| {
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let turn = f64::from(wind) + side * (0.18 + 0.11 * i as f64) * golden(i as u32 + 1);
            let wave = physics::water::Wave::new(
                DVec2::new(turn.sin(), turn.cos()),
                wavelength * 0.011 * f64::from(height),
                wavelength,
                golden(i as u32 + 7) * TAU,
            );
            wavelength *= 0.71;
            wave
        })
        .collect();
    WaveSet::new(1.0 / 120.0, 120 * 3600, 0.75, waves).expect("a wind sea's terms are valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::water::{WaterBody, WaterId};

    /// At whole ticks the angles are the physics' own, so the `f32` mirror
    /// lands on `at_rest` and on `Surface::sample`'s height within a
    /// millimeter.
    #[test]
    fn the_mirror_matches_the_physics_surface() {
        for set in [
            WaveSet::seeded(
                7,
                1.0 / 120.0,
                120 * 60,
                6,
                DVec2::new(1.0, 0.3),
                9.0,
                0.12,
                0.5,
            )
            .unwrap(),
            WaveSet::seeded(
                11,
                1.0 / 120.0,
                120 * 600,
                8,
                DVec2::new(-0.4, 1.0),
                14.0,
                0.3,
                0.5,
            )
            .unwrap(),
        ] {
            let body = WaterBody::ocean(WaterId(0), 0.0).with_waves(set.clone());
            let swell = Swell::from_set(&set);
            for tick in [0u64, 1, 977, 120 * 600 + 3, 9_000_000] {
                let angles = swell.angles_at(tick, 0.0);
                let phases = set.phases(tick);
                for p in [Vec2::ZERO, Vec2::new(3.5, -7.25), Vec2::new(-40.0, 22.0)] {
                    let d = swell.displacement(&angles, p, f32::INFINITY, 1.0);
                    let exact = set.at_rest(p.as_dvec2(), &phases);
                    assert!((f64::from(d.y) - exact.height).abs() < 1e-3);
                    assert!((Vec2::new(d.x, d.z).as_dvec2() - exact.horizontal).length() < 1e-3);
                    let x = p + Vec2::new(d.x, d.z);
                    let sample = body
                        .surface()
                        .sample(f64::from(x.x), f64::from(x.y), tick)
                        .unwrap();
                    assert!(
                        (sample.height - f64::from(d.y)).abs() < 1e-3,
                        "tick {tick} at {p}: {} vs {}",
                        sample.height,
                        d.y
                    );
                }
            }
        }
    }

    /// Between ticks the angle moves on smoothly toward the next tick's.
    #[test]
    fn fractions_of_a_tick_interpolate() {
        let swell = Swell::from_set(&wind_sea(0.25, 22.0, 1.0, 8));
        let a = swell.angles_at(10, 0.0);
        let b = swell.angles_at(11, 0.0);
        let mid = swell.angles_at(10, 0.5);
        // The signed difference of two angles, in [−π, π).
        let turn = |x: f32, y: f32| {
            (x - y + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
        };
        for i in 0..swell.count {
            assert!((turn(mid[i], a[i]) - 0.5 * turn(b[i], a[i])).abs() < 1e-4);
        }
        // `angles` at a time is `angles_at` its tick and fraction.
        assert_eq!(swell.angles(10.5 / 120.0), mid);
    }

    /// The wind sea never folds: its summed Q stays the set's 0.75.
    #[test]
    fn the_wind_sea_never_loops() {
        let set = wind_sea(1.0, 30.0, 2.0, 8);
        let swell = Swell::from_set(&set);
        let sum: f32 = swell.terms[..swell.count]
            .iter()
            .map(|t| t.q * t.k * t.amplitude)
            .sum();
        assert!((sum - 0.75).abs() < 1e-4, "{sum}");
    }
}
