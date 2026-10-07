//! Distant light from a sky: its radiance projected onto order-two spherical
//! harmonics for diffuse light, and prefiltered into a small cube map with
//! one GGX lobe per mip level for glossy reflections.
//!
//! The renderer supplies the sky as a function of direction and runs these
//! once, when the sky or the Sun changes; nothing here touches a GPU. The
//! projection follows Ramamoorthi and Hanrahan, "An Efficient Representation
//! for Irradiance Environment Maps" (SIGGRAPH 2001), and the prefilter
//! follows the split-sum approximation in Karis, "Real Shading in Unreal
//! Engine 4" (SIGGRAPH 2013).
//!
//! Cube faces follow the order and orientation every wgpu backend samples:
//! +X, −X, +Y, −Y, +Z, −Z, with each face's texel rows stored from the top.

use std::f32::consts::PI;

use glam::Vec3;

/// Normalization constants of the real spherical-harmonic basis, bands 0 to
/// 2, in the order [`basis`] returns its polynomials.
const K: [f32; 9] = [
    0.282_095, 0.488_603, 0.488_603, 0.488_603, 1.092_548, 1.092_548, 0.315_392, 1.092_548,
    0.546_274,
];

/// The clamped-cosine kernel's weight per band: π, 2π/3, and π/4.
const A: [f32; 9] = [
    PI,
    2.0 * PI / 3.0,
    2.0 * PI / 3.0,
    2.0 * PI / 3.0,
    PI / 4.0,
    PI / 4.0,
    PI / 4.0,
    PI / 4.0,
    PI / 4.0,
];

/// The basis polynomials at unit direction `d`, without their constants:
/// 1, y, z, x, xy, yz, 3z² − 1, xz, and x² − y². The lit shader evaluates
/// the same nine terms.
#[must_use]
pub fn basis(d: Vec3) -> [f32; 9] {
    [
        1.0,
        d.y,
        d.z,
        d.x,
        d.x * d.y,
        d.y * d.z,
        3.0 * d.z * d.z - 1.0,
        d.x * d.z,
        d.x * d.x - d.y * d.y,
    ]
}

/// Radiance over the sphere as order-two spherical harmonics: nine
/// coefficients per color channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sh9 {
    /// The projected radiance coefficients, in [`basis`] order.
    pub radiance: [Vec3; 9],
}

impl Sh9 {
    /// Projects `radiance` by sampling it at the texel centers of a cube with
    /// `face_size` texels along each edge, weighting each by its solid angle.
    ///
    /// The cube's samples are symmetric under every axis reflection, so a
    /// uniform sky projects onto band 0 alone.
    #[must_use]
    pub fn project(face_size: u32, radiance: impl Fn(Vec3) -> Vec3) -> Self {
        let size = face_size.max(1);
        let mut sum = [Vec3::ZERO; 9];
        let mut total = 0.0;
        for face in 0..6 {
            for y in 0..size {
                for x in 0..size {
                    let d = texel_direction(face, size, x, y);
                    let weight = texel_solid_angle(size, x, y);
                    let light = radiance(d) * weight;
                    for ((coefficient, k), p) in sum.iter_mut().zip(K).zip(basis(d)) {
                        *coefficient += light * (k * p);
                    }
                    total += weight;
                }
            }
        }
        // The solid angles sum to 4π up to rounding; normalizing keeps a
        // uniform sky exact.
        let scale = 4.0 * PI / total;
        Self {
            radiance: sum.map(|c| c * scale),
        }
    }

    /// Irradiance on a surface facing `n`: the projected radiance convolved
    /// with the clamped cosine lobe. Order-two ringing can dip below zero
    /// opposite a bright source; the result is clamped there.
    #[must_use]
    pub fn irradiance(&self, n: Vec3) -> Vec3 {
        let p = basis(n.normalize_or(Vec3::Y));
        let e = self
            .radiance
            .iter()
            .zip(A)
            .zip(K)
            .zip(p)
            .fold(Vec3::ZERO, |e, (((c, a), k), p)| e + *c * (a * k * p));
        e.max(Vec3::ZERO)
    }

    /// Every coefficient times `scale`.
    #[must_use]
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            radiance: self.radiance.map(|c| c * scale),
        }
    }

    /// The irradiance coefficients as the lit shader reads them: each band's
    /// cosine weight and basis constant folded in, so irradiance is the dot
    /// product of these with [`basis`], per channel.
    #[must_use]
    pub fn uniform(&self) -> [[f32; 4]; 9] {
        std::array::from_fn(|i| (self.radiance[i] * (A[i] * K[i])).extend(0.0).to_array())
    }
}

