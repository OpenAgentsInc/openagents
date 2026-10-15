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
        let locked = game.locked_key();
        let keys_left: Vec<V2> = game
            .house
            .keys
            .iter()
            .zip(&game.taken)
            .enumerate()
            .filter(|(i, (_, taken))| !**taken && Some(*i) != locked)
            .map(|(_, (key, _))| *key)
            .collect();
        if let Some(key) = keys_left
            .iter()
            .copied()
            .min_by(|a, b| route_length(game, *a).total_cmp(&route_length(game, *b)))
        {
            return (key, false);
        }
        // The puzzle: for the key locked in the cone cabinet, or the spear.
        if locked.is_some() || (game.weapons_on() && !game.has(Tool::Spear)) {
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
        // The browser checks play this seed, the speedrun check in under
        // four minutes.
        assert!(escaped.contains(&crate::bot::ESCAPE_SEED), "{escaped:?}");
        let seven = play(ESCAPE_SEED, Options::default(), 600);
        eprintln!("seed {ESCAPE_SEED}: {:.1} s", seven.seconds());
        assert!(seven.seconds() < 200.0, "{}", seven.seconds());
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
    fn a_perfect_run_takes_a_minute_or_more_on_normal() {
        use crate::tuning::Difficulty;
        // The route-optimal player alone in the house: the fastest escape
        // there can be. Version 4 (Easy) was 11 seconds (#11667).
        let alone = Options { man: false };
        let easy = measure(Difficulty::Easy.tuning(), 0..24, alone, true, 600);
        let normal = measure(Difficulty::Normal.tuning(), 0..24, alone, true, 600);
        let hard = measure(Difficulty::Hard.tuning(), 0..24, alone, true, 600);
        for m in [&easy, &normal, &hard] {
            assert_eq!(m.wins, 24, "{m:?}");
        }
        assert!(easy.fastest().unwrap() < 20.0, "{easy:?}");
        assert!(normal.fastest().unwrap() >= 60.0, "{normal:?}");
        assert!(
            hard.fastest().unwrap() > normal.fastest().unwrap(),
            "{hard:?}"
        );
        assert!(
            hard.median().unwrap() > normal.median().unwrap(),
            "{hard:?}"
        );
    }

    /// Prints the measures (`cargo test -p icecream-rules measures --
    /// --ignored --nocapture`).
    #[test]
    #[ignore = "a report, not a check"]
    fn measures() {
        let clock = |s: Option<f32>| s.map_or("-".to_owned(), |s| format!("{s:.1}"));
        for difficulty in crate::tuning::Difficulty::ALL {
            let (name, mut tuning) = (difficulty.id(), difficulty.tuning());
            // Try changes: ICE_TRY="normal:unlock=6,loops=8".
            for change in std::env::var("ICE_TRY").unwrap_or_default().split(';') {
                if let Some((who, pairs)) = change.split_once(':')
                    && who == name
                {
                    for pair in pairs.split(',') {
                        if let Some((k, v)) = pair.split_once('=') {
                            tuning.set(k, v.parse().unwrap()).unwrap();
                        }
                    }
                }
            }
            let alone = measure(tuning, 0..24, Options { man: false }, true, 600);
            let racer = measure(tuning, 0..24, Options::default(), true, 600);
            let bot = measure(tuning, 0..24, Options::default(), false, 600);
            eprintln!(
                "{name}: racer alone fastest {} median {}; racer with him {} wins, fastest {} median {}; bot {} wins, fastest {} median {}",
                clock(alone.fastest()),
                clock(alone.median()),
                racer.wins,
                clock(racer.fastest()),
                clock(racer.median()),
                bot.wins,
                clock(bot.fastest()),
                clock(bot.median()),
            );
        }
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

/// The length of the route between two points for the player's body.
fn leg(house: &crate::layout::House, from: V2, to: V2) -> f32 {
    let mut last = from;
    house
        .route(from, to, PLAYER_RADIUS)
        .iter()
        .fold(0.0, |sum, p| {
            let d = p.distance(last);
            last = *p;
            sum + d
        })
}

/// The route-optimal player: the fastest a perfect run could be. It knows
/// where every key and button is from the start, takes them in the order
/// with the shortest route (the sprinkle buttons in their order, then the
/// cone cabinet, when a key is locked in it), sprints the whole way,
/// never hides and never fights, then holds E at the front door. Game
/// Lab's difficulty measures use it for the fastest possible escape.
#[derive(Clone, Debug, Default)]
pub struct Racer {
    plan: Vec<(V2, bool)>,
    planned: bool,
    path: Vec<V2>,
    target: Option<V2>,
    replan: u32,
}

impl Racer {
    /// The stops: each key left on the floor, and the puzzle's buttons
    /// then its cabinet while a key is locked in it; pressed (`true`) or
    /// walked over.
    fn plan(game: &Game) -> Vec<(V2, bool)> {
        let house = &game.house;
        let locked = game.locked_key();
        let keys: Vec<V2> = house
            .keys
            .iter()
            .enumerate()
            .filter(|(i, _)| !game.taken[*i] && Some(*i) != locked)
            .map(|(_, k)| *k)
            .collect();
        let chain: Vec<V2> = if locked.is_some() && !game.solved {
            house.buttons[game.lit as usize..]
                .iter()
                .copied()
                .chain([house.cabinet])
                .collect()
        } else {
            Vec::new()
        };
        // Every order of the keys, with the puzzle's chain woven through
        // in its own order: few enough stops to try them all.
        let n = keys.len() + chain.len();
        let mut best: (f32, Vec<(V2, bool)>) = (f32::MAX, Vec::new());
        let mut order: Vec<usize> = (0..n).collect();
        let mut walk = |order: &[usize]| {
            // Chain stops must come in their order.
            let chain_seen: Vec<usize> =
                order.iter().copied().filter(|i| *i >= keys.len()).collect();
            if chain_seen.windows(2).any(|w| w[0] > w[1]) {
                return;
            }
            let stops: Vec<(V2, bool)> = order
                .iter()
                .map(|i| {
                    if *i < keys.len() {
                        (keys[*i], false)
                    } else {
                        let c = *i - keys.len();
                        (chain[c], c + 1 < chain.len())
                    }
                })
                .collect();
            let mut at = game.player.pos;
            let mut length = 0.0;
            for (stop, _) in stops.iter().chain([&(house.front.stand, false)]) {
                length += leg(house, at, *stop);
                at = *stop;
                if length >= best.0 {
                    return;
                }
            }
            best = (length, stops);
        };
        permute(&mut order, 0, &mut walk);
        best.1
    }

    /// The next step's input.
    pub fn input(&mut self, game: &Game) -> Input {
        let player = &game.player;
        let mut input = Input::default();
        if player.hidden.is_some() {
            input.interact = game.tick % 2 == 0;
            return input;
        }
        if !self.planned {
            self.plan = Self::plan(game);
            self.planned = true;
        }
        // Drop stops already done.
        while let Some((stop, press)) = self.plan.first().copied() {
            let done = if press {
                !game.house.buttons[..game.lit as usize]
                    .iter()
                    .all(|b| b.distance(stop) > 0.01)
                    || game.solved
            } else {
                let key = game.house.keys.iter().position(|k| k.distance(stop) < 0.01);
                key.is_some_and(|i| game.taken[i])
                    || (key.is_none() && game.solved && stop.distance(game.house.cabinet) < 0.01)
            };
            if done {
                self.plan.remove(0);
            } else {
                break;
            }
        }
        let (target, press) = self
            .plan
            .first()
            .copied()
            .unwrap_or((game.house.front.stand, false));
        if self.target != Some(target) || self.replan == 0 || self.path.is_empty() {
            self.path = game.house.route(player.pos, target, PLAYER_RADIUS);
            self.target = Some(target);
            self.replan = 15;
        }
        self.replan -= 1;
        while self.path.len() > 1 && self.path[0].distance(player.pos) < 0.3 {
            self.path.remove(0);
        }
        let near = player.pos.distance(target) < 0.5;
        let at_door = self.plan.is_empty() && near;
        if at_door {
            input.interact = true;
        } else if press && near {
            input.interact = game.tick % 2 == 0;
        } else if let Some(next) = self.path.first() {
            input.walk = ((*next - player.pos) * 4.0).clamped(1.0);
            input.sprint = true;
        }
        input
    }
}

/// Calls `f` with every order of `items[k..]` after `items[..k]`.
fn permute(items: &mut Vec<usize>, k: usize, f: &mut impl FnMut(&[usize])) {
    if k == items.len() {
        f(items);
        return;
    }
    for i in k..items.len() {
        items.swap(k, i);
        permute(items, k + 1, f);
        items.swap(k, i);
    }
}

/// One bot's runs over seeded houses: wins, and the escape times.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measure {
    pub runs: u32,
    pub wins: u32,
    /// Seconds of each escape, shortest first.
    pub escapes: Vec<f32>,
}

impl Measure {
    #[must_use]
    pub fn fastest(&self) -> Option<f32> {
        self.escapes.first().copied()
    }

    #[must_use]
    pub fn median(&self) -> Option<f32> {
        (!self.escapes.is_empty()).then(|| self.escapes[self.escapes.len() / 2])
    }
}

/// Plays `seeds` with `tuning`, the regular bot (`race` false) or the
/// [`Racer`], for up to `limit` seconds each.
#[must_use]
pub fn measure(
    tuning: crate::Tuning,
    seeds: std::ops::Range<u64>,
    options: crate::game::Options,
    race: bool,
    limit: u32,
) -> Measure {
    use crate::game::Status;
    let runs: Vec<(Status, f32)> = std::thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .map(|seed| {
                scope.spawn(move || {
                    let mut game = Game::tuned(seed, options, tuning);
                    let (mut bot, mut racer) = (Bot::default(), Racer::default());
                    for _ in 0..limit * crate::HZ {
                        let input = if race {
                            racer.input(&game)
                        } else {
                            bot.input(&game)
                        };
                        game.step(input);
                        if game.status != Status::Playing {
                            break;
                        }
                    }
                    (game.status, game.seconds())
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut escapes: Vec<f32> = runs
        .iter()
        .filter(|(s, _)| *s == Status::Escaped)
        .map(|(_, t)| *t)
        .collect();
    escapes.sort_by(f32::total_cmp);
    Measure {
        runs: runs.len() as u32,
        wins: runs
            .iter()
            .filter(|(s, _)| matches!(s, Status::Escaped | Status::Melted))
            .count() as u32,
        escapes,
    }
}
