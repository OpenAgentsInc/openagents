//! Placement rules. A wall stands when its panels are contiguous, at least
//! one merges with stone, and no span between supports is longer than
//! [`SPAN_LIMIT`].
//!
//! Panels are sampled on their midplanes. Two panels share an edge when
//! samples along one's edge lie on the other (within half its thickness);
//! a panel merges with stone when its samples lie on a stone box. Spans are
//! horizontal: the distance from a deck sample to the nearest support is
//! the shortest path over the deck's samples, counting only horizontal
//! travel, so upright walls and towers have no span. A cantilever of length
//! L bends its root like a simple span of 2L, so the effective span is
//! twice the largest distance to a support: a 20-foot bridge and a 10-foot
//! cantilever are both at the limit.
//!
//! Supports: an upright panel standing on stone (directly or on another
//! such panel) supports the deck it touches. A deck that touches stone must
//! keep its span within the limit from stone alone, or, if it needs the
//! upright supports to do so, be made of half-size panels, as the SRD
//! requires. A deck that touches no stone (a tower's roof) rests on its
//! supports.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use super::{BUDGET, Form, Placement, RANGE, SPAN_LIMIT, box_distance};

/// How far apart two surfaces may be and still count as touching, m.
pub const TOLERANCE: f64 = 0.03;
/// Target spacing of midplane samples, m.
pub const SPACING: f64 = 0.25;
/// Shortest shared edge that joins two panels, m.
pub const MIN_SHARED: f64 = 0.4;

/// A static scene collider tagged `stone`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stone {
    pub center: DVec3,
    pub half: DVec3,
    pub orientation: DQuat,
}

impl Stone {
    /// An axis-aligned stone box from its minimum and maximum corners.
    #[must_use]
    pub fn aabb(min: DVec3, max: DVec3) -> Self {
        Self {
            center: (min + max) * 0.5,
            half: (max - min) * 0.5,
            orientation: DQuat::IDENTITY,
        }
    }

    #[must_use]
    pub fn distance(&self, p: DVec3) -> f64 {
        box_distance(self.center, self.orientation, self.half, p)
    }
}

/// Why a wall cannot stand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum Refusal {
    Empty,
    /// More stone than ten panels.
    OverBudget {
        cost: u32,
    },
    /// Panels of different forms; half-size panels replace every panel.
    MixedForms,
    OutOfRange {
        panel: usize,
    },
    /// The panel shares no edge with the rest of the wall.
    Disconnected {
        panel: usize,
    },
    /// No panel merges with stone, or a deck rests on nothing.
    Unsupported,
    /// A span longer than 20 feet, m.
    SpanTooLong {
        span: f64,
    },
    /// Supports bring the span within 20 feet, but only half-size panels
    /// may create supports; the span from stone alone, m.
    NeedsHalfPanels {
        span: f64,
    },
}

/// Two panels joined along a shared edge.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seam {
    pub a: usize,
    pub b: usize,
    /// Middle of the shared region, m.
    pub at: DVec3,
    /// Length of the shared region, m.
    pub length: f64,
}

/// A panel merged with a stone collider.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Footing {
    pub panel: usize,
    pub stone: usize,
    /// Middle of the contact, m.
    pub at: DVec3,
    /// Whether the panel's face rests on the stone, rather than only an
    /// edge.
    pub bearing: bool,
}

/// An admitted wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub panels: Vec<Placement>,
    pub seams: Vec<Seam>,
    pub footings: Vec<Footing>,
    /// The largest effective span between supports, m.
    pub span: f64,
    /// Whether upright half-size supports carry a span stone alone could
    /// not.
    pub uses_supports: bool,
}

struct Sample {
    panel: usize,
    point: DVec3,
    boundary: bool,
}

fn samples(index: usize, panel: &Placement) -> Vec<Sample> {
    let size = panel.form.size();
    let nx = (size.x / SPACING).ceil().max(1.0) as usize;
    let ny = (size.y / SPACING).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            let local = DVec3::new(
                -size.x * 0.5 + size.x * i as f64 / nx as f64,
                -size.y * 0.5 + size.y * j as f64 / ny as f64,
                0.0,
            );
            out.push(Sample {
                panel: index,
                point: panel.to_world(local),
                boundary: i == 0 || j == 0 || i == nx || j == ny,
            });
        }
    }
    out
}

fn panel_distance(panel: &Placement, p: DVec3) -> f64 {
    box_distance(panel.center, panel.orientation, panel.half(), p)
}

