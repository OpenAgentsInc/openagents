//! The spectral ocean: a seeded wave spectrum synthesized by FFT into
//! looping tiles, after Tessendorf ("Simulating Ocean Water", SIGGRAPH 2001
//! course notes).
//!
//! - **Spectrum.** JONSWAP (Hasselmann et al., "Measurements of
//!   Wind-Wave Growth and Swell Decay during the Joint North Sea Wave
//!   Project", 1973) from the wind speed and fetch, with the TMA depth
//!   factor for finite depth (Bouws et al., "Similarity of the Wind Wave
//!   Spectrum in Finite Depth Water", JGR 1985) and the dispersion relation
//!   `ω² = g k tanh(k h)`.
//! - **Directional spreading.** `cos^{2s}(θ/2)` (Longuet-Higgins et al.,
//!   1963) with the frequency-dependent `s` of Hasselmann et al. (1980) and
//!   the swell term and normalization Horvath gives ("Empirical Directional
//!   Wave Spectra for Computer Graphics", DigiPro 2015).
//! - **Amplitudes.** Each lattice mode's height is the spectrum integrated
//!   over its cell, with a seeded random phase and no random magnitude, so
//!   the tile's variance is the spectrum's integral by Parseval's theorem
//!   rather than a draw around it.
//! - **Time.** Every mode's angular frequency is snapped to a whole multiple
//!   of `2π` over the loop period, as Tessendorf suggests for looping, and
//!   its phase is an integer count `(harmonic · tick) mod period` read from a
//!   table, so a tick reproduces its tile bit for bit and the tile repeats
//!   exactly.
//! - **Cascades.** Up to [`MAX_CASCADES`] tiles of decreasing size split the
//!   wavenumbers into bands so none is counted twice (as Tessendorf's tiles
//!   at several scales are commonly layered). Cascade 0, the longest waves,
//!   is the gameplay band: `physics::water` samples it at
//!   [`GAMEPLAY_SIZE`]² by bilinear interpolation, exactly as a renderer's
//!   texture filter does, and the renderer draws the same modes.
//! - **Choppiness.** Horizontal displacement `D = Σ i k̂ h̃ e^{ik·x}` moves
//!   water toward each crest, scaled by `λ`, and the Jacobian of `x + λD`
//!   falls below one where crests sharpen (Tessendorf, section 4.6), which
//!   is where whitecaps form.
//!
//! Everything is `f32` after the mode table, and the transform is
//! [`super::fft::Fft2`].

use std::f64::consts::{PI, TAU};
use std::sync::{Arc, Mutex};

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::fft::Fft2;
use super::surface::G;

/// The gameplay band's grid side.
pub const GAMEPLAY_SIZE: usize = 64;
/// Cascade 0 holds waves of at least `patch / GAMEPLAY_CUT` m, four texels
/// or more of the gameplay grid.
pub const GAMEPLAY_CUT: f64 = 16.0;
/// Each cascade's tile is this many times smaller than the last. Not a
/// whole number, so the tiles' repeats never line up.
pub const RATIO: f64 = 5.3;
/// The most cascades.
pub const MAX_CASCADES: usize = 3;
/// The longest loop, ticks: the phase table holds one entry a tick.
pub const MAX_PERIOD: u64 = 1 << 17;
/// JONSWAP's peak enhancement factor.
pub const GAMMA: f64 = 3.3;

