//! The gameplay surface: a sum of Gerstner waves on a deterministic clock.
//!
//! Each wave moves surface particles on circles (Fournier and Reeves, "A
//! Simple Model of Ocean Waves", SIGGRAPH 1986), in the parameterization of
//! Finch ("Effective Water Simulation from Physical Models", *GPU Gems*,
//! chapter 1, 2004): a particle whose rest position is `p0` sits at
//!
//! ```text
//! x = p0 + Σ q_i A_i D_i cos θ_i,   y = Σ A_i sin θ_i,
//! θ_i = k_i D_i · p0 − ω_i t + φ_i,
//! ```
//!
//! with `q_i = Q / (k_i A_i N)`, so the authored steepness `Q` in [0, 1]
//! never folds the surface into loops. Angular frequencies come from the
//! deep-water dispersion `ω² = g k` and are then snapped to whole multiples
//! of `2π / T` for a period `T` of whole ticks, as Tessendorf suggests for
//! looping animation ("Simulating Ocean Water", SIGGRAPH 2001 course
//! notes). Each wave's phase is therefore an integer count of `2π / P`
//! steps, `(n_i · tick) mod P`, so any tick is reachable in O(1) and
//! stepping the counter one tick at a time reaches exactly the same bits.

use std::f64::consts::TAU;

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// The most Gerstner terms a surface carries.
pub const MAX_WAVES: usize = 8;

/// Standard gravity for the dispersion relation, m/s².
pub const G: f64 = 9.81;

/// One Gerstner term.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wave {
    /// Direction of travel in the (x, z) plane, unit.
    pub direction: DVec2,
    /// Amplitude, m.
    pub amplitude: f64,
    /// Wavelength, m.
    pub wavelength: f64,
    /// Phase offset, rad.
    pub phase: f64,
    /// Angular frequency as a whole multiple of `2π / T`; set by
    /// [`WaveSet::new`].
    pub harmonic: u64,
}

impl Wave {
    /// A wave traveling along `direction` (normalized here).
    #[must_use]
    pub fn new(direction: DVec2, amplitude: f64, wavelength: f64, phase: f64) -> Self {
        Self {
            direction: direction.normalize_or(DVec2::X),
            amplitude,
            wavelength,
            phase,
            harmonic: 0,
        }
    }

    /// Wavenumber `k = 2π / λ`, 1/m.
    #[must_use]
    pub fn wavenumber(&self) -> f64 {
        TAU / self.wavelength
    }
}

/// Up to [`MAX_WAVES`] Gerstner terms on a clock of whole ticks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveSet {
    pub waves: Vec<Wave>,
    /// Steepness `Q` in [0, 1]: 0 is sinusoidal, 1 the sharpest crest
    /// without loops.
    pub steepness: f64,
    /// The loop period, ticks. Every wave's harmonic divides into it.
    pub period: u64,
    /// One tick, s; the world's step.
    pub tick: f64,
    /// Seed of the spectrum the terms were drawn from, if any. The
    /// spectral phase (W4) synthesizes its tile from it.
    pub seed: Option<u64>,
}

impl Default for WaveSet {
    fn default() -> Self {
        Self::calm()
    }
}

/// Integer phase counters, one per wave, in steps of `2π / period`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phases {
    count: [u64; MAX_WAVES],
}

/// What the waves do to the particle at a rest position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Displacement {
    /// Horizontal offset from the rest position, m.
    pub horizontal: DVec2,
    /// Height above the level, m.
    pub height: f64,
    /// Unit normal, pointing up.
    pub normal: DVec3,
    /// The particle's velocity, m/s.
    pub velocity: DVec3,
}

impl WaveSet {
    /// No waves.
    #[must_use]
    pub fn calm() -> Self {
        Self {
            waves: Vec::new(),
            steepness: 0.0,
            period: 1,
            tick: 1.0 / 120.0,
            seed: None,
        }
    }

    /// Waves on a clock of `tick` seconds that loops every `period` ticks,
    /// each frequency snapped to the nearest nonzero multiple of `2π / T`.
    ///
    /// # Errors
    ///
    /// Returns a message for more than [`MAX_WAVES`] terms, a zero period,
    /// a tick that is not positive, a wavelength that is not positive, or a
    /// steepness outside [0, 1].
    pub fn new(tick: f64, period: u64, steepness: f64, waves: Vec<Wave>) -> Result<Self, String> {
        if waves.len() > MAX_WAVES {
            return Err(format!(
                "{} Gerstner terms exceed the limit of {MAX_WAVES}",
                waves.len()
            ));
        }
        if period == 0 || !(tick > 0.0 && tick.is_finite()) {
            return Err("a wave clock needs a positive tick and period".into());
        }
        if !(0.0..=1.0).contains(&steepness) {
            return Err(format!("steepness {steepness} is outside [0, 1]"));
        }
        let span = tick * period as f64;
        let mut set = Self {
            waves,
            steepness,
            period,
            tick,
            seed: None,
        };
        for wave in &mut set.waves {
            if !(wave.wavelength > 0.0 && wave.wavelength.is_finite()) {
                return Err(format!("wavelength {} is not positive", wave.wavelength));
            }
            let omega = (G * wave.wavenumber()).sqrt();
            wave.harmonic = ((omega * span / TAU).round() as u64).max(1);
        }
        Ok(set)
    }

