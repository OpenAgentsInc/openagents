//! Feathers of light: sprites from the `feather` sheet, one at a time or
//! fanned into a guardian's tail.
//!
//! A feather is an additive, camera-facing sprite laid along its shaft:
//! the eyespot at the sprite's head and the quill back along its tail, three
//! times as long as it is wide, as the sheet is painted
//! (`scripts/blender/fx/feather.py`). Its color is emitted luminance in
//! cd/m² before exposure, so a bright fan blooms and lights nothing by
//! itself; a scene that wants its light on the ground adds lamps.
//!
//! The Draconia peacock shot draws its drifting feathers and the guardian's
//! fanned tail from here (`docs/verse/draconia/shots.md`); a wand's light
//! spell or shield can draw from it too.

use glam::Vec3;

use super::{Facing, Ribbon, RibbonPoint, Sprite, sheet};

/// A feather's length over its width, as the sheet is painted.
pub const ASPECT: f32 = 3.0;

/// The sheet's frame holding the feather.
const FEATHER: u32 = 0;
/// The sheet's frame holding the soft dot.
const DOT: u32 = 1;
/// The sheet's frame holding the round eyespot.
const EYE: u32 = 2;
/// The sheet's frame holding the flat disc.
const DISC: u32 = 3;

fn from_sheet(frame: u32) -> ([f32; 4], u32) {
    let sheet = sheet::find("feather").unwrap_or(&sheet::SHEETS[0]);
    (
        sheet.rect(frame),
        sheet::layer("feather").unwrap_or_default(),
    )
}

/// One feather `length` m long whose eyespot is at `eyespot`, its shaft
/// running toward `quill` (a direction), glowing `luminance` (cd/m²,
/// per channel) at `alpha`.
#[must_use]
pub fn feather(eyespot: Vec3, quill: Vec3, length: f32, luminance: [f32; 3], alpha: f32) -> Sprite {
    let (rect, layer) = from_sheet(FEATHER);
    let half = length / (2.0 * ASPECT);
    let dir = quill.normalize_or(Vec3::NEG_Y);
    Sprite {
        at: eyespot,
        half,
        angle: 0.0,
        tail: dir * (length - 2.0 * half),
        facing: Facing::Camera,
        color: luminance,
        alpha,
        additive: 1.0,
        lit: false,
        scene_lit: false,
        density: 1.0,
        layer,
        rect_a: rect,
        rect_b: rect,
        mix: 0.0,
        priority: 8,
    }
}

/// A soft round glow of radius `half` m at `at`: a star, a bokeh disc, or
/// the halo round a bright point.
#[must_use]
pub fn dot(at: Vec3, half: f32, luminance: [f32; 3], alpha: f32) -> Sprite {
    let (rect, layer) = from_sheet(DOT);
    Sprite {
        at,
        half,
        tail: Vec3::ZERO,
        rect_a: rect,
        rect_b: rect,
        layer,
        priority: 6,
        ..feather(at, Vec3::NEG_Y, half * 2.0 * ASPECT, luminance, alpha)
    }
}

/// A round eyespot of radius `half` m at `at`: a dark blue pupil, a pale
/// fill, a thick white rim, and a soft halo.
#[must_use]
pub fn eyespot(at: Vec3, half: f32, luminance: [f32; 3]) -> Sprite {
    let (rect, layer) = from_sheet(EYE);
    Sprite {
        rect_a: rect,
        rect_b: rect,
        layer,
        priority: 8,
        ..dot(at, half, luminance, 1.0)
    }
}

