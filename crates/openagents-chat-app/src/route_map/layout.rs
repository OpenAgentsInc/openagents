//! The route map's geometry: where each node sits, how big it draws, the
//! camera that pans and zooms over it, hit-testing, which labels show at
//! a zoom, and which node an arrow key moves to.
//!
//! The layout is a radial tree around the front: each node gets an angle
//! span in proportion to the leaves under it and sits on its depth's ring.
//! It depends only on the tree's shape and order, which come from the
//! sources, so positions hold across refreshes of local counts, engine
//! readings, and records. World units are points at zoom 1.

use super::{Kind, Map};

/// The ring radius at each depth, in world units.
pub const RINGS: [f32; 6] = [0.0, 240.0, 500.0, 790.0, 1080.0, 1320.0];
/// Empty leaves between families, so they read as groups.
const FAMILY_PAD: f32 = 2.0;
/// The least and most zoom.
pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 4.0;
/// How much one zoom step (Cmd + or −, a wheel notch) multiplies by.
pub const ZOOM_STEP: f32 = 1.25;

/// A point in world or screen units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    #[must_use]
    pub fn distance(self, other: Point) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// Where every node sits.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Node centers, by node index.
    pub positions: Vec<Point>,
    /// Node radii in world units, by node index.
    pub radii: Vec<f32>,
    /// The angle each node's span starts and ends at, in radians.
    pub spans: Vec<(f32, f32)>,
}

/// The least room a node takes, in leaves: a route or Coder needs room
/// for its own circle and label even with few members.
fn min_leaves(kind: Kind) -> f32 {
    match kind {
        Kind::Route | Kind::Coder | Kind::Family => 2.5,
        _ => 1.0,
    }
}

/// A node's radius in world units: by kind, grown by its weight.
#[must_use]
pub fn radius(kind: Kind, weight: f32) -> f32 {
    let base = match kind {
        Kind::Front => 34.0,
        Kind::Family => 22.0,
        Kind::Route => 16.0,
        Kind::Coder => 20.0,
        Kind::Model => 14.0,
        Kind::Plugin => 12.0,
        Kind::Engine => 10.0,
        Kind::Answer | Kind::Knowledge | Kind::Screen => 7.0,
    };
    base * (0.7 + 0.6 * weight.clamp(0.0, 1.0))
}

impl Layout {
    /// Lays `map` out.
    #[must_use]
    pub fn of(map: &Map) -> Self {
        let n = map.nodes.len();
        let mut leaves = vec![0.0_f32; n];
        // Children come after their parents, so a reverse pass sums leaves.
        for index in (0..n).rev() {
            let children = map.children(index);
            let mut sum: f32 = children.iter().map(|&c| leaves[c]).sum();
            if map.nodes[index].kind == Kind::Front {
                sum += FAMILY_PAD * children.len() as f32;
            }
            leaves[index] = sum.max(min_leaves(map.nodes[index].kind));
        }
        let mut spans = vec![(0.0_f32, 0.0_f32); n];
        let mut positions = vec![Point::default(); n];
        let roots: Vec<usize> = (0..n).filter(|&i| map.nodes[i].parent.is_none()).collect();
        let total: f32 = roots.iter().map(|&r| leaves[r]).sum::<f32>().max(1.0);
        let mut start = -std::f32::consts::FRAC_PI_2;
        for root in roots {
            let width = std::f32::consts::TAU * leaves[root] / total;
            spans[root] = (start, start + width);
            start += width;
        }
        for index in 0..n {
            let (from, to) = spans[index];
            let children = map.children(index);
            let pad = if map.nodes[index].kind == Kind::Front {
                FAMILY_PAD
            } else {
                0.0
            };
            let room = leaves[index].max(1.0);
            let mut at = from + (to - from) * (pad / 2.0) / room;
            for child in children {
                let width = (to - from) * leaves[child] / room;
                spans[child] = (at, at + width);
                at += width + (to - from) * pad / room;
            }
            let depth = usize::from(map.nodes[index].depth).min(RINGS.len() - 1);
            let angle = (from + to) / 2.0;
            positions[index] = if depth == 0 {
                Point::default()
            } else {
                Point::new(RINGS[depth] * angle.cos(), RINGS[depth] * angle.sin())
            };
        }
        let radii = map
            .nodes
            .iter()
            .map(|node| radius(node.kind, node.weight))
            .collect();
        Layout {
            positions,
            radii,
            spans,
        }
    }

    /// The world rectangle every node fits in: min and max corners.
    #[must_use]
    pub fn bounds(&self) -> (Point, Point) {
        let mut min = Point::new(f32::MAX, f32::MAX);
        let mut max = Point::new(f32::MIN, f32::MIN);
        for (p, r) in self.positions.iter().zip(&self.radii) {
            min.x = min.x.min(p.x - r);
            min.y = min.y.min(p.y - r);
            max.x = max.x.max(p.x + r);
            max.y = max.y.max(p.y + r);
        }
        if self.positions.is_empty() {
            return (Point::default(), Point::default());
        }
        (min, max)
    }
}

/// The camera: the world point at the viewport's center and the zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center: Point,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            center: Point::default(),
            zoom: 1.0,
        }
    }
}

