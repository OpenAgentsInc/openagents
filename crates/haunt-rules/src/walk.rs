//! Walking: the house's walls with their doorways, floor props as boxes,
//! and a move that slides along whatever it meets, like Doom.

use haunt_assets::Catalog;

use crate::check::{blocks_floor, footprint};
use crate::doc::{Area, House, OUTSIDE, Wall};

/// The player's radius, m.
pub const RADIUS: f32 = 0.3;

/// A wall segment along one axis: `fixed` is its x (for a north-south
/// wall) or z (for an east-west wall), and it runs from `a` to `b` along
/// the other axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub along_x: bool,
    pub fixed: f32,
    pub a: f32,
    pub b: f32,
}

impl Segment {
    fn ends(&self) -> ([f32; 2], [f32; 2]) {
        if self.along_x {
            ([self.a, self.fixed], [self.b, self.fixed])
        } else {
            ([self.fixed, self.a], [self.fixed, self.b])
        }
    }

    /// The nearest point of the segment to `p`.
    #[must_use]
    pub fn nearest(&self, p: [f32; 2]) -> [f32; 2] {
        if self.along_x {
            [p[0].clamp(self.a, self.b), self.fixed]
        } else {
            [self.fixed, p[1].clamp(self.a, self.b)]
        }
    }
}

/// What a walker bumps into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Walls {
    pub segments: Vec<Segment>,
    /// Floor props, `[x0, z0, x1, z1]`.
    pub boxes: Vec<[f32; 4]>,
}

fn cut(spans: &mut Vec<(f32, f32)>, s: f32, e: f32) {
    let mut out = Vec::new();
    for (a, b) in spans.drain(..) {
        if e <= a || s >= b {
            out.push((a, b));
            continue;
        }
        if s > a {
            out.push((a, s));
        }
        if e < b {
            out.push((e, b));
        }
    }
    *spans = out;
}

/// An area's walls with its doorways left open.
#[must_use]
pub fn area_segments(area: &Area) -> Vec<Segment> {
    let mut out = Vec::new();
    for wall in Wall::ALL {
        let (fixed, start, end) = area.wall_line(wall);
        let mut spans = vec![(start, end)];
        for door in area.doors.iter().filter(|d| d.wall == wall) {
            let (s, e) = area.door_span(door);
            cut(&mut spans, s, e);
        }
        out.extend(spans.into_iter().map(|(a, b)| Segment {
            along_x: wall.along_x(),
            fixed,
            a,
            b,
        }));
    }
    out
}

impl Walls {
    /// Every wall of the house, with doorways open, and its floor props.
    #[must_use]
    pub fn of(house: &House, catalog: &Catalog) -> Self {
        let mut walls = Walls::default();
        for area in &house.areas {
            walls.segments.extend(area_segments(area));
            for p in &area.props {
                if let Some(asset) = catalog.asset(&p.asset)
                    && blocks_floor(p, asset.surface)
                {
                    walls.boxes.push(footprint(area, p, asset.footprint));
                }
            }
        }
        walls
    }

    /// Pushes a circle of `radius` at `p` out of every wall and box.
    #[must_use]
    pub fn push_out(&self, mut p: [f32; 2], radius: f32) -> [f32; 2] {
        for _ in 0..3 {
            for s in &self.segments {
                let q = s.nearest(p);
                p = away(p, q, radius);
                // The doorway's jambs are the segment's ends; nearest()
                // covers them.
                let _ = s.ends();
            }
            for b in &self.boxes {
                let q = [p[0].clamp(b[0], b[2]), p[1].clamp(b[1], b[3])];
                p = away(p, q, radius);
            }
        }
        p
    }

    /// Moves from `from` by `delta`, sliding along walls: the move is split
    /// into small steps, each pushed back out.
    #[must_use]
    pub fn slide(&self, from: [f32; 2], delta: [f32; 2], radius: f32) -> [f32; 2] {
        let len = delta[0].hypot(delta[1]);
        let steps = (len / (radius * 0.5)).ceil().max(1.0) as usize;
        let mut p = from;
        for _ in 0..steps {
            p = [
                p[0] + delta[0] / steps as f32,
                p[1] + delta[1] / steps as f32,
            ];
            p = self.push_out(p, radius);
        }
        p
    }
}

fn away(p: [f32; 2], q: [f32; 2], radius: f32) -> [f32; 2] {
    let (dx, dz) = (p[0] - q[0], p[1] - q[1]);
    let d = dx.hypot(dz);
    if d >= radius {
        return p;
    }
    if d < 1e-6 {
        return [p[0] + radius, p[1]];
    }
    let k = (radius - d) / d;
    [p[0] + dx * k, p[1] + dz * k]
}

/// The area a point stands in, if any.
#[must_use]
pub fn area_at<'a>(house: &'a House, p: [f32; 2]) -> Option<&'a Area> {
    house.areas.iter().find(|a| a.contains(p, 0.0))
}

/// Whether a point has left the house through its way out: outside every
/// area, and within 2 m of the exit door.
#[must_use]
pub fn left_by_exit(house: &House, p: [f32; 2]) -> bool {
    if area_at(house, p).is_some() {
        return false;
    }
    let Some(area) = house.area(&house.exit.area) else {
        return false;
    };
    area.doors
        .iter()
        .filter(|d| d.id == house.exit.door && d.to == OUTSIDE)
        .any(|d| {
            let c = area.door_centre(d);
            (c[0] - p[0]).hypot(c[1] - p[1]) < 2.0
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::v1;

    #[test]
    fn walking_into_a_wall_slides_along_it() {
        let house = v1();
        let walls = Walls::of(&house, haunt_assets::catalog());
        // In the entry hall (x 6.5 to 9.5), pushing east-north-east.
        let start = [8.0, 24.0];
        let end = walls.slide(start, [3.0, 1.0], RADIUS);
        assert!(end[0] <= 9.5 - RADIUS + 1e-3, "{end:?}");
        assert!(end[1] > 24.9, "slid north along the wall: {end:?}");
    }

    #[test]
    fn a_doorway_lets_you_through_and_the_back_door_lets_you_out() {
        let house = v1();
        let walls = Walls::of(&house, haunt_assets::catalog());
        // Through the front door from the yard into the entry hall.
        let mut p = [8.0, 18.0];
        for _ in 0..40 {
            p = walls.slide(p, [0.0, 0.1], RADIUS);
        }
        assert!(p[1] > 21.0, "{p:?}");
        assert_eq!(
            area_at(&house, p).map(|a| a.id.as_str()),
            Some("entry-hall")
        );
        // A wall beside the door stops you.
        let mut q = [3.0, 18.0];
        for _ in 0..40 {
            q = walls.slide(q, [0.0, 0.1], RADIUS);
        }
        assert!(q[1] < 20.0, "{q:?}");
        // Out of the back door.
        let mut r = [8.5, 66.0];
        for _ in 0..40 {
            r = walls.slide(r, [0.0, 0.1], RADIUS);
        }
        assert!(left_by_exit(&house, r), "{r:?}");
    }
}
