//! Everglade's generated ground on the vertex-color path. The workshop,
//! the glade, and the stations are textured placements from the zone pack
//! (`layout`), and the boards are drawn by `boards`.

use super::{HALF_EXTENT, HALL, PATH_HALF_WIDTH, RETURN_PORTAL, STRONGROOM, YARD, height};
use crate::mesh::{Mesh, Vertex};
use glam::Vec3;

/// Ground cell size, m.
const CELL: f32 = 2.0;
const GRASS: [f32; 3] = [0.075, 0.17, 0.045];
/// The forest floor under the tree ring: darker and browner.
const FOREST: [f32; 3] = [0.05, 0.085, 0.03];
const PATH: [f32; 3] = [0.3, 0.24, 0.14];
const YARD_DIRT: [f32; 3] = [0.24, 0.2, 0.13];
const FLOOR: [f32; 3] = [0.3, 0.27, 0.22];

fn vertex(pos: Vec3, color: [f32; 3]) -> Vertex {
    Vertex {
        pos: pos.to_array(),
        color,
        fog: 1.0,
    }
}

/// Sunlight from above and slightly behind the approach, so slopes facing
/// the arriving player read lighter.
pub(super) fn shade(color: [f32; 3], a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let light = Vec3::new(0.35, 0.85, -0.4).normalize();
    let k = 0.62 + 0.45 * normal.dot(light).abs();
    color.map(|v| v * k)
}

fn inside(rect: ([f32; 2], [f32; 2]), x: f32, z: f32) -> bool {
    let (center, half) = rect;
    (x - center[0]).abs() <= half[0] && (z - center[1]).abs() <= half[1]
}

/// The ground's color at a point: path, yard, and floors in the clearing,
/// grass darkening into forest floor toward the tree ring.
fn ground_color(x: f32, z: f32) -> [f32; 3] {
    if inside(HALL, x, z) || inside(STRONGROOM, x, z) {
        return FLOOR;
    }
    if inside(YARD, x, z) {
        return YARD_DIRT;
    }
    let path_end = YARD.0[1] - YARD.1[1];
    if x.abs() <= PATH_HALF_WIDTH && (RETURN_PORTAL.z - 2.0..=path_end).contains(&z) {
        return PATH;
    }
    let t = ((x.hypot(z) - super::CLEARING_RADIUS) / (super::RING_RADIUS - super::CLEARING_RADIUS))
        .clamp(0.0, 1.0);
    std::array::from_fn(|i| GRASS[i] + (FOREST[i] - GRASS[i]) * t)
}

/// The heightfield as shaded triangles over the whole walkable square.
pub(super) fn ground(mesh: &mut Mesh) {
    let n = (2.0 * HALF_EXTENT / CELL).round() as i32;
    let at = |i: i32, j: i32| {
        let x = -HALF_EXTENT + i as f32 * CELL;
        let z = -HALF_EXTENT + j as f32 * CELL;
        Vec3::new(x, height(x, z), z)
    };
    mesh.faces.reserve(n as usize * n as usize * 6);
    for i in 0..n {
        for j in 0..n {
            let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                let (a, b, c) = (corners[a], corners[b], corners[c]);
                let center = (a + b + c) / 3.0;
                let color = shade(ground_color(center.x, center.z), a, b, c);
                for p in [a, b, c] {
                    mesh.faces.push(vertex(p, color));
                }
            }
        }
    }
}
