//! Bounded ground navigation over the same footprints as ordinary walking.
//! Routes use a two-meter grid and exact segment clearance; they never move a player.
use std::{cmp::Reverse, collections::BinaryHeap, fmt};

use crate::controller::{Footprint, RADIUS};

const CELL: f32 = 2.0;
const CLEARANCE: f32 = RADIUS + 0.05;
const MAX_NODES: usize = 80_000;
const MAX_BLOCKERS: usize = 2_048;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationStatus {
    #[default]
    Idle,
    Walking,
    Arrived,
    Cancelled,
    Blocked,
}

#[derive(Clone, Debug, Default)]
pub struct Navigation {
    pub(crate) status: NavigationStatus,
    pub(crate) route: Option<Route>,
    pub(crate) next: usize,
    pub(crate) stalled: f32,
}
impl Navigation {
    #[must_use]
    pub fn status(&self) -> NavigationStatus {
        self.status
    }
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status == NavigationStatus::Walking
    }
    #[must_use]
    pub fn destination(&self) -> Option<[f32; 2]> {
        self.route.as_ref().map(|r| r.destination)
    }
    #[must_use]
    pub fn waypoints(&self) -> &[[f32; 2]] {
        self.route
            .as_ref()
            .map_or(&[], |r| &r.waypoints[self.next..])
    }
    pub(crate) fn start(&mut self, route: Route) {
        self.route = Some(route);
        self.next = 0;
        self.stalled = 0.0;
        self.status = NavigationStatus::Walking;
    }
    pub(crate) fn stop(&mut self, status: NavigationStatus) {
        self.status = status;
        if let Some(route) = &self.route {
            self.next = route.waypoints.len();
        }
        self.stalled = 0.0;
    }
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
    if !in_bounds(a, half) || !in_bounds(b, half) {
        return false;
    }
    !blockers.iter().any(|block| {
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
    })
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
    let walkable: Vec<bool> = (0..count)
        .map(|i| clear(point(i, side, half), blockers, half))
        .collect();
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
                if walkable[index] && segment_clear(at, p, blockers, half) {
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
                    && segment_clear(
                        point(current, side, half),
                        point(next, side, half),
                        blockers,
                        half,
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
            if segment_clear(anchor, next, blockers, half) {
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
    #[test]
    fn every_map_landmark_has_a_clear_route_from_spawn() {
        let world = crate::world::build();
        for landmark in crate::minimap::LANDMARKS {
            let route = plan(
                [crate::world::SPAWN.x, crate::world::SPAWN.z],
                [landmark.x, landmark.z],
                &world.blockers,
                crate::world::HALF,
            );
            assert!(route.is_ok(), "{}: {route:?}", landmark.label);
        }
    }

    #[test]
    fn seeded_world_gym_is_reachable_through_the_doorway() {
        let world = crate::world::build();
        let route = plan(
            [0.0, -10.0],
            [48.0, 0.0],
            &world.blockers,
            crate::world::HALF,
        )
        .unwrap();
        let mut previous = [0.0, -10.0];
        for next in route.waypoints {
            assert!(segment_clear(
                previous,
                next,
                &world.blockers,
                crate::world::HALF
            ));
            previous = next;
        }
        assert_eq!(previous, [48.0, 0.0]);
    }
}
