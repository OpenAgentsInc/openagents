//! The game state as data, so it outlives the code that steps it.
//!
//! A hosted zone (`docs/verse/ice-cream-man-live.md`) runs these rules as a
//! WebAssembly module it can swap while people play. The page that draws
//! the game keeps the state; each step it hands the state to the module and
//! takes the next one back. A new module, or a rolled-back one, carries on
//! from the same state at the next step.
//!
//! The state is JSON. The house is never in it: it is built again from the
//! seed and the tuning ([`HouseCache`] keeps the last one), so a step moves
//! a few kilobytes. [`STATE_VERSION`] names the shape; a module refuses a
//! state newer than it reads, and the page reloads the zone instead.

use serde::{Deserialize, Serialize};

use crate::layout::{House, Shape};
use crate::{Game, Tuning};

/// The version of the state's shape these rules read and write. Change it
/// when a field changes meaning or a new field has no sensible default.
pub const STATE_VERSION: u32 = 1;

#[derive(Serialize)]
struct Out<'a> {
    version: u32,
    seed: u64,
    game: &'a Game,
}

#[derive(Deserialize)]
struct In {
    version: u32,
    seed: u64,
    game: Game,
}

/// The last house built, reused while the seed and the house's settings
/// stay the same.
#[derive(Default)]
pub struct HouseCache {
    key: Option<(u64, Shape, u32)>,
    house: Option<House>,
}

impl HouseCache {
    /// The house for `seed` and `tuning`, built only when it changed.
    pub fn house(&mut self, seed: u64, tuning: &Tuning) -> House {
        let key = (seed, Shape::of(tuning), tuning.extra_closets);
        if self.key != Some(key) || self.house.is_none() {
            self.house = Some(House::of(seed, tuning));
            self.key = Some(key);
        }
        self.house.clone().expect("the house was just built")
    }
}

/// What a new tuning did to a run in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Retune {
    /// The run plays on with the new numbers. `house_changed` when hiding
    /// spots were added or removed.
    Applied { house_changed: bool },
    /// The new numbers build another house, so they start with the next
    /// run; this one keeps the old numbers.
    NextRun(&'static str),
}

impl Game {
    /// This run's state, without the house.
    #[must_use]
    pub fn to_state(&self) -> String {
        serde_json::to_string(&Out {
            version: STATE_VERSION,
            seed: self.house.seed,
            game: self,
        })
        .expect("a game always serializes")
    }

    /// A run from its state, with its house from `cache`.
    ///
    /// # Errors
    /// The text isn't a state, it is newer than these rules read, or it
    /// doesn't fit its house.
    pub fn from_state(text: &str, cache: &mut HouseCache) -> Result<Self, String> {
        let state: In = serde_json::from_str(text).map_err(|e| format!("not a game state: {e}"))?;
        if state.version > STATE_VERSION {
            return Err(format!(
                "the state is version {}, these rules read up to {STATE_VERSION}",
                state.version
            ));
        }
        let mut game = state.game;
        game.house = cache.house(state.seed, &game.tuning);
        game.fits_house()?;
        Ok(game)
    }

    fn fits_house(&self) -> Result<(), String> {
        let house = &self.house;
        if self.taken.len() != house.keys.len() || self.opened.len() != house.fridges.len() {
            return Err("the state's keys or fridges don't match its house".into());
        }
        if self.player.hidden.is_some_and(|i| i >= house.closets.len()) {
            return Err("the state hides in a closet its house doesn't have".into());
        }
        Ok(())
    }

    /// Plays on with `tuning` from the next step. Numbers apply at once.
    /// Hiding spots are added to (or taken from) the same house, keeping
    /// every key, fridge and closet the run already knows. A change to the
    /// house's size or shape waits for the next run.
    pub fn retune(&mut self, tuning: Tuning) -> Retune {
        if Shape::of(&tuning) != Shape::of(&self.tuning) {
            return Retune::NextRun("the house would change shape");
        }
        let house_changed = tuning.extra_closets != self.tuning.extra_closets;
        if house_changed {
            let house = House::of(self.house.seed, &tuning);
            let kept = self.player.hidden.is_none_or(|i| i < house.closets.len());
            if house.keys != self.house.keys || house.fridges != self.house.fridges || !kept {
                return Retune::NextRun("the house would move its keys or fridges");
            }
            self.house = house;
        }
        self.tuning = tuning;
        self.man.tuning = tuning;
        self.player.health = self.player.health.min(self.max_health());
        Retune::Applied { house_changed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Input, Options};

    fn run() -> Game {
        let mut game = Game::tuned(7, Options::default(), Tuning::default());
        let input = Input {
            walk: crate::v(0.0, -1.0),
            sprint: true,
            ..Input::default()
        };
        for _ in 0..90 {
            game.step(input);
        }
        game
    }

    #[test]
    fn a_state_reads_back_as_the_same_run() {
        let game = run();
        let mut cache = HouseCache::default();
        let back = Game::from_state(&game.to_state(), &mut cache).unwrap();
        assert_eq!(back, game);
        // And it steps on exactly as the original does.
        let (mut a, mut b) = (game, back);
        for _ in 0..300 {
            a.step(Input::default());
            b.step(Input::default());
        }
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn a_step_moves_kilobytes_not_the_house() {
        let state = run().to_state();
        assert!(state.len() < 16_000, "{} bytes", state.len());
        assert!(!state.contains("\"rooms\""));
    }

    #[test]
    fn a_newer_state_is_refused() {
        let state = run().to_state().replacen(
            &format!("\"version\":{STATE_VERSION}"),
            &format!("\"version\":{}", STATE_VERSION + 1),
            1,
        );
        assert!(Game::from_state(&state, &mut HouseCache::default()).is_err());
    }

    #[test]
    fn slower_man_and_more_closets_apply_mid_run() {
        let mut game = run();
        let closets = game.house.closets.len();
        let pos = game.player.pos;
        let mut tuning = game.tuning;
        tuning.man_chase *= 0.8;
        tuning.extra_closets += 2;
        assert_eq!(
            game.retune(tuning),
            Retune::Applied {
                house_changed: true
            }
        );
        assert_eq!(game.man.tuning.man_chase, tuning.man_chase);
        assert!(game.house.closets.len() > closets);
        assert_eq!(game.player.pos, pos);
        // The new state reads back with the new house.
        let back = Game::from_state(&game.to_state(), &mut HouseCache::default()).unwrap();
        assert_eq!(back.house.closets.len(), game.house.closets.len());
    }

    #[test]
    fn a_bigger_house_waits_for_the_next_run() {
        let mut game = run();
        let before = game.clone();
        let mut tuning = game.tuning;
        tuning.house_size += 1;
        assert!(matches!(game.retune(tuning), Retune::NextRun(_)));
        assert_eq!(game, before);
    }
}
