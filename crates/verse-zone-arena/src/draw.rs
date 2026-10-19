//! The arena's geometry in the Grid's hidden-line look: near-black solids
//! with bright edges. The player's robot is drawn in white light and the
//! enemy in amber, so the two read apart at any distance; effects take their
//! robot's color, and the shield a cool white.

use std::f32::consts::{PI, TAU};

use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};
use verse_gfx::palette;
use verse_pbr::mesh::{Mesh, Vertex};

use crate::rules::{
    ARENA_RADIUS, Effect, Fight, Flash, Outcome, Robot, SLAM_REACH, SLAM_WINDUP, ShotKind, Side,
};

/// The four steps of a robot's line color.
#[derive(Clone, Copy)]
struct Tone {
    full: [f32; 3],
    half: [f32; 3],
    glow: [f32; 3],
}

fn white(step: Intensity) -> [f32; 3] {
    palette::neutral(palette::amber(step))
}

fn tone(side: Side) -> Tone {
    match side {
        Side::Player => Tone {
            full: white(Intensity::Full),
            half: white(Intensity::Half),
            glow: [1.4, 1.4, 1.4],
        },
        Side::Enemy => Tone {
            full: crate::enemy_amber(1.0),
            half: crate::enemy_amber(0.32),
            glow: crate::enemy_amber(1.5),
        },
    }
}

/// The shield's cool white.
const SHIELD: [f32; 3] = [0.55, 0.8, 1.0];

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

/// A box with near-black faces and `color` edges; `transform` maps the
/// unit cube centered on the origin into the world.
fn block(mesh: &mut Mesh, transform: Mat4, color: [f32; 3]) {
    let c: [Vec3; 8] = std::array::from_fn(|i| {
        let x = if i & 1 == 0 { -0.5 } else { 0.5 };
        let y = if i & 2 == 0 { -0.5 } else { 0.5 };
        let z = if i & 4 == 0 { -0.5 } else { 0.5 };
        transform.transform_point3(Vec3::new(x, y, z))
    });
    let face = palette::field();
    for [a, b, d, e] in [
        [0, 1, 3, 2],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 3, 7, 5],
    ] {
        let [a, b, d, e] = [c[a], c[b], c[d], c[e]].map(|p| vertex(p, face));
        mesh.faces.extend_from_slice(&[a, b, d, a, d, e]);
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
        line(mesh, c[a], c[b], color);
    }
}

/// A horizontal circle of `radius` around `center`.
fn ring(mesh: &mut Mesh, center: Vec3, radius: f32, segments: usize, color: [f32; 3]) {
    let point = |i: usize| {
        let a = i as f32 / segments as f32 * TAU;
        center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius)
    };
    for i in 0..segments {
        line(mesh, point(i), point(i + 1), color);
    }
}

/// A circle of `radius` around `center` in the plane through `axis_a` and
/// `axis_b`.
fn circle(mesh: &mut Mesh, center: Vec3, axes: (Vec3, Vec3), radius: f32, color: [f32; 3]) {
    let (a, b) = axes;
    let point = |i: usize| {
        let t = i as f32 / 32.0 * TAU;
        center + (a * t.cos() + b * t.sin()) * radius
    };
    for i in 0..32 {
        line(mesh, point(i), point(i + 1), color);
    }
}