/// The unit direction through cube face `face` at face coordinates `s`
/// (right) and `t` (down), each in [−1, 1].
#[must_use]
pub fn face_direction(face: usize, s: f32, t: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -t, -s),
        1 => Vec3::new(-1.0, -t, s),
        2 => Vec3::new(s, 1.0, t),
        3 => Vec3::new(s, -1.0, -t),
        4 => Vec3::new(s, -t, 1.0),
        _ => Vec3::new(-s, -t, -1.0),
    }
    .normalize()
}

/// The direction through the center of texel (`x`, `y`) of a face with
/// `size` texels along each edge.
#[must_use]
pub fn texel_direction(face: usize, size: u32, x: u32, y: u32) -> Vec3 {
    let size = size.max(1) as f32;
    let s = 2.0 * (x as f32 + 0.5) / size - 1.0;
    let t = 2.0 * (y as f32 + 0.5) / size - 1.0;
    face_direction(face, s, t)
}

/// The solid angle texel (`x`, `y`) subtends, steradians: the difference of
/// the projected-area integral `atan2(st, √(s² + t² + 1))` at its corners.
#[must_use]
pub fn texel_solid_angle(size: u32, x: u32, y: u32) -> f32 {
    let step = 2.0 / size.max(1) as f32;
    let area = |s: f32, t: f32| (s * t).atan2((s * s + t * t + 1.0).sqrt());
    let (s0, t0) = (x as f32 * step - 1.0, y as f32 * step - 1.0);
    let (s1, t1) = (s0 + step, t0 + step);
    area(s0, t0) - area(s0, t1) - area(s1, t0) + area(s1, t1)
}

/// A prefiltered environment cube: level 0 holds the radiance itself, and
/// each further level halves the edge and blurs by a wider GGX lobe, up to
/// perceptual roughness 1 at the last.
#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    /// Texels along level 0's edge.
    pub size: u32,
    /// Per level, six faces of `side²` texels, face by face, each face's rows
    /// from the top.
    pub levels: Vec<Vec<Vec3>>,
}

impl Cube {
    /// The level count of a full mip chain for `size`: down to one texel.
    #[must_use]
    pub fn level_count(size: u32) -> u32 {
        32 - size.max(1).leading_zeros()
    }

    /// The edge length of `level`.
    #[must_use]
    pub fn side(&self, level: usize) -> u32 {
        (self.size >> level).max(1)
    }

    /// The perceptual roughness level `level` of `count` holds. The shader
    /// reads its level of detail as roughness × (count − 1).
    #[must_use]
    pub fn roughness(level: usize, count: usize) -> f32 {
        if count <= 1 {
            0.0
        } else {
            level as f32 / (count - 1) as f32
        }
    }

    /// Prefilters `radiance` into a cube of edge `size` (a power of two),
    /// averaging `samples` GGX-distributed directions per texel above level
    /// 0. The sky is evaluated directly at each sample, so no level reads
    /// another and a smooth sky needs few samples.
    #[must_use]
    pub fn prefilter(size: u32, samples: u32, radiance: impl Fn(Vec3) -> Vec3) -> Self {
        let mut run = Prefilter::new(size, samples);
        while !run.advance(u64::MAX, &radiance) {}
        run.into_cube()
    }
}

/// [`Cube::prefilter`] a slice at a time, so a bake can spread over frames:
/// each [`Prefilter::advance`] evaluates the radiance about as many times
/// as its budget allows, and the finished cube is the one
/// [`Cube::prefilter`] gives.
#[derive(Clone, Debug)]
pub struct Prefilter {
    size: u32,
    samples: u32,
    count: usize,
    levels: Vec<Vec<Vec3>>,
}

impl Prefilter {
    /// A prefilter of edge `size` (rounded up to a power of two) with
    /// `samples` GGX samples per blurred texel, not yet started.
    #[must_use]
    pub fn new(size: u32, samples: u32) -> Self {
        let size = size.max(1).next_power_of_two();
        Self {
            size,
            samples: samples.max(1),
            count: Cube::level_count(size) as usize,
            levels: Vec::new(),
        }
    }

    /// Radiance evaluations the whole prefilter takes.
    #[must_use]
    pub fn cost(&self) -> u64 {
        (0..self.count)
            .map(|level| {
                let side = u64::from((self.size >> level).max(1));
                6 * side
                    * side
                    * if level == 0 {
                        1
                    } else {
                        u64::from(self.samples)
                    }
            })
            .sum()
    }