fn horizontal_distance(a: DVec3, b: DVec3) -> f64 {
    DVec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

/// Check a wall against the SRD placement rules.
///
/// # Errors
///
/// Returns the first rule the wall breaks.
pub fn validate(panels: &[Placement], stone: &[Stone], caster: DVec3) -> Result<Plan, Refusal> {
    survey(panels, stone, Some(caster))
}

/// The joints a wall would have, checking contiguity and stone contact but
/// not range or spans: for demonstrating what the span rule prevents.
///
/// # Errors
///
/// Returns a refusal when the panels are not contiguous or touch no stone.
pub fn force(panels: &[Placement], stone: &[Stone]) -> Result<Plan, Refusal> {
    survey(panels, stone, None)
}

fn survey(panels: &[Placement], stone: &[Stone], caster: Option<DVec3>) -> Result<Plan, Refusal> {
    let enforce = caster.is_some();
    let Some(first) = panels.first() else {
        return Err(Refusal::Empty);
    };
    if panels.iter().any(|p| p.form != first.form) {
        return Err(Refusal::MixedForms);
    }
    let cost: u32 = panels.iter().map(|p| p.form.cost()).sum();
    if cost > BUDGET {
        return Err(Refusal::OverBudget { cost });
    }
    if let Some(caster) = caster {
        if let Some(panel) = panels
            .iter()
            .position(|p| p.center.distance(caster) > RANGE)
        {
            return Err(Refusal::OutOfRange { panel });
        }
    }
    let mut points = Vec::new();
    let mut ranges = Vec::new();
    for (i, panel) in panels.iter().enumerate() {
        let start = points.len();
        points.extend(samples(i, panel));
        ranges.push(start..points.len());
    }
    let mut graph: Vec<Vec<(usize, f64)>> = vec![Vec::new(); points.len()];
    // Grid neighbours inside each panel.
    for (i, panel) in panels.iter().enumerate() {
        let size = panel.form.size();
        let nx = (size.x / SPACING).ceil().max(1.0) as usize;
        let start = ranges[i].start;
        for k in ranges[i].clone() {
            let local = k - start;
            let (col, _) = (local % (nx + 1), local / (nx + 1));
            let mut link = |m: usize| {
                let w = horizontal_distance(points[k].point, points[m].point);
                graph[k].push((m, w));
                graph[m].push((k, w));
            };
            if col < nx {
                link(k + 1);
            }
            if k + nx + 1 < ranges[i].end {
                link(k + nx + 1);
            }
        }
    }
    // Seams between panels.
    let mut seams = Vec::new();
    for a in 0..panels.len() {
        for b in (a + 1)..panels.len() {
            let (pa, pb) = (&panels[a], &panels[b]);
            if pa.center.distance(pb.center) > pa.half().length() + pb.half().length() + TOLERANCE {
                continue;
            }
            let mut matched = Vec::new();
            for (from, to, other) in [(a, b, pb), (b, a, pa)] {
                let reach = panels[from].half().z + TOLERANCE;
                for k in ranges[from].clone() {
                    if points[k].boundary && panel_distance(other, points[k].point) <= reach {
                        matched.push(k);
                        // Join to the nearest sample of the other panel.
                        let nearest = ranges[to]
                            .clone()
                            .min_by(|&x, &y| {
                                points[x]
                                    .point
                                    .distance_squared(points[k].point)
                                    .total_cmp(&points[y].point.distance_squared(points[k].point))
                            })
                            .expect("panels have samples");
                        let w = horizontal_distance(points[k].point, points[nearest].point);
                        graph[k].push((nearest, w));
                        graph[nearest].push((k, w));
                    }
                }
            }
            // The shared region's extent on whichever side matched more.
            let extent = |panel: usize| {
                let side: Vec<DVec3> = matched
                    .iter()
                    .filter(|&&k| points[k].panel == panel)
                    .map(|&k| points[k].point)
                    .collect();
                side.iter()
                    .flat_map(|p| side.iter().map(move |q| p.distance(*q)))
                    .fold(0.0, f64::max)
            };
            let length = extent(a).max(extent(b));
            if length >= MIN_SHARED {
                let at =
                    matched.iter().map(|&k| points[k].point).sum::<DVec3>() / matched.len() as f64;
                seams.push(Seam { a, b, at, length });
            }
        }
    }
    // Contiguity.
    let mut reached = vec![false; panels.len()];
    let mut stack = vec![0];
    reached[0] = true;
    while let Some(i) = stack.pop() {
        for seam in &seams {
            let other = if seam.a == i {
                seam.b
            } else if seam.b == i {
                seam.a
            } else {
                continue;
            };
            if !reached[other] {
                reached[other] = true;
                stack.push(other);
            }
        }
    }
    if let Some(panel) = reached.iter().position(|r| !r) {
        return Err(Refusal::Disconnected { panel });
    }
    // Stone contacts.
    let mut on_stone = vec![false; points.len()];
    let mut footings = Vec::new();
    for (i, panel) in panels.iter().enumerate() {
        let reach = panel.half().z + TOLERANCE;
        for (s, stone) in stone.iter().enumerate() {
            let touching: Vec<usize> = ranges[i]
                .clone()
                .filter(|&k| stone.distance(points[k].point) <= reach)
                .collect();
            if touching.is_empty() {
                continue;
            }
            for &k in &touching {
                on_stone[k] = true;
            }
            footings.push(Footing {
                panel: i,
                stone: s,
                at: touching.iter().map(|&k| points[k].point).sum::<DVec3>()
                    / touching.len() as f64,
                bearing: touching.iter().any(|&k| !points[k].boundary),
            });
        }
    }
    if footings.is_empty() {
        return Err(Refusal::Unsupported);
    }
    // Upright panels standing on stone, directly or through other upright
    // panels.
    let mut grounded: Vec<bool> = (0..panels.len())
        .map(|i| panels[i].vertical() && footings.iter().any(|f| f.panel == i))
        .collect();
    loop {
        let mut changed = false;
        for seam in &seams {
            for (x, y) in [(seam.a, seam.b), (seam.b, seam.a)] {
                if grounded[x] && !grounded[y] && panels[y].vertical() {
                    grounded[y] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Decks: connected panels that are not upright.
    let mut deck = vec![usize::MAX; panels.len()];
    let mut decks = 0;
    for start in 0..panels.len() {
        if panels[start].vertical() || deck[start] != usize::MAX {
            continue;
        }
        deck[start] = decks;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            for seam in &seams {
                for (x, y) in [(seam.a, seam.b), (seam.b, seam.a)] {
                    if x == i && !panels[y].vertical() && deck[y] == usize::MAX {
                        deck[y] = decks;
                        stack.push(y);
                    }
                }
            }
        }
        decks += 1;
    }
    let mut span: f64 = 0.0;
    let mut uses_supports = false;
    for d in 0..decks {
        let member = |k: usize| deck[points[k].panel] == d;
        let stone_sources: Vec<usize> = (0..points.len())
            .filter(|&k| member(k) && on_stone[k])
            .collect();
        let support_sources: Vec<usize> = (0..points.len())
            .filter(|&k| {
                member(k)
                    && panels.iter().enumerate().any(|(i, panel)| {
                        grounded[i]
                            && panel_distance(panel, points[k].point) <= panel.half().z + TOLERANCE
                    })
            })
            .collect();
        let all: Vec<usize> = stone_sources
            .iter()
            .chain(&support_sources)
            .copied()
            .collect();
        if all.is_empty() {
            if enforce {
                return Err(Refusal::Unsupported);
            }
            continue;
        }
        let with_supports = 2.0 * farthest(&graph, &all, &member);
        if stone_sources.is_empty() {
            // A roof on upright panels.
            if enforce && with_supports > SPAN_LIMIT + 1e-6 {
                return Err(Refusal::SpanTooLong {
                    span: with_supports,
                });
            }
            span = span.max(with_supports);
            continue;
        }
        let from_stone = 2.0 * farthest(&graph, &stone_sources, &member);
        if !enforce {
            span = span.max(from_stone.min(with_supports));
        } else if from_stone <= SPAN_LIMIT + 1e-6 {
            span = span.max(from_stone);
        } else if with_supports <= SPAN_LIMIT + 1e-6 {
            if first.form != Form::Half {
                return Err(Refusal::NeedsHalfPanels { span: from_stone });
            }
            uses_supports = true;
            span = span.max(with_supports);
        } else {
            return Err(Refusal::SpanTooLong {
                span: with_supports,
            });
        }
    }
    Ok(Plan {
        panels: panels.to_vec(),
        seams,
        footings,
        span,
        uses_supports,
    })
}

/// Largest shortest-path distance from `sources` to any member sample.
fn farthest(
    graph: &[Vec<(usize, f64)>],
    sources: &[usize],
    member: &impl Fn(usize) -> bool,
) -> f64 {
    let mut distance = vec![f64::INFINITY; graph.len()];
    // Non-negative floats order like their bit patterns.
    let mut heap = BinaryHeap::new();
    for &s in sources {
        distance[s] = 0.0;
        heap.push(Reverse((0u64, s)));
    }
    while let Some(Reverse((bits, k))) = heap.pop() {
        let d = f64::from_bits(bits);
        if d > distance[k] {
            continue;
        }
        for &(m, w) in &graph[k] {
            if !member(m) {
                continue;
            }
            let next = d + w;
            if next < distance[m] {
                distance[m] = next;
                heap.push(Reverse((next.to_bits(), m)));
            }
        }
    }
    (0..graph.len())
        .filter(|&k| member(k))
        .map(|k| distance[k])
        .fold(0.0, f64::max)
}
