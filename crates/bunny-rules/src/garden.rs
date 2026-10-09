//! A garden: a graph of straight three-lane corridors joined at right-angle
//! junctions, with carrots and obstacles placed by lane along each corridor,
//! and the level validator.
//!
//! World axes: `x` east, `z` south. An edge runs from node `a` to node `b`;
//! along it, `s` is the distance from `a`. A lane is `-1`, `0` or `1`, in the
//! edge's frame: positive is to the right when facing from `a` to `b`.

use crate::{CORRIDOR_HALF, LANE_WIDTH, TIER_QUARTERS, UNIT, per_tick, ticks};

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

/// An obstacle's kind. Each blocks some sizes and gives way to others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// A fence panel: blocks every size.
    Fence,
    /// A gap under a fence: only a Kit or a Bunny fits.
    Gap,
    /// A flower pot: a Jack or bigger smashes it.
    Pot,
    /// A garden gnome: a Big Bun or bigger smashes it.
    Gnome,
    /// A wheelbarrow: only a Giant smashes it.
    Barrow,
}

/// What happens when the bunny runs into an obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    Pass,
    Smash,
    Block,
}

impl CellKind {
    /// What a bunny of `tier` (0-based) does to this obstacle.
    #[must_use]
    pub fn contact(self, tier: u8) -> Contact {
        match self {
            Self::Fence => Contact::Block,
            Self::Gap if tier <= 1 => Contact::Pass,
            Self::Gap => Contact::Block,
            Self::Pot if tier >= 2 => Contact::Smash,
            Self::Gnome if tier >= 3 => Contact::Smash,
            Self::Barrow if tier >= 4 => Contact::Smash,
            Self::Pot | Self::Gnome | Self::Barrow => Contact::Block,
        }
    }

    /// Whether the farmer can't walk a corridor with this obstacle in it:
    /// he never climbs fences or crawls through the bunny's gaps.
    #[must_use]
    pub fn bars_farmer(self) -> bool {
        matches!(self, Self::Fence | Self::Gap)
    }
}

/// An obstacle in one lane at one point of a corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub edge: usize,
    pub s: i32,
    pub lane: i8,
    pub kind: CellKind,
}

/// A carrot in one lane at one point of a corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carrot {
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

/// A whole garden level.
#[derive(Clone, Debug)]
pub struct Garden {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub carrots: Vec<Carrot>,
    pub cells: Vec<Cell>,
    pub spawn_edge: usize,
    pub spawn_s: i32,
    pub spawn_fwd: bool,
    /// The farmer's shed, where he starts.
    pub shed: usize,
    /// The junctions he walks while tending the garden.
    pub patrol: Vec<usize>,
    /// The farmer's walking and running speeds, in units per tick.
    pub farmer_walk: i32,
    pub farmer_run: i32,
    /// How long the farmer winds up his net before it lands.
    pub windup: u32,
    exits: Vec<Vec<Exit>>,
}

impl Garden {
    /// The ways out of a junction.
    #[must_use]
    pub fn exits(&self, node: usize) -> &[Exit] {
        &self.exits[node]
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
            .cells
            .iter()
            .any(|cell| cell.edge == edge && cell.kind.bars_farmer())
    }

    /// Whether, at `tier`, some barrier of obstacles strictly between `from`
    /// and `to` on `edge` blocks all three lanes. `alive` says which cells
    /// are still standing.
    #[must_use]
    pub fn barred(&self, edge: usize, from: i32, to: i32, tier: u8, alive: &[bool]) -> bool {
        let (lo, hi) = (from.min(to), from.max(to));
        let mut rows: Vec<i32> = self
            .cells
            .iter()
            .filter(|cell| cell.edge == edge && cell.s > lo && cell.s < hi)
            .map(|cell| cell.s)
            .collect();
        rows.sort_unstable();
        rows.dedup();
        rows.into_iter().any(|s| {
            [-1_i8, 0, 1].iter().all(|lane| {
                self.cells.iter().enumerate().any(|(index, cell)| {
                    alive[index]
                        && cell.edge == edge
                        && cell.s == s
                        && cell.lane == *lane
                        && cell.kind.contact(tier) == crate::garden::Contact::Block
                })
            })
        })
    }

