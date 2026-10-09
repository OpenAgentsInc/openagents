//! A simple player: heads for the nearest carrot it can reach at its size,
//! steers around obstacles it would bump, and turns at junctions. Tests use
//! it to show a garden can be cleared with the real controls.

use crate::UNIT;
use crate::game::{BUFFER, Game, Input, Move, Status};
use crate::garden::{Contact, Garden};

const FAR: i64 = i64::MAX / 4;
/// What turning back costs, as extra distance.
const TURN_BACK: i64 = 6 * UNIT as i64;
/// How far ahead it looks for obstacles.
const LOOKAHEAD: i32 = 5 * UNIT;

/// Junction distances from `sources` (node, starting distance) at the
/// bunny's size, through corridors no standing obstacle bars.
fn distances(game: &Game, sources: &[(usize, i64)]) -> Vec<i64> {
    let g = &game.garden;
    let tier = game.bunny.tier;
    let mut dist = vec![FAR; g.nodes.len()];
    let mut done = vec![false; g.nodes.len()];
    for (node, d) in sources {
        dist[*node] = dist[*node].min(*d);
    }
    loop {
        let Some(node) = (0..g.nodes.len())
            .filter(|n| !done[*n] && dist[*n] < FAR)
            .min_by_key(|n| dist[*n])
        else {
            break;
        };
        done[node] = true;
        for exit in g.exits(node) {
            let e = &g.edges[exit.edge];
            if g.barred(exit.edge, 0, e.len, tier, &game.alive) {
                continue;
            }
            let next = e.end(exit.fwd);
            let d = dist[node] + i64::from(e.len);
            if d < dist[next] {
                dist[next] = d;
            }
        }
    }
    dist
}

/// The cost of a carrot from junction distances, or `FAR`.
fn carrot_cost(game: &Game, dist: &[i64], carrot: usize) -> i64 {
    let g = &game.garden;
    let c = g.carrots[carrot];
    let e = &g.edges[c.edge];
    let tier = game.bunny.tier;
    let mut best = FAR;
    if dist[e.a] < FAR && !g.barred(c.edge, 0, c.s, tier, &game.alive) {
        best = best.min(dist[e.a] + i64::from(c.s));
    }
    if dist[e.b] < FAR && !g.barred(c.edge, e.len, c.s, tier, &game.alive) {
        best = best.min(dist[e.b] + i64::from(e.len - c.s));
    }
    best
}

enum Plan {
    /// The carrot is ahead in this corridor, in this lane (own frame).
    Here(i8),
    /// Turn back.
    Back,
    /// Run to the junction ahead and leave it toward this carrot.
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
    let ahead_open = !g.barred(b.edge, b.s, e.end_s(b.fwd), tier, &game.alive);
    let behind_open = !g.barred(b.edge, b.s, e.end_s(!b.fwd), tier, &game.alive);
    let from_ahead = distances(game, &[(ahead, 0)]);
    let from_behind = distances(game, &[(behind, 0)]);
    let mut best: Option<(i64, Plan)> = None;
    let mut consider = |cost: i64, plan: Plan| {
        if cost < FAR && best.as_ref().is_none_or(|(old, _)| cost < *old) {
            best = Some((cost, plan));
        }
    };
    for (index, c) in g.carrots.iter().enumerate() {
        if game.eaten[index] {
            continue;
        }
        if c.edge == b.edge && !g.barred(b.edge, b.s, c.s, tier, &game.alive) {
            let along = i64::from((c.s - b.s).abs());
            let in_front = if b.fwd { c.s > b.s } else { c.s < b.s };
            if in_front {
                consider(along, Plan::Here(if b.fwd { c.lane } else { -c.lane }));
            } else {
                consider(along + TURN_BACK, Plan::Back);
            }
        }
        if ahead_open {
            consider(
                i64::from(to_end) + carrot_cost(game, &from_ahead, index),
                Plan::Via(index),
            );
        }
        if behind_open {
            consider(
                i64::from(e.len - to_end) + carrot_cost(game, &from_behind, index) + TURN_BACK,
                Plan::Back,
            );
        }
    }
    best.map(|(_, plan)| plan)
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
    let g = &game.garden;
    let near = b.to_end(g) <= b.speed() * BUFFER as i32 + UNIT / 2;
    match plan(game)? {
        Plan::Back => (waiting || b.cooldown == 0).then_some(Input::Back),
        Plan::Here(lane) => {
            if near {
                return None;
            }
            steer(game, lane)
        }
        Plan::Via(carrot) => {
            let side = exit_toward(game, carrot)?;
            if waiting || near {
                match side {
                    Way::Straight => None,
                    Way::Left => (b.pending.is_none()).then_some(Input::Left),
                    Way::Right => (b.pending.is_none()).then_some(Input::Right),
                    Way::Back => Some(Input::Back),
                }
            } else {
                steer(game, b.lane)
            }
        }
    }
}

