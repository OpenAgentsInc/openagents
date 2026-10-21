//! The Hand Duel in the Grid's hidden-line look: near-black solids with
//! bright edges. The player's shield, sword, and hands are drawn in white
//! light, the Barrow Warden in amber, and his tell in a brighter amber, so
//! a blow's line reads at a glance.

use std::f32::consts::{PI, TAU};

use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};
use openagents_web_hands::{CONNECTIONS, Landmark};
use verse_gfx::palette;
use verse_pbr::mesh::{Mesh, Vertex};

use crate::body::{Side, picture_to_body};
use crate::gesture::{BLADE, Potion, blade_tip, shield_cover};
use crate::rules::{
    Blow, Fight, Outcome, Phase, RECOVER, SPARK_LIFE, WARDEN_REACH, WARDEN_START, WINDUP,
};
use crate::{HandDuel, enemy_amber};

fn white(step: Intensity) -> [f32; 3] {
    palette::neutral(palette::amber(step))
}

fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    c.map(|v| v * k)
}

/// A shield stop's cool white.
const STEEL: [f32; 3] = [0.55, 0.8, 1.0];
/// The drawn hand's size against its tracked size.
const BONE_SCALE: f32 = 0.35;
/// A potion's red.
const POTION: [f32; 3] = [0.95, 0.16, 0.12];

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

/// A circle of `radius` around `center` in the plane of `axes`.
fn circle(
    mesh: &mut Mesh,
    center: Vec3,
    axes: (Vec3, Vec3),
    radius: f32,
    segments: usize,
    color: [f32; 3],
) {
    let (a, b) = axes;
    let point = |i: usize| {
        let t = i as f32 / segments as f32 * TAU;
        center + (a * t.cos() + b * t.sin()) * radius
    };
    for i in 0..segments {
        line(mesh, point(i), point(i + 1), color);
    }
}

/// Two unit axes across `normal`.
fn across(normal: Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize_or(Vec3::Z);
    let helper = if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let a = helper.cross(n).normalize();
    (a, n.cross(a))
}

/// The barrow field: rings on the ground, broken standing stones, and the
/// barrow's doorway behind the Warden's start.
#[must_use]
pub fn world() -> Mesh {
    let mut mesh = Mesh::default();
    let full = white(Intensity::Full);
    let half = white(Intensity::Half);
    let quarter = white(Intensity::Quarter);
    let flat = (Vec3::X, Vec3::Z);
    let center = Vec3::new(0.0, 0.01, 1.0);
    circle(&mut mesh, center, flat, 6.0, 96, half);
    circle(&mut mesh, center, flat, 6.4, 96, quarter);
    // The Warden's path from the barrow.
    for i in 0..12 {
        let z = WARDEN_REACH + i as f32 * 0.9;
        line(
            &mut mesh,
            Vec3::new(-0.6, 0.01, z),
            Vec3::new(-0.6, 0.01, z + 0.45),
            quarter,
        );
        line(
            &mut mesh,
            Vec3::new(0.6, 0.01, z),
            Vec3::new(0.6, 0.01, z + 0.45),
            quarter,
        );
    }
    // Standing stones, some broken.
    for i in 0..9 {
        let a = (i as f32 + 0.5) / 9.0 * TAU + 0.2;
        let at = Vec3::new(a.cos() * 11.0, 0.0, a.sin() * 11.0 + 4.0);
        if at.z < -2.0 {
            continue;
        }
        let tall = if i % 3 == 0 { 1.4 } else { 3.2 };
        block(
            &mut mesh,
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.9, tall, 0.6),
                Quat::from_rotation_y(-a) * Quat::from_rotation_z(0.04 * (i as f32 - 4.0)),
                at + Vec3::Y * tall * 0.5,
            ),
            half,
        );
    }
    // The barrow's doorway.
    let door = Vec3::new(0.0, 0.0, WARDEN_START + 3.0);
    for x in [-1.3, 1.3] {
        block(
            &mut mesh,
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.7, 3.0, 0.7),
                Quat::IDENTITY,
                door + Vec3::new(x, 1.5, 0.0),
            ),
            full,
        );
    }
    block(
        &mut mesh,
        Mat4::from_scale_rotation_translation(
            Vec3::new(3.6, 0.6, 0.9),
            Quat::IDENTITY,
            door + Vec3::Y * 3.3,
        ),
        full,
    );
    // The mound over it.
    for k in 0..5 {
        let r = 4.5 + k as f32 * 0.9;
        let h = 4.2 - k as f32 * 0.7;
        let c = door + Vec3::new(0.0, 0.0, 2.5);
        let mut last = None;
        for i in 0..=24 {
            let t = i as f32 / 24.0 * PI;
            let p = c + Vec3::new(-t.cos() * r, t.sin() * h, 0.0);
            if let Some(q) = last {
                line(&mut mesh, q, p, if k == 0 { half } else { quarter });
            }
            last = Some(p);
        }
    }
    mesh
}