    /// The junctions reachable from the spawn point at `tier`, with every
    /// obstacle standing.
    #[must_use]
    pub fn reachable(&self, tier: u8) -> Vec<bool> {
        let alive = vec![true; self.cells.len()];
        let mut seen = vec![false; self.nodes.len()];
        let mut queue = Vec::new();
        let spawn = &self.edges[self.spawn_edge];
        if !self.barred(self.spawn_edge, self.spawn_s, 0, tier, &alive) {
            queue.push(spawn.a);
        }
        if !self.barred(self.spawn_edge, self.spawn_s, spawn.len, tier, &alive) {
            queue.push(spawn.b);
        }
        while let Some(node) = queue.pop() {
            if std::mem::replace(&mut seen[node], true) {
                continue;
            }
            for exit in self.exits(node) {
                let e = &self.edges[exit.edge];
                if !self.barred(exit.edge, 0, e.len, tier, &alive) {
                    queue.push(e.end(exit.fwd));
                }
            }
        }
        seen
    }

    /// Whether a carrot can be reached at `tier`.
    #[must_use]
    pub fn carrot_reachable(&self, carrot: &Carrot, tier: u8, nodes: &[bool]) -> bool {
        let alive = vec![true; self.cells.len()];
        let e = &self.edges[carrot.edge];
        (carrot.edge == self.spawn_edge
            && !self.barred(carrot.edge, self.spawn_s, carrot.s, tier, &alive))
            || (nodes[e.a] && !self.barred(carrot.edge, 0, carrot.s, tier, &alive))
            || (nodes[e.b] && !self.barred(carrot.edge, e.len, carrot.s, tier, &alive))
    }

    /// Checks the level: every carrot is reachable by a Giant, so growing
    /// never strands food; at every size the reachable carrots are enough
    /// to grow to the next; things sit inside their corridors, away from
    /// the junctions; and the farmer starts far from the bunny.
    pub fn validate(&self) -> Result<(), String> {
        if self.carrots.is_empty() {
            return Err("no carrots".into());
        }
        for (index, e) in self.edges.iter().enumerate() {
            let a = self.nodes[e.a];
            let b = self.nodes[e.b];
            if (b.x - a.x, b.z - a.z) != (e.dx * e.len, e.dz * e.len)
                || e.dx.abs() + e.dz.abs() != 1
            {
                return Err(format!("edge {index} is not straight"));
            }
        }
        let margin = 25 * UNIT / 10;
        for carrot in &self.carrots {
            let len = self.edges[carrot.edge].len;
            if carrot.s < margin || carrot.s > len - margin || carrot.lane.abs() > 1 {
                return Err(format!("carrot off its corridor: {carrot:?}"));
            }
        }
        for cell in &self.cells {
            let len = self.edges[cell.edge].len;
            if cell.s < margin || cell.s > len - margin || cell.lane.abs() > 1 {
                return Err(format!("obstacle off its corridor: {cell:?}"));
            }
            if self.carrots.iter().any(|c| {
                c.edge == cell.edge && c.lane == cell.lane && (c.s - cell.s).abs() < UNIT / 2
            }) {
                return Err(format!("a carrot sits in an obstacle: {cell:?}"));
            }
        }
        for node in 0..self.nodes.len() {
            if self.exits(node).len() < 2 {
                return Err(format!("junction {node} is a dead end"));
            }
        }
        let giant = self.reachable(4);
        for carrot in &self.carrots {
            if !self.carrot_reachable(carrot, 4, &giant) {
                return Err(format!("a Giant can't reach {carrot:?}"));
            }
        }
        for tier in 0..4_u8 {
            let nodes = self.reachable(tier);
            let food = self
                .carrots
                .iter()
                .filter(|c| self.carrot_reachable(c, tier, &nodes))
                .count() as u32
                * crate::game::CARROT_QUARTERS;
            if food < TIER_QUARTERS[usize::from(tier) + 1] {
                return Err(format!("tier {tier} can't find enough food to grow"));
            }
        }
        let (sx, sz) = self.point(self.spawn_edge, self.spawn_s, 0);
        let shed = self.nodes[self.shed];
        let (dx, dz) = (i64::from(shed.x - sx), i64::from(shed.z - sz));
        if dx * dx + dz * dz < i64::from(25 * UNIT).pow(2) {
            return Err("the farmer starts too close".into());
        }
        Ok(())
    }