/// The arena floor: two bright rings at the edge, a faint ring at the
/// middle, the start marks, and eight pylons standing outside the ring.
#[must_use]
pub fn world() -> Mesh {
    let mut mesh = Mesh::default();
    let full = white(Intensity::Full);
    let half = white(Intensity::Half);
    let quarter = white(Intensity::Quarter);
    ring(&mut mesh, Vec3::Y * 0.01, ARENA_RADIUS, 128, full);
    ring(&mut mesh, Vec3::Y * 0.01, ARENA_RADIUS + 0.4, 128, half);
    ring(&mut mesh, Vec3::Y * 0.01, 6.0, 64, quarter);
    for i in 0..16 {
        let a = i as f32 / 16.0 * TAU;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        line(
            &mut mesh,
            dir * (ARENA_RADIUS - 1.2) + Vec3::Y * 0.01,
            dir * ARENA_RADIUS + Vec3::Y * 0.01,
            half,
        );
    }
    for i in 0..8 {
        let a = (i as f32 + 0.5) / 8.0 * TAU;
        let at = Vec3::new(a.cos(), 0.0, a.sin()) * (ARENA_RADIUS + 3.0);
        let yaw = Quat::from_rotation_y(-a);
        block(
            &mut mesh,
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.8, 5.0, 0.8),
                yaw,
                at + Vec3::Y * 2.5,
            ),
            half,
        );
        block(
            &mut mesh,
            Mat4::from_scale_rotation_translation(
                Vec3::new(1.2, 0.3, 1.2),
                yaw,
                at + Vec3::Y * 5.15,
            ),
            full,
        );
    }
    for z in [crate::rules::PLAYER_START.z, crate::rules::ENEMY_START.z] {
        let c = Vec3::new(0.0, 0.01, z);
        line(&mut mesh, c - Vec3::X * 0.8, c + Vec3::X * 0.8, quarter);
        line(&mut mesh, c - Vec3::Z * 0.8, c + Vec3::Z * 0.8, quarter);
    }
    mesh
}

/// One limb or plate: a box in the robot's frame, feet at the origin and
/// facing +Z, and which line step it takes.
struct Part {
    transform: Mat4,
    bright: bool,
}

fn part(center: Vec3, size: Vec3, bright: bool) -> Part {
    Part {
        transform: Mat4::from_scale_rotation_translation(size, Quat::IDENTITY, center),
        bright,
    }
}

/// A box hanging `length` below `pivot`, swung `angle` about X.
fn swung(pivot: Vec3, angle: f32, offset: Vec3, size: Vec3, bright: bool) -> Part {
    Part {
        transform: Mat4::from_translation(pivot)
            * Mat4::from_rotation_x(angle)
            * Mat4::from_scale_rotation_translation(size, Quat::IDENTITY, offset),
        bright,
    }
}

/// A leg from `hip`: thigh, shin, and foot, with the knee bending back as
/// the leg swings.
fn leg(parts: &mut Vec<Part>, hip: Vec3, swing: f32, thigh: Vec3, shin: Vec3, foot: Vec3) {
    let knee = hip + Quat::from_rotation_x(swing) * Vec3::new(0.0, -thigh.y, 0.0);
    parts.push(swung(
        hip,
        swing,
        Vec3::new(0.0, -thigh.y * 0.5, 0.0),
        thigh,
        false,
    ));
    let bend = swing - swing.abs() * 0.6;
    parts.push(swung(
        knee,
        bend,
        Vec3::new(0.0, -shin.y * 0.5, 0.0),
        shin,
        true,
    ));
    let ankle = knee + Quat::from_rotation_x(bend) * Vec3::new(0.0, -shin.y, 0.0);
    parts.push(part(
        Vec3::new(ankle.x, foot.y * 0.5, ankle.z + foot.z * 0.2),
        foot,
        false,
    ));
}

