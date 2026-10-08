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
    /// Uses the legacy surface color in the scene's display scale.
    pub lit: bool,
    /// Takes the scene's sun, ambient, shadow, and local lights.
    pub scene_lit: bool,
    /// Optical density for scene lighting, from 0 to 8.
    pub density: f32,
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
    /// Frame blend, sheet layer, additive, and light mode: 0 emitted,
    /// 1 legacy surface color, or 2 plus density for scene lighting.
    pub params: [f32; 4],
    /// Quad center (xyz) and particle half width (w), in meters.
    /// Negative width marks a projected ground quad that skips soft fade.
    pub center_half: [f32; 4],
}

/// Sprites a frame draws at `tier`; the rest are dropped by priority and
/// size. Each is six 76-byte vertices, so the high tier's 2048, enough for
/// an eight-meteor swarm's trails and blasts together, cost about 934 KB a
/// frame.
#[must_use]
pub fn budget(tier: Tier) -> usize {
    match tier {
        Tier::Low => 160,
        Tier::Medium => 768,
        Tier::High => 2048,
    }
}

/// Builds `sprites`' quads for a camera at `eye`, at most `budget` of
/// them, back to front, into `out` (cleared first).
pub fn vertices(sprites: &[Sprite], eye: Vec3, budget: usize, out: &mut Vec<SpriteVertex>) {
    vertices_with_ribbons(sprites, &[], eye, budget, out);
}