impl Camera {
    /// The camera that fits `bounds` in a `width` by `height` viewport
    /// with `margin` points around it.
    #[must_use]
    pub fn fit(bounds: (Point, Point), width: f32, height: f32, margin: f32) -> Self {
        let (min, max) = bounds;
        let w = (max.x - min.x).max(1.0);
        let h = (max.y - min.y).max(1.0);
        let zoom = ((width - 2.0 * margin).max(1.0) / w)
            .min((height - 2.0 * margin).max(1.0) / h)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        Camera {
            center: Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0),
            zoom,
        }
    }

    /// A world point on screen, in a `width` by `height` viewport.
    #[must_use]
    pub fn to_screen(&self, world: Point, width: f32, height: f32) -> Point {
        Point::new(
            (world.x - self.center.x) * self.zoom + width / 2.0,
            (world.y - self.center.y) * self.zoom + height / 2.0,
        )
    }

    /// A screen point in the world.
    #[must_use]
    pub fn to_world(&self, screen: Point, width: f32, height: f32) -> Point {
        Point::new(
            (screen.x - width / 2.0) / self.zoom + self.center.x,
            (screen.y - height / 2.0) / self.zoom + self.center.y,
        )
    }

    /// Moves the view by a drag of `dx`, `dy` screen points.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.center.x -= dx / self.zoom;
        self.center.y -= dy / self.zoom;
    }

    /// Zooms by `factor` keeping the world point under `anchor` (screen)
    /// where it is.
    pub fn zoom_at(&mut self, factor: f32, anchor: Point, width: f32, height: f32) {
        let before = self.to_world(anchor, width, height);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.to_world(anchor, width, height);
        self.center.x += before.x - after.x;
        self.center.y += before.y - after.y;
    }

    /// The camera `t` of the way from `self` to `to` (0 to 1), zooming
    /// geometrically so the motion looks even.
    #[must_use]
    pub fn toward(&self, to: &Camera, t: f32) -> Camera {
        if t <= 0.0 {
            return *self;
        }
        if t >= 1.0 {
            return *to;
        }
        Camera {
            center: Point::new(
                self.center.x + (to.center.x - self.center.x) * t,
                self.center.y + (to.center.y - self.center.y) * t,
            ),
            zoom: (self.zoom.ln() + (to.zoom.ln() - self.zoom.ln()) * t).exp(),
        }
    }
}

/// The node under `screen`, if any: the nearest whose drawn circle, or a
/// 6-point target around a small one, holds the point. `visible` says
/// which nodes count.
#[must_use]
pub fn hit(
    layout: &Layout,
    camera: &Camera,
    screen: Point,
    width: f32,
    height: f32,
    visible: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (index, (&p, &r)) in layout.positions.iter().zip(&layout.radii).enumerate() {
        if !visible(index) {
            continue;
        }
        let at = camera.to_screen(p, width, height);
        let reach = (r * camera.zoom).max(6.0);
        let d = at.distance(screen);
        if d <= reach && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((index, d));
        }
    }
    best.map(|(index, _)| index)
}

/// What a label shows at a zoom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    /// No label.
    None,
    /// The node's name.
    Name,
    /// The name and its evidence line (numbers, stage).
    Evidence,
}

/// Which label a node of `kind` at `depth` shows at `zoom`: the families
/// always, routes from mid zoom, members closer, and evidence lines only
/// zoomed in, so a zoomed-out map isn't cluttered.
#[must_use]
pub fn detail(kind: Kind, depth: u8, zoom: f32) -> Detail {
    let name_from: f32 = match (kind, depth) {
        (Kind::Front | Kind::Family, _) => 0.0,
        (Kind::Route, _) => 0.42,
        (Kind::Coder | Kind::Model, _) => 0.55,
        (_, 0..=3) => 0.95,
        _ => 1.35,
    };
    let evidence_from = match kind {
        Kind::Route | Kind::Plugin | Kind::Engine | Kind::Coder => name_from.max(1.6),
        _ => f32::INFINITY,
    };
    if zoom >= evidence_from {
        Detail::Evidence
    } else if zoom >= name_from {
        Detail::Name
    } else {
        Detail::None
    }
}

/// A direction an arrow key moves in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// The node an arrow key moves to from `from`: the nearest visible node
/// within 60 degrees of the arrow's direction, on screen.
#[must_use]
pub fn step(
    layout: &Layout,
    from: usize,
    direction: Direction,
    visible: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    let origin = layout.positions[from];
    let (dx, dy) = match direction {
        Direction::Left => (-1.0, 0.0),
        Direction::Right => (1.0, 0.0),
        Direction::Up => (0.0, -1.0),
        Direction::Down => (0.0, 1.0),
    };
    let mut best: Option<(usize, f32)> = None;
    for (index, &p) in layout.positions.iter().enumerate() {
        if index == from || !visible(index) {
            continue;
        }
        let (vx, vy) = (p.x - origin.x, p.y - origin.y);
        let length = (vx * vx + vy * vy).sqrt();
        if length < 0.01 {
            continue;
        }
        let cos = (vx * dx + vy * dy) / length;
        if cos < 0.5 {
            continue;
        }
        // Prefer straight ahead: distance grows as the angle widens.
        let score = length * (2.0 - cos);
        if best.is_none_or(|(_, b)| score < b) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index)
}
