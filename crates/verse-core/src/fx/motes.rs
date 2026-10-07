//! Motes suspended in the water around an eye under the surface
//! (`docs/verse/water.md`, phase W7): specks of silt and plankton that
//! drift slowly and show the water's motion and depth.
//!
//! They keep no state. Each mote has a home in a cube of
//! [`CELL`] m around the origin, drifts on slow sines of the clock, and is
//! wrapped into the cube around the eye, so the same eye and time give the
//! same motes on every client, and a mote that leaves one side comes back
//! on the other, faded at the cube's faces so it never pops. They draw
//! through the existing sprite pipeline with the `water_motes` effect's
//! look ([`super::Style`]).

use glam::Vec3;

use super::{Sprite, Style};
use verse_engine::quality::Tier;

/// The side of the cube the motes fill around the eye, m.
pub const CELL: f32 = 7.0;

/// Motes a tier draws, within the water effects' share of its sprite
/// budget (`docs/verse/water.md`, budgets: 64, 256, and 512).
#[must_use]
pub fn count(tier: Tier) -> usize {
    match tier {
        Tier::Low => 48,
        Tier::Medium => 160,
        Tier::High => 320,
    }
}

/// Motes a zone draws, which does not know the renderer's tier:
/// Medium's. The renderer keeps a frame's sprites within its own tier's
/// budget, dropping these low-priority specks first.
pub const ZONE_COUNT: usize = 160;

/// A hash of `i` and `salt` to 0 to 1.
fn unit(i: u32, salt: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Appends up to `count` motes around `eye` under water whose surface
/// stands at `level`, at water clock `time`, to `out`: `amount` (0 to 1,
/// how much of the view lies under the water) fades them in, and `tint`
/// (the water's in-scatter color, linear) colors them. A mote above the
/// surface is left out.
pub fn underwater(
    eye: Vec3,
    level: f32,
    time: f32,
    count: usize,
    amount: f32,
    tint: [f32; 3],
    out: &mut Vec<Sprite>,
) {
    let amount = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if amount <= 0.0 || !eye.is_finite() || !level.is_finite() || !time.is_finite() {
        return;
    }
    let Some(style) = Style::named("water_motes") else {
        return;
    };
    // The motes take the water's color, brightened so they read against
    // it.
    let peak = tint.iter().copied().fold(1e-4_f32, f32::max);
    let color = tint.map(|c| (c / peak).clamp(0.2, 1.0));
    let half = CELL * 0.5;
    for i in 0..count as u32 {
        let home = Vec3::new(unit(i, 1), unit(i, 2), unit(i, 3)) * CELL;
        // Slow drift, each mote on its own phases, and a little sinking.
        let phase = unit(i, 4) * std::f32::consts::TAU;
        let drift = Vec3::new(
            0.18 * (time * 0.21 + phase).sin() + 0.05 * time,
            0.08 * (time * 0.17 + phase * 1.3).sin() - 0.012 * time,
            0.18 * (time * 0.19 + phase * 0.7).cos() + 0.03 * time,
        );
        let rel = home + drift - eye;
        let wrapped = rel - (rel / CELL).floor() * CELL - Vec3::splat(half);
        let at = eye + wrapped;
        if at.y > level - 0.03 {
            continue;
        }
        let edge = wrapped.abs().max_element();
        let fade = 1.0 - smoothstep(half * 0.7, half, edge);
        // Too close to the eye, a speck would fill the view.
        let near = smoothstep(0.15, 0.5, wrapped.length());
        let alpha = fade * near * amount;
        if alpha <= 0.01 {
            continue;
        }
        let size = 0.006 + 0.01 * unit(i, 5);
        if let Some(mut s) = style.sprite(0, at, Vec3::ZERO, 0.5, size, color, i) {
            s.alpha *= alpha;
            out.push(s);
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motes_fill_the_water_around_the_eye_and_stay_under_the_surface() {
        let eye = Vec3::new(40.0, -2.0, -15.0);
        let mut out = Vec::new();
        underwater(
            eye,
            0.0,
            12.0,
            count(Tier::High),
            1.0,
            [0.1, 0.3, 0.25],
            &mut out,
        );
        assert!(out.len() > count(Tier::High) / 2, "{}", out.len());
        for s in &out {
            assert!(s.at.y < 0.0);
            assert!((s.at - eye).abs().max_element() <= CELL * 0.5 + 1e-3);
            assert!(s.alpha > 0.0 && s.alpha <= 1.0);
        }
        // The same eye and time give the same motes.
        let mut again = Vec::new();
        underwater(
            eye,
            0.0,
            12.0,
            count(Tier::High),
            1.0,
            [0.1, 0.3, 0.25],
            &mut again,
        );
        assert_eq!(out, again);
    }

    #[test]
    fn no_motes_above_the_water_or_with_none_of_the_view_under_it() {
        let mut out = Vec::new();
        underwater(
            Vec3::new(0.0, 5.0, 0.0),
            0.0,
            1.0,
            64,
            1.0,
            [0.2; 3],
            &mut out,
        );
        assert!(out.is_empty(), "the whole cube stands over the surface");
        underwater(
            Vec3::new(0.0, -5.0, 0.0),
            0.0,
            1.0,
            64,
            0.0,
            [0.2; 3],
            &mut out,
        );
        assert!(out.is_empty());
        underwater(
            Vec3::new(0.0, -5.0, 0.0),
            0.0,
            f32::NAN,
            64,
            1.0,
            [0.2; 3],
            &mut out,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn motes_fit_within_each_tiers_water_effects_budget() {
        for (tier, water_fx) in [(Tier::Low, 64), (Tier::Medium, 256), (Tier::High, 512)] {
            assert!(count(tier) <= water_fx);
            assert!(count(tier) <= super::super::budget(tier));
        }
        assert_eq!(ZONE_COUNT, count(Tier::Medium));
    }
}