    /// `count` terms drawn from `seed` around a wind blowing along `wind`:
    /// wavelengths from half to twice `wavelength`, directions within 45°
    /// of the wind, and amplitudes in proportion to wavelength so every
    /// term has the same slope, `amplitude` at `wavelength`.
    ///
    /// # Errors
    ///
    /// As [`WaveSet::new`].
    #[allow(clippy::too_many_arguments)]
    pub fn seeded(
        seed: u64,
        tick: f64,
        period: u64,
        count: usize,
        wind: DVec2,
        wavelength: f64,
        amplitude: f64,
        steepness: f64,
    ) -> Result<Self, String> {
        let mut state = seed;
        let mut next = || {
            // SplitMix64 (Steele, Lea, and Flood, OOPSLA 2014).
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        };
        let base = wind.normalize_or(DVec2::X);
        let waves = (0..count)
            .map(|_| {
                let scale = 2f64.powf(next() * 2.0 - 1.0);
                let turn = (next() - 0.5) * std::f64::consts::FRAC_PI_2;
                let (s, c) = turn.sin_cos();
                let direction = DVec2::new(base.x * c - base.y * s, base.x * s + base.y * c);
                Wave::new(
                    direction,
                    amplitude * scale,
                    wavelength * scale,
                    next() * TAU,
                )
            })
            .collect();
        let mut set = Self::new(tick, period, steepness, waves)?;
        set.seed = Some(seed);
        Ok(set)
    }

    /// Whether the surface ever moves.
    #[must_use]
    pub fn is_calm(&self) -> bool {
        self.waves.iter().all(|w| w.amplitude == 0.0)
    }

    /// Wave `i`'s angular frequency, rad/s.
    #[must_use]
    pub fn omega(&self, i: usize) -> f64 {
        TAU * self.waves[i].harmonic as f64 / (self.period as f64 * self.tick)
    }

    /// Every wave's phase counter at `tick`, in O(1).
    #[must_use]
    pub fn phases(&self, tick: u64) -> Phases {
        let mut count = [0; MAX_WAVES];
        let folded = u128::from(tick % self.period);
        for (c, wave) in count.iter_mut().zip(&self.waves) {
            *c = (u128::from(wave.harmonic) * folded % u128::from(self.period)) as u64;
        }
        Phases { count }
    }

    /// Advance phase counters by one tick.
    pub fn advance(&self, phases: &mut Phases) {
        for (c, wave) in phases.count.iter_mut().zip(&self.waves) {
            *c = ((u128::from(*c) + u128::from(wave.harmonic)) % u128::from(self.period)) as u64;
        }
    }

    fn q(&self, wave: &Wave) -> f64 {
        let ka = wave.wavenumber() * wave.amplitude;
        if ka <= 0.0 {
            0.0
        } else {
            self.steepness / (ka * self.waves.len() as f64)
        }
    }

    /// Horizontal displacement only, for the fixed-point inversion.
    fn horizontal(&self, p0: DVec2, phases: &Phases) -> DVec2 {
        let mut d = DVec2::ZERO;
        for (wave, &c) in self.waves.iter().zip(&phases.count) {
            let theta = self.theta(wave, c, p0);
            d += wave.direction * (self.q(wave) * wave.amplitude * theta.cos());
        }
        d
    }

    fn theta(&self, wave: &Wave, count: u64, p0: DVec2) -> f64 {
        wave.wavenumber() * wave.direction.dot(p0) - TAU * count as f64 / self.period as f64
            + wave.phase
    }

    /// The displacement, normal, and velocity of the particle at rest
    /// position `p0`.
    #[must_use]
    pub fn at_rest(&self, p0: DVec2, phases: &Phases) -> Displacement {
        let mut horizontal = DVec2::ZERO;
        let mut height = 0.0;
        // Tangents ∂P/∂x0 and ∂P/∂z0 of the displaced surface.
        let mut tx = DVec3::X;
        let mut tz = DVec3::Z;
        let mut velocity = DVec3::ZERO;
        for (i, (wave, &c)) in self.waves.iter().zip(&phases.count).enumerate() {
            let theta = self.theta(wave, c, p0);
            let (s, co) = theta.sin_cos();
            let (a, k, q, d) = (
                wave.amplitude,
                wave.wavenumber(),
                self.q(wave),
                wave.direction,
            );
            let omega = self.omega(i);
            horizontal += d * (q * a * co);
            height += a * s;
            let qak = q * a * k * s;
            let ak = a * k * co;
            tx += DVec3::new(-qak * d.x * d.x, ak * d.x, -qak * d.x * d.y);
            tz += DVec3::new(-qak * d.x * d.y, ak * d.y, -qak * d.y * d.y);
            velocity += DVec3::new(
                q * a * omega * s * d.x,
                -a * omega * co,
                q * a * omega * s * d.y,
            );
        }
        Displacement {
            horizontal,
            height,
            normal: tz.cross(tx).normalize_or(DVec3::Y),
            velocity,
        }
    }

    /// The surface over world point `(x, z)`: the rest position whose
    /// particle lands there is found by fixed-point iteration, which
    /// contracts by at most the steepness each round.
    #[must_use]
    pub fn at(&self, x: DVec2, phases: &Phases) -> Displacement {
        if self.waves.is_empty() {
            return Displacement {
                horizontal: DVec2::ZERO,
                height: 0.0,
                normal: DVec3::Y,
                velocity: DVec3::ZERO,
            };
        }
        let mut p0 = x;
        for _ in 0..INVERSIONS {
            p0 = x - self.horizontal(p0, phases);
        }
        self.at_rest(p0, phases)
    }
}

/// Fixed-point rounds for inverting the horizontal displacement.
pub const INVERSIONS: usize = 5;
