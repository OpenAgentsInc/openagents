//! Bounded visibility-graph routing for boxes on a horizontal static floor.
use crate::kinematic::{Aabb, sweep_box};
use glam::DVec3;

/// Returns the next waypoint on the shortest visible-corner route.
///
/// At most 64 obstacles are admitted. Nodes use the character's expanded box
/// corners with clearance. The solver has no crowds, slopes, or dynamic bodies.
/// `None` means no route exists; stable obstacle order resolves equal paths.
pub fn next_waypoint(
    start: DVec3,
    target: DVec3,
    half: DVec3,
    obstacles: &[Aabb],
) -> Result<Option<DVec3>, String> {
    if obstacles.len() > 64 || (start.y - target.y).abs() > 1e-6 {
        return Err("Unsupported static navigation query".into());
    }
    sweep_box(start, half, DVec3::ZERO, obstacles)?;
    sweep_box(target, half, DVec3::ZERO, obstacles)?;
    if sweep_box(start, half, target - start, obstacles)?.is_none() {
        return Ok(Some(target));
    }
    let mut nodes = vec![start, target];
    for bounds in obstacles {
        if bounds.max.y <= start.y - half.y + 1e-6 || bounds.min.y >= start.y + half.y - 1e-6 {
            continue;
        }
        for x in [bounds.min.x - half.x - 0.01, bounds.max.x + half.x + 0.01] {
            for z in [bounds.min.z - half.z - 0.01, bounds.max.z + half.z + 0.01] {
                let p = DVec3::new(x, start.y, z);
                if sweep_box(p, half, DVec3::ZERO, obstacles).is_ok() {
                    nodes.push(p);
                }
            }
        }
    }
    let mut distance = vec![f64::INFINITY; nodes.len()];
    let mut parent = vec![usize::MAX; nodes.len()];
    let mut visited = vec![false; nodes.len()];
    distance[0] = 0.;
    for _ in 0..nodes.len() {
        let Some(current) = (0..nodes.len())
            .filter(|i| !visited[*i] && distance[*i].is_finite())
            .min_by(|a, b| distance[*a].total_cmp(&distance[*b]))
        else {
            return Ok(None);
        };
        if current == 1 {
            let mut next = 1;
            while parent[next] != 0 {
                next = parent[next];
            }
            return Ok(Some(nodes[next]));
        }
        visited[current] = true;
        for next in 0..nodes.len() {
            if visited[next] {
                continue;
            }
            let delta = nodes[next] - nodes[current];
            let candidate = distance[current] + delta.length();
            if candidate < distance[next]
                && sweep_box(nodes[current], half, delta, obstacles)?.is_none()
            {
                distance[next] = candidate;
                parent[next] = current;
            }
        }
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_around_a_column_without_intersecting_it() {
        let half = DVec3::new(0.35, 0.9, 0.35);
        let column = Aabb {
            min: DVec3::new(-0.7, 0., -0.7),
            max: DVec3::new(0.7, 10., 0.7),
        };
        let target = DVec3::new(3., 0.9, 0.);
        let mut position = DVec3::new(-3., 0.9, 0.);
        let first = next_waypoint(position, target, half, &[column])
            .unwrap()
            .unwrap();
        assert!(first.z.abs() > 1.05);
        for _ in 0..150 {
            let waypoint = next_waypoint(position, target, half, &[column])
                .unwrap()
                .unwrap();
            let delta = waypoint - position;
            let movement = delta.normalize_or_zero() * delta.length().min(0.1);
            assert!(
                sweep_box(position, half, movement, &[column])
                    .unwrap()
                    .is_none()
            );
            position += movement;
        }
        assert!(position.distance(target) < 1e-8);
    }
    #[test]
    fn refuses_bad_queries_and_reports_an_unreachable_target() {
        let half = DVec3::splat(0.1);
        let walls = [
            Aabb {
                min: DVec3::new(1., -1., -2.),
                max: DVec3::new(2., 1., 2.),
            },
            Aabb {
                min: DVec3::new(-2., -1., -2.),
                max: DVec3::new(-1., 1., 2.),
            },
            Aabb {
                min: DVec3::new(-2., -1., 1.),
                max: DVec3::new(2., 1., 2.),
            },
            Aabb {
                min: DVec3::new(-2., -1., -2.),
                max: DVec3::new(2., 1., -1.),
            },
        ];
        assert_eq!(
            next_waypoint(DVec3::new(4., 0., 0.), DVec3::ZERO, half, &walls).unwrap(),
            None
        );
        assert!(next_waypoint(DVec3::ZERO, DVec3::Y, half, &[]).is_err());
        assert!(next_waypoint(DVec3::ZERO, DVec3::ZERO, half, &[walls[0]; 65]).is_err());
    }
}
