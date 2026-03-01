#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CadMeasurePoint3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl CadMeasurePoint3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

pub fn vector_between_points(start: CadMeasurePoint3, end: CadMeasurePoint3) -> CadMeasurePoint3 {
    CadMeasurePoint3::new(end.x - start.x, end.y - start.y, end.z - start.z)
}

pub fn distance_between_points(
    start: CadMeasurePoint3,
    end: CadMeasurePoint3,
    tolerance: f64,
) -> f64 {
    let delta = vector_between_points(start, end);
    let distance = vector_length(delta);
    if !distance.is_finite() {
        return 0.0;
    }
    if distance <= tolerance.max(0.0) {
        return 0.0;
    }
    distance
}

pub fn angle_between_vectors_deg(
    lhs: CadMeasurePoint3,
    rhs: CadMeasurePoint3,
    tolerance: f64,
) -> Option<f64> {
    let lhs_length = vector_length(lhs);
    let rhs_length = vector_length(rhs);
    let min_length = tolerance.max(1e-12);
    if lhs_length <= min_length || rhs_length <= min_length {
        return None;
    }
    let cosine = (dot(lhs, rhs) / (lhs_length * rhs_length)).clamp(-1.0, 1.0);
    Some(cosine.acos().to_degrees())
}

fn dot(lhs: CadMeasurePoint3, rhs: CadMeasurePoint3) -> f64 {
    lhs.x * rhs.x + lhs.y * rhs.y + lhs.z * rhs.z
}

fn vector_length(vector: CadMeasurePoint3) -> f64 {
    dot(vector, vector).sqrt()
}

#[cfg(test)]
mod tests {
    use super::{
        CadMeasurePoint3, angle_between_vectors_deg, distance_between_points, vector_between_points,
    };

    #[test]
    fn distance_respects_tolerance_and_is_deterministic() {
        let a = CadMeasurePoint3::new(1.0, 2.0, 3.0);
        let b = CadMeasurePoint3::new(4.0, 6.0, 3.0);
        let first = distance_between_points(a, b, 1e-9);
        let second = distance_between_points(a, b, 1e-9);
        assert_eq!(first, second);
        assert!((first - 5.0).abs() < 1e-12);
        let collapsed =
            distance_between_points(a, CadMeasurePoint3::new(1.0 + 1e-7, 2.0, 3.0), 1e-6);
        assert_eq!(collapsed, 0.0);
    }

    #[test]
    fn angle_returns_expected_values() {
        let x = CadMeasurePoint3::new(1.0, 0.0, 0.0);
        let y = CadMeasurePoint3::new(0.0, 1.0, 0.0);
        let angle = angle_between_vectors_deg(x, y, 1e-9).expect("perpendicular vectors");
        assert!((angle - 90.0).abs() < 1e-10);

        let parallel = angle_between_vectors_deg(x, CadMeasurePoint3::new(2.0, 0.0, 0.0), 1e-9)
            .expect("parallel vectors");
        assert!((parallel - 0.0).abs() < 1e-10);

        let too_small = angle_between_vectors_deg(
            vector_between_points(
                CadMeasurePoint3::new(0.0, 0.0, 0.0),
                CadMeasurePoint3::new(0.0, 0.0, 0.0),
            ),
            y,
            1e-6,
        );
        assert!(too_small.is_none());
    }
}