/// The player's robot, the Striker: slim and upright, a long laser barrel
/// on its right arm and a missile pack on its back.
fn striker(stride: f32, moving: f32) -> (Vec<Part>, Vec<(Vec3, Vec3)>) {
    let mut parts = Vec::new();
    let phase = stride * 2.4;
    let swing = phase.sin() * 0.55 * moving;
    let bob = (phase * 2.0).cos().abs() * 0.05 * moving;
    let lift = Vec3::Y * bob;
    let hip = 1.15;
    for s in [-1.0_f32, 1.0] {
        leg(
            &mut parts,
            Vec3::new(s * 0.28, hip, 0.0) + lift,
            swing * s,
            Vec3::new(0.22, 0.6, 0.26),
            Vec3::new(0.2, 0.55, 0.24),
            Vec3::new(0.3, 0.1, 0.5),
        );
    }
    parts.push(part(
        Vec3::new(0.0, hip + 0.05, 0.0) + lift,
        Vec3::new(0.8, 0.3, 0.5),
        false,
    ));
    parts.push(part(
        Vec3::new(0.0, 1.65, 0.0) + lift,
        Vec3::new(0.95, 0.72, 0.6),
        true,
    ));
    parts.push(part(
        Vec3::new(0.0, 1.75, 0.33) + lift,
        Vec3::new(0.56, 0.36, 0.08),
        false,
    ));
    parts.push(part(
        Vec3::new(0.0, 2.25, 0.02) + lift,
        Vec3::new(0.42, 0.32, 0.42),
        true,
    ));
    parts.push(part(
        Vec3::new(0.0, 1.88, -0.42) + lift,
        Vec3::new(0.7, 0.45, 0.25),
        false,
    ));
    for s in [-1.0_f32, 1.0] {
        parts.push(part(
            Vec3::new(s * 0.64, 1.95, 0.0) + lift,
            Vec3::new(0.36, 0.3, 0.46),
            true,
        ));
        // Arms swing against the legs.
        let arm = -swing * s * 0.4;
        parts.push(swung(
            Vec3::new(s * 0.68, 1.85, 0.0) + lift,
            arm,
            Vec3::new(0.0, -0.25, 0.0),
            Vec3::new(0.2, 0.5, 0.22),
            false,
        ));
    }
    // The laser barrel, held level, and the left fist.
    parts.push(part(
        Vec3::new(0.62, 1.42, 0.3) + lift,
        Vec3::new(0.2, 0.2, 0.8),
        true,
    ));
    parts.push(part(
        Vec3::new(-0.68, 1.3, 0.08) + lift,
        Vec3::new(0.24, 0.24, 0.3),
        false,
    ));
    // The visor and the missile tubes' mouths, as glowing lines.
    let mut glow = vec![
        (
            Vec3::new(-0.16, 2.27, 0.235) + lift,
            Vec3::new(0.16, 2.27, 0.235) + lift,
        ),
        (
            Vec3::new(-0.12, 2.22, 0.235) + lift,
            Vec3::new(0.12, 2.22, 0.235) + lift,
        ),
        (
            Vec3::new(0.62, 1.42, 0.71) + lift,
            Vec3::new(0.62, 1.42, 0.72) + lift,
        ),
    ];
    for x in [-0.24, -0.08, 0.08, 0.24] {
        glow.push((
            Vec3::new(x - 0.05, 2.111, -0.42) + lift,
            Vec3::new(x + 0.05, 2.111, -0.42) + lift,
        ));
    }
    (parts, glow)
}

