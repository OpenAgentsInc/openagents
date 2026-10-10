//! A simple player: heads for the nearest edible it can reach and eat at
//! its size, keeps away from the farmer, jumps and ducks what it must,
//! steers around what it would bump, and dodges the net. Tests use it to
//! show a garden can be cleared with the real controls; playtests use it to
//! measure difficulty.

use crate::UNIT;
use crate::game::{BUFFER, FarmerState, Game, Input, Move, Status};
use crate::kinds::Contact;

const FAR: i64 = i64::MAX / 4;
/// What turning back costs, as extra distance.
const TURN_BACK: i64 = 6 * UNIT as i64;
/// How far ahead it looks for obstacles.
const LOOKAHEAD: i32 = 5 * UNIT;
/// What passing close to the farmer costs, as extra distance.
const FARMER_COST: i64 = 60 * UNIT as i64;
/// It keeps this far from a farmer coming its way.
const KEEP_AWAY: i32 = 9 * UNIT;

/// The bot. It keeps no memory between ticks yet; the type leaves room.
#[derive(Clone, Debug, Default)]
pub struct Bot;

impl Bot {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// The bot's input for this tick, if any.
    pub fn input(&mut self, game: &Game) -> Option<Input> {
        input(game)
    }
}

/// Whether the farmer is a danger now.
fn farmer_threat(game: &Game) -> bool {
    game.farmer_on
        && !matches!(
            game.farmer.state,
            FarmerState::Shed { .. } | FarmerState::Dazed { .. } | FarmerState::Spooked { .. }
        )
}

/// Junction distances from `sources` (node, starting distance) at the
/// bunny's size, through corridors no standing obstacle bars, with a cost
/// for passing the farmer.
fn distances(game: &Game, sources: &[(usize, i64)]) -> Vec<i64> {
    let g = &game.garden;
    let tier = game.bunny.tier;
    let mut dist = vec![FAR; g.nodes.len()];
    let mut done = vec![false; g.nodes.len()];
    for (node, d) in sources {
        dist[*node] = dist[*node].min(*d);
    }
    let threat = farmer_threat(game);
    let (fx, fz) = game.farmer_point();
    let near_farmer = |node: usize| {
        let n = g.nodes[node];
        threat && (n.x - fx).abs() + (n.z - fz).abs() < 10 * UNIT
    };
    while let Some(node) = (0..g.nodes.len())
        .filter(|n| !done[*n] && dist[*n] < FAR)
        .min_by_key(|n| dist[*n])
    {
        done[node] = true;
        for exit in g.exits(node) {
            let e = &g.edges[exit.edge];
            if g.barred(exit.edge, 0, e.len, tier, &game.alive, &game.eaten) {
                continue;
            }
            let next = e.end(exit.fwd);
            let mut d = dist[node] + i64::from(e.len);
            if threat && exit.edge == game.farmer.edge {
                d += FARMER_COST;
            }
            if near_farmer(next) {
                d += FARMER_COST;
            }
            if d < dist[next] {
                dist[next] = d;
            }
        }
    }
    dist
}

fn wanted(game: &Game, index: usize) -> bool {
    !game.eaten[index] && game.garden.edibles[index].kind.min_tier() <= game.bunny.tier
}

/// The cost of an edible from junction distances, or `FAR`.
fn edible_cost(game: &Game, dist: &[i64], index: usize) -> i64 {
    let g = &game.garden;
    let c = g.edibles[index];
    let e = &g.edges[c.edge];
    let tier = game.bunny.tier;
    let mut best = FAR;
    if dist[e.a] < FAR && !g.barred(c.edge, 0, c.s, tier, &game.alive, &game.eaten) {
        best = best.min(dist[e.a] + i64::from(c.s));
    }
    if dist[e.b] < FAR && !g.barred(c.edge, e.len, c.s, tier, &game.alive, &game.eaten) {
        best = best.min(dist[e.b] + i64::from(e.len - c.s));
    }
    best
}

enum Plan {
    /// The edible is ahead in this corridor, in this lane (own frame).
    Here(i8, usize),
    /// Turn back.
    Back,
    /// Run to the junction ahead and leave it toward this edible.
    Via(usize),
}

