//! Lagrange 1: a construction station on a Lissajous orbit about Sun–Earth L1.
//! Physics lives in `verse-lagrange`; this module maps input and draws the scene.
//!
//! Scene axes follow the station frame: -Z faces the Sun, +Z the Earth, +Y the
//! ecliptic north pole. Geometry is generated here; no asset pack is fetched.

use crate::{
    controller::{InputState, PlayerController, TURN_SPEED},
    mesh::{Mesh, Vertex},
    world::World,
};
use glam::{DVec3, Mat4, Quat, Vec3};
use verse_lagrange::{
    Command, PartKind, PartState, Station,
    orbit::{EARTH_RADIUS, MOON_RADIUS, SUN_RADIUS},
    physics::DebugKind,
    station::{self, AIRLOCK},
};

/// Distance at which the Sun, Earth, and Moon are drawn with their true
/// angular sizes, inside the camera's 2 km far plane.
const SKY: f32 = 1_850.0;
/// Camera pitch that thrusts level; tilting beyond the band climbs or dives.
const LEVEL_PITCH: f32 = 0.28;
const LEVEL_BAND: f32 = 0.12;
/// The return portal, beside the airlock.
pub(crate) const RETURN_PORTAL: Vec3 = Vec3::new(-5.0, 4.4, 22.5);

const WHITE: [f32; 3] = [0.86, 0.86, 0.83];
const GOLD: [f32; 3] = [0.85, 0.62, 0.2];
const METAL: [f32; 3] = [0.52, 0.54, 0.58];
const CELL: [f32; 3] = [0.05, 0.09, 0.28];
const SAFETY: [f32; 3] = [0.92, 0.6, 0.08];

pub(crate) struct Lagrange {
    pub station: Station,
    /// Draw contacts, their impulses, joints, and thrust.
    pub overlay: bool,
    rendered: Mesh,
}

impl Lagrange {
    pub fn new() -> Self {
        let mut zone = Self {
            station: Station::new(),
            overlay: false,
            rendered: Mesh::default(),
        };
        zone.rendered = zone.build_dynamic();
        zone
    }

    /// Feet position for the shared player controller.
    pub fn spawn() -> Vec3 {
        feet(station::SPAWN)
    }

    pub fn spawn_yaw() -> f32 {
        std::f32::consts::PI
    }

    pub fn move_player(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        camera_pitch: f32,
        dt: f32,
    ) {
        if !input.mouse_look {
            if input.left {
                player.yaw = crate::controller::wrap(player.yaw + TURN_SPEED * dt);
            }
            if input.right {
                player.yaw = crate::controller::wrap(player.yaw - TURN_SPEED * dt);
            }
        }
        let strafe_left = input.strafe_left || (input.mouse_look && input.left);
        let strafe_right = input.strafe_right || (input.mouse_look && input.right);
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let ahead = axis(input.forward, input.backward);
        let side = axis(strafe_right, strafe_left);
        // Tilting the view past the level band flies along the view.
        let tilt = camera_pitch - LEVEL_PITCH;
        let elevation = if tilt.abs() > LEVEL_BAND {
            -(tilt - LEVEL_BAND * tilt.signum())
        } else {
            0.0
        };
        let forward =
            crate::controller::forward(player.yaw) * elevation.cos() + Vec3::Y * elevation.sin();
        let right = Vec3::new(-player.yaw.cos(), 0.0, player.yaw.sin());
        let direction = forward * ahead + right * side;
        self.station.step(
            f64::from(dt),
            &Command {
                direction: direction.as_dvec3(),
                yaw: f64::from(player.yaw),
                climb: input.jump,
            },
        );
        let astronaut = self.station.astronaut();
        player.pos = feet(astronaut.interpolated(self.station.alpha()).0);
        player.speed = astronaut.vel.length() as f32;
        player.set_surface_height(player.pos.y);
    }