/// Where the Warden's glaive is: its head (the blade) and its butt.
fn glaive(fight: &Fight) -> (Vec3, Vec3) {
    let w = fight.warden_pos();
    let rest = (
        w + Vec3::new(0.7, 2.5, -0.3),
        w + Vec3::new(-0.2, 0.25, 0.15),
    );
    let wound = |blow: Blow| match blow {
        Blow::Overhead => (w + Vec3::new(0.2, 3.1, 0.3), w + Vec3::new(0.1, 1.0, -0.2)),
        Blow::Left => (w + Vec3::new(1.5, 2.3, 0.2), w + Vec3::new(-0.4, 1.0, -0.3)),
        Blow::Right => (w + Vec3::new(-1.5, 2.3, 0.2), w + Vec3::new(0.4, 1.0, -0.3)),
        Blow::Thrust => (
            w + Vec3::new(0.15, 1.35, 0.1),
            w + Vec3::new(0.2, 1.25, 1.7),
        ),
    };
    let landed = |blow: Blow| {
        let head = blow.impact() + Vec3::Z * 0.05;
        let butt = w + Vec3::new(0.0, 1.1, -0.1);
        (head, butt)
    };
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let lerp = |a: (Vec3, Vec3), b: (Vec3, Vec3), t: f32| (a.0.lerp(b.0, t), a.1.lerp(b.1, t));
    match fight.warden.phase {
        Phase::Windup { blow, start } => {
            let p = ((fight.now - start) / WINDUP).clamp(0.0, 1.0);
            let mut pose = lerp(rest, wound(blow), ease((p / 0.7).min(1.0)));
            // The last of the windup trembles: the blow is coming.
            if p > 0.7 {
                let k = (fight.now * 60.0).sin() * 0.03 * (p - 0.7) / 0.3;
                pose.0 += Vec3::new(k, k * 0.5, 0.0);
            }
            pose
        }
        Phase::Recover { until } => match fight.warden.struck {
            Some((blow, at)) if fight.now - at < 0.12 => landed(blow),
            Some((blow, _)) => {
                let total = until - fight.now;
                let t = (1.0 - total / RECOVER).clamp(0.0, 1.0);
                lerp(landed(blow), rest, ease(t))
            }
            None => rest,
        },
        Phase::Stagger { .. } => (w + Vec3::new(1.1, 1.6, 0.6), w + Vec3::new(-0.6, 0.6, -0.2)),
        Phase::Dead { .. } => (
            w + Vec3::new(1.4, 0.05, 0.6),
            w + Vec3::new(-0.6, 0.05, 0.1),
        ),
        Phase::Approach | Phase::Ready { .. } => rest,
    }
}

