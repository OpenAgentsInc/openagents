//! Under the water (`docs/verse/water.md`, phase W7): what the physical
//! renderer's frame carries so every lit surface and the sky know about
//! the water around and above them.
//!
//! - [`EyeSurface`] and [`line`]: the surface over the eye from the CPU
//!   query (`physics::water` or [`super::Water::surface_height`]), as a
//!   tangent plane, turned into the height of each near-plane point over
//!   the water. That height is affine across the screen, so three numbers
//!   split every pixel into above and below, and the line where it is zero
//!   is the waterline when the near plane straddles the surface.
//! - [`extents`] and [`list_rows`]: the caustic and underwater list, each
//!   body's wet bounds and a plane through its rest levels, so a surface
//!   lit under any body, not only the sea, takes its caustics and the
//!   sun's extinction down to it.
//! - [`wave_rows`]: the caustics' waves, the frame's own detail waves,
//!   which bend the drawn surface's normals.
//! - [`Submersion`]: how much of the view is under water, eased over a few
//!   frames for the sound and the motes.
//!
//! Caustics use the refracted light field's area ratio: light entering a
//! wavy surface lands on a point below with intensity in proportion to the
//! area it left over the area it reaches (Wallace, "Rendering Realtime
//! Caustics in WebGL", 2016), here from the detail waves' curvature in
//! closed form at the point the refracted sun ray entered, then attenuated
//! by depth and blocked by the sun's shadow map (Guardado and
//! Sánchez-Crespo, "Rendering Water Caustics", GPU Gems, chapter 2, 2004).

use glam::{Mat4, Vec2, Vec3};
use verse_engine::quality::Tier;

use super::MAX_BODIES;
use super::frame::{Water, WaterVertex};

/// Rows of the frame's caustic and underwater list: three a body.
pub const LIST_ROWS: usize = 3 * MAX_BODIES;
/// The most caustic waves a frame carries, two rows each.
pub const MAX_WAVES: usize = 8;
/// Rows of the frame's caustic waves.
pub const WAVE_ROWS: usize = 2 * MAX_WAVES;
/// The shortest and longest detail waves the caustics take, m: shorter
/// ones fold into noise a meter down, longer ones barely focus.
pub const WAVELENGTHS: (f32, f32) = (0.25, 1.25);
/// How far past a body's wet rest vertices its list bounds reach, m.
pub const PAD: f32 = 0.75;
/// A list entry's residual for the sea, whose depth the shader takes
/// from its own spells (`water_depth`).
pub const SEA: f32 = -1.0;

/// What a tier draws under the water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Bodies in the caustic and underwater list.
    pub list: usize,
    /// Caustic waves, and the layers they split into.
    pub waves: usize,
    pub layers: usize,
    /// Samples along the view for the sun's shafts; zero for none.
    pub shafts: u32,
    /// How far the view under water wavers, in screen fractions; zero for
    /// none.
    pub distortion: f32,
    /// Whether the waterline draws its meniscus.
    pub meniscus: bool,
}

/// What `tier` draws under the water: on Low, fog, the split waterline,
/// and four-wave caustics, with no pass or target; Medium six caustic
/// waves, distortion, eight-sample shafts, and the waterline's meniscus;
/// High eight waves in two layers and sixteen samples. The spec's table
/// keeps the meniscus for High; the issue gives it to Medium too, and so
/// does this.
#[must_use]
pub fn settings(tier: Tier) -> Settings {
    match tier {
        Tier::Low => Settings {
            list: 4,
            waves: 4,
            layers: 1,
            shafts: 0,
            distortion: 0.0,
            meniscus: false,
        },
        Tier::Medium => Settings {
            list: MAX_BODIES,
            waves: 6,
            layers: 1,
            shafts: 8,
            distortion: 0.0025,
            meniscus: true,
        },
        Tier::High => Settings {
            list: MAX_BODIES,
            waves: 8,
            layers: 2,
            shafts: 16,
            distortion: 0.0025,
            meniscus: true,
        },
    }
}

/// The water's surface over the eye, from the CPU surface query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeSurface {
    /// The body the eye stands in or over ([`Water::bodies`]).
    pub body: usize,
    /// The surface's height over the eye, m.
    pub height: f32,
    /// Its slope there, dh/dx and dh/dz.
    pub slope: [f32; 2],
}