    /// Whether every level is filled.
    #[must_use]
    pub fn done(&self) -> bool {
        self.levels.len() == self.count
            && self
                .levels
                .last()
                .is_some_and(|l| l.len() == self.texels(self.count - 1))
    }

    fn texels(&self, level: usize) -> usize {
        let side = (self.size >> level).max(1) as usize;
        6 * side * side
    }

    /// Fills texels until about `budget` radiance evaluations are spent,
    /// finishing at least one texel. Returns whether the cube is done.
    pub fn advance(&mut self, budget: u64, radiance: &impl Fn(Vec3) -> Vec3) -> bool {
        let mut spent = 0_u64;
        while !self.done() {
            if self
                .levels
                .last()
                .is_none_or(|l| l.len() == self.texels(self.levels.len() - 1))
            {
                let level = self.levels.len();
                self.levels.push(Vec::with_capacity(self.texels(level)));
            }
            let level = self.levels.len() - 1;
            let side = (self.size >> level).max(1);
            let roughness = Cube::roughness(level, self.count);
            let texels = &mut self.levels[level];
            let i = texels.len() as u32;
            let (face, y, x) = (i / (side * side), i / side % side, i % side);
            let n = texel_direction(face as usize, side, x, y);
            if roughness <= 0.0 {
                texels.push(radiance(n));
                spent += 1;
            } else {
                texels.push(ggx_average(n, roughness, self.samples, radiance));
                spent += u64::from(self.samples);
            }
            if spent >= budget {
                break;
            }
        }
        self.done()
    }

    /// The finished cube; levels not yet filled are left short.
    #[must_use]
    pub fn into_cube(self) -> Cube {
        Cube {
            size: self.size,
            levels: self.levels,
        }
    }
}

/// The radiance a GGX lobe of `roughness` around `n` gathers, with the view
/// along the normal and each sample weighted by its cosine (Karis 2013).
fn ggx_average(n: Vec3, roughness: f32, samples: u32, radiance: &impl Fn(Vec3) -> Vec3) -> Vec3 {
    let a = roughness * roughness;
    let up = if n.y.abs() < 0.999 { Vec3::Y } else { Vec3::X };
    let tx = up.cross(n).normalize();
    let ty = n.cross(tx);
    let mut sum = Vec3::ZERO;
    let mut weight = 0.0;
    for i in 0..samples {
        let (u, v) = hammersley(i, samples);
        let phi = 2.0 * PI * u;
        let cos_theta = ((1.0 - v) / (1.0 + (a * a - 1.0) * v)).max(0.0).sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let h = tx * (sin_theta * phi.cos()) + ty * (sin_theta * phi.sin()) + n * cos_theta;
        let l = h * (2.0 * n.dot(h)) - n;
        let nol = n.dot(l);
        if nol > 0.0 {
            sum += radiance(l) * nol;
            weight += nol;
        }
    }
    if weight > 0.0 {
        sum / weight
    } else {
        radiance(n)
    }
}