    pub fn tick(&mut self) {
        self.rendered = self.build_dynamic();
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// Fixed station structure, stars, and the Sun.
    pub fn world() -> World {
        let mut mesh = Mesh::default();
        stars(&mut mesh);
        sun(&mut mesh);
        // Main truss along x with solar wings face-on to the Sun.
        lattice(
            &mut mesh,
            Vec3::new(-30.0, 6.0, 0.0),
            Vec3::new(30.0, 6.0, 0.0),
            1.4,
            METAL,
        );
        for side in [-1.0_f32, 1.0] {
            let (a, b) = (13.0 * side, 30.0 * side);
            panel(
                &mut mesh,
                [
                    Vec3::new(a, -0.5, -1.1),
                    Vec3::new(b, -0.5, -1.1),
                    Vec3::new(b, 12.5, -1.1),
                    Vec3::new(a, 12.5, -1.1),
                ],
                CELL,
            );
            for i in 0..=17 {
                let x = a + (b - a) * i as f32 / 17.0;
                line(
                    &mut mesh,
                    Vec3::new(x, -0.5, -1.13),
                    Vec3::new(x, 12.5, -1.13),
                    [0.25, 0.3, 0.45],
                );
            }
            for j in 0..=13 {
                let y = -0.5 + j as f32;
                line(
                    &mut mesh,
                    Vec3::new(a, y, -1.13),
                    Vec3::new(b, y, -1.13),
                    [0.25, 0.3, 0.45],
                );
            }
            // Radiators stand edge-on to the Sun so they reject heat to space.
            let x = 8.0 * side;
            panel(
                &mut mesh,
                [
                    Vec3::new(x, 6.7, -0.6),
                    Vec3::new(x, 6.7, 9.0),
                    Vec3::new(x, 15.0, 9.0),
                    Vec3::new(x, 15.0, -0.6),
                ],
                [0.9, 0.92, 0.95],
            );
            // Station-keeping thruster pods at the truss tips.
            shaded_box(
                &mut mesh,
                Vec3::new(30.8 * side, 6.0, 0.0),
                Vec3::splat(0.6),
                Quat::IDENTITY,
                METAL,
            );
        }
        // Habitat, node, and airlock.
        cylinder(
            &mut mesh,
            Vec3::new(0.0, 6.0, 8.0),
            Vec3::Z,
            2.1,
            12.0,
            20,
            WHITE,
        );
        for z in [3.0, 8.0, 13.0] {
            ring(&mut mesh, Vec3::new(0.0, 6.0, z), Vec3::Z, 2.13, GOLD);
        }
        shaded_box(
            &mut mesh,
            Vec3::new(0.0, 6.0, 15.5),
            Vec3::new(1.6, 1.6, 1.5),
            Quat::IDENTITY,
            WHITE,
        );
        let hatch = |x: f32, y: f32| Vec3::new(x, 6.0 + y, 17.02);
        for (a, b) in [
            ((-0.7, -0.7), (0.7, -0.7)),
            ((0.7, -0.7), (0.7, 0.7)),
            ((0.7, 0.7), (-0.7, 0.7)),
            ((-0.7, 0.7), (-0.7, -0.7)),
        ] {
            line(&mut mesh, hatch(a.0, a.1), hatch(b.0, b.1), SAFETY);
        }
        ring(
            &mut mesh,
            AIRLOCK.as_vec3(),
            Vec3::Z,
            station::REFILL_RANGE as f32,
            [0.2, 0.9, 0.5],
        );
        // Robotic arm on the truss.
        let shoulder = Vec3::new(-6.0, 6.8, 0.0);
        let elbow = Vec3::new(-6.0, 12.5, 4.5);
        let wrist = Vec3::new(-2.5, 9.5, 8.0);
        segment(&mut mesh, shoulder, elbow, 0.35, WHITE);
        segment(&mut mesh, elbow, wrist, 0.3, WHITE);
        shaded_box(&mut mesh, wrist, Vec3::splat(0.35), Quat::IDENTITY, METAL);
        // Keel jig: an open frame below the truss where the ship is built.
        let jig = station::JIG.as_vec3();
        for (x, y) in [(-3.5, -3.5), (3.5, -3.5), (3.5, 3.5), (-3.5, 3.5)] {
            segment(
                &mut mesh,
                jig + Vec3::new(x, y, -12.5),
                jig + Vec3::new(x, y, 9.5),
                0.14,
                SAFETY,
            );
        }
        for i in 0..=7 {
            let z = -12.5 + i as f32 * 22.0 / 7.0;
            let c = |x: f32, y: f32| jig + Vec3::new(x, y, z);
            for (a, b) in [
                ((-3.5, -3.5), (3.5, -3.5)),
                ((3.5, -3.5), (3.5, 3.5)),
                ((3.5, 3.5), (-3.5, 3.5)),
                ((-3.5, 3.5), (-3.5, -3.5)),
            ] {
                segment(&mut mesh, c(a.0, a.1), c(b.0, b.1), 0.1, SAFETY);
            }
        }
        for x in [-3.5, 3.5] {
            segment(
                &mut mesh,
                Vec3::new(x, 5.3, 0.0),
                jig + Vec3::new(x, 3.5, 0.0),
                0.16,
                METAL,
            );
        }
        // Parts depot backboard and rack arms.
        shaded_box(
            &mut mesh,
            Vec3::new(-15.3, -6.0, 1.25),
            Vec3::new(0.3, 3.0, 7.75),
            Quat::IDENTITY,
            METAL,
        );
        for kind in PartKind::ALL {
            let at = kind.stowage().as_vec3();
            segment(
                &mut mesh,
                Vec3::new(-15.0, at.y, at.z),
                at - Vec3::X * 1.1,
                0.08,
                SAFETY,
            );
        }
        segment(
            &mut mesh,
            Vec3::new(-15.0, -6.0, 1.0),
            Vec3::new(-3.5, -6.0, 1.0),
            0.12,
            METAL,
        );
        World {
            mesh,
            blockers: Vec::new(),
        }
    }

    fn build_dynamic(&self) -> Mesh {
        let mut mesh = Mesh::default();
        let s = &self.station;
        // Earth and Moon move across the sky as the station circles L1.
        let earth = scene(s.orbit.earth_vector());
        let moon = scene(s.orbit.moon_vector());
        body_disc(&mut mesh, earth, EARTH_RADIUS, [0.12, 0.32, 0.85], true);
        body_disc(&mut mesh, moon, MOON_RADIUS, [0.62, 0.62, 0.6], false);
        let at = earth.normalize() * SKY;
        let r = 22.0;
        let (u, v) = basis(at);
        for (du, dv) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let corner = at + (u * du + v * dv) * r;
            line(&mut mesh, corner, corner - u * du * 7.0, [0.3, 0.8, 0.9]);
            line(&mut mesh, corner, corner - v * dv * 7.0, [0.3, 0.8, 0.9]);
        }
        // Astronaut in a white suit with a maneuvering pack.
        let alpha = s.alpha();
        let astronaut_pos = s.astronaut().interpolated(alpha).0;
        let astronaut = astronaut_pos.as_vec3();
        let carrying = s.parts.iter().any(|p| p.state == PartState::Carried);
        suit(
            &mut mesh,
            feet(astronaut_pos),
            s.heading_yaw() as f32,
            carrying,
        );
        for p in &s.plumes {
            let pos = DVec3::from(p.pos).as_vec3();
            let dir = DVec3::from(p.dir).as_vec3();
            let fade = 1.0 - (p.age / 0.35) as f32;
            line(
                &mut mesh,
                pos,
                pos + dir * (0.9 * fade),
                [0.8 * fade, 0.9 * fade, 1.2 * fade],
            );
        }
        if s.keeping_glow > 0.0 {
            let k = (s.keeping_glow / 1.2) as f32;
            for side in [-1.0_f32, 1.0] {
                let base = Vec3::new(31.2 * side, 6.0, 0.0);
                for d in [Vec3::X * side, Vec3::Y, -Vec3::Y, Vec3::Z] {
                    line(
                        &mut mesh,
                        base,
                        base + d * 3.0 * k,
                        [1.4 * k, 1.1 * k, 0.6 * k],
                    );
                }
            }
        }
        for part in &s.parts {
            let (pos, orientation) = s.body(part).interpolated(alpha);
            let transform = Mat4::from_rotation_translation(orientation.as_quat(), pos.as_vec3());
            part_mesh(&mut mesh, part.kind, transform);
            if part.state == PartState::Carried {
                let slot = part.kind.slot().as_vec3();
                let ready = s.snapshot().latch_ready;
                let color = if ready {
                    [0.2, 1.3, 0.5]
                } else {
                    [1.2, 0.7, 0.15]
                };
                outline(&mut mesh, slot, part.kind.size().as_vec3() * 0.5, color);
                dashed(&mut mesh, pos.as_vec3(), slot, color);
            }
        }
        if let Some(next) = s.next_part()
            && !carrying
        {
            outline(
                &mut mesh,
                next.stowage().as_vec3(),
                next.size().as_vec3() * 0.55,
                [0.9, 0.8, 0.3],
            );
        }
        if let Some(target) = s.target {
            dashed(&mut mesh, astronaut, target.as_vec3(), [0.3, 0.8, 0.9]);
        }
        if self.overlay {
            for l in s.debug_lines() {
                let color = match l.kind {
                    DebugKind::ContactNormal => [0.3, 0.9, 1.2],
                    DebugKind::ContactImpulse => [1.4, 0.3, 0.2],
                    DebugKind::Joint => [0.9, 0.9, 0.3],
                    DebugKind::Strained => [1.5, 0.2, 0.9],
                    DebugKind::Thrust => [1.2, 0.7, 1.4],
                };
                line(&mut mesh, l.from.as_vec3(), l.to.as_vec3(), color);
            }
        }
        mesh
    }
}

