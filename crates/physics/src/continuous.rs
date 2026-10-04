//! Relative-motion continuous collision for translated upright capsules.
use glam::DVec3;

/// Finds the first contact between a moving sphere and a translating capsule.
/// Capsule feet and sphere centers share one normalized time interval. The
/// capsule retains its upright orientation and dimensions throughout the step.
pub fn sphere_capsule(
    sphere_start: DVec3,
    sphere_end: DVec3,
    sphere_radius: f64,
    feet_start: DVec3,
    feet_end: DVec3,
    capsule_radius: f64,
    capsule_height: f64,
) -> Result<Option<f64>, String> {
    if [sphere_start, sphere_end, feet_start, feet_end]
        .iter()
        .any(|p| !p.is_finite() || p.abs().max_element() > 1_000_000.)
        || !sphere_radius.is_finite()
        || !(0. ..=100.).contains(&sphere_radius)
        || !capsule_radius.is_finite()
        || !(0.001..=100.).contains(&capsule_radius)
        || !capsule_height.is_finite()
        || capsule_height < 2. * capsule_radius
        || capsule_height > 1000.
    {
        return Err("Invalid relative-motion capsule sweep".into());
    }
    let start = sphere_start - feet_start;
    let delta = (sphere_end - sphere_start) - (feet_end - feet_start);
    let radius = sphere_radius + capsule_radius;
    let lower = capsule_radius;
    let upper = capsule_height - capsule_radius;
    let closest = DVec3::new(0., start.y.clamp(lower, upper), 0.);
    if start.distance_squared(closest) <= radius * radius {
        return Ok(Some(0.));
    }
    let mut earliest: Option<f64> = None;
    let mut admit = |t: f64| {
        if (0. ..=1.).contains(&t) && earliest.is_none_or(|old| t < old) {
            earliest = Some(t);
        }
    };
    // The cylinder and its two hemispherical caps partition the boundary.
    let a = delta.x * delta.x + delta.z * delta.z;
    let b = start.x * delta.x + start.z * delta.z;
    let c = start.x * start.x + start.z * start.z - radius * radius;
    if let Some(t) = entry(a, b, c) {
        let y = start.y + delta.y * t;
        if (lower..=upper).contains(&y) {
            admit(t);
        }
    }
    for (height, bottom) in [(lower, true), (upper, false)] {
        let offset = start - DVec3::Y * height;
        if let Some(t) = entry(
            delta.length_squared(),
            offset.dot(delta),
            offset.length_squared() - radius * radius,
        ) {
            let y = start.y + delta.y * t;
            if (bottom && y <= lower) || (!bottom && y >= upper) {
                admit(t);
            }
        }
    }
    Ok(earliest)
}

// Solve a*t² + 2*b*t + c = 0 for first entry from outside the shape.
fn entry(a: f64, b: f64, c: f64) -> Option<f64> {
    if a <= 0. {
        return None;
    }
    let discriminant = b * b - a * c;
    let tolerance = 16. * f64::EPSILON * (b * b).abs().max((a * c).abs());
    if discriminant < -tolerance {
        return None;
    }
    let root = discriminant.max(0.).sqrt();
    // Rationalization avoids cancellation for near contacts approaching a surface.
    let t = if b < 0. {
        c / (-b + root)
    } else {
        (-b - root) / a
    };
    (0. ..=1.).contains(&t).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_actor_crosses_stationary_projectile() {
        let t = sphere_capsule(
            DVec3::Y,
            DVec3::Y,
            0.06,
            DVec3::new(-2., 0., 0.),
            DVec3::new(2., 0., 0.),
            0.35,
            1.8,
        )
        .unwrap()
        .unwrap();
        assert!((t - 0.3975).abs() < 1e-12);
        // Testing only the final actor pose would miss this contact.
        assert_eq!(
            sphere_capsule(
                DVec3::Y,
                DVec3::Y,
                0.06,
                DVec3::new(2., 0., 0.),
                DVec3::new(2., 0., 0.),
                0.35,
                1.8
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn relative_translation_preserves_contact_and_equal_motion_misses() {
        let a = DVec3::new(-3., 1., 0.);
        let b = DVec3::new(3., 1., 0.);
        let base = sphere_capsule(a, b, 0.05, DVec3::ZERO, DVec3::ZERO, 0.35, 1.8).unwrap();
        let shift = DVec3::new(200., -100., 80.);
        let motion = DVec3::new(5., 2., -1.);
        let moved = sphere_capsule(
            a + shift,
            b + shift + motion,
            0.05,
            shift,
            shift + motion,
            0.35,
            1.8,
        )
        .unwrap();
        assert!((base.unwrap() - moved.unwrap()).abs() < 1e-12);
        assert_eq!(
            sphere_capsule(a, a + motion, 0.05, DVec3::ZERO, motion, 0.35, 1.8).unwrap(),
            None
        );
    }

    #[test]
    fn caps_tangency_and_initial_overlap() {
        assert!(
            sphere_capsule(
                DVec3::new(0., 3., 0.),
                DVec3::ZERO,
                0.05,
                DVec3::ZERO,
                DVec3::ZERO,
                0.35,
                1.8
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            sphere_capsule(DVec3::Y, DVec3::Y, 0., DVec3::ZERO, DVec3::ZERO, 0.35, 1.8).unwrap(),
            Some(0.)
        );
        let t = sphere_capsule(
            DVec3::new(-1., 1., 0.4),
            DVec3::new(1., 1., 0.4),
            0.05,
            DVec3::ZERO,
            DVec3::ZERO,
            0.35,
            1.8,
        )
        .unwrap()
        .unwrap();
        assert!((t - 0.5).abs() < 1e-7);
        assert!(
            sphere_capsule(
                DVec3::NAN,
                DVec3::ZERO,
                0.05,
                DVec3::ZERO,
                DVec3::ZERO,
                0.35,
                1.8
            )
            .is_err()
        );
    }
}
