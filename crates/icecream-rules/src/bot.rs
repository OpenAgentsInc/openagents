//! A simple player for tests and captures: walks to the nearest key, then
//! to the front door and holds E; fudges the Ice Cream Man when he is in
//! view and farts him when he is close.

use crate::game::{Game, Input, Tool};
use crate::geom::{V2, first_hit};
use crate::man::ManState;
use crate::tuning::PLAYER_RADIUS;

#[derive(Clone, Debug, Default)]
pub struct Bot {
    path: Vec<V2>,
    target: Option<V2>,
    replan: u32,
}

impl Bot {
    /// The next step's input.
    pub fn input(&mut self, game: &Game) -> Input {
        let player = &game.player;
        let mut input = Input::default();
        if player.hidden.is_some() {
            input.interact = game.tick % 2 == 0;
            return input;
        }
        let keys_left: Vec<V2> = game
            .house
            .keys
            .iter()
            .zip(&game.taken)
            .filter(|(_, taken)| !**taken)
            .map(|(key, _)| *key)
            .collect();
        let target = keys_left
            .iter()
            .copied()
            .min_by(|a, b| {
                let length = |to: V2| {
                    let route = game.house.route(player.pos, to, PLAYER_RADIUS);
                    let mut last = player.pos;
                    route.iter().fold(0.0, |sum, p| {
                        let d = p.distance(last);
                        last = *p;
                        sum + d
                    })
                };
                length(*a).total_cmp(&length(*b))
            })
            .unwrap_or(game.house.front.stand);
        if self.target != Some(target) || self.replan == 0 || self.path.is_empty() {
            self.path = game.house.route(player.pos, target, PLAYER_RADIUS);
            self.target = Some(target);
            self.replan = 15;
        }
        self.replan -= 1;
        while self.path.len() > 1 && self.path[0].distance(player.pos) < 0.3 {
            self.path.remove(0);
        }
        let at_door = keys_left.is_empty() && player.pos.distance(target) < 0.5;
        if at_door {
            input.interact = true;
        } else if let Some(next) = self.path.first() {
            let to = *next - player.pos;
            input.walk = (to * 4.0).clamped(1.0);
        }

        if game.man_here() {
            let man = &game.man;
            let to = man.pos - player.pos;
            let distance = to.length();
            let seen = distance < 12.0
                && first_hit(&game.house.blocks, player.pos, man.pos, 0.0, 1.0).is_none()
                && !matches!(man.state, ManState::Frozen | ManState::Confused);
            input.sprint = man.state == ManState::Chase && !at_door;
            if seen {
                input.aim = to.normalized();
                if distance < game.tuning.fart_radius - 0.8 && player.farts > 0 {
                    input.select = Some(Tool::Fart);
                    input.fire = true;
                } else if player.fudge > 0 {
                    input.select = Some(Tool::Fudge);
                    input.fire = true;
                }
            }
        }
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Options, Status};

    fn play(seed: u64, options: Options, seconds: u32) -> Game {
        let mut game = Game::new(seed, options);
        let mut bot = Bot::default();
        for _ in 0..seconds * crate::HZ {
            let input = bot.input(&game);
            game.step(input);
            if game.status != Status::Playing {
                break;
            }
        }
        game
    }

    #[test]
    fn the_bot_escapes_every_house_when_he_is_out() {
        for seed in 0..20 {
            let game = play(seed, Options { man: false }, 400);
            assert_eq!(game.status, Status::Escaped, "seed {seed}");
        }
    }

    #[test]
    fn the_bot_escapes_seeded_runs_with_him_in_the_house() {
        let escaped: Vec<u64> = (0..20)
            .filter(|seed| play(*seed, Options::default(), 600).status == Status::Escaped)
            .collect();
        assert!(escaped.len() >= 8, "{escaped:?}");
        // The browser check plays this seed.
        assert!(escaped.contains(&crate::bot::ESCAPE_SEED), "{escaped:?}");
    }

    #[test]
    fn the_same_seed_and_inputs_end_the_same_way() {
        let a = play(11, Options::default(), 90);
        let b = play(11, Options::default(), 90);
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_ne!(
            a.fingerprint(),
            play(12, Options::default(), 90).fingerprint()
        );
    }
}

/// A seed the bot escapes with the Ice Cream Man in the house: the
/// browser check plays it.
pub const ESCAPE_SEED: u64 = 7;