/// A seeded wave spectrum and the controls a sea state sets.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spectrum {
    pub seed: u64,
    /// Wind speed 10 m above the water, m/s.
    pub wind_speed: f64,
    /// The direction the waves travel, rad about +Y: 0 is +z, as the
    /// character's yaw.
    pub wind: f64,
    /// Fetch, m: how far the wind has blown over open water.
    pub fetch: f64,
    /// The peak's wavelength, m, or 0 for JONSWAP's own from wind and fetch.
    pub peak_wavelength: f64,
    /// Scales every height.
    pub amplitude: f64,
    /// The horizontal displacement's `λ`: 0 for round crests.
    pub choppiness: f64,
    /// Directional spread: 1 a wind sea's, 0 a narrow swell's.
    pub spread: f64,
    /// The share of the energy that travels against the wind, 0 to 0.5:
    /// above 0 the waves partly stand.
    pub standing: f64,
    /// Scales every angular frequency: above 1 the sea runs fast.
    pub time_scale: f64,
    /// The water's depth for dispersion and the TMA factor, m.
    pub depth: f64,
    /// Cascade 0's tile, m, or 0 for eight peak wavelengths.
    pub patch: f64,
    /// The loop, ticks (at most [`MAX_PERIOD`]).
    pub period: u64,
    /// One tick, s.
    pub tick: f64,
}

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            seed: 1,
            wind_speed: 8.0,
            wind: 0.0,
            fetch: 20_000.0,
            peak_wavelength: 0.0,
            amplitude: 1.0,
            choppiness: 0.8,
            spread: 1.0,
            standing: 0.0,
            time_scale: 1.0,
            depth: 200.0,
            patch: 0.0,
            period: 120 * 256,
            tick: 1.0 / 120.0,
        }
    }
}

/// One cascade's tile and band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    /// The tile's side, m.
    pub patch: f64,
    /// Texels along a side.
    pub size: usize,
    /// The band, `low ≤ |k| < high`, rad/m.
    pub low: f64,
    pub high: f64,
}

impl Cascade {
    /// The shortest wavelength in the band, m.
    #[must_use]
    pub fn shortest(&self) -> f64 {
        TAU / self.high
    }
}

