//! Plain 2D geometry on the floor plane (x right, z toward the viewer):
//! vectors, boxes, circle-against-box sliding and segment tests. Height
//! (y) is only a number on each box.

use std::ops::{Add, Mul, Neg, Sub};

/// A point or direction on the floor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub z: f32,
}

/// A point, for tables.
#[must_use]
pub const fn v(x: f32, z: f32) -> V2 {
    V2 { x, z }
}

impl V2 {
    pub const ZERO: Self = v(0.0, 0.0);

    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.z * other.z
    }

    #[must_use]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// The unit vector, or zero for a tiny one.
    #[must_use]
    pub fn normalized(self) -> Self {
        let length = self.length();
        if length < 1e-6 {
            Self::ZERO
        } else {
            self * (1.0 / length)
        }
    }

    /// Shortened to at most `max`.
    #[must_use]
    pub fn clamped(self, max: f32) -> Self {
        let length = self.length();
        if length > max {
            self * (max / length)
        } else {
            self
        }
    }

    /// Turned by an angle given as its cosine and sine.
    #[must_use]
    pub fn rotated(self, cos: f32, sin: f32) -> Self {
        v(self.x * cos - self.z * sin, self.x * sin + self.z * cos)
    }
}

impl Add for V2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        v(self.x + o.x, self.z + o.z)
    }
}

impl Sub for V2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        v(self.x - o.x, self.z - o.z)
    }
}

impl Mul<f32> for V2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        v(self.x * s, self.z * s)
    }
}

impl Neg for V2 {
    type Output = Self;
    fn neg(self) -> Self {
        v(-self.x, -self.z)
    }
}

/// A solid box on the floor, `height` tall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub min: V2,
    pub max: V2,
    pub height: f32,
}

impl Block {
    #[must_use]
    pub fn contains(&self, p: V2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.z >= self.min.z && p.z <= self.max.z
    }

    /// The box grown by `r` on every side.
    #[must_use]
    pub fn grown(&self, r: f32) -> Self {
        Self {
            min: self.min - v(r, r),
            max: self.max + v(r, r),
            height: self.height,
        }
    }

    /// The closest point of the box to `p`.
    #[must_use]
    pub fn closest(&self, p: V2) -> V2 {
        v(
            p.x.clamp(self.min.x, self.max.x),
            p.z.clamp(self.min.z, self.max.z),
        )
    }

    /// Where along `a` to `b` (0 to 1) the segment first enters the box,
    /// if it does.
    #[must_use]
    pub fn segment(&self, a: V2, b: V2) -> Option<f32> {
        let d = b - a;
        let mut low = 0.0_f32;
        let mut high = 1.0_f32;
        for (start, delta, min, max) in [
            (a.x, d.x, self.min.x, self.max.x),
            (a.z, d.z, self.min.z, self.max.z),
        ] {
            if delta.abs() < 1e-9 {
                if start < min || start > max {
                    return None;
                }
            } else {
                let (mut t0, mut t1) = ((min - start) / delta, (max - start) / delta);
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                }
                low = low.max(t0);
                high = high.min(t1);
                if low > high {
                    return None;
                }
            }
        }
        Some(low)
    }
}

/// Moves a circle of `radius` from `from` by `delta`, sliding along the
/// blocks taller than `feet`. Steps are short (under the wall thickness
/// plus the radius), so nothing tunnels.
#[must_use]
pub fn slide(blocks: &[Block], from: V2, delta: V2, radius: f32, feet: f32) -> V2 {
    let mut p = from + delta;
    for _ in 0..3 {
        for block in blocks.iter().filter(|b| b.height > feet) {
            let c = block.closest(p);
            let d = p - c;
            let gap = d.dot(d);
            if gap >= radius * radius {
                continue;
            }
            if gap > 1e-10 {
                p = c + d.normalized() * radius;
            } else {
                // The centre is inside: leave by the nearest side.
                let pushes = [
                    (p.x - block.min.x + radius, v(-1.0, 0.0)),
                    (block.max.x - p.x + radius, v(1.0, 0.0)),
                    (p.z - block.min.z + radius, v(0.0, -1.0)),
                    (block.max.z - p.z + radius, v(0.0, 1.0)),
                ];
                let mut best = pushes[0];
                for push in pushes {
                    if push.0 < best.0 {
                        best = push;
                    }
                }
                p = p + best.1 * best.0;
            }
        }
    }
    p
}

/// Where along `a` to `b` (0 to 1) the first block at least `min_height`
/// tall, grown by `radius`, is hit.
#[must_use]
pub fn first_hit(blocks: &[Block], a: V2, b: V2, radius: f32, min_height: f32) -> Option<f32> {
    blocks
        .iter()
        .filter(|block| block.height >= min_height)
        .filter_map(|block| block.grown(radius).segment(a, b))
        .fold(None, |best: Option<f32>, t| {
            Some(best.map_or(t, |b| b.min(t)))
        })
}

/// The shortest distance from `p` to the segment `a` to `b`.
#[must_use]
pub fn segment_distance(a: V2, b: V2, p: V2) -> f32 {
    let d = b - a;
    let length = d.dot(d);
    let t = if length < 1e-12 {
        0.0
    } else {
        ((p - a).dot(d) / length).clamp(0.0, 1.0)
    };
    (a + d * t).distance(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> Block {
        Block {
            min: v(0.0, -50.0),
            max: v(0.3, 50.0),
            height: 3.0,
        }
    }

    #[test]
    fn a_circle_slides_along_a_wall_and_never_passes_it() {
        let mut p = v(-1.0, 0.0);
        for _ in 0..100 {
            p = slide(&[wall()], p, v(0.25, 0.1), 0.35, 0.0);
            assert!(p.x <= -0.349, "{p:?}");
        }
        assert!(p.z > 9.0, "it slid along: {p:?}");
    }

    #[test]
    fn a_low_block_is_stepped_over_from_above() {
        let low = Block {
            min: v(0.0, -1.0),
            max: v(1.0, 1.0),
            height: 0.5,
        };
        assert!((slide(&[low], v(-1.0, 0.0), v(0.9, 0.0), 0.35, 0.6).x + 0.1).abs() < 1e-5);
        assert!(slide(&[low], v(-1.0, 0.0), v(0.9, 0.0), 0.35, 0.0).x <= -0.349);
    }

    #[test]
    fn segments_find_the_first_block() {
        let blocks = [wall()];
        assert!(first_hit(&blocks, v(-2.0, 0.0), v(2.0, 0.0), 0.0, 1.0).is_some());
        assert!(first_hit(&blocks, v(-2.0, 0.0), v(-1.0, 0.0), 0.0, 1.0).is_none());
        assert!(first_hit(&blocks, v(-2.0, 0.0), v(2.0, 0.0), 0.0, 3.5).is_none());
        let t = first_hit(&blocks, v(-1.0, 0.0), v(1.0, 0.0), 0.0, 0.0).unwrap();
        assert!((t - 0.5).abs() < 1e-5);
        assert!((segment_distance(v(0.0, 0.0), v(10.0, 0.0), v(5.0, 2.0)) - 2.0).abs() < 1e-5);
    }
}
