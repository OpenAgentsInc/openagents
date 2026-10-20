//! Where each unit of a graph goes, in graph units (logical pixels at zoom 1).
//!
//! The layout is deterministic, so a graph always opens the same way and a
//! unit's place says something: data flows left to right. Top-level units
//! are put in columns by the longest path of links that reach them, links
//! that would point back are drawn as loops instead of reordering the
//! columns, and units with no inputs move as far right as their outputs
//! allow. A graph unit stacks its children inside it, in two columns when it
//! has more than four. Unit lays out with a force simulation instead; a
//! fixed layout keeps each unit where a reader last saw it.

use std::collections::BTreeMap;

use crate::GraphSpec;

/// A leaf unit's width.
pub const UNIT_W: f32 = 212.0;
/// A unit inside a graph unit.
pub const CHILD_W: f32 = 184.0;
/// The title and line over a unit's pins.
pub const HEADER: f32 = 50.0;
/// One row of pins.
pub const PIN_ROW: f32 = 22.0;
/// Space inside a graph unit around its children.
pub const PAD: f32 = 14.0;
/// Space between children.
pub const CHILD_GAP: f32 = 10.0;
/// Space between columns, where the links run.
pub const GAP_X: f32 = 120.0;
/// Space between units in one column.
pub const GAP_Y: f32 = 36.0;

/// A unit's box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }

    fn union(self, other: Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect {
            x,
            y,
            w: (self.x + self.w).max(other.x + other.w) - x,
            h: (self.y + self.h).max(other.y + other.h) - y,
        }
    }
}

/// A placed graph: one box per unit and one flag per link, in spec order.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub boxes: Vec<Rect>,
    /// How deep each unit sits: 0 at the top.
    pub depth: Vec<usize>,
    /// Whether a link runs right to left, against the flow.
    pub back: Vec<bool>,
}

/// Which side of a unit a pin is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Input,
    Output,
}

impl Layout {
    /// The box around every unit.
    pub fn bounds(&self) -> Rect {
        self.boxes
            .iter()
            .copied()
            .reduce(Rect::union)
            .unwrap_or_default()
    }

    /// Where a pin sits: inputs down the left edge, outputs down the right.
    pub fn pin(&self, spec: &GraphSpec, unit: usize, pin: &str) -> Option<(f32, f32, Side)> {
        let u = spec.units.get(unit)?;
        let b = self.boxes.get(unit)?;
        let row = |i: usize| b.y + HEADER + PIN_ROW * (i as f32 + 0.5);
        if let Some(i) = u.inputs.iter().position(|p| p.id == pin) {
            return Some((b.x, row(i), Side::Input));
        }
        let i = u.outputs.iter().position(|p| p.id == pin)?;
        Some((b.x + b.w, row(i), Side::Output))
    }

    /// Moves `unit` and every unit inside it.
    pub fn translate(&mut self, spec: &GraphSpec, unit: usize, dx: f32, dy: f32) {
        let Some(id) = spec.units.get(unit).map(|u| u.id.as_str()) else {
            return;
        };
        for (at, u) in spec.units.iter().enumerate() {
            if at == unit || within(spec, &u.id, id) {
                self.boxes[at].x += dx;
                self.boxes[at].y += dy;
            }
        }
    }

    /// The deepest unit under a point, if any.
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        self.boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.contains(x, y))
            .max_by_key(|(at, _)| self.depth[*at])
            .map(|(at, _)| at)
    }
}

/// Whether `unit` sits somewhere inside `ancestor`.
fn within(spec: &GraphSpec, unit: &str, ancestor: &str) -> bool {
    let mut at = unit;
    for _ in 0..spec.units.len() {
        match spec.unit(at).and_then(|u| u.parent.as_deref()) {
            Some(parent) if parent == ancestor => return true,
            Some(parent) => at = parent,
            None => return false,
        }
    }
    false
}

fn pin_rows(spec: &GraphSpec, unit: usize) -> usize {
    let u = &spec.units[unit];
    u.inputs.len().max(u.outputs.len())
}

