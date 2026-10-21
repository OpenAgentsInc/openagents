//! Checks of the V4.1 Metal math against the host (#12420).
//!
//! The CUDA kernels round `exp` and `log1p` once from double precision, and
//! their host models do the same (`(x as f64).exp() as f32`). Apple GPUs have
//! no double precision, so `common.metal` computes both in float-float
//! arithmetic. These checks compare the GPU with the host for every float
//! input whose host result is a normal float or zero; the GPU flushes
//! subnormal results to zero, which [`ftz`] applies to the host value. The
//! software binary64 the sampler uses is checked against `f64` on random and
//! edge cases.

use super::{Args, V41Metal, le_bytes};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

/// The GPU's view of a host result: subnormals become zero of the same sign.
pub(crate) fn ftz(x: f32) -> f32 {
    if x.is_subnormal() {
        0.0_f32.copysign(x)
    } else {
        x
    }
}

/// The V4.1 kernels; every Mac these tests run on has a Metal 3 GPU, so a
/// failure to compile them fails the test.
pub(crate) fn metal() -> Option<V41Metal> {
    Some(V41Metal::new().expect("the V4.1 Metal kernels compile"))
}

/// Runs `kernel` over the float bits `[first, first + count)`.
fn gpu_range(
    metal: &V41Metal,
    kernel: &'static str,
    first: u32,
    count: u32,
) -> Result<Vec<u32>, Box<dyn Error>> {
    let out = metal.zeros(count as usize * 4)?;
    let dummy = metal.zeros(16)?;
    let mut s = metal.begin_submission();
    let args = Args::new()
        .ptr(s.span(&dummy, 0, 1, 4, "input")?)
        .ptr(s.span(&out, 0, count as usize, 4, "output")?)
        .i32(first as i32)
        .i32(count as i32);
    s.launch(kernel, args, ((count as usize).div_ceil(256), 1, 1), 256, 0)?;
    s.commit()?;
    Ok(out.i32s().into_iter().map(|v| v as u32).collect())
}