fn feet(center: DVec3) -> Vec3 {
    center.as_vec3() - Vec3::Y * 0.9
}

/// Rotating-frame vector (x Sun→Earth, y along-track, z north) to scene axes.
fn scene(v: DVec3) -> Vec3 {
    Vec3::new(v.y as f32, v.z as f32, v.x as f32)
}

fn vertex(pos: Vec3, color: [f32; 3], fog: f32) -> Vertex {
    Vertex {
        pos: pos.to_array(),
        color,
        fog,
    }
}

fn line(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    mesh.lines.push(vertex(a, color, 0.0));
    mesh.lines.push(vertex(b, color, 0.0));
}

fn dashed(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    let steps = ((b - a).length() / 0.5).clamp(1.0, 400.0) as usize;
    for i in (0..steps).step_by(2) {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        line(mesh, a.lerp(b, t0), a.lerp(b, t1), color);
    }
}

/// Direct sunlight from -Z with a faint fill from the full Earth at +Z.
fn lit(normal: Vec3, base: [f32; 3]) -> [f32; 3] {
    let n = normal.normalize_or_zero();
    let k = 0.1 + 0.9 * (-n.z).max(0.0) + 0.06 * n.z.max(0.0) + 0.1 * n.y.abs();
    base.map(|c| c * k)
}

