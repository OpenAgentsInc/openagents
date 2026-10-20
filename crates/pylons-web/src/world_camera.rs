//! Fixed Grid positions, projected by the camera that drew the current frame.

use crate::camera::{PILLAR_HEIGHT, PILLAR_WIDTH, Placed};

/// A stable place on the Grid floor. Camera motion and viewport size never
/// enter the placement: each ring holds twelve pylons around the plaza.
#[must_use]
pub fn anchor(slot: usize) -> [f64; 3] {
    let angle = (slot % 12) as f64 * std::f64::consts::TAU / 12.0;
    let radius = 18.0 + (slot / 12) as f64 * 12.0;
    [radius * angle.sin(), 0.0, -10.0 + radius * angle.cos()]
}

/// Verse's column-major view-projection matrix and viewport in CSS pixels.
#[derive(Clone, Copy, Debug)]
pub struct WorldCamera {
    matrix: [f64; 16],
    width: f64,
    height: f64,
}

impl WorldCamera {
    #[must_use]
    pub fn new(matrix: [f32; 16], width: f64, height: f64) -> Option<Self> {
        (matrix.iter().all(|v| v.is_finite())
            && width.is_finite()
            && height.is_finite()
            && width > 0.0
            && height > 0.0)
            .then_some(Self {
                matrix: matrix.map(f64::from),
                width,
                height,
            })
    }

    pub fn project(&self, point: [f64; 3]) -> Option<(f64, f64, f64)> {
        let clip: [f64; 4] = std::array::from_fn(|row| {
            self.matrix[row] * point[0]
                + self.matrix[4 + row] * point[1]
                + self.matrix[8 + row] * point[2]
                + self.matrix[12 + row]
        });
        // WGPU's clip depth is 0..w. Never mirror a point behind the eye
        // onto the front of the screen.
        if clip[3] <= 0.0 || clip[2] < 0.0 || clip[2] > clip[3] {
            return None;
        }
        Some((
            (clip[0] / clip[3] + 1.0) * self.width / 2.0,
            (1.0 - clip[1] / clip[3]) * self.height / 2.0,
            clip[3],
        ))
    }

    /// The same world anchor in this frame. The beam is a billboard; its
    /// foot is the projection of the fixed floor point, never a screen slot.
    #[must_use]
    pub fn place(&self, point: [f64; 3]) -> Option<Placed> {
        let (x, y, depth) = self.project(point)?;
        let (_, top, _) = self.project([point[0], point[1] + PILLAR_HEIGHT, point[2]])?;
        // Row zero gives the camera's right direction, scaled by its lens.
        let right = [self.matrix[0], self.matrix[4], self.matrix[8]];
        let length = right.iter().map(|v| v * v).sum::<f64>().sqrt();
        if length <= f64::EPSILON || y <= top {
            return None;
        }
        let edge = std::array::from_fn(|i| point[i] + right[i] / length * PILLAR_WIDTH);
        let (edge_x, _, _) = self.project(edge)?;
        let width = (edge_x - x).abs().clamp(2.0, 9.0);
        if x < -80.0 || x > self.width + 80.0 || y < 0.0 || top > self.height {
            return None;
        }
        Some(Placed {
            x,
            y,
            height: y - top,
            width,
            scale: (22.0 / depth).clamp(0.62, 1.0),
            depth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    fn camera(eye: Vec3, target: Vec3, width: f64, height: f64) -> (WorldCamera, Mat4) {
        let matrix = Mat4::perspective_rh(1.0, (width / height) as f32, 0.1, 2000.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y);
        (
            WorldCamera::new(matrix.to_cols_array(), width, height).unwrap(),
            matrix,
        )
    }

    #[test]
    fn the_foot_tracks_its_world_landmark_during_orbit_and_translation() {
        let point = anchor(0);
        let landmark = Vec3::from_array(point.map(|v| v as f32));
        let mut positions = Vec::new();
        for eye in [
            Vec3::new(0.0, 6.0, -22.0),
            Vec3::new(8.0, 6.0, -20.0),
            Vec3::new(4.0, 6.0, -22.0),
        ] {
            let (camera, matrix) = camera(eye, Vec3::new(0.0, 1.6, -10.0), 1200.0, 800.0);
            let placed = camera.place(point).unwrap();
            let ndc = matrix.project_point3(landmark);
            assert!((placed.x - f64::from((ndc.x + 1.0) * 600.0)).abs() < 0.001);
            assert!((placed.y - f64::from((1.0 - ndc.y) * 400.0)).abs() < 0.001);
            positions.push((placed.x, placed.y));
        }
        assert!(positions.windows(2).all(|p| (p[0].0 - p[1].0).abs() > 10.0));
        assert_eq!(point, anchor(0));
    }

    #[test]
    fn resize_changes_pixels_without_moving_the_anchor() {
        let point = anchor(1);
        let eye = Vec3::new(0.0, 6.0, -22.0);
        let target = Vec3::new(0.0, 1.6, -10.0);
        let full = camera(eye, target, 1200.0, 800.0).0.place(point).unwrap();
        let half = camera(eye, target, 600.0, 400.0).0.place(point).unwrap();
        assert!((full.x - half.x * 2.0).abs() < 0.001);
        assert!((full.y - half.y * 2.0).abs() < 0.001);
        assert!((full.height - half.height * 2.0).abs() < 0.001);
    }

    #[test]
    fn a_pylon_behind_the_camera_or_outside_clip_depth_is_hidden() {
        let (camera, _) = camera(Vec3::new(0.0, 2.0, 0.0), -Vec3::Z, 1200.0, 800.0);
        assert!(camera.place([0.0, 0.0, 10.0]).is_none());
        assert!(camera.place([0.0, 0.0, -3000.0]).is_none());
        assert!(WorldCamera::new([f32::NAN; 16], 1200.0, 800.0).is_none());
        assert!(WorldCamera::new([0.0; 16], 0.0, 800.0).is_none());
    }
}
