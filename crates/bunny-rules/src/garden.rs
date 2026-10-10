//! A garden: a graph of straight three-lane corridors joined at right-angle
//! junctions, with edibles, obstacles and power-ups placed by lane along
//! each corridor, and the level validator.
//!
//! World axes: `x` east, `z` south. An edge runs from node `a` to node `b`;
//! along it, `s` is the distance from `a`. A lane is `-1`, `0` or `1`, in the
//! edge's frame: positive is to the right when facing from `a` to `b`.

use crate::kinds::{Contact, EdibleKind, ObstacleKind, PowerKind};
use crate::{CORRIDOR_HALF, HZ, LANE_WIDTH, TIER_QUARTERS, TIER_SPEED, UNIT};

/// A junction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub x: i32,
    pub z: i32,
}

/// A straight corridor between two junctions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub len: i32,
    /// The unit direction from `a` to `b`.
    pub dx: i32,
    pub dz: i32,
}

impl Edge {
    /// The end the bunny reaches running in direction `fwd`.
    #[must_use]
    pub fn end(&self, fwd: bool) -> usize {
        if fwd { self.b } else { self.a }
    }

    /// The distance along the edge of the end reached running `fwd`.
    #[must_use]
    pub fn end_s(&self, fwd: bool) -> i32 {
        if fwd { self.len } else { 0 }
    }
}

/// An edible in one lane at one point of a corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edible {
    pub edge: usize,
    pub s: i32,
    pub lane: i8,
    pub kind: EdibleKind,
    /// Hangs in the air: eaten only in a jump.
    pub air: bool,
}

/// An obstacle in one lane at one point of a corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Obstacle {
    pub edge: usize,
    pub s: i32,
    pub lane: i8,
    pub kind: ObstacleKind,
}

/// A power-up in one lane at one point of a corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Power {
    pub edge: usize,
    pub s: i32,
    pub lane: i8,
    pub kind: PowerKind,
}

/// A spot in a corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub edge: usize,
    pub s: i32,
    pub lane: i8,
}

/// One way out of a junction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    pub edge: usize,
    /// Whether leaving runs from the edge's `a` to its `b`.
    pub fwd: bool,
    pub dx: i32,
    pub dz: i32,
}

/// How the farmer plays a garden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarmerPlan {
    /// Walking and running speeds, in units per tick.
    pub walk: i32,
    pub run: i32,
    /// How long he winds up his net before it lands, in ticks.
    pub windup: u32,
    /// How long a chase lasts before a scatter, and the scatter, in ticks;
    /// a scatter of 0 means he never stops chasing.
    pub chase: u32,
    pub scatter: u32,
    /// Whether he heads for the junction ahead of the bunny instead of the
    /// bunny itself.
    pub ambush: bool,
    /// How long a golden carrot spooks him, in ticks.
    pub spook: u32,
}

/// A whole garden level.
#[derive(Clone, Debug)]
pub struct Garden {
    pub name: String,
    /// Its place in the level list (1 to 20) and its set (1 to 4).
    pub number: u8,
    pub set: u8,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub edibles: Vec<Edible>,
    pub obstacles: Vec<Obstacle>,
    pub powers: Vec<Power>,
    pub spawn_edge: usize,
    pub spawn_s: i32,
    pub spawn_fwd: bool,
    /// The farmer's shed, where he starts.
    pub shed: usize,
    /// The junctions he walks while tending the garden.
    pub patrol: Vec<usize>,
    pub farmer: FarmerPlan,
    /// Par time for the clear bonus, in seconds.
    pub par: u32,
    /// Where the bonus vegetable appears.
    pub bonus: Spot,
    /// SHA-256 of the level file the garden was read from.
    pub digest: [u8; 32],
    pub(crate) exits: Vec<Vec<Exit>>,
}

/// The least distance between a junction and anything placed in a corridor.
pub const END_MARGIN: i32 = 25 * UNIT / 10;
/// Things at the same point closer than this collide.
const OVERLAP: i32 = UNIT / 2;
/// An obstacle is in view at least this long before the bunny reaches it.
pub const SIGHT_TICKS: i32 = 72;

