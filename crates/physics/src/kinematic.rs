//! Continuous axis-aligned box queries and kinematic wall sliding.
use glam::DVec3;

/// An axis-aligned static obstacle in world meters.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}
/// First blocking contact along a displacement, with deterministic obstacle order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepHit {
    pub fraction: f64,
    pub normal: DVec3,
    pub obstacle: usize,
}
/// Sweeps a box center through static boxes using their Minkowski expansion.
///
/// This is an exact axis-aligned box query, not a capsule or mesh query.
/// An initially penetrating box is refused; touching surfaces are allowed.
pub fn sweep_box(
    center: DVec3,
    half: DVec3,
    delta: DVec3,
    obstacles: &[Aabb],
) -> Result<Option<SweepHit>, String> {
    if !center.is_finite() || !half.is_finite() || !delta.is_finite() || half.min_element() < 0. {
        return Err("Invalid box sweep".into());
    }
    let mut closest: Option<SweepHit> = None;
    for (obstacle, bounds) in obstacles.iter().enumerate() {
        if !bounds.min.is_finite() || !bounds.max.is_finite() || !bounds.min.cmple(bounds.max).all()
        {
            return Err("Invalid obstacle bounds".into());
        }
        let min = bounds.min - half;
        let max = bounds.max + half;
        if center.cmpgt(min).all() && center.cmplt(max).all() {
            return Err("Box sweep starts inside an obstacle".into());
        }
        let mut enter: f64 = 0.;
        let mut exit: f64 = 1.;
        let mut normal = DVec3::ZERO;
        let mut missed = false;
        for axis in 0..3 {
            if delta[axis].abs() < 1e-12 {
                if center[axis] < min[axis] || center[axis] > max[axis] {
                    missed = true;
                    break;
                }
                continue;
            }
            let a = (min[axis] - center[axis]) / delta[axis];
            let b = (max[axis] - center[axis]) / delta[axis];
            let near = a.min(b);
            let far = a.max(b);
            if near >= enter {
                enter = near;
                normal = DVec3::ZERO;
                normal[axis] = -delta[axis].signum();
            }
            exit = exit.min(far);
            if enter > exit {
                missed = true;
                break;
            }
        }
        if !missed
            && (0. ..=1.).contains(&enter)
            && delta.dot(normal) < 0.
            && closest.is_none_or(|hit| enter < hit.fraction)
        {
            closest = Some(SweepHit {
                fraction: enter,
                normal,
                obstacle,
            });
        }
    }
    Ok(closest)
}
/// Moves a box while projecting remaining displacement along blocking surfaces.
///
/// Four contacts bound the work. A small separation prevents numerical re-entry.
pub fn move_and_slide(
    center: DVec3,
    half: DVec3,
    delta: DVec3,
    obstacles: &[Aabb],
) -> Result<DVec3, String> {
    let mut position = center;
    let mut remaining = delta;
    for _ in 0..4 {
        let Some(hit) = sweep_box(position, half, remaining, obstacles)? else {
            return Ok(position + remaining);
        };
        let safe = (hit.fraction - 1e-4 / remaining.length().max(1e-12)).max(0.);
        position += remaining * safe;
        remaining *= 1. - safe;
        remaining -= hit.normal * remaining.dot(hit.normal).min(0.);
        if remaining.length_squared() < 1e-18 {
            break;
        }
    }
    Ok(position)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wall() -> Aabb {
        Aabb {
            min: DVec3::new(1., -10., -10.),
            max: DVec3::new(1.01, 10., 10.),
        }
    }
    #[test]
    fn fast_box_cannot_tunnel_through_a_thin_wall() {
        let hit = sweep_box(DVec3::ZERO, DVec3::splat(0.25), DVec3::X * 100., &[wall()])
            .unwrap()
            .unwrap();
        assert!((hit.fraction - 0.0075).abs() < 1e-12);
        assert_eq!(hit.normal, -DVec3::X);
        let p =
            move_and_slide(DVec3::ZERO, DVec3::splat(0.25), DVec3::X * 100., &[wall()]).unwrap();
        assert!(p.x < 0.75 && p.x > 0.749);
    }
    #[test]
    fn slides_along_walls_and_stops_at_a_corner() {
        let p = move_and_slide(
            DVec3::ZERO,
            DVec3::splat(0.25),
            DVec3::new(3., 0., 2.),
            &[wall()],
        )
        .unwrap();
        assert!((p.z - 2.).abs() < 1e-9 && p.x < 0.75);
        let corner = Aabb {
            min: DVec3::new(-10., -10., 1.),
            max: DVec3::new(10., 10., 1.01),
        };
        let p = move_and_slide(
            DVec3::ZERO,
            DVec3::splat(0.25),
            DVec3::new(3., 0., 3.),
            &[wall(), corner],
        )
        .unwrap();
        assert!(p.x < 0.75 && p.z < 0.75);
    }
    #[test]
    fn contact_allows_parallel_and_outward_motion_but_blocks_inward() {
        let start = DVec3::new(0.75, 0., 0.);
        assert!(
            sweep_box(start, DVec3::splat(0.25), DVec3::Z, &[wall()])
                .unwrap()
                .is_none()
        );
        assert!(
            sweep_box(start, DVec3::splat(0.25), -DVec3::X, &[wall()])
                .unwrap()
                .is_none()
        );
        assert_eq!(
            sweep_box(start, DVec3::splat(0.25), DVec3::X, &[wall()])
                .unwrap()
                .unwrap()
                .fraction,
            0.
        );
        assert!(sweep_box(DVec3::X, DVec3::splat(0.25), DVec3::Z, &[wall()]).is_err());
        assert!(sweep_box(DVec3::NAN, DVec3::ONE, DVec3::ZERO, &[]).is_err());
    }
}
