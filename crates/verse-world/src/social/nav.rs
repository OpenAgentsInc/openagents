//! Bounded ground navigation over the same footprints as ordinary walking,
//! shared by Verse's zones and a social world's seat actors.
//! Routes use a two-meter grid and exact segment clearance; they never move a player.
use std::{cmp::Reverse, collections::BinaryHeap, fmt};

use super::controller::{Footprint, RADIUS};

const CELL: f32 = 2.0;
const CLEARANCE: f32 = RADIUS + 0.05;
const MAX_NODES: usize = 80_000;
const MAX_BLOCKERS: usize = 4_096;
/// Side of the square buckets a plan indexes blockers in, m, so a grid
/// step tests only the blockers near it.
const BUCKET: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavError {
    InvalidDestination,
    StartBlocked,
    DestinationBlocked,
    NoRoute,
    WorldBounds,
}
impl fmt::Display for NavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDestination => "Choose a point inside the map",
            Self::StartBlocked => "Move away from the obstacle first",
            Self::DestinationBlocked => "That destination is blocked",
            Self::NoRoute => "No route found",
            Self::WorldBounds => "This world exceeds navigation limits",
        })
    }
}
impl std::error::Error for NavError {}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub destination: [f32; 2],
    /// Ordered ground positions, excluding the start and including the exact goal.
    pub waypoints: Vec<[f32; 2]>,
}

fn in_bounds(point: [f32; 2], half: f32) -> bool {
    point
        .iter()
        .all(|v| v.is_finite() && v.abs() <= half - RADIUS)
}
fn clear(point: [f32; 2], blockers: &[Footprint], half: f32) -> bool {
    in_bounds(point, half)
        && !blockers.iter().any(|b| {
            (b.min[0] - CLEARANCE..=b.max[0] + CLEARANCE).contains(&point[0])
                && (b.min[1] - CLEARANCE..=b.max[1] + CLEARANCE).contains(&point[1])
        })
}

/// Exact clearance for a straight ground segment, including thin walls and corners.
#[must_use]
pub fn segment_clear(a: [f32; 2], b: [f32; 2], blockers: &[Footprint], half: f32) -> bool {
    in_bounds(a, half) && in_bounds(b, half) && !blockers.iter().any(|block| crosses(a, b, block))
}

/// Whether the segment from `a` to `b` comes within [`CLEARANCE`] of `block`.
fn crosses(a: [f32; 2], b: [f32; 2], block: &Footprint) -> bool {
    let mut enter = 0.0_f32;
    let mut leave = 1.0_f32;
    for axis in 0..2 {
        let lo = block.min[axis] - CLEARANCE;
        let hi = block.max[axis] + CLEARANCE;
        let delta = b[axis] - a[axis];
        if delta.abs() < f32::EPSILON {
            if a[axis] < lo || a[axis] > hi {
                return false;
            }
        } else {
            let t0 = (lo - a[axis]) / delta;
            let t1 = (hi - a[axis]) / delta;
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter > leave {
                return false;
            }
        }
    }
    true
}

/// The blockers of one plan in square buckets over the world, each bucket
/// listing the blockers whose inflated footprint reaches it.
struct Buckets {
    side: usize,
    half: f32,
    lists: Vec<Vec<u32>>,
}

impl Buckets {
    fn new(blockers: &[Footprint], half: f32) -> Self {
        let side = (2.0 * half / BUCKET).ceil() as usize + 1;
        let mut lists = vec![Vec::new(); side * side];
        let mut this = Self {
            side,
            half,
            lists: Vec::new(),
        };
        for (i, block) in blockers.iter().enumerate() {
            let (x0, z0) = this.cell(block.min[0] - CLEARANCE, block.min[1] - CLEARANCE);
            let (x1, z1) = this.cell(block.max[0] + CLEARANCE, block.max[1] + CLEARANCE);
            for z in z0..=z1 {
                for x in x0..=x1 {
                    lists[z * side + x].push(i as u32);
                }
            }
        }
        this.lists = lists;
        this
    }

