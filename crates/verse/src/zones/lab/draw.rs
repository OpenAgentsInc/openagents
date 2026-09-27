//! Physics Lab geometry: the hall and stage, every collider as a solid with
//! colored edges, and the overlays that show what the solver did.

use super::{
    STAGE,
    scenes::{FLOOR_HALF, G, Rig, Scene},
};
use crate::{
    controller::Footprint,
    mesh::{Mesh, Vertex},
    world::World,
};
use glam::{DQuat, DVec3, Quat, Vec3};
use physics::{BodyKind, DebugKind, Shape};

const FACE: [f32; 3] = [0.022, 0.03, 0.045];
const GRID: [f32; 3] = [0.035, 0.085, 0.11];
const STAGE_GRID: [f32; 3] = [0.06, 0.17, 0.21];
const TRIM: [f32; 3] = [0.12, 0.42, 0.52];
const STATIC: [f32; 3] = [0.45, 0.5, 0.56];
const KINEMATIC: [f32; 3] = [1.0, 0.68, 0.2];
const DYNAMIC: [f32; 3] = [0.3, 0.85, 1.0];
const RESTING: [f32; 3] = [0.16, 0.45, 0.55];
const HOLD: [f32; 3] = [0.35, 1.0, 0.5];
const SLIP: [f32; 3] = [1.0, 0.3, 0.25];
const CONTACT: [f32; 3] = [1.0, 0.9, 0.2];
const NORMAL: [f32; 3] = [1.0, 0.35, 0.8];
const ANCHOR: [f32; 3] = [0.4, 1.0, 0.6];
const FORCE: [f32; 3] = [1.0, 0.55, 0.15];
const PLUME: [f32; 3] = [1.4, 0.8, 0.3];

/// Scene position of a physics point.
fn at(p: DVec3) -> Vec3 {
    STAGE + p.as_vec3()
}

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

/// Dark faces lit from above and slightly toward the viewer, so solids read
/// as solids while their edges carry the color.
fn shade(normal: Vec3) -> [f32; 3] {
    let light = Vec3::new(0.3, 0.9, -0.35).normalize();
    let k = 0.7 + 0.8 * normal.normalize_or_zero().dot(light).max(0.0);
    FACE.map(|c| c * k)
}

fn tri(mesh: &mut Mesh, a: Vec3, b: Vec3, c: Vec3) {
    let color = shade((b - a).cross(c - a));
    for p in [a, b, c] {
        mesh.faces.push(vertex(p, color));
    }
}

fn quad(mesh: &mut Mesh, q: [Vec3; 4]) {
    tri(mesh, q[0], q[1], q[2]);
    tri(mesh, q[0], q[2], q[3]);
}

fn arrow(mesh: &mut Mesh, from: Vec3, to: Vec3, color: [f32; 3]) {
    let d = to - from;
    if d.length() < 1e-3 {
        return;
    }
    line(mesh, from, to, color);
    let back = -d.normalize() * (0.12_f32).min(d.length() * 0.4);
    let side = if d.normalize().y.abs() < 0.9 {
        d.cross(Vec3::Y).normalize()
    } else {
        d.cross(Vec3::X).normalize()
    } * back.length()
        * 0.5;
    line(mesh, to, to + back + side, color);
    line(mesh, to, to + back - side, color);
}

fn cross(mesh: &mut Mesh, p: Vec3, size: f32, color: [f32; 3]) {
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        line(mesh, p - axis * size, p + axis * size, color);
    }
}

fn ring(mesh: &mut Mesh, center: Vec3, u: Vec3, v: Vec3, radius: f32, color: [f32; 3]) {
    let n = 24;
    let p = |i: usize| {
        let t = i as f32 / n as f32 * std::f32::consts::TAU;
        center + (u * t.cos() + v * t.sin()) * radius
    };
    for i in 0..n {
        line(mesh, p(i), p(i + 1), color);
    }
}

