//! A simple player for tests, captures and Game Lab's measures: walks to
//! the nearest key, solves the spear puzzle when the earned weapons are
//! in the game, then goes to the front door and holds E. On the way it
//! picks up dropped weapons, eats a dropped cone when hurt, fudges the
//! Ice Cream Man when he is in view (and hits him with what it has while
//! he's frozen), farts him when he is close, and fights off the minions
//! that come near.

use crate::game::{DropKind, Game, Input, Tool};
use crate::geom::{V2, first_hit};
use crate::man::ManState;
use crate::tuning::PLAYER_RADIUS;

#[derive(Clone, Debug, Default)]
pub struct Bot {
    path: Vec<V2>,
    target: Option<V2>,
    replan: u32,
}

/// The length of the route from the player to `to`.
fn route_length(game: &Game, to: V2) -> f32 {
    let route = game.house.route(game.player.pos, to, PLAYER_RADIUS);
    let mut last = game.player.pos;
    route.iter().fold(0.0, |sum, p| {
        let d = p.distance(last);
        last = *p;
        sum + d
    })
}

impl Bot {
    /// Where to go next, and whether E is to be pressed there (a button)
    /// rather than held (the front door).
    fn goal(game: &Game) -> (V2, bool) {
        let player = &game.player;
        // A weapon on the floor nearby, or a cone when hurt.
        let hurt = player.health < game.tuning.max_health * 0.75;
        let detour = game
            .drops
            .iter()
            .filter(|d| match d.kind {
                DropKind::Weapon(_) => d.at.distance(player.pos) < 14.0,
                DropKind::Cone => hurt && d.at.distance(player.pos) < 10.0,
            })
            .min_by(|a, b| {
                a.at.distance(player.pos)
                    .total_cmp(&b.at.distance(player.pos))
            });
        if let Some(d) = detour {
            return (d.at, false);
        }
        let keys_left: Vec<V2> = game
            .house
            .keys
            .iter()
            .zip(&game.taken)
            .filter(|(_, taken)| !**taken)
            .map(|(key, _)| *key)
            .collect();
        if let Some(key) = keys_left
            .iter()
            .copied()
            .min_by(|a, b| route_length(game, *a).total_cmp(&route_length(game, *b)))
        {
            return (key, false);
        }
        if game.weapons_on() && !game.has(Tool::Spear) {
            if !game.solved {
                if let Some(button) = game.house.buttons.get(game.lit as usize) {
                    return (*button, true);
                }
            } else if let Some(d) = game
                .drops
                .iter()
                .find(|d| d.kind == DropKind::Weapon(Tool::Spear))
            {
                return (d.at, false);
            }
        }
        (game.house.front.stand, false)
    }

