use super::{Result, failed};
use glam::Vec3;
use std::collections::{HashMap, HashSet};
const RADIUS: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const CELL: f32 = 4.;
#[derive(Clone, Copy)]
struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
}
pub(super) struct Collision {
    triangles: Vec<Triangle>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    large: Vec<usize>,
}
impl Collision {
    pub fn new(values: &[f32]) -> Result<Self> {
        if values.len() % 9 != 0
            || values.len() > 500_000 * 9
            || values.iter().any(|v| !v.is_finite() || v.abs() > 30_000.)
        {
            return Err(failed("The town collision mesh is incomplete."));
        }
        let mut result = Self {
            triangles: Vec::new(),
            grid: HashMap::new(),
            large: Vec::new(),
        };
        for v in values.chunks_exact(9) {
            let t = Triangle {
                a: Vec3::from_slice(&v[0..3]),
                b: Vec3::from_slice(&v[3..6]),
                c: Vec3::from_slice(&v[6..9]),
            };
            if (t.b - t.a).cross(t.c - t.a).length_squared() < 1e-14 {
                continue;
            }
            let min = t.a.min(t.b).min(t.c);
            let max = t.a.max(t.b).max(t.c);
            let x0 = (min.x / CELL).floor() as i32;
            let x1 = (max.x / CELL).floor() as i32;
            let z0 = (min.z / CELL).floor() as i32;
            let z1 = (max.z / CELL).floor() as i32;
            let index = result.triangles.len();
            result.triangles.push(t);
            if (x1 - x0 + 1) as i64 * (z1 - z0 + 1) as i64 > 256 {
                result.large.push(index);
            } else {
                for x in x0..=x1 {
                    for z in z0..=z1 {
                        result.grid.entry((x, z)).or_default().push(index);
                    }
                }
            }
        }
        Ok(result)
    }
    fn nearby(&self, p: Vec3, radius: f32) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut ids = Vec::new();
        for x in ((p.x - radius) / CELL).floor() as i32..=((p.x + radius) / CELL).floor() as i32 {
            for z in ((p.z - radius) / CELL).floor() as i32..=((p.z + radius) / CELL).floor() as i32
            {
                if let Some(list) = self.grid.get(&(x, z)) {
                    for &i in list {
                        if seen.insert(i) {
                            ids.push(i);
                        }
                    }
                }
            }
        }
        for &i in &self.large {
            if seen.insert(i) {
                ids.push(i);
            }
        }
        ids
    }
    fn clear_among(&self, p: Vec3, triangles: &[usize]) -> bool {
        let a = p + Vec3::Y * RADIUS;
        let b = p + Vec3::Y * (HEIGHT - RADIUS);
        triangles.iter().all(|&i| {
            segment_triangle_distance2(a, b, self.triangles[i]) >= (RADIUS - 0.003).powi(2)
        })
    }
    fn clear(&self, p: Vec3) -> bool {
        self.clear_among(p, &self.nearby(p, RADIUS))
    }
    // A floor height describes a point. Rounded feet also contact nearby bevels
    // before the capsule axis reaches them; solve that contact without changing
    // the collision margin or allowing a step higher than the controller limit.
    fn supported_floor(&self, p: Vec3, rise: f32) -> Option<f32> {
        let mut low = self.floor(p, rise)?;
        let ids = self.nearby(p, RADIUS);
        let at = |height| self.clear_among(Vec3::new(p.x, height, p.z), &ids);
        if at(low) {
            return Some(low);
        }
        let mut high = (p.y + rise).min(low + RADIUS);
        if !at(high) {
            return None;
        }
        for _ in 0..10 {
            let middle = (low + high) * 0.5;
            if at(middle) {
                high = middle;
            } else {
                low = middle;
            }
        }
        Some(high)
    }
    fn floor(&self, p: Vec3, rise: f32) -> Option<f32> {
        let mut best = None;
        for offset in [
            Vec3::ZERO,
            Vec3::X * 0.16,
            -Vec3::X * 0.16,
            Vec3::Z * 0.16,
            -Vec3::Z * 0.16,
        ] {
            let sample = p + offset;
            for i in self.nearby(sample, 0.) {
                let t = self.triangles[i];
                let ab = t.b - t.a;
                let ac = t.c - t.a;
                let denominator = ab.x * ac.z - ab.z * ac.x;
                if denominator.abs() < 1e-8 {
                    continue;
                }
                let dx = sample.x - t.a.x;
                let dz = sample.z - t.a.z;
                let u = (dx * ac.z - dz * ac.x) / denominator;
                let v = (ab.x * dz - ab.z * dx) / denominator;
                if u < -0.0001 || v < -0.0001 || u + v > 1.0001 {
                    continue;
                }
                let normal = ab.cross(ac).normalize();
                if normal.y.abs() < 0.65 {
                    continue;
                }
                let y = t.a.y + ab.y * u + ac.y * v;
                if y <= p.y + rise && y > p.y - 60. && best.is_none_or(|last| y > last) {
                    best = Some(y);
                }
            }
        }
        best
    }
    pub fn move_capsule(&self, mut p: Vec3, delta: Vec3, dy: f32, grounded: &mut bool) -> Vec3 {
        let steps = (delta.length().max(dy.abs()) / 0.07).ceil().max(1.) as usize;
        let horizontal = delta / steps as f32;
        let vertical = dy / steps as f32;
        for _ in 0..steps {
            for movement in [
                horizontal,
                Vec3::new(horizontal.x, 0., 0.),
                Vec3::new(0., 0., horizontal.z),
            ] {
                if movement.length_squared() < 1e-12 {
                    continue;
                }
                let next = p + movement;
                if self.clear(next) {
                    p = next;
                    if movement == horizontal {
                        break;
                    }
                } else if *grounded {
                    if let Some(floor) = self.supported_floor(next, 0.27) {
                        let step = Vec3::new(next.x, floor, next.z);
                        if floor >= p.y - 0.1 && floor <= p.y + 0.27 && self.clear(step) {
                            p = step;
                            if movement == horizontal {
                                break;
                            }
                        }
                    }
                }
            }
            let next = p + Vec3::Y * vertical;
            if vertical <= 0. {
                if let Some(floor) = self.supported_floor(p, 0.035) {
                    if next.y <= floor + 0.003 {
                        p.y = floor;
                        *grounded = true;
                        continue;
                    }
                }
            }
            *grounded = false;
            if self.clear(next) {
                p = next;
            }
        }
        p
    }
}
fn point_triangle(p: Vec3, t: Triangle) -> Vec3 {
    let ab = t.b - t.a;
    let ac = t.c - t.a;
    let ap = p - t.a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0. && d2 <= 0. {
        return t.a;
    }
    let bp = p - t.b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0. && d4 <= d3 {
        return t.b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return t.a + ab * (d1 / (d1 - d3));
    }
    let cp = p - t.c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0. && d5 <= d6 {
        return t.c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return t.a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && (d4 - d3) >= 0. && (d5 - d6) >= 0. {
        return t.b + (t.c - t.b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let inverse = 1. / (va + vb + vc);
    t.a + ab * (vb * inverse) + ac * (vc * inverse)
}
fn segments(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f32 {
    let u = b - a;
    let v = d - c;
    let w = a - c;
    let aa = u.dot(u);
    let bb = u.dot(v);
    let cc = v.dot(v);
    let dd = u.dot(w);
    let ee = v.dot(w);
    let denominator = aa * cc - bb * bb;
    let mut s = if denominator > 1e-10 {
        ((bb * ee - cc * dd) / denominator).clamp(0., 1.)
    } else {
        0.
    };
    let mut t = if cc > 1e-10 { (bb * s + ee) / cc } else { 0. };
    if t < 0. {
        t = 0.;
        s = (-dd / aa.max(1e-10)).clamp(0., 1.);
    } else if t > 1. {
        t = 1.;
        s = ((bb - dd) / aa.max(1e-10)).clamp(0., 1.);
    }
    (a + u * s - c - v * t).length_squared()
}
fn segment_triangle_distance2(a: Vec3, b: Vec3, t: Triangle) -> f32 {
    let direction = b - a;
    let edge1 = t.b - t.a;
    let edge2 = t.c - t.a;
    let h = direction.cross(edge2);
    let determinant = edge1.dot(h);
    if determinant.abs() > 1e-8 {
        let inverse = 1. / determinant;
        let s = a - t.a;
        let u = inverse * s.dot(h);
        let q = s.cross(edge1);
        let v = inverse * direction.dot(q);
        let distance = inverse * edge2.dot(q);
        if u >= 0. && v >= 0. && u + v <= 1. && distance >= 0. && distance <= 1. {
            return 0.;
        }
    }
    (a - point_triangle(a, t))
        .length_squared()
        .min((b - point_triangle(b, t)).length_squared())
        .min(segments(a, b, t.a, t.b))
        .min(segments(a, b, t.b, t.c))
        .min(segments(a, b, t.c, t.a))
}