fn cuboid(mesh: &mut Mesh, center: Vec3, rotation: Quat, half: Vec3, edge: [f32; 3]) {
    let c = |i: usize| {
        let local = Vec3::new(
            if i & 1 == 0 { -half.x } else { half.x },
            if i & 2 == 0 { -half.y } else { half.y },
            if i & 4 == 0 { -half.z } else { half.z },
        );
        center + rotation * local
    };
    let corners: [Vec3; 8] = std::array::from_fn(c);
    // Each face wound outward.
    for [a, b, d, e] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        quad(mesh, [corners[a], corners[b], corners[d], corners[e]]);
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
        line(mesh, corners[a], corners[b], edge);
    }
}

fn sphere(mesh: &mut Mesh, center: Vec3, rotation: Quat, radius: f32, edge: [f32; 3]) {
    let (lat, lon) = (8, 12);
    let p = |i: usize, j: usize| {
        let theta = i as f32 / lat as f32 * std::f32::consts::PI;
        let phi = j as f32 / lon as f32 * std::f32::consts::TAU;
        center
            + rotation
                * Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                )
                * radius
    };
    for i in 0..lat {
        for j in 0..lon {
            quad(mesh, [p(i, j), p(i, j + 1), p(i + 1, j + 1), p(i + 1, j)]);
        }
    }
    // An equator and two meridians make spin visible.
    let (x, y, z) = (rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z);
    ring(mesh, center, x, z, radius * 1.01, edge);
    ring(mesh, center, x, y, radius * 1.01, edge);
    ring(mesh, center, y, z, radius * 1.01, edge);
}

fn capsule(
    mesh: &mut Mesh,
    center: Vec3,
    rotation: Quat,
    radius: f32,
    half_length: f32,
    edge: [f32; 3],
) {
    let (u, v, axis) = (rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z);
    let n = 12;
    let dir = |j: usize| {
        let t = j as f32 / n as f32 * std::f32::consts::TAU;
        u * t.cos() + v * t.sin()
    };
    let (a, b) = (center - axis * half_length, center + axis * half_length);
    for j in 0..n {
        let (d0, d1) = (dir(j), dir(j + 1));
        quad(
            mesh,
            [
                a + d0 * radius,
                a + d1 * radius,
                b + d1 * radius,
                b + d0 * radius,
            ],
        );
        // Hemispherical caps as two bands and a tip.
        for (end, sign) in [(a, -1.0_f32), (b, 1.0)] {
            let band = |k: f32, d: Vec3| {
                let angle = k * std::f32::consts::FRAC_PI_2;
                end + (d * angle.cos() + axis * sign * angle.sin()) * radius
            };
            for k in 0..3 {
                let (k0, k1) = (k as f32 / 3.0, (k + 1) as f32 / 3.0);
                quad(
                    mesh,
                    [band(k0, d0), band(k0, d1), band(k1, d1), band(k1, d0)],
                );
            }
        }
    }
    for j in (0..n).step_by(3) {
        line(mesh, a + dir(j) * radius, b + dir(j) * radius, edge);
    }
    ring(mesh, a, u, v, radius, edge);
    ring(mesh, b, u, v, radius, edge);
}