/// A connected, camera-facing trail, sharing the particle atlas and pass.
#[derive(Clone, Debug, PartialEq)]
pub struct Ribbon {
    /// Ordered from the hot head to the cooling tail.
    pub points: Vec<RibbonPoint>,
    pub layer: u32,
    pub rect: [f32; 4],
    pub priority: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RibbonPoint {
    pub at: Vec3,
    pub half: f32,
    pub color: [f32; 3],
    pub alpha: f32,
}

/// Builds sprites and connected ribbon segments under one priority budget.
pub fn vertices_with_ribbons(
    sprites: &[Sprite],
    ribbons: &[Ribbon],
    eye: Vec3,
    budget: usize,
    out: &mut Vec<SpriteVertex>,
) {
    out.clear();
    // Each ribbon uses common edges at its joints. Keeping the segment in
    // the same sort as sprites lets smoke cover it without an extra pass.
    let mut segments = Vec::new();
    for ribbon in ribbons {
        let points = &ribbon.points;
        if points.len() < 2
            || points.iter().any(|p| {
                !p.at.is_finite()
                    || !p.half.is_finite()
                    || !p.alpha.is_finite()
                    || p.color.iter().any(|c| !c.is_finite())
            })
        {
            continue;
        }
        let mut sides = Vec::with_capacity(points.len());
        for (i, p) in points.iter().enumerate() {
            let a = points[i.saturating_sub(1)].at;
            let b = points[(i + 1).min(points.len() - 1)].at;
            let tangent = (b - a).normalize_or(Vec3::Y);
            let toward = (eye - p.at).normalize_or(Vec3::Z);
            let mut side = tangent.cross(toward).normalize_or(Vec3::X);
            if sides
                .last()
                .is_some_and(|previous: &Vec3| previous.dot(side) < 0.0)
            {
                side = -side;
            }
            sides.push(side);
        }
        for i in 0..points.len() - 1 {
            let a = points[i];
            let b = points[i + 1];
            if a.at.distance_squared(b.at) < 1e-8 || a.alpha.max(b.alpha) < 1e-3 {
                continue;
            }
            let center = (a.at + b.at) * 0.5;
            let near =
                ((eye.distance(center) - a.half.max(b.half) * 0.5 - 0.3) / 1.5).clamp(0.0, 1.0);
            if near <= 0.0 {
                continue;
            }
            let vertex = |p: RibbonPoint, side: Vec3, sign: f32, _u: f32| {
                let r = ribbon.rect;
                let uv = [
                    r[0] + (r[2] - r[0]) * 0.5,
                    r[1] + (r[3] - r[1]) * (sign * 0.5 + 0.5),
                ];
                SpriteVertex {
                    pos: (p.at + side * p.half.max(0.0) * sign).to_array(),
                    color: [
                        p.color[0] * near,
                        p.color[1] * near,
                        p.color[2] * near,
                        p.alpha.clamp(0.0, 1.0) * near,
                    ],
                    uv_a: uv,
                    uv_b: uv,
                    params: [0.0, ribbon.layer as f32, 1.0, 0.0],
                    center_half: center.extend(a.half.max(b.half)).to_array(),
                }
            };
            let (al, ar) = (
                vertex(a, sides[i], -1.0, 0.0),
                vertex(a, sides[i], 1.0, 0.0),
            );
            let (bl, br) = (
                vertex(b, sides[i + 1], -1.0, 1.0),
                vertex(b, sides[i + 1], 1.0, 1.0),
            );
            segments.push((
                center,
                a.half.max(b.half) + a.at.distance(b.at),
                ribbon.priority,
                [al, ar, br, al, br, bl],
            ));
        }
    }
    let mut order: Vec<(usize, f32, u8, f32)> = sprites
        .iter()
        .enumerate()
        .filter(|(_, s)| visible(s))
        .map(|(i, s)| {
            (
                i,
                eye.distance_squared(s.at),
                s.priority,
                s.half + s.tail.length(),
            )
        })
        .chain(
            segments
                .iter()
                .enumerate()
                .map(|(i, (at, reach, priority, _))| {
                    (
                        sprites.len() + i,
                        eye.distance_squared(*at),
                        *priority,
                        *reach,
                    )
                }),
        )
        .collect();
    if order.len() > budget {
        order.select_nth_unstable_by_key(budget, |&(i, d, priority, reach)| {
            (
                std::cmp::Reverse(priority),
                std::cmp::Reverse(ordered(reach / d.sqrt().max(0.1))),
                i,
            )
        });
        order.truncate(budget);
    }
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out.reserve(order.len() * 6);
    for (i, _, _, _) in order {
        if i < sprites.len() {
            quad(&sprites[i], eye, out);
        } else {
            out.extend_from_slice(&segments[i - sprites.len()].3);
        }
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
        if s.scene_lit {
            2.0 + if s.density.is_finite() {
                s.density.clamp(0.0, 8.0)
            } else {
                1.0
            }
        } else if s.lit {
            1.0
        } else {
            0.0
        },
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
            center_half: center
                .extend(if s.facing == Facing::Ground {
                    -s.half
                } else {
                    s.half
                })
                .to_array(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite() -> Sprite {
        Sprite {
            at: Vec3::ZERO,
            half: 0.5,
            angle: 0.0,
            tail: Vec3::X * 4.0,
            facing: Facing::Camera,
            color: [1.0; 3],
            alpha: 1.0,
            additive: 0.0,
            lit: false,
            scene_lit: false,
            density: 1.0,
            layer: 1,
            rect_a: [0.0, 0.0, 1.0, 1.0],
            rect_b: [0.0, 0.0, 1.0, 1.0],
            mix: 0.0,
            priority: 1,
        }
    }

    #[test]
    fn scene_density_zero_is_distinct_from_legacy_and_emitted_color() {
        let mut s = sprite();
        let mut vertices = Vec::new();
        for (legacy, scene, density, mode) in [
            (false, false, 1.0, 0.0),
            (true, false, 1.0, 1.0),
            (false, true, 0.0, 2.0),
            (true, true, 1.5, 3.5),
            (false, true, 50.0, 10.0),
            (false, true, -1.0, 2.0),
            (false, true, f32::NAN, 3.0),
        ] {
            s.lit = legacy;
            s.scene_lit = scene;
            s.density = density;
            vertices.clear();
            quad(&s, Vec3::Z * 10.0, &mut vertices);
            assert_eq!(vertices.len(), 6);
            assert!(vertices.iter().all(|vertex| vertex.params[3] == mode));
        }
    }

    #[test]
    fn lighting_samples_the_quad_center_even_when_a_tail_moves_it() {
        let s = sprite();
        let mut vertices = Vec::new();
        quad(&s, Vec3::Z * 10.0, &mut vertices);
        assert!(
            vertices
                .iter()
                .all(|v| v.center_half == [2.0, 0.0, 0.0, 0.5])
        );
        assert_eq!(std::mem::size_of::<SpriteVertex>(), 76);
    }

    #[test]
    fn projected_ground_quads_skip_soft_fade_without_an_extra_vertex_field() {
        let mut s = sprite();
        s.facing = Facing::Ground;
        let mut vertices = Vec::new();
        quad(&s, Vec3::Z * 10.0, &mut vertices);
        assert!(vertices.iter().all(|v| v.center_half[3] == -s.half));
        s.facing = Facing::Camera;
        vertices.clear();
        quad(&s, Vec3::Z * 10.0, &mut vertices);
        assert!(vertices.iter().all(|v| v.center_half[3] == s.half));
    }
}

#[cfg(test)]
mod ribbon_tests {
    use super::*;
    #[test]
    fn connected_joints_share_edges_and_fit_the_sprite_budget() {
        let ribbon = Ribbon {
            points: vec![
                Vec3::ZERO,
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(2.0, 1.0, 0.0),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, at)| RibbonPoint {
                at,
                half: 0.2 + i as f32 * 0.1,
                color: [3.0, 1.0, 0.1],
                alpha: 1.0 - i as f32 * 0.3,
            })
            .collect(),
            layer: 2,
            rect: [0.0, 0.0, 1.0, 1.0],
            priority: 8,
        };
        let mut out = Vec::new();
        vertices_with_ribbons(&[], &[ribbon.clone()], Vec3::Z * 20.0, 10, &mut out);
        assert_eq!(out.len(), 12);
        // Depth ties keep the source order, and the next quad reuses both edges.
        let first = [out[0].pos, out[1].pos, out[2].pos, out[5].pos];
        let second = [out[6].pos, out[7].pos, out[8].pos, out[11].pos];
        assert_eq!(first.iter().filter(|p| second.contains(p)).count(), 2);
        vertices_with_ribbons(&[], &[ribbon], Vec3::Z * 20.0, 1, &mut out);
        assert_eq!(out.len(), 6);
    }
}