impl Spectrum {
    /// Whether every control is finite and in range.
    ///
    /// # Errors
    ///
    /// Names the first control that is not.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f64| v.is_finite() && v > 0.0;
        let checks = [
            (
                self.wind_speed.is_finite() && self.wind_speed >= 0.0,
                "wind speed",
            ),
            (self.wind.is_finite(), "wind direction"),
            (positive(self.fetch), "fetch"),
            (
                self.peak_wavelength.is_finite() && self.peak_wavelength >= 0.0,
                "peak wavelength",
            ),
            (
                self.amplitude.is_finite() && self.amplitude >= 0.0,
                "amplitude",
            ),
            (
                self.choppiness.is_finite() && self.choppiness >= 0.0,
                "choppiness",
            ),
            ((0.0..=1.0).contains(&self.spread), "spread"),
            ((0.0..=0.5).contains(&self.standing), "standing ratio"),
            (positive(self.time_scale), "time scale"),
            (positive(self.depth), "depth"),
            (self.patch.is_finite() && self.patch >= 0.0, "patch"),
            ((1..=MAX_PERIOD).contains(&self.period), "period"),
            (positive(self.tick), "tick"),
        ];
        match checks.iter().find(|(ok, _)| !ok) {
            Some((_, name)) => Err(format!("the spectrum's {name} is out of range")),
            None => Ok(()),
        }
    }

    /// The unit direction the waves travel, (x, z).
    #[must_use]
    pub fn direction(&self) -> DVec2 {
        DVec2::new(self.wind.sin(), self.wind.cos())
    }

    /// The angular frequency of wavenumber `k` over this depth, rad/s.
    #[must_use]
    pub fn dispersion(&self, k: f64) -> f64 {
        (G * k * (k * self.depth).min(20.0).tanh()).sqrt()
    }

    /// `dω/dk` at wavenumber `k`, m/s.
    fn group(&self, k: f64, omega: f64) -> f64 {
        let kh = k * self.depth;
        let sech2 = if kh > 20.0 {
            0.0
        } else {
            1.0 / kh.cosh().powi(2)
        };
        G * (kh.min(20.0).tanh() + kh * sech2) / (2.0 * omega.max(1e-9))
    }

    /// The wavenumber of angular frequency `omega` over this depth, rad/m.
    #[must_use]
    pub fn wavenumber(&self, omega: f64) -> f64 {
        // Newton's method on g k tanh(k h) − ω², from the larger of the
        // deep- and shallow-water answers, which lies above the root.
        let h = self.depth;
        let w2 = omega * omega;
        let mut k = (w2 / G).max(omega / (G * h).sqrt());
        for _ in 0..30 {
            let t = (k * h).min(20.0).tanh();
            let f = G * k * t - w2;
            let df = G * (t + k * h * (1.0 - t * t));
            let next = k - f / df.max(1e-12);
            if (next - k).abs() <= 1e-15 * k {
                return next;
            }
            k = next;
        }
        k
    }

    /// The peak's angular frequency, rad/s: JONSWAP's
    /// `22 (g² / (U F))^{1/3}`, or the set peak wavelength's.
    #[must_use]
    pub fn peak_omega(&self) -> f64 {
        if self.peak_wavelength > 0.0 {
            return self.dispersion(TAU / self.peak_wavelength);
        }
        let u = self.wind_speed.max(0.1);
        22.0 * (G * G / (u * self.fetch)).cbrt()
    }

    /// The peak's wavelength, m.
    #[must_use]
    pub fn peak(&self) -> f64 {
        TAU / self.wavenumber(self.peak_omega())
    }

    /// Cascade 0's tile, m.
    #[must_use]
    pub fn patch(&self) -> f64 {
        if self.patch > 0.0 {
            self.patch
        } else {
            (8.0 * self.peak()).clamp(32.0, 2048.0)
        }
    }

    /// The frequency spectrum `S(ω)`, m²·s: JONSWAP times the TMA factor,
    /// times the amplitude squared.
    #[must_use]
    pub fn jonswap(&self, omega: f64) -> f64 {
        if omega <= 1e-6 || self.wind_speed <= 0.0 {
            return 0.0;
        }
        let wp = self.peak_omega();
        let alpha = 0.076 * (self.wind_speed * self.wind_speed / (self.fetch * G)).powf(0.22);
        let sigma = if omega <= wp { 0.07 } else { 0.09 };
        let r = (-(omega - wp).powi(2) / (2.0 * sigma * sigma * wp * wp)).exp();
        let s =
            alpha * G * G / omega.powi(5) * (-1.25 * (wp / omega).powi(4)).exp() * GAMMA.powf(r);
        // Kitaigorodskii's depth factor in Thompson and Vincent's form.
        let wh = omega * (self.depth / G).sqrt();
        let tma = if wh <= 1.0 {
            0.5 * wh * wh
        } else if wh < 2.0 {
            1.0 - 0.5 * (2.0 - wh).powi(2)
        } else {
            1.0
        };
        s * tma * self.amplitude * self.amplitude
    }

    /// The directional spreading `D(θ; ω)`, normalized over the circle,
    /// at `theta` rad off the wind.
    #[must_use]
    pub fn spreading(&self, theta: f64, omega: f64) -> f64 {
        let wp = self.peak_omega();
        let r = omega / wp;
        let mut s = if r <= 1.0 {
            6.97 * r.powf(4.06)
        } else {
            let mu = -2.33 - 1.45 * (self.wind_speed * wp / G - 1.17);
            9.77 * r.powf(mu)
        };
        let swell = 1.0 - self.spread;
        s += 16.0 * (wp / omega).tanh() * swell * swell;
        let s = s.clamp(1e-3, 200.0);
        let q = ((2.0 * s - 1.0) * std::f64::consts::LN_2 + 2.0 * ln_gamma(s + 1.0)
            - ln_gamma(2.0 * s + 1.0))
        .exp()
            / PI;
        q * (theta * 0.5).cos().abs().powf(2.0 * s)
    }

    /// The directional wavenumber spectrum `E(k)`, m⁴: the energy per unit
    /// area of wavenumber, with `standing` of it turned against the wind.
    #[must_use]
    pub fn density(&self, k: DVec2) -> f64 {
        let length = k.length();
        if length <= 1e-9 {
            return 0.0;
        }
        let omega = self.dispersion(length);
        let wind = self.direction();
        let theta = wind.perp_dot(k).atan2(wind.dot(k));
        let mut spread = (1.0 - self.standing) * self.spreading(theta, omega);
        if self.standing > 0.0 {
            spread += self.standing * self.spreading(theta + PI, omega);
        }
        self.jonswap(omega) * spread * self.group(length, omega) / length
    }

    /// The significant wave height `4 √m₀`, m, over every frequency.
    #[must_use]
    pub fn significant_height(&self) -> f64 {
        let wp = self.peak_omega();
        let (from, to, steps) = (0.2 * wp, 12.0 * wp, 4000);
        let dw = (to - from) / steps as f64;
        let m0: f64 = (0..steps)
            .map(|i| self.jonswap(from + (i as f64 + 0.5) * dw) * dw)
            .sum();
        4.0 * m0.sqrt()
    }

    /// `count` cascades: cascade 0 the gameplay band at
    /// [`GAMEPLAY_SIZE`] texels on every tier, so a renderer draws the grid
    /// `physics::water` samples, and each next one [`RATIO`] times smaller
    /// at `size` texels, the last running to its tile's Nyquist limit.
    #[must_use]
    pub fn cascades(&self, count: usize, size: usize) -> Vec<Cascade> {
        let count = count.clamp(1, MAX_CASCADES);
        let mut patch = self.patch();
        let mut low = 0.0;
        (0..count)
            .map(|c| {
                let cut = if c == 0 {
                    GAMEPLAY_CUT
                } else if c + 1 < count {
                    (size / 4) as f64
                } else {
                    (size / 2 - 1) as f64
                };
                let cascade = Cascade {
                    patch,
                    size: if c == 0 { GAMEPLAY_SIZE } else { size },
                    low,
                    high: TAU * cut / patch,
                };
                low = cascade.high;
                patch /= RATIO;
                cascade
            })
            .collect()
    }

    /// The tick at `time` s.
    #[must_use]
    pub fn tick_at(&self, time: f64) -> u64 {
        (time / self.tick + 1e-6).floor().max(0.0) as u64
    }
}