fn tri(mesh: &mut Mesh, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
    for p in [a, b, c] {
        mesh.faces.push(vertex(p, color, 1.0));
    }
}

fn quad(mesh: &mut Mesh, q: [Vec3; 4], color: [f32; 3]) {
    tri(mesh, q[0], q[1], q[2], color);
    tri(mesh, q[0], q[2], q[3], color);
}

/// A flat panel lit on its sunward side.
fn panel(mesh: &mut Mesh, q: [Vec3; 4], base: [f32; 3]) {
    let normal = (q[1] - q[0]).cross(q[3] - q[0]);
    let facing = if normal.z > 0.0 { -normal } else { normal };
    quad(mesh, q, lit(facing, base));
}

fn shaded_box(mesh: &mut Mesh, center: Vec3, half: Vec3, rotation: Quat, base: [f32; 3]) {
    let t = Mat4::from_rotation_translation(rotation, center);
    shaded_box_at(mesh, t, half, base);
}

fn shaded_box_at(mesh: &mut Mesh, t: Mat4, half: Vec3, base: [f32; 3]) {
    let corner = |x: f32, y: f32, z: f32| t.transform_point3(Vec3::new(x, y, z) * half);
    for (normal, q) in [
        (
            Vec3::X,
            [(1., -1., -1.), (1., 1., -1.), (1., 1., 1.), (1., -1., 1.)],
        ),
        (
            -Vec3::X,
            [
                (-1., -1., -1.),
                (-1., -1., 1.),
                (-1., 1., 1.),
                (-1., 1., -1.),
            ],
        ),
        (
            Vec3::Y,
            [(-1., 1., -1.), (-1., 1., 1.), (1., 1., 1.), (1., 1., -1.)],
        ),
        (
            -Vec3::Y,
            [
                (-1., -1., -1.),
                (1., -1., -1.),
                (1., -1., 1.),
                (-1., -1., 1.),
            ],
        ),
        (
            Vec3::Z,
            [(-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)],
        ),
        (
            -Vec3::Z,
            [
                (-1., -1., -1.),
                (-1., 1., -1.),
                (1., 1., -1.),
                (1., -1., -1.),
            ],
        ),
    ] {
        let color = lit(t.transform_vector3(normal), base);
        quad(mesh, q.map(|(x, y, z)| corner(x, y, z)), color);
    }
}

fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.normalize();
    let helper = if a.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = a.cross(helper).normalize();
    (u, a.cross(u))
}