fn plan(game: &Game) -> Option<Plan> {
    let g = &game.garden;
    let b = &game.bunny;
    let tier = b.tier;
    let e = &g.edges[b.edge];
    let ahead = e.end(b.fwd);
    let behind = e.end(!b.fwd);
    let to_end = b.to_end(g);
    let ahead_open = !g.barred(b.edge, b.s, e.end_s(b.fwd), tier, &game.alive, &game.eaten);
    let behind_open = !g.barred(b.edge, b.s, e.end_s(!b.fwd), tier, &game.alive, &game.eaten);
    let from_ahead = distances(game, &[(ahead, 0)]);
    let from_behind = distances(game, &[(behind, 0)]);
    let mut best: Option<(i64, Plan)> = None;
    let mut consider = |cost: i64, plan: Plan| {
        if cost < FAR && best.as_ref().is_none_or(|(old, _)| cost < *old) {
            best = Some((cost, plan));
        }
    };
    for (index, c) in g.edibles.iter().enumerate() {
        if !wanted(game, index) {
            continue;
        }
        if c.edge == b.edge && !g.barred(b.edge, b.s, c.s, tier, &game.alive, &game.eaten) {
            let along = i64::from((c.s - b.s).abs());
            let in_front = if b.fwd { c.s > b.s } else { c.s < b.s };
            if in_front {
                consider(
                    along,
                    Plan::Here(if b.fwd { c.lane } else { -c.lane }, index),
                );
            } else {
                consider(along + TURN_BACK, Plan::Back);
            }
        }
        if ahead_open {
            consider(
                i64::from(to_end) + edible_cost(game, &from_ahead, index),
                Plan::Via(index),
            );
        }
        if behind_open {
            consider(
                i64::from(e.len - to_end) + edible_cost(game, &from_behind, index) + TURN_BACK,
                Plan::Back,
            );
        }
    }
    best.map(|(_, plan)| plan)
}

/// The farmer's distance ahead of the bunny in its corridor, when he is in
/// it, in front, and a danger.
fn farmer_ahead(game: &Game) -> Option<i32> {
    if !farmer_threat(game) || game.farmer.edge != game.bunny.edge {
        return None;
    }
    let ds = game.farmer.s - game.bunny.s;
    let ahead = if game.bunny.fwd { ds } else { -ds };
    (ahead > 0).then_some(ahead)
}

/// The farmer's distance from the bunny when he stands at or near the
/// junction ahead of it, out of its corridor.
fn farmer_at_the_junction(game: &Game) -> Option<i32> {
    if !farmer_threat(game) || game.farmer.edge == game.bunny.edge {
        return None;
    }
    let g = &game.garden;
    let node = g.nodes[g.edges[game.bunny.edge].end(game.bunny.fwd)];
    let (fx, fz) = game.farmer_point();
    if (node.x - fx).abs() + (node.z - fz).abs() > 6 * UNIT {
        return None;
    }
    Some(game.bunny.to_end(g) + (node.x - fx).abs() + (node.z - fz).abs())
}

/// Whether the farmer is close behind in the bunny's corridor.
fn farmer_behind(game: &Game) -> bool {
    if !farmer_threat(game) || game.farmer.edge != game.bunny.edge {
        return false;
    }
    let ds = game.farmer.s - game.bunny.s;
    let behind = if game.bunny.fwd { -ds } else { ds };
    behind > 0 && behind < 16 * UNIT
}

/// The bot's input for this tick, if any.
#[must_use]
pub fn input(game: &Game) -> Option<Input> {
    if game.status != Status::Playing {
        return None;
    }
    let b = &game.bunny;
    let waiting = match b.mv {
        Move::Run => false,
        Move::Wait | Move::Skid { then: None, .. } => true,
        Move::Skid { .. } | Move::Tumble { .. } | Move::UTurn { .. } => return None,
    };
    // The net: jump out of it.
    if game.farmer.windup > 0 && game.farmer.windup <= 12 {
        let (bx, bz) = game.bunny_point();
        let (fx, fz) = game.farmer_point();
        if (bx - fx).abs() + (bz - fz).abs() < 4 * UNIT && b.air == 0 && b.duck == 0 {
            return Some(Input::Jump);
        }
    }
    if let Some(gap) = farmer_ahead(game).or_else(|| farmer_at_the_junction(game))
        && gap < KEEP_AWAY
    {
        if waiting || b.cooldown == 0 {
            return Some(Input::Back);
        }
        // Can't turn yet: pass him in the lane farthest from his.
        let his = if b.fwd {
            game.farmer.lane_pos
        } else {
            -game.farmer.lane_pos
        };
        let lane = if his > 0 { -1 } else { 1 };
        if lane != b.lane {
            return Some(if lane < b.lane {
                Input::Left
            } else {
                Input::Right
            });
        }
    }
    let g = &game.garden;
    let near = b.to_end(g) <= game.bunny_speed() * BUFFER as i32 + UNIT / 2;
    if !waiting && let Some(action) = act(game) {
        return Some(action);
    }
    match plan(game)? {
        Plan::Back => ((waiting || b.cooldown == 0) && !farmer_behind(game)).then_some(Input::Back),
        Plan::Here(lane, index) => {
            let e = g.edibles[index];
            if e.air && b.air == 0 {
                let gap = (e.s - b.s).abs();
                if gap < UNIT * 18 / 10 && b.lane == lane {
                    return Some(Input::Jump);
                }
            }
            if near {
                return None;
            }
            steer(game, lane)
        }
        Plan::Via(index) => {
            let side = exit_toward(game, index)?;
            if waiting || near {
                match side {
                    Way::Straight => None,
                    Way::Left => b.pending.is_none().then_some(Input::Left),
                    Way::Right => b.pending.is_none().then_some(Input::Right),
                    Way::Back => Some(Input::Back),
                }
            } else {
                steer(game, b.lane)
            }
        }
    }
}