impl Garden {
    /// The ways out of a junction.
    #[must_use]
    pub fn exits(&self, node: usize) -> &[Exit] {
        &self.exits[node]
    }

    pub(crate) fn link(&mut self) {
        let mut exits = vec![Vec::new(); self.nodes.len()];
        for (index, e) in self.edges.iter().enumerate() {
            exits[e.a].push(Exit {
                edge: index,
                fwd: true,
                dx: e.dx,
                dz: e.dz,
            });
            exits[e.b].push(Exit {
                edge: index,
                fwd: false,
                dx: -e.dx,
                dz: -e.dz,
            });
        }
        self.exits = exits;
    }

    /// A point on an edge, `lateral` units right of its centre line in the
    /// edge's frame.
    #[must_use]
    pub fn point(&self, edge: usize, s: i32, lateral: i32) -> (i32, i32) {
        let e = &self.edges[edge];
        let a = self.nodes[e.a];
        (
            a.x + e.dx * s - e.dz * lateral,
            a.z + e.dz * s + e.dx * lateral,
        )
    }

    /// Whether a point is inside some corridor or junction.
    #[must_use]
    pub fn in_corridor(&self, x: i32, z: i32) -> bool {
        let half = CORRIDOR_HALF + UNIT / 10;
        self.edges.iter().any(|e| {
            let a = self.nodes[e.a];
            let b = self.nodes[e.b];
            x >= a.x.min(b.x) - half
                && x <= a.x.max(b.x) + half
                && z >= a.z.min(b.z) - half
                && z <= a.z.max(b.z) + half
        })
    }

    /// Whether the farmer may walk an edge.
    #[must_use]
    pub fn farmer_may_walk(&self, edge: usize) -> bool {
        !self
            .obstacles
            .iter()
            .any(|o| o.edge == edge && o.kind.bars_farmer())
    }

    /// The edibles a win needs: every one but the bonus vegetable.
    #[must_use]
    pub fn food(&self) -> usize {
        self.edibles.len()
    }

    /// Total growth in the garden's edibles, in quarters.
    #[must_use]
    pub fn total_quarters(&self) -> u32 {
        self.edibles.iter().map(|e| e.kind.quarters()).sum()
    }

    /// Whether lane `lane` at distance `s` of `edge` stops a bunny of
    /// `tier`: a standing obstacle it can't get past even with a jump or a
    /// duck, or food on the ground too big for it to eat. `alive` and
    /// `eaten` say which obstacles still stand and which edibles are gone.
    #[must_use]
    pub fn lane_blocked(
        &self,
        edge: usize,
        s: i32,
        lane: i8,
        tier: u8,
        alive: &[bool],
        eaten: &[bool],
    ) -> bool {
        self.obstacles.iter().enumerate().any(|(index, o)| {
            alive[index]
                && o.edge == edge
                && o.s == s
                && o.lane == lane
                && o.kind.contact(tier) == Contact::Block
        }) || self.edibles.iter().enumerate().any(|(index, e)| {
            !eaten[index]
                && !e.air
                && e.edge == edge
                && e.s == s
                && e.lane == lane
                && e.kind.min_tier() > tier
        })
    }

    /// Whether, at `tier`, some row strictly between `from` and `to` on
    /// `edge` stops all three lanes.
    #[must_use]
    pub fn barred(
        &self,
        edge: usize,
        from: i32,
        to: i32,
        tier: u8,
        alive: &[bool],
        eaten: &[bool],
    ) -> bool {
        let (lo, hi) = (from.min(to), from.max(to));
        let mut rows: Vec<i32> = self
            .obstacles
            .iter()
            .filter(|o| o.edge == edge && o.s > lo && o.s < hi)
            .map(|o| o.s)
            .chain(
                self.edibles
                    .iter()
                    .filter(|e| e.edge == edge && e.s > lo && e.s < hi && e.kind.min_tier() > 0)
                    .map(|e| e.s),
            )
            .collect();
        rows.sort_unstable();
        rows.dedup();
        rows.into_iter().any(|s| {
            [-1_i8, 0, 1]
                .iter()
                .all(|lane| self.lane_blocked(edge, s, *lane, tier, alive, eaten))
        })
    }

