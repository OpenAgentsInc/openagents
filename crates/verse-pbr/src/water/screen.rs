//! Water in screen space on Medium and High (`docs/verse/water.md`, phase
//! W5): what a tier copies and traces, the planar mirror's camera, and the
//! choice of the body it mirrors. `screen.wgsl` is the shader half, spliced
//! after the shared `water.wgsl`; `copy.wgsl` writes the depth copy.
//!
//! On Medium and High a frame with water or particles draws its opaque
//! scene, resolves it into a single-sample color copy, writes a depth copy
//! (each pixel's view depth, the clip w, in meters), and then draws the
//! water, glass, lines, and particles over the multisampled scene again,
//! reading the copies for refraction, absorption, contact foam, reflection,
//! and the particles' soft fade. Before all of it, a planar mirror draws
//! the opaque scene once more from the eye reflected in the nearest
//! visible flat body's plane. Low adds no pass and no render target: its
//! water keeps the two-half path, the transmitted half multiplying what
//! lies behind it.
//!
//! The mirror's camera clips at the water's plane by replacing its
//! projection's depth row with the plane, as Lengyel's oblique near plane
//! does ("Oblique View Frustum Depth Projection and Clipping", JGT 2005),
//! here in reversed depth: depth along any ray still falls monotonically
//! with distance, so the depth test orders the mirrored scene correctly.

use glam::{Mat4, Vec3, Vec4};
use verse_engine::quality::Tier;

use super::{Body, MAX_BODIES};

/// The shader half, spliced after [`super::SHARED`].
pub const SHADER: &str = include_str!("screen.wgsl");
/// The depth copy's shader: a full-screen triangle writing each pixel's
/// view depth, or [`FAR`] where nothing was drawn. `// VERSE_DEPTH_TEXTURE`
/// becomes the depth texture's type.
pub const COPY: &str = include_str!("copy.wgsl");

/// The depth copy's format: one half float a pixel, renderable on every
/// backend Medium and High run on. It keeps view depth to about a
/// thousandth of itself (6 cm at 100 m), finer than the contact foam's
/// band or the trace's thickness at that distance.
pub const DEPTH_COPY: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
/// The depth copy's bytes a pixel.
const DEPTH_COPY_BYTES: u64 = 2;
/// The depth copy's value where nothing was drawn (the sky), m: within
/// the half float's range.
pub const FAR: f32 = 60_000.0;

/// The copy shader for a depth buffer of `samples` samples.
#[must_use]
pub fn copy_source(samples: u32) -> String {
    COPY.replace(
        "// VERSE_DEPTH_TEXTURE",
        if samples > 1 {
            "alias DepthTexture = texture_depth_multisampled_2d;"
        } else {
            "alias DepthTexture = texture_depth_2d;"
        },
    )
}

/// What a tier copies and traces for its water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Whether the scene's color and depth are copied before the water.
    pub copies: bool,
    /// The planar mirror's size as a divisor of the view's: 2 for half
    /// resolution, 1 for full; 0 for none.
    pub mirror_divisor: u32,
    /// Steps of the screen-space reflection's march; 0 for none.
    pub ssr_steps: u32,
    /// How far refraction spreads the channels (dispersion), 0 to 1.
    pub dispersion: f32,
    /// Particles fade over this much depth before what they meet, m; 0
    /// for no fade.
    pub soft_particles: f32,
}

impl Plan {
    /// The plan for `tier`.
    #[must_use]
    pub const fn of(tier: Tier) -> Self {
        match tier {
            Tier::Low => Self {
                copies: false,
                mirror_divisor: 0,
                ssr_steps: 0,
                dispersion: 0.0,
                soft_particles: 0.0,
            },
            Tier::Medium => Self {
                copies: true,
                mirror_divisor: 2,
                ssr_steps: 0,
                dispersion: 0.0,
                soft_particles: 0.6,
            },
            Tier::High => Self {
                copies: true,
                mirror_divisor: 1,
                ssr_steps: 32,
                dispersion: 1.0,
                soft_particles: 0.6,
            },
        }
    }