    /// The first garden: a 5 by 4 grid of junctions, 14 m apart, with
    /// fence gaps only small bunnies fit through, and pots, gnomes and a
    /// wheelbarrow that only bigger ones smash.
    #[must_use]
    pub fn first() -> Self {
        use CellKind::{Barrow, Fence, Gap, Gnome, Pot};
        let mut g = Builder::new(14);
        let n: Vec<Vec<usize>> = (0..5)
            .map(|c| (0..4).map(|r| g.node(c, r)).collect())
            .collect();
        // Top row.
        let e = g.edge(n[0][0], n[1][0]);
        g.carrots(e, 0, &[5.0, 7.0, 9.0, 11.0]);
        let spawn = e;
        let e = g.edge(n[1][0], n[2][0]);
        g.cells(e, 6.0, &[(0, Pot)]);
        g.cells(e, 9.5, &[(1, Pot)]);
        g.carrots(e, -1, &[4.0, 6.0, 8.0, 10.0]);
        let e = g.edge(n[2][0], n[3][0]);
        g.cells(e, 7.0, &[(-1, Pot), (0, Pot), (1, Pot)]);
        g.carrots(e, 0, &[3.5, 5.0, 9.0, 10.5]);
        let e = g.edge(n[3][0], n[4][0]);
        g.carrots(e, 1, &[4.0, 6.0, 8.0, 10.0]);
        // Bottom row.
        let e = g.edge(n[0][3], n[1][3]);
        g.carrots(e, 0, &[4.0, 6.0, 8.0, 10.0]);
        let e = g.edge(n[1][3], n[2][3]);
        g.cells(e, 7.0, &[(0, Barrow)]);
        g.carrots(e, -1, &[4.0, 7.0, 10.0]);
        g.carrots(e, 1, &[7.0]);
        let e = g.edge(n[2][3], n[3][3]);
        g.carrots(e, 1, &[4.0, 6.0, 8.0, 10.0]);
        let e = g.edge(n[3][3], n[4][3]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        // Left side.
        let e = g.edge(n[0][0], n[0][1]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[0][1], n[0][2]);
        g.cells(e, 7.0, &[(-1, Fence), (0, Fence), (1, Gap)]);
        g.carrots(e, 1, &[3.5, 5.0, 9.0, 10.5]);
        let e = g.edge(n[0][2], n[0][3]);
        g.cells(e, 6.0, &[(1, Pot)]);
        g.carrots(e, -1, &[4.0, 6.0, 8.0]);
        g.carrots(e, 1, &[10.0]);
        // Right side.
        let e = g.edge(n[4][0], n[4][1]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[4][1], n[4][2]);
        g.cells(e, 6.0, &[(-1, Gnome), (0, Gnome)]);
        g.carrots(e, 1, &[4.0, 6.0, 8.0, 10.0]);
        let e = g.edge(n[4][2], n[4][3]);
        g.carrots(e, -1, &[4.0, 7.0, 10.0]);
        // Second row.
        let e = g.edge(n[0][1], n[1][1]);
        g.carrots(e, 1, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[1][1], n[2][1]);
        g.cells(e, 7.0, &[(-1, Pot), (1, Pot)]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[2][1], n[3][1]);
        g.carrots(e, -1, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[3][1], n[4][1]);
        g.cells(e, 7.0, &[(0, Pot)]);
        g.carrots(e, 1, &[4.0, 7.0, 10.0]);
        // Third row.
        let e = g.edge(n[0][2], n[1][2]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[1][2], n[2][2]);
        g.carrots(e, 1, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[2][2], n[3][2]);
        g.cells(e, 7.0, &[(-1, Gnome), (0, Gnome), (1, Gnome)]);
        g.carrots(e, 0, &[4.0, 10.0]);
        g.carrots(e, -1, &[4.0, 10.0]);
        let e = g.edge(n[3][2], n[4][2]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        // Inner columns.
        let e = g.edge(n[1][0], n[1][1]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[3][0], n[3][1]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[2][1], n[2][2]);
        g.cells(e, 7.0, &[(-1, Fence), (0, Gap), (1, Fence)]);
        g.carrots(e, 0, &[3.5, 10.5]);
        g.carrots(e, -1, &[4.5]);
        let e = g.edge(n[1][2], n[1][3]);
        g.carrots(e, 1, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[3][2], n[3][3]);
        g.carrots(e, -1, &[4.0, 7.0, 10.0]);
        let e = g.edge(n[2][2], n[2][3]);
        g.carrots(e, 0, &[4.0, 7.0, 10.0]);
        let patrol = vec![n[4][2], n[3][2], n[2][2], n[2][3], n[3][3], n[4][3]];
        g.finish(spawn, 2.0, true, n[4][3], patrol)
    }
}

/// Builds a garden on a square grid.
struct Builder {
    spacing: i32,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    carrots: Vec<Carrot>,
    cells: Vec<Cell>,
}

fn metres(value: f32) -> i32 {
    (value * UNIT as f32).round() as i32
}

impl Builder {
    fn new(spacing_metres: i32) -> Self {
        Self {
            spacing: spacing_metres * UNIT,
            nodes: Vec::new(),
            edges: Vec::new(),
            carrots: Vec::new(),
            cells: Vec::new(),
        }
    }

    fn node(&mut self, column: i32, row: i32) -> usize {
        self.nodes.push(Node {
            x: column * self.spacing,
            z: row * self.spacing,
        });
        self.nodes.len() - 1
    }

    fn edge(&mut self, a: usize, b: usize) -> usize {
        let (pa, pb) = (self.nodes[a], self.nodes[b]);
        let (dx, dz) = (pb.x - pa.x, pb.z - pa.z);
        let len = dx.abs() + dz.abs();
        self.edges.push(Edge {
            a,
            b,
            len,
            dx: dx.signum(),
            dz: dz.signum(),
        });
        self.edges.len() - 1
    }

    fn carrots(&mut self, edge: usize, lane: i8, at: &[f32]) {
        for s in at {
            self.carrots.push(Carrot {
                edge,
                s: metres(*s),
                lane,
            });
        }
    }

    fn cells(&mut self, edge: usize, at: f32, lanes: &[(i8, CellKind)]) {
        for (lane, kind) in lanes {
            self.cells.push(Cell {
                edge,
                s: metres(at),
                lane: *lane,
                kind: *kind,
            });
        }
    }

    fn finish(
        self,
        spawn_edge: usize,
        spawn_at: f32,
        spawn_fwd: bool,
        shed: usize,
        patrol: Vec<usize>,
    ) -> Garden {
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
        Garden {
            nodes: self.nodes,
            edges: self.edges,
            carrots: self.carrots,
            cells: self.cells,
            spawn_edge,
            spawn_s: metres(spawn_at),
            spawn_fwd,
            shed,
            patrol,
            farmer_walk: per_tick(3_000),
            farmer_run: per_tick(5_000),
            windup: ticks(500),
            exits,
        }
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

    #[test]
    fn the_first_garden_passes_the_validator() {
        let garden = Garden::first();
        garden.validate().unwrap();
        assert!(garden.carrots.len() >= 60, "{}", garden.carrots.len());
        assert!(garden.windup >= crate::ticks(350));
    }

    #[test]
    fn gaps_let_only_small_bunnies_through_and_smashing_needs_size() {
        assert_eq!(CellKind::Gap.contact(0), Contact::Pass);
        assert_eq!(CellKind::Gap.contact(1), Contact::Pass);
        assert_eq!(CellKind::Gap.contact(2), Contact::Block);
        assert_eq!(CellKind::Pot.contact(1), Contact::Block);
        assert_eq!(CellKind::Pot.contact(2), Contact::Smash);
        assert_eq!(CellKind::Gnome.contact(2), Contact::Block);
        assert_eq!(CellKind::Gnome.contact(3), Contact::Smash);
        assert_eq!(CellKind::Barrow.contact(3), Contact::Block);
        assert_eq!(CellKind::Barrow.contact(4), Contact::Smash);
        assert_eq!(CellKind::Fence.contact(4), Contact::Block);
    }

    #[test]
    fn a_small_bunny_cannot_cross_a_full_row_of_pots_but_a_jack_can() {
        let garden = Garden::first();
        let alive = vec![true; garden.cells.len()];
        let row = garden
            .cells
            .iter()
            .find(|c| {
                c.kind == CellKind::Pot
                    && garden
                        .cells
                        .iter()
                        .filter(|o| o.edge == c.edge && o.s == c.s)
                        .count()
                        == 3
            })
            .unwrap();
        let len = garden.edges[row.edge].len;
        assert!(garden.barred(row.edge, 0, len, 0, &alive));
        assert!(!garden.barred(row.edge, 0, len, 2, &alive));
    }

    #[test]
    fn a_validator_refuses_a_stranded_carrot() {
        let mut garden = Garden::first();
        // Fence a carrot in from both sides.
        let edge = garden.carrots[0].edge;
        let at = garden.carrots[0].s;
        for s in [at - UNIT * 6 / 10, at + UNIT * 6 / 10] {
            for lane in [-1, 0, 1] {
                garden.cells.push(Cell {
                    edge,
                    s,
                    lane,
                    kind: CellKind::Fence,
                });
            }
        }
        let error = garden.validate().unwrap_err();
        assert!(error.contains("Giant"), "{error}");
    }
}