/// A feather drawn as strokes of light rather than a sprite, so it stays
/// sharp however close the camera comes: an egg-shaped eyespot (a rim
/// loop round a pale fill), a curved shaft from it through `bend` to
/// `quill`, and fine, sparse barbs swept toward the tip, every one its own
/// thin ribbon. `eye` is the eyespot's width, m; `seed` varies the barbs.
#[must_use]
pub fn fine(
    eyespot_at: Vec3,
    bend: Vec3,
    quill: Vec3,
    eye: f32,
    luminance: [f32; 3],
    seed: u32,
) -> (Vec<Sprite>, Vec<Ribbon>) {
    let at = |u: f32| {
        let v = 1.0 - u;
        eyespot_at * (v * v) + bend * (2.0 * v * u) + quill * (u * u)
    };
    let tangent = |u: f32| {
        ((bend - eyespot_at) * (2.0 * (1.0 - u)) + (quill - bend) * (2.0 * u))
            .normalize_or(Vec3::NEG_Y)
    };
    let plane = (bend - eyespot_at)
        .cross(quill - eyespot_at)
        .normalize_or(Vec3::Z);
    let hash = |i: u32| {
        let mut x = i.wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 13;
        x as f32 / u32::MAX as f32
    };
    let mut ribbons = Vec::new();
    let t0 = tangent(0.0);
    let n0 = plane.cross(t0).normalize_or(Vec3::X);
    // The egg: longer along the shaft, wider toward the tip.
    let (a, b) = (eye * 0.58, eye * 0.5);
    let egg: Vec<Vec3> = (0..=40)
        .map(|i| {
            let th = i as f32 / 40.0 * std::f32::consts::TAU;
            eyespot_at - t0 * (a * th.cos()) + n0 * (b * th.sin() * (1.0 + 0.16 * th.cos()))
        })
        .collect();
    ribbons.push(glow_line(&egg, eye * 0.1, luminance));
    let sprites = vec![
        // The flat fill under the rim, and its soft glow.
        {
            let (rect, layer) = from_sheet(DISC);
            Sprite {
                rect_a: rect,
                rect_b: rect,
                layer,
                priority: 8,
                ..dot(eyespot_at, (a + b) * 0.5, luminance.map(|c| c * 0.36), 1.0)
            }
        },
        dot(eyespot_at, b * 3.0, luminance.map(|c| c * 0.05), 1.0),
    ];
    // The shaft, from the egg's foot to the quill.
    let length = eyespot_at.distance(quill).max(1e-3);
    let start = (a / length).min(0.5);
    let shaft: Vec<Vec3> = (0..=24)
        .map(|i| at(start + (1.0 - start) * i as f32 / 24.0))
        .collect();
    ribbons.push(glow_line(&shaft, eye * 0.03, luminance.map(|c| c * 0.9)));
    // Barbs from the shaft, swept toward the tip, longest round the eye.
    let barb = luminance.map(|c| c * 0.4);
    let per_side = 100;
    for side in [-1.0_f32, 1.0] {
        for i in 0..per_side {
            let k = (i as f32 + hash(i * 2 + side.max(0.0) as u32)) / per_side as f32;
            let u = start * 0.3 + (0.92 - start * 0.3) * k;
            let p = at(u);
            let t = tangent(u);
            let n = plane.cross(t).normalize_or(Vec3::X) * side;
            let reach = eye * (0.25 + 1.5 * (1.0 - u).powf(2.0)) * (0.8 + 0.4 * hash(i + 999));
            let sweep = 0.55 + 0.25 * hash(i + 77);
            let dir = (n * sweep.cos() - t * sweep.sin()).normalize_or(n);
            let end = p + dir * reach - t * reach * 0.12;
            let mut line = glow_line(&[p, end], eye * 0.006, barb);
            if let Some(last) = line.points.last_mut() {
                last.color = [0.0; 3];
            }
            line.priority = 6;
            ribbons.push(line);
        }
    }
    // A crown of barbs round the tip, beyond the egg.
    for i in 0..36 {
        let phi = -1.35 + 2.7 * (i as f32 + 0.5) / 36.0;
        let dir = (-t0 * phi.cos() + n0 * phi.sin()).normalize_or(-t0);
        let from = eyespot_at + dir * (a.max(b) * 1.05);
        let to = from + dir * eye * (0.35 + 0.35 * hash(i + 4242));
        let mut line = glow_line(&[from, to], eye * 0.006, barb);
        if let Some(last) = line.points.last_mut() {
            last.color = [0.0; 3];
        }
        line.priority = 6;
        ribbons.push(line);
    }
    (sprites, ribbons)
}