    fn standing(&self) -> (Vec<bool>, Vec<bool>) {
        (
            vec![true; self.obstacles.len()],
            vec![false; self.edibles.len()],
        )
    }

    /// The junctions reachable from the spawn point at `tier`, with every
    /// obstacle standing.
    #[must_use]
    pub fn reachable(&self, tier: u8) -> Vec<bool> {
        let (alive, eaten) = self.standing();
        let spawn = &self.edges[self.spawn_edge];
        let mut starts = Vec::new();
        if !self.barred(self.spawn_edge, self.spawn_s, 0, tier, &alive, &eaten) {
            starts.push(spawn.a);
        }
        if !self.barred(
            self.spawn_edge,
            self.spawn_s,
            spawn.len,
            tier,
            &alive,
            &eaten,
        ) {
            starts.push(spawn.b);
        }
        self.flood(&starts, tier, &alive, &eaten)
    }

    /// The junctions reachable from `starts` at `tier`.
    #[must_use]
    pub fn flood(&self, starts: &[usize], tier: u8, alive: &[bool], eaten: &[bool]) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut queue: Vec<usize> = starts.to_vec();
        while let Some(node) = queue.pop() {
            if std::mem::replace(&mut seen[node], true) {
                continue;
            }
            for exit in self.exits(node) {
                let e = &self.edges[exit.edge];
                if !self.barred(exit.edge, 0, e.len, tier, alive, eaten) {
                    queue.push(e.end(exit.fwd));
                }
            }
        }
        seen
    }

    /// Whether a spot `s` on `edge` can be reached at `tier`, given the
    /// junctions reachable at that tier.
    #[must_use]
    pub fn spot_reachable(&self, edge: usize, s: i32, tier: u8, nodes: &[bool]) -> bool {
        let (alive, eaten) = self.standing();
        let e = &self.edges[edge];
        (edge == self.spawn_edge && !self.barred(edge, self.spawn_s, s, tier, &alive, &eaten))
            || (nodes[e.a] && !self.barred(edge, 0, s, tier, &alive, &eaten))
            || (nodes[e.b] && !self.barred(edge, e.len, s, tier, &alive, &eaten))
    }

    /// Checks the level against the spec's rules:
    ///
    /// - corridors are straight and pass through no junction; everything
    ///   sits inside its corridor, away from the junctions, and no edible
    ///   sits in an obstacle;
    /// - no row of obstacles is a wall at every size;
    /// - every obstacle that a bunny must react to is in view for 1.2 s at
    ///   the fastest size that must react, from either end;
    /// - the edibles hold at least 1.2 times a Giant's growth;
    /// - eating what it can reach, a Kit grows to a Giant and clears the
    ///   garden;
    /// - no edible is ever stranded: each stays reachable at every size big
    ///   enough to eat it, and a bunny that grows somewhere it could get to
    ///   while smaller can always get back to the rest of the garden;
    /// - the farmer starts at least 25 m from the bunny.
    pub fn validate(&self) -> Result<(), String> {
        if self.edibles.is_empty() {
            return Err("no edibles".into());
        }
        self.validate_shape()?;
        self.validate_placement()?;
        self.validate_sight()?;
        let total = self.total_quarters();
        if total * 10 < TIER_QUARTERS[4] * 12 {
            return Err(format!(
                "the edibles give {} GP, under 1.2 times a Giant's {}",
                total / 4,
                TIER_QUARTERS[4] / 4
            ));
        }
        self.validate_growth()?;
        self.validate_stranding()?;
        let (sx, sz) = self.point(self.spawn_edge, self.spawn_s, 0);
        let shed = self.nodes[self.shed];
        let (dx, dz) = (i64::from(shed.x - sx), i64::from(shed.z - sz));
        if dx * dx + dz * dz < i64::from(25 * UNIT).pow(2) {
            return Err("the farmer starts too close".into());
        }
        if self.patrol.is_empty() || self.exits(self.shed).is_empty() {
            return Err("the farmer has nowhere to walk".into());
        }
        if !self.farmer_may_walk(self.exits(self.shed)[0].edge) {
            return Err("the farmer's shed opens onto a fenced corridor".into());
        }
        if self.farmer.windup < crate::ticks(350) {
            return Err("the net's wind-up is under 0.35 s".into());
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), String> {
        for (index, e) in self.edges.iter().enumerate() {
            let a = self.nodes[e.a];
            let b = self.nodes[e.b];
            if (b.x - a.x, b.z - a.z) != (e.dx * e.len, e.dz * e.len)
                || e.dx.abs() + e.dz.abs() != 1
                || e.len <= 2 * END_MARGIN
            {
                return Err(format!("corridor {index} is not straight"));
            }
            for (n, node) in self.nodes.iter().enumerate() {
                if n == e.a || n == e.b {
                    continue;
                }
                let inside = if e.dx != 0 {
                    node.z == a.z && node.x > a.x.min(b.x) && node.x < a.x.max(b.x)
                } else {
                    node.x == a.x && node.z > a.z.min(b.z) && node.z < a.z.max(b.z)
                };
                if inside {
                    return Err(format!("corridor {index} runs through junction {n}"));
                }
            }
        }
        if self
            .nodes
            .iter()
            .enumerate()
            .any(|(i, n)| self.nodes[..i].iter().any(|m| m.x == n.x && m.z == n.z))
        {
            return Err("two junctions in one place".into());
        }
        let (alive, eaten) = self.standing();
        let spawn = self.edges[self.spawn_edge];
        if self.spawn_s < END_MARGIN || self.spawn_s > spawn.len - END_MARGIN {
            return Err("the bunny starts in a junction".into());
        }
        let ahead = spawn.end_s(self.spawn_fwd);
        if self.barred(self.spawn_edge, self.spawn_s, ahead, 0, &alive, &eaten) {
            return Err("a Kit can't run out of the spawn corridor".into());
        }
        Ok(())
    }

    fn validate_placement(&self) -> Result<(), String> {
        let inside = |edge: usize, s: i32, lane: i8| {
            let len = self.edges[edge].len;
            s >= END_MARGIN && s <= len - END_MARGIN && lane.abs() <= 1
        };
        for e in &self.edibles {
            if !inside(e.edge, e.s, e.lane) {
                return Err(format!("{} off its corridor: {e:?}", e.kind.word()));
            }
        }
        for p in &self.powers {
            if !inside(p.edge, p.s, p.lane) {
                return Err(format!("{} off its corridor: {p:?}", p.kind.word()));
            }
        }
        if !inside(self.bonus.edge, self.bonus.s, self.bonus.lane) {
            return Err("the bonus spot is off its corridor".into());
        }
        let near = |edge: usize, s: i32, lane: i8, other: (usize, i32, i8)| {
            edge == other.0 && lane == other.2 && (s - other.1).abs() < OVERLAP
        };
        for o in &self.obstacles {
            if !inside(o.edge, o.s, o.lane) {
                return Err(format!("{} off its corridor: {o:?}", o.kind.word()));
            }
            let at = (o.edge, o.s, o.lane);
            if self
                .edibles
                .iter()
                .any(|e| !e.air && near(e.edge, e.s, e.lane, at))
                || self.powers.iter().any(|p| near(p.edge, p.s, p.lane, at))
                || near(self.bonus.edge, self.bonus.s, self.bonus.lane, at)
            {
                return Err(format!("something sits in a {}: {o:?}", o.kind.word()));
            }
        }
        // No row is a wall for every size.
        for o in &self.obstacles {
            let wall = (0..5_u8).all(|tier| {
                [-1_i8, 0, 1].iter().all(|lane| {
                    self.obstacles.iter().any(|p| {
                        p.edge == o.edge
                            && p.s == o.s
                            && p.lane == *lane
                            && p.kind.contact(tier) == Contact::Block
                    })
                })
            });
            if wall {
                return Err(format!("a row no size gets past: {o:?}"));
            }
        }
        Ok(())
    }

    fn validate_sight(&self) -> Result<(), String> {
        for o in &self.obstacles {
            let fastest = (0..5_u8)
                .filter(|tier| !matches!(o.kind.contact(*tier), Contact::Pass | Contact::Smash))
                .map(|tier| TIER_SPEED[usize::from(tier)])
                .max();
            let Some(speed) = fastest else {
                continue;
            };
            let need = speed * SIGHT_TICKS;
            let len = self.edges[o.edge].len;
            if o.s < need || len - o.s < need {
                return Err(format!(
                    "a {} comes into view less than {:.1} s before the bunny reaches it: {o:?}",
                    o.kind.word(),
                    SIGHT_TICKS as f32 / HZ as f32
                ));
            }
        }
        Ok(())
    }

    /// Growing by eating only what it can reach and eat, a Kit gets to be a
    /// Giant and eats everything.
    fn validate_growth(&self) -> Result<(), String> {
        let mut quarters = 0;
        let mut eaten = vec![false; self.edibles.len()];
        loop {
            let tier = crate::tier_for(quarters);
            let nodes = self.reachable(tier);
            let mut ate = false;
            for (index, e) in self.edibles.iter().enumerate() {
                if !eaten[index]
                    && e.kind.min_tier() <= tier
                    && self.spot_reachable(e.edge, e.s, tier, &nodes)
                {
                    eaten[index] = true;
                    quarters += e.kind.quarters();
                    ate = true;
                }
            }
            if !ate {
                break;
            }
        }
        if let Some(index) = eaten.iter().position(|e| !e) {
            return Err(format!(
                "a bunny that grows by eating can't clear the garden: {:?}",
                self.edibles[index]
            ));
        }
        Ok(())
    }

    fn validate_stranding(&self) -> Result<(), String> {
        let (alive, eaten) = self.standing();
        let mut been = vec![false; self.nodes.len()];
        for tier in 0..5_u8 {
            let nodes = self.reachable(tier);
            for (index, e) in self.edibles.iter().enumerate() {
                if e.kind.min_tier() <= tier && !self.spot_reachable(e.edge, e.s, tier, &nodes) {
                    return Err(format!(
                        "{} {index} is out of reach for a {}",
                        e.kind.word(),
                        crate::TIER_NAMES[usize::from(tier)]
                    ));
                }
            }
            // Wherever a smaller bunny could be, a bunny this size gets
            // back to the spawn's part of the garden.
            for (node, was) in been.iter().enumerate() {
                if *was && !nodes[node] {
                    let back = self.flood(&[node], tier, &alive, &eaten);
                    if !back.iter().zip(&nodes).any(|(b, n)| *b && *n) {
                        return Err(format!(
                            "a bunny that grows to a {} at junction {node} is shut in",
                            crate::TIER_NAMES[usize::from(tier)]
                        ));
                    }
                }
            }
            for (node, reach) in nodes.iter().enumerate() {
                been[node] |= *reach;
            }
        }
        Ok(())
    }
}

