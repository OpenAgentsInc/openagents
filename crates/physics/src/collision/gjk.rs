//! Distance and penetration between convex shapes from their support
//! functions: GJK (Gilbert, Johnson, and Keerthi, "A fast procedure for
//! computing the distance between complex objects in three-dimensional
//! space", 1988), with the closest-point simplex reduction of Ericson
//! (Real-Time Collision Detection, 2005, 5.1 and 9.5), and EPA for depth
//! when the shapes overlap (van den Bergen, "Proximity Queries and
//! Penetration Depth Computation on 3D Game Objects", GDC 2001).
//!
//! Genesis (<https://github.com/Genesis-Embodied-AI/Genesis>, genesis-world
//! `df30b61a`, Apache-2.0) has the same GJK and EPA pair in
//! `genesis/engine/solvers/rigid/collider/` (`gjk.py`, `epa.py`), which is
//! where this port started (#11656); it is credited here, used as an oracle
//! for contact sets, and none of its code is translated.
//!
//! Each shape is a convex core plus a radius (a sphere is a point, a
//! capsule a segment, a box or hull radius zero), so rounded shapes cost
//! nothing extra. Loops run a fixed order with fixed iteration caps and
//! ties go to the first candidate, so the result is a pure function of the
//! inputs.

use glam::DVec3;

/// A convex core in the world frame.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Core<'a> {
    Point(DVec3),
    Segment(DVec3, DVec3),
    /// World-space corners of a polytope.
    Points(&'a [DVec3]),
}

impl Core<'_> {
    pub(crate) fn support(&self, d: DVec3) -> DVec3 {
        match *self {
            Self::Point(p) => p,
            Self::Segment(p, q) => {
                if (q - p).dot(d) > 0.0 {
                    q
                } else {
                    p
                }
            }
            Self::Points(points) => {
                let mut best = points[0];
                let mut far = best.dot(d);
                for p in &points[1..] {
                    let h = p.dot(d);
                    if h > far {
                        far = h;
                        best = *p;
                    }
                }
                best
            }
        }
    }

    /// A point inside the core.
    pub(crate) fn center(&self) -> DVec3 {
        match *self {
            Self::Point(p) => p,
            Self::Segment(p, q) => (p + q) * 0.5,
            Self::Points(points) => points.iter().copied().sum::<DVec3>() / points.len() as f64,
        }
    }
}

/// A vertex of the Minkowski difference `A - B`, with the points of `A`
/// and `B` it came from.
#[derive(Clone, Copy, Debug)]
struct Vertex {
    w: DVec3,
    a: DVec3,
    b: DVec3,
}

fn support(a: &Core, b: &Core, d: DVec3) -> Vertex {
    let pa = a.support(d);
    let pb = b.support(-d);
    Vertex {
        w: pa - pb,
        a: pa,
        b: pb,
    }
}

/// How two cores lie: the normal points from `A` to `B`, and `distance`
/// is the gap between the cores, negative when they overlap. `pa` and
/// `pb` are the witness points on each core.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Proximity {
    pub normal: DVec3,
    pub distance: f64,
    pub pa: DVec3,
    pub pb: DVec3,
}

/// The cores' proximity, or `None` when they are farther apart than
/// `cutoff`.
pub(crate) fn proximity(a: &Core, b: &Core, cutoff: f64, scale: f64) -> Option<Proximity> {
    let tol = scale.max(1e-9) * 1e-10;
    let mut simplex: Vec<Vertex> = Vec::with_capacity(4);
    let start = b.center() - a.center();
    let start = if start.length_squared() > tol * tol {
        start
    } else {
        DVec3::X
    };
    simplex.push(support(a, b, -start));
    let mut v = simplex[0].w;
    let mut weights = vec![1.0];
    let mut overlap = false;
    for _ in 0..64 {
        let vv = v.length_squared();
        if vv <= tol * tol {
            overlap = true;
            break;
        }
        let w = support(a, b, -v);
        // The gap's lower bound already passes the cutoff: done early.
        let lower = v.dot(w.w) / vv.sqrt();
        if lower > cutoff {
            return None;
        }
        if vv - v.dot(w.w) <= vv * 1e-12 || simplex.iter().any(|s| s.w == w.w) {
            break;
        }
        simplex.push(w);
        let (point, kept) = closest(&simplex);
        if kept.is_empty() {
            overlap = true;
            break;
        }
        simplex = kept.iter().map(|&(i, _)| simplex[i]).collect();
        weights = kept.iter().map(|&(_, l)| l).collect();
        v = point;
    }
    if !overlap {
        let distance = v.length();
        if distance > cutoff {
            return None;
        }
        let pa = simplex
            .iter()
            .zip(&weights)
            .map(|(s, l)| s.a * *l)
            .sum::<DVec3>();
        let pb = simplex
            .iter()
            .zip(&weights)
            .map(|(s, l)| s.b * *l)
            .sum::<DVec3>();
        return Some(Proximity {
            normal: -v / distance,
            distance,
            pa,
            pb,
        });
    }
    Some(epa(a, b, simplex, scale).unwrap_or_else(|| axis_fallback(a, b)))
}