/// A long feather whose shaft curves: the head (the eyespot and the vane
/// round it) as a sprite, and the shaft and the lower vane as two ribbons
/// along a quadratic curve from `eyespot` through `bend` (a control point)
/// to `quill`, so a feather seen close sweeps in an arc instead of a
/// straight stroke. `head` is the head sprite's length, m.
#[must_use]
pub fn curved(
    eyespot: Vec3,
    bend: Vec3,
    quill: Vec3,
    head: f32,
    luminance: [f32; 3],
) -> (Sprite, [Ribbon; 2]) {
    let at = |u: f32| {
        let v = 1.0 - u;
        eyespot * (v * v) + bend * (2.0 * v * u) + quill * (u * u)
    };
    let tangent = (bend - eyespot).normalize_or(Vec3::NEG_Y);
    let sprite = feather(eyespot, tangent, head, luminance, 1.0);
    let (dot_rect, layer) = from_sheet(DOT);
    let (vane_rect, _) = from_sheet(FEATHER);
    // The vane's cross-section a quarter of the way down a straight
    // feather: barbs either side of the shaft.
    let column = vane_rect[0] + (vane_rect[2] - vane_rect[0]) * 0.3;
    let vane_rect = [column, vane_rect[1], column, vane_rect[3]];
    let width = head / (2.0 * ASPECT);
    let n = 24;
    // The ribbons start inside the head sprite, past its eyespot.
    let start = (head * 0.3) / eyespot.distance(quill).max(1e-3);
    let points = |half: f32, gain: f32| -> Vec<RibbonPoint> {
        (0..=n)
            .map(|i| {
                let u = start + (1.0 - start) * i as f32 / n as f32;
                let fade = (1.0 - u).powf(0.6) * ((u - start) / 0.08).clamp(0.0, 1.0);
                RibbonPoint {
                    at: at(u),
                    half: half * (1.0 - 0.6 * u),
                    color: luminance.map(|c| c * gain * fade),
                    alpha: 1.0,
                }
            })
            .collect()
    };
    (
        sprite,
        [
            Ribbon {
                points: points(width * 0.12, 0.9),
                layer,
                rect: dot_rect,
                priority: 8,
            },
            Ribbon {
                points: points(width * 0.5, 0.12),
                layer,
                rect: vane_rect,
                priority: 7,
            },
        ],
    )
}

/// A faint rainbow arc, as light splits at a bright edge: six bands from
/// red outside to violet inside, round `center` at `radius` m in the plane
/// of `right` and `up`, from angle `from` to `to` (radians).
#[must_use]
pub fn rainbow(
    center: Vec3,
    right: Vec3,
    up: Vec3,
    radius: f32,
    width: f32,
    from: f32,
    to: f32,
    luminance: f32,
) -> Vec<Ribbon> {
    let (rect, layer) = from_sheet(DOT);
    let bands = [
        [1.0, 0.25, 0.2],
        [1.0, 0.6, 0.15],
        [0.9, 0.95, 0.2],
        [0.25, 1.0, 0.35],
        [0.2, 0.5, 1.0],
        [0.55, 0.25, 1.0],
    ];
    let n = 32;
    bands
        .iter()
        .enumerate()
        .map(|(k, color)| {
            let r = radius - width * k as f32 / bands.len() as f32;
            Ribbon {
                points: (0..=n)
                    .map(|i| {
                        let u = i as f32 / n as f32;
                        let a = from + (to - from) * u;
                        let fade = (u * std::f32::consts::PI).sin();
                        RibbonPoint {
                            at: center + (right * a.cos() + up * a.sin()) * r,
                            half: width * 0.45,
                            color: color.map(|c| c * luminance * fade),
                            alpha: 1.0,
                        }
                    })
                    .collect(),
                layer,
                rect,
                priority: 5,
            }
        })
        .collect()
}

/// A stroke of light through `points`, `half` m wide, glowing
/// `luminance`: a peacock's neck or crest, a wand's beam, a rune's line.
#[must_use]
pub fn glow_line(points: &[Vec3], half: f32, luminance: [f32; 3]) -> Ribbon {
    let (rect, layer) = from_sheet(DOT);
    Ribbon {
        points: points
            .iter()
            .map(|&at| RibbonPoint {
                at,
                half,
                color: luminance,
                alpha: 1.0,
            })
            .collect(),
        layer,
        rect,
        priority: 8,
    }
}