/// Point `i` of an `n`-point Hammersley set on the unit square.
fn hammersley(i: u32, n: u32) -> (f32, f32) {
    (
        i as f32 / n as f32,
        i.reverse_bits() as f32 * (1.0 / 4_294_967_296.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texel_solid_angles_cover_the_sphere() {
        for size in [1, 4, 16, 32] {
            let mut total = 0.0;
            for _face in 0..6 {
                for y in 0..size {
                    for x in 0..size {
                        total += texel_solid_angle(size, x, y);
                    }
                }
            }
            assert!((total - 4.0 * PI).abs() < 1e-3, "{size}: {total}");
        }
    }

    #[test]
    fn each_face_center_looks_along_its_axis() {
        let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        for (face, axis) in axes.into_iter().enumerate() {
            assert!(face_direction(face, 0.0, 0.0).distance(axis) < 1e-6);
        }
        // The side faces keep +Y up: their top rows look upward.
        for face in [0, 1, 4, 5] {
            assert!(face_direction(face, 0.0, -0.9).y > 0.5);
        }
    }

    /// A uniform sky projects to a constant: every normal receives π times
    /// its radiance.
    #[test]
    fn a_uniform_sky_projects_to_constant_irradiance() {
        let light = Vec3::new(0.2, 0.5, 1.5);
        let sh = Sh9::project(16, |_| light);
        for i in 1..9 {
            assert!(sh.radiance[i].abs().max_element() < 1e-4, "{i}: {sh:?}");
        }
        for n in [
            Vec3::Y,
            -Vec3::Y,
            Vec3::X,
            Vec3::new(0.3, -0.5, 0.8).normalize(),
            Vec3::new(-1.0, 1.0, 1.0).normalize(),
        ] {
            let e = sh.irradiance(n);
            assert!((e - light * PI).abs().max_element() < 1e-3, "{n}: {e}");
        }
    }

    /// A sky brighter above than below lights up-facing surfaces most,
    /// horizontal ones between, and down-facing ones least; and the order-two
    /// fit matches the exact cosine integral of a linear sky.
    #[test]
    fn a_sky_brighter_above_lights_upward_normals_more() {
        let sky = |d: Vec3| Vec3::splat(1.0 + 0.8 * d.y);
        let sh = Sh9::project(32, sky);
        let up = sh.irradiance(Vec3::Y).x;
        let side = sh.irradiance(Vec3::X).x;
        let down = sh.irradiance(-Vec3::Y).x;
        assert!(up > side && side > down, "{up} {side} {down}");
        // L = 1 + 0.8 y has irradiance π (1 + 0.8 × 2/3 n.y) exactly.
        assert!((up - PI * (1.0 + 0.8 * 2.0 / 3.0)).abs() < 2e-3, "{up}");
        assert!((side - PI).abs() < 2e-3, "{side}");
        // Color follows each channel: a blue zenith over a warm ground.
        let tinted = Sh9::project(16, |d| {
            if d.y > 0.0 {
                Vec3::new(0.2, 0.4, 1.0)
            } else {
                Vec3::new(0.4, 0.3, 0.1)
            }
        });
        let up = tinted.irradiance(Vec3::Y);
        let down = tinted.irradiance(-Vec3::Y);
        assert!(up.z > up.x && down.x > down.z, "{up} {down}");
    }

    #[test]
    fn the_shader_uniform_reproduces_the_irradiance() {
        let sh = Sh9::project(16, |d| Vec3::new(1.0 + d.x, 0.5 + 0.4 * d.y * d.y, 0.3));
        let uniform = sh.uniform();
        for n in [Vec3::Y, Vec3::new(0.6, -0.2, 0.77).normalize()] {
            let p = basis(n);
            let shader = Vec3::new(
                (0..9).map(|i| uniform[i][0] * p[i]).sum(),
                (0..9).map(|i| uniform[i][1] * p[i]).sum(),
                (0..9).map(|i| uniform[i][2] * p[i]).sum(),
            );
            assert!(
                (shader.max(Vec3::ZERO) - sh.irradiance(n))
                    .abs()
                    .max_element()
                    < 1e-5
            );
        }
    }

    #[test]
    fn a_prefilter_in_slices_gives_the_whole_cube() {
        let sky = |d: Vec3| Vec3::new(0.2 + d.y.max(0.0), 0.4, 0.8 - 0.3 * d.x);
        let whole = Cube::prefilter(8, 16, sky);
        let mut run = Prefilter::new(8, 16);
        let budget = 200;
        let mut slices = 0;
        while !run.advance(budget, &sky) {
            slices += 1;
        }
        assert!(slices as u64 >= run.cost() / budget - 1, "{slices}");
        assert_eq!(run.into_cube(), whole);
    }

    #[test]
    fn prefiltering_keeps_a_uniform_sky_and_blurs_a_bright_one() {
        let light = Vec3::new(0.4, 0.6, 0.9);
        let cube = Cube::prefilter(16, 32, |_| light);
        assert_eq!(cube.levels.len(), 5);
        assert_eq!(Cube::roughness(0, 5), 0.0);
        assert_eq!(Cube::roughness(4, 5), 1.0);
        for (level, texels) in cube.levels.iter().enumerate() {
            let side = cube.side(level) as usize;
            assert_eq!(texels.len(), 6 * side * side);
            for t in texels {
                assert!((*t - light).abs().max_element() < 1e-5);
            }
        }
        // A sky lit only above: a rough level spreads light below the
        // horizon that the sharp level keeps above it.
        let cube = Cube::prefilter(8, 64, |d| Vec3::splat(f32::from(u8::from(d.y > 0.0))));
        // −X face, the texel row just below the horizon.
        let below = |level: usize| {
            let side = cube.side(level) as usize;
            cube.levels[level][side * side + (side / 2) * side + side / 2].x
        };
        assert_eq!(below(0), 0.0);
        assert!(below(2) > 0.05, "{}", below(2));
    }
}
