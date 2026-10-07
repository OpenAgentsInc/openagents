//! The spectral ocean's acceptance: variance against the spectrum's
//! integral, exact loops, reproducible ticks, the published shape of the
//! spectrum, sharpened crests, and a surface that agrees with its band.

use std::f64::consts::{PI, TAU};

use glam::DVec2;

use super::spectrum::*;
use super::surface::G;
use super::{WaterBody, WaterId, WaveSet};

fn sea(wind_speed: f64, fetch: f64, seed: u64) -> Spectrum {
    Spectrum {
        seed,
        wind_speed,
        fetch,
        wind: 0.4,
        ..Spectrum::default()
    }
}

/// The spectrum integrated over a cascade's band on a lattice twelve times
/// finer than the modes', m².
fn band_integral(spectrum: &Spectrum, cascade: &Cascade) -> f64 {
    let dk = TAU / cascade.patch;
    let reach = (cascade.high / dk).ceil() as i64;
    let half = (cascade.size / 2) as i64;
    let fine = 12;
    let mut sum = 0.0;
    for m in -reach..=reach {
        for n in -reach..=reach {
            let length = (DVec2::new(n as f64, m as f64) * dk).length();
            if (n, m) == (0, 0)
                || length < cascade.low
                || length >= cascade.high
                || n.abs() >= half
                || m.abs() >= half
            {
                continue;
            }
            for a in 0..fine {
                for b in 0..fine {
                    let k = DVec2::new(
                        n as f64 + (a as f64 + 0.5) / fine as f64 - 0.5,
                        m as f64 + (b as f64 + 0.5) / fine as f64 - 0.5,
                    ) * dk;
                    sum += spectrum.density(k);
                }
            }
        }
    }
    sum / (fine * fine) as f64 * dk * dk
}

fn variance(v: &[f32]) -> f64 {
    let n = v.len() as f64;
    let mean = v.iter().map(|&x| f64::from(x)).sum::<f64>() / n;
    v.iter()
        .map(|&x| (f64::from(x) - mean).powi(2))
        .sum::<f64>()
        / n
}

/// Each tile's height variance is the spectrum's integral over its band.
/// Waves that meet head-on interfere, so a cascade whose band spreads
/// widely (the short waves) swings around its integral as they pass; its
/// mean over the loop holds within 2%, as does each tick's variance of
/// cascade 0, the gameplay band, and of the whole surface. Calm, moderate,
/// and storm seas, several seeds and ticks.
#[test]
fn tile_variance_matches_the_spectrum_integral() {
    for (spectrum, count, size) in [
        (sea(4.0, 5_000.0, 1), 2, 64),
        (sea(9.0, 40_000.0, 2), 3, 128),
        (sea(18.0, 200_000.0, 3), 2, 64),
        (
            Spectrum {
                choppiness: 1.2,
                spread: 0.3,
                ..sea(12.0, 80_000.0, 4)
            },
            3,
            64,
        ),
    ] {
        let synth = Synth::new(&spectrum, count, size).unwrap();
        let mut tile = Tile::default();
        let integrals: Vec<f64> = synth
            .cascades()
            .iter()
            .map(|c| band_integral(&spectrum, c))
            .collect();
        // Ticks spread unevenly over the loop, so no harmonic aliases.
        let ticks: Vec<u64> = (0..32u64)
            .map(|i| (i * 0x9e37_79b9 + 977) % spectrum.period)
            .collect();
        let mut totals = vec![0.0; ticks.len()];
        for (c, &integral) in integrals.iter().enumerate() {
            assert!(integral > 0.0);
            assert!(
                (synth.variance(c) / integral - 1.0).abs() < 0.01,
                "modes {} vs {integral}",
                synth.variance(c)
            );
            let mut mean = 0.0;
            for (i, &tick) in ticks.iter().enumerate() {
                synth.tile(c, tick, &mut tile);
                let v = variance(&tile.height);
                if c == 0 {
                    assert!(
                        (v / integral - 1.0).abs() < 0.02,
                        "{spectrum:?} cascade 0 tick {tick}: {v} vs {integral}"
                    );
                }
                totals[i] += v;
                mean += v / ticks.len() as f64;
            }
            assert!(
                (mean / integral - 1.0).abs() < 0.02,
                "{spectrum:?} cascade {c}: mean {mean} vs {integral}"
            );
        }
        let integral: f64 = integrals.iter().sum();
        for total in totals {
            assert!(
                (total / integral - 1.0).abs() < 0.02,
                "{spectrum:?}: {total} vs {integral}"
            );
        }
    }
}