impl EyeSurface {
    /// The sea's surface over `eye`, from [`Water::surface_height`] with
    /// central differences 0.25 m apart.
    #[must_use]
    pub fn of_sea(water: &Water, eye: Vec3) -> Self {
        let h = |x: f32, z: f32| water.surface_height(x, z, f32::INFINITY);
        let e = 0.25;
        Self {
            body: 0,
            height: h(eye.x, eye.z),
            slope: [
                (h(eye.x + e, eye.z) - h(eye.x - e, eye.z)) / (2.0 * e),
                (h(eye.x, eye.z + e) - h(eye.x, eye.z - e)) / (2.0 * e),
            ],
        }
    }

    /// Whether every value is finite and the body is one of `count`.
    #[must_use]
    pub fn valid(&self, count: usize) -> bool {
        self.body < count
            && self.height.is_finite()
            && self.slope.iter().all(|s| s.is_finite() && s.abs() < 10.0)
    }
}

/// The eye is this far over or under the surface at most for the frame
/// to split the view, m: farther, the whole view lies on one side.
pub const SPLIT_REACH: f32 = 2.0;

/// The waterline across the screen: `[a, b, c, 1]` such that the point
/// of the near plane at normalized device coordinates `(x, y)` stands
/// `a + b·x + c·y` m over the surface's tangent plane at the eye, so a
/// pixel looks out from under the water where that is negative.
/// `view_proj` is the view's world-to-clip matrix with depth from 0 at
/// the near plane (not reversed). Points of one plane perpendicular to
/// the view share a clip w, so the near plane's points are affine in
/// `(x, y)` and three samples give the line exactly.
#[must_use]
pub fn line(view_proj: Mat4, eye: Vec3, surface: &EyeSurface) -> [f32; 4] {
    let inverse = view_proj.inverse();
    let slope = Vec2::from_array(surface.slope);
    let over = |x: f32, y: f32| {
        let p = inverse.project_point3(Vec3::new(x, y, 0.0));
        p.y - (surface.height + slope.dot(Vec2::new(p.x - eye.x, p.z - eye.z)))
    };
    let a = over(0.0, 0.0);
    let b = over(1.0, 0.0) - a;
    let c = over(0.0, 1.0) - a;
    if [a, b, c].iter().all(|v| v.is_finite()) {
        [a, b, c, 1.0]
    } else {
        [0.0; 4]
    }
}