/// What stands in the bunny's lane at distance `s` of its corridor, at its
/// size.
fn contact_at(game: &Game, s: i32, own_lane: i8) -> Option<Contact> {
    let g = &game.garden;
    let b = &game.bunny;
    let lane = if b.fwd { own_lane } else { -own_lane };
    let mut worst: Option<Contact> = None;
    for (index, o) in g.obstacles.iter().enumerate() {
        if game.alive[index] && o.edge == b.edge && o.s == s && o.lane == lane {
            let c = o.kind.contact(b.tier);
            worst = Some(match (worst, c) {
                (_, Contact::Block) | (Some(Contact::Block), _) => Contact::Block,
                (Some(prev), _) if prev != Contact::Pass => prev,
                _ => c,
            });
        }
    }
    if g.lane_blocked(b.edge, s, lane, b.tier, &game.alive, &game.eaten) {
        worst = Some(Contact::Block);
    }
    worst
}

/// The rows of things ahead within `reach`, nearest first.
fn rows_ahead(game: &Game, reach: i32) -> Vec<i32> {
    let g = &game.garden;
    let b = &game.bunny;
    let dir = if b.fwd { 1 } else { -1 };
    let mut rows: Vec<i32> = g
        .obstacles
        .iter()
        .enumerate()
        .filter(|(index, o)| game.alive[*index] && o.edge == b.edge)
        .map(|(_, o)| o.s)
        .chain(
            g.edibles
                .iter()
                .enumerate()
                .filter(|(index, e)| {
                    !game.eaten[*index] && e.edge == b.edge && e.kind.min_tier() > b.tier
                })
                .map(|(_, e)| e.s),
        )
        .filter(|s| (s - b.s) * dir > 0 && (s - b.s) * dir <= reach)
        .collect();
    rows.sort_by_key(|s| (s - b.s).abs());
    rows.dedup();
    rows
}

/// Jumps or ducks what is just ahead in the bunny's lane.
fn act(game: &Game) -> Option<Input> {
    let b = &game.bunny;
    if b.air > 0 || b.duck > 0 {
        return None;
    }
    let row = *rows_ahead(game, UNIT * 16 / 10).first()?;
    let gap = (row - b.s).abs();
    match contact_at(game, row, b.lane) {
        Some(Contact::Jump) if gap <= UNIT * 12 / 10 => Some(Input::Jump),
        Some(Contact::Duck) if gap <= UNIT * 8 / 10 => Some(Input::Duck),
        _ => None,
    }
}

enum Way {
    Straight,
    Left,
    Right,
    Back,
}