fn cylinder(
    mesh: &mut Mesh,
    center: Vec3,
    axis: Vec3,
    radius: f32,
    length: f32,
    segments: usize,
    base: [f32; 3],
) {
    frustum(mesh, center, axis, radius, radius, length, segments, base);
}

#[allow(clippy::too_many_arguments)]
fn frustum(
    mesh: &mut Mesh,
    center: Vec3,
    axis: Vec3,
    r0: f32,
    r1: f32,
    length: f32,
    segments: usize,
    base: [f32; 3],
) {
    let (u, v) = basis(axis);
    let a = axis.normalize();
    let bottom = center - a * (length / 2.0);
    let top = center + a * (length / 2.0);
    let at = |i: usize| {
        let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
        u * angle.cos() + v * angle.sin()
    };
    for i in 0..segments {
        let (d0, d1) = (at(i), at(i + 1));
        let color = lit(d0 + d1, base);
        quad(
            mesh,
            [
                bottom + d0 * r0,
                bottom + d1 * r0,
                top + d1 * r1,
                top + d0 * r1,
            ],
            color,
        );
        tri(
            mesh,
            bottom,
            bottom + d0 * r0,
            bottom + d1 * r0,
            lit(-a, base),
        );
        tri(mesh, top, top + d1 * r1, top + d0 * r1, lit(a, base));
    }
}

fn ring(mesh: &mut Mesh, center: Vec3, axis: Vec3, radius: f32, color: [f32; 3]) {
    let (u, v) = basis(axis);
    let n = 32;
    for i in 0..n {
        let p = |i: usize| {
            let t = i as f32 / n as f32 * std::f32::consts::TAU;
            center + (u * t.cos() + v * t.sin()) * radius
        };
        line(mesh, p(i), p(i + 1), color);
    }
}

/// A square-section beam between two points.
fn segment(mesh: &mut Mesh, a: Vec3, b: Vec3, width: f32, base: [f32; 3]) {
    let d = b - a;
    let rotation = Quat::from_rotation_arc(Vec3::Z, d.normalize());
    shaded_box(
        mesh,
        (a + b) / 2.0,
        Vec3::new(width / 2.0, width / 2.0, d.length() / 2.0),
        rotation,
        base,
    );
}

/// A lattice truss: four longerons with diagonal bracing lines.
fn lattice(mesh: &mut Mesh, a: Vec3, b: Vec3, width: f32, base: [f32; 3]) {
    let (u, v) = basis(b - a);
    let h = width / 2.0;
    let corners = [u * h + v * h, -u * h + v * h, -u * h - v * h, u * h - v * h];
    for c in corners {
        segment(mesh, a + c, b + c, 0.1, base);
    }
    let bays = ((b - a).length() / width).max(1.0) as usize;
    for i in 0..bays {
        let p0 = a.lerp(b, i as f32 / bays as f32);
        let p1 = a.lerp(b, (i + 1) as f32 / bays as f32);
        for k in 0..4 {
            let (c0, c1) = (corners[k], corners[(k + 1) % 4]);
            line(mesh, p0 + c0, p1 + c1, base);
            line(mesh, p0 + c0, p0 + c1, base);
        }
    }
}

fn outline(mesh: &mut Mesh, center: Vec3, half: Vec3, color: [f32; 3]) {
    let c = |x: f32, y: f32, z: f32| center + Vec3::new(x, y, z) * half;
    for (p, q) in [
        ((-1., -1., -1.), (1., -1., -1.)),
        ((1., -1., -1.), (1., 1., -1.)),
        ((1., 1., -1.), (-1., 1., -1.)),
        ((-1., 1., -1.), (-1., -1., -1.)),
        ((-1., -1., 1.), (1., -1., 1.)),
        ((1., -1., 1.), (1., 1., 1.)),
        ((1., 1., 1.), (-1., 1., 1.)),
        ((-1., 1., 1.), (-1., -1., 1.)),
        ((-1., -1., -1.), (-1., -1., 1.)),
        ((1., -1., -1.), (1., -1., 1.)),
        ((1., 1., -1.), (1., 1., 1.)),
        ((-1., 1., -1.), (-1., 1., 1.)),
    ] {
        line(mesh, c(p.0, p.1, p.2), c(q.0, q.1, q.2), color);
    }
}