/// The enemy, the Brute: a wide hull on short legs, a single eye slit,
/// heavy pauldrons, and a plasma cannon on its right fist. `raise` lifts
/// its arms for the slam, 0 to 1.
fn brute(stride: f32, moving: f32, raise: f32) -> (Vec<Part>, Vec<(Vec3, Vec3)>) {
    let mut parts = Vec::new();
    let phase = stride * 1.9;
    let swing = phase.sin() * 0.4 * moving;
    let hip = 1.45;
    for s in [-1.0_f32, 1.0] {
        leg(
            &mut parts,
            Vec3::new(s * 0.52, hip, 0.0),
            swing * s,
            Vec3::new(0.42, 0.75, 0.48),
            Vec3::new(0.38, 0.7, 0.44),
            Vec3::new(0.58, 0.16, 0.8),
        );
    }
    parts.push(part(
        Vec3::new(0.0, hip, 0.0),
        Vec3::new(1.0, 0.35, 0.7),
        false,
    ));
    parts.push(part(
        Vec3::new(0.0, 2.05, 0.0),
        Vec3::new(1.7, 0.95, 1.2),
        true,
    ));
    parts.push(part(
        Vec3::new(0.0, 2.6, -0.15),
        Vec3::new(0.9, 0.2, 0.7),
        false,
    ));
    for s in [-1.0_f32, 1.0] {
        parts.push(part(
            Vec3::new(s * 1.08, 2.4, 0.0),
            Vec3::new(0.56, 0.5, 0.82),
            true,
        ));
        let shoulder = Vec3::new(s * 1.12, 2.15, 0.0);
        let arm = -raise * 2.4 - swing * s * 0.3;
        parts.push(swung(
            shoulder,
            arm,
            Vec3::new(0.0, -0.35, 0.0),
            Vec3::new(0.42, 0.7, 0.46),
            false,
        ));
        parts.push(swung(
            shoulder,
            arm,
            Vec3::new(0.0, -0.95, 0.1),
            Vec3::new(0.62, 0.6, 0.66),
            true,
        ));
    }
    let shoulder = Vec3::new(1.12, 2.15, 0.0);
    parts.push(swung(
        shoulder,
        -raise * 2.4 - swing * 0.3,
        Vec3::new(0.0, -0.95, 0.65),
        Vec3::new(0.26, 0.26, 0.6),
        true,
    ));
    let glow = vec![
        (Vec3::new(-0.3, 2.18, 0.61), Vec3::new(0.3, 2.18, 0.61)),
        (Vec3::new(-0.22, 2.12, 0.61), Vec3::new(0.22, 2.12, 0.61)),
        (Vec3::new(0.0, 2.7, -0.3), Vec3::new(0.0, 3.3, -0.4)),
    ];
    (parts, glow)
}

/// The enemy is drawn larger than its parts, about 3.1 m tall.
pub const BRUTE_SCALE: f32 = 1.2;

/// A robot's mesh at its pose: hit flash, stun dimming, and a fallen pose
/// once destroyed.
fn robot(mesh: &mut Mesh, robot: &Robot, now: f32, raise: f32) {
    let t = tone(robot.side);
    let moving = (robot.speed / 4.0).clamp(0.0, 1.0);
    let (parts, glow) = match robot.side {
        Side::Player => striker(robot.stride, moving),
        Side::Enemy => brute(robot.stride, moving, raise),
    };
    let scale = match robot.side {
        Side::Player => 1.0,
        Side::Enemy => BRUTE_SCALE,
    };
    let mut root = Mat4::from_scale_rotation_translation(
        Vec3::splat(scale),
        Quat::from_rotation_y(robot.yaw),
        robot.pos,
    );
    if !robot.alive() {
        // Toppled backward onto the floor.
        root = root
            * Mat4::from_translation(Vec3::new(0.0, 0.35, -0.3))
            * Mat4::from_rotation_x(-1.35);
    }
    let flash = now - robot.hit_at < 0.12;
    let stunned = robot.stunned(now);
    let (full, half) = if flash {
        (t.glow, t.full)
    } else if stunned || !robot.alive() {
        (t.half, t.half)
    } else {
        (t.full, t.half)
    };
    for p in parts {
        block(mesh, root * p.transform, if p.bright { full } else { half });
    }
    let glow_color = if robot.alive() { t.glow } else { t.half };
    for (a, b) in glow {
        line(
            mesh,
            root.transform_point3(a),
            root.transform_point3(b),
            glow_color,
        );
    }
    if stunned && robot.alive() {
        // Sparks circling over the head.
        let top = robot.pos + Vec3::Y * (2.6 * scale + 0.3);
        for i in 0..3 {
            let a = now * 5.0 + i as f32 * TAU / 3.0;
            let p = top + Vec3::new(a.cos(), 0.0, a.sin()) * 0.5;
            line(mesh, p - Vec3::Y * 0.12, p + Vec3::Y * 0.12, t.glow);
            line(mesh, p - Vec3::X * 0.12, p + Vec3::X * 0.12, t.glow);
        }
        ring(mesh, top, 0.5, 24, t.half);
    }
}