/// The share of the screen [`line`] puts under the water, 0 to 1, from a
/// 16 × 16 grid of pixel centers.
#[must_use]
pub fn under_share(line: [f32; 4]) -> f32 {
    if line[3] < 0.5 {
        return 0.0;
    }
    let n = 16;
    let mut under = 0;
    for j in 0..n {
        for i in 0..n {
            let x = (i as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let y = (j as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            if line[0] + line[1] * x + line[2] * y < 0.0 {
                under += 1;
            }
        }
    }
    under as f32 / (n * n) as f32
}

/// How much of the view is under water, eased so the sound and the motes
/// come and go over a few frames instead of in one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Submersion {
    /// 0 above water, 1 fully under.
    pub amount: f32,
}

impl Submersion {
    /// The time constant of the easing, s.
    pub const EASE: f32 = 0.18;

    /// Eases toward `share` over `dt` s and returns the amount.
    pub fn step(&mut self, share: f32, dt: f32) -> f32 {
        let share = if share.is_finite() {
            share.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let k = 1.0 - (-dt.max(0.0) / Self::EASE).exp();
        self.amount += (share - self.amount) * k;
        self.amount = self.amount.clamp(0.0, 1.0);
        self.amount
    }
}

/// Where a body's water stands: the bounds of its wet rest vertices,
/// padded by [`PAD`], and the least-squares plane through their heights,
/// `y = plane[0] + plane[1]·x + plane[2]·z`, with the largest distance of
/// any of them from it. A pond is flat; a stream falls along its course,
/// and the residual keeps its banks out of its water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    pub min: Vec2,
    pub max: Vec2,
    pub plane: [f32; 3],
    pub residual: f32,
}

impl Extent {
    /// The plane's height at `(x, z)`, m.
    #[must_use]
    pub fn level_at(&self, x: f32, z: f32) -> f32 {
        self.plane[0] + self.plane[1] * x + self.plane[2] * z
    }
}

/// Each body's [`Extent`], from the wet vertices of its seas, ponds, and
/// streams (not falls or orbs).
#[must_use]
pub fn extents(vertices: &[WaterVertex]) -> [Option<Extent>; MAX_BODIES] {
    // Sums over centered coordinates, so a zone far from the origin keeps
    // its precision: count, x, z, y, xx, xz, zz, xy, zy.
    let mut first: [Option<Vec3>; MAX_BODIES] = [None; MAX_BODIES];
    let mut sums = [[0.0_f64; 9]; MAX_BODIES];
    let mut bounds: [Option<(Vec2, Vec2)>; MAX_BODIES] = [None; MAX_BODIES];
    let wet = |v: &WaterVertex| {
        v.kind < 2.99 && v.depth > 0.0 && v.body.is_finite() && Vec3::from(v.pos).is_finite()
    };
    for v in vertices.iter().filter(|v| wet(v)) {
        let k = (v.body.max(0.0) as usize).min(MAX_BODIES - 1);
        let p = Vec3::from(v.pos);
        let o = *first[k].get_or_insert(p);
        let (x, y, z) = (
            f64::from(p.x - o.x),
            f64::from(p.y - o.y),
            f64::from(p.z - o.z),
        );
        let s = &mut sums[k];
        for (slot, term) in s
            .iter_mut()
            .zip([1.0, x, z, y, x * x, x * z, z * z, x * y, z * y])
        {
            *slot += term;
        }
        let q = Vec2::new(p.x, p.z);
        bounds[k] = Some(bounds[k].map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
    }
    let mut out = [None; MAX_BODIES];
    for k in 0..MAX_BODIES {
        let (Some(o), Some((lo, hi))) = (first[k], bounds[k]) else {
            continue;
        };
        let [n, sx, sz, sy, sxx, sxz, szz, sxy, szy] = sums[k];
        let (mx, mz, my) = (sx / n, sz / n, sy / n);
        let (cxx, cxz, czz) = (sxx / n - mx * mx, sxz / n - mx * mz, szz / n - mz * mz);
        let (cxy, czy) = (sxy / n - mx * my, szy / n - mz * my);
        let det = cxx * czz - cxz * cxz;
        // A line of vertices or a single one has no tilt to fit.
        let (b, c) = if det.abs() > 1e-9 {
            ((cxy * czz - czy * cxz) / det, (czy * cxx - cxy * cxz) / det)
        } else {
            (0.0, 0.0)
        };
        let a = my - b * mx - c * mz;
        let plane = [
            (a + f64::from(o.y) - b * f64::from(o.x) - c * f64::from(o.z)) as f32,
            b as f32,
            c as f32,
        ];
        let mut extent = Extent {
            min: lo - Vec2::splat(PAD),
            max: hi + Vec2::splat(PAD),
            plane,
            residual: 0.0,
        };
        extent.residual = vertices
            .iter()
            .filter(|v| wet(v) && (v.body.max(0.0) as usize).min(MAX_BODIES - 1) == k)
            .map(|v| (v.pos[1] - extent.level_at(v.pos[0], v.pos[2])).abs())
            .fold(0.0, f32::max);
        out[k] = Some(extent);
    }
    out
}

/// The frame's caustic and underwater list and how many bodies it holds:
/// per body its bounds (min x, min z, max x, max z); its plane, risen by
/// a flood's height over the level it was built at, and residual ([`SEA`]
/// for the sea, whose bounds are unbounded); and its absorption with the
/// caustics' strength. At most `settings.list` bodies, nearest the eye
/// first when there are more.
#[must_use]
pub fn list_rows(
    water: &Water,
    extents: &[Option<Extent>; MAX_BODIES],
    settings: &Settings,
    eye: Vec3,
) -> ([[f32; 4]; LIST_ROWS], usize) {
    let mut rows = [[0.0; 4]; LIST_ROWS];
    let mut entries: Vec<(f32, [[f32; 4]; 3])> = Vec::new();
    for (k, body) in water.bodies[..water.count.min(MAX_BODIES)]
        .iter()
        .enumerate()
    {
        let a = body.absorption;
        let optics = [a[0], a[1], a[2], water.caustics];
        let rise = body.level - body.rest;
        if k == 0 && water.sea {
            let far = 1.0e6;
            entries.push((
                0.0,
                [[-far, -far, far, far], [body.level, 0.0, 0.0, SEA], optics],
            ));
            continue;
        }
        let Some(e) = extents[k] else {
            continue;
        };
        let near = Vec2::new(eye.x, eye.z).clamp(e.min, e.max);
        let distance = near.distance(Vec2::new(eye.x, eye.z));
        entries.push((
            distance,
            [
                [e.min.x, e.min.y, e.max.x, e.max.y],
                [
                    e.plane[0] + rise,
                    e.plane[1] + body.level_gradient[0],
                    e.plane[2] + body.level_gradient[1],
                    e.residual,
                ],
                optics,
            ],
        ));
    }
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));
    let count = entries.len().min(settings.list).min(MAX_BODIES);
    for (i, (_, entry)) in entries.iter().take(count).enumerate() {
        rows[i * 3..i * 3 + 3].copy_from_slice(entry);
    }
    (rows, count)
}

