//! Where every slot of the tree stands: a cone that grows up like a tree,
//! with the root at its base and the members in a wide band at its crown.
//!
//! Each slot keeps its place for the whole run, so a join, a payment, or an
//! exit changes a few balances and never moves the rest of the tree. A level
//! is a ring, twisted a little from the level above it, and children sit on
//! either side of their parent's angle, so every link points up and out.

use glam::Vec3;

/// The tree's shape for one depth.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub depth: u32,
    /// The members' ring radius.
    pub radius: f32,
    /// The root's height above the floor.
    pub root: f32,
    /// The members' height above the floor.
    pub crown: f32,
}

impl Shape {
    #[must_use]
    pub fn new(depth: u32) -> Self {
        let d = depth as f32;
        Self {
            depth,
            radius: 6.0 + d * d * 0.15,
            root: 3.6,
            crown: 6.0 + d * 1.5,
        }
    }

    /// The position of the node at `height` (0 for members) and `index`.
    #[must_use]
    pub fn position(&self, height: u32, index: u64) -> Vec3 {
        let level = self.depth.saturating_sub(height);
        if level == 0 {
            return Vec3::new(0.0, self.root, 0.0);
        }
        let share = level as f32 / self.depth as f32;
        let count = (1u64 << level) as f32;
        let angle = std::f32::consts::TAU * (index as f32 + 0.5) / count + level as f32 * 0.07;
        let mut radius = self.radius * share.powf(1.25);
        let mut y = self.root + (self.crown - self.root) * share.powf(0.9);
        if height == 0 && self.depth >= 8 {
            // Thousands of members: four rings deep, so neighbours separate.
            let band = (index % 4) as f32 - 1.5;
            radius += band * 0.9;
            y -= band * 0.18;
        }
        Vec3::new(angle.cos() * radius, y, angle.sin() * radius)
    }

    /// The path from a member down to the root, one point per level.
    #[must_use]
    pub fn path(&self, pos: u64) -> Vec<Vec3> {
        (0..=self.depth)
            .map(|h| self.position(h, pos >> h))
            .collect()
    }

    /// The point `t` (0 to 1) of the way along `path`.
    #[must_use]
    pub fn along(path: &[Vec3], t: f32) -> Vec3 {
        if path.len() < 2 {
            return path.first().copied().unwrap_or(Vec3::ZERO);
        }
        let span = t.clamp(0.0, 1.0) * (path.len() - 1) as f32;
        let i = (span.floor() as usize).min(path.len() - 2);
        path[i].lerp(path[i + 1], span - i as f32)
    }

    /// The shared Bitcoin output, at the foot of the trunk below the root.
    #[must_use]
    pub fn coin(&self) -> Vec3 {
        Vec3::new(0.0, self.root - 2.3, 0.0)
    }

    /// The signer, standing beside the tree.
    #[must_use]
    pub fn signer(&self) -> Vec3 {
        Vec3::new(-self.radius - 7.0, self.root + 1.5, -self.radius * 0.35)
    }

    /// Where the `i`th of `n` light clients stands, turning slowly with
    /// `time`.
    #[must_use]
    pub fn follower(&self, i: usize, n: usize, time: f32) -> Vec3 {
        let angle = std::f32::consts::TAU * i as f32 / n.max(1) as f32 + time * 0.03;
        let r = self.radius + 6.0;
        Vec3::new(angle.cos() * r, 0.9, angle.sin() * r)
    }

    /// Where the `i`th newest of the recent blocks sits, on an arc behind
    /// the tree.
    #[must_use]
    pub fn block(&self, i: usize) -> Vec3 {
        let angle = -std::f32::consts::FRAC_PI_2 - 0.5 + i as f32 * 0.09;
        let r = self.radius + 11.0;
        Vec3::new(angle.cos() * r, 0.6, angle.sin() * r)
    }

    /// Where a member's payout lands on the floor, below and outside its
    /// slot at the crown.
    #[must_use]
    pub fn landing(&self, pos: u64) -> Vec3 {
        let p = self.position(0, pos);
        let out = Vec3::new(p.x, 0.0, p.z).normalize_or_zero() * 3.5;
        Vec3::new(p.x + out.x, 0.25, p.z + out.z)
    }

    /// Where the `i`th of `n` unroll transactions at `level` hangs.
    #[must_use]
    pub fn package(&self, level: u32, i: usize, n: usize) -> Vec3 {
        if n <= 1 {
            return Vec3::new(0.0, self.root + 1.6, 0.0);
        }
        let share = (level as f32 + 1.0) / 3.5;
        let angle = std::f32::consts::TAU * (i as f32 + 0.5) / n as f32;
        let r = self.radius * share;
        let y = self.root + (self.crown - self.root) * share;
        Vec3::new(angle.cos() * r, y, angle.sin() * r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_at_the_base_and_children_grow_above_their_parents() {
        for depth in [4, 8, 12, 16] {
            let shape = Shape::new(depth);
            assert_eq!(shape.position(depth, 0), Vec3::new(0.0, shape.root, 0.0));
            assert!(shape.coin().y < shape.root && shape.coin().y > 0.0);
            for pos in [0u64, 1, (1 << depth) / 3, (1 << depth) - 1] {
                let path = shape.path(pos);
                assert_eq!(path.len(), depth as usize + 1);
                for pair in path.windows(2) {
                    assert!(pair[0].y > pair[1].y, "depth {depth} pos {pos}");
                }
                assert!(path.iter().all(|p| p.is_finite()));
            }
        }
    }

    #[test]
    fn members_stay_apart_and_inside_the_floor() {
        let shape = Shape::new(12);
        let mut closest = f32::MAX;
        for pos in 0..4096u64 {
            let p = shape.position(0, pos);
            let q = shape.position(0, (pos + 1) % 4096);
            closest = closest.min(p.distance(q));
            assert!(Vec3::new(p.x, 0.0, p.z).length() < shape.radius + 2.0);
        }
        assert!(closest > 0.5, "{closest}");
    }

    #[test]
    fn along_walks_the_path_end_to_end() {
        let shape = Shape::new(6);
        let path = shape.path(17);
        assert_eq!(Shape::along(&path, 0.0), path[0]);
        assert_eq!(Shape::along(&path, 1.0), *path.last().unwrap());
        assert!(Shape::along(&path, 0.5).y < path[0].y);
    }
}