fn disc(mesh: &mut Mesh, center: Vec3, radius: f32, color: [f32; 3], segments: usize) {
    let (u, v) = basis(center);
    for i in 0..segments {
        let p = |i: usize| {
            let t = i as f32 / segments as f32 * std::f32::consts::TAU;
            center + (u * t.cos() + v * t.sin()) * radius
        };
        mesh.faces.push(vertex(center, color, 0.0));
        mesh.faces.push(vertex(p(i), color, 0.0));
        mesh.faces.push(vertex(p(i + 1), color, 0.0));
    }
}

/// A body drawn at the sky distance with its true angular radius.
fn body_disc(mesh: &mut Mesh, vector: Vec3, radius_m: f64, color: [f32; 3], earth: bool) {
    let distance = f64::from(vector.length());
    let dir = vector.normalize();
    let at = dir * SKY;
    let radius = (radius_m / distance) as f32 * SKY;
    disc(mesh, at, radius, color, 28);
    if earth {
        // Seen from L1 the Earth is always nearly full: its day side faces us.
        let (u, v) = basis(at);
        for (x, y, r) in [
            (0.3, 0.4, 0.25),
            (-0.4, -0.1, 0.3),
            (0.1, -0.5, 0.2),
            (-0.2, 0.55, 0.18),
        ] {
            disc(
                mesh,
                at - dir * 0.5 + (u * x + v * y) * radius,
                r * radius,
                [0.95, 0.95, 0.97],
                10,
            );
        }
        disc(
            mesh,
            at - dir * 0.3 + (u * -0.1 + v * 0.1) * radius,
            radius * 0.3,
            [0.25, 0.45, 0.2],
            10,
        );
    }
}

fn sun(mesh: &mut Mesh) {
    let at = Vec3::new(0.0, 0.0, -SKY);
    // 1 AU minus the station's 1.5 million km sunward offset.
    let distance = verse_lagrange::orbit::AU * (1.0 - verse_lagrange::orbit::mu()) - 1.497e9;
    let radius = (SUN_RADIUS / distance) as f32 * SKY;
    disc(mesh, at, radius, [3.0, 2.9, 2.6], 36);
    for i in 0..12 {
        let t = i as f32 / 12.0 * std::f32::consts::TAU;
        let d = Vec3::new(t.cos(), t.sin(), 0.0);
        line(
            mesh,
            at + d * radius * 1.2,
            at + d * radius * if i % 2 == 0 { 4.5 } else { 2.6 },
            [1.6, 1.3, 0.8],
        );
    }
}

fn stars(mesh: &mut Mesh) {
    let mut seed: u64 = 0x5eed_1a61_2026;
    let mut next = || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) as f32) / (1u64 << 31) as f32
    };
    for _ in 0..900 {
        let z = next() * 2.0 - 1.0;
        let t = next() * std::f32::consts::TAU;
        let r = (1.0 - z * z).sqrt();
        let dir = Vec3::new(r * t.cos(), z, r * t.sin());
        // Keep the Sun's neighborhood dark; glare hides stars there.
        if dir.z < -0.97 {
            continue;
        }
        let b = 0.25 + next().powi(3) * 0.9;
        let tint = next();
        let color = [b * (0.85 + 0.15 * tint), b * 0.9, b * (1.05 - 0.2 * tint)];
        let at = dir * 1_900.0;
        let (u, v) = basis(at);
        let s = 1.4 + b * 2.0;
        mesh.faces.push(vertex(at + u * s, color, 0.0));
        mesh.faces.push(vertex(at + v * s, color, 0.0));
        mesh.faces.push(vertex(at - u * s, color, 0.0));
    }
}