/// Overlap along the line between the cores' centers: an upper bound on
/// the depth, when EPA cannot run.
fn axis_fallback(a: &Core, b: &Core) -> Proximity {
    let n = (b.center() - a.center()).normalize_or(DVec3::Y);
    let pa = a.support(n);
    let pb = b.support(-n);
    Proximity {
        normal: n,
        distance: (pb - pa).dot(n),
        pa,
        pb,
    }
}

/// The point of the simplex nearest the origin, with the vertices that
/// span it and their barycentric weights. Empty when the origin is inside
/// a tetrahedron.
fn closest(s: &[Vertex]) -> (DVec3, Vec<(usize, f64)>) {
    match s.len() {
        1 => (s[0].w, vec![(0, 1.0)]),
        2 => segment(s[0].w, s[1].w, [0, 1]),
        3 => triangle(s[0].w, s[1].w, s[2].w, [0, 1, 2]),
        _ => tetrahedron(s),
    }
}

fn segment(a: DVec3, b: DVec3, ids: [usize; 2]) -> (DVec3, Vec<(usize, f64)>) {
    let ab = b - a;
    let t = -a.dot(ab);
    if t <= 0.0 {
        return (a, vec![(ids[0], 1.0)]);
    }
    let denom = ab.length_squared();
    if t >= denom {
        return (b, vec![(ids[1], 1.0)]);
    }
    let t = t / denom;
    (a + ab * t, vec![(ids[0], 1.0 - t), (ids[1], t)])
}

/// Ericson 5.1.5, the closest point of a triangle to the origin.
fn triangle(a: DVec3, b: DVec3, c: DVec3, ids: [usize; 3]) -> (DVec3, Vec<(usize, f64)>) {
    let ab = b - a;
    let ac = c - a;
    let ap = -a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, vec![(ids[0], 1.0)]);
    }
    let bp = -b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, vec![(ids[1], 1.0)]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (a + ab * v, vec![(ids[0], 1.0 - v), (ids[1], v)]);
    }
    let cp = -c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, vec![(ids[2], 1.0)]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (a + ac * w, vec![(ids[0], 1.0 - w), (ids[2], w)]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (b + (c - b) * w, vec![(ids[1], 1.0 - w), (ids[2], w)]);
    }
    let denom = va + vb + vc;
    if denom.abs() <= f64::MIN_POSITIVE {
        // A degenerate triangle: its longest edge.
        let mut best = segment(a, b, [ids[0], ids[1]]);
        for (p, q, i) in [(b, c, [ids[1], ids[2]]), (a, c, [ids[0], ids[2]])] {
            let other = segment(p, q, i);
            if other.0.length_squared() < best.0.length_squared() {
                best = other;
            }
        }
        return best;
    }
    let v = vb / denom;
    let w = vc / denom;
    (
        a + ab * v + ac * w,
        vec![(ids[0], 1.0 - v - w), (ids[1], v), (ids[2], w)],
    )
}