/// The Barrow Warden: a tall skeletal revenant in a ragged mantle, with a
/// hooked glaive, all in amber line.
fn warden(mesh: &mut Mesh, fight: &Fight) {
    let w = fight.warden_pos();
    let now = fight.now;
    let flash = now - fight.warden.hit_at < 0.12;
    let full = if flash {
        [1.6, 1.6, 1.6]
    } else {
        enemy_amber(1.0)
    };
    let half = enemy_amber(0.4);
    let dead = matches!(fight.warden.phase, Phase::Dead { .. });
    // Leaning back while staggered; flat once dead.
    let lean = match fight.warden.phase {
        Phase::Stagger { .. } => 0.25,
        _ => 0.0,
    };
    let walk = if fight.warden.phase == Phase::Approach {
        (now * 5.0).sin()
    } else {
        0.0
    };
    let body = |p: Vec3| -> Vec3 {
        if dead {
            // Fallen backward along +z.
            w + Vec3::new(p.x, 0.08 + p.z * 0.1, p.y * 0.9)
        } else {
            let leaned = Vec3::new(p.x, p.y, p.z + p.y * lean);
            w + leaned
        }
    };
    // Legs.
    for (side, step) in [(-0.18_f32, walk), (0.18, -walk)] {
        let hip = Vec3::new(side, 1.05, 0.0);
        let knee = Vec3::new(side * 1.1, 0.55, -0.08 + step * 0.15);
        let foot = Vec3::new(side * 1.2, 0.02, step * 0.25);
        line(mesh, body(hip), body(knee), full);
        line(mesh, body(knee), body(foot), full);
        line(mesh, body(foot), body(foot - Vec3::Z * 0.22), half);
    }
    // Pelvis, spine, and ribs.
    line(
        mesh,
        body(Vec3::new(-0.2, 1.05, 0.0)),
        body(Vec3::new(0.2, 1.05, 0.0)),
        full,
    );
    line(
        mesh,
        body(Vec3::new(0.0, 1.05, 0.0)),
        body(Vec3::new(0.0, 1.95, 0.0)),
        full,
    );
    for (i, y) in [1.35_f32, 1.5, 1.65, 1.8].iter().enumerate() {
        let r = 0.2 + i as f32 * 0.02;
        let mut last = None;
        for k in 0..=12 {
            let a = k as f32 / 12.0 * PI;
            let p = body(Vec3::new(-a.cos() * r, *y, -a.sin() * r * 0.6));
            if let Some(q) = last {
                line(mesh, q, p, if i % 2 == 0 { full } else { half });
            }
            last = Some(p);
        }
    }
    // The mantle: ragged lines from the shoulders.
    let shoulders = [Vec3::new(-0.34, 1.95, 0.0), Vec3::new(0.34, 1.95, 0.0)];
    line(mesh, body(shoulders[0]), body(shoulders[1]), full);
    for i in 0..9 {
        let x = -0.42 + i as f32 * 0.105;
        let sway = (now * 1.7 + i as f32).sin() * 0.04;
        let low = 1.0 + (i % 3) as f32 * 0.12;
        line(
            mesh,
            body(Vec3::new(x * 0.8, 1.95, 0.08)),
            body(Vec3::new(x, low, 0.18 + sway)),
            half,
        );
    }
    // The skull: a box, a jaw, and two eye glows.
    let skull = Vec3::new(0.0, 2.2, 0.0);
    if dead {
        line(mesh, body(skull), body(skull + Vec3::Y * 0.2), full);
    } else {
        block(
            mesh,
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.26, 0.3, 0.28),
                Quat::from_rotation_x(lean),
                body(skull),
            ),
            full,
        );
        let tell = fight.telegraph().map_or(0.0, |(_, p)| p);
        let glow = enemy_amber(1.6 + 2.5 * tell);
        for x in [-0.06, 0.06] {
            let eye = body(skull + Vec3::new(x, 0.03, -0.15));
            line(mesh, eye - Vec3::X * 0.035, eye + Vec3::X * 0.035, glow);
        }
        // A crown of three spikes.
        for x in [-0.09_f32, 0.0, 0.09] {
            let base = body(skull + Vec3::new(x, 0.15, 0.0));
            line(
                mesh,
                base,
                base + Vec3::Y * (0.12 + 0.06 * (x == 0.0) as u8 as f32),
                full,
            );
        }
    }
    // The glaive and the arms that hold it.
    let (head, butt) = glaive(fight);
    let shaft = (head - butt).normalize_or(Vec3::Y);
    line(mesh, butt, head, full);
    // The hooked blade.
    let side = shaft.cross(Vec3::Z).normalize_or(Vec3::X);
    let edge = head - shaft * 0.55;
    line(mesh, head, edge + side * 0.28, full);
    line(mesh, edge + side * 0.28, edge, full);
    line(mesh, head, head + shaft * 0.2 - side * 0.12, full);
    let grips = [butt.lerp(head, 0.3), butt.lerp(head, 0.55)];
    for (shoulder, grip) in shoulders.iter().zip(grips) {
        let s = body(*shoulder);
        let elbow = s.lerp(grip, 0.5) + Vec3::new(0.0, -0.18, -0.1);
        line(mesh, s, elbow, full);
        line(mesh, elbow, grip, full);
    }
}

