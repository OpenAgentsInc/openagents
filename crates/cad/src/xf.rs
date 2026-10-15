//! Rigid transforms: a 3×3 rotation and a translation, millimetres.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    /// Rows of the rotation.
    pub r: [[f64; 3]; 3],
    pub t: [f64; 3],
}

impl Xf {
    pub const IDENTITY: Xf = Xf {
        r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    #[must_use]
    pub fn translate(t: [f64; 3]) -> Xf {
        Xf { t, ..Xf::IDENTITY }
    }

    /// Rotation by `angle` radians about a unit `axis` through the origin.
    #[must_use]
    pub fn axis_angle(axis: [f64; 3], angle: f64) -> Xf {
        let [x, y, z] = axis;
        let (s, c) = angle.sin_cos();
        let k = 1.0 - c;
        Xf {
            r: [
                [c + x * x * k, x * y * k - z * s, x * z * k + y * s],
                [y * x * k + z * s, c + y * y * k, y * z * k - x * s],
                [z * x * k - y * s, z * y * k + x * s, c + z * z * k],
            ],
            t: [0.0; 3],
        }
    }

    /// Rotation by degrees about X, then Y, then Z (fixed axes).
    #[must_use]
    pub fn rotate_xyz(degrees: [f64; 3]) -> Xf {
        let rx = Xf::axis_angle([1.0, 0.0, 0.0], degrees[0].to_radians());
        let ry = Xf::axis_angle([0.0, 1.0, 0.0], degrees[1].to_radians());
        let rz = Xf::axis_angle([0.0, 0.0, 1.0], degrees[2].to_radians());
        rz.mul(&ry).mul(&rx)
    }

    /// `self` after `o`: (self · o)(p) = self(o(p)).
    #[must_use]
    pub fn mul(&self, o: &Xf) -> Xf {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|k| self.r[i][k] * o.r[k][j]).sum();
            }
        }
        let t = self.apply(o.t);
        Xf { r, t }
    }

    /// The inverse: rotation transposed, translation undone.
    #[must_use]
    pub fn inverse(&self) -> Xf {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = self.r[j][i];
            }
        }
        let back = Xf { r, t: [0.0; 3] }.rotate(self.t);
        Xf {
            r,
            t: [-back[0], -back[1], -back[2]],
        }
    }

    /// The rotation as URDF's roll, pitch and yaw (radians): R = Rz(yaw) ·
    /// Ry(pitch) · Rx(roll), about fixed axes.
    #[must_use]
    pub fn rpy(&self) -> [f64; 3] {
        let r = &self.r;
        let pitch = (-r[2][0]).clamp(-1.0, 1.0).asin();
        if r[2][0].abs() > 1.0 - 1e-9 {
            // Gimbal lock: roll and yaw share an axis; put it all in yaw.
            return [0.0, pitch, (-r[0][1]).atan2(r[1][1])];
        }
        [r[2][1].atan2(r[2][2]), pitch, r[1][0].atan2(r[0][0])]
    }

    #[must_use]
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let mut out = self.t;
        for (i, o) in out.iter_mut().enumerate() {
            *o += (0..3).map(|k| self.r[i][k] * p[k]).sum::<f64>();
        }
        out
    }

    #[must_use]
    pub fn rotate(&self, v: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (i, o) in out.iter_mut().enumerate() {
            *o = (0..3).map(|k| self.r[i][k] * v[k]).sum();
        }
        out
    }

    #[must_use]
    pub fn translation(&self) -> [f64; 3] {
        self.t
    }

    /// The rotation as a unit axis and an angle in radians (0 with +Z for
    /// none).
    #[must_use]
    pub fn axis_angle_of(r: &[[f64; 3]; 3]) -> ([f64; 3], f64) {
        let trace = r[0][0] + r[1][1] + r[2][2];
        let angle = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos();
        if angle < 1e-9 {
            return ([0.0, 0.0, 1.0], 0.0);
        }
        if (std::f64::consts::PI - angle) < 1e-6 {
            // Half a turn: the axis from the diagonal.
            let xx = ((r[0][0] + 1.0) / 2.0).max(0.0).sqrt();
            let yy = ((r[1][1] + 1.0) / 2.0).max(0.0).sqrt();
            let zz = ((r[2][2] + 1.0) / 2.0).max(0.0).sqrt();
            let mut axis = [xx, yy, zz];
            if xx >= yy && xx >= zz {
                axis[1] = yy.copysign(r[0][1]);
                axis[2] = zz.copysign(r[0][2]);
            } else if yy >= zz {
                axis[0] = xx.copysign(r[0][1]);
                axis[2] = zz.copysign(r[1][2]);
            } else {
                axis[0] = xx.copysign(r[0][2]);
                axis[1] = yy.copysign(r[1][2]);
            }
            let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
            return ([axis[0] / n, axis[1] / n, axis[2] / n], angle);
        }
        let s = 2.0 * angle.sin();
        (
            [
                (r[2][1] - r[1][2]) / s,
                (r[0][2] - r[2][0]) / s,
                (r[1][0] - r[0][1]) / s,
            ],
            angle,
        )
    }

    /// The rotation as degrees about X, then Y, then Z (fixed axes), the
    /// inverse of [`Xf::rotate_xyz`].
    #[must_use]
    pub fn to_xyz_degrees(&self) -> [f64; 3] {
        let r = &self.r;
        let sy = (-r[2][0]).clamp(-1.0, 1.0);
        let y = sy.asin();
        let (x, z) = if sy.abs() < 1.0 - 1e-12 {
            (r[2][1].atan2(r[2][2]), r[1][0].atan2(r[0][0]))
        } else {
            // Gimbal lock: Y is ±90°, so only X − Z (or X + Z) is fixed;
            // Z is taken as 0.
            ((-r[1][2]).atan2(r[1][1]), 0.0)
        };
        [x.to_degrees(), y.to_degrees(), z.to_degrees()]
    }

    #[must_use]
    pub fn rotation_axis_angle(&self) -> ([f64; 3], f64) {
        Xf::axis_angle_of(&self.r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    #[test]
    fn rotations_round_trip_through_axis_and_angle() {
        for degrees in [
            [0.0, 0.0, 90.0],
            [30.0, -45.0, 120.0],
            [180.0, 0.0, 0.0],
            [0.0, 180.0, 90.0],
        ] {
            let xf = Xf::rotate_xyz(degrees);
            let (axis, angle) = xf.rotation_axis_angle();
            let again = Xf::axis_angle(axis, angle);
            for p in [[1.0, 2.0, 3.0], [-4.0, 0.5, 2.0]] {
                assert!(close(xf.apply(p), again.apply(p)), "{degrees:?}");
            }
        }
        // And back to degrees, gimbal lock included.
        for degrees in [
            [10.0, 20.0, 30.0],
            [-150.0, 45.0, 170.0],
            [30.0, 90.0, 40.0],
            [30.0, -90.0, 40.0],
        ] {
            let xf = Xf::rotate_xyz(degrees);
            let again = Xf::rotate_xyz(xf.to_xyz_degrees());
            for p in [[1.0, 2.0, 3.0], [-4.0, 0.5, 2.0]] {
                assert!(close(xf.apply(p), again.apply(p)), "{degrees:?}");
            }
        }
        for degrees in [[30.0, -45.0, 120.0], [0.0, 90.0, 10.0], [-170.0, 20.0, 5.0]] {
            let xf = Xf::translate([1.0, -2.0, 3.0]).mul(&Xf::rotate_xyz(degrees));
            let back = xf.inverse().mul(&xf);
            assert!(
                close(back.apply([4.0, 5.0, 6.0]), [4.0, 5.0, 6.0]),
                "{degrees:?}"
            );
            let [roll, pitch, yaw] = xf.rpy();
            let again = Xf::rotate_xyz([roll, pitch, yaw].map(f64::to_degrees));
            for p in [[1.0, 2.0, 3.0], [-4.0, 0.5, 2.0]] {
                assert!(close(xf.rotate(p), again.rotate(p)), "{degrees:?}");
            }
        }
        let quarter = Xf::rotate_xyz([0.0, 0.0, 90.0]);
        assert!(close(quarter.apply([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        let moved = Xf::translate([10.0, 0.0, 0.0]).mul(&quarter);
        assert!(close(moved.apply([1.0, 0.0, 0.0]), [10.0, 1.0, 0.0]));
    }
}
