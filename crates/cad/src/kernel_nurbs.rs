use serde::{Deserialize, Serialize};

use crate::kernel_math::Point3;
use crate::{CadError, CadResult};

const KNOT_EPS: f64 = 1e-12;

fn validate_knots(knots: &[f64], n_points: usize, degree: usize) -> bool {
    if n_points == 0 || degree >= n_points {
        return false;
    }
    if knots.len() != n_points + degree + 1 {
        return false;
    }
    for window in knots.windows(2) {
        if window[1] + KNOT_EPS < window[0] {
            return false;
        }
    }
    true
}

fn find_span(knots: &[f64], n: usize, degree: usize, t: f64) -> usize {
    if t >= knots[n + 1] {
        return n;
    }
    if t <= knots[degree] {
        return degree;
    }

    let mut low = degree;
    let mut high = n + 1;
    let mut mid = (low + high) / 2;
    while t < knots[mid] || t >= knots[mid + 1] {
        if t < knots[mid] {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }
    mid
}

fn basis_functions(knots: &[f64], span: usize, degree: usize, t: f64) -> Vec<f64> {
    let mut n = vec![0.0; degree + 1];
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    n[0] = 1.0;

    for j in 1..=degree {
        left[j] = t - knots[span + 1 - j];
        right[j] = knots[span + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            if denom.abs() <= KNOT_EPS {
                n[j] = saved;
                continue;
            }
            let temp = n[r] / denom;
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }

    n
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BSplineCurve {
    pub control_points: Vec<Point3>,
    pub knots: Vec<f64>,
    pub degree: usize,
}

impl BSplineCurve {
    pub fn new(control_points: Vec<Point3>, knots: Vec<f64>, degree: usize) -> CadResult<Self> {
        if !validate_knots(&knots, control_points.len(), degree) {
            return Err(CadError::InvalidParameter {
                name: "knots".to_string(),
                reason: format!(
                    "invalid knot vector for n_points={} degree={} len={}",
                    control_points.len(),
                    degree,
                    knots.len()
                ),
            });
        }
        Ok(Self {
            control_points,
            knots,
            degree,
        })
    }

    pub fn clamped_uniform(control_points: Vec<Point3>, degree: usize) -> CadResult<Self> {
        if control_points.len() <= degree {
            return Err(CadError::InvalidParameter {
                name: "degree".to_string(),
                reason: "degree must be less than control point count".to_string(),
            });
        }
        let n = control_points.len();
        let m = n + degree + 1;
        let mut knots = vec![0.0; m];

        let n_internal = m.saturating_sub(2 * (degree + 1));
        for i in 0..=degree {
            knots[i] = 0.0;
            knots[m - 1 - i] = 1.0;
        }
        for i in 1..=n_internal {
            knots[degree + i] = i as f64 / (n_internal + 1) as f64;
        }

        Self::new(control_points, knots, degree)
    }

    pub fn evaluate(&self, t: f64) -> Point3 {
        let n = self.control_points.len() - 1;
        let t = t.clamp(self.knots[self.degree], self.knots[n + 1]);
        let span = find_span(&self.knots, n, self.degree, t);
        let basis = basis_functions(&self.knots, span, self.degree, t);

        let mut point = Point3::origin();
        for (i, &b) in basis.iter().enumerate() {
            let idx = span - self.degree + i;
            let cp = self.control_points[idx];
            point.x += b * cp.x;
            point.y += b * cp.y;
            point.z += b * cp.z;
        }
        point
    }

    pub fn parameter_domain(&self) -> (f64, f64) {
        (
            self.knots[self.degree],
            self.knots[self.control_points.len()],
        )
    }

    pub fn insert_knot(&self, t: f64) -> CadResult<Self> {
        let n = self.control_points.len() - 1;
        let p = self.degree;
        let span = find_span(&self.knots, n, p, t);

        let mut new_knots = Vec::with_capacity(self.knots.len() + 1);
        new_knots.extend_from_slice(&self.knots[..=span]);
        new_knots.push(t);
        new_knots.extend_from_slice(&self.knots[span + 1..]);

        let mut new_points = Vec::with_capacity(self.control_points.len() + 1);
        for i in 0..=(span.saturating_sub(p)) {
            new_points.push(self.control_points[i]);
        }

        for i in (span - p + 1)..=span {
            let denom = self.knots[i + p] - self.knots[i];
            let alpha = if denom.abs() <= KNOT_EPS {
                0.0
            } else {
                (t - self.knots[i]) / denom
            };
            let prev = self.control_points[i - 1];
            let curr = self.control_points[i];
            new_points.push(Point3::new(
                (1.0 - alpha) * prev.x + alpha * curr.x,
                (1.0 - alpha) * prev.y + alpha * curr.y,
                (1.0 - alpha) * prev.z + alpha * curr.z,
            ));
        }

        for i in span..=n {
            new_points.push(self.control_points[i]);
        }

        Self::new(new_points, new_knots, p)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WeightedPoint {
    pub point: Point3,
    pub weight: f64,
}

impl WeightedPoint {
    pub fn new(point: Point3, weight: f64) -> CadResult<Self> {
        if !weight.is_finite() || weight <= 0.0 {
            return Err(CadError::InvalidParameter {
                name: "weight".to_string(),
                reason: "NURBS weight must be finite and positive".to_string(),
            });
        }
        Ok(Self { point, weight })
    }

    pub fn unweighted(point: Point3) -> Self {
        Self { point, weight: 1.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NurbsCurve {
    pub control_points: Vec<WeightedPoint>,
    pub knots: Vec<f64>,
    pub degree: usize,
}

impl NurbsCurve {
    pub fn new(
        control_points: Vec<WeightedPoint>,
        knots: Vec<f64>,
        degree: usize,
    ) -> CadResult<Self> {
        if !validate_knots(&knots, control_points.len(), degree) {
            return Err(CadError::InvalidParameter {
                name: "knots".to_string(),
                reason: format!(
                    "invalid NURBS knot vector for n_points={} degree={} len={}",
                    control_points.len(),
                    degree,
                    knots.len()
                ),
            });
        }
        Ok(Self {
            control_points,
            knots,
            degree,
        })
    }

    pub fn evaluate(&self, t: f64) -> Point3 {
        let n = self.control_points.len() - 1;
        let t = t.clamp(self.knots[self.degree], self.knots[n + 1]);
        let span = find_span(&self.knots, n, self.degree, t);
        let basis = basis_functions(&self.knots, span, self.degree, t);

        let mut numerator = Point3::origin();
        let mut denominator = 0.0;
        for (i, &b) in basis.iter().enumerate() {
            let idx = span - self.degree + i;
            let cp = self.control_points[idx];
            let w = b * cp.weight;
            numerator.x += w * cp.point.x;
            numerator.y += w * cp.point.y;
            numerator.z += w * cp.point.z;
            denominator += w;
        }

        if denominator.abs() <= KNOT_EPS {
            return Point3::origin();
        }
        Point3::new(
            numerator.x / denominator,
            numerator.y / denominator,
            numerator.z / denominator,
        )
    }

    pub fn parameter_domain(&self) -> (f64, f64) {
        (
            self.knots[self.degree],
            self.knots[self.control_points.len()],
        )
    }

    pub fn circle(center: Point3, radius: f64) -> CadResult<Self> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err(CadError::InvalidParameter {
                name: "radius".to_string(),
                reason: "circle radius must be finite and positive".to_string(),
            });
        }

        let w = std::f64::consts::FRAC_1_SQRT_2;
        let control_points = vec![
            WeightedPoint::new(Point3::new(center.x + radius, center.y, center.z), 1.0)?,
            WeightedPoint::new(
                Point3::new(center.x + radius, center.y + radius, center.z),
                w,
            )?,
            WeightedPoint::new(Point3::new(center.x, center.y + radius, center.z), 1.0)?,
            WeightedPoint::new(
                Point3::new(center.x - radius, center.y + radius, center.z),
                w,
            )?,
            WeightedPoint::new(Point3::new(center.x - radius, center.y, center.z), 1.0)?,
            WeightedPoint::new(
                Point3::new(center.x - radius, center.y - radius, center.z),
                w,
            )?,
            WeightedPoint::new(Point3::new(center.x, center.y - radius, center.z), 1.0)?,
            WeightedPoint::new(
                Point3::new(center.x + radius, center.y - radius, center.z),
                w,
            )?,
            WeightedPoint::new(Point3::new(center.x + radius, center.y, center.z), 1.0)?,
        ];
        let knots = vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ];
        Self::new(control_points, knots, 2)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BSplineSurface {
    pub control_points: Vec<Point3>,
    pub n_u: usize,
    pub n_v: usize,
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
    pub degree_u: usize,
    pub degree_v: usize,
}

impl BSplineSurface {
    pub fn new(
        control_points: Vec<Point3>,
        n_u: usize,
        n_v: usize,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        degree_u: usize,
        degree_v: usize,
    ) -> CadResult<Self> {
        if control_points.len() != n_u * n_v {
            return Err(CadError::InvalidParameter {
                name: "control_points".to_string(),
                reason: format!(
                    "control point count {} does not match n_u*n_v {}",
                    control_points.len(),
                    n_u * n_v
                ),
            });
        }
        if !validate_knots(&knots_u, n_u, degree_u) || !validate_knots(&knots_v, n_v, degree_v) {
            return Err(CadError::InvalidParameter {
                name: "knots".to_string(),
                reason: "invalid surface knot vectors".to_string(),
            });
        }
        Ok(Self {
            control_points,
            n_u,
            n_v,
            knots_u,
            knots_v,
            degree_u,
            degree_v,
        })
    }

    pub fn evaluate(&self, u: f64, v: f64) -> Point3 {
        let nu = self.n_u - 1;
        let nv = self.n_v - 1;
        let u = u.clamp(self.knots_u[self.degree_u], self.knots_u[nu + 1]);
        let v = v.clamp(self.knots_v[self.degree_v], self.knots_v[nv + 1]);

        let span_u = find_span(&self.knots_u, nu, self.degree_u, u);
        let span_v = find_span(&self.knots_v, nv, self.degree_v, v);
        let basis_u = basis_functions(&self.knots_u, span_u, self.degree_u, u);
        let basis_v = basis_functions(&self.knots_v, span_v, self.degree_v, v);

        let mut point = Point3::origin();
        for (j, &bv) in basis_v.iter().enumerate() {
            let v_idx = span_v - self.degree_v + j;
            for (i, &bu) in basis_u.iter().enumerate() {
                let u_idx = span_u - self.degree_u + i;
                let cp = self.control_points[v_idx * self.n_u + u_idx];
                let b = bu * bv;
                point.x += b * cp.x;
                point.y += b * cp.y;
                point.z += b * cp.z;
            }
        }
        point
    }

    pub fn parameter_domain(&self) -> ((f64, f64), (f64, f64)) {
        (
            (self.knots_u[self.degree_u], self.knots_u[self.n_u]),
            (self.knots_v[self.degree_v], self.knots_v[self.n_v]),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NurbsSurface {
    pub control_points: Vec<WeightedPoint>,
    pub n_u: usize,
    pub n_v: usize,
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
    pub degree_u: usize,
    pub degree_v: usize,
}

impl NurbsSurface {
    pub fn new(
        control_points: Vec<WeightedPoint>,
        n_u: usize,
        n_v: usize,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        degree_u: usize,
        degree_v: usize,
    ) -> CadResult<Self> {
        if control_points.len() != n_u * n_v {
            return Err(CadError::InvalidParameter {
                name: "control_points".to_string(),
                reason: format!(
                    "control point count {} does not match n_u*n_v {}",
                    control_points.len(),
                    n_u * n_v
                ),
            });
        }
        if !validate_knots(&knots_u, n_u, degree_u) || !validate_knots(&knots_v, n_v, degree_v) {
            return Err(CadError::InvalidParameter {
                name: "knots".to_string(),
                reason: "invalid NURBS surface knot vectors".to_string(),
            });
        }
        Ok(Self {
            control_points,
            n_u,
            n_v,
            knots_u,
            knots_v,
            degree_u,
            degree_v,
        })
    }

    pub fn evaluate(&self, u: f64, v: f64) -> Point3 {
        let nu = self.n_u - 1;
        let nv = self.n_v - 1;
        let u = u.clamp(self.knots_u[self.degree_u], self.knots_u[nu + 1]);
        let v = v.clamp(self.knots_v[self.degree_v], self.knots_v[nv + 1]);

        let span_u = find_span(&self.knots_u, nu, self.degree_u, u);
        let span_v = find_span(&self.knots_v, nv, self.degree_v, v);
        let basis_u = basis_functions(&self.knots_u, span_u, self.degree_u, u);
        let basis_v = basis_functions(&self.knots_v, span_v, self.degree_v, v);

        let mut numerator = Point3::origin();
        let mut denominator = 0.0;
        for (j, &bv) in basis_v.iter().enumerate() {
            let v_idx = span_v - self.degree_v + j;
            for (i, &bu) in basis_u.iter().enumerate() {
                let u_idx = span_u - self.degree_u + i;
                let cp = self.control_points[v_idx * self.n_u + u_idx];
                let b = bu * bv * cp.weight;
                numerator.x += b * cp.point.x;
                numerator.y += b * cp.point.y;
                numerator.z += b * cp.point.z;
                denominator += b;
            }
        }

        if denominator.abs() <= KNOT_EPS {
            return Point3::origin();
        }
        Point3::new(
            numerator.x / denominator,
            numerator.y / denominator,
            numerator.z / denominator,
        )
    }

    pub fn parameter_domain(&self) -> ((f64, f64), (f64, f64)) {
        (
            (self.knots_u[self.degree_u], self.knots_u[self.n_u]),
            (self.knots_v[self.degree_v], self.knots_v[self.n_v]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BSplineCurve, BSplineSurface, NurbsCurve, NurbsSurface, WeightedPoint, basis_functions,
        find_span,
    };
    use crate::CadError;
    use crate::kernel_math::Point3;

    #[test]
    fn bspline_curve_degree_one_matches_line_interpolation() {
        let points = vec![Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 0.0, 0.0)];
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        let curve = BSplineCurve::new(points, knots, 1).expect("curve");
        let mid = curve.evaluate(0.5);
        assert!((mid.x - 5.0).abs() < 1e-9);
        assert!(mid.y.abs() < 1e-9);
        assert!(mid.z.abs() < 1e-9);
    }

    #[test]
    fn bspline_knot_insertion_preserves_evaluation() {
        let curve = BSplineCurve::clamped_uniform(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(5.0, 3.0, 0.0),
                Point3::new(10.0, 0.0, 0.0),
            ],
            2,
        )
        .expect("curve");
        let sample_before = curve.evaluate(0.33);
        let refined = curve.insert_knot(0.4).expect("refined");
        let sample_after = refined.evaluate(0.33);
        assert!((sample_before.x - sample_after.x).abs() < 1e-6);
        assert!((sample_before.y - sample_after.y).abs() < 1e-6);
        assert!((sample_before.z - sample_after.z).abs() < 1e-6);
    }

    #[test]
    fn nurbs_circle_stays_on_radius() {
        let circle = NurbsCurve::circle(Point3::origin(), 5.0).expect("circle");
        let p0 = circle.evaluate(0.0);
        let p25 = circle.evaluate(0.25);
        let r0 = (p0.x * p0.x + p0.y * p0.y).sqrt();
        let r25 = (p25.x * p25.x + p25.y * p25.y).sqrt();
        assert!((r0 - 5.0).abs() < 1e-6);
        assert!((r25 - 5.0).abs() < 1e-3);
    }

    #[test]
    fn bspline_surface_bilinear_patch_midpoint() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
        ];
        let surf = BSplineSurface::new(
            points,
            2,
            2,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            1,
            1,
        )
        .expect("surface");
        let mid = surf.evaluate(0.5, 0.5);
        assert!((mid.x - 5.0).abs() < 1e-9);
        assert!((mid.y - 5.0).abs() < 1e-9);
        assert!(mid.z.abs() < 1e-9);
    }

    #[test]
    fn nurbs_surface_with_unit_weights_matches_bspline_surface() {
        let points = vec![
            WeightedPoint::unweighted(Point3::new(0.0, 0.0, 0.0)),
            WeightedPoint::unweighted(Point3::new(10.0, 0.0, 0.0)),
            WeightedPoint::unweighted(Point3::new(0.0, 10.0, 0.0)),
            WeightedPoint::unweighted(Point3::new(10.0, 10.0, 0.0)),
        ];
        let surf = NurbsSurface::new(
            points,
            2,
            2,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            1,
            1,
        )
        .expect("surface");
        let mid = surf.evaluate(0.5, 0.5);
        assert!((mid.x - 5.0).abs() < 1e-9);
        assert!((mid.y - 5.0).abs() < 1e-9);
        assert!(mid.z.abs() < 1e-9);
    }

    #[test]
    fn invalid_knot_vector_maps_to_openagents_error_model() {
        let err = BSplineCurve::new(
            vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
            vec![0.0, 0.0, 1.0],
            1,
        )
        .expect_err("invalid knots");
        assert!(matches!(err, CadError::InvalidParameter { .. }));
    }

    #[test]
    fn span_and_basis_partition_of_unity() {
        let knots = vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0];
        let span = find_span(&knots, 3, 2, 0.25);
        assert_eq!(span, 2);
        let basis = basis_functions(&knots, span, 2, 0.25);
        let sum: f64 = basis.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }
}