/// The tell: where the coming blow will land, a ring that tightens and
/// brightens as it nears, with the line it will come along.
fn tell(mesh: &mut Mesh, fight: &Fight) {
    let Some((blow, p)) = fight.telegraph() else {
        return;
    };
    let at = blow.impact();
    let toward = (crate::EYE - at).normalize();
    let axes = across(toward);
    let color = enemy_amber(0.8 + 2.4 * p);
    let radius = 0.3 - 0.16 * p;
    circle(mesh, at, axes, radius, 32, color);
    circle(mesh, at, axes, radius * 0.4, 16, enemy_amber(0.5 + p));
    // The blow's line, dashed, from the glaive's head to the mark.
    let (head, _) = glaive(fight);
    for i in 0..10 {
        if i % 2 == 1 {
            continue;
        }
        let a = head.lerp(at, i as f32 / 10.0);
        let b = head.lerp(at, (i + 1) as f32 / 10.0);
        line(mesh, a, b, enemy_amber(0.6 + p));
    }
}

/// The round shield on the left forearm, facing out from the chest.
fn shield(mesh: &mut Mesh, duel: &HandDuel) {
    let fight = duel.fight();
    let full = white(Intensity::Full);
    let half = white(Intensity::Half);
    let blocked = fight
        .sparks
        .iter()
        .any(|s| s.blocked && fight.now - s.born < SPARK_LIFE);
    let (center, normal) = match duel.hand(Side::Left) {
        Some(hand) if duel.gestures().shield_up => (hand.pos + Vec3::Z * 0.04, shield_cover(&hand)),
        // Lowered: it hangs from the forearm, facing forward and down.
        Some(hand) => (
            hand.pos + Vec3::new(0.0, -0.15, 0.05),
            Vec3::new(0.2, -0.5, 1.0),
        ),
        // Out of sight: at the left side, low.
        None => (Vec3::new(0.62, 1.0, 0.25), Vec3::new(0.3, -0.4, 1.0)),
    };
    let rim = if blocked { STEEL } else { full };
    let axes = across(normal);
    circle(mesh, center, axes, 0.29, 40, rim);
    circle(mesh, center, axes, 0.255, 40, half);
    circle(mesh, center, axes, 0.07, 16, full);
    // An eight-rayed sun on the face, our own device.
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU + PI / 8.0;
        let dir = axes.0 * a.cos() + axes.1 * a.sin();
        line(mesh, center + dir * 0.08, center + dir * 0.23, half);
    }
    if blocked {
        circle(mesh, center, axes, 0.36, 40, STEEL);
    }
}

/// A potion flask in the left hand.
fn flask(mesh: &mut Mesh, at: Vec3, fill: f32) {
    let glass = white(Intensity::Full);
    let axes = (Vec3::X, Vec3::Z);
    circle(mesh, at, axes, 0.06, 16, glass);
    circle(mesh, at + Vec3::Y * 0.1, axes, 0.06, 16, glass);
    for i in 0..4 {
        let a = i as f32 / 4.0 * TAU;
        let r = Vec3::new(a.cos(), 0.0, a.sin()) * 0.06;
        line(mesh, at + r, at + r + Vec3::Y * 0.1, glass);
    }
    line(mesh, at + Vec3::Y * 0.1, at + Vec3::Y * 0.17, glass);
    circle(mesh, at + Vec3::Y * 0.17, axes, 0.02, 8, glass);
    // The draught inside, draining as it is drunk.
    let level = 0.09 * (1.0 - fill);
    circle(mesh, at + Vec3::Y * level, axes, 0.05, 16, POTION);
    line(mesh, at, at + Vec3::Y * level, POTION);
}

