//! Bounded local door, held-item, effect, and depth-tested label geometry.
use super::{DemoItem, DoorId, Doors};
use crate::{
    controller::{Footprint, PlayerController},
    mesh::Mesh,
};
use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};

pub fn geometry() -> (Mesh, Vec<Footprint>) {
    let mut mesh = Mesh::default();
    let mut blockers = Vec::with_capacity(4);
    for id in DoorId::ALL {
        let base = id.position();
        for side in [-1.0, 1.0] {
            let at = base + Vec3::new(side * 1.6, 1.8, 0.0);
            mesh.cube(
                Mat4::from_translation(at) * Mat4::from_scale(Vec3::new(0.36, 3.6, 0.36)),
                Intensity::Half,
            );
            blockers.push(Footprint {
                min: [at.x - 0.18, at.z - 0.18],
                max: [at.x + 0.18, at.z + 0.18],
            });
        }
        if id == DoorId::Spark {
            mesh.cube(
                Mat4::from_translation(base + Vec3::new(0.0, 3.6, 0.0))
                    * Mat4::from_scale(Vec3::new(3.56, 0.3, 0.36)),
                Intensity::Full,
            );
            let zig = [-1.3, -0.65, 0.0, 0.65, 1.3]
                .map(|x| base + Vec3::new(x, 3.93 + if x == 0.0 { 0.24 } else { 0.0 }, 0.0));
            for p in zig.windows(2) {
                mesh.line(p[0], p[1], Intensity::Full);
            }
        } else {
            for ring in [1.6, 1.78] {
                let points: Vec<Vec3> = (0..=24)
                    .map(|i| {
                        let angle = i as f32 / 24.0 * std::f32::consts::PI;
                        base + Vec3::new(angle.cos() * ring, 3.1 + angle.sin() * 0.95, 0.0)
                    })
                    .collect();
                for p in points.windows(2) {
                    mesh.line(p[0], p[1], Intensity::Full);
                }
            }
        }
    }
    (mesh, blockers)
}

pub fn held_mesh(item: DemoItem, player: &PlayerController) -> Mesh {
    let forward = player.forward();
    let right = forward.cross(Vec3::Y);
    let at = player.pos + right * 0.62 + forward * 0.1 + Vec3::Y * 0.98;
    let transform = Mat4::from_translation(at)
        * Mat4::from_rotation_y(player.yaw)
        * Mat4::from_scale(Vec3::splat(0.22));
    let mut mesh = Mesh::default();
    match item {
        DemoItem::Prism => {
            let points = [
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-0.7, 0.0, -0.5),
                Vec3::new(0.7, 0.0, -0.5),
                Vec3::new(0.0, 0.0, 0.7),
                Vec3::new(0.0, -1.0, 0.0),
            ]
            .map(|p| transform.transform_point3(p));
            for i in 1..4 {
                let j = if i == 3 { 1 } else { i + 1 };
                mesh.quad([points[0], points[i], points[j], points[j]]);
                mesh.quad([points[4], points[j], points[i], points[i]]);
                mesh.line(points[0], points[i], Intensity::Full);
                mesh.line(points[4], points[i], Intensity::Full);
                mesh.line(points[i], points[j], Intensity::Half);
            }
        }
        DemoItem::Ring => {
            let points: Vec<Vec3> = (0..24)
                .map(|i| {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    transform.transform_point3(Vec3::new(a.cos(), a.sin(), 0.0))
                })
                .collect();
            mesh.polyline_loop(&points, Intensity::Full);
        }
        DemoItem::Bolt => {
            let points = [
                [-0.2, 1.0],
                [0.4, 1.0],
                [-0.1, 0.15],
                [0.5, 0.15],
                [-0.4, -1.0],
                [-0.05, -0.15],
                [-0.5, -0.15],
            ]
            .map(|[x, y]| transform.transform_point3(Vec3::new(x, y, 0.0)));
            mesh.polyline_loop(&points, Intensity::Full);
        }
        DemoItem::Empty => {}
    }
    mesh
}

pub(super) fn dynamic(doors: &Doors) -> Mesh {
    let mut mesh = Mesh::default();
    for id in DoorId::ALL {
        let state = doors.state(id);
        label(
            &mut mesh,
            if id == DoorId::Spark {
                "SPARK / LOCAL"
            } else {
                "HALO / LOCAL"
            },
            id.position() + Vec3::new(0.0, 4.4, -0.2),
            0.16,
            Intensity::Full,
        );
        label(
            &mut mesh,
            state.caption(),
            id.position() + Vec3::new(0.0, 3.0, -0.24),
            0.11,
            Intensity::Half,
        );
        if let Some(u) = state.reaction_progress() {
            mesh.extend(&effect(id, u, state.reaction));
        }
    }
    mesh
}

pub(super) fn effect(id: DoorId, u: f32, reaction: u64) -> Mesh {
    let mut mesh = Mesh::default();
    let base = id.position();
    let u = u.clamp(0.0, 1.0);
    if id == DoorId::Spark {
        for path in 0..8 {
            let mut previous = None;
            for segment in 0..=6 {
                let phase =
                    path as f32 * 1.7 + segment as f32 * 2.3 + (reaction % 17) as f32 + u * 24.0;
                let x = -1.2 + path as f32 / 7.0 * 2.4 + phase.sin() * 0.12;
                let y = 0.15 + segment as f32 / 6.0 * 3.2;
                let point = base + Vec3::new(x, y, -0.16);
                if let Some(p) = previous {
                    mesh.line(
                        p,
                        point,
                        if segment % 2 == 0 {
                            Intensity::Full
                        } else {
                            Intensity::Half
                        },
                    );
                }
                previous = Some(point);
            }
        }
    } else {
        for ring in 0..4 {
            let phase = (u + ring as f32 * 0.25) % 1.0;
            let radius = 0.9 + 0.22 * (phase * std::f32::consts::PI).sin();
            let points: Vec<Vec3> = (0..24)
                .map(|i| {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    base + Vec3::new(a.cos() * radius, 0.2 + phase * 3.0, -0.08 + a.sin() * 0.16)
                })
                .collect();
            mesh.polyline_loop(
                &points,
                if ring % 2 == 0 {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
            );
        }
    }
    mesh
}

/// Bounded ASCII text in an XY plane facing -Z. Quads use the world depth buffer.
fn label(mesh: &mut Mesh, text: &str, anchor: Vec3, height: f32, intensity: Intensity) {
    let letters: Vec<u8> = text
        .bytes()
        .take(32)
        .map(|b| b.to_ascii_uppercase())
        .collect();
    let cell = height / 7.0;
    let width = letters.len() as f32 * 6.0 * cell;
    for (i, letter) in letters.into_iter().enumerate() {
        let glyph = glyph(letter);
        for (row, bits) in glyph.into_iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                let x = width / 2.0 - (i as f32 * 6.0 + col as f32) * cell;
                let y = (6 - row) as f32 * cell;
                let p = anchor + Vec3::new(x, y, 0.0);
                mesh.amber_quad(
                    [
                        p,
                        p + Vec3::new(-cell, 0.0, 0.0),
                        p + Vec3::new(-cell, cell, 0.0),
                        p + Vec3::new(0.0, cell, 0.0),
                    ],
                    intensity,
                );
            }
        }
    }
}
fn glyph(c: u8) -> [u8; 7] {
    match c {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 2, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        _ => [0; 7],
    }
}