/// The lab's static geometry and what the player cannot walk through.
pub(super) fn world() -> World {
    let mut mesh = Mesh::default();
    let half = super::HALF_EXTENT;
    let mut x = -half;
    while x <= half {
        line(
            &mut mesh,
            Vec3::new(x, 0.0, -half),
            Vec3::new(x, 0.0, half),
            GRID,
        );
        line(
            &mut mesh,
            Vec3::new(-half, 0.0, x),
            Vec3::new(half, 0.0, x),
            GRID,
        );
        x += 2.0;
    }
    let edge = FLOOR_HALF as f32;
    let corner = |x: f32, z: f32, y: f32| STAGE + Vec3::new(x, y, z);
    // A dark stage floor under a fine grid.
    quad(
        &mut mesh,
        [
            corner(-edge, -edge, -0.004),
            corner(-edge, edge, -0.004),
            corner(edge, edge, -0.004),
            corner(edge, -edge, -0.004),
        ],
    );
    let mut t = -edge;
    while t <= edge + 1e-3 {
        line(
            &mut mesh,
            corner(t, -edge, 0.002),
            corner(t, edge, 0.002),
            STAGE_GRID,
        );
        line(
            &mut mesh,
            corner(-edge, t, 0.002),
            corner(edge, t, 0.002),
            STAGE_GRID,
        );
        t += 0.5;
    }
    // A rail around the stage, lines only so it never hides the scene.
    let rail = edge + 0.4;
    let posts = 16;
    for i in 0..posts {
        let s = -rail + 2.0 * rail * i as f32 / posts as f32;
        for p in [
            corner(s, -rail, 0.0),
            corner(rail, s, 0.0),
            corner(-s, rail, 0.0),
            corner(-rail, -s, 0.0),
        ] {
            line(&mut mesh, p, p + Vec3::Y * 0.9, TRIM);
        }
    }
    for y in [0.0, 0.9] {
        for (a, b) in [
            ((-rail, -rail), (rail, -rail)),
            ((rail, -rail), (rail, rail)),
            ((rail, rail), (-rail, rail)),
            ((-rail, rail), (-rail, -rail)),
        ] {
            line(&mut mesh, corner(a.0, a.1, y), corner(b.0, b.1, y), TRIM);
        }
    }
    // The back wall carries the lab's name.
    let wall = STAGE.z + edge + 1.6;
    let (w, h) = (8.0, 5.5);
    quad(
        &mut mesh,
        [
            Vec3::new(-w, 0.0, wall),
            Vec3::new(-w, h, wall),
            Vec3::new(w, h, wall),
            Vec3::new(w, 0.0, wall),
        ],
    );
    for (a, b) in [
        (Vec3::new(-w, 0.0, wall), Vec3::new(-w, h, wall)),
        (Vec3::new(-w, h, wall), Vec3::new(w, h, wall)),
        (Vec3::new(w, h, wall), Vec3::new(w, 0.0, wall)),
    ] {
        line(&mut mesh, a, b, TRIM);
    }
    label(
        &mut mesh,
        "PHYSICS LAB",
        Vec3::new(0.0, h - 1.0, wall - 0.05),
        0.6,
        TRIM,
    );
    World {
        mesh,
        blockers: vec![
            Footprint {
                min: [-rail - 0.2, STAGE.z - rail - 0.2],
                max: [rail + 0.2, STAGE.z + rail + 0.2],
            },
            Footprint {
                min: [-w, wall - 0.2],
                max: [w, wall + 0.2],
            },
        ],
    }
}

/// Block lettering from the shared door font, recolored.
fn label(mesh: &mut Mesh, text: &str, anchor: Vec3, height: f32, color: [f32; 3]) {
    let mut letters = Mesh::default();
    crate::doors::scene_label(
        &mut letters,
        text,
        anchor,
        height,
        coder_ui::theme::Intensity::Full,
    );
    mesh.faces
        .extend(letters.faces.into_iter().map(|v| Vertex { color, ..v }));
}

