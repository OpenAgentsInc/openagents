//! The Pylon Field's site (`docs/compute/verse-compute.md`, The Pylon
//! Field): the circle of standing stones in the north woods, the
//! Wellspring's basin at its middle, and the pylon sites on a ring outside
//! the stones. The foliage round places the stones ([`super::foliage`])
//! and records the site here as it finds open ground for it, so the woods
//! that follow leave the field clear. Everything standing on the site is
//! drawn by `zones::everglade::compute` from its compute source; the
//! layout itself places only the stones.

use std::sync::OnceLock;

/// The standing stones' ring, m from the middle.
pub const STONES: f32 = 5.5;
/// The Wellspring's basin: its rim's outer radius, m. Its stepped plinth
/// reaches half a meter further.
pub const BASIN: f32 = 2.6;
/// The ring the capacity book's wells stand on, inside the stones, m.
pub const WELLS: f32 = 4.2;
/// How many wells the field has room for.
pub const MAX_WELLS: usize = 6;
/// The ring the pylon sites stand on, m from the middle.
pub const RING: f32 = 9.5;
/// How many sites the ring is tried at, evenly round it.
pub const TRIES: usize = 14;
/// How much ground each pylon site keeps clear, m.
pub const SITE_CLEAR: f32 = 1.4;
/// How far the woods keep back from the middle, m.
pub const CLEAR: f32 = RING + SITE_CLEAR + 0.6;

/// The field as the layout found it.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// The middle of the stones and the basin, x and z.
    pub center: [f32; 2],
    /// The open pylon sites, the southernmost first, then round the ring
    /// counterclockwise seen from above. The first is this computer's.
    pub sites: Vec<[f32; 2]>,
}

impl Field {
    /// The ring's direction from the middle toward `at`, unit length.
    fn toward(&self, at: [f32; 2]) -> [f32; 2] {
        let d = [at[0] - self.center[0], at[1] - self.center[1]];
        let n = d[0].hypot(d[1]).max(1e-6);
        [d[0] / n, d[1] / n]
    }

    /// Where an agent stands to see pylon site `i`: on the field's side of
    /// it, and the heading toward the pylon.
    #[must_use]
    pub fn site_stand(&self, i: usize) -> Option<([f32; 2], f32)> {
        let site = *self.sites.get(i)?;
        let [dx, dz] = self.toward(site);
        let stand = [site[0] - dx * 1.6, site[1] - dz * 1.6];
        Some((stand, dx.atan2(dz)))
    }

    /// Where an agent stands at the Wellspring, beside the basin on the
    /// town's side, and the heading toward it.
    #[must_use]
    pub fn basin_stand(&self) -> ([f32; 2], f32) {
        let [cx, cz] = self.center;
        ([cx, cz - (BASIN + 0.85)], 0.0)
    }

    /// Where an agent stands to be in the field: inside the stones on the
    /// town's side.
    #[must_use]
    pub fn stand(&self) -> [f32; 2] {
        let [cx, cz] = self.center;
        [cx + 1.5, cz - (STONES - 1.6)]
    }

    /// The `i`th well's place on its ring, toward the north first.
    #[must_use]
    pub fn well(&self, i: usize) -> [f32; 2] {
        let a = std::f32::consts::FRAC_PI_2 + i as f32 / MAX_WELLS as f32 * std::f32::consts::TAU;
        [
            self.center[0] + a.cos() * WELLS,
            self.center[1] + a.sin() * WELLS,
        ]
    }
}

static FOUND: OnceLock<Option<Field>> = OnceLock::new();

/// Records the field the foliage round found; the layout is the same
/// every time, so the first record stands.
pub(super) fn record(field: Option<Field>) {
    let _ = FOUND.set(field);
}

/// Drops from `out` what would stand in the field: every blocking piece
/// but the standing stones inside [`CLEAR`] of the middle, and anything
/// at all on the basin or a pylon site, so the field is a clearing in the
/// woods. A layout without a field is left as it was.
pub fn clear(out: &mut Vec<super::Placement>) {
    let Some(Some(field)) = FOUND.get() else {
        return;
    };
    let near = |at: [f32; 2], p: [f32; 2], r: f32| (at[0] - p[0]).hypot(at[1] - p[1]) < r;
    out.retain(|p| {
        if p.model.starts_with("foliage/standing_stone") && near(p.at, field.center, STONES + 0.5) {
            return true;
        }
        let blocks = !matches!(p.collision, super::Collision::None);
        let on_site = near(p.at, field.center, BASIN + 0.6)
            || field.sites.iter().any(|s| near(p.at, *s, SITE_CLEAR));
        !(on_site || (blocks && near(p.at, field.center, CLEAR)))
    });
}

/// The ring's candidate sites round `center`, the southernmost first.
#[must_use]
pub fn candidates(center: [f32; 2]) -> Vec<[f32; 2]> {
    (0..TRIES)
        .map(|k| {
            // From due south (-z), counterclockwise seen from above.
            let a = -std::f32::consts::FRAC_PI_2 + k as f32 / TRIES as f32 * std::f32::consts::TAU;
            [center[0] + a.cos() * RING, center[1] + a.sin() * RING]
        })
        .collect()
}

/// The Pylon Field, once the layout has placed the stones; `None` when the
/// woods had no clearing for them.
#[must_use]
pub fn site() -> Option<&'static Field> {
    if FOUND.get().is_none() {
        let _ = super::placements();
    }
    FOUND.get().and_then(Option::as_ref)
}
