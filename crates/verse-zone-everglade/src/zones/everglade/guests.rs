//! Private characters standing in the town (`docs/verse/private-assets.md`):
//! licensed characters the owner placed in owner-local configuration, each
//! from a private pack this repository never holds. Nothing here names one.
//!
//! A guest is a private pack's [`NEAR`] and [`FAR`] forms standing still at
//! its place, playing their `idle` clip, drawn in the frame's figure after
//! the town's wildlife and lit by the baked probes. The near level draws
//! within [`NEAR_REACH`] of the eye and the far level from there to the
//! wildlife's cull distance. Each guest blocks a small footprint, so the
//! player walks around rather than through her.

use glam::Vec3;

use super::height;
use super::wildlife::{CULL, Creature, Route, Wildlife};
use crate::controller::Footprint;
use crate::zones::everglade_pack::ZonePack;
use crate::zones::everglade_pack::compile::private::{FAR, NEAR};

/// Where the near level gives way to the far, m.
pub const NEAR_REACH: f32 = 18.0;
/// Half the side of a guest's footprint at its compiled size, m.
const HALF: f32 = 0.25;
/// A guest's height for blocking, at its compiled size, m.
const TALL: f32 = 1.7;

/// Where a guest stands: x and z, m; facing, as the controller's yaw; and
/// times its compiled size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand {
    pub at: [f32; 2],
    pub yaw: f32,
    pub scale: f32,
}

/// The guest in `pack` standing at `stand`, both levels.
///
/// # Errors
///
/// Returns a message when the pack lacks the near form or a form can't
/// play.
pub fn guest(pack: &ZonePack, stand: Stand) -> Result<Wildlife, String> {
    if pack.form(NEAR).is_none() {
        return Err("Private pack has no guest".into());
    }
    let [x, z] = stand.at;
    let route = Route::Sit {
        at: Vec3::new(x, height(x, z), z),
        yaw: stand.yaw,
    };
    let creature = |form| Creature {
        form,
        route: route.clone(),
        scale: stand.scale,
        phase: 0.0,
    };
    let mut levels = vec![(creature(NEAR), (0.0, NEAR_REACH))];
    if pack.form(FAR).is_some() {
        levels.push((creature(FAR), (NEAR_REACH, CULL)));
    } else {
        levels[0].1 = (0.0, CULL);
    }
    Wildlife::banded(pack, levels)
}

/// The block a guest at `stand` makes: a footprint and its top.
#[must_use]
pub fn block(stand: Stand) -> (Footprint, f32) {
    let [x, z] = stand.at;
    let half = HALF * stand.scale;
    (
        Footprint {
            min: [x - half, z - half],
            max: [x + half, z + half],
        },
        height(x, z) + TALL * stand.scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::everglade_pack::compile::private;

    #[test]
    fn a_guest_draws_its_near_level_close_and_its_far_level_beyond() {
        let pack = private::decode(&private::sample()).unwrap();
        let stand = Stand {
            at: [105.5, -31.2],
            yaw: -1.571,
            scale: 1.0,
        };
        let mut guest = guest(&pack, stand).unwrap();
        assert_eq!(guest.creatures().len(), 2);
        let at = Vec3::new(105.5, height(105.5, -31.2), -31.2);
        guest.tick(0.1, at + Vec3::new(0.0, 1.6, 4.0));
        assert_eq!(guest.drawn(), 1, "only the near level, close by");
        guest.tick(0.1, at + Vec3::new(0.0, 1.6, 30.0));
        assert_eq!(guest.drawn(), 1, "only the far level, at a distance");
        guest.tick(0.1, at + Vec3::new(0.0, 1.6, 90.0));
        assert_eq!(guest.drawn(), 0, "nothing beyond the cull");
        let (footprint, top) = block(stand);
        assert!(footprint.min[0] < 105.5 && footprint.max[0] > 105.5);
        assert!(top > at.y + 1.0);
    }
}
