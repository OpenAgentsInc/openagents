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

use super::{Facing, Sprite, sheet};

/// A feather's length over its width, as the sheet is painted.
pub const ASPECT: f32 = 3.0;

/// The sheet's frame holding the feather.
const FEATHER: u32 = 0;
/// The sheet's frame holding the soft dot.
const DOT: u32 = 1;

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
            let length = r * 0.96;
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
                out.push(feather(eye, -dir, length * (0.35 + 0.65 * open), lum, 1.0));
                out.push(dot(eye, length * 0.06, lum.map(|c| c * 0.08), 1.0));
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
