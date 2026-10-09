//! The clock tower's load dial (P4, `docs/compute/verse-compute.md`): a
//! ring of light round the tower's front clock face whose lit arc, from
//! twelve o'clock round clockwise, is the pool's load: the busy share of
//! the slots on its online pylons, as the Wellspring's state counts them.
//! It brightens while the pool's newest aggregate recomputed. With no
//! Wellspring state, or no online slot, the ring is dark.

use super::*;
use crate::zones::everglade::layout;

/// The tower's model and the front clock face in its frame: its height
/// and how far out of the front wall the ring stands, m.
const TOWER: &str = "generated/clock_tower";
const CLOCK: (f32, f32) = (10.2, 0.26);
/// The ring's radii round the clock's 0.95 m rim, m.
const RING: (f32, f32) = (1.06, 1.24);
/// The ring's segments.
const SEGMENTS: usize = 24;
/// How far from the tower the eye still sees the dial, m.
const DIAL_REACH: f32 = 160.0;
/// A lit segment's light, and a dark one's faint glow.
const LIT_LUMINANCE: f32 = 3.6;
const DARK_LUMINANCE: f32 = 0.35;

/// The front clock face's center and the tower's outward front and right,
/// in the world; `None` when the town has no clock tower.
#[must_use]
pub fn face() -> Option<(Vec3, Vec3, Vec3)> {
    static FACE: OnceLock<Option<(Vec3, Vec3, Vec3)>> = OnceLock::new();
    *FACE.get_or_init(|| {
        let p = layout::placements()
            .into_iter()
            .find(|p| p.model == TOWER)?;
        let turn = Quat::from_rotation_y(p.yaw);
        let out = turn * Vec3::Z;
        let right = turn * Vec3::X;
        let [x, z] = p.at;
        let base = Vec3::new(x, super::super::super::height(x, z) + p.lift, z);
        Some((
            base + Vec3::Y * CLOCK.0 * p.scale + out * CLOCK.1 * p.scale,
            out,
            right,
        ))
    })
}

/// The pool's load, 0 to 1, from the Wellspring's state, and whether its
/// aggregate recomputed; `None` with no state or no online slot.
#[must_use]
pub fn load(well: Option<&State>) -> Option<(f32, bool)> {
    let Some(State::Wellspring {
        busy,
        total,
        verified,
        ..
    }) = well
    else {
        return None;
    };
    (*total > 0).then(|| ((*busy as f32 / *total as f32).clamp(0.0, 1.0), *verified))
}

/// The dial's glows from `eye` into `out`.
pub(super) fn draw(out: &mut Vec<GlowVertex>, well: Option<&State>, eye: Vec3, soft: f32) {
    let (Some((center, _, right)), Some((load, verified))) = (face(), load(well)) else {
        return;
    };
    if eye.distance(center) > DIAL_REACH {
        return;
    }
    let lit = (load * SEGMENTS as f32).round() as usize;
    let boost = if verified { 1.25 } else { 1.0 };
    let at = |a: f32, r: f32| center + Vec3::Y * (r * a.cos()) + right * (r * a.sin());
    for k in 0..SEGMENTS {
        // A small gap between segments, so the arc reads as a dial.
        let a0 = (k as f32 + 0.08) / SEGMENTS as f32 * TAU;
        let a1 = (k as f32 + 0.92) / SEGMENTS as f32 * TAU;
        let luminance = if k < lit {
            LIT_LUMINANCE * boost
        } else {
            DARK_LUMINANCE
        };
        let radiance = times(LIGHT, luminance * soft);
        glow_quad(
            out,
            [
                at(a0, RING.0),
                at(a0, RING.1),
                at(a1, RING.1),
                at(a1, RING.0),
            ],
            [radiance; 4],
            [[0.0, -1.0], [0.0, 1.0], [0.0, 1.0], [0.0, -1.0]],
        );
    }
}