/// A unit's size, its children placed relative to its corner.
fn measure(
    spec: &GraphSpec,
    unit: usize,
    width: f32,
    offsets: &mut BTreeMap<usize, (f32, f32)>,
    sizes: &mut BTreeMap<usize, (f32, f32)>,
) -> (f32, f32) {
    let id = spec.units[unit].id.as_str();
    let children: Vec<usize> = spec
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.parent.as_deref() == Some(id))
        .map(|(at, _)| at)
        .collect();
    let rows = pin_rows(spec, unit) as f32;
    let top = HEADER + rows * PIN_ROW + if rows > 0.0 { 6.0 } else { 0.0 };
    if children.is_empty() {
        let size = (width, top + if rows > 0.0 { 4.0 } else { 10.0 });
        sizes.insert(unit, size);
        return size;
    }
    let columns = if children.len() > 4 { 2 } else { 1 };
    let measured: Vec<(usize, (f32, f32))> = children
        .iter()
        .map(|&c| (c, measure(spec, c, CHILD_W, offsets, sizes)))
        .collect();
    let column_w = measured.iter().map(|(_, s)| s.0).fold(0.0, f32::max);
    let mut heights = vec![0.0f32; columns];
    let per_column = children.len().div_ceil(columns);
    for (at, (child, (_, h))) in measured.iter().enumerate() {
        let column = at / per_column;
        let x = PAD + column as f32 * (column_w + CHILD_GAP);
        let y = top + heights[column];
        offsets.insert(*child, (x, y));
        heights[column] += h + CHILD_GAP;
    }
    let inner_w = columns as f32 * column_w + (columns as f32 - 1.0) * CHILD_GAP;
    let inner_h = heights.iter().copied().fold(0.0, f32::max) - CHILD_GAP;
    let size = ((inner_w + 2.0 * PAD).max(width), top + inner_h + PAD);
    sizes.insert(unit, size);
    size
}