fn suit(mesh: &mut Mesh, feet: Vec3, yaw: f32, carrying: bool) {
    let t = Mat4::from_translation(feet) * Mat4::from_rotation_y(yaw);
    let part = |mesh: &mut Mesh, center: Vec3, half: Vec3, color: [f32; 3]| {
        shaded_box_at(mesh, t * Mat4::from_translation(center), half, color);
    };
    for x in [-0.14, 0.14] {
        part(
            mesh,
            Vec3::new(x, 0.42, 0.0),
            Vec3::new(0.11, 0.42, 0.12),
            WHITE,
        );
    }
    part(
        mesh,
        Vec3::new(0.0, 1.12, 0.0),
        Vec3::new(0.3, 0.33, 0.18),
        WHITE,
    );
    part(
        mesh,
        Vec3::new(0.0, 1.62, 0.02),
        Vec3::new(0.17, 0.17, 0.17),
        WHITE,
    );
    part(
        mesh,
        Vec3::new(0.0, 1.62, 0.17),
        Vec3::new(0.13, 0.1, 0.02),
        GOLD,
    );
    // Maneuvering pack with its nitrogen tanks.
    part(
        mesh,
        Vec3::new(0.0, 1.18, -0.36),
        Vec3::new(0.34, 0.42, 0.17),
        [0.62, 0.63, 0.66],
    );
    for x in [-0.4, 0.4] {
        part(
            mesh,
            Vec3::new(x, 1.35, -0.3),
            Vec3::new(0.06, 0.06, 0.2),
            METAL,
        );
    }
    for x in [-0.4, 0.4] {
        let (center, half) = if carrying {
            (Vec3::new(x, 1.2, 0.35), Vec3::new(0.08, 0.08, 0.36))
        } else {
            (Vec3::new(x, 1.0, 0.02), Vec3::new(0.08, 0.32, 0.08))
        };
        part(mesh, center, half, WHITE);
    }
}

fn part_mesh(mesh: &mut Mesh, kind: PartKind, t: Mat4) {
    let half = kind.size().as_vec3() * 0.5;
    match kind {
        PartKind::MainEngine => {
            shaded_box_at(
                mesh,
                t * Mat4::from_translation(Vec3::Z * 1.0),
                Vec3::new(0.8, 0.8, 0.5),
                METAL,
            );
            // Regeneratively cooled bell, flaring aft.
            let bell_center = t.transform_point3(Vec3::Z * -0.4);
            let axis = t.transform_vector3(Vec3::Z);
            frustum(
                mesh,
                bell_center,
                axis,
                1.05,
                0.4,
                2.2,
                18,
                [0.62, 0.36, 0.22],
            );
        }
        PartKind::PropellantTank => {
            let center = t.transform_point3(Vec3::ZERO);
            let axis = t.transform_vector3(Vec3::Z);
            cylinder(mesh, center, axis, 1.3, 3.2, 20, WHITE);
            frustum(mesh, center + axis * 1.8, axis, 1.3, 0.5, 0.4, 20, WHITE);
            frustum(mesh, center - axis * 1.8, axis, 0.5, 1.3, 0.4, 20, WHITE);
        }
        PartKind::KeelTrussAft | PartKind::KeelTrussFore => {
            for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                shaded_box_at(
                    mesh,
                    t * Mat4::from_translation(Vec3::new(
                        x * (half.x - 0.06),
                        y * (half.y - 0.06),
                        0.0,
                    )),
                    Vec3::new(0.06, 0.06, half.z),
                    METAL,
                );
            }
            for i in 0..4 {
                let z0 = -half.z + i as f32 * half.z / 2.0;
                let p = |x: f32, y: f32, z: f32| {
                    t.transform_point3(Vec3::new(x * half.x, y * half.y, z))
                };
                line(
                    mesh,
                    p(1.0, 1.0, z0),
                    p(-1.0, 1.0, z0 + half.z / 2.0),
                    METAL,
                );
                line(
                    mesh,
                    p(1.0, -1.0, z0),
                    p(-1.0, -1.0, z0 + half.z / 2.0),
                    METAL,
                );
                line(
                    mesh,
                    p(1.0, 1.0, z0),
                    p(1.0, -1.0, z0 + half.z / 2.0),
                    METAL,
                );
                line(
                    mesh,
                    p(-1.0, 1.0, z0),
                    p(-1.0, -1.0, z0 + half.z / 2.0),
                    METAL,
                );
            }
        }
        PartKind::RcsPod => {
            shaded_box_at(mesh, t, Vec3::new(0.5, 0.4, 0.5), WHITE);
            for x in [-1.0, 1.0] {
                shaded_box_at(
                    mesh,
                    t * Mat4::from_translation(Vec3::new(x * 0.9, 0.0, 0.0)),
                    Vec3::new(0.3, 0.25, 0.25),
                    METAL,
                );
            }
        }
        PartKind::AvionicsBay => {
            shaded_box_at(mesh, t, half * 0.95, GOLD);
            shaded_box_at(
                mesh,
                t * Mat4::from_translation(Vec3::Y * 0.75),
                Vec3::new(0.6, 0.02, 0.6),
                [0.9, 0.92, 0.95],
            );
        }
    }
}
