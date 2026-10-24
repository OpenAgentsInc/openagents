//! Source terrain: fixed seed 1, ten-octave 3D simplex sampled at y=0.
//! Adapted from SimonDev and Jonas Wagner; see third_party/quick-mmorpg/NOTICE
//! and third_party/simplex-noise/LICENSE.
const PERM: [u8; 256] = [
    134, 32, 123, 160, 45, 84, 8, 75, 127, 33, 73, 189, 243, 14, 227, 213, 116, 101, 71, 118, 232,
    177, 38, 36, 132, 247, 23, 223, 201, 145, 29, 203, 113, 131, 200, 129, 192, 37, 179, 195, 198,
    158, 253, 34, 69, 242, 251, 30, 252, 254, 110, 219, 186, 222, 89, 121, 185, 235, 2, 107, 218,
    249, 119, 25, 124, 156, 136, 238, 161, 55, 7, 40, 208, 43, 22, 144, 142, 39, 163, 80, 141, 140,
    99, 130, 44, 92, 31, 109, 209, 68, 204, 180, 115, 170, 199, 20, 173, 157, 27, 175, 149, 230,
    54, 137, 91, 26, 28, 85, 202, 63, 56, 205, 5, 67, 83, 96, 49, 117, 62, 48, 229, 90, 42, 120,
    150, 61, 153, 72, 212, 16, 59, 224, 220, 17, 122, 228, 81, 174, 233, 6, 78, 112, 35, 57, 53,
    143, 93, 47, 217, 105, 221, 100, 250, 0, 50, 162, 10, 102, 194, 245, 111, 41, 128, 88, 13, 4,
    184, 58, 241, 155, 178, 172, 3, 248, 87, 214, 52, 171, 76, 196, 24, 240, 103, 97, 159, 138, 82,
    225, 108, 133, 135, 239, 176, 65, 11, 181, 79, 234, 246, 244, 86, 125, 216, 9, 169, 187, 106,
    206, 188, 74, 210, 1, 197, 226, 164, 60, 139, 193, 168, 98, 66, 154, 182, 77, 215, 191, 147,
    21, 167, 211, 114, 12, 166, 94, 207, 51, 95, 165, 19, 183, 152, 237, 148, 255, 231, 151, 126,
    15, 70, 236, 46, 104, 18, 190, 146, 64,
];
const PERM2: [u8; 256] = [
    117, 193, 231, 146, 225, 180, 92, 76, 249, 22, 228, 173, 138, 36, 243, 55, 4, 182, 199, 17,
    237, 102, 178, 218, 241, 248, 187, 154, 212, 198, 255, 65, 226, 204, 7, 90, 158, 141, 109, 19,
    155, 140, 70, 169, 101, 44, 68, 57, 2, 131, 171, 46, 38, 59, 67, 221, 43, 175, 74, 184, 93, 1,
    9, 127, 186, 132, 143, 110, 95, 52, 227, 177, 174, 11, 116, 194, 27, 181, 49, 253, 112, 23, 73,
    88, 208, 251, 100, 242, 69, 83, 48, 10, 235, 63, 71, 79, 85, 5, 236, 164, 12, 159, 24, 156,
    130, 152, 250, 106, 56, 191, 197, 35, 82, 162, 246, 214, 240, 118, 201, 94, 62, 134, 179, 125,
    207, 120, 50, 128, 31, 14, 45, 136, 108, 219, 200, 151, 72, 51, 15, 105, 89, 133, 223, 247,
    129, 30, 205, 150, 6, 139, 58, 111, 220, 123, 167, 91, 29, 8, 81, 37, 148, 245, 18, 190, 0,
    210, 54, 172, 211, 60, 97, 114, 244, 13, 170, 77, 66, 189, 222, 104, 224, 119, 96, 115, 153,
    122, 233, 40, 232, 103, 147, 99, 229, 47, 163, 234, 252, 209, 34, 145, 25, 80, 78, 41, 86, 160,
    166, 203, 213, 33, 144, 217, 192, 87, 39, 42, 21, 26, 113, 28, 157, 126, 196, 254, 142, 20,
    165, 64, 238, 135, 3, 185, 183, 202, 239, 61, 84, 195, 32, 168, 53, 176, 107, 124, 137, 216,
    161, 215, 75, 16, 98, 206, 121, 149, 188, 230,
];
const GRAD: [[f64; 3]; 12] = [
    [1., 1., 0.],
    [-1., 1., 0.],
    [1., -1., 0.],
    [-1., -1., 0.],
    [1., 0., 1.],
    [-1., 0., 1.],
    [1., 0., -1.],
    [-1., 0., -1.],
    [0., 1., 1.],
    [0., -1., 1.],
    [0., 1., -1.],
    [0., -1., -1.],
];
fn p(i: i64, seed: u8) -> i64 {
    (if seed == 2 { PERM2 } else { PERM })[(i & 255) as usize] as i64
}
pub fn simplex(x: f64, y: f64, z: f64) -> f64 {
    simplex_seed(x, y, z, 1)
}
pub fn simplex_seed(x: f64, y: f64, z: f64, seed: u8) -> f64 {
    let skew = (x + y + z) / 3.;
    let i = (x + skew).floor() as i64;
    let j = (y + skew).floor() as i64;
    let k = (z + skew).floor() as i64;
    let t = (i + j + k) as f64 / 6.;
    let a = x - i as f64 + t;
    let b = y - j as f64 + t;
    let c = z - k as f64 + t;
    let (first, second) = if a >= b {
        if b >= c {
            ([1, 0, 0], [1, 1, 0])
        } else if a >= c {
            ([1, 0, 0], [1, 0, 1])
        } else {
            ([0, 0, 1], [1, 0, 1])
        }
    } else if b < c {
        ([0, 0, 1], [0, 1, 1])
    } else if a < c {
        ([0, 1, 0], [0, 1, 1])
    } else {
        ([0, 1, 0], [1, 1, 0])
    };
    let offsets = [[0, 0, 0], first, second, [1, 1, 1]];
    let mut sum = 0.;
    for (index, [di, dj, dk]) in offsets.into_iter().enumerate() {
        let shift = index as f64 / 6.;
        let dx = a - di as f64 + shift;
        let dy = b - dj as f64 + shift;
        let dz = c - dk as f64 + shift;
        let mut q = 0.6 - dx * dx - dy * dy - dz * dz;
        if q < 0. {
            continue;
        }
        let g = GRAD[(p(i + di + p(j + dj + p(k + dk, seed), seed), seed) % 12) as usize];
        q *= q;
        sum += q * q * (g[0] * dx + g[1] * dy + g[2] * dz);
    }
    32. * sum
}
/// Height in world metres. The source's six-unit scale is applied once.
pub fn height(x: f64, z: f64) -> f64 {
    let (mut amplitude, mut frequency, mut total, mut normalization) = (1., 1., 0., 0.);
    for _ in 0..10 {
        total += (simplex(x / 300. * frequency, 0., z / 300. * frequency) * 0.5 + 0.5) * amplitude;
        normalization += amplitude;
        amplitude *= 2_f64.powf(-0.5);
        frequency *= 1.6;
    }
    (total / normalization).powf(7.5) * (800. / 6.)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_upstream_seed_one() {
        for (x, z, expected) in [
            (0.0, 0.0, 0.736569563735987),
            (1.0, 1.0, 0.7342184041878568),
            (10.0, 20.0, 0.15256843541036047),
            (-10.0, -20.0, 2.3792317194758743),
            (80.0, -50.0, 5.27133569089374),
            (250.0, 320.0, 0.42701217917858086),
            (-500.0, 400.0, 0.30526885565738576),
            (1000.0, -1000.0, 0.04911379445711341),
            (0.13, -0.28, 0.929551149657892),
        ] {
            assert!((height(x, z) - expected).abs() < 1e-10, "{x},{z}");
        }
    }
}

/// The source scenery/biome noise uses the fixed seed-two permutation.
pub fn noise(
    x: f64,
    y: f64,
    z: f64,
    seed: u8,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
    exponent: f64,
    scale: f64,
    multiplier: f64,
) -> f64 {
    let (mut amplitude, mut frequency, mut total, mut normalization) = (1., 1., 0., 0.);
    for _ in 0..octaves {
        total += (simplex_seed(
            x / scale * frequency,
            y / scale * frequency,
            z / scale * frequency,
            seed,
        ) * 0.5
            + 0.5)
            * amplitude;
        normalization += amplitude;
        amplitude *= 2_f64.powf(-persistence);
        frequency *= lacunarity;
    }
    (total / normalization).powf(exponent) * multiplier
}
