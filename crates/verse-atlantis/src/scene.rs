//! Retained Wizard Woods terrain with the source renderer's sampling convention.
//!
//! Height interpolation and normals are adapted from Ruins of Atlantis
//! `render_wgpu/src/gfx/terrain.rs` at the revision in `data/wizard_woods/provenance.json`.
//! The platform-specific upload and filesystem discovery are intentionally absent.

use glam::Vec3;
use serde::Deserialize;
use std::sync::OnceLock;

/// The authoritative source snapshot uses a 300-meter-wide, 129-by-129 grid.
#[derive(Debug, Deserialize)]
pub struct Terrain {
    size: usize,
    extent: f32,
    heights: Vec<f32>,
    #[serde(skip)]
    normals: Vec<[f32; 3]>,
}

impl Terrain {
    /// Parse the pinned, compile-time data once. This does not access the network.
    pub fn bundled() -> &'static Self {
        static TERRAIN: OnceLock<Terrain> = OnceLock::new();
        TERRAIN.get_or_init(|| {
            let mut terrain: Terrain =
                serde_json::from_str(include_str!("../data/wizard_woods/terrain.json"))
                    .expect("the retained Wizard Woods terrain must be valid JSON");
            assert_eq!(terrain.size, 129);
            assert_eq!(terrain.extent, 150.0);
            assert_eq!(terrain.heights.len(), terrain.size * terrain.size);
            assert!(terrain.heights.iter().all(|h| h.is_finite()));
            terrain.normals = terrain.compute_normals();
            terrain
        })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// World half-extent, in meters: this grid covers X and Z from -150 to +150.
    pub fn extent(&self) -> f32 {
        self.extent
    }

    /// Heights use row-major `z * size + x` order.
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// A mesh vertex in the same world coordinates used by the height sampler.
    pub fn vertex(&self, x: usize, z: usize) -> [f32; 3] {
        let x = x.min(self.size - 1);
        let z = z.min(self.size - 1);
        let step = 2.0 * self.extent / (self.size - 1) as f32;
        [
            -self.extent + x as f32 * step,
            self.heights[z * self.size + x],
            -self.extent + z as f32 * step,
        ]
    }

    pub fn normal(&self, x: usize, z: usize) -> [f32; 3] {
        self.normals[z.min(self.size - 1) * self.size + x.min(self.size - 1)]
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.sample(x, z).0
    }

    /// Bilinear interpolation, with out-of-bounds finite positions clamped to edges.
    /// Nonfinite coordinates return the center sample instead of indexing invalid data.
    pub fn sample(&self, x: f32, z: f32) -> (f32, [f32; 3]) {
        let x = if x.is_finite() { x } else { 0.0 };
        let z = if z.is_finite() { z } else { 0.0 };
        let n = self.size as i32;
        let gx =
            ((x.clamp(-self.extent, self.extent) / self.extent) * 0.5 + 0.5) * (n as f32 - 1.0);
        let gz =
            ((z.clamp(-self.extent, self.extent) / self.extent) * 0.5 + 0.5) * (n as f32 - 1.0);
        let x0 = gx.floor() as i32;
        let z0 = gz.floor() as i32;
        let x1 = (x0 + 1).clamp(0, n - 1);
        let z1 = (z0 + 1).clamp(0, n - 1);
        let tx = (gx - x0 as f32).clamp(0.0, 1.0);
        let tz = (gz - z0 as f32).clamp(0.0, 1.0);
        let idx = |x: i32, z: i32| z as usize * self.size + x as usize;
        let h00 = self.heights[idx(x0, z0)];
        let h10 = self.heights[idx(x1, z0)];
        let h01 = self.heights[idx(x0, z1)];
        let h11 = self.heights[idx(x1, z1)];
        let h0 = h00 * (1.0 - tx) + h10 * tx;
        let h1 = h01 * (1.0 - tx) + h11 * tx;
        let n00 = Vec3::from_array(self.normals[idx(x0, z0)]);
        let n10 = Vec3::from_array(self.normals[idx(x1, z0)]);
        let n01 = Vec3::from_array(self.normals[idx(x0, z1)]);
        let n11 = Vec3::from_array(self.normals[idx(x1, z1)]);
        let normal = n00.lerp(n10, tx).lerp(n01.lerp(n11, tx), tz).normalize();
        (h0 * (1.0 - tz) + h1 * tz, normal.to_array())
    }

    fn compute_normals(&self) -> Vec<[f32; 3]> {
        let step = 2.0 * self.extent / (self.size as f32 - 1.0);
        let idx = |x: isize, z: isize| {
            z.clamp(0, self.size as isize - 1) as usize * self.size
                + x.clamp(0, self.size as isize - 1) as usize
        };
        let mut normals = Vec::with_capacity(self.heights.len());
        for z in 0..self.size as isize {
            for x in 0..self.size as isize {
                let sx = (self.heights[idx(x + 1, z)] - self.heights[idx(x - 1, z)]) / (2.0 * step);
                let sz = (self.heights[idx(x, z + 1)] - self.heights[idx(x, z - 1)]) / (2.0 * step);
                normals.push(Vec3::new(-sx, 1.0, -sz).normalize().to_array());
            }
        }
        normals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_snapshot_has_original_dimensions_and_spawn_height() {
        let terrain = Terrain::bundled();
        assert_eq!(terrain.size(), 129);
        assert_eq!(terrain.extent(), 150.0);
        assert_eq!(terrain.height(0.0, 0.0), -3.302_828_6);
        assert_eq!(terrain.vertex(0, 0), [-150.0, -2.975_497_7, -150.0]);
        assert_eq!(terrain.heights().len(), 16_641);
    }

    #[test]
    fn every_mesh_vertex_matches_the_ground_sampler() {
        let terrain = Terrain::bundled();
        for z in 0..terrain.size() {
            for x in 0..terrain.size() {
                let p = terrain.vertex(x, z);
                assert!((terrain.height(p[0], p[2]) - p[1]).abs() < 1e-5);
                let n = Vec3::from_array(terrain.normal(x, z));
                assert!((n.length() - 1.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn finite_outside_samples_clamp_and_nonfinite_inputs_remain_finite() {
        let terrain = Terrain::bundled();
        assert_eq!(
            terrain.height(f32::MAX, -f32::MAX),
            terrain.height(150.0, -150.0)
        );
        assert_eq!(
            terrain.height(f32::NAN, f32::INFINITY),
            terrain.height(0.0, 0.0)
        );
        let a = terrain.vertex(10, 20);
        let b = terrain.vertex(11, 21);
        let expected = (a[1] + b[1] + terrain.vertex(11, 20)[1] + terrain.vertex(10, 21)[1]) / 4.0;
        assert!((terrain.height((a[0] + b[0]) / 2.0, (a[2] + b[2]) / 2.0) - expected).abs() < 1e-5);
    }
}
