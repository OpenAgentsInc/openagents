//! Lightning in water: our rule, since SRD 5.2.1's only damage rule for
//! water is Resistance to Fire under it (`docs/verse/water.md`, Lightning
//! in water).
//!
//! 1. A contact point is where a lightning effect meets a water surface.
//! 2. Every creature in the same body within 6 m of a contact point that is
//!    in the water (wading, swimming, diving, or water walking), and not in
//!    the spell's own area, makes the spell's save: half damage on a
//!    failure, none on a success.
//! 3. Once per cast, or once per bolt; never both direct and conducted.
//! 4. A spell attack never conducts.
//! 5. Ice and boats insulate, and a walkable ice cell stops conduction
//!    across it.
//! 6. Resistance and Immunity apply as usual; Water Breathing gives none.

use glam::{DVec2, DVec3};
use physics::water::WaterId;

use super::{Basin, Wader, WaterSpells, ice::IceState};
use crate::spells::{Dice, FEET, Save};

/// How far lightning conducts through water, m: 20 feet (ours).
pub const CONDUCTION: f64 = 20.0 * FEET;
/// How close a water walker's feet must be to the surface to touch it, m.
pub const TOUCH: f64 = 0.3;
/// Lightning Bolt's line, sampled for contact points every this far, m:
/// its 5-foot width.
pub const LINE_STEP: f64 = 5.0 * FEET;

/// How a lightning spell reaches its targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// A saving throw: it conducts.
    Save,
    /// A spell attack, as Shocking Grasp: it never conducts.
    Attack,
}

/// A contact point: where lightning met a body's surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub body: WaterId,
    pub at: DVec3,
}

/// One creature conduction reached.
#[derive(Clone, Debug, PartialEq)]
pub struct Conducted {
    pub creature: u64,
    pub save: Save,
    pub damage: u32,
}

/// Call Lightning's d10s: 3, or 4 when cast outdoors in the zone's Storm
/// weather, which it takes control of (SRD 5.2.1).
#[must_use]
pub const fn call_lightning_dice(storm: bool) -> u32 {
    if storm { 4 } else { 3 }
}

impl WaterSpells {
    /// The contact point of a strike at `at`: projected onto the surface
    /// below or above it.
    #[must_use]
    pub fn contact(&self, basin: &Basin, at: DVec3, tick: u64) -> Option<Contact> {
        let s = self.sample(basin, DVec2::new(at.x, at.z), tick)?;
        Some(Contact {
            body: s.body,
            at: DVec3::new(at.x, s.height, at.z),
        })
    }

    /// Every contact point of a line from `a` to `b`, such as Lightning
    /// Bolt's: each 1.5 m step that crosses water.
    #[must_use]
    pub fn line_contacts(&self, basin: &Basin, a: DVec3, b: DVec3, tick: u64) -> Vec<Contact> {
        let length = a.distance(b);
        let steps = (length / LINE_STEP).ceil().max(1.0) as usize;
        (0..=steps)
            .filter_map(|k| {
                let p = a.lerp(b, k as f64 / steps as f64);
                let c = self.contact(basin, p, tick)?;
                // The line meets the water only where it runs at or under
                // the surface.
                (p.y <= c.at.y + 0.5).then_some(c)
            })
            .collect()
    }

    /// Whether lightning at `contact` reaches `wader` at `tick` by our rule:
    /// in the same body, within 6 m, in the water, not insulated by ice or
    /// a boat, and no walkable ice between.
    #[must_use]
    pub fn reaches(&self, basin: &Basin, contact: &Contact, wader: &Wader, tick: u64) -> bool {
        if wader.boat {
            return false;
        }
        let p = wader.xz();
        let Some(water) = self.sample(basin, p, tick) else {
            return false;
        };
        if water.body != contact.body {
            return false;
        }
        // The nearest point of the creature to the contact.
        let low = wader.feet.y.min(wader.head());
        let high = wader.feet.y.max(wader.head());
        let nearest = DVec3::new(wader.feet.x, contact.at.y.clamp(low, high), wader.feet.z);
        if nearest.distance(contact.at) > CONDUCTION {
            return false;
        }
        let walking = self.walks(wader.id, tick) && (wader.feet.y - water.height).abs() <= TOUCH;
        let wet = wader.feet.y < water.height - 0.05;
        if !(wet || walking) {
            return false;
        }
        // Standing on ice insulates.
        if self
            .ice
            .state(basin, p, tick)
            .is_some_and(|s| s >= IceState::Floes)
            && wader.feet.y >= water.height - 0.15
        {
            return false;
        }
        // A walkable ice cell between stops it.
        let from = DVec2::new(contact.at.x, contact.at.z);
        let steps = (from.distance(p) / 0.5).ceil() as usize;
        (1..steps).all(|k| {
            let q = from.lerp(p, k as f64 / steps as f64);
            self.ice.state(basin, q, tick) != Some(IceState::Walkable)
        })
    }

    /// Conducts bolt `bolt` from `contacts` to `waders` at `tick`: each one
    /// reached, not in `direct` (the spell's own area), and not already
    /// reached by this bolt makes the save against `dc` with its
    /// `dexterity` modifier and takes half of `damage` on a failure.
    #[allow(clippy::too_many_arguments)]
    pub fn conduct(
        &mut self,
        basin: &Basin,
        dice: &mut Dice,
        bolt: u64,
        delivery: Delivery,
        contacts: &[Contact],
        waders: &[Wader],
        direct: &[u64],
        damage: u32,
        dc: i32,
        dexterity: impl Fn(u64) -> i32,
        tick: u64,
    ) -> Vec<Conducted> {
        if delivery == Delivery::Attack {
            return Vec::new();
        }
        let mut out = Vec::new();
        for w in waders {
            if direct.contains(&w.id)
                || self.conducted.contains(&(bolt, w.id))
                || !contacts.iter().any(|c| self.reaches(basin, c, w, tick))
            {
                continue;
            }
            if self.conducted.len() < 4096 {
                self.conducted.insert((bolt, w.id));
            }
            let save = dice.save(w.id, "Dexterity", dexterity(w.id), dc);
            let taken = if save.success { 0 } else { damage / 2 };
            out.push(Conducted {
                creature: w.id,
                save,
                damage: taken,
            });
        }
        out
    }
}