/// Waves for a stage with no detail of its own: directions, wavelengths
/// (m), and slopes (amplitude over wavelength).
const FALLBACK: [([f32; 2], f32, f32); MAX_WAVES] = [
    ([0.80, 0.60], 1.20, 0.012),
    ([-0.45, 0.89], 0.95, 0.012),
    ([0.10, -0.99], 0.76, 0.013),
    ([-0.93, -0.36], 0.61, 0.012),
    ([0.62, -0.78], 0.49, 0.011),
    ([0.99, 0.14], 0.39, 0.011),
    ([-0.30, -0.95], 0.31, 0.010),
    ([-0.71, 0.70], 0.26, 0.010),
];

/// The caustics' waves: per wave its direction (x, z), wavenumber
/// (rad/m), and amplitude (m); then its angular frequency (rad/s), phase
/// (rad), and two zeros. They are the frame's detail waves between
/// [`WAVELENGTHS`], which bend the drawn surface's normals, spread evenly
/// over that band, or a fixed set when it has none. With two layers, the
/// shader takes the even waves for the first and the odd for the second.
#[must_use]
pub fn wave_rows(water: &Water, settings: &Settings) -> ([[f32; 4]; WAVE_ROWS], usize) {
    let mut rows = [[0.0; 4]; WAVE_ROWS];
    let (short, long) = WAVELENGTHS;
    let mut band: Vec<_> = water.detail[..water.detail_count.min(water.detail.len())]
        .iter()
        .filter(|w| (short..=long).contains(&w.wavelength))
        .copied()
        .collect();
    band.sort_by(|a, b| b.wavelength.total_cmp(&a.wavelength));
    let want = settings.waves.min(MAX_WAVES);
    let mut picked = Vec::with_capacity(want);
    if band.len() >= want {
        for i in 0..want {
            picked.push(band[i * band.len() / want.max(1)]);
        }
    } else {
        for i in 0..want {
            let (dir, wavelength, slope) = FALLBACK[i * FALLBACK.len() / want];
            picked.push(super::frame::Wave {
                dir,
                wavelength,
                amplitude: wavelength * slope,
                phase: i as f32 * 1.7,
            });
        }
    }
    for (i, w) in picked.iter().enumerate() {
        rows[i * 2] = [w.dir[0], w.dir[1], w.k(), w.amplitude];
        rows[i * 2 + 1] = [w.omega(), w.phase, 0.0, 0.0];
    }
    (rows, picked.len())
}

/// The analytic caustics' focusing at `world`, `depth` m under a surface
/// lit from `sun` (unit, toward it), as the shader's `water_caustic`
/// computes it from `rows`' first `count` waves split into `layers` at
/// water clock `time`: the area ratio of the refracted light field, each
/// layer's multiplied. Tests and tools read it; the shader keeps its own
/// copy.
#[must_use]
pub fn focus(
    rows: &[[f32; 4]; WAVE_ROWS],
    count: usize,
    layers: usize,
    world: Vec3,
    depth: f32,
    sun: Vec3,
    time: f32,
) -> f32 {
    let d = refracted_sun(sun);
    let p = Vec2::new(world.x, world.z) - Vec2::new(d.x, d.z) * (depth / (-d.y).max(0.2));
    let lever = (1.0 - 1.0 / 1.33) * depth.min(LEVER_DEPTH);
    let layers = layers.clamp(1, 2);
    let product: f32 = (0..layers)
        .map(|layer| layer_focus(rows, count, layer, layers, p, lever, time))
        .product();
    product.min(8.0)
}