    /// The bucket holding `(x, z)`, clamped to the world.
    fn cell(&self, x: f32, z: f32) -> (usize, usize) {
        let at = |v: f32| (((v + self.half) / BUCKET).floor().max(0.0) as usize).min(self.side - 1);
        (at(x), at(z))
    }

    /// [`segment_clear`] for a segment between two in-bounds points, testing
    /// only the blockers in the buckets the segment's box reaches.
    fn segment_clear(&self, a: [f32; 2], b: [f32; 2], blockers: &[Footprint]) -> bool {
        let (x0, z0) = self.cell(a[0].min(b[0]), a[1].min(b[1]));
        let (x1, z1) = self.cell(a[0].max(b[0]), a[1].max(b[1]));
        for z in z0..=z1 {
            for x in x0..=x1 {
                for &i in &self.lists[z * self.side + x] {
                    if crosses(a, b, &blockers[i as usize]) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn point(index: usize, side: usize, half: f32) -> [f32; 2] {
    [
        (index % side) as f32 * CELL - half,
        (index / side) as f32 * CELL - half,
    ]
}
fn heuristic(a: usize, b: usize, side: usize) -> u32 {
    let dx = (a % side).abs_diff(b % side) as u32;
    let dy = (a / side).abs_diff(b / side) as u32;
    1000 * dx.max(dy) + 414 * dx.min(dy)
}

/// Plan to an exact clear destination. Blocked destinations are never silently moved.
/// Narrow passages smaller than the grid resolution can conservatively have no route.
pub fn plan(
    start: [f32; 2],
    destination: [f32; 2],
    blockers: &[Footprint],
    half: f32,
) -> Result<Route, NavError> {
    if !half.is_finite()
        || !(CELL..=280.0).contains(&half)
        || blockers.len() > MAX_BLOCKERS
        || blockers.iter().any(|b| {
            (0..2).any(|i| !b.min[i].is_finite() || !b.max[i].is_finite() || b.min[i] > b.max[i])
        })
    {
        return Err(NavError::WorldBounds);
    }
    if !in_bounds(destination, half) {
        return Err(NavError::InvalidDestination);
    }
    if !clear(start, blockers, half) {
        return Err(NavError::StartBlocked);
    }
    if !clear(destination, blockers, half) {
        return Err(NavError::DestinationBlocked);
    }
    if segment_clear(start, destination, blockers, half) {
        return Ok(Route {
            destination,
            waypoints: vec![destination],
        });
    }
    let side = (half * 2.0 / CELL).floor() as usize + 1;
    let count = side
        .checked_mul(side)
        .filter(|n| *n <= MAX_NODES)
        .ok_or(NavError::WorldBounds)?;
    // Every in-bounds grid point is walkable until a blocker's inflated
    // footprint covers it: the same test as `clear`, applied per blocker to
    // the points it can reach rather than per point to every blocker.
    let mut walkable: Vec<bool> = (0..count)
        .map(|i| in_bounds(point(i, side, half), half))
        .collect();
    for block in blockers {
        let range = |axis: usize| {
            let lo = ((block.min[axis] - CLEARANCE + half) / CELL).floor() as i64 - 1;
            let hi = ((block.max[axis] + CLEARANCE + half) / CELL).ceil() as i64 + 1;
            lo.max(0) as usize..=(hi.max(0) as usize).min(side - 1)
        };
        for y in range(1) {
            for x in range(0) {
                let index = y * side + x;
                let p = point(index, side, half);
                if (block.min[0] - CLEARANCE..=block.max[0] + CLEARANCE).contains(&p[0])
                    && (block.min[1] - CLEARANCE..=block.max[1] + CLEARANCE).contains(&p[1])
                {
                    walkable[index] = false;
                }
            }
        }
    }
    let buckets = Buckets::new(blockers, half);
    let nearest = |at: [f32; 2]| -> Option<usize> {
        let x = ((at[0] + half) / CELL).round() as i32;
        let y = ((at[1] + half) / CELL).round() as i32;
        let mut best: Option<(f32, usize)> = None;
        for ny in y - 2..=y + 2 {
            for nx in x - 2..=x + 2 {
                if nx < 0 || ny < 0 || nx >= side as i32 || ny >= side as i32 {
                    continue;
                }
                let index = ny as usize * side + nx as usize;
                let p = point(index, side, half);
                if walkable[index] && buckets.segment_clear(at, p, blockers) {
                    let candidate = (distance(at, p), index);
                    if best.is_none_or(|previous| candidate < previous) {
                        best = Some(candidate);
                    }
                }
            }
        }
        best.map(|(_, i)| i)
    };
    let first = nearest(start).ok_or(NavError::NoRoute)?;
    let last = nearest(destination).ok_or(NavError::NoRoute)?;
    let mut costs = vec![u32::MAX; count];
    let mut parents = vec![usize::MAX; count];
    let mut queue = BinaryHeap::new();
    costs[first] = 0;
    queue.push(Reverse((heuristic(first, last, side), 0_u32, first)));
    let mut visited = 0;
    let mut found = false;
    while let Some(Reverse((_, cost, current))) = queue.pop() {
        if cost != costs[current] {
            continue;
        }
        visited += 1;
        if visited > MAX_NODES {
            return Err(NavError::NoRoute);
        }
        if current == last {
            found = true;
            break;
        }
        let x = (current % side) as i32;
        let y = (current / side) as i32;
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= side as i32 || ny >= side as i32 {
                    continue;
                }
                let next = ny as usize * side + nx as usize;
                let next_cost = cost + if dx == 0 || dy == 0 { 1000 } else { 1414 };
                if walkable[next]
                    && next_cost < costs[next]
                    && buckets.segment_clear(
                        point(current, side, half),
                        point(next, side, half),
                        blockers,
                    )
                {
                    costs[next] = next_cost;
                    parents[next] = current;
                    queue.push(Reverse((
                        next_cost + heuristic(next, last, side),
                        next_cost,
                        next,
                    )));
                }
            }
        }
    }
    if !found {
        return Err(NavError::NoRoute);
    }
    let mut path = vec![destination];
    let mut node = last;
    loop {
        path.push(point(node, side, half));
        if node == first {
            break;
        }
        node = parents[node];
    }
    path.reverse();
    let mut waypoints = Vec::new();
    let mut anchor = start;
    let mut index = 0;
    while index < path.len() {
        let mut farthest = index;
        for (candidate, &next) in path.iter().enumerate().skip(index + 1) {
            if buckets.segment_clear(anchor, next, blockers) {
                farthest = candidate;
            } else {
                break;
            }
        }
        waypoints.push(path[farthest]);
        anchor = path[farthest];
        index = farthest + 1;
    }
    Ok(Route {
        destination,
        waypoints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_are_deterministic_and_do_not_cut_inflated_corners() {
        let walls = [Footprint {
            min: [-2.0, -2.0],
            max: [2.0, 2.0],
        }];
        let route = plan([-8.0, 0.0], [8.0, 0.0], &walls, 20.0).unwrap();
        assert_eq!(route, plan([-8.0, 0.0], [8.0, 0.0], &walls, 20.0).unwrap());
        assert!(route.waypoints.len() >= 2);
        let mut previous = [-8.0, 0.0];
        for next in route.waypoints {
            assert!(segment_clear(previous, next, &walls, 20.0));
            previous = next;
        }
        assert_eq!(previous, [8.0, 0.0]);
        assert!(!segment_clear([-3.0, 0.0], [0.0, -3.0], &walls, 20.0));
    }
    #[test]
    fn rejects_invalid_blocked_and_sealed_destinations() {
        assert_eq!(
            plan([0.0, 0.0], [f32::NAN, 0.0], &[], 20.0),
            Err(NavError::InvalidDestination)
        );
        assert_eq!(
            plan([0.0, 0.0], [21.0, 0.0], &[], 20.0),
            Err(NavError::InvalidDestination)
        );
        let walls = [Footprint {
            min: [-1.0, -20.0],
            max: [1.0, 20.0],
        }];
        assert_eq!(
            plan([-5.0, 0.0], [0.0, 0.0], &walls, 20.0),
            Err(NavError::DestinationBlocked)
        );
        assert_eq!(
            plan([-5.0, 0.0], [5.0, 0.0], &walls, 20.0),
            Err(NavError::NoRoute)
        );
    }
}