/// `ln Γ(x)` for `x > 0`, by Lanczos's approximation (g = 7, nine terms).
fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (PI / (PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * TAU.ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// A seeded uniform value in [0, 1) for one lattice mode: SplitMix64
/// (Steele, Lea, and Flood, OOPSLA 2014) over the seed, the cascade, and
/// the mode's integer wave vector.
fn mode_random(seed: u64, cascade: usize, n: i64, m: i64) -> f64 {
    let mut z = seed
        ^ (cascade as u64).wrapping_mul(0xd6e8_feb8_6659_fd93)
        ^ (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (m as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// One cascade's modes in its band.
#[derive(Clone, Debug)]
struct Modes {
    cascade: Cascade,
    /// Integer wave vectors, cycles per tile along x and z.
    nm: Vec<(i32, i32)>,
    /// `h₀(k)` and `conj(h₀(−k))`, m.
    h0: Vec<[f32; 2]>,
    h0m: Vec<[f32; 2]>,
    /// The wave vector, rad/m, and `1 / |k|`.
    k: Vec<[f32; 3]>,
    /// The angular frequency as a whole multiple of `2π / period`.
    harmonic: Vec<u64>,
}

impl Modes {
    fn new(spectrum: &Spectrum, index: usize, cascade: Cascade) -> Self {
        let dk = TAU / cascade.patch;
        let reach = (cascade.high / dk).ceil() as i64;
        let span = spectrum.period as f64 * spectrum.tick;
        // The spectrum integrated over a lattice cell, 4 × 4 points.
        let cell = |n: i64, m: i64| {
            let mut sum = 0.0;
            for a in 0..4 {
                for b in 0..4 {
                    let k = DVec2::new(
                        (n as f64 + (a as f64 + 0.5) / 4.0 - 0.5) * dk,
                        (m as f64 + (b as f64 + 0.5) / 4.0 - 0.5) * dk,
                    );
                    sum += spectrum.density(k);
                }
            }
            sum / 16.0 * dk * dk
        };
        let amplitude = |n: i64, m: i64| {
            let energy = cell(n, m);
            let phase = mode_random(spectrum.seed, index, n, m) * TAU;
            let a = (energy * 0.5).sqrt();
            [(a * phase.cos()) as f32, (a * phase.sin()) as f32]
        };
        let mut modes = Self {
            cascade,
            nm: Vec::new(),
            h0: Vec::new(),
            h0m: Vec::new(),
            k: Vec::new(),
            harmonic: Vec::new(),
        };
        let half = (cascade.size / 2) as i64;
        for m in -reach..=reach {
            for n in -reach..=reach {
                let k = DVec2::new(n as f64, m as f64) * dk;
                let length = k.length();
                if (n, m) == (0, 0)
                    || length < cascade.low
                    || length >= cascade.high
                    || n.abs() >= half
                    || m.abs() >= half
                {
                    continue;
                }
                let minus = amplitude(-n, -m);
                let omega = spectrum.dispersion(length) * spectrum.time_scale;
                modes.nm.push((n as i32, m as i32));
                modes.h0.push(amplitude(n, m));
                modes.h0m.push([minus[0], -minus[1]]);
                modes
                    .k
                    .push([k.x as f32, k.y as f32, (1.0 / length) as f32]);
                modes
                    .harmonic
                    .push(((omega * span / TAU).round() as u64).max(1));
            }
        }
        modes
    }

    /// The texel a mode lands on in a `size`-texel grid.
    fn texel(&self, i: usize, size: usize) -> usize {
        let (n, m) = self.nm[i];
        let wrap = |v: i32| v.rem_euclid(size as i32) as usize;
        wrap(m) * size + wrap(n)
    }
}

/// One tick of a cascade for a renderer, `size × size` each, row-major
/// along x with z down the rows: texel `(i, j)` is the rest point
/// `(i, j) · patch / size`.
#[derive(Clone, Debug, Default)]
pub struct Tile {
    pub size: usize,
    /// Horizontal displacement x and z and height, m.
    pub dx: Vec<f32>,
    pub height: Vec<f32>,
    pub dz: Vec<f32>,
    /// Slopes `∂h/∂x` and `∂h/∂z`.
    pub sx: Vec<f32>,
    pub sz: Vec<f32>,
    /// The Jacobian of the horizontal map `x + D`: below one where crests
    /// sharpen, below zero where they fold.
    pub jacobian: Vec<f32>,
    scratch: [Vec<f32>; 8],
}

impl Tile {
    /// Heap storage retained by all fields and FFT scratch grids, bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        [&self.dx, &self.height, &self.dz, &self.sx, &self.sz, &self.jacobian]
            .into_iter().chain(self.scratch.iter()).map(|v| v.capacity() * 4).sum()
    }
}

/// One tick of the gameplay band at [`GAMEPLAY_SIZE`]²: displacement and
/// the water's velocity at each rest point.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub patch: f64,
    pub size: usize,
    /// The folded tick.
    pub tick: u64,
    /// The spectrum's significant height, m, and the wavenumber a
    /// renderer shoals the band by, rad/m: the peak's, or the band's edge
    /// when the peak lies beyond it.
    pub significant: f64,
    pub shoal: f64,
    /// Per texel: x, height, z displacement (m), then velocity x, y, z
    /// (m/s).
    pub texels: Vec<[f32; 6]>,
}

impl Field {
    /// The bilinear sample at rest point `p`: displacement x, height, z,
    /// and velocity x, y, z.
    #[must_use]
    pub fn sample(&self, p: DVec2) -> [f64; 6] {
        let n = self.size;
        let u = p.x / self.patch * n as f64;
        let v = p.y / self.patch * n as f64;
        let (fu, fv) = (u.floor(), v.floor());
        let (tu, tv) = (u - fu, v - fv);
        let wrap = |a: f64| (a.rem_euclid(n as f64) as usize).min(n - 1);
        let (i0, j0) = (wrap(fu), wrap(fv));
        let (i1, j1) = ((i0 + 1) % n, (j0 + 1) % n);
        let at = |i: usize, j: usize| &self.texels[j * n + i];
        let mut out = [0.0; 6];
        for (c, slot) in out.iter_mut().enumerate() {
            let a = f64::from(at(i0, j0)[c]);
            let b = f64::from(at(i1, j0)[c]);
            let d = f64::from(at(i0, j1)[c]);
            let e = f64::from(at(i1, j1)[c]);
            *slot = (a * (1.0 - tu) + b * tu) * (1.0 - tv) + (d * (1.0 - tu) + e * tu) * tv;
        }
        out
    }
}

/// A spectrum's cascades ready to synthesize: mode tables, the phase
/// table, and the transforms. Building one is the slow part; a tick is an
/// FFT per pair of fields.
#[derive(Clone, Debug)]
pub struct Synth {
    spectrum: Spectrum,
    modes: Vec<Modes>,
    /// `cos` and `sin` of `2π j / period`.
    phase: Vec<[f32; 2]>,
    fft: Fft2,
    gameplay: Fft2,
    significant: f64,
    shoal: f64,
}

impl Synth {
    /// Heap storage retained by mode tables, phases, and both transforms.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.phase.capacity() * std::mem::size_of::<[f32; 2]>()
            + self.fft.heap_bytes() + self.gameplay.heap_bytes()
            + self.modes.capacity() * std::mem::size_of::<Modes>()
            + self.modes.iter().map(|m| m.nm.capacity() * 8 + m.h0.capacity() * 8
                + m.h0m.capacity() * 8 + m.k.capacity() * 12 + m.harmonic.capacity() * 8).sum::<usize>()
    }
    /// `count` cascades of `size` texels.
    ///
    /// # Errors
    ///
    /// As [`Spectrum::validate`], and for a size that is not a power of two
    /// from 16 to 512.
    pub fn new(spectrum: &Spectrum, count: usize, size: usize) -> Result<Self, String> {
        spectrum.validate()?;
        if !(16..=512).contains(&size) || !size.is_power_of_two() {
            return Err(format!(
                "cascade size {size} is not a power of two in 16..=512"
            ));
        }
        let modes: Vec<Modes> = spectrum
            .cascades(count, size)
            .into_iter()
            .enumerate()
            .map(|(i, c)| Modes::new(spectrum, i, c))
            .collect();
        let shoal = spectrum
            .wavenumber(spectrum.peak_omega())
            .min(modes[0].cascade.high);
        let period = spectrum.period;
        let phase = (0..period)
            .map(|j| {
                let a = TAU * j as f64 / period as f64;
                [a.cos() as f32, a.sin() as f32]
            })
            .collect();
        Ok(Self {
            spectrum: *spectrum,
            modes,
            phase,
            fft: Fft2::new(size),
            gameplay: Fft2::new(GAMEPLAY_SIZE),
            significant: spectrum.significant_height(),
            shoal,
        })
    }

    #[must_use]
    pub fn spectrum(&self) -> &Spectrum {
        &self.spectrum
    }

    /// The spectrum's significant height, m.
    #[must_use]
    pub fn significant(&self) -> f64 {
        self.significant
    }

    /// The cascades this synthesizes.
    #[must_use]
    pub fn cascades(&self) -> Vec<Cascade> {
        self.modes.iter().map(|m| m.cascade).collect()
    }

    /// Modes in cascade `c`'s band.
    #[must_use]
    pub fn mode_count(&self, c: usize) -> usize {
        self.modes.get(c).map_or(0, |m| m.nm.len())
    }

    /// The height variance cascade `c`'s modes carry, `Σ ½|h₀(k)|²·2`, m².
    #[must_use]
    pub fn variance(&self, c: usize) -> f64 {
        self.modes.get(c).map_or(0.0, |m| {
            m.h0.iter()
                .map(|h| 2.0 * (f64::from(h[0]).powi(2) + f64::from(h[1]).powi(2)))
                .sum()
        })
    }

    /// `h̃(k, t)` and `∂h̃/∂t` for mode `i` of `modes` at folded `tick`.
    fn evolve(&self, modes: &Modes, i: usize, tick: u64) -> ([f32; 2], [f32; 2]) {
        let period = self.spectrum.period;
        let j = (u128::from(modes.harmonic[i]) * u128::from(tick % period) % u128::from(period))
            as usize;
        let [c, s] = self.phase[j];
        let [ar, ai] = modes.h0[i];
        let [br, bi] = modes.h0m[i];
        // h₀ e^{−iθ} + conj(h₀(−k)) e^{+iθ}.
        let (pr, pi) = (ar * c + ai * s, ai * c - ar * s);
        let (mr, mi) = (br * c - bi * s, bi * c + br * s);
        let omega = (TAU * modes.harmonic[i] as f64 / (period as f64 * self.spectrum.tick)) as f32;
        // ∂/∂t: −iω for the first, +iω for the second.
        let dt = [omega * (pi - mi), omega * (mr - pr)];
        ([pr + mr, pi + mi], dt)
    }

    /// Cascade `c` at `tick` into `tile`.
    ///
    /// # Panics
    ///
    /// When `c` is not a cascade of this synth.
    pub fn tile(&self, c: usize, tick: u64, tile: &mut Tile) {
        let modes = &self.modes[c];
        let n = modes.cascade.size;
        let fft = if n == GAMEPLAY_SIZE {
            &self.gameplay
        } else {
            &self.fft
        };
        let lambda = self.spectrum.choppiness as f32;
        for v in &mut tile.scratch {
            v.clear();
            v.resize(n * n, 0.0);
        }
        let [ar, ai, br, bi, cr, ci, dr, di] = &mut tile.scratch;
        for i in 0..modes.nm.len() {
            let ([hr, hi], _) = self.evolve(modes, i, tick);
            let [kx, kz, inv] = modes.k[i];
            let t = modes.texel(i, n);
            let (ux, uz) = (kx * inv * lambda, kz * inv * lambda);
            // Pairs packed as A + iB: (h, dx), (dz, sx), (sz, jxx), (jzz, jxz).
            // dx = i λ k̂x h̃, sx = i kx h̃, jxx = −λ kx² / k h̃.
            let dx = [-ux * hi, ux * hr];
            let dz = [-uz * hi, uz * hr];
            let sx = [-kx * hi, kx * hr];
            let sz = [-kz * hi, kz * hr];
            let jxx = [-ux * kx * hr, -ux * kx * hi];
            let jzz = [-uz * kz * hr, -uz * kz * hi];
            let jxz = [-ux * kz * hr, -ux * kz * hi];
            ar[t] = hr - dx[1];
            ai[t] = hi + dx[0];
            br[t] = dz[0] - sx[1];
            bi[t] = dz[1] + sx[0];
            cr[t] = sz[0] - jxx[1];
            ci[t] = sz[1] + jxx[0];
            dr[t] = jzz[0] - jxz[1];
            di[t] = jzz[1] + jxz[0];
        }
        for (re, im) in [(&mut *ar, &mut *ai), (br, bi), (cr, ci), (dr, di)] {
            fft.inverse(re, im);
        }
        tile.size = n;
        let [ar, ai, br, bi, cr, ci, dr, di] = &tile.scratch;
        tile.height.clone_from(ar);
        tile.dx.clone_from(ai);
        tile.dz.clone_from(br);
        tile.sx.clone_from(bi);
        tile.sz.clone_from(cr);
        tile.jacobian.clear();
        tile.jacobian.extend(
            ci.iter()
                .zip(dr)
                .zip(di)
                .map(|((jxx, jzz), jxz)| (1.0 + jxx) * (1.0 + jzz) - jxz * jxz),
        );
    }

    /// The gameplay band at `tick`.
    #[must_use]
    pub fn field(&self, tick: u64) -> Field {
        let n = GAMEPLAY_SIZE;
        let modes = &self.modes[0];
        let lambda = self.spectrum.choppiness as f32;
        let mut s: [Vec<f32>; 6] = std::array::from_fn(|_| vec![0.0; n * n]);
        let [ar, ai, br, bi, cr, ci] = &mut s;
        for i in 0..modes.nm.len() {
            let ([hr, hi], [vr, vi]) = self.evolve(modes, i, tick);
            let [kx, kz, inv] = modes.k[i];
            let t = modes.texel(i, n);
            let (ux, uz) = (kx * inv * lambda, kz * inv * lambda);
            let dx = [-ux * hi, ux * hr];
            let dz = [-uz * hi, uz * hr];
            let vx = [-ux * vi, ux * vr];
            let vz = [-uz * vi, uz * vr];
            // (h, dx), (dz, vy), (vx, vz).
            ar[t] = hr - dx[1];
            ai[t] = hi + dx[0];
            br[t] = dz[0] - vi;
            bi[t] = dz[1] + vr;
            cr[t] = vx[0] - vz[1];
            ci[t] = vx[1] + vz[0];
        }
        for (re, im) in [(&mut *ar, &mut *ai), (br, bi), (cr, ci)] {
            self.gameplay.inverse(re, im);
        }
        let [ar, ai, br, bi, cr, ci] = &s;
        Field {
            patch: modes.cascade.patch,
            size: n,
            tick: tick % self.spectrum.period,
            significant: self.significant,
            shoal: self.shoal,
            texels: (0..n * n)
                .map(|t| [ai[t], ar[t], br[t], cr[t], bi[t], ci[t]])
                .collect(),
        }
    }
}

/// Memoized gameplay synths and fields, so every body sampled at one tick
/// shares one transform. The cache only remembers: a field is a pure
/// function of the spectrum and the folded tick.
struct Cache {
    synths: Vec<(Spectrum, Arc<Synth>)>,
    fields: Vec<(Spectrum, u64, Arc<Field>)>,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache {
    synths: Vec::new(),
    fields: Vec::new(),
});

/// The gameplay band of `spectrum` at `tick` (folded on its period), or
/// none for an invalid spectrum.
#[must_use]
pub fn field(spectrum: &Spectrum, tick: u64) -> Option<Arc<Field>> {
    let tick = tick % spectrum.period.max(1);
    let synth = {
        let mut cache = CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, _, f)) = cache
            .fields
            .iter()
            .find(|(s, t, _)| s == spectrum && *t == tick)
        {
            return Some(f.clone());
        }
        match cache.synths.iter().position(|(s, _)| s == spectrum) {
            Some(i) => cache.synths[i].1.clone(),
            None => {
                let synth = Arc::new(Synth::new(spectrum, 1, GAMEPLAY_SIZE).ok()?);
                if cache.synths.len() >= 4 {
                    cache.synths.remove(0);
                }
                cache.synths.push((*spectrum, synth.clone()));
                synth
            }
        }
    };
    let field = Arc::new(synth.field(tick));
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if cache.fields.len() >= 8 {
        cache.fields.remove(0);
    }
    cache.fields.push((*spectrum, tick, field.clone()));
    Some(field)
}