    /// The next step's input.
    pub fn input(&mut self, game: &Game) -> Input {
        let player = &game.player;
        let mut input = Input::default();
        if player.hidden.is_some() {
            input.interact = game.tick % 2 == 0;
            return input;
        }
        let (target, press) = Self::goal(game);
        if self.target != Some(target) || self.replan == 0 || self.path.is_empty() {
            self.path = game.house.route(player.pos, target, PLAYER_RADIUS);
            self.target = Some(target);
            self.replan = 15;
        }
        self.replan -= 1;
        while self.path.len() > 1 && self.path[0].distance(player.pos) < 0.3 {
            self.path.remove(0);
        }
        let all_keys = game.taken.iter().all(|t| *t);
        let at_door =
            all_keys && target == game.house.front.stand && player.pos.distance(target) < 0.5;
        let at_button = press && player.pos.distance(target) < 0.5;
        if at_door {
            input.interact = true;
        } else if at_button {
            input.interact = game.tick % 2 == 0;
        } else if let Some(next) = self.path.first() {
            let to = *next - player.pos;
            input.walk = (to * 4.0).clamped(1.0);
        }

        let t = &game.tuning;
        let sees = |at: V2, range: f32| {
            at.distance(player.pos) < range
                && first_hit(&game.house.blocks, player.pos, at, 0.0, 1.0).is_none()
        };
        // The nearest of his minions in view and close.
        let minion = game
            .minions
            .iter()
            .filter(|m| !m.friendly && m.health > 0.0 && sees(m.pos, 7.0))
            .min_by(|a, b| {
                a.pos
                    .distance(player.pos)
                    .total_cmp(&b.pos.distance(player.pos))
            })
            .map(|m| m.pos);
        let chased = game.man_here() && game.man.state == ManState::Chase;
        input.sprint = (chased || minion.is_some_and(|m| m.distance(player.pos) < 4.0))
            && !at_door
            && !at_button;
        let man = game.man_here().then_some(game.man.pos);
        let man_d = man.map_or(f32::MAX, |m| m.distance(player.pos));
        let minion_first = minion.is_some_and(|m| m.distance(player.pos) < man_d);

        if let (Some(at), true) = (minion, minion_first || man.is_none_or(|m| !sees(m, 12.0))) {
            let d = at.distance(player.pos);
            let pick = if game.has(Tool::Sword) && d <= t.sword_reach + 0.2 {
                Some(Tool::Sword)
            } else if game.has(Tool::Spear) && d <= t.spear_reach {
                Some(Tool::Spear)
            } else if player.fudge > 0 {
                Some(Tool::Fudge)
            } else if game.has(Tool::Pistol) && player.cones > 0 {
                Some(Tool::Pistol)
            } else {
                None
            };
            if let Some(tool) = pick {
                input.aim = (at - player.pos).normalized();
                input.select = Some(tool);
                input.fire = true;
            }
            return input;
        }

        if let Some(at) = man.filter(|m| sees(*m, 12.0)) {
            let to = at - player.pos;
            let distance = to.length();
            let dazed = matches!(game.man.state, ManState::Frozen | ManState::Confused);
            input.aim = to.normalized();
            let hitter = if game.has(Tool::Spear) && distance <= t.spear_reach {
                Some(Tool::Spear)
            } else if game.has(Tool::Sword) && distance <= t.sword_reach {
                Some(Tool::Sword)
            } else if game.has(Tool::Sugar)
                && player.sugar > 0
                && distance > t.sugar_radius + 0.5
                && distance < t.sugar_range
            {
                Some(Tool::Sugar)
            } else if game.has(Tool::Pistol) && player.cones > 0 {
                Some(Tool::Pistol)
            } else {
                None
            };
            if dazed {
                // Frozen or confused: hit him with what it has.
                if let Some(tool) = hitter {
                    input.select = Some(tool);
                    input.fire = true;
                }
            } else if distance < t.fart_radius - 0.8 && player.farts > 0 {
                input.select = Some(Tool::Fart);
                input.fire = true;
            } else if player.fudge > 0 {
                input.select = Some(Tool::Fudge);
                input.fire = true;
            } else if let Some(tool) = hitter {
                input.select = Some(tool);
                input.fire = true;
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

    #[test]
    fn the_bot_earns_the_new_weapons_and_survives_minions() {
        let mut tuning = crate::Tuning::default();
        tuning.earned_weapons = 1;
        tuning.minion_count = 3;
        let mut spears = 0;
        let mut melted = 0;
        let mut won = 0;
        for seed in 0..12 {
            let mut game = Game::tuned(seed, Options::default(), tuning);
            let mut bot = Bot::default();
            for _ in 0..600 * crate::HZ {
                let input = bot.input(&game);
                game.step(input);
                if game.status != Status::Playing {
                    break;
                }
            }
            spears += u32::from(game.has(Tool::Spear));
            melted += game.stats.minions_melted;
            won += u32::from(matches!(game.status, Status::Escaped | Status::Melted));
        }
        assert!(spears >= 9, "the bot solves the button puzzle: {spears}");
        assert!(melted >= 6, "and fights off minions: {melted}");
        assert!(won >= 10, "{won} of 12");
    }
}

/// A seed the bot escapes with the Ice Cream Man in the house: the
/// browser check plays it.
pub const ESCAPE_SEED: u64 = 7;