fn layer_focus(
    rows: &[[f32; 4]; WAVE_ROWS],
    count: usize,
    layer: usize,
    layers: usize,
    p: Vec2,
    lever: f32,
    time: f32,
) -> f32 {
    let (mut hxx, mut hzz, mut hxz, mut variance) = (0.0, 0.0, 0.0, 0.0);
    for i in (0..count.min(MAX_WAVES)).filter(|i| i % layers == layer) {
        let a = rows[i * 2];
        let b = rows[i * 2 + 1];
        let k = a[2];
        let theta = k * (a[0] * p.x + a[1] * p.y) - b[0] * time + b[1];
        let curvature = a[3] * k * k;
        let m = -curvature * theta.sin();
        hxx += m * a[0] * a[0];
        hzz += m * a[1] * a[1];
        hxz += m * a[0] * a[1];
        variance += 0.5 * curvature * curvature;
    }
    let det = (1.0 + lever * hxx) * (1.0 + lever * hzz) - lever * lever * hxz * hxz;
    // The area ratio read at the light's source rather than where it lands
    // averages 1 + lever²·E[tr H²] to second order (Jensen's inequality);
    // dividing that out keeps the bed's mean light where it was.
    (1.0 / det.abs().max(FOCUS_FLOOR)).min(FOCUS_CEILING) / (1.0 + lever * lever * variance)
}

/// The depth past which refraction's lever stops growing, m: deeper water
/// folds the light field so often that its caustics wash out to the mean.
pub const LEVER_DEPTH: f32 = 3.5;
/// The area ratio's floor and the focus's ceiling, which keep a fold's
/// infinite intensity finite.
pub const FOCUS_FLOOR: f32 = 0.12;
pub const FOCUS_CEILING: f32 = 6.0;