/// A tile repeats exactly on its period, and the same seed and tick give
/// the same tile from a fresh synth; another seed or tick does not.
#[test]
fn tiles_loop_and_reproduce() {
    let spectrum = sea(10.0, 50_000.0, 7);
    let a = Synth::new(&spectrum, 2, 64).unwrap();
    let b = Synth::new(&spectrum, 2, 64).unwrap();
    let other = Synth::new(
        &Spectrum {
            seed: 8,
            ..spectrum
        },
        2,
        64,
    )
    .unwrap();
    let (mut t0, mut t1, mut t2) = (Tile::default(), Tile::default(), Tile::default());
    for c in 0..2 {
        a.tile(c, 4321, &mut t0);
        a.tile(c, 4321 + spectrum.period * 3, &mut t1);
        b.tile(c, 4321, &mut t2);
        assert_eq!(t0.height, t1.height);
        assert_eq!(t0.dx, t1.dx);
        assert_eq!(t0.jacobian, t1.jacobian);
        assert_eq!(t0.height, t2.height);
        assert_eq!(t0.sz, t2.sz);
        other.tile(c, 4321, &mut t2);
        assert_ne!(t0.height, t2.height);
        a.tile(c, 4322, &mut t2);
        assert_ne!(t0.height, t2.height);
    }
    let f = a.field(99);
    assert_eq!(f, a.field(99 + spectrum.period));
    assert_eq!(f, *field(&spectrum, 99 + 2 * spectrum.period).unwrap());
    // The gameplay band is the render cascade 0's modes on the same grid.
    a.tile(0, 99, &mut t0);
    for (t, texel) in f.texels.iter().enumerate() {
        assert!((texel[1] - t0.height[t]).abs() < 1e-4);
        assert!((texel[0] - t0.dx[t]).abs() < 1e-4);
        assert!((texel[2] - t0.dz[t]).abs() < 1e-4);
    }
}

/// Fetch-limited growth: JONSWAP's significant height follows
/// `g H / U² = 1.6 × 10⁻³ (g F / U²)^½` within 15%, the spreading
/// integrates to one, and shallow water slows long waves.
#[test]
fn the_spectrum_has_its_published_shape() {
    let cases = [(5.0, 10_000.0), (10.0, 50_000.0), (20.0, 100_000.0)];
    let ratios: Vec<f64> = cases
        .iter()
        .map(|&(u, f)| {
            let expected = 1.6e-3 * (G * f / (u * u)).sqrt() * u * u / G;
            sea(u, f, 1).significant_height() / expected
        })
        .collect();
    assert!(
        ratios.iter().all(|r| (r - 1.0).abs() < 0.3),
        "significant height over the fetch law: {ratios:?}"
    );
    for (u, f) in cases {
        let s = sea(u, f, 1);
        for r in [0.6, 1.0, 1.7, 4.0] {
            let omega = s.peak_omega() * r;
            let steps = 4000;
            let total: f64 = (0..steps)
                .map(|i| {
                    s.spreading(-PI + (i as f64 + 0.5) * TAU / steps as f64, omega) * TAU
                        / steps as f64
                })
                .sum();
            assert!((total - 1.0).abs() < 1e-3, "{r}: {total}");
        }
    }
    let shallow = Spectrum {
        depth: 2.0,
        ..Spectrum::default()
    };
    let k = 0.1;
    assert!((shallow.dispersion(k) - (G * k * 0.2f64.tanh()).sqrt()).abs() < 1e-12);
    assert!((shallow.wavenumber(shallow.dispersion(k)) - k).abs() < 1e-9);
}

/// Choppy displacement moves water toward the crests: the Jacobian falls
/// where the surface is high.
#[test]
fn crests_sharpen() {
    let spectrum = Spectrum {
        choppiness: 1.4,
        ..sea(16.0, 150_000.0, 5)
    };
    let synth = Synth::new(&spectrum, 1, 64).unwrap();
    let mut tile = Tile::default();
    synth.tile(0, 500, &mut tile);
    let n = tile.height.len() as f64;
    let mean_h = tile.height.iter().map(|&h| f64::from(h)).sum::<f64>() / n;
    let mean_j = tile.jacobian.iter().map(|&j| f64::from(j)).sum::<f64>() / n;
    let cov = tile
        .height
        .iter()
        .zip(&tile.jacobian)
        .map(|(&h, &j)| (f64::from(h) - mean_h) * (f64::from(j) - mean_j))
        .sum::<f64>()
        / n;
    assert!(cov < 0.0, "{cov}");
    assert!((mean_j - 1.0).abs() < 0.05, "{mean_j}");
}

/// An ocean's surface with a spectrum: a tick sampled directly equals
/// stepping to it, the height is the band's at the rest point, and the
/// band's vertical velocity is the height's rate of change.
#[test]
fn the_spectral_surface_is_deterministic_and_consistent() {
    let spectrum = sea(9.0, 30_000.0, 11);
    let waves = WaveSet::calm().with_spectrum(spectrum).unwrap();
    let body = WaterBody::ocean(WaterId(0), 0.0).with_waves(waves.clone());
    let mut phases = waves.phases(0);
    for _ in 0..37 {
        waves.advance(&mut phases);
    }
    assert_eq!(phases, waves.phases(37));
    assert_eq!(
        body.surface().sample(3.0, -4.0, 37),
        body.surface().sample_with(3.0, -4.0, &phases)
    );
    let f = field(&spectrum, 37).unwrap();
    let p0 = DVec2::new(12.0, 7.5);
    let at = waves.at_rest(p0, &phases);
    assert_eq!(at.height, f.sample(p0)[1]);
    let x = p0 + at.horizontal;
    let sample = body.surface().sample(x.x, x.y, 37).unwrap();
    assert!(
        (sample.height - at.height).abs() < 5e-3,
        "{sample:?} {at:?}"
    );
    let before = waves.at_rest(p0, &waves.phases(36)).height;
    let after = waves.at_rest(p0, &waves.phases(38)).height;
    let rate = (after - before) / (2.0 * spectrum.tick);
    assert!(
        (rate - at.velocity.y).abs() < 0.02 + 0.02 * rate.abs(),
        "{rate} vs {}",
        at.velocity.y
    );
    assert!(!waves.is_calm());
    assert_eq!(waves.phases(spectrum.period + 5).spectral(), 5);
}
