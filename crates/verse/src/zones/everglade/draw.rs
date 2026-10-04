//! Everglade's greybox geometry on the vertex-color path: the generated
//! ground, the workshop's floor outline, and a marker at every station.
//! Textured meshes replace the greybox later (#10487); the ground stays
//! generated.

use super::{
    HALF_EXTENT, HALL, PATH_HALF_WIDTH, RETURN_PORTAL, STATIONS, STRONGROOM, YARD, height,
};
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
const OUTLINE: [f32; 3] = [0.75, 0.6, 0.38];
const POST: [f32; 3] = [0.34, 0.32, 0.3];
/// Lamplight gold for marker edges, rings, and lettering.
const MARKER: [f32; 3] = [1.0, 0.78, 0.36];
/// Height of the hall's wall outline: one Medieval Village wall, m.
const WALL_HEIGHT: f32 = 3.12;
/// Marker post height and half width, m.
const POST_HEIGHT: f32 = 1.3;
const POST_HALF: f32 = 0.14;

fn vertex(pos: Vec3, color: [f32; 3]) -> Vertex {
    Vertex {
        pos: pos.to_array(),
        color,
        fog: 1.0,
    }
}

fn line(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    mesh.lines.push(vertex(a, color));
    mesh.lines.push(vertex(b, color));
}

/// Sunlight from above and slightly behind the approach, so slopes facing
/// the arriving player read lighter.
fn shade(color: [f32; 3], a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
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

/// The outline of a rectangle on the ground at height `y` above it.
fn rectangle(mesh: &mut Mesh, rect: ([f32; 2], [f32; 2]), y: f32, color: [f32; 3]) {
    let (c, h) = rect;
    let corners = [
        Vec3::new(c[0] - h[0], y, c[1] - h[1]),
        Vec3::new(c[0] + h[0], y, c[1] - h[1]),
        Vec3::new(c[0] + h[0], y, c[1] + h[1]),
        Vec3::new(c[0] - h[0], y, c[1] + h[1]),
    ];
    for i in 0..4 {
        line(mesh, corners[i], corners[(i + 1) % 4], color);
    }
}

/// Wireframe outlines of the hall and strongroom: floor, corner posts, and
/// eaves, so the workshop's volume reads before its walls are built. The
/// clearing is flat, so these stand at height zero.
pub(super) fn outlines(mesh: &mut Mesh) {
    for rect in [HALL, STRONGROOM] {
        let (c, h) = rect;
        rectangle(mesh, rect, 0.03, OUTLINE);
        rectangle(mesh, rect, WALL_HEIGHT, OUTLINE);
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let base = Vec3::new(c[0] + sx * h[0], 0.0, c[1] + sz * h[1]);
            line(mesh, base, base + Vec3::Y * WALL_HEIGHT, OUTLINE);
        }
    }
    // The hall's double door, in the middle of its south wall.
    let south = HALL.0[1] - HALL.1[1];
    for x in [-1.2, 1.2] {
        let base = Vec3::new(x, 0.0, south);
        line(mesh, base, base + Vec3::Y * 2.4, OUTLINE);
    }
    line(
        mesh,
        Vec3::new(-1.2, 2.4, south),
        Vec3::new(1.2, 2.4, south),
        OUTLINE,
    );
    rectangle(mesh, YARD, 0.03, PATH);
}

/// A shaded box between `min` and `max`, with lit edges.
fn post(mesh: &mut Mesh, min: Vec3, max: Vec3) {
    let c = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
    };
    let corners: [Vec3; 8] = std::array::from_fn(c);
    for [a, b, d, e] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        let q = [corners[a], corners[b], corners[d], corners[e]];
        let color = shade(POST, q[0], q[1], q[2]);
        for p in [q[0], q[1], q[2], q[0], q[2], q[3]] {
            mesh.faces.push(vertex(p, color));
        }
    }
    for (a, b) in [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        line(mesh, corners[a], corners[b], MARKER);
    }
}

/// Every station: a ring on its standing point, a post where its furniture
/// will stand, and its name above the post, reading from the approach side.
pub(super) fn markers(mesh: &mut Mesh) {
    for station in &STATIONS {
        let stand = station.position();
        let segments = 20;
        for k in 0..segments {
            let angle = |k: usize| k as f32 / segments as f32 * std::f32::consts::TAU;
            let p = |a: f32| stand + Vec3::new(a.cos() * 0.6, 0.04, a.sin() * 0.6);
            line(mesh, p(angle(k)), p(angle(k + 1)), MARKER);
        }
        let base = station.marker();
        let half = Vec3::new(POST_HALF, 0.0, POST_HALF);
        post(mesh, base - half, base + half + Vec3::Y * POST_HEIGHT);
        let mut letters = Mesh::default();
        crate::doors::scene_label(
            &mut letters,
            station.sign,
            base + Vec3::new(0.0, POST_HEIGHT + 0.25, -0.2),
            0.28,
            coder_ui::theme::Intensity::Full,
        );
        mesh.faces.extend(
            letters
                .faces
                .into_iter()
                .map(|v| Vertex { color: MARKER, ..v }),
        );
    }
}

/// A small diamond bobbing above each marker post, `elapsed` seconds in.
pub(super) fn beacons(elapsed: f32) -> Mesh {
    let mut mesh = Mesh::default();
    for (i, station) in STATIONS.iter().enumerate() {
        let bob = (elapsed * 1.5 + i as f32 * 0.7).sin() * 0.08;
        let center = station.marker() + Vec3::Y * (POST_HEIGHT + 0.9 + bob);
        let (r, h) = (0.12, 0.2);
        let top = center + Vec3::Y * h;
        let bottom = center - Vec3::Y * h;
        let ring = [
            center + Vec3::X * r,
            center + Vec3::Z * r,
            center - Vec3::X * r,
            center - Vec3::Z * r,
        ];
        for k in 0..4 {
            let (a, b) = (ring[k], ring[(k + 1) % 4]);
            line(&mut mesh, a, b, MARKER);
            line(&mut mesh, a, top, MARKER);
            line(&mut mesh, a, bottom, MARKER);
        }
    }
    mesh
}