fn tetrahedron(s: &[Vertex]) -> (DVec3, Vec<(usize, f64)>) {
    let p = [s[0].w, s[1].w, s[2].w, s[3].w];
    let faces = [[0, 1, 2, 3], [0, 3, 1, 2], [0, 2, 3, 1], [1, 3, 2, 0]];
    let mut best: Option<(DVec3, Vec<(usize, f64)>)> = None;
    let mut outside_any = false;
    for [i, j, k, opposite] in faces {
        let n = (p[j] - p[i]).cross(p[k] - p[i]);
        let origin_side = -p[i].dot(n);
        let other_side = (p[opposite] - p[i]).dot(n);
        // The origin is beyond this face when it lies on the other side
        // from the fourth vertex.
        if origin_side * other_side < 0.0 {
            outside_any = true;
            let candidate = triangle(p[i], p[j], p[k], [i, j, k]);
            if best
                .as_ref()
                .is_none_or(|b| candidate.0.length_squared() < b.0.length_squared())
            {
                best = Some(candidate);
            }
        }
    }
    if !outside_any {
        return (DVec3::ZERO, Vec::new());
    }
    best.unwrap_or((DVec3::ZERO, Vec::new()))
}

/// The expanding polytope: depth and normal of overlapping cores from
/// GJK's final simplex.
fn epa(a: &Core, b: &Core, mut simplex: Vec<Vertex>, scale: f64) -> Option<Proximity> {
    let tol = scale.max(1e-9) * 1e-9;
    // Grow the simplex to a tetrahedron that spans space.
    let axes = [
        DVec3::X,
        DVec3::Y,
        DVec3::Z,
        DVec3::NEG_X,
        DVec3::NEG_Y,
        DVec3::NEG_Z,
    ];
    if simplex.len() == 1 {
        for d in axes {
            let w = support(a, b, d);
            if (w.w - simplex[0].w).length() > tol {
                simplex.push(w);
                break;
            }
        }
    }
    if simplex.len() == 2 {
        let line = (simplex[1].w - simplex[0].w).normalize_or_zero();
        let side = line.any_orthonormal_vector();
        for k in 0..6 {
            let angle = f64::from(k) * std::f64::consts::FRAC_PI_3;
            let d = glam::DQuat::from_axis_angle(line, angle) * side;
            let w = support(a, b, d);
            if (w.w - simplex[0].w).cross(line).length() > tol {
                simplex.push(w);
                break;
            }
        }
    }
    if simplex.len() == 3 {
        let n = (simplex[1].w - simplex[0].w).cross(simplex[2].w - simplex[0].w);
        for d in [n, -n] {
            let w = support(a, b, d);
            if (w.w - simplex[0].w).dot(n.normalize_or_zero()).abs() > tol {
                simplex.push(w);
                break;
            }
        }
    }
    if simplex.len() < 4 {
        return None;
    }
    let mut verts = simplex;
    let inside = verts.iter().map(|v| v.w).sum::<DVec3>() * 0.25;
    struct Face {
        v: [usize; 3],
        normal: DVec3,
        distance: f64,
        alive: bool,
    }
    let make = |verts: &[Vertex], v: [usize; 3]| -> Option<Face> {
        let (p, q, r) = (verts[v[0]].w, verts[v[1]].w, verts[v[2]].w);
        let n = (q - p).cross(r - p);
        let len = n.length();
        if len <= tol * tol {
            return None;
        }
        let (v, n) = if n.dot(p - inside) < 0.0 {
            ([v[0], v[2], v[1]], -n / len)
        } else {
            (v, n / len)
        };
        Some(Face {
            v,
            normal: n,
            distance: n.dot(p),
            alive: true,
        })
    };
    let mut faces: Vec<Face> = Vec::new();
    for v in [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]] {
        faces.push(make(&verts, v)?);
    }
    let mut best = 0;
    for _ in 0..96 {
        best = usize::MAX;
        let mut near = f64::INFINITY;
        for (i, f) in faces.iter().enumerate() {
            if f.alive && f.distance < near {
                near = f.distance;
                best = i;
            }
        }
        if best == usize::MAX {
            return None;
        }
        let n = faces[best].normal;
        let w = support(a, b, n);
        if w.w.dot(n) - near <= tol || faces.len() > 512 {
            break;
        }
        let index = verts.len();
        verts.push(w);
        let mut horizon: Vec<[usize; 2]> = Vec::new();
        let seen: Vec<usize> = (0..faces.len())
            .filter(|&i| faces[i].alive && faces[i].normal.dot(w.w - verts[faces[i].v[0]].w) > tol)
            .collect();
        for &i in &seen {
            let v = faces[i].v;
            for e in 0..3 {
                let (p, q) = (v[e], v[(e + 1) % 3]);
                let twin = seen.iter().any(|&o| {
                    let u = faces[o].v;
                    (0..3).any(|g| u[g] == q && u[(g + 1) % 3] == p)
                });
                if !twin {
                    horizon.push([p, q]);
                }
            }
        }
        for &i in &seen {
            faces[i].alive = false;
        }
        for [p, q] in horizon {
            if let Some(f) = make(&verts, [p, q, index]) {
                faces.push(f);
            }
        }
    }
    let face = &faces[best];
    let n = face.normal;
    // Barycentric coordinates of the origin's projection on the face.
    let (p, q, r) = (verts[face.v[0]].w, verts[face.v[1]].w, verts[face.v[2]].w);
    let x = n * face.distance;
    let total = (q - p).cross(r - p).dot(n);
    let (l1, l2) = if total.abs() > 0.0 {
        (
            (r - q).cross(x - q).dot(n) / total,
            (p - r).cross(x - r).dot(n) / total,
        )
    } else {
        (1.0 / 3.0, 1.0 / 3.0)
    };
    let l3 = 1.0 - l1 - l2;
    let pick = |f: &dyn Fn(&Vertex) -> DVec3| {
        f(&verts[face.v[0]]) * l1 + f(&verts[face.v[1]]) * l2 + f(&verts[face.v[2]]) * l3
    };
    Some(Proximity {
        normal: n,
        distance: -face.distance.max(0.0),
        pa: pick(&|v| v.a),
        pb: pick(&|v| v.b),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(center: DVec3, half: f64) -> Vec<DVec3> {
        (0..8)
            .map(|i| {
                let s = |bit: u32| if i & bit == 0 { -half } else { half };
                center + DVec3::new(s(1), s(2), s(4))
            })
            .collect()
    }

    #[test]
    fn separated_boxes_report_the_gap_and_its_direction() {
        let (a, b) = (
            cube(DVec3::ZERO, 0.5),
            cube(DVec3::new(2.0, 0.3, -0.2), 0.5),
        );
        let (a, b) = (Core::Points(&a), Core::Points(&b));
        let p = proximity(&a, &b, 10.0, 2.0).unwrap();
        assert!((p.distance - 1.0).abs() < 1e-12, "{p:?}");
        assert!((p.normal - DVec3::X).length() < 1e-12);
        assert!(proximity(&a, &b, 0.5, 2.0).is_none());
    }

    #[test]
    fn overlapping_shapes_report_the_least_depth() {
        let (a, b) = (cube(DVec3::ZERO, 0.5), cube(DVec3::new(0.0, 0.8, 0.1), 0.5));
        let (a, b) = (Core::Points(&a), Core::Points(&b));
        let p = proximity(&a, &b, 1.0, 2.0).unwrap();
        assert!((p.distance + 0.2).abs() < 1e-9, "{p:?}");
        assert!((p.normal - DVec3::Y).length() < 1e-9);
        // A point deep in a box leaves through the nearest face.
        let point = Core::Point(DVec3::new(0.0, 0.0, -0.3));
        let p = proximity(&a, &point, 1.0, 1.0).unwrap();
        assert!((p.distance + 0.2).abs() < 1e-9, "{p:?}");
        assert!((p.normal + DVec3::Z).length() < 1e-9);
        // A segment piercing a box.
        let seg = Core::Segment(DVec3::new(-2.0, 0.4, 0.0), DVec3::new(2.0, 0.4, 0.0));
        let p = proximity(&a, &seg, 1.0, 2.0).unwrap();
        assert!((p.distance + 0.1).abs() < 1e-9, "{p:?}");
        assert!((p.normal - DVec3::Y).length() < 1e-9);
    }

    #[test]
    fn a_point_and_a_segment_match_the_closed_forms() {
        let seg = Core::Segment(DVec3::new(-1.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0));
        let point = Core::Point(DVec3::new(0.3, 2.0, 0.0));
        let p = proximity(&seg, &point, 5.0, 2.0).unwrap();
        assert!((p.distance - 2.0).abs() < 1e-12);
        assert!((p.pa - DVec3::new(0.3, 0.0, 0.0)).length() < 1e-12);
        assert!((p.pb - DVec3::new(0.3, 2.0, 0.0)).length() < 1e-12);
    }
}