    /// Drops optional optics without changing the surface's fallback.
    #[must_use]
    pub fn with_effects(mut self, effects: verse_engine::quality::WaterEffects) -> Self {
        self.copies &= effects.copies();
        if !effects.mirror() {
            self.mirror_divisor = 0;
        }
        if !effects.ssr() {
            self.ssr_steps = 0;
        }
        self
    }

    /// The mirror's size for a view `width` by `height`.
    #[must_use]
    pub fn mirror_size(&self, width: u32, height: u32) -> Option<[u32; 2]> {
        (self.copies && self.mirror_divisor > 0).then(|| {
            [
                width.div_ceil(self.mirror_divisor).max(1),
                height.div_ceil(self.mirror_divisor).max(1),
            ]
        })
    }

    /// The bytes the plan's targets hold for a view `width` by `height`
    /// whose scene format takes `scene_bytes` a pixel: the color copy, the
    /// depth copy, and the mirror's color and depth.
    #[must_use]
    pub fn bytes(&self, width: u32, height: u32, scene_bytes: u64) -> u64 {
        if !self.copies {
            return 0;
        }
        let pixels = u64::from(width) * u64::from(height);
        // The mirror's color and its 32-bit depth buffer.
        let mirror = self
            .mirror_size(width, height)
            .map_or(0, |[w, h]| u64::from(w) * u64::from(h) * (scene_bytes + 4));
        pixels * (scene_bytes + DEPTH_COPY_BYTES) + mirror
    }

    /// The frame uniform's `water_screen` for this plan, when the copies
    /// hold this frame's scene: copies on, march steps, dispersion, and
    /// the particles' fade depth.
    #[must_use]
    pub fn uniform(&self, copied: bool) -> [f32; 4] {
        if !copied || !self.copies {
            return [0.0; 4];
        }
        [
            1.0,
            self.ssr_steps as f32,
            self.dispersion,
            self.soft_particles,
        ]
    }
}

/// A body's surface is flat enough to mirror when its swell rises less
/// than this, m.
pub const FLAT_SWELL: f32 = 0.12;
/// A body's rest vertices must lie within this height of each other to
/// share one plane, m.
pub const FLAT_SPREAD: f32 = 0.08;
/// The eye must stand at least this far above a plane to mirror in it, m.
pub const MIRROR_CLEARANCE: f32 = 0.05;

/// Where a body's surface lies, from its rest vertices
/// ([`super::SurfaceGpu::bounds`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

/// Whether `body`, its rest vertices within `bounds`, is one plane.
#[must_use]
pub fn flat(body: &Body, bounds: &Bounds) -> bool {
    let rise: f32 = body.swell.terms[..body.swell.count.min(body.swell.terms.len())]
        .iter()
        .map(|t| t.amplitude.abs())
        .sum::<f32>()
        * body.swell_gain.abs();
    rise < FLAT_SWELL && bounds.max.y - bounds.min.y < FLAT_SPREAD
}