/// A wire sphere for a shield, pulsing with the clock.
fn sphere(mesh: &mut Mesh, center: Vec3, radius: f32, color: [f32; 3], now: f32) {
    let r = radius * (1.0 + 0.03 * (now * 6.0).sin());
    for k in [-0.6_f32, -0.2, 0.2, 0.6] {
        let y = r * k;
        ring(
            mesh,
            center + Vec3::Y * y,
            (r * r - y * y).sqrt(),
            32,
            color,
        );
    }
    let spin = now * 0.8;
    for i in 0..3 {
        let a = spin + i as f32 * PI / 3.0;
        circle(
            mesh,
            center,
            (Vec3::new(a.cos(), 0.0, a.sin()), Vec3::Y),
            r,
            color,
        );
    }
}

/// Everything that moves this frame: both robots and their rings, the
/// shields, the slam's warning, the shots in flight, and the effects.
#[must_use]
pub fn dynamic(fight: &Fight) -> Mesh {
    let mut mesh = Mesh::default();
    let now = fight.now;
    let raise = fight
        .brain
        .windup
        .map_or(0.0, |w| ((now - w.started) / SLAM_WINDUP).clamp(0.0, 1.0));
    robot(&mut mesh, &fight.player, now, 0.0);
    robot(&mut mesh, &fight.enemy, now, raise);
    let p = tone(Side::Player);
    let e = tone(Side::Enemy);
    // The player's ring, as the Grid draws under its figure.
    ring(
        &mut mesh,
        fight.player.pos.with_y(0.02),
        0.9,
        40,
        white(Intensity::Half),
    );
    // The target ring under the enemy, with ticks turning around it.
    if fight.enemy.alive() {
        let c = fight.enemy.pos.with_y(0.03);
        ring(&mut mesh, c, 1.7, 48, e.full);
        ring(&mut mesh, c, 1.9, 48, e.half);
        for i in 0..4 {
            let a = now * 1.2 + i as f32 * TAU / 4.0;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            line(&mut mesh, c + d * 1.95, c + d * 2.35, e.full);
        }
    }
    // The slam's warning: its reach on the floor, filling as it winds up.
    if fight.brain.windup.is_some() {
        let c = fight.enemy.pos.with_y(0.04);
        ring(&mut mesh, c, SLAM_REACH, 64, e.full);
        ring(&mut mesh, c, SLAM_REACH * raise.max(0.05), 64, e.glow);
    }
    for robot in [&fight.player, &fight.enemy] {
        if robot.shield_at(now) > 0.0 && robot.alive() {
            let (radius, color) = match robot.side {
                Side::Player => (1.5, SHIELD),
                Side::Enemy => (2.4, e.half),
            };
            sphere(
                &mut mesh,
                robot.pos + Vec3::Y * radius * 0.95,
                radius,
                color,
                now,
            );
        }
    }
    for shot in &fight.shots {
        let dir = shot.vel.normalize_or_zero();
        match shot.kind {
            ShotKind::Missile => {
                let rot =
                    Quat::from_rotation_arc(Vec3::Z, if dir == Vec3::ZERO { Vec3::Z } else { dir });
                block(
                    &mut mesh,
                    Mat4::from_scale_rotation_translation(
                        Vec3::new(0.12, 0.12, 0.45),
                        rot,
                        shot.pos,
                    ),
                    p.glow,
                );
                line(&mut mesh, shot.pos, shot.pos - dir * 1.4, p.half);
            }
            ShotKind::Bolt => {
                let spin = Quat::from_euler(glam::EulerRot::YXZ, now * 9.0, PI / 4.0, PI / 4.0);
                block(
                    &mut mesh,
                    Mat4::from_scale_rotation_translation(Vec3::splat(0.32), spin, shot.pos),
                    e.glow,
                );
                line(&mut mesh, shot.pos, shot.pos - dir * 1.8, e.half);
            }
        }
    }
    for flash in &fight.flashes {
        effect(&mut mesh, flash, now);
    }
    if fight.outcome == Outcome::Won {
        // The fallen enemy's last sparks.
        let c = fight.enemy.pos + Vec3::Y * 0.6;
        let t = (now * 3.0).fract();
        ring(&mut mesh, c, 1.0 + t * 2.0, 32, e.half);
    }
    mesh
}

