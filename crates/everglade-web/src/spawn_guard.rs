//! Where a player may not appear on the Grid (#12479): within reach of
//! anything that opens when someone stands near it. The shared terminal
//! opens its chat on approach, and the help desk and the Game Lab arch
//! offer theirs. A player who appears inside one never walked there, so
//! their first keys would go to a chat instead of moving them.
//!
//! A fresh arrival keeps out of every reach by [`MARGIN`], and a returning
//! player whose last spot is inside one arrives fresh instead.

use verse_world::social::controller::Footprint;

/// Room kept around each reach, m: a player who appears near one still
/// walks a few steps to get there.
pub const MARGIN: f32 = 2.0;

/// The Game Lab arch's widest reach, m (`verse::zones::game_place`).
pub const GAME_LAB_REACH: f32 = 4.5;

/// Something that opens when a player stands within `reach` of `at`
/// (`[x, z]`, m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trigger {
    pub at: [f32; 2],
    pub reach: f32,
}

#[allow(clippy::cast_possible_truncation)]
fn flat(p: [f64; 3]) -> [f32; 2] {
    [p[0] as f32, p[2] as f32]
}

/// The Grid's triggers: the shared terminal, the help desk's counter, and
/// the Game Lab arch at `game_lab` (`[x, z]`).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn triggers(game_lab: [f32; 2]) -> [Trigger; 3] {
    [
        Trigger {
            at: flat(pylons_view::terminal::POSITION),
            reach: pylons_view::terminal::LEAVE_RANGE as f32,
        },
        Trigger {
            at: flat(grid_help_desk::world([0.0, 0.0, grid_help_desk::STAND])),
            reach: grid_help_desk::LEAVE_RANGE as f32,
        },
        Trigger {
            at: game_lab,
            reach: GAME_LAB_REACH,
        },
    ]
}

/// Whether feet at `[x, z]` are within any trigger's reach plus
/// [`MARGIN`].
#[must_use]
pub fn too_close(at: [f32; 2], triggers: &[Trigger]) -> bool {
    triggers
        .iter()
        .any(|t| (at[0] - t.at[0]).hypot(at[1] - t.at[1]) < t.reach + MARGIN)
}

/// Each trigger's reach plus [`MARGIN`] as a square footprint, for the
/// spawn picker to keep clear of beside the world's own blockers.
#[must_use]
pub fn keep_out(triggers: &[Trigger]) -> Vec<Footprint> {
    triggers
        .iter()
        .map(|t| {
            let r = t.reach + MARGIN;
            Footprint {
                min: [t.at[0] - r, t.at[1] - r],
                max: [t.at[0] + r, t.at[1] + r],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use verse_world::social::arrival::{SpawnArea, pick_spawn};

    /// `verse::zones::game_place::POSITION`.
    const GAME_LAB: [f32; 2] = [-8.0, 3.0];
    /// The walk-in arches (`verse::zones::gate`, side and forward of the
    /// spawn at z -10): Everglade, Lagrange 2, the RITUAL arch, and the
    /// hosted arches (ICE CREAM MAN, then HAUNTED HOUSE).
    const ARCHES: [[f32; 2]; 6] = [
        [9.0, 1.0],
        [-20.0, 9.0],
        [0.0, 16.0],
        [20.0, 9.0],
        [27.0, 9.0],
        [34.0, 9.0],
    ];
    /// How far in front of an arch its approach reaches, m.
    const ARCH_REACH: f32 = 4.0;

    fn key(i: u32) -> String {
        format!("{i:064x}")
    }

    #[test]
    fn ten_thousand_spawns_land_outside_every_reach() {
        let mut all: Vec<Trigger> = triggers(GAME_LAB).to_vec();
        all.extend(ARCHES.iter().map(|&at| Trigger {
            at,
            reach: ARCH_REACH,
        }));
        let blockers = keep_out(&triggers(GAME_LAB));
        let area = SpawnArea::GRID;
        for i in 0..10_000 {
            let a = pick_spawn(&key(i), area, &[], &blockers);
            let at = [a.pos.x, a.pos.z];
            assert!(!too_close(at, &all), "{i}: {at:?}");
            // Nowhere near what opens the terminal's chat or the desk's.
            let p = [f64::from(at[0]), 0.0, f64::from(at[1])];
            assert!(!pylons_view::terminal::in_reach(p, true), "{at:?}");
            assert!(!grid_help_desk::in_reach(p, true), "{at:?}");
        }
    }

    #[test]
    fn a_spot_inside_a_reach_is_too_close() {
        let all = triggers(GAME_LAB);
        let terminal = flat(pylons_view::terminal::POSITION);
        assert!(too_close(terminal, &all));
        assert!(too_close([terminal[0], terminal[1] - 1.5], &all));
        assert!(too_close(GAME_LAB, &all));
        assert!(!too_close([0.0, -14.0], &all));
        // The squares hold each disc.
        let squares = keep_out(&all);
        for t in &all {
            assert!(
                squares
                    .iter()
                    .any(|s| s.contains(t.at[0] + t.reach, t.at[1], 0.0))
            );
        }
    }
}
