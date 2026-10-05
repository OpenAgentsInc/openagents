//! Sprites: what a frame asks the renderer to draw, one per particle, and
//! the quads the renderer builds from them.
//!
//! A frame's sprites are kept within the quality tier's [`budget`], highest
//! priority and largest on screen first, then drawn back to front in one
//! premultiplied-alpha pass. Additive particles write no alpha, so they
//! add light in any order; alpha particles cover what's behind them, which
//! the back-to-front order gets right. One blend state draws both, and a
//! particle can move between them over its life (fire cooling into smoke).

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use verse_engine::quality::Tier;

/// Which way a sprite's quad faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Camera,
    Ground,
}

/// One particle to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    pub at: Vec3,
    /// Half the quad's width, m.
    pub half: f32,
    /// Turn in the quad's plane, radians.
    pub angle: f32,
    /// From the head back along the trail, m; zero for a round particle.
    pub tail: Vec3,
    pub facing: Facing,
    /// Emitted luminance (cd/m² before exposure), or a lit surface color.
    pub color: [f32; 3],
    pub alpha: f32,
    /// 1 adds light, 0 covers like smoke, between mixes.
    pub additive: f32,
    pub lit: bool,
    /// The sheet's texture-array layer.
    pub layer: u32,
    /// The two frames' texture rectangles and how far between them.
    pub rect_a: [f32; 4],
    pub rect_b: [f32; 4],
    pub mix: f32,
    pub priority: u8,
}

/// One vertex of a sprite quad.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct SpriteVertex {
    pub pos: [f32; 3],
    /// RGB, and alpha.
    pub color: [f32; 4],
    pub uv_a: [f32; 2],
    pub uv_b: [f32; 2],
    /// Frame blend, sheet layer, additive, and 1 for lit.
    pub params: [f32; 4],
}

/// Sprites a frame draws at `tier`; the rest are dropped by priority and
/// size. Each is six 60-byte vertices, so the high tier's 1536 cost about
/// 550 KB a frame.
#[must_use]
pub fn budget(tier: Tier) -> usize {
    match tier {
        Tier::Low => 160,
        Tier::Medium => 768,
        Tier::High => 1536,
    }
}

/// Builds `sprites`' quads for a camera at `eye`, at most `budget` of
/// them, back to front, into `out` (cleared first).
pub fn vertices(sprites: &[Sprite], eye: Vec3, budget: usize, out: &mut Vec<SpriteVertex>) {
    out.clear();
    let mut order: Vec<(usize, f32)> = sprites
        .iter()
        .enumerate()
        .filter(|(_, s)| visible(s))
        .map(|(i, s)| (i, eye.distance_squared(s.at)))
        .collect();
    if order.len() > budget {
        // Keep the highest priority, then the largest on screen; ties by
        // index, so the choice is the same every run.
        let key = |&(i, d): &(usize, f32)| {
            let s = &sprites[i];
            let reach = (s.half + s.tail.length()) / d.sqrt().max(0.1);
            (
                std::cmp::Reverse(s.priority),
                std::cmp::Reverse(ordered(reach)),
                i,
            )
        };
        order.select_nth_unstable_by_key(budget, key);
        order.truncate(budget);
    }
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out.reserve(order.len() * 6);
    for (i, _) in order {
        quad(&sprites[i], eye, out);
    }
}

fn ordered(x: f32) -> u32 {
    // Non-negative floats order like their bits.
    x.max(0.0).to_bits()
}

fn visible(s: &Sprite) -> bool {
    s.half > 1e-4
        && s.alpha > 1e-3
        && s.at.is_finite()
        && s.tail.is_finite()
        && s.color.iter().all(|c| c.is_finite())
        && (s.color.iter().any(|c| *c > 0.0) || s.additive < 1.0)
}

fn quad(s: &Sprite, eye: Vec3, out: &mut Vec<SpriteVertex>) {
    // A sprite brushing the camera would fill the view; it fades out
    // instead.
    let near = ((eye.distance(s.at) - s.half * 0.5 - 0.3) / 1.5).clamp(0.0, 1.0);
    if near <= 0.0 {
        return;
    }
    let (center, right, up) = match s.facing {
        Facing::Ground => {
            let (sin, cos) = s.angle.sin_cos();
            (
                s.at,
                Vec3::new(cos, 0.0, sin) * s.half,
                Vec3::new(-sin, 0.0, cos) * s.half,
            )
        }
        Facing::Camera => {
            let toward = (eye - s.at).normalize_or(Vec3::Z);
            let along = s.tail - toward * s.tail.dot(toward);
            if along.length() > s.half * 0.05 {
                // Laid along the flight: the sheet's horizontal axis runs
                // from the head back along the tail, rounded at both ends.
                let dir = along.normalize();
                let side = toward.cross(dir).normalize_or(Vec3::Y) * s.half;
                let length = along.length() * 0.5 + s.half;
                (s.at + along * 0.5, dir * length, side)
            } else {
                let right = Vec3::Y.cross(toward).normalize_or(Vec3::X);
                let up = toward.cross(right);
                let (sin, cos) = s.angle.sin_cos();
                (
                    s.at,
                    (right * cos + up * sin) * s.half,
                    (up * cos - right * sin) * s.half,
                )
            }
        }
    };
    let color = [
        s.color[0] * near,
        s.color[1] * near,
        s.color[2] * near,
        s.alpha * near,
    ];
    let params = [
        s.mix.clamp(0.0, 1.0),
        s.layer as f32,
        s.additive.clamp(0.0, 1.0),
        if s.lit { 1.0 } else { 0.0 },
    ];
    let uv = |r: [f32; 4], x: f32, y: f32| {
        [
            r[0] + (r[2] - r[0]) * (x * 0.5 + 0.5),
            r[1] + (r[3] - r[1]) * (0.5 - y * 0.5),
        ]
    };
    for (x, y) in [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ] {
        out.push(SpriteVertex {
            pos: (center + right * x + up * y).to_array(),
            color,
            uv_a: uv(s.rect_a, x, y),
            uv_b: uv(s.rect_b, x, y),
            params,
        });
    }
}
