//! The boards Verse draws in the glade, as the Gym draws its boards: the
//! Task Wall in the yard and a monitor on every desk. They are depth-tested
//! faces on the zone's vertex-color path; the studio workspace (#10465)
//! fills their cards and log lines.

use super::layout::{Board, DESKS, TASK_COLUMNS, TASK_WALL};
use crate::mesh::{Mesh, Vertex};
use glam::{Mat4, Vec3};

const WOOD: [f32; 3] = [0.2, 0.12, 0.05];
const SLATE: [f32; 3] = [0.025, 0.04, 0.03];
const CHALK: [f32; 3] = [0.82, 0.78, 0.62];
const CHALK_DIM: [f32; 3] = [0.4, 0.38, 0.3];
const BEZEL: [f32; 3] = [0.07, 0.07, 0.08];
const SCREEN: [f32; 3] = [0.015, 0.03, 0.035];
const GLOW: [f32; 3] = [0.45, 0.85, 0.55];
const GLOW_DIM: [f32; 3] = [0.14, 0.32, 0.2];

fn vertex(pos: Vec3, color: [f32; 3]) -> Vertex {
    Vertex {
        pos: pos.to_array(),
        color,
        fog: 1.0,
    }
}

/// A flat quad in board space at depth `z`, from `min` to `max` in x and y.
fn panel(mesh: &mut Mesh, min: [f32; 2], max: [f32; 2], z: f32, color: [f32; 3]) {
    let [a, b, c, d] = [
        Vec3::new(min[0], min[1], z),
        Vec3::new(max[0], min[1], z),
        Vec3::new(max[0], max[1], z),
        Vec3::new(min[0], max[1], z),
    ]
    .map(|p| vertex(p, color));
    mesh.faces.extend_from_slice(&[a, b, c, a, c, d]);
}

/// A shaded box between `min` and `max`.
fn slab(mesh: &mut Mesh, min: Vec3, max: Vec3, color: [f32; 3]) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
    };
    let corners: [Vec3; 8] = std::array::from_fn(corner);
    for [a, b, c, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        let q = [corners[a], corners[b], corners[c], corners[d]];
        let shaded = super::draw::shade(color, q[0], q[1], q[2]);
        for p in [q[0], q[1], q[2], q[0], q[2], q[3]] {
            mesh.faces.push(vertex(p, shaded));
        }
    }
}

/// Lettering centered on `(x, y)` just in front of the face at depth `z`.
fn letters(mesh: &mut Mesh, text: &str, x: f32, y: f32, z: f32, height: f32, color: [f32; 3]) {
    let mut glyphs = Mesh::default();
    crate::doors::scene_label(
        &mut glyphs,
        text,
        Vec3::new(x, y, z),
        height,
        coder_ui::theme::Intensity::Full,
    );
    mesh.faces
        .extend(glyphs.faces.into_iter().map(|v| Vertex { color, ..v }));
}

/// Moves board-space faces into the glade.
fn place(world: &mut Mesh, board: &Board, local: Mesh) {
    let transform: Mat4 = board.transform();
    world.faces.extend(local.faces.into_iter().map(|v| Vertex {
        pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
        ..v
    }));
}

/// The Task Wall: a framed slate on two legs with its title, a column per
/// task state, and the rules between them.
fn task_wall(world: &mut Mesh) {
    let board = TASK_WALL;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut mesh = Mesh::default();
    slab(
        &mut mesh,
        Vec3::new(-hw, -hh, 0.0),
        Vec3::new(hw, hh, 0.04),
        SLATE,
    );
    let bar = 0.07;
    for (min, max) in [
        ([-hw - bar, hh], [hw + bar, hh + bar]),
        ([-hw - bar, -hh - bar], [hw + bar, -hh]),
        ([-hw - bar, -hh], [-hw, hh]),
        ([hw, -hh], [hw + bar, hh]),
    ] {
        slab(
            &mut mesh,
            Vec3::new(min[0], min[1], -0.03),
            Vec3::new(max[0], max[1], 0.06),
            WOOD,
        );
    }
    // Legs from the ground to the frame.
    let ground = -board.center.y;
    for x in [-hw + 0.05, hw - 0.05] {
        slab(
            &mut mesh,
            Vec3::new(x - 0.06, ground, 0.0),
            Vec3::new(x + 0.06, -hh - bar, 0.08),
            WOOD,
        );
    }
    let face = -0.01;
    letters(&mut mesh, "TASK WALL", 0.0, hh - 0.24, face, 0.14, CHALK);
    let rule = hh - 0.3;
    panel(
        &mut mesh,
        [-hw + 0.06, rule - 0.012],
        [hw - 0.06, rule],
        face,
        CHALK_DIM,
    );
    let column = (w - 0.12) / TASK_COLUMNS.len() as f32;
    for (i, title) in TASK_COLUMNS.into_iter().enumerate() {
        // Board -X is the viewer's right, so columns run from +X.
        let center = hw - 0.06 - (i as f32 + 0.5) * column;
        letters(&mut mesh, title, center, rule - 0.13, face, 0.07, CHALK);
        if i > 0 {
            let x = center + column / 2.0;
            panel(
                &mut mesh,
                [x - 0.006, -hh + 0.08],
                [x + 0.006, rule - 0.02],
                face,
                CHALK_DIM,
            );
        }
    }
    place(world, &board, mesh);
}

/// A desk monitor: a bezel on a short stand with the seat's name and
/// placeholder log lines.
fn monitor(world: &mut Mesh, board: &Board, seat: usize) {
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut mesh = Mesh::default();
    slab(
        &mut mesh,
        Vec3::new(-hw - 0.03, -hh - 0.03, 0.0),
        Vec3::new(hw + 0.03, hh + 0.03, 0.05),
        BEZEL,
    );
    // The stand reaches down to the workbench's top at 0.89 m.
    let desk = 0.89 - board.center.y;
    slab(
        &mut mesh,
        Vec3::new(-0.04, desk, 0.02),
        Vec3::new(0.04, -hh - 0.03, 0.05),
        BEZEL,
    );
    slab(
        &mut mesh,
        Vec3::new(-0.16, desk, -0.06),
        Vec3::new(0.16, desk + 0.02, 0.1),
        BEZEL,
    );
    let face = -0.005;
    panel(&mut mesh, [-hw, -hh], [hw, hh], face, SCREEN);
    let text = -0.01;
    letters(
        &mut mesh,
        &format!("SEAT {}", seat + 1),
        0.0,
        hh - 0.12,
        text,
        0.07,
        GLOW,
    );
    for (i, length) in [0.62_f32, 0.44, 0.55, 0.3].into_iter().enumerate() {
        let y = hh - 0.2 - i as f32 * 0.075;
        // Lines start at the viewer's left, board +X.
        let start = hw - 0.06;
        panel(
            &mut mesh,
            [start - length, y - 0.025],
            [start, y],
            text,
            GLOW_DIM,
        );
    }
    place(world, board, mesh);
}

/// Every board in the glade.
pub(super) fn draw(world: &mut Mesh) {
    task_wall(world);
    for (seat, desk) in DESKS.iter().enumerate() {
        monitor(world, &desk.monitor, seat);
    }
}