/// A fanned tail: rows of feathers growing from `center` in the plane of
/// `right` and `up`, their eyespots on arcs out to `radius` m.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fan {
    pub center: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// The outermost eyespots' distance from the center, m.
    pub radius: f32,
    /// Rows of eyespots, outermost first, each nearer the center.
    pub rows: u32,
    /// About how many feathers in all.
    pub feathers: u32,
    /// How far below the horizontal the fan reaches at each side, radians.
    pub droop: f32,
    /// The eyespots' luminance, cd/m².
    pub luminance: [f32; 3],
    /// The vanes' and shafts' share of that luminance, 0 to 1: lower keeps
    /// a dense fan from washing into one sheet of light, so its eyespots
    /// read as rows of bright points.
    pub vane: f32,
    /// The feathers' width as a share of the sheet's, 0 to 1: narrower
    /// feathers have egg-shaped eyespots and leave darker gaps between.
    pub slim: f32,
    /// How far the fan has opened, 0 folded to 1 fully fanned.
    pub open: f32,
    /// Seconds, for a slow shimmer.
    pub time: f32,
}

impl Fan {
    /// The fan's feathers, outermost row first, plus a soft halo behind
    /// each eyespot.
    #[must_use]
    pub fn sprites(&self) -> Vec<Sprite> {
        let mut out = Vec::new();
        let rows = self.rows.max(1);
        let open = self.open.clamp(0.0, 1.0);
        // Rows share the feathers by their arc length.
        let weights: Vec<f32> = (0..rows).map(|r| 1.0 - r as f32 * 0.14).collect();
        let total: f32 = weights.iter().sum();
        let span = std::f32::consts::PI + 2.0 * self.droop;
        for (row, weight) in weights.iter().enumerate() {
            let k = row as f32 / rows as f32;
            let r = self.radius * (1.0 - 0.72 * k);
            let n = ((self.feathers as f32 * weight / total).round() as u32).max(3);
            // Outer feathers are no longer than about half the fan's radius, so
            // their eyespots stay apart along the arc; inner ones reach the
            // center.
            let length = (r * 0.96).min(self.radius * 0.5);
            for i in 0..n {
                // Rows interleave, so inner eyespots sit between outer ones.
                let u = (i as f32 + 0.5 + 0.5 * (row % 2) as f32) / (n as f32 + 0.5);
                let theta = -self.droop + span * u;
                let theta =
                    std::f32::consts::FRAC_PI_2 + (theta - std::f32::consts::FRAC_PI_2) * open;
                let dir = self.right * theta.cos() + self.up * theta.sin();
                let seed = (row as u32 * 131 + i * 17) as f32;
                let shimmer = 0.85 + 0.15 * (self.time * 1.7 + seed).sin();
                let eye = self.center + dir * r * (0.35 + 0.65 * open);
                let lum = self.luminance.map(|c| c * shimmer * (1.0 - 0.2 * k));
                let vane = self.vane.clamp(0.0, 1.0);
                let mut quill = feather(
                    eye,
                    -dir,
                    length * (0.35 + 0.65 * open),
                    lum.map(|c| c * vane),
                    1.0,
                );
                let slim = self.slim.clamp(0.1, 1.0);
                // Keep the sprite's length while narrowing it: the tail
                // grows by what the head's half width gives up.
                quill.tail += quill.tail.normalize_or_zero() * 2.0 * quill.half * (1.0 - slim);
                quill.half *= slim;
                out.push(quill);
                // A round eyespot, burning brighter than its vane.
                out.push(eyespot(
                    eye,
                    length * 0.14 * slim,
                    lum.map(|c| c * (1.0 - vane) * 1.6),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fan holds about the feathers asked for, each laid from its eyespot
    /// back toward the center, and the eyespots stay within the radius.
    #[test]
    fn a_fan_spreads_its_feathers_round_the_center() {
        let fan = Fan {
            center: Vec3::new(0.0, 1.0, -2.0),
            right: Vec3::X,
            up: Vec3::Y,
            radius: 6.0,
            rows: 6,
            feathers: 100,
            droop: 0.1,
            luminance: [4.0; 3],
            vane: 0.5,
            slim: 1.0,
            open: 1.0,
            time: 0.0,
        };
        let sprites = fan.sprites();
        let feathers: Vec<&Sprite> = sprites.iter().filter(|s| s.tail != Vec3::ZERO).collect();
        assert!((90..=110).contains(&feathers.len()), "{}", feathers.len());
        for s in feathers {
            let out = s.at - fan.center;
            assert!(out.length() <= 6.0 + 1e-3);
            assert!(s.tail.dot(out) < 0.0, "a feather points back to the center");
        }
    }
}