/// Places every unit of `spec`. The spec should be valid.
pub fn layout(spec: &GraphSpec) -> Layout {
    let n = spec.units.len();
    let index: BTreeMap<&str, usize> = spec
        .units
        .iter()
        .enumerate()
        .map(|(at, u)| (u.id.as_str(), at))
        .collect();
    let roots: Vec<usize> = (0..n)
        .filter(|&at| spec.units[at].parent.is_none())
        .collect();
    let root = |id: &str| index.get(spec.root_of(id)).copied();

    // Edges between top-level units, in link order, once each; feedback
    // links are loops from the start.
    let mut edges: Vec<(usize, usize)> = vec![];
    let mut back_edges = vec![];
    for link in &spec.links {
        if link.feedback {
            if let (Some(a), Some(b)) = (root(&link.from.unit), root(&link.to.unit)) {
                back_edges.push((a, b));
            }
            continue;
        }
        if let (Some(a), Some(b)) = (root(&link.from.unit), root(&link.to.unit))
            && a != b
            && !edges.contains(&(a, b))
        {
            edges.push((a, b));
        }
    }

    // Back edges: the ones a depth-first walk in spec order meets while
    // their target is still open.
    let mut state = vec![0u8; n];
    fn walk(at: usize, edges: &[(usize, usize)], state: &mut [u8], back: &mut Vec<(usize, usize)>) {
        state[at] = 1;
        for &(a, b) in edges.iter().filter(|(a, _)| *a == at) {
            match state[b] {
                0 => walk(b, edges, state, back),
                1 => back.push((a, b)),
                _ => {}
            }
        }
        state[at] = 2;
    }
    for &r in &roots {
        if state[r] == 0 {
            walk(r, &edges, &mut state, &mut back_edges);
        }
    }
    let forward: Vec<(usize, usize)> = edges
        .iter()
        .copied()
        .filter(|e| !back_edges.contains(e))
        .collect();

    // Longest path from the units nothing feeds.
    let mut layer = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for &(a, b) in &forward {
            if layer[b] < layer[a] + 1 {
                layer[b] = layer[a] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Units nothing feeds sit just left of the first unit they feed.
    for &r in &roots {
        if forward.iter().any(|&(_, b)| b == r) {
            continue;
        }
        if let Some(min) = forward
            .iter()
            .filter(|&&(a, _)| a == r)
            .map(|&(_, b)| layer[b])
            .min()
        {
            layer[r] = min.saturating_sub(1);
        }
    }

    // Sizes, and children's places inside their parents.
    let mut offsets = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    for &r in &roots {
        measure(spec, r, UNIT_W, &mut offsets, &mut sizes);
    }

    // Columns, each ordered by where its inputs come from.
    let columns = roots.iter().map(|&r| layer[r]).max().map_or(0, |m| m + 1);
    let mut order: Vec<Vec<usize>> = vec![vec![]; columns];
    for &r in &roots {
        order[layer[r]].push(r);
    }
    let mut rank = vec![0.0f32; n];
    for column in order.iter_mut() {
        let keyed: Vec<(f32, usize)> = column
            .iter()
            .enumerate()
            .map(|(at, &u)| {
                let feeds: Vec<f32> = forward
                    .iter()
                    .filter(|&&(_, b)| b == u)
                    .map(|&(a, _)| rank[a])
                    .collect();
                let key = if feeds.is_empty() {
                    at as f32
                } else {
                    feeds.iter().sum::<f32>() / feeds.len() as f32 + at as f32 * 1e-3
                };
                (key, u)
            })
            .collect();
        let mut keyed = keyed;
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        *column = keyed.into_iter().map(|(_, u)| u).collect();
        for (at, &u) in column.iter().enumerate() {
            rank[u] = at as f32;
        }
    }

    let mut boxes = vec![Rect::default(); n];
    let mut x = 0.0;
    for column in &order {
        let width = column.iter().map(|u| sizes[u].0).fold(0.0, f32::max);
        let height: f32 = column.iter().map(|u| sizes[u].1).sum::<f32>()
            + GAP_Y * (column.len().max(1) - 1) as f32;
        let mut y = -height / 2.0;
        for &u in column {
            let (w, h) = sizes[&u];
            boxes[u] = Rect {
                x: x + (width - w) / 2.0,
                y,
                w,
                h,
            };
            y += h + GAP_Y;
        }
        x += width + GAP_X;
    }
    // Children, parents first: spec order may list a child before its parent.
    let mut depth = vec![0usize; n];
    for at in 0..n {
        let mut d = 0;
        let mut id = spec.units[at].id.as_str();
        while let Some(parent) = spec.unit(id).and_then(|u| u.parent.as_deref()) {
            d += 1;
            id = parent;
            if d > n {
                break;
            }
        }
        depth[at] = d;
    }
    let mut by_depth: Vec<usize> = (0..n).filter(|&at| depth[at] > 0).collect();
    by_depth.sort_by_key(|&at| depth[at]);
    for at in by_depth {
        let parent = spec.units[at]
            .parent
            .as_deref()
            .and_then(|p| index.get(p).copied());
        if let (Some(p), Some(&(ox, oy)), Some(&(w, h))) =
            (parent, offsets.get(&at), sizes.get(&at))
        {
            boxes[at] = Rect {
                x: boxes[p].x + ox,
                y: boxes[p].y + oy,
                w,
                h,
            };
        }
    }

    let back = spec
        .links
        .iter()
        .map(|link| match (root(&link.from.unit), root(&link.to.unit)) {
            (Some(a), Some(b)) if a != b => link.feedback || layer[b] <= layer[a],
            _ => false,
        })
        .collect();
    Layout { boxes, depth, back }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Accent, Link, Unit};

    fn chain() -> GraphSpec {
        let mut spec = GraphSpec::new("t", "T", "");
        spec.units.push(
            Unit::new("b", "B", "", Accent::Core)
                .input("in", "in", "x")
                .output("out", "out", "x"),
        );
        spec.units
            .push(Unit::new("a", "A", "", Accent::Info).output("out", "out", "x"));
        spec.units.push(
            Unit::new("c", "C", "", Accent::Info)
                .input("in", "in", "x")
                .output("back", "back", "x"),
        );
        spec.units
            .push(Unit::new("c1", "C1", "", Accent::Info).inside("c"));
        spec.links.push(Link::new(("a", "out"), ("b", "in")));
        spec.links.push(Link::new(("b", "out"), ("c", "in")));
        spec.links.push(Link::new(("c", "back"), ("b", "in")));
        spec
    }

    #[test]
    fn data_flows_left_to_right_and_loops_are_marked() {
        let spec = chain();
        spec.validate().unwrap();
        let l = layout(&spec);
        let (a, b, c) = (l.boxes[1], l.boxes[0], l.boxes[2]);
        assert!(a.x + a.w < b.x && b.x + b.w < c.x, "{a:?} {b:?} {c:?}");
        assert_eq!(l.back, vec![false, false, true]);
    }

    #[test]
    fn children_sit_inside_their_graph_unit_and_move_with_it() {
        let spec = chain();
        let mut l = layout(&spec);
        let (c, c1) = (l.boxes[2], l.boxes[3]);
        assert!(c.contains(c1.x, c1.y) && c.contains(c1.x + c1.w, c1.y + c1.h));
        assert_eq!(l.hit(c1.x + 1.0, c1.y + 1.0), Some(3));
        l.translate(&spec, 2, 10.0, 5.0);
        assert_eq!(l.boxes[3].x, c1.x + 10.0);
        assert_eq!(l.boxes[3].y, c1.y + 5.0);
    }

    #[test]
    fn pins_sit_on_their_edges() {
        let spec = chain();
        let l = layout(&spec);
        let (x, _, side) = l.pin(&spec, 0, "in").unwrap();
        assert_eq!((x, side), (l.boxes[0].x, Side::Input));
        let (x, _, side) = l.pin(&spec, 0, "out").unwrap();
        assert_eq!((x, side), (l.boxes[0].x + l.boxes[0].w, Side::Output));
    }

    #[test]
    fn the_layout_is_the_same_every_time() {
        let spec = chain();
        assert_eq!(layout(&spec), layout(&spec));
    }
}