/// Which way to leave the junction ahead for edible `index`.
fn exit_toward(game: &Game, index: usize) -> Option<Way> {
    let g = &game.garden;
    let b = &game.bunny;
    let tier = b.tier;
    let c = g.edibles[index];
    let ce = &g.edges[c.edge];
    let mut sources = Vec::new();
    if !g.barred(c.edge, 0, c.s, tier, &game.alive, &game.eaten) {
        sources.push((ce.a, i64::from(c.s)));
    }
    if !g.barred(c.edge, ce.len, c.s, tier, &game.alive, &game.eaten) {
        sources.push((ce.b, i64::from(ce.len - c.s)));
    }
    let to_edible = distances(game, &sources);
    let node = g.edges[b.edge].end(b.fwd);
    let (hx, hz) = b.heading(g);
    let mut best: Option<(i64, Way)> = None;
    for exit in g.exits(node) {
        let e = &g.edges[exit.edge];
        let cost = if exit.edge == c.edge {
            let start = e.end_s(!exit.fwd);
            if g.barred(c.edge, start, c.s, tier, &game.alive, &game.eaten) {
                FAR
            } else {
                i64::from((c.s - start).abs())
            }
        } else if g.barred(exit.edge, 0, e.len, tier, &game.alive, &game.eaten) {
            FAR
        } else {
            i64::from(e.len) + to_edible[e.end(exit.fwd)]
        };
        let way = if (exit.dx, exit.dz) == (hx, hz) {
            Way::Straight
        } else if (exit.dx, exit.dz) == (hz, -hx) {
            Way::Left
        } else if (exit.dx, exit.dz) == (-hz, hx) {
            Way::Right
        } else {
            Way::Back
        };
        let mut cost = if matches!(way, Way::Back) {
            cost + TURN_BACK
        } else {
            cost
        };
        if farmer_threat(game) && exit.edge == game.farmer.edge && cost < FAR {
            cost += FARMER_COST;
        }
        if cost < FAR && best.as_ref().is_none_or(|(old, _)| cost < *old) {
            best = Some((cost, way));
        }
    }
    best.map(|(_, way)| way)
}

/// Moves toward `want` (own frame), unless something it would bump is
/// coming up in that lane.
fn steer(game: &Game, want: i8) -> Option<Input> {
    let b = &game.bunny;
    let mut lane = want;
    // Only the nearest row decides; farther rows are steered for later.
    if let Some(row) = rows_ahead(game, LOOKAHEAD).first().copied() {
        let blocked = |own: i8| contact_at(game, row, own) == Some(Contact::Block);
        if blocked(lane) {
            lane = [b.lane, 0, -1, 1]
                .into_iter()
                .filter(|l| !blocked(*l))
                .min_by_key(|l| (l - want).abs())?;
        }
    }
    match lane.cmp(&b.lane) {
        std::cmp::Ordering::Less => Some(Input::Left),
        std::cmp::Ordering::Greater => Some(Input::Right),
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, HZ, level};

    fn play(number: usize, seed: u64, farmer: bool) -> (Status, u32, u8, usize) {
        let mut game = Game::with_seed(level::garden(number), seed, false);
        game.farmer_on = farmer;
        let mut biggest = 0;
        let mut smashed = 0;
        for _ in 0..HZ * 600 {
            let inputs: Vec<Input> = input(&game).into_iter().collect();
            game.step(&inputs);
            biggest = biggest.max(game.bunny.tier);
            smashed += game
                .events
                .iter()
                .filter(|e| matches!(e, Event::Smashed(..)))
                .count();
            if game.status != Status::Playing {
                break;
            }
        }
        (
            game.status,
            game.tick / HZ,
            biggest,
            smashed + game.food_left * 1_000_000,
        )
    }

    #[test]
    fn the_bot_clears_every_garden_without_the_farmer() {
        for number in 1..=level::COUNT {
            let (status, seconds, biggest, left) = play(number, 0, false);
            assert_eq!(
                status,
                Status::Won,
                "garden {number}: {} left after {seconds} s",
                left / 1_000_000
            );
            assert_eq!(biggest, 4, "garden {number}: it grew to a Giant");
        }
    }

    /// The spec's bot playtest: the bot clears garden 1 in at least 90% of
    /// 200 seeds with the farmer on, on Normal.
    #[test]
    fn the_bot_clears_garden_one_in_nine_of_ten_seeds_with_the_farmer() {
        let seeds = 200;
        let won = (0..seeds)
            .filter(|seed| play(1, *seed, true).0 == Status::Won)
            .count();
        println!("the bot cleared garden 1 in {won} of {seeds} seeds");
        assert!(won * 10 >= seeds as usize * 9, "{won} of {seeds}");
    }

    /// How often the bot clears each garden with the farmer, for tuning.
    #[test]
    #[ignore = "a playtest report"]
    fn playtest_every_garden() {
        for number in 1..=level::COUNT {
            let runs: Vec<(Status, u32, u8, usize)> =
                (0..50).map(|seed| play(number, seed, true)).collect();
            let won = runs.iter().filter(|r| r.0 == Status::Won).count();
            let time: u32 = runs
                .iter()
                .filter(|r| r.0 == Status::Won)
                .map(|r| r.1)
                .sum();
            println!(
                "garden {number}: cleared {won} of 50, {} s on average",
                time / won.max(1) as u32
            );
        }
    }
}