/// Every collider, the scenario's own marks, and the debug overlay.
pub(super) fn scene(scene: &Scene, alpha: f64, overlay: bool) -> Mesh {
    let mut mesh = Mesh::default();
    let world = &scene.world;
    label(
        &mut mesh,
        scene.kind.sign(),
        STAGE + Vec3::new(0.0, 3.4, FLOOR_HALF as f32 + 1.5),
        0.4,
        DYNAMIC,
    );
    let pose = |id: physics::BodyId| -> (Vec3, Quat) {
        let (p, q) = world[id].interpolated(alpha);
        (at(p), q.as_quat())
    };
    for collider in world.colliders() {
        let body = &world[collider.body];
        // The stage floor is already drawn as part of the hall.
        if body.kind == BodyKind::Static && collider.shape.bound() > FLOOR_HALF - 1.0 {
            continue;
        }
        let (pos, rotation) = pose(collider.body);
        let center = pos + rotation * collider.offset.as_vec3();
        let rotation = rotation * collider.rotation.as_quat();
        let moving = !body.sleeping;
        let scripted = matches!(scene.rig, Rig::Manifold { top, .. } if top == collider.body);
        let edge = match (scene.highlight(collider.body), body.kind) {
            _ if scripted => KINEMATIC,
            (Some(true), _) => HOLD,
            (Some(false), _) => SLIP,
            (None, BodyKind::Static) => STATIC,
            (None, BodyKind::Kinematic) => KINEMATIC,
            (None, BodyKind::Dynamic) if moving => DYNAMIC,
            (None, BodyKind::Dynamic) => RESTING,
        };
        match collider.shape {
            // The scripted box is a wireframe so the manifold between the
            // boxes stays visible.
            Shape::Cuboid { half } if scripted => {
                let mut solid = Mesh::default();
                cuboid(&mut solid, center, rotation, half.as_vec3(), edge);
                mesh.lines.extend(solid.lines);
            }
            Shape::Cuboid { half } => cuboid(&mut mesh, center, rotation, half.as_vec3(), edge),
            Shape::Sphere { radius } => sphere(&mut mesh, center, rotation, radius as f32, edge),
            Shape::Capsule {
                radius,
                half_length,
            } => capsule(
                &mut mesh,
                center,
                rotation,
                radius as f32,
                half_length as f32,
                edge,
            ),
        }
    }
    rig(&mut mesh, scene, alpha);
    if overlay {
        // The manifold scene never steps, so it has no contact reports; draw
        // its detected points directly.
        if let Rig::Manifold { points, .. } = &scene.rig {
            for p in points {
                line(&mut mesh, at(p.point), at(p.point + p.normal * 0.3), NORMAL);
            }
        }
        for debug in world.debug_lines() {
            let color = match debug.kind {
                DebugKind::ContactNormal => NORMAL,
                DebugKind::ContactImpulse => CONTACT,
                DebugKind::Joint => ANCHOR,
                DebugKind::Strained => SLIP,
                DebugKind::Thrust => PLUME,
            };
            line(&mut mesh, at(debug.from), at(debug.to), color);
        }
        let points = match &scene.rig {
            Rig::Manifold { points, .. } => points.iter().map(|p| p.point).collect(),
            _ => world.contacts.iter().map(|c| c.point).collect::<Vec<_>>(),
        };
        for point in points {
            cross(&mut mesh, at(point), 0.05, CONTACT);
        }
        for (_, joint) in world.joints() {
            let (a, b) = joint.anchors(world);
            cross(&mut mesh, at(a), 0.07, ANCHOR);
            cross(&mut mesh, at(b), 0.07, ANCHOR);
        }
    }
    // Fixed joint pivots have no collider; draw a small block for each.
    for (_, joint) in world.joints() {
        for id in [joint.a, joint.b] {
            if world[id].kind == BodyKind::Static {
                cuboid(
                    &mut mesh,
                    at(world[id].pos),
                    Quat::IDENTITY,
                    Vec3::splat(0.06),
                    STATIC,
                );
            }
        }
    }
    mesh
}

