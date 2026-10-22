//! Triggers at run time: given where the player stands and looks, which
//! triggers fire this frame. Each fires once per visit to its area (or once
//! ever, when `every_visit` is false), so a walk-through is predictable.

use std::collections::BTreeSet;

use crate::doc::{Action, House, When};
use crate::walk::area_at;

/// How near the middle of an area counts as reaching it, as a share of the
/// area's smaller side.
const MIDDLE: f32 = 0.25;
/// How narrow a look must be to count as looking at a prop, degrees.
const LOOK_CONE: f32 = 14.0;
/// How far away a looked-at prop may be, m.
const LOOK_RANGE: f32 = 9.0;

/// One fired trigger.
#[derive(Clone, Debug, PartialEq)]
pub struct Fired {
    pub area: String,
    pub index: usize,
    /// The prop it watches, for placing its sounds.
    pub prop: Option<String>,
    pub actions: Vec<Action>,
}

/// The trigger state of one walk.
#[derive(Clone, Debug, Default)]
pub struct Triggers {
    area: Option<String>,
    /// `(area, trigger)` fired this visit.
    this_visit: BTreeSet<(String, usize)>,
    /// `(area, trigger)` fired ever.
    ever: BTreeSet<(String, usize)>,
}

impl Triggers {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The area the player was last in.
    #[must_use]
    pub fn area(&self) -> Option<&str> {
        self.area.as_deref()
    }

    /// The triggers that fire for a player at `p` (x, z) looking along yaw
    /// `look` (degrees). Call once a frame.
    pub fn step(&mut self, house: &House, p: [f32; 2], look: f32) -> Vec<Fired> {
        let Some(area) = area_at(house, p) else {
            return Vec::new();
        };
        if self.area.as_deref() != Some(area.id.as_str()) {
            self.area = Some(area.id.clone());
            self.this_visit.clear();
        }
        let centre = area.centre();
        let middle_r = area.width().min(area.depth()) * MIDDLE;
        let at_middle = (p[0] - centre[0]).hypot(p[1] - centre[1]) <= middle_r.max(0.6)
            || (area.width() < 3.5 && (p[1] - centre[1]).abs() < 1.0)
            || (area.depth() < 3.5 && (p[0] - centre[0]).abs() < 1.0);
        let prop_at = |id: &str| {
            area.props
                .iter()
                .find(|x| x.id == id)
                .map(|x| area.world(x.at))
        };
        let mut out = Vec::new();
        for (index, t) in area.triggers.iter().enumerate() {
            let key = (area.id.clone(), index);
            if self.this_visit.contains(&key) || (!t.every_visit && self.ever.contains(&key)) {
                continue;
            }
            let fire = match &t.when {
                When::Enter(on) => *on,
                When::EnterMiddle(on) => *on && at_middle,
                When::Near { prop, within } => {
                    prop_at(prop).is_some_and(|q| (q[0] - p[0]).hypot(q[1] - p[1]) <= *within)
                }
                When::Look { prop } => prop_at(prop).is_some_and(|q| {
                    let (dx, dz) = (q[0] - p[0], q[1] - p[1]);
                    let d = dx.hypot(dz);
                    if d > LOOK_RANGE || d < 0.2 {
                        return false;
                    }
                    let to = dx.atan2(dz).to_degrees();
                    let off = (to - look + 540.0).rem_euclid(360.0) - 180.0;
                    off.abs() <= LOOK_CONE
                }),
            };
            if fire {
                self.this_visit.insert(key.clone());
                self.ever.insert(key);
                out.push(Fired {
                    area: area.id.clone(),
                    index,
                    prop: t.prop().map(str::to_owned),
                    actions: t.actions.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::v1;

    #[test]
    fn the_parlor_dips_its_candles_at_the_middle_once_a_visit() {
        let house = v1();
        let mut t = Triggers::new();
        // Just inside the parlor: nothing yet.
        assert!(t.step(&house, [8.0, 32.5], 0.0).is_empty());
        let fired = t.step(&house, [7.0, 36.0], 0.0);
        assert_eq!(fired.len(), 1, "{fired:?}");
        assert!(matches!(fired[0].actions[0], Action::Light(_)));
        assert!(t.step(&house, [7.0, 36.1], 0.0).is_empty());
        // Leave and come back: it fires again.
        let _ = t.step(&house, [8.0, 30.0], 180.0);
        let _ = t.step(&house, [8.0, 32.5], 0.0);
        assert_eq!(t.step(&house, [7.0, 36.0], 0.0).len(), 1);
    }

    #[test]
    fn near_and_entering_triggers_fire_where_they_should() {
        let house = v1();
        let mut t = Triggers::new();
        // Entering the long hall flashes lightning.
        let fired = t.step(&house, [5.2, 40.5], 0.0);
        assert!(
            fired
                .iter()
                .any(|f| f.actions.contains(&Action::Sfx("thunder".into())))
        );
        // The cat hisses only up close.
        let _ = t.step(&house, [5.2, 59.0], 0.0);
        assert!(t.step(&house, [5.2, 60.0], 0.0).is_empty());
        let fired = t.step(&house, [2.5, 65.0], 0.0);
        assert_eq!(fired[0].prop.as_deref(), Some("cat-1"));
    }
}