/// Compares the GPU and `host` for every float bit pattern in the ranges;
/// returns `(checked, mismatches)` with up to 20 examples.
fn exhaustive(
    metal: &V41Metal,
    kernel: &'static str,
    ranges: &[(u32, u32)],
    host: fn(f32) -> f32,
) -> Result<(u64, Vec<(u32, u32, u32)>), Box<dyn Error>> {
    const CHUNK: u32 = 1 << 24;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let mut checked = 0_u64;
    let mut bad = Vec::new();
    let mut bad_count = 0_u64;
    for &(start, end) in ranges {
        let mut first = start;
        while first < end {
            let count = CHUNK.min(end - first);
            let gpu = gpu_range(metal, kernel, first, count)?;
            let per = (count as usize).div_ceil(threads);
            let found: Vec<(u64, Vec<(u32, u32, u32)>)> = std::thread::scope(|scope| {
                let handles: Vec<_> = gpu
                    .chunks(per)
                    .enumerate()
                    .map(|(t, part)| {
                        scope.spawn(move || {
                            let mut n = 0_u64;
                            let mut examples = Vec::new();
                            for (i, g) in part.iter().enumerate() {
                                let bits = first + (t * per + i) as u32;
                                let want = ftz(host(f32::from_bits(bits)));
                                let got = f32::from_bits(*g);
                                let same = want.to_bits() == got.to_bits()
                                    || (want.is_nan() && got.is_nan());
                                if !same {
                                    n += 1;
                                    if examples.len() < 1000 {
                                        examples.push((bits, got.to_bits(), want.to_bits()));
                                    }
                                }
                            }
                            (n, examples)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("checker thread"))
                    .collect()
            });
            for (n, examples) in found {
                bad_count += n;
                for e in examples {
                    if bad.len() < 1000 {
                        bad.push(e);
                    }
                }
            }
            checked += u64::from(count);
            first += count;
        }
    }
    if bad_count > 0 {
        eprintln!("{kernel}: {bad_count} mismatches of {checked}");
    }
    Ok((checked, if bad_count > 0 { bad } else { Vec::new() }))
}

fn host_exp(x: f32) -> f32 {
    (f64::from(x)).exp() as f32
}

fn host_log1p(x: f32) -> f32 {
    (f64::from(x)).ln_1p() as f32
}

/// Every float from -104 to +89, plus both zeros and the specials (about
/// two seconds on an M5 Max).
#[test]
fn exp_once_equals_the_host_for_every_float() -> TestResult {
    let Some(metal) = metal() else { return Ok(()) };
    let ranges = [
        (0x0000_0000, 0x42b2_0001), // +0 to 89
        (0x8000_0000, 0xc2d0_0001), // -0 to -104
        (0x7f80_0000, 0x7f80_0001), // +inf
        (0xff80_0000, 0xff80_0001), // -inf
        (0x7fc0_0000, 0x7fc0_0001), // NaN
    ];
    let (checked, bad) = exhaustive(&metal, "v41_check_exp", &ranges, host_exp)?;
    eprintln!("exp_once: {checked} inputs");
    assert!(
        bad.is_empty(),
        "exp_once mismatches (input, gpu, host): {bad:x?}"
    );
    Ok(())
}

/// Every float from +0 to +inf.
#[test]
fn log1p_once_equals_the_host_for_every_nonnegative_float() -> TestResult {
    let Some(metal) = metal() else { return Ok(()) };
    // Subnormal inputs can't occur: the GPU flushes every subnormal it makes.
    let ranges = [(0x0000_0000, 0x0000_0001), (0x0080_0000, 0x7f80_0001)];
    let (checked, bad) = exhaustive(&metal, "v41_check_log1p", &ranges, host_log1p)?;
    eprintln!("log1p_once: {checked} inputs");
    assert!(
        bad.is_empty(),
        "log1p_once mismatches (input, gpu, host): {bad:x?}"
    );
    Ok(())
}

#[test]
fn software_double_rounds_as_f64() -> TestResult {
    let Some(metal) = metal() else { return Ok(()) };
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut pairs: Vec<(f64, f64)> = Vec::new();
    // Values like the sampler's: float weights in (0, 1], their sums, and
    // differences of nearby sums.
    for _ in 0..200_000 {
        let pick = next() % 6;
        let a = f64::from(f32::from_bits(
            (next() as u32 % 0x3f80_0000).max(0x0080_0000),
        ));
        let b = match pick {
            0 => f64::from(f32::from_bits(
                (next() as u32 % 0x3f80_0000).max(0x0080_0000),
            )),
            1 => a * (1.0 + (next() % 1000) as f64 * 1e-12),
            2 => (next() % 130_000) as f64 * 0.731,
            3 => f64::from_bits(next() % 0x40f0_0000_0000_0000 + 0x3000_0000_0000_0000),
            4 => -a,
            _ => 0.0,
        };
        pairs.push((a, b));
        pairs.push((b, a));
    }
    pairs.extend([
        (1.0, 1.0),
        (1.0, -1.0),
        (0.0, -0.0),
        (-0.0, -0.0),
        (1.0, 2f64.powi(-60)),
    ]);
    pairs.push((1.0 + 2f64.powi(-52), 2f64.powi(-53)));
    pairs.push((1.0, -(2f64.powi(-53))));
    let words: Vec<u64> = pairs
        .iter()
        .flat_map(|(a, b)| [a.to_bits(), b.to_bits()])
        .collect();
    let input = metal.byte_buffer(&le_bytes(&words, u64::to_le_bytes))?;
    let out = metal.zeros(pairs.len() * 32)?;
    let mut s = metal.begin_submission();
    let args = Args::new()
        .ptr(s.span(&input, 0, words.len(), 8, "pairs")?)
        .ptr(s.span(&out, 0, pairs.len() * 4, 8, "results")?)
        .i32(0)
        .i32(pairs.len() as i32);
    s.launch(
        "v41_check_double",
        args,
        (pairs.len().div_ceil(256), 1, 1),
        256,
        0,
    )?;
    s.commit()?;
    let got: Vec<u64> = out
        .bytes()
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().expect("8 bytes")))
        .collect();
    for (i, (a, b)) in pairs.iter().enumerate() {
        let want = [
            (a + b).to_bits(),
            (a - b).to_bits(),
            (a * b).to_bits(),
            u64::from(a >= b),
        ];
        assert_eq!(&got[4 * i..4 * i + 4], &want, "{a:e} {b:e}");
    }
    Ok(())
}