/// Marks that belong to one scenario, drawn with or without the overlay.
fn rig(mesh: &mut Mesh, scene: &Scene, alpha: f64) {
    let world = &scene.world;
    match &scene.rig {
        Rig::Manifold { points, .. } => {
            if points.len() >= 2 {
                let centroid = points.iter().map(|p| p.point).sum::<DVec3>() / points.len() as f64;
                let normal = points[0].normal;
                let u = normal.any_orthonormal_vector();
                let v = normal.cross(u);
                let mut sorted: Vec<DVec3> = points.iter().map(|p| p.point).collect();
                sorted.sort_by(|a, b| {
                    let angle = |p: &DVec3| (*p - centroid).dot(v).atan2((*p - centroid).dot(u));
                    angle(a).total_cmp(&angle(b))
                });
                for i in 0..sorted.len() {
                    let (a, b) = (sorted[i], sorted[(i + 1) % sorted.len()]);
                    line(
                        mesh,
                        at(a) + Vec3::Y * 0.004,
                        at(b) + Vec3::Y * 0.004,
                        CONTACT,
                    );
                }
            }
        }
        Rig::Friction { block, load, .. } => {
            let (p, _) = world[*block].interpolated(alpha);
            // One body weight of force draws three meters long.
            let scale = 3.0 / (world[*block].mass * G) as f32;
            let head = at(p) - Vec3::X * 0.32;
            arrow(mesh, head - Vec3::X * (*load as f32 * scale), head, FORCE);
            // Friction: the tangential impulse the floor returned.
            let (sum, count) = world
                .contacts
                .iter()
                .filter(|c| c.body_a == *block || c.body_b == *block)
                .fold((DVec3::ZERO, 0), |(s, n), c| {
                    let sign = if c.body_b == *block { 1.0 } else { -1.0 };
                    let tangent = c.impulse - c.normal * c.impulse.dot(c.normal);
                    (s + tangent * sign, n + 1)
                });
            if count > 0 {
                let force = sum / super::DT;
                let base = at(DVec3::new(p.x, 0.02, p.z));
                arrow(mesh, base, base + force.as_vec3() * scale, SLIP);
            }
        }
        Rig::Torsion { ball, torque, .. } => {
            let (p, _) = world[*ball].interpolated(alpha);
            let top = at(p) + Vec3::Y * 0.55;
            let r = 0.25 + *torque as f32 * 0.3;
            let n = 18;
            let point = |i: usize| {
                let t = i as f32 / n as f32 * std::f32::consts::TAU * 0.8;
                top + Vec3::new(t.cos(), 0.0, -t.sin()) * r
            };
            for i in 0..n {
                line(mesh, point(i), point(i + 1), FORCE);
            }
            arrow(mesh, point(n - 1), point(n), FORCE);
        }
        Rig::Tunneling { trails, .. } => {
            for trail in trails {
                for pair in trail.windows(2) {
                    line(mesh, at(pair[0]), at(pair[1]), RESTING);
                }
            }
        }
        Rig::Momentum { origin, .. } => {
            let o = at(*origin);
            cross(mesh, o, 0.1, STATIC);
            let now = world.momentum(*origin);
            arrow(mesh, o, o + now.linear.as_vec3() * 0.04, FORCE);
            arrow(mesh, o, o + now.angular.as_vec3() * 0.1, NORMAL);
        }
        Rig::Stack { .. } | Rig::SoftGrip { .. } => {}
        Rig::Tether {
            pivot,
            bob,
            anchor,
            tether,
            length,
            ..
        } => {
            let (p, _) = world[*bob].interpolated(alpha);
            line(mesh, at(*pivot), at(p), STATIC);
            if let Some(joint) = world.joint(*tether) {
                let (_, end) = joint.anchors(world);
                let taut = end.distance(*anchor) >= *length - 0.01;
                line(
                    mesh,
                    at(*anchor),
                    at(end),
                    if taut { HOLD } else { RESTING },
                );
            }
        }
        Rig::Thrusters {
            craft,
            set,
            throttles,
            target,
            ..
        } => {
            let body = &world[*craft];
            let (p, q) = body.interpolated(alpha);
            for (thruster, u) in set.thrusters.iter().zip(throttles) {
                let mount = p + q * thruster.pos;
                if *u > 1e-3 {
                    let exhaust = -(q * thruster.dir);
                    line(
                        mesh,
                        at(mount),
                        at(mount + exhaust * (0.15 + 0.6 * u)),
                        PLUME,
                    );
                } else {
                    cross(mesh, at(mount), 0.02, STATIC);
                }
            }
            let goal = at(target.0);
            let facing = DQuat::from_rotation_y(target.1) * DVec3::Z;
            cross(mesh, goal, 0.15, HOLD);
            arrow(mesh, goal, goal + facing.as_vec3() * 0.5, HOLD);
        }
    }
}
