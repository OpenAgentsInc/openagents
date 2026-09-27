//! Interest management scaffolding (who gets what data).
//!
//! Initial implementation: simple spherical interest around a point
//! (e.g., the client camera). Keep it dependency-light and easy to test.

/// Interest providers decide whether to include an item for a given client.
pub trait InterestProvider<T> {
    fn in_interest(&self, item: &T) -> bool;
}

/// Types that can expose a point in world space for interest testing.
pub trait HasPoint {
    fn point(&self) -> [f32; 3];
}

/// Spherical interest volume in world coordinates.
#[derive(Clone, Copy, Debug)]
pub struct SphereInterest {
    pub center: [f32; 3],
    pub radius: f32,
}

impl<T: HasPoint> InterestProvider<T> for SphereInterest {
    fn in_interest(&self, item: &T) -> bool {
        let p = item.point();
        let dx = p[0] - self.center[0];
        let dy = p[1] - self.center[1];
        let dz = p[2] - self.center[2];
        dx * dx + dy * dy + dz * dz <= self.radius * self.radius
    }
}

/// Helper: compute a chunk's approximate world-space center given the chunk
/// origin and voxel size (meters per voxel).
#[inline]
#[must_use]
pub fn chunk_center_world(
    origin_m: [f32; 3],
    voxel_m: f32,
    chunk: (u32, u32, u32),
    chunk_dim: u32,
) -> [f32; 3] {
    #[allow(clippy::cast_precision_loss)]
    {
        let size = (chunk_dim as f32) * voxel_m;
        let half = size * 0.5;
        [
            origin_m[0] + (chunk.0 as f32) * size + half,
            origin_m[1] + (chunk.1 as f32) * size + half,
            origin_m[2] + (chunk.2 as f32) * size + half,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Pt([f32; 3]);
    impl HasPoint for Pt {
        fn point(&self) -> [f32; 3] {
            self.0
        }
    }

    #[test]
    fn sphere_interest_includes_points_within_radius() {
        let s = SphereInterest {
            center: [0.0, 0.0, 0.0],
            radius: 5.0,
        };
        assert!(s.in_interest(&Pt([3.0, 0.0, 0.0])));
        assert!(!s.in_interest(&Pt([6.0, 0.0, 0.0])));
    }

    #[test]
    fn chunk_center_helper_is_reasonable() {
        let c = chunk_center_world([0.0, 0.0, 0.0], 0.25, (1, 2, 3), 32);
        // Center should be at origin + chunk * size + half extent
        let size = 32.0 * 0.25;
        let half = size * 0.5;
        assert!((c[0] - (1.0 * size + half)).abs() < 1e-5);
        assert!((c[1] - (2.0 * size + half)).abs() < 1e-5);
        assert!((c[2] - (3.0 * size + half)).abs() < 1e-5);
    }
}
