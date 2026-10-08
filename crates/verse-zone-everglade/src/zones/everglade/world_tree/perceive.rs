//! Perceiving and reaching the tree's places. [`sweep`] is what an agent
//! sees from where it stands: each node within reach whose standing point
//! a sight sweep (`social/sight.rs`) reaches unblocked, which it adds to
//! its known subgraph. [`route`] grounds a named node: code resolves it to
//! its standing point and `social/nav.rs` routes there, so an agent never
//! names coordinates.

use glam::Vec3;
use verse_world::social::nav::{self, NavError, Route};
use verse_world::social::sight::Sight;
use world_tree::{Kind, Known, Tree};

use super::super::{HALF_EXTENT, height};
use crate::controller::Footprint;

/// How far an agent notices a place, m.
pub const REACH: f32 = 24.0;
/// The eye's and the target's height over the ground, m.
pub const EYE: f32 = 1.6;
/// The sight sweep's radius, m.
const RADIUS: f32 = 0.05;

/// The nodes `sight` lets an agent with eyes at `eye` see within `reach`
/// m: districts and buildings by their standing points, and rooms and
/// objects too, so long as the line to their standing point is clear.
#[must_use]
pub fn sweep<'t>(tree: &'t Tree, sight: &dyn Sight, eye: Vec3, reach: f32) -> Vec<&'t str> {
    tree.nodes()
        .iter()
        .filter(|n| matches!(n.kind, Kind::Building | Kind::Room | Kind::Object))
        .filter(|n| {
            let [x, z] = n.stand;
            let target = Vec3::new(x, height(x, z) + EYE, z);
            let d = target - eye;
            Vec3::new(d.x, 0.0, d.z).length() <= reach && sight.sweep(eye, target, RADIUS) >= 1.0
        })
        .map(|n| n.id.as_str())
        .collect()
}

/// Updates `known` for an agent at `at`, looking from its eyes: enters the
/// room it stands in and learns what it sees. Returns how many nodes it
/// learned.
pub fn perceive(known: &mut Known, tree: &Tree, sight: &dyn Sight, at: [f32; 2]) -> usize {
    let mut learned = known.enter_at(tree, at);
    let eye = Vec3::new(at[0], height(at[0], at[1]) + EYE, at[1]);
    for id in sweep(tree, sight, eye, REACH) {
        learned += known.see(tree, id);
    }
    learned
}

/// Why a node can't be reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroundError {
    /// No node has the ID.
    Unknown(String),
    Nav(NavError),
}

impl std::fmt::Display for GroundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "{id} isn't in the world tree"),
            Self::Nav(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for GroundError {}

/// The doorways into a place inside a room, outermost first: the room's
/// building's and the room's own, where they have one
/// ([`world_tree::Node::entry`]), with their IDs. A place outside every
/// room has none.
fn doorways<'t>(tree: &'t Tree, id: &str) -> Vec<(&'t str, [[f32; 2]; 2])> {
    let Some(room) = tree.enclosing(id, Kind::Room) else {
        return Vec::new();
    };
    tree.parent(&room.id)
        .into_iter()
        .chain([room])
        .filter_map(|n| n.entry.map(|e| (n.id.as_str(), e)))
        .collect()
}

/// The route from `from` to node `id`'s standing point over `blockers`,
/// and the heading to face there. Navigation's grid can be too coarse for
/// a doorway, so when the direct plan fails the route leaves the room and
/// building `from` stands in through their doorways, enters the target's
/// the same way, crossing each doorway straight, and plans the legs
/// between.
///
/// # Errors
///
/// When the node isn't in the tree, a doorway is blocked, or navigation
/// finds no route.
pub fn route(
    tree: &Tree,
    blockers: &[Footprint],
    from: [f32; 2],
    id: &str,
) -> Result<(Route, Option<f32>), GroundError> {
    let (stand, facing) = tree
        .stand(id)
        .ok_or_else(|| GroundError::Unknown(id.to_owned()))?;
    let route = |waypoints| Route {
        destination: stand,
        waypoints,
    };
    let failed = match plan_to(from, stand, facing, blockers) {
        Ok(waypoints) => return Ok((route(waypoints), facing)),
        Err(failed) => failed,
    };
    let inner = doorways(tree, id);
    let outer = tree
        .room_at(from)
        .map(|room| doorways(tree, &room.id))
        .unwrap_or_default();
    let shared = outer
        .iter()
        .zip(&inner)
        .take_while(|(a, b)| a.0 == b.0)
        .count();
    if shared == outer.len() && shared == inner.len() {
        return Err(GroundError::Nav(failed));
    }
    // Each doorway crossed: out of `from`'s places, then into the target's.
    let crossings = outer[shared..]
        .iter()
        .rev()
        .map(|(_, [o, i])| [*i, *o])
        .chain(inner[shared..].iter().map(|(_, e)| *e));
    let mut waypoints = Vec::new();
    let mut at = from;
    for [a, b] in crossings {
        if !nav::segment_clear(a, b, blockers, HALF_EXTENT) {
            return Err(GroundError::Nav(NavError::NoRoute));
        }
        waypoints.extend(
            nav::plan(at, a, blockers, HALF_EXTENT)
                .map_err(GroundError::Nav)?
                .waypoints,
        );
        waypoints.push(b);
        at = b;
    }
    waypoints.extend(plan_to(at, stand, facing, blockers).map_err(GroundError::Nav)?);
    Ok((route(waypoints), facing))
}

/// How far behind a standing point [`plan_to`] looks for a point the grid
/// reaches, m.
// A rear booth's clear side aisle can join the coarse grid only at its
// front. Every longer approach still needs exact segment clearance.
const BEHIND: [f32; 7] = [0.6, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

/// The waypoints from `at` to `stand`. A point tucked behind furniture can
/// sit where navigation's grid doesn't reach; then the route ends with a
/// straight step from a point near it the grid reaches, behind it first.
fn plan_to(
    at: [f32; 2],
    stand: [f32; 2],
    facing: Option<f32>,
    blockers: &[Footprint],
) -> Result<Vec<[f32; 2]>, NavError> {
    let failed = match nav::plan(at, stand, blockers, HALF_EXTENT) {
        Ok(route) => return Ok(route.waypoints),
        Err(failed) => failed,
    };
    // Behind first, then turning a little more to each side each time.
    let back = facing.unwrap_or(0.0) + std::f32::consts::PI;
    for distance in BEHIND {
        for turn in [0.0, 0.5, -0.5, 1.0, -1.0, 1.6, -1.6] {
            let (sin, cos) = (back + turn).sin_cos();
            let near = [stand[0] + sin * distance, stand[1] + cos * distance];
            if !nav::segment_clear(near, stand, blockers, HALF_EXTENT) {
                continue;
            }
            if let Ok(route) = nav::plan(at, near, blockers, HALF_EXTENT) {
                let mut waypoints = route.waypoints;
                waypoints.push(stand);
                return Ok(waypoints);
            }
        }
    }
    Err(failed)
}
