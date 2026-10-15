//! Interference between two closed meshes, exact on their triangles
//! (#11648): the volume inside both, and whether their surfaces touch.
//!
//! The volume of A ∩ B is, by the divergence theorem, a third of the flux
//! of `x − x0` through its boundary, which is the part of A's surface
//! inside B plus the part of B's surface inside A. Over one flat triangle
//! `(x − x0)·n` is constant, so each triangle adds its height `(v − x0)·n`
//! times its area inside the other solid, and the whole check comes down
//! to that area:
//!
//! - a triangle no triangle of the other crosses is wholly inside or
//!   outside: one point-in-solid test (three skewed rays, majority) says;
//! - a crossed triangle's inside area is the area enclosed by the segments
//!   where the other's triangles cut it and the pieces of its own edges
//!   that are inside (Green's theorem over oriented segments, no polygon
//!   building).
//!
//! Both meshes' triangles sit in bounding-volume hierarchies, so only
//! triangles near each other are paired and every ray meets only the
//! triangles near it. Faces lying on each other (a declared contact, a
//! part resting on another) are the degenerate case of the method: B is
//! moved by 10⁻⁸ mm in a direction no face is likely to lie along, so no
//! two faces are exactly in one plane and the result moves by at most the
//! faces' area times 10⁻⁸ mm. Whether the solids touch (their
//! surfaces cross or come within [`TOUCH_MM`]) is measured on the meshes
//! as they are.
//!
//! The result is exact for the triangles (to rounding); a curved face's
//! triangles stand off it by at most the display tolerance it was
//! tessellated to. The previous check sampled the shared box on a 28³
//! grid ([`crate::overlap::volume`]) and could miss a slab thinner than a
//! grid cell.

use solidcraft_engine::geom::Mesh;

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: P) -> f64 {
    dot(a, a).sqrt()
}

/// Surfaces this close count as touching (mm).
pub const TOUCH_MM: f64 = 0.01;

/// How far B is moved off exact coincidences (mm), and the direction.
const NUDGE: f64 = 1e-8;
const NUDGE_DIR: P = [0.471_9, 0.618_1, 0.629_0];

#[derive(Clone, Copy)]
struct Tri {
    v: [P; 3],
    /// Unit outward normal.
    n: P,
    area: f64,
    lo: P,
    hi: P,
}

impl Tri {
    fn new(v: [P; 3]) -> Option<Tri> {
        let c = cross(sub(v[1], v[0]), sub(v[2], v[0]));
        let l = len(c);
        if l < 1e-14 {
            return None;
        }
        let mut lo = v[0];
        let mut hi = v[0];
        for p in &v[1..] {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        Some(Tri {
            v,
            n: scale(c, 1.0 / l),
            area: l / 2.0,
            lo,
            hi,
        })
    }
}

fn boxes_meet(a: (P, P), b: (P, P)) -> bool {
    (0..3).all(|k| a.0[k] <= b.1[k] && b.0[k] <= a.1[k])
}

struct Node {
    lo: P,
    hi: P,
    /// Leaf: a range of `order`; inner: children.
    start: usize,
    count: usize,
    left: usize,
    right: usize,
}

/// A bounding-volume hierarchy over a mesh's triangles.
struct Bvh {
    tris: Vec<Tri>,
    order: Vec<usize>,
    nodes: Vec<Node>,
}

impl Bvh {
    fn new(mesh: &Mesh, offset: P) -> Bvh {
        let p = |i: u32| {
            let q = mesh.positions[i as usize];
            [q.x + offset[0], q.y + offset[1], q.z + offset[2]]
        };
        let tris: Vec<Tri> = mesh
            .triangles
            .iter()
            .filter_map(|t| Tri::new([p(t[0]), p(t[1]), p(t[2])]))
            .collect();
        let mut bvh = Bvh {
            order: (0..tris.len()).collect(),
            tris,
            nodes: Vec::new(),
        };
        if !bvh.tris.is_empty() {
            bvh.build(0, bvh.tris.len());
        }
        bvh
    }

    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for &i in &self.order[start..end] {
            let t = &self.tris[i];
            for k in 0..3 {
                lo[k] = lo[k].min(t.lo[k]);
                hi[k] = hi[k].max(t.hi[k]);
            }
        }
        let me = self.nodes.len();
        self.nodes.push(Node {
            lo,
            hi,
            start,
            count: end - start,
            left: 0,
            right: 0,
        });
        if end - start <= 4 {
            return me;
        }
        let axis = (0..3)
            .max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
            .unwrap_or(0);
        let centre = |t: &Tri| t.lo[axis] + t.hi[axis];
        let mid = start + (end - start) / 2;
        let tris = &self.tris;
        self.order[start..end].select_nth_unstable_by(mid - start, |&a, &b| {
            centre(&tris[a]).total_cmp(&centre(&tris[b]))
        });
        let left = self.build(start, mid);
        let right = self.build(mid, end);
        let node = &mut self.nodes[me];
        node.count = 0;
        node.left = left;
        node.right = right;
        me
    }