fn effect(mesh: &mut Mesh, flash: &Flash, now: f32) {
    let t = ((now - flash.at) / flash.life()).clamp(0.0, 1.0);
    let fade = 1.0 - t;
    let dim = |c: [f32; 3]| c.map(|v| v * (0.35 + 0.65 * fade));
    match flash.effect {
        Effect::Beam { from, to, hit } => {
            let core = if hit {
                tone(Side::Player).glow
            } else {
                tone(Side::Player).half
            };
            line(mesh, from, to, dim(core));
            let d = (to - from).normalize_or_zero();
            let side = d.cross(Vec3::Y).normalize_or_zero() * 0.05;
            let up = side.cross(d).normalize_or_zero() * 0.05;
            for o in [side, -side, up, -up] {
                line(mesh, from + o, to + o, dim(tone(Side::Player).half));
            }
            if hit {
                for i in 0..6 {
                    let a = i as f32 / 6.0 * TAU + now * 7.0;
                    let s = Vec3::new(a.cos(), a.sin() * 0.7, a.sin()) * (0.3 + 0.5 * t);
                    line(mesh, to, to + s, dim(core));
                }
            }
        }
        Effect::Blast { at, radius, side } => {
            let tn = tone(side);
            let r = radius * (0.3 + 0.7 * t);
            ring(mesh, at, r, 32, dim(tn.glow));
            ring(mesh, at.with_y(0.03), r * 0.8, 32, dim(tn.half));
            circle(mesh, at, (Vec3::X, Vec3::Y), r * 0.7, dim(tn.full));
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                let d = Vec3::new(a.cos(), 0.6 * (i % 2) as f32, a.sin());
                line(mesh, at + d * r * 0.4, at + d * r, dim(tn.full));
            }
        }
        Effect::Pulse { at, radius } => {
            let r = radius * (0.2 + 0.8 * t);
            ring(mesh, at.with_y(0.05), r, 64, dim(SHIELD));
            ring(mesh, at.with_y(0.6), r * 0.9, 64, dim(SHIELD));
            ring(mesh, at.with_y(1.2), r * 0.75, 48, dim(SHIELD));
            for i in 0..12 {
                let a = i as f32 / 12.0 * TAU + t;
                let d = Vec3::new(a.cos(), 0.0, a.sin());
                let base = at.with_y(0.05) + d * r;
                line(mesh, base, base + Vec3::Y * 0.8 * fade, dim(SHIELD));
            }
        }
        Effect::Slam { at, radius } => {
            let tn = tone(Side::Enemy);
            let r = radius * (0.4 + 0.6 * t);
            ring(mesh, at.with_y(0.04), r, 64, dim(tn.glow));
            ring(mesh, at.with_y(0.04), r * 0.7, 64, dim(tn.full));
            for i in 0..10 {
                let a = i as f32 / 10.0 * TAU + 0.3;
                let d = Vec3::new(a.cos(), 0.0, a.sin());
                let mid = d.cross(Vec3::Y) * 0.3 * ((i % 3) as f32 - 1.0);
                let p0 = at.with_y(0.04) + d * 1.2;
                let p1 = p0 + d * r * 0.5 + mid;
                let p2 = p1 + d * r * 0.4 - mid;
                line(mesh, p0, p1, dim(tn.full));
                line(mesh, p1, p2, dim(tn.half));
            }
        }
    }
}