enum Way {
    Straight,
    Left,
    Right,
    Back,
}

/// Which way to leave the junction ahead for `carrot`.
fn exit_toward(game: &Game, carrot: usize) -> Option<Way> {
    let g = &game.garden;
    let b = &game.bunny;
    let tier = b.tier;
    let c = g.carrots[carrot];
    let ce = &g.edges[c.edge];
    let mut sources = Vec::new();
    if !g.barred(c.edge, 0, c.s, tier, &game.alive) {
        sources.push((ce.a, i64::from(c.s)));
    }
    if !g.barred(c.edge, ce.len, c.s, tier, &game.alive) {
        sources.push((ce.b, i64::from(ce.len - c.s)));
    }
    let to_carrot = distances(game, &sources);
    let node = g.edges[b.edge].end(b.fwd);
    let (hx, hz) = b.heading(g);
    let mut best: Option<(i64, Way)> = None;
    for exit in g.exits(node) {
        let e = &g.edges[exit.edge];
        let cost = if exit.edge == c.edge {
            let start = e.end_s(!exit.fwd);
            if g.barred(c.edge, start, c.s, tier, &game.alive) {
                FAR
            } else {
                i64::from((c.s - start).abs())
            }
        } else if g.barred(exit.edge, 0, e.len, tier, &game.alive) {
            FAR
        } else {
            i64::from(e.len) + to_carrot[e.end(exit.fwd)]
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
        let cost = if matches!(way, Way::Back) {
            cost + TURN_BACK
        } else {
            cost
        };
        if cost < FAR && best.as_ref().is_none_or(|(old, _)| cost < *old) {
            best = Some((cost, way));
        }
    }
    best.map(|(_, way)| way)
}

/// Moves toward `want` (own frame), unless an obstacle it would bump is
/// coming up in that lane.
fn steer(game: &Game, want: i8) -> Option<Input> {
    let g: &Garden = &game.garden;
    let b = &game.bunny;
    let dir = if b.fwd { 1 } else { -1 };
    let next_row = g
        .cells
        .iter()
        .enumerate()
        .filter(|(index, cell)| {
            game.alive[*index]
                && cell.edge == b.edge
                && (cell.s - b.s) * dir > 0
                && (cell.s - b.s) * dir <= LOOKAHEAD
                && cell.kind.contact(b.tier) == Contact::Block
        })
        .map(|(_, cell)| cell.s)
        .min_by_key(|s| (s - b.s).abs());
    let mut lane = want;
    if let Some(row) = next_row {
        let blocked = |own: i8| {
            let edge_lane = if b.fwd { own } else { -own };
            g.cells.iter().enumerate().any(|(index, cell)| {
                game.alive[index]
                    && cell.edge == b.edge
                    && cell.s == row
                    && cell.lane == edge_lane
                    && cell.kind.contact(b.tier) == Contact::Block
            })
        };
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
    use crate::{Event, HZ};

    #[test]
    fn the_bot_clears_the_first_garden_without_the_farmer() {
        let mut game = Game::new(Garden::first());
        game.farmer_on = false;
        let mut biggest = 0;
        let mut smashed = 0;
        for _ in 0..HZ * 600 {
            let input: Vec<Input> = input(&game).into_iter().collect();
            game.step(&input);
            biggest = biggest.max(game.bunny.tier);
            smashed += game
                .events
                .iter()
                .filter(|e| matches!(e, Event::Smashed(_)))
                .count();
            if game.status != Status::Playing {
                break;
            }
        }
        assert_eq!(
            game.status,
            Status::Won,
            "{} carrots left after {} s",
            game.carrots_left,
            game.tick / HZ
        );
        assert_eq!(biggest, 4, "it grew to a Giant");
        assert!(smashed > 0);
    }
}