/// The sword in the right hand, and its trail through a swing.
fn sword(mesh: &mut Mesh, duel: &HandDuel) {
    let Some(hand) = duel.hand(Side::Right) else {
        return;
    };
    if !duel.gestures().drawn {
        return;
    }
    let full = white(Intensity::Full);
    let half = white(Intensity::Half);
    let tip = blade_tip(&hand);
    let guard = hand.pos + hand.up * 0.08;
    let pommel = hand.pos - hand.up * 0.1;
    let cross = hand.up.cross(Vec3::Z).normalize_or(Vec3::X) * 0.13;
    line(mesh, pommel, guard, half);
    line(mesh, guard - cross, guard + cross, full);
    // A two-edged blade, narrowing to the point.
    let edge = cross * 0.25;
    let near = guard + hand.up * 0.02;
    let shoulder = guard + hand.up * (BLADE - 0.2);
    line(mesh, near - edge, shoulder - edge * 0.8, full);
    line(mesh, near + edge, shoulder + edge * 0.8, full);
    line(mesh, shoulder - edge * 0.8, tip, full);
    line(mesh, shoulder + edge * 0.8, tip, full);
    line(mesh, near, tip, half);
    // The swing's trail fades behind the tip.
    let trail = duel.trail();
    let n = trail.len();
    for (i, pair) in trail.windows(2).enumerate() {
        let k = (i + 1) as f32 / n as f32;
        line(mesh, pair[0], pair[1], scale(full, 0.3 + 1.2 * k));
    }
}

/// The tracked hand's own bones, faint, so the player sees what the camera
/// reads.
fn bones(mesh: &mut Mesh, points: &[Landmark; 21], aspect: f32, reach: f32, color: [f32; 3]) {
    // Drawn at about a third of their size about the wrist, so the bones sit
    // inside the shield and the hilt rather than over the whole view.
    let wrist = picture_to_body(points[0].x, points[0].y, aspect);
    let at = |p: &Landmark| {
        let (x, y) = picture_to_body(p.x, p.y, aspect);
        Vec3::new(
            wrist.0 + (x - wrist.0) * BONE_SCALE,
            wrist.1 + (y - wrist.1) * BONE_SCALE,
            reach,
        )
    };
    for [a, b] in CONNECTIONS {
        line(mesh, at(&points[*a]), at(&points[*b]), color);
    }
}

/// Bursts at contact points: amber for a cut, cool white for a shield stop.
fn sparks(mesh: &mut Mesh, fight: &Fight) {
    for (n, spark) in fight.sparks.iter().enumerate() {
        let age = (fight.now - spark.born) / SPARK_LIFE;
        let color = if spark.blocked {
            scale(STEEL, 2.0 * (1.0 - age))
        } else {
            enemy_amber(2.5 * (1.0 - age))
        };
        for i in 0..10 {
            let a = (i as f32 + n as f32 * 0.37) * 2.39996;
            let dir = Vec3::new(a.cos(), (a * 1.7).sin(), -0.4 - (i % 3) as f32 * 0.2)
                .normalize_or(Vec3::Y);
            let from = spark.at + dir * (0.05 + 0.4 * age);
            line(mesh, from, from + dir * 0.12, color);
        }
    }
}

/// This frame's moving parts.
#[must_use]
pub fn dynamic(duel: &HandDuel) -> Mesh {
    let mut mesh = Mesh::default();
    let fight = duel.fight();
    warden(&mut mesh, fight);
    tell(&mut mesh, fight);
    shield(&mut mesh, duel);
    if let Some(hand) = duel.hand(Side::Left)
        && let Potion::Held { .. } = duel.gestures().potion
    {
        flask(
            &mut mesh,
            hand.pos + Vec3::Y * 0.02,
            duel.gestures().drink_progress(duel.hand_time()),
        );
    }
    sword(&mut mesh, duel);
    let faint = white(Intensity::Quarter);
    for side in [Side::Left, Side::Right] {
        if let (Some(points), Some(hand)) = (duel.points(side), duel.hand(side)) {
            bones(&mut mesh, &points, duel.aspect(), hand.pos.z, faint);
        }
    }
    sparks(&mut mesh, fight);
    if fight.outcome == Outcome::Won {
        // A ring rises where he fell.
        let t = (fight.now * 0.8).fract();
        circle(
            &mut mesh,
            fight.warden_pos() + Vec3::Y * (0.05 + t * 2.0),
            (Vec3::X, Vec3::Z),
            0.9 - t * 0.5,
            40,
            enemy_amber(1.5 * (1.0 - t)),
        );
    }
    mesh
}