/// The body to mirror: of the first `count` bodies, the flat ones the eye
/// stands above whose surfaces lie in view, the nearest to the eye.
/// `view_proj` is the view's (not reversed).
#[must_use]
pub fn pick(
    bodies: &[Body],
    bounds: &[Option<Bounds>; MAX_BODIES],
    view_proj: Mat4,
    eye: Vec3,
) -> Option<usize> {
    bodies
        .iter()
        .take(MAX_BODIES)
        .enumerate()
        .filter_map(|(i, body)| {
            let b = bounds[i]?;
            let above = eye.y - body.level > MIRROR_CLEARANCE;
            let lo = b.min.with_y(body.level - 0.01);
            let hi = b.max.with_y(body.level + 0.01);
            let seen = crate::pbr::textured::drawn(lo, hi, view_proj, eye, f32::INFINITY);
            (above && !body.eye_inside && flat(body, &b) && seen)
                .then(|| (i, eye.clamp(lo, hi).distance_squared(eye)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The planar mirror's camera for water at `level`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mirror {
    /// World to clip, reversed depth, clipped at the plane: what the
    /// mirror's frame draws with.
    pub view_proj: Mat4,
    /// The mirrored view without the clip plane, reversed depth: its
    /// inverse turns screen positions into rays (the sky's).
    pub unclipped: Mat4,
    /// The mirrored view as the culling tests take it (not reversed).
    pub cull: Mat4,
    /// The eye reflected in the plane.
    pub eye: Vec3,
    pub level: f32,
}

/// How strongly the clip plane's distance enters the mirror's depth:
/// smaller keeps far reflections in range, larger spreads depth more.
const OBLIQUE: f32 = 0.25;

impl Mirror {
    /// The mirror of the view `view_proj` (not reversed) from `eye` in the
    /// horizontal plane at `level`. `reversed` is the renderer's
    /// reversed-depth matrix ([`crate::pbr::gpu::reversed_depth`]).
    #[must_use]
    pub fn new(view_proj: Mat4, eye: Vec3, level: f32, reversed: Mat4) -> Self {
        // y → 2·level − y.
        let reflect = Mat4::from_cols(
            Vec4::X,
            -Vec4::Y,
            Vec4::Z,
            Vec4::new(0.0, 2.0 * level, 0.0, 1.0),
        );
        let cull = view_proj * reflect;
        let unclipped = reversed * cull;
        // Depth row: w − s·(plane · p), so a point on the plane sits at
        // depth one (the near plane, reversed), one below it fails the
        // clip test, and depth falls with distance along every ray.
        let plane = Vec4::new(0.0, 1.0, 0.0, -level);
        let w_row = unclipped.row(3);
        let z_row = w_row - plane * OBLIQUE;
        let mut rows = [unclipped.row(0), unclipped.row(1), z_row, unclipped.row(3)];
        // Keep the rows finite whatever the input.
        for row in &mut rows {
            if !row.is_finite() {
                *row = Vec4::ZERO;
            }
        }
        let view_proj = Mat4::from_cols(rows[0], rows[1], rows[2], rows[3]).transpose();
        Self {
            view_proj,
            unclipped,
            cull,
            eye: Vec3::new(eye.x, 2.0 * level - eye.y, eye.z),
            level,
        }
    }

    /// The mirrored view's axis, from the view's.
    #[must_use]
    pub fn forward(&self, forward: Vec3) -> Vec3 {
        Vec3::new(forward.x, -forward.y, forward.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water::{Preset, frame::wind_sea};

    fn view() -> (Mat4, Vec3) {
        let eye = Vec3::new(0.0, 3.0, 10.0);
        let vp = Mat4::perspective_rh(0.9, 16.0 / 9.0, 0.1, 400.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
        (vp, eye)
    }

    fn clip(m: Mat4, p: Vec3) -> Vec4 {
        m * p.extend(1.0)
    }

    #[test]
    fn low_copies_nothing_and_adds_no_target() {
        let low = Plan::of(Tier::Low);
        assert!(!low.copies);
        assert_eq!(low.bytes(1920, 1080, 8), 0);
        assert_eq!(low.mirror_size(1920, 1080), None);
        assert_eq!(low.uniform(true), [0.0; 4]);
    }

    #[test]
    fn medium_mirrors_at_half_size_and_high_at_full_with_a_march() {
        let medium = Plan::of(Tier::Medium);
        let high = Plan::of(Tier::High);
        assert_eq!(medium.mirror_size(1920, 1080), Some([960, 540]));
        assert_eq!(high.mirror_size(1920, 1080), Some([1920, 1080]));
        assert_eq!(medium.ssr_steps, 0);
        assert!(high.ssr_steps > 0);
        assert_eq!(medium.uniform(false), [0.0; 4]);
    }

    #[test]
    fn targets_fit_the_specification_budgets_at_1080p() {
        // `docs/verse/water.md`, Budgets per tier: 32 MiB on Medium, 96 on
        // High, for an eight-byte scene format (the larger of the two).
        let mib = 1024 * 1024;
        assert!(Plan::of(Tier::Medium).bytes(1920, 1080, 8) <= 32 * mib);
        assert!(Plan::of(Tier::High).bytes(1920, 1080, 8) <= 96 * mib);
    }

    #[test]
    fn the_mirror_keeps_what_stands_above_the_plane_and_clips_what_lies_below() {
        let (vp, eye) = view();
        let reversed = crate::pbr::gpu::reversed_depth();
        let level = 0.5;
        let m = Mirror::new(vp, eye, level, reversed);
        assert_eq!(m.eye, Vec3::new(0.0, -2.0, 10.0));
        // A point above the water, in front of the eye: inside the clip
        // volume (reversed depth, 0 ≤ z ≤ w).
        for p in [
            Vec3::new(0.0, 1.5, 0.0),
            Vec3::new(1.0, 4.0, -20.0),
            Vec3::new(-3.0, 0.6, 2.0),
        ] {
            let c = clip(m.view_proj, p);
            assert!(c.w > 0.0 && c.z >= 0.0 && c.z <= c.w, "{p}: {c}");
        }
        // Below the water: clipped by the plane.
        for p in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, -3.0, -10.0)] {
            let c = clip(m.view_proj, p);
            assert!(c.z > c.w, "{p}: {c}");
        }
        // The screen position is the plain mirrored view's: the oblique
        // row changes depth only.
        let p = Vec3::new(1.0, 2.0, -4.0);
        let a = clip(m.view_proj, p);
        let b = clip(m.unclipped, p);
        assert!((a.truncate().truncate() / a.w - b.truncate().truncate() / b.w).length() < 1e-5);
    }

    #[test]
    fn the_depth_copy_validates_for_one_and_four_samples() {
        for samples in [1, 4] {
            let source = copy_source(samples);
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
        }
    }

    #[test]
    fn mirror_depth_falls_with_distance_along_a_ray() {
        let (vp, eye) = view();
        let m = Mirror::new(vp, eye, 0.0, crate::pbr::gpu::reversed_depth());
        let dir = Vec3::new(0.1, 0.3, -1.0).normalize();
        let mut last = f32::INFINITY;
        for step in 1..40 {
            let p = m.eye + dir * (step as f32 * 2.5);
            if p.y <= 0.0 {
                continue;
            }
            let c = clip(m.view_proj, p);
            let depth = c.z / c.w;
            assert!(depth < last, "depth must fall with distance");
            last = depth;
        }
    }

    #[test]
    fn the_nearest_flat_body_in_view_is_mirrored() {
        let (vp, eye) = view();
        let preset = Preset::default();
        let near = Body::still(0.0, &preset);
        let far = Body::still(0.2, &preset);
        let mut waves = Body::still(0.0, &preset);
        waves.swell = crate::water::Swell::from_set(&wind_sea(0.0, 20.0, 1.2, 6));
        let bounds = |x: f32, z: f32, y: f32| {
            Some(Bounds {
                min: Vec3::new(x - 2.0, y, z - 2.0),
                max: Vec3::new(x + 2.0, y, z + 2.0),
            })
        };
        let mut b = [None; MAX_BODIES];
        b[0] = bounds(0.0, 0.0, 0.0);
        b[1] = bounds(0.0, -40.0, 0.2);
        b[2] = bounds(0.0, 4.0, 0.0);
        assert_eq!(pick(&[near, far, waves], &b, vp, eye), Some(0));
        // Behind the eye, no body is in view.
        let mut behind = [None; MAX_BODIES];
        behind[0] = bounds(0.0, 40.0, 0.0);
        assert_eq!(pick(&[near], &behind, vp, eye), None);
        // A wavy sea is never mirrored.
        assert_eq!(pick(&[waves], &b, vp, eye), None);
        // Nor water the eye is under.
        assert_eq!(pick(&[near], &b, vp, Vec3::new(0.0, -1.0, 10.0)), None);
    }
}
