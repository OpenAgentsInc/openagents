//! How much two closed meshes overlap, estimated by sampling: the fallback
//! for pairs of solids the kernel can't intersect exactly.

use solidcraft_engine::geom::Mesh;

/// Triangles binned by their extent across the ray direction (Y and Z),
/// so an inside test along +X only meets the triangles in its bin.
struct Index<'a> {
    mesh: &'a Mesh,
    lo: [f64; 3],
    hi: [f64; 3],
    bins: Vec<Vec<usize>>,
}

const BINS: usize = 24;

impl<'a> Index<'a> {
    fn new(mesh: &'a Mesh) -> Self {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for p in &mesh.positions {
            for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[k] = lo[k].min(x);
                hi[k] = hi[k].max(x);
            }
        }
        let mut index = Index {
            mesh,
            lo,
            hi,
            bins: vec![Vec::new(); BINS * BINS],
        };
        for (t, tri) in mesh.triangles.iter().enumerate() {
            let ys = tri.map(|i| mesh.positions[i as usize].y);
            let zs = tri.map(|i| mesh.positions[i as usize].z);
            let (y0, y1) = (
                index.bin(1, ys.iter().copied().fold(f64::MAX, f64::min)),
                index.bin(1, ys.iter().copied().fold(f64::MIN, f64::max)),
            );
            let (z0, z1) = (
                index.bin(2, zs.iter().copied().fold(f64::MAX, f64::min)),
                index.bin(2, zs.iter().copied().fold(f64::MIN, f64::max)),
            );
            for y in y0..=y1 {
                for z in z0..=z1 {
                    index.bins[y * BINS + z].push(t);
                }
            }
        }
        index
    }

    fn bin(&self, axis: usize, x: f64) -> usize {
        let span = (self.hi[axis] - self.lo[axis]).max(1e-9);
        (((x - self.lo[axis]) / span * BINS as f64).floor().max(0.0) as usize).min(BINS - 1)
    }

    /// Whether `p` is inside: the number of crossings of a ray along +X
    /// (tilted a hair off the axes) is odd.
    fn inside(&self, p: [f64; 3]) -> bool {
        if (0..3).any(|k| p[k] < self.lo[k] || p[k] > self.hi[k]) {
            return false;
        }
        let d = [1.0, 1.3e-7, 2.9e-7];
        let mut crossings = 0;
        for &t in &self.bins[self.bin(1, p[1]) * BINS + self.bin(2, p[2])] {
            let tri = self.mesh.triangles[t];
            let v = |i: usize| {
                let q = self.mesh.positions[tri[i] as usize];
                [q.x, q.y, q.z]
            };
            if ray_hits(p, d, v(0), v(1), v(2)) {
                crossings += 1;
            }
        }
        crossings % 2 == 1
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Möller–Trumbore: does the ray from `o` along `d` cross triangle abc?
fn ray_hits(o: [f64; 3], d: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-14 {
        return false;
    }
    let s = sub(o, a);
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

/// Overlaps smaller than this (mm³) are touching, not interference.
pub const IGNORED_MM3: f64 = 1.0;

/// A mesh's box: (min, max).
#[must_use]
pub fn bounds(mesh: &Mesh) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in &mesh.positions {
        for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[k] = lo[k].min(x);
            hi[k] = hi[k].max(x);
        }
    }
    (lo, hi)
}

/// The estimated volume (mm³) inside both meshes: zero unless at least
/// three sample points of the shared box are inside both.
#[must_use]
pub fn volume(a: &Mesh, b: &Mesh) -> f64 {
    let (ia, ib) = (Index::new(a), Index::new(b));
    let lo: Vec<f64> = (0..3).map(|k| ia.lo[k].max(ib.lo[k])).collect();
    let hi: Vec<f64> = (0..3).map(|k| ia.hi[k].min(ib.hi[k])).collect();
    if (0..3).any(|k| hi[k] <= lo[k]) {
        return 0.0;
    }
    let n = 28usize;
    let step: Vec<f64> = (0..3).map(|k| (hi[k] - lo[k]) / n as f64).collect();
    let mut count = 0usize;
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let p = [
                    lo[0] + (i as f64 + 0.5) * step[0],
                    lo[1] + (j as f64 + 0.5) * step[1],
                    lo[2] + (k as f64 + 0.5) * step[2],
                ];
                if ia.inside(p) && ib.inside(p) {
                    count += 1;
                }
            }
        }
    }
    if count < 3 {
        return 0.0;
    }
    count as f64 * step[0] * step[1] * step[2]
}