/// The lane offset of a lane index.
#[must_use]
pub fn lane_offset(lane: i8) -> i32 {
    i32::from(lane) * LANE_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;

    #[test]
    fn gardens_one_to_five_pass_the_validator() {
        for number in 1..=level::COUNT {
            let garden = level::garden(number);
            garden
                .validate()
                .unwrap_or_else(|e| panic!("garden {number}: {e}"));
            assert_eq!(garden.number as usize, number);
            assert!(garden.farmer.windup >= crate::ticks(350));
            assert!(
                garden.total_quarters() * 10 >= TIER_QUARTERS[4] * 12,
                "garden {number}"
            );
        }
    }

    #[test]
    fn a_small_bunny_cannot_cross_a_full_row_of_pots_but_a_jack_can() {
        let garden = level::garden(3);
        let (alive, eaten) = garden.standing();
        let row = garden
            .obstacles
            .iter()
            .find(|o| {
                o.kind == ObstacleKind::Pot
                    && garden
                        .obstacles
                        .iter()
                        .filter(|p| p.edge == o.edge && p.s == o.s)
                        .count()
                        == 3
            })
            .expect("garden 3 has a full row of pots");
        let len = garden.edges[row.edge].len;
        assert!(garden.barred(row.edge, 0, len, 0, &alive, &eaten));
        assert!(garden.barred(row.edge, 0, len, 1, &alive, &eaten));
        assert!(!garden.barred(row.edge, 0, len, 2, &alive, &eaten));
    }

    #[test]
    fn lettuce_is_solid_for_a_kit() {
        let garden = level::garden(3);
        let (alive, eaten) = garden.standing();
        let lettuce = garden
            .edibles
            .iter()
            .find(|e| e.kind == EdibleKind::Lettuce)
            .unwrap();
        assert!(garden.lane_blocked(lettuce.edge, lettuce.s, lettuce.lane, 0, &alive, &eaten));
        assert!(!garden.lane_blocked(lettuce.edge, lettuce.s, lettuce.lane, 2, &alive, &eaten));
    }

    fn fence_in(garden: &mut Garden, edge: usize, at: i32) {
        for s in [at - UNIT * 6 / 10, at + UNIT * 6 / 10] {
            for lane in [-1, 0, 1] {
                garden.obstacles.push(Obstacle {
                    edge,
                    s,
                    lane,
                    kind: ObstacleKind::Fence,
                });
            }
        }
    }

    #[test]
    fn the_validator_refuses_a_wall_row() {
        let mut garden = level::garden(1);
        let e = *garden
            .edibles
            .iter()
            .find(|e| e.edge != garden.spawn_edge && e.kind == EdibleKind::Carrot)
            .unwrap();
        garden.edibles.retain(|o| o.edge != e.edge || *o == e);
        fence_in(&mut garden, e.edge, e.s);
        let error = garden.validate().unwrap_err();
        assert!(error.contains("no size gets past"), "{error}");
    }

    #[test]
    fn the_validator_refuses_food_a_grown_bunny_cannot_reach() {
        // Gaps on both sides of a seedling: a Kit gets to it, a Jack can't.
        let mut garden = level::garden(1);
        let e = *garden
            .edibles
            .iter()
            .find(|e| e.edge != garden.spawn_edge)
            .unwrap();
        for s in [e.s - UNIT, e.s + UNIT] {
            for lane in [-1, 0, 1] {
                garden.obstacles.push(Obstacle {
                    edge: e.edge,
                    s,
                    lane,
                    kind: ObstacleKind::Gap,
                });
            }
        }
        garden
            .edibles
            .retain(|o| o.edge != e.edge || o == &e || (o.s - e.s).abs() > 2 * UNIT);
        let error = garden.validate_stranding().unwrap_err();
        assert!(error.contains("out of reach for a Jack"), "{error}");
        let _ = EdibleKind::Seedling;
    }

    #[test]
    fn the_validator_refuses_an_obstacle_seen_too_late() {
        let mut garden = level::garden(1);
        let edge = garden.spawn_edge;
        garden.obstacles.push(Obstacle {
            edge,
            s: 3 * UNIT,
            lane: 1,
            kind: ObstacleKind::Pot,
        });
        garden.edibles.retain(|e| !(e.edge == edge && e.lane == 1));
        let error = garden.validate().unwrap_err();
        assert!(error.contains("into view"), "{error}");
    }

    #[test]
    fn the_validator_refuses_too_little_food() {
        let mut garden = level::garden(1);
        garden.edibles.truncate(40);
        let error = garden.validate().unwrap_err();
        assert!(error.contains("1.2 times"), "{error}");
    }
}