/// The sun's light under a level surface, travelling down: `sun` (unit,
/// toward the sun) refracted into water of index 1.33 (Snell's law).
#[must_use]
pub fn refracted_sun(sun: Vec3) -> Vec3 {
    let i = -sun.normalize_or(Vec3::Y);
    let n = Vec3::Y;
    let eta = 1.0 / 1.33;
    let cos_i = -n.dot(i);
    let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
    if k < 0.0 {
        return Vec3::NEG_Y;
    }
    (eta * i + (eta * cos_i - k.sqrt()) * n).normalize_or(Vec3::NEG_Y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water::frame::{Kind, Water, WaterVertex};

    fn view(eye: Vec3, pitch: f32) -> Mat4 {
        let dir = Vec3::new(0.0, pitch.sin(), -pitch.cos());
        let proj = Mat4::perspective_rh(1.0, 16.0 / 10.0, 0.1, 500.0);
        proj * Mat4::look_to_rh(eye, dir, Vec3::Y)
    }

    #[test]
    fn a_level_eye_at_the_surface_splits_the_view_at_its_middle() {
        let surface = EyeSurface {
            body: 0,
            height: 1.0,
            slope: [0.0, 0.0],
        };
        let line = line(
            view(Vec3::new(3.0, 1.0, 2.0), 0.0),
            Vec3::new(3.0, 1.0, 2.0),
            &surface,
        );
        assert_eq!(line[3], 1.0);
        assert!(line[0].abs() < 1e-4, "{line:?}");
        assert!(line[1].abs() < 1e-4, "level: no tilt across: {line:?}");
        assert!(
            line[2] > 0.0,
            "the top of the near plane is above: {line:?}"
        );
        assert!((under_share(line) - 0.5).abs() < 0.07);
    }

    #[test]
    fn the_split_follows_the_eye_through_the_surface() {
        let surface = EyeSurface {
            body: 0,
            height: 0.0,
            slope: [0.0, 0.0],
        };
        let at = |y: f32| {
            let eye = Vec3::new(0.0, y, 0.0);
            under_share(line(view(eye, 0.0), eye, &surface))
        };
        assert_eq!(at(0.5), 0.0, "well above");
        assert_eq!(at(-0.5), 1.0, "well under");
        let (above, below) = (at(0.02), at(-0.02));
        assert!(above < 0.5 && above > 0.0, "{above}");
        assert!(below > 0.5 && below < 1.0, "{below}");
    }

    #[test]
    fn a_tilted_surface_tilts_the_line() {
        let eye = Vec3::ZERO;
        let surface = EyeSurface {
            body: 0,
            height: 0.0,
            slope: [0.2, 0.0],
        };
        let line = line(view(eye, 0.0), eye, &surface);
        assert!(line[1] < -1e-3, "the surface rises to the right: {line:?}");
    }

    #[test]
    fn submersion_eases_toward_the_share_under() {
        let mut s = Submersion::default();
        let mut last = 0.0;
        for _ in 0..30 {
            let now = s.step(1.0, 1.0 / 60.0);
            assert!(now > last);
            last = now;
        }
        assert!(last > 0.9 && last <= 1.0);
        assert_eq!(s.step(f32::NAN, 1.0), s.amount);
    }

    fn patch(body: usize, level: impl Fn(f32, f32) -> f32) -> Vec<WaterVertex> {
        let mut out = Vec::new();
        for j in 0..9 {
            for i in 0..9 {
                let (x, z) = (10.0 + i as f32, -4.0 + j as f32);
                let depth = if i == 0 { -0.2 } else { 1.0 };
                out.push(
                    WaterVertex::new(Vec3::new(x, level(x, z), z), depth, Kind::Body(0.0))
                        .in_body(body),
                );
            }
        }
        out
    }

    #[test]
    fn a_pond_is_flat_and_a_stream_falls_along_its_course() {
        let mut v = patch(1, |_, _| 2.5);
        v.extend(patch(2, |x, _| 1.0 - 0.05 * x));
        let e = extents(&v);
        assert!(e[0].is_none());
        let pond = e[1].unwrap();
        assert!((pond.level_at(14.0, 0.0) - 2.5).abs() < 1e-4);
        assert!(pond.residual < 1e-4);
        assert!(
            (pond.min.x - (11.0 - PAD)).abs() < 1e-4,
            "dry vertices stay out"
        );
        let stream = e[2].unwrap();
        assert!((stream.plane[1] + 0.05).abs() < 1e-4, "{stream:?}");
        assert!((stream.level_at(12.0, 3.0) - 0.4).abs() < 1e-3);
    }

    #[test]
    fn the_list_holds_the_sea_first_and_the_nearest_bodies_within_the_tier() {
        let mut water = Water::calm(0.0);
        water.sea = true;
        for _ in 0..5 {
            water.add(water.bodies[0]);
        }
        water.bodies[0].level = 0.3;
        let mut e = [None; MAX_BODIES];
        for (k, slot) in e.iter_mut().enumerate().skip(1).take(5) {
            let at = Vec2::new(k as f32 * 100.0, 0.0);
            *slot = Some(Extent {
                min: at - Vec2::ONE,
                max: at + Vec2::ONE,
                plane: [k as f32, 0.0, 0.0],
                residual: 0.0,
            });
        }
        let (rows, count) = list_rows(&water, &e, &settings(Tier::Low), Vec3::new(300.0, 0.0, 0.0));
        assert_eq!(count, 4);
        assert_eq!(rows[1], [0.3, 0.0, 0.0, SEA]);
        assert_eq!(rows[4][0], 3.0, "then the body around the eye");
        let (_, all) = list_rows(&water, &e, &settings(Tier::High), Vec3::ZERO);
        assert_eq!(all, 6);
    }

    #[test]
    fn caustic_waves_come_from_the_detail_band_and_average_one() {
        let mut water = Water::calm(0.0);
        water.set_detail(0.6, 0.08, 1.6, 0.012);
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let s = settings(tier);
            let (rows, count) = wave_rows(&water, &s);
            assert_eq!(count, s.waves);
            for i in 0..count {
                let wavelength = std::f32::consts::TAU / rows[i * 2][2];
                assert!(
                    (WAVELENGTHS.0 - 1e-3..=WAVELENGTHS.1 + 1e-3).contains(&wavelength),
                    "{wavelength}"
                );
            }
            // Light is neither made nor lost: the area ratio over a patch of
            // bed averages near one.
            let sun = Vec3::new(0.3, 0.9, 0.2).normalize();
            let n = 64;
            let mut sum = 0.0;
            for j in 0..n {
                for i in 0..n {
                    let p = Vec3::new(i as f32 * 0.07, -1.0, j as f32 * 0.07);
                    sum += focus(&rows, count, s.layers, p, 1.0, sun, 3.0);
                }
            }
            let mean = sum / (n * n) as f32;
            assert!((0.8..1.35).contains(&mean), "{tier:?}: {mean}");
        }
        let bare = Water {
            detail_count: 0,
            ..Water::calm(0.0)
        };
        assert_eq!(wave_rows(&bare, &settings(Tier::High)).1, MAX_WAVES);
    }

    #[test]
    fn sunlight_bends_toward_the_vertical_under_water() {
        let sun = Vec3::new(1.0, 1.0, 0.0).normalize();
        let d = refracted_sun(sun);
        assert!(d.y < 0.0);
        let angle = (-d.y).acos();
        assert!((angle.sin() - (45.0_f32.to_radians().sin() / 1.33)).abs() < 1e-4);
    }
}