    fn bounds(&self) -> Option<(P, P)> {
        self.nodes.first().map(|n| (n.lo, n.hi))
    }

    /// Every triangle whose box meets `b`.
    fn near(&self, b: (P, P), out: &mut Vec<usize>) {
        out.clear();
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n];
            if !boxes_meet((node.lo, node.hi), b) {
                continue;
            }
            if node.count > 0 {
                for &i in &self.order[node.start..node.start + node.count] {
                    if boxes_meet((self.tris[i].lo, self.tris[i].hi), b) {
                        out.push(i);
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
    }

    /// How many triangles a ray from `o` along `d` crosses.
    fn crossings(&self, o: P, d: P) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let inv = [1.0 / d[0], 1.0 / d[1], 1.0 / d[2]];
        let mut n = 0;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            // Slab test.
            let mut t0 = 0.0f64;
            let mut t1 = f64::INFINITY;
            for k in 0..3 {
                let a = (node.lo[k] - o[k]) * inv[k];
                let b = (node.hi[k] - o[k]) * inv[k];
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
            if t0 > t1 {
                continue;
            }
            if node.count > 0 {
                for &t in &self.order[node.start..node.start + node.count] {
                    if ray_hits(o, d, &self.tris[t]) {
                        n += 1;
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
        n
    }

    /// Whether `p` is inside the closed mesh: three skewed rays, majority.
    fn contains(&self, p: P) -> bool {
        let Some((lo, hi)) = self.bounds() else {
            return false;
        };
        if (0..3).any(|k| p[k] < lo[k] || p[k] > hi[k]) {
            return false;
        }
        const RAYS: [P; 3] = [
            [0.577_3, 0.577_4, 0.577_6],
            [-0.612_3, 0.314_1, 0.725_4],
            [0.271_8, -0.841_4, 0.467_3],
        ];
        RAYS.iter()
            .filter(|d| self.crossings(p, **d) % 2 == 1)
            .count()
            >= 2
    }
}

/// Möller–Trumbore: does the ray from `o` along `d` cross the triangle?
fn ray_hits(o: P, d: P, t: &Tri) -> bool {
    let (e1, e2) = (sub(t.v[1], t.v[0]), sub(t.v[2], t.v[0]));
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-300 {
        return false;
    }
    let s = sub(o, t.v[0]);
    let u = dot(s, p) / det;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross(s, e1);
    let v = dot(d, q) / det;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    dot(e2, q) / det > 0.0
}

/// The side of `t`'s plane a point is on (exactly on it counts as above,
/// the same way every time).
fn side(t: &Tri, p: P) -> f64 {
    dot(sub(p, t.v[0]), t.n)
}

/// Where the edges of `t` cross the plane of `u`: the segment `t` makes
/// there, if it reaches across.
fn cut_by_plane(t: &Tri, u: &Tri) -> Option<(P, P)> {
    let d = t.v.map(|p| side(u, p));
    let up = d.map(|x| x >= 0.0);
    if up.iter().all(|x| *x) || up.iter().all(|x| !*x) {
        return None;
    }
    let mut points = Vec::with_capacity(2);
    for i in 0..3 {
        let j = (i + 1) % 3;
        if up[i] != up[j] {
            let s = d[i] / (d[i] - d[j]);
            points.push(add(t.v[i], scale(sub(t.v[j], t.v[i]), s)));
        }
    }
    (points.len() == 2).then(|| (points[0], points[1]))
}

/// The segment where triangles `t` and `u` meet, oriented so the inside
/// of `u`'s solid is on its left seen from `t`'s outside.
fn meet(t: &Tri, u: &Tri) -> Option<(P, P)> {
    let a = cut_by_plane(t, u)?;
    let b = cut_by_plane(u, t)?;
    let line = cross(t.n, u.n);
    if len(line) < 1e-12 {
        return None;
    }
    let along = |p: P| dot(p, line);
    let (ta0, ta1) = (along(a.0), along(a.1));
    let (tb0, tb1) = (along(b.0), along(b.1));
    let lo = ta0.min(ta1).max(tb0.min(tb1));
    let hi = ta0.max(ta1).min(tb0.max(tb1));
    if lo >= hi || (ta1 - ta0).abs() < 1e-300 {
        return None;
    }
    let at = |t: f64| add(a.0, scale(sub(a.1, a.0), (t - ta0) / (ta1 - ta0)));
    // Along n_t × n_u the inside of u's solid is on the left.
    Some((at(lo), at(hi)))
}

/// Where segment a→b crosses triangle `u` (the fraction along it).
fn edge_crosses(a: P, b: P, u: &Tri) -> Option<f64> {
    let (da, db) = (side(u, a), side(u, b));
    if (da >= 0.0) == (db >= 0.0) {
        return None;
    }
    let s = da / (da - db);
    let p = add(a, scale(sub(b, a), s));
    for i in 0..3 {
        let e = sub(u.v[(i + 1) % 3], u.v[i]);
        if dot(cross(e, sub(p, u.v[i])), u.n) < 0.0 {
            return None;
        }
    }
    Some(s)
}

/// The area of `t` inside the other solid, and whether any triangle of it
/// crosses `t`.
fn inside_area(t: &Tri, other: &Bvh, near: &mut Vec<usize>) -> (f64, bool) {
    other.near((t.lo, t.hi), near);
    let mut cuts = Vec::new();
    for &i in near.iter() {
        if let Some(s) = meet(t, &other.tris[i]) {
            cuts.push(s);
        }
    }
    if cuts.is_empty() {
        let c = scale(add(add(t.v[0], t.v[1]), t.v[2]), 1.0 / 3.0);
        return (if other.contains(c) { t.area } else { 0.0 }, false);
    }
    // Green's theorem in t's plane: area = ½ Σ x₁y₂ − x₂y₁ over the
    // boundary's oriented segments.
    let o = t.v[0];
    let e1 = {
        let e = sub(t.v[1], o);
        scale(e, 1.0 / len(e))
    };
    let e2 = cross(t.n, e1);
    let flat = |p: P| {
        let d = sub(p, o);
        (dot(d, e1), dot(d, e2))
    };
    let piece = |a: P, b: P| {
        let (x1, y1) = flat(a);
        let (x2, y2) = flat(b);
        0.5 * (x1 * y2 - x2 * y1)
    };
    let mut area: f64 = cuts.iter().map(|(a, b)| piece(*a, *b)).sum();
    for k in 0..3 {
        let (a, b) = (t.v[k], t.v[(k + 1) % 3]);
        let mut at: Vec<f64> = near
            .iter()
            .filter_map(|&i| edge_crosses(a, b, &other.tris[i]))
            .collect();
        at.push(0.0);
        at.push(1.0);
        at.sort_by(f64::total_cmp);
        at.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
        for w in at.windows(2) {
            let p = add(a, scale(sub(b, a), w[0]));
            let q = add(a, scale(sub(b, a), w[1]));
            if other.contains(scale(add(p, q), 0.5)) {
                area += piece(p, q);
            }
        }
    }
    (area.clamp(0.0, t.area), true)
}

/// The closest distance from `p` to triangle `t` (Ericson, Real-Time
/// Collision Detection, 5.1.5).
fn distance(p: P, t: &Tri) -> f64 {
    let [a, b, c] = t.v;
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return len(ap);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return len(bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return len(sub(p, add(a, scale(ab, v))));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return len(cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return len(sub(p, add(a, scale(ac, w))));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return len(sub(p, add(b, scale(sub(c, b), w))));
    }
    side(t, p).abs()
}

/// What two closed meshes share.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shared {
    /// The volume inside both (mm³), exact on the triangles.
    pub volume: f64,
    /// Whether the surfaces cross or come within [`TOUCH_MM`].
    pub touching: bool,
    /// Triangles of the two meshes compared.
    pub triangles: usize,
}

/// The volume inside both meshes and whether they touch.
#[must_use]
pub fn shared(a: &Mesh, b: &Mesh) -> Shared {
    let ba = Bvh::new(a, [0.0; 3]);
    let nudge = scale(NUDGE_DIR, NUDGE / len(NUDGE_DIR));
    let bb = Bvh::new(b, nudge);
    let (Some(la), Some(lb)) = (ba.bounds(), bb.bounds()) else {
        return Shared::default();
    };
    let lo: P = [0, 1, 2].map(|k| la.0[k].max(lb.0[k]) - TOUCH_MM);
    let hi: P = [0, 1, 2].map(|k| la.1[k].min(lb.1[k]) + TOUCH_MM);
    if (0..3).any(|k| lo[k] > hi[k]) {
        return Shared::default();
    }
    let x0 = scale(add(lo, hi), 0.5);
    let mut near = Vec::new();
    let mut in_region = Vec::new();
    let mut volume = 0.0;
    let mut crossed = false;
    let mut triangles = 0;
    for (mine, other) in [(&ba, &bb), (&bb, &ba)] {
        mine.near((lo, hi), &mut in_region);
        triangles += in_region.len();
        for &i in &in_region {
            let t = &mine.tris[i];
            let (area, crosses) = inside_area(t, other, &mut near);
            crossed |= crosses;
            if area > 0.0 {
                volume += dot(sub(t.v[0], x0), t.n) * area / 3.0;
            }
        }
    }
    let volume = volume.max(0.0);
    // Touching, on the meshes as they are: a crossing, or a corner of one
    // within reach of the other's surface.
    let touching = crossed || volume > 0.0 || {
        let b0 = Bvh::new(b, [0.0; 3]);
        let close = |from: &Bvh, to: &Bvh, near: &mut Vec<usize>| {
            let mut found = false;
            let mut list = Vec::new();
            from.near((lo, hi), &mut list);
            'outer: for &i in &list {
                for p in from.tris[i].v {
                    let r = [TOUCH_MM; 3];
                    to.near((sub(p, r), add(p, r)), near);
                    if near.iter().any(|&j| distance(p, &to.tris[j]) <= TOUCH_MM) {
                        found = true;
                        break 'outer;
                    }
                }
            }
            found
        };
        close(&ba, &b0, &mut near) || close(&b0, &ba, &mut near)
    };
    Shared {
        volume,
        touching,
        triangles,
    }
}

/// A fingerprint of a mesh's triangles where they are, so an unchanged
/// pair is not measured twice.
#[must_use]
pub fn fingerprint(mesh: &Mesh) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    mesh.positions.len().hash(&mut h);
    for p in &mesh.positions {
        p.x.to_bits().hash(&mut h);
        p.y.to_bits().hash(&mut h);
        p.z.to_bits().hash(&mut h);
    }
    mesh.triangles.hash(&mut h);
    h.finish()
}

/// [`shared`], remembered by the two meshes' fingerprints while this
/// process runs, so an edit measures only the pairs it moved or changed.
/// Returns the result and whether it was remembered.
#[must_use]
pub fn shared_cached(a: &Mesh, fa: u64, b: &Mesh, fb: u64) -> (Shared, bool) {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static SEEN: OnceLock<Mutex<HashMap<(u64, u64), Shared>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(s) = seen.lock().ok().and_then(|m| m.get(&(fa, fb)).copied()) {
        return (s, true);
    }
    let s = shared(a, b);
    if let Ok(mut m) = seen.lock() {
        if m.len() > 50_000 {
            m.clear();
        }
        m.insert((fa, fb), s);
    }
    (s, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_engine::geom::Vec3;

    /// A prism: a polygon in XY (counter-clockwise), its cap split into the
    /// triangles given, from z0 to z1, with outward triangles.
    fn prism(poly: &[[f64; 2]], cap: &[[usize; 3]], z0: f64, z1: f64) -> Mesh {
        let n = poly.len();
        let mut m = Mesh::default();
        for z in [z0, z1] {
            for p in poly {
                m.positions.push(Vec3::new(p[0], p[1], z));
            }
        }
        for t in cap {
            // Bottom faces down (clockwise from above), top faces up.
            m.triangles.push([t[0] as u32, t[2] as u32, t[1] as u32]);
            m.triangles
                .push([(t[0] + n) as u32, (t[1] + n) as u32, (t[2] + n) as u32]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            let (a, b, c, d) = (i as u32, j as u32, (j + n) as u32, (i + n) as u32);
            m.triangles.push([a, b, c]);
            m.triangles.push([a, c, d]);
        }
        m
    }

    fn cuboid(lo: P, hi: P) -> Mesh {
        prism(
            &[
                [lo[0], lo[1]],
                [hi[0], lo[1]],
                [hi[0], hi[1]],
                [lo[0], hi[1]],
            ],
            &[[0, 1, 2], [0, 2, 3]],
            lo[2],
            hi[2],
        )
    }

    fn rotated(m: &Mesh, degrees: f64) -> Mesh {
        let (s, c) = degrees.to_radians().sin_cos();
        let mut out = m.clone();
        for p in &mut out.positions {
            *p = Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
        }
        out
    }

    #[test]
    fn boxes_overlap_by_their_shared_box_and_touching_faces_are_contact() {
        let a = cuboid([0.0; 3], [10.0, 10.0, 10.0]);
        let b = cuboid([6.0, 2.0, 3.0], [20.0, 20.0, 20.0]);
        let s = shared(&a, &b);
        assert!((s.volume - 4.0 * 8.0 * 7.0).abs() < 1e-3, "{s:?}");
        assert!(s.touching);
        // Resting on top: faces in one plane, opposite ways. Contact, no volume.
        let top = cuboid([2.0, 2.0, 10.0], [8.0, 8.0, 15.0]);
        let s = shared(&a, &top);
        assert!(s.volume < 1e-3, "{s:?}");
        assert!(s.touching);
        // Flush inside: a block in a corner of a bigger one shares faces facing the same way.
        let corner = cuboid([0.0; 3], [4.0, 5.0, 6.0]);
        let s = shared(&a, &corner);
        assert!((s.volume - 120.0).abs() < 1e-3, "{s:?}");
        // Side by side with a gap: clear.
        let apart = cuboid([10.5, 0.0, 0.0], [20.0, 10.0, 10.0]);
        let s = shared(&a, &apart);
        assert_eq!(s.volume, 0.0);
        assert!(!s.touching);
        // A turned box: two cubes, one turned 45° about the other's axis.
        let c = cuboid([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
        let s = shared(&c, &rotated(&c, 45.0));
        // The regular octagon of inradius 5: 8·5²·tan(22.5°), times 10.
        let want = 8.0 * 25.0 * (22.5f64).to_radians().tan() * 10.0;
        assert!((s.volume - want).abs() < 1e-4, "{} vs {want}", s.volume);
    }

    /// The area two convex polygons share (Sutherland–Hodgman), for checking.
    fn convex_overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
        let mut out: Vec<[f64; 2]> = a.to_vec();
        for i in 0..b.len() {
            let (p, q) = (b[i], b[(i + 1) % b.len()]);
            let inside =
                |x: [f64; 2]| (q[0] - p[0]) * (x[1] - p[1]) - (q[1] - p[1]) * (x[0] - p[0]) >= 0.0;
            let input = std::mem::take(&mut out);
            for k in 0..input.len() {
                let (s, e) = (input[k], input[(k + 1) % input.len()]);
                let cut = |s: [f64; 2], e: [f64; 2]| {
                    let (d1, d2) = (
                        (q[0] - p[0]) * (s[1] - p[1]) - (q[1] - p[1]) * (s[0] - p[0]),
                        (q[0] - p[0]) * (e[1] - p[1]) - (q[1] - p[1]) * (e[0] - p[0]),
                    );
                    let t = d1 / (d1 - d2);
                    [s[0] + (e[0] - s[0]) * t, s[1] + (e[1] - s[1]) * t]
                };
                match (inside(s), inside(e)) {
                    (true, true) => out.push(e),
                    (true, false) => out.push(cut(s, e)),
                    (false, true) => {
                        out.push(cut(s, e));
                        out.push(e);
                    }
                    _ => {}
                }
            }
        }
        (0..out.len())
            .map(|i| {
                let (p, q) = (out[i], out[(i + 1) % out.len()]);
                0.5 * (p[0] * q[1] - q[0] * p[1])
            })
            .sum()
    }

    #[test]
    fn faceted_cylinders_overlap_by_their_polygons() {
        let ngon = |n: usize, r: f64, cx: f64, cy: f64, turn: f64| -> Vec<[f64; 2]> {
            (0..n)
                .map(|i| {
                    let a = turn + i as f64 * std::f64::consts::TAU / n as f64;
                    [cx + r * a.cos(), cy + r * a.sin()]
                })
                .collect()
        };
        let fan = |n: usize| -> Vec<[usize; 3]> { (1..n - 1).map(|i| [0, i, i + 1]).collect() };
        for (dx, dy, turn) in [
            (7.0, 0.0, 0.0),
            (3.3, 4.1, 0.05),
            (19.5, 0.3, 0.2),
            (0.0, 0.0, 0.1),
        ] {
            let a = ngon(48, 10.0, 0.0, 0.0, 0.0);
            let b = ngon(40, 10.0, dx, dy, turn);
            let want = convex_overlap(&a, &b) * 12.0;
            let s = shared(
                &prism(&a, &fan(48), 0.0, 12.0),
                &prism(&b, &fan(40), 3.0, 15.0),
            );
            assert!(
                (s.volume - want * 9.0 / 12.0).abs() < 1e-4,
                "{dx} {dy}: {} vs {}",
                s.volume,
                want * 0.75
            );
        }
    }

    #[test]
    fn a_thin_sliver_the_grid_missed_is_caught() {
        // A 100 mm cube, and an L-shaped bracket whose top plate dips 0.2 mm
        // into the cube's top while its leg runs down beside it: the two
        // boxes share the whole cube, so the 28³ grid steps 3.6 mm in Z and
        // never lands in the 0.2 mm slab.
        let cube = cuboid([0.0; 3], [100.0, 100.0, 100.0]);
        // The L in XZ, extruded along Y: built as a prism in XY then turned
        // so the polygon's Y becomes Z.
        let l = prism(
            &[
                [0.0, 99.8],
                [101.0, 99.8],
                [101.0, 0.0],
                [111.0, 0.0],
                [111.0, 109.8],
                [0.0, 109.8],
            ],
            &[[0, 1, 4], [0, 4, 5], [1, 2, 3], [1, 3, 4]],
            0.0,
            100.0,
        );
        // (x, y, z) → (x, z, y) mirrors; reverse the triangles to keep them outward.
        let mut bracket = l.clone();
        for p in &mut bracket.positions {
            *p = Vec3::new(p.x, p.z, p.y);
        }
        for t in &mut bracket.triangles {
            t.swap(1, 2);
        }
        let s = shared(&cube, &bracket);
        // 100 × 100 × 0.2.
        assert!((s.volume - 2000.0).abs() < 1e-3, "{s:?}");
        assert_eq!(crate::overlap::volume(&cube, &bracket), 0.0);
        // Lifted to rest exactly on the cube: contact, not interference.
        let mut resting = bracket.clone();
        for p in &mut resting.positions {
            p.z += 0.2;
        }
        let s = shared(&cube, &resting);
        assert!(s.volume < 1e-2, "{s:?}");
        assert!(s.touching);
    }
}
