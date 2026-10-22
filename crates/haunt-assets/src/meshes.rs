//! The `mesh` assets, made in code like The Ice Cream Man's: chunky
//! low-poly shapes in palette colours with dark cartoon outlines (the
//! shell draws the outline from each vertex's hull direction).
//!
//! Every mesh stands on the floor at y = 0, centred on its footprint, and
//! faces +z (north, yaw 0): its front is the +z side. A state that changes
//! the shape (a coffin's lid, a jack-o'-lantern's face) is a different
//! mesh; a state that only moves it (rocking, swaying) is the shell's to
//! animate.

use glam::{Mat4, Quat, Vec3};
use toon_mesh::{Mesh, Shape};

use crate::colour;

fn c(token: &str) -> u32 {
    colour(token).unwrap_or(0xFF00FF)
}

/// A box from its centre and size.
fn bx(m: &mut Mesh, centre: [f32; 3], size: [f32; 3], token: &str) {
    let (c3, s3) = (Vec3::from(centre), Vec3::from(size));
    m.block(c3 - s3 / 2.0, c3 + s3 / 2.0, c(token), 1.0);
}

/// A box turned about y.
fn bx_yaw(m: &mut Mesh, centre: [f32; 3], size: [f32; 3], yaw: f32, token: &str) {
    m.add(
        Shape::Cube,
        Mat4::from_scale_rotation_translation(
            Vec3::from(size),
            Quat::from_rotation_y(yaw),
            Vec3::from(centre),
        ),
        c(token),
        1.0,
    );
}

/// A box tilted about x then turned about y (a lid, a leaning board).
fn bx_tilt(m: &mut Mesh, centre: [f32; 3], size: [f32; 3], tilt: f32, yaw: f32, token: &str) {
    m.add(
        Shape::Cube,
        Mat4::from_scale_rotation_translation(
            Vec3::from(size),
            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(tilt),
            Vec3::from(centre),
        ),
        c(token),
        1.0,
    );
}

/// A cylinder (or cone) standing on `base`.
fn cyl(m: &mut Mesh, base: [f32; 3], radius: f32, top: f32, height: f32, sides: u32, token: &str) {
    let b = Vec3::from(base);
    m.add(
        Shape::Frustum {
            sides,
            bottom: 0.5,
            top: 0.5 * top / radius.max(1e-3),
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(radius * 2.0, height, radius * 2.0),
            Quat::IDENTITY,
            b + Vec3::Y * height / 2.0,
        ),
        c(token),
        1.0,
    );
}

/// A cylinder between two points.
fn rod(m: &mut Mesh, from: [f32; 3], to: [f32; 3], radius: f32, sides: u32, token: &str) {
    let (a, b) = (Vec3::from(from), Vec3::from(to));
    let d = b - a;
    let len = d.length().max(1e-4);
    let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
    m.add(
        Shape::Frustum {
            sides,
            bottom: 0.5,
            top: 0.5,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(radius * 2.0, len, radius * 2.0),
            rot,
            (a + b) / 2.0,
        ),
        c(token),
        1.0,
    );
}

/// An ellipsoid.
fn ball(m: &mut Mesh, centre: [f32; 3], size: [f32; 3], token: &str) {
    m.add(
        Shape::Sphere {
            rings: 6,
            segments: 10,
        },
        Mat4::from_scale_rotation_translation(Vec3::from(size), Quat::IDENTITY, Vec3::from(centre)),
        c(token),
        1.0,
    );
}

/// A flat panel facing +z with no outline (a face, a painting).
fn decal(m: &mut Mesh, centre: [f32; 3], size: [f32; 2], token: &str) {
    m.add(
        Shape::Cube,
        Mat4::from_scale_rotation_translation(
            Vec3::new(size[0], size[1], 0.01),
            Quat::IDENTITY,
            Vec3::from(centre),
        ),
        c(token),
        0.0,
    );
}

/// A small flame: a candle-coloured cone with no outline.
fn flame(m: &mut Mesh, base: [f32; 3], height: f32) {
    let b = Vec3::from(base);
    m.add(
        Shape::Frustum {
            sides: 5,
            bottom: 0.5,
            top: 0.0,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(height * 0.45, height, height * 0.45),
            Quat::IDENTITY,
            b + Vec3::Y * height / 2.0,
        ),
        c("candle"),
        0.0,
    );
}

fn jack_o_lantern(face: &str) -> Mesh {
    let mut m = Mesh::new();
    // Ribbed pumpkin: three squashed balls side by side.
    ball(&mut m, [0.0, 0.2, 0.0], [0.5, 0.38, 0.46], "pumpkin");
    ball(
        &mut m,
        [-0.12, 0.2, 0.0],
        [0.28, 0.36, 0.42],
        "pumpkin_deep",
    );
    ball(&mut m, [0.12, 0.2, 0.0], [0.28, 0.36, 0.42], "pumpkin_deep");
    cyl(&mut m, [0.0, 0.37, 0.0], 0.04, 0.03, 0.1, 5, "moss");
    // The face glows candle-yellow.
    let z = 0.235;
    match face {
        "sly" => {
            decal(&mut m, [-0.1, 0.25, z], [0.1, 0.04], "candle");
            decal(&mut m, [0.1, 0.27, z], [0.1, 0.06], "candle");
            decal(&mut m, [0.03, 0.12, z], [0.2, 0.04], "candle");
        }
        "toothy" => {
            decal(&mut m, [-0.1, 0.26, z], [0.08, 0.08], "candle");
            decal(&mut m, [0.1, 0.26, z], [0.08, 0.08], "candle");
            decal(&mut m, [0.0, 0.13, z], [0.26, 0.08], "candle");
            for x in [-0.06, 0.06] {
                decal(&mut m, [x, 0.15, z + 0.005], [0.04, 0.04], "pumpkin");
            }
        }
        _ => {
            decal(&mut m, [-0.1, 0.27, z], [0.08, 0.07], "candle");
            decal(&mut m, [0.1, 0.27, z], [0.08, 0.07], "candle");
            decal(&mut m, [0.0, 0.2, z], [0.05, 0.04], "candle");
            decal(&mut m, [0.0, 0.12, z], [0.24, 0.05], "candle");
        }
    }
    m
}

fn candle(lit: bool) -> Mesh {
    let mut m = Mesh::new();
    cyl(&mut m, [0.0, 0.0, 0.0], 0.06, 0.06, 0.03, 8, "brick");
    cyl(&mut m, [0.0, 0.03, 0.0], 0.035, 0.035, 0.22, 7, "mustard");
    if lit {
        flame(&mut m, [0.0, 0.25, 0.0], 0.06);
    }
    m
}

fn candelabra(lit: bool) -> Mesh {
    let mut m = Mesh::new();
    cyl(&mut m, [0.0, 0.0, 0.0], 0.15, 0.08, 0.06, 8, "mustard");
    rod(
        &mut m,
        [0.0, 0.06, 0.0],
        [0.0, 0.62, 0.0],
        0.025,
        6,
        "mustard",
    );
    rod(
        &mut m,
        [-0.17, 0.62, 0.0],
        [0.17, 0.62, 0.0],
        0.022,
        6,
        "mustard",
    );
    for x in [-0.17, 0.0, 0.17] {
        let y = if x == 0.0 { 0.66 } else { 0.62 };
        cyl(&mut m, [x, y, 0.0], 0.03, 0.03, 0.18, 6, "paper");
        if lit {
            flame(&mut m, [x, y + 0.18, 0.0], 0.06);
        }
    }
    m
}

fn cauldron() -> Mesh {
    let mut m = Mesh::new();
    ball(&mut m, [0.0, 0.42, 0.0], [0.86, 0.66, 0.86], "licorice");
    m.add(
        Shape::Torus {
            segments: 14,
            sides: 5,
            radius: 0.36,
            tube: 0.05,
        },
        Mat4::from_rotation_translation(
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.72, 0.0),
        ),
        c("licorice"),
        1.0,
    );
    // The green brew, flush with the rim.
    m.add(
        Shape::Disc { sides: 14 },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.7, 1.0, 0.7),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.71, 0.0),
        ),
        c("moss"),
        0.0,
    );
    for k in 0..3 {
        let a = k as f32 * 2.1;
        cyl(
            &mut m,
            [0.26 * a.cos(), 0.0, 0.26 * a.sin()],
            0.05,
            0.04,
            0.14,
            5,
            "licorice",
        );
    }
    m
}

fn coffin(open: bool) -> Mesh {
    let mut m = Mesh::new();
    // A six-sided coffin: wide at the shoulders.
    let wood = "pumpkin_deep";
    bx(&mut m, [0.0, 0.25, -0.35], [0.7, 0.5, 1.2], wood);
    bx_yaw(&mut m, [0.0, 0.25, 0.55], [0.55, 0.5, 0.7], 0.0, wood);
    bx(&mut m, [0.0, 0.5, -0.35], [0.6, 0.02, 1.1], "licorice");
    if open {
        // The lid leans against the coffin's side.
        bx(&mut m, [0.25, 0.62, 0.15], [0.72, 0.08, 1.9], "brick");
        bx(&mut m, [0.0, 0.47, -0.2], [0.5, 0.04, 1.3], "brick");
    } else {
        bx(&mut m, [0.0, 0.54, -0.35], [0.72, 0.08, 1.22], "brick");
        bx(&mut m, [0.0, 0.54, 0.55], [0.57, 0.08, 0.72], "brick");
        // A cross on the lid.
        bx(&mut m, [0.0, 0.59, 0.0], [0.08, 0.02, 0.6], "mustard");
        bx(&mut m, [0.0, 0.59, 0.12], [0.3, 0.02, 0.08], "mustard");
    }
    m
}

fn tombstone() -> Mesh {
    let mut m = Mesh::new();
    bx_tilt(
        &mut m,
        [0.0, 0.38, 0.0],
        [0.6, 0.76, 0.18],
        -0.12,
        0.0,
        "dusk",
    );
    m.add(
        Shape::Frustum {
            sides: 10,
            bottom: 0.5,
            top: 0.5,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.6, 0.18, 0.6),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 - 0.12),
            Vec3::new(0.0, 0.78, -0.05),
        ),
        c("dusk"),
        1.0,
    );
    bx(&mut m, [0.0, 0.03, 0.05], [0.7, 0.06, 0.3], "moss");
    decal(&mut m, [0.0, 0.5, 0.1], [0.3, 0.05], "plum");
    decal(&mut m, [0.0, 0.4, 0.09], [0.2, 0.04], "plum");
    m
}

fn cobweb() -> Mesh {
    let mut m = Mesh::new();
    // Thick strokes radiating from the top-left corner, and three arcs.
    let corner = Vec3::new(-0.6, 1.2, 0.0);
    for k in 0..5 {
        let a = -std::f32::consts::FRAC_PI_2 * k as f32 / 4.0;
        let end = corner + Vec3::new(a.cos().abs() * 1.1, a.sin() * 1.1, 0.0);
        rod(&mut m, corner.into(), end.into(), 0.012, 4, "paper");
    }
    for r in [0.35_f32, 0.7, 1.0] {
        let mut prev = corner + Vec3::new(r, 0.0, 0.0);
        for k in 1..=6 {
            let a = -std::f32::consts::FRAC_PI_2 * k as f32 / 6.0;
            let p = corner + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
            rod(&mut m, prev.into(), p.into(), 0.01, 4, "paper");
            prev = p;
        }
    }
    m
}

fn portrait() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.5, 0.0], [0.8, 1.0, 0.06], "mustard");
    decal(&mut m, [0.0, 0.5, 0.035], [0.64, 0.84], "night_teal");
    // A stern sitter: shoulders, head, and two cream eyes the shell turns
    // to follow the player.
    decal(&mut m, [0.0, 0.25, 0.04], [0.5, 0.3], "licorice");
    decal(&mut m, [0.0, 0.58, 0.04], [0.26, 0.32], "dusk");
    decal(&mut m, [-0.06, 0.62, 0.045], [0.06, 0.04], "paper");
    decal(&mut m, [0.06, 0.62, 0.045], [0.06, 0.04], "paper");
    m
}

fn grandfather_clock() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.15, 0.0], [0.6, 0.3, 0.4], "brick");
    bx(&mut m, [0.0, 1.0, 0.0], [0.45, 1.4, 0.32], "pumpkin_deep");
    bx(&mut m, [0.0, 1.9, 0.0], [0.58, 0.55, 0.4], "brick");
    bx_tilt(
        &mut m,
        [0.0, 2.17, 0.0],
        [0.62, 0.08, 0.42],
        0.0,
        0.0,
        "licorice",
    );
    m.add(
        Shape::Disc { sides: 14 },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.38, 1.0, 0.38),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 1.9, 0.205),
        ),
        c("paper"),
        0.0,
    );
    rod(
        &mut m,
        [0.0, 1.9, 0.21],
        [0.0, 2.02, 0.21],
        0.01,
        4,
        "licorice",
    );
    rod(
        &mut m,
        [0.0, 1.9, 0.21],
        [0.08, 1.9, 0.21],
        0.01,
        4,
        "licorice",
    );
    decal(&mut m, [0.0, 1.0, 0.165], [0.26, 0.9], "night_teal");
    rod(
        &mut m,
        [0.0, 1.4, 0.17],
        [0.0, 0.8, 0.17],
        0.012,
        4,
        "mustard",
    );
    ball(&mut m, [0.0, 0.76, 0.17], [0.1, 0.1, 0.03], "mustard");
    m
}

fn gramophone() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.35, 0.0], [0.5, 0.7, 0.5], "brick");
    bx(&mut m, [0.0, 0.76, 0.0], [0.42, 0.12, 0.42], "pumpkin_deep");
    cyl(&mut m, [0.0, 0.82, 0.0], 0.17, 0.17, 0.02, 12, "licorice");
    rod(
        &mut m,
        [0.12, 0.82, -0.12],
        [0.12, 1.0, -0.12],
        0.02,
        5,
        "mustard",
    );
    // The horn flares up and toward the front.
    m.add(
        Shape::Frustum {
            sides: 10,
            bottom: 0.05,
            top: 0.5,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.5, 0.4, 0.5),
            Quat::from_rotation_x(0.9),
            Vec3::new(0.12, 1.0, 0.08),
        ),
        c("mustard"),
        1.0,
    );
    m
}

fn armchair() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.25, 0.0], [0.9, 0.3, 0.85], "brick");
    bx(&mut m, [0.0, 0.47, 0.05], [0.66, 0.14, 0.72], "brick");
    bx(&mut m, [0.0, 0.8, -0.36], [0.9, 0.8, 0.18], "brick");
    for x in [-0.4, 0.4] {
        bx(&mut m, [x, 0.55, 0.05], [0.14, 0.3, 0.8], "brick");
        bx(&mut m, [x, 1.05, -0.36], [0.2, 0.25, 0.22], "brick");
    }
    for (x, z) in [(-0.38, -0.35), (0.38, -0.35), (-0.38, 0.35), (0.38, 0.35)] {
        cyl(&mut m, [x, 0.0, z], 0.04, 0.03, 0.1, 5, "licorice");
    }
    m
}

fn rocking_chair() -> Mesh {
    let mut m = Mesh::new();
    let wood = "pumpkin_deep";
    for x in [-0.3, 0.3] {
        // The rockers: three short boards in a curve.
        bx_tilt(
            &mut m,
            [x, 0.06, -0.38],
            [0.05, 0.05, 0.36],
            0.35,
            0.0,
            wood,
        );
        bx(&mut m, [x, 0.03, 0.0], [0.05, 0.05, 0.42], wood);
        bx_tilt(
            &mut m,
            [x, 0.06, 0.38],
            [0.05, 0.05, 0.36],
            -0.35,
            0.0,
            wood,
        );
        rod(&mut m, [x, 0.05, 0.22], [x, 0.45, 0.22], 0.025, 5, wood);
        rod(&mut m, [x, 0.05, -0.22], [x, 1.05, -0.3], 0.025, 5, wood);
    }
    bx(&mut m, [0.0, 0.46, 0.0], [0.66, 0.06, 0.5], wood);
    for k in 0..4 {
        let x = -0.21 + 0.14 * k as f32;
        rod(&mut m, [x, 0.48, -0.24], [x, 1.0, -0.3], 0.018, 4, wood);
    }
    bx(&mut m, [0.0, 1.02, -0.3], [0.66, 0.08, 0.06], wood);
    m
}

fn broom() -> Mesh {
    let mut m = Mesh::new();
    rod(
        &mut m,
        [0.0, 0.35, 0.0],
        [0.0, 1.5, -0.1],
        0.025,
        5,
        "pumpkin_deep",
    );
    m.add(
        Shape::Frustum {
            sides: 7,
            bottom: 0.5,
            top: 0.18,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.3, 0.42, 0.3),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.21, 0.02),
        ),
        c("mustard"),
        1.0,
    );
    bx(&mut m, [0.0, 0.36, 0.0], [0.1, 0.04, 0.1], "brick");
    m
}

fn potion_shelf() -> Mesh {
    let mut m = Mesh::new();
    let wood = "pumpkin_deep";
    for x in [-0.68, 0.68] {
        bx(&mut m, [x, 0.9, 0.0], [0.06, 1.8, 0.38], wood);
    }
    bx(&mut m, [0.0, 0.9, -0.17], [1.36, 1.8, 0.04], "licorice");
    let bottles = ["moss", "pumpkin", "plum", "dusk", "mustard", "brick"];
    for (row, y) in [0.05_f32, 0.62, 1.2].iter().enumerate() {
        bx(&mut m, [0.0, *y, 0.0], [1.36, 0.05, 0.38], wood);
        for k in 0..4 {
            let x = -0.45 + 0.3 * k as f32;
            let token = bottles[(row * 4 + k) % bottles.len()];
            let tall = 0.18 + 0.06 * ((row + k) % 3) as f32;
            ball(&mut m, [x, y + 0.1, 0.02], [0.16, 0.16, 0.16], token);
            cyl(
                &mut m,
                [x, y + 0.16, 0.02],
                0.03,
                0.03,
                tall - 0.08,
                5,
                token,
            );
        }
    }
    m
}

fn skull() -> Mesh {
    let mut m = Mesh::new();
    ball(&mut m, [0.0, 0.14, 0.0], [0.24, 0.22, 0.24], "paper");
    bx(&mut m, [0.0, 0.05, 0.04], [0.14, 0.08, 0.14], "paper");
    decal(&mut m, [-0.05, 0.15, 0.115], [0.05, 0.05], "licorice");
    decal(&mut m, [0.05, 0.15, 0.115], [0.05, 0.05], "licorice");
    decal(&mut m, [0.0, 0.09, 0.115], [0.025, 0.03], "licorice");
    m
}

fn chains() -> Mesh {
    let mut m = Mesh::new();
    for (k, x) in [-0.12_f32, 0.12].iter().enumerate() {
        let links = 6 + k;
        for i in 0..links {
            let y = 1.6 - 0.2 * i as f32;
            let turn = if i % 2 == 0 {
                0.0
            } else {
                std::f32::consts::FRAC_PI_2
            };
            m.add(
                Shape::Torus {
                    segments: 8,
                    sides: 4,
                    radius: 0.06,
                    tube: 0.018,
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(1.0, 1.6, 1.0),
                    Quat::from_rotation_y(turn),
                    Vec3::new(*x, y, 0.0),
                ),
                c("dusk"),
                1.0,
            );
        }
    }
    m
}

fn cage() -> Mesh {
    let mut m = Mesh::new();
    cyl(&mut m, [0.0, 0.0, 0.0], 0.48, 0.48, 0.06, 12, "licorice");
    cyl(&mut m, [0.0, 1.5, 0.0], 0.48, 0.1, 0.3, 12, "licorice");
    for k in 0..12 {
        let a = k as f32 / 12.0 * std::f32::consts::TAU;
        let (x, z) = (0.45 * a.cos(), 0.45 * a.sin());
        rod(&mut m, [x, 0.05, z], [x, 1.5, z], 0.018, 4, "licorice");
    }
    rod(
        &mut m,
        [0.0, 1.8, 0.0],
        [0.0, 1.8, 0.0],
        0.02,
        4,
        "licorice",
    );
    m
}

fn chandelier() -> Mesh {
    let mut m = Mesh::new();
    // It hangs from y = 1 down; the shell hangs it from the ceiling.
    rod(
        &mut m,
        [0.0, 0.55, 0.0],
        [0.0, 1.0, 0.0],
        0.02,
        5,
        "licorice",
    );
    m.add(
        Shape::Torus {
            segments: 16,
            sides: 5,
            radius: 0.5,
            tube: 0.035,
        },
        Mat4::from_rotation_translation(
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.3, 0.0),
        ),
        c("mustard"),
        1.0,
    );
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        let (x, z) = (0.5 * a.cos(), 0.5 * a.sin());
        rod(&mut m, [x, 0.3, z], [0.0, 0.55, 0.0], 0.012, 4, "licorice");
        cyl(&mut m, [x, 0.3, z], 0.025, 0.025, 0.12, 5, "paper");
        flame(&mut m, [x, 0.42, z], 0.06);
    }
    m
}

fn cracked_mirror() -> Mesh {
    let mut m = Mesh::new();
    m.add(
        Shape::Disc { sides: 14 },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.8, 1.0, 1.2),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.6, 0.0),
        ),
        c("mustard"),
        1.0,
    );
    m.add(
        Shape::Disc { sides: 14 },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.66, 1.0, 1.04),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.6, 0.02),
        ),
        c("dusk"),
        0.0,
    );
    // The crack: a jagged stroke.
    let points = [
        [-0.2, 0.95],
        [-0.05, 0.75],
        [-0.12, 0.6],
        [0.08, 0.45],
        [0.02, 0.28],
    ];
    for w in points.windows(2) {
        rod(
            &mut m,
            [w[0][0], w[0][1], 0.03],
            [w[1][0], w[1][1], 0.03],
            0.008,
            3,
            "licorice",
        );
    }
    m
}

fn piano() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.7, -0.1], [1.6, 1.3, 0.5], "licorice");
    bx(&mut m, [0.0, 0.72, 0.2], [1.5, 0.1, 0.3], "licorice");
    decal(&mut m, [0.0, 0.775, 0.22], [1.4, 0.22], "paper");
    for k in 0..12 {
        let x = -0.6 + 0.11 * k as f32;
        if k % 7 != 2 && k % 7 != 6 {
            bx(&mut m, [x, 0.79, 0.16], [0.05, 0.02, 0.12], "licorice");
        }
    }
    for x in [-0.7, 0.7] {
        bx(&mut m, [x, 0.35, 0.25], [0.08, 0.7, 0.08], "licorice");
    }
    cyl(&mut m, [0.55, 1.35, -0.1], 0.03, 0.03, 0.12, 5, "paper");
    flame(&mut m, [0.55, 1.47, -0.1], 0.05);
    m
}

fn bookshelf(fallen: bool) -> Mesh {
    let mut m = Mesh::new();
    let wood = "pumpkin_deep";
    for x in [-0.68, 0.68] {
        bx(&mut m, [x, 1.0, 0.0], [0.06, 2.0, 0.38], wood);
    }
    bx(&mut m, [0.0, 1.0, -0.17], [1.36, 2.0, 0.04], "licorice");
    bx(&mut m, [0.0, 1.98, 0.0], [1.4, 0.06, 0.4], wood);
    let spines = [
        "brick",
        "night_teal",
        "mustard",
        "plum",
        "moss",
        "pumpkin_deep",
    ];
    for (row, y) in [0.05_f32, 0.55, 1.05, 1.5].iter().enumerate() {
        bx(&mut m, [0.0, *y, 0.0], [1.36, 0.05, 0.38], wood);
        let mut x = -0.6;
        let mut k = 0;
        while x < 0.58 {
            let w = 0.07 + 0.02 * ((row + k) % 3) as f32;
            let h = 0.3 + 0.06 * ((row * 3 + k) % 3) as f32;
            if !(fallen && row == 2 && k == 3) {
                bx(
                    &mut m,
                    [x + w / 2.0, y + h / 2.0 + 0.025, 0.02],
                    [w, h, 0.28],
                    spines[(row + k) % spines.len()],
                );
            }
            x += w + 0.01;
            k += 1;
        }
    }
    if fallen {
        bx_yaw(&mut m, [0.2, 0.04, 0.3], [0.3, 0.07, 0.22], 0.4, "brick");
    }
    m
}

fn hanging_sheet(ghost: bool) -> Mesh {
    let mut m = Mesh::new();
    // It hangs from y = 1.8 (a line across); as a ghost it gains eyes.
    rod(
        &mut m,
        [-0.7, 1.8, 0.0],
        [0.7, 1.8, 0.0],
        0.012,
        4,
        "licorice",
    );
    for k in 0..4 {
        let x = -0.45 + 0.3 * k as f32;
        let sway = if k % 2 == 0 { 0.04 } else { -0.04 };
        bx_tilt(
            &mut m,
            [x, 1.05, sway],
            [0.32, 1.5, 0.04],
            sway,
            0.0,
            "paper",
        );
    }
    if ghost {
        decal(&mut m, [-0.12, 1.4, 0.06], [0.1, 0.16], "licorice");
        decal(&mut m, [0.12, 1.4, 0.06], [0.1, 0.16], "licorice");
        decal(&mut m, [0.0, 1.15, 0.06], [0.12, 0.14], "licorice");
    }
    m
}

fn banging_door(open: bool) -> Mesh {
    let mut m = Mesh::new();
    let frame = "licorice";
    bx(&mut m, [-0.52, 1.05, 0.0], [0.08, 2.1, 0.1], frame);
    bx(&mut m, [0.52, 1.05, 0.0], [0.08, 2.1, 0.1], frame);
    bx(&mut m, [0.0, 2.08, 0.0], [1.12, 0.08, 0.1], frame);
    let ajar = if open { 0.22 } else { 0.0 };
    // The door swings on its left hinge.
    let hinge = Vec3::new(-0.48, 0.0, 0.02);
    let rot = Quat::from_rotation_y(-ajar);
    let place = |p: Vec3| hinge + rot * (p - hinge);
    let panel = place(Vec3::new(0.0, 1.03, 0.04));
    m.add(
        Shape::Cube,
        Mat4::from_scale_rotation_translation(Vec3::new(0.94, 2.02, 0.05), rot, panel),
        c("brick"),
        1.0,
    );
    for y in [0.55_f32, 1.45] {
        let p = place(Vec3::new(0.0, y, 0.07));
        m.add(
            Shape::Cube,
            Mat4::from_scale_rotation_translation(Vec3::new(0.7, 0.6, 0.02), rot, p),
            c("pumpkin_deep"),
            1.0,
        );
    }
    let knob = place(Vec3::new(0.34, 1.0, 0.1));
    ball(&mut m, knob.into(), [0.07, 0.07, 0.07], "mustard");
    m
}

fn lightning_window(flash: bool) -> Mesh {
    let mut m = Mesh::new();
    let frame = "licorice";
    bx(&mut m, [0.0, 0.0, 0.0], [1.3, 0.08, 0.14], frame);
    bx(&mut m, [0.0, 1.4, 0.0], [1.3, 0.08, 0.14], frame);
    for x in [-0.62, 0.0, 0.62] {
        bx(&mut m, [x, 0.7, 0.0], [0.07, 1.4, 0.12], frame);
    }
    bx(&mut m, [0.0, 0.7, 0.0], [1.24, 0.06, 0.1], frame);
    let glass = if flash { "paper" } else { "night_teal" };
    decal(&mut m, [0.0, 0.7, -0.02], [1.2, 1.34], glass);
    // The sill.
    bx(&mut m, [0.0, -0.06, 0.08], [1.4, 0.06, 0.2], "pumpkin_deep");
    m
}

fn hay_bale() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.25, 0.0], [1.0, 0.5, 0.6], "mustard");
    for x in [-0.25, 0.25] {
        bx(&mut m, [x, 0.25, 0.0], [0.03, 0.52, 0.62], "pumpkin_deep");
    }
    m
}

fn corn_stalks() -> Mesh {
    let mut m = Mesh::new();
    for k in 0..7 {
        let a = k as f32 / 7.0 * std::f32::consts::TAU;
        let lean = Vec3::new(a.cos() * 0.25, 0.0, a.sin() * 0.25);
        rod(
            &mut m,
            [a.cos() * 0.1, 0.0, a.sin() * 0.1],
            (Vec3::Y * 2.0 + lean).into(),
            0.03,
            4,
            "mustard",
        );
        let leaf = Vec3::Y * (0.9 + 0.1 * k as f32) + lean * 0.5;
        bx_tilt(&mut m, leaf.into(), [0.06, 0.5, 0.02], 0.6, a, "mustard");
    }
    bx(&mut m, [0.0, 0.8, 0.0], [0.34, 0.1, 0.34], "brick");
    m
}

fn iron_fence() -> Mesh {
    let mut m = Mesh::new();
    for y in [0.25_f32, 1.3] {
        bx(&mut m, [0.0, y, 0.0], [3.0, 0.05, 0.05], "licorice");
    }
    let mut x = -1.45;
    while x <= 1.46 {
        rod(&mut m, [x, 0.0, 0.0], [x, 1.45, 0.0], 0.022, 4, "licorice");
        m.add(
            Shape::Frustum {
                sides: 4,
                bottom: 0.5,
                top: 0.0,
            },
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.08, 0.15, 0.08),
                Quat::IDENTITY,
                Vec3::new(x, 1.52, 0.0),
            ),
            c("licorice"),
            1.0,
        );
        x += 0.18;
    }
    for x in [-1.5, 1.5] {
        bx(&mut m, [x, 0.8, 0.0], [0.14, 1.6, 0.14], "licorice");
        ball(&mut m, [x, 1.66, 0.0], [0.16, 0.16, 0.16], "licorice");
    }
    m
}

fn gnarled_tree() -> Mesh {
    let mut m = Mesh::new();
    let wood = "night_teal";
    // A leaning, twisting trunk, then forked crooked branches.
    let trunk = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.15, 1.2, 0.05),
        Vec3::new(-0.1, 2.3, -0.1),
        Vec3::new(0.2, 3.3, 0.0),
    ];
    let radii = [0.42, 0.32, 0.24];
    for i in 0..3 {
        rod(
            &mut m,
            trunk[i].into(),
            trunk[i + 1].into(),
            radii[i],
            6,
            wood,
        );
    }
    // Roots.
    for k in 0..4 {
        let a = k as f32 * 1.6 + 0.3;
        rod(
            &mut m,
            [0.0, 0.2, 0.0],
            [a.cos() * 0.75, 0.0, a.sin() * 0.75],
            0.1,
            4,
            wood,
        );
    }
    let branches: [([f32; 3], [f32; 3], [f32; 3]); 5] = [
        ([0.1, 2.0, 0.0], [-1.0, 3.0, 0.3], [-1.6, 3.2, 0.8]),
        ([0.0, 2.6, -0.05], [0.9, 3.6, -0.4], [1.6, 3.5, -0.9]),
        ([0.2, 3.3, 0.0], [-0.3, 4.4, 0.4], [-0.9, 5.2, 0.2]),
        ([0.2, 3.3, 0.0], [0.8, 4.4, 0.3], [1.1, 5.4, 0.9]),
        ([0.15, 1.6, 0.05], [0.9, 2.2, 0.6], [1.3, 2.9, 0.7]),
    ];
    for (a, b, tip) in branches {
        rod(&mut m, a, b, 0.12, 5, wood);
        rod(&mut m, b, tip, 0.06, 4, wood);
    }
    m
}

fn moon() -> Mesh {
    let mut m = Mesh::new();
    m.add(
        Shape::Disc { sides: 24 },
        Mat4::from_scale_rotation_translation(
            Vec3::new(6.0, 1.0, 6.0),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 3.0, 0.0),
        ),
        c("paper"),
        0.0,
    );
    for (x, y, r) in [(-1.2_f32, 3.8, 0.9), (1.0, 2.2, 0.6), (0.6, 4.2, 0.4)] {
        m.add(
            Shape::Disc { sides: 12 },
            Mat4::from_scale_rotation_translation(
                Vec3::new(r, 1.0, r),
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                Vec3::new(x, y, 0.01),
            ),
            c("dusk"),
            0.0,
        );
    }
    m
}

fn coat_stand() -> Mesh {
    let mut m = Mesh::new();
    rod(
        &mut m,
        [0.0, 0.0, 0.0],
        [0.0, 1.8, 0.0],
        0.03,
        5,
        "pumpkin_deep",
    );
    for k in 0..3 {
        let a = k as f32 * 2.1;
        rod(
            &mut m,
            [0.0, 0.05, 0.0],
            [a.cos() * 0.25, 0.0, a.sin() * 0.25],
            0.025,
            4,
            "pumpkin_deep",
        );
    }
    // A long dark coat and a hat: it looks like someone standing.
    m.add(
        Shape::Frustum {
            sides: 8,
            bottom: 0.5,
            top: 0.3,
        },
        Mat4::from_scale_rotation_translation(
            Vec3::new(0.5, 1.1, 0.35),
            Quat::IDENTITY,
            Vec3::new(0.0, 1.05, 0.06),
        ),
        c("plum"),
        1.0,
    );
    cyl(&mut m, [0.0, 1.72, 0.0], 0.2, 0.2, 0.03, 10, "licorice");
    cyl(&mut m, [0.0, 1.75, 0.0], 0.12, 0.11, 0.16, 10, "licorice");
    m
}

fn counter() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.42, 0.0], [2.0, 0.84, 0.6], "pumpkin_deep");
    bx(&mut m, [0.0, 0.87, 0.0], [2.08, 0.06, 0.66], "paper");
    for x in [-0.5, 0.5] {
        decal(&mut m, [x, 0.45, 0.305], [0.85, 0.6], "brick");
        ball(&mut m, [x + 0.3, 0.6, 0.31], [0.05, 0.05, 0.05], "mustard");
    }
    m
}

fn sign() -> Mesh {
    let mut m = Mesh::new();
    bx(&mut m, [0.0, 0.22, 0.0], [0.8, 0.45, 0.04], "paper");
    bx(
        &mut m,
        [0.0, 0.22, -0.01],
        [0.86, 0.5, 0.03],
        "pumpkin_deep",
    );
    m
}

/// The mesh for asset `id` in `state`, or `None` for cards, effects, and
/// ids the library doesn't make.
#[must_use]
pub fn build(id: &str, state: &str) -> Option<Mesh> {
    Some(match id {
        "jack-o-lantern" => jack_o_lantern(state),
        "candle" => candle(state != "out"),
        "candelabra" => candelabra(state != "dipped"),
        "cauldron" => cauldron(),
        "coffin" => coffin(state == "open"),
        "tombstone" => tombstone(),
        "cobweb" => cobweb(),
        "portrait" => portrait(),
        "grandfather-clock" => grandfather_clock(),
        "gramophone" => gramophone(),
        "armchair" => armchair(),
        "rocking-chair" => rocking_chair(),
        "broom" => broom(),
        "potion-shelf" => potion_shelf(),
        "skull" => skull(),
        "chains" => chains(),
        "cage" => cage(),
        "chandelier" => chandelier(),
        "cracked-mirror" => cracked_mirror(),
        "piano" => piano(),
        "bookshelf" => bookshelf(state == "book-falls"),
        "hanging-sheet" => hanging_sheet(state == "ghost"),
        "banging-door" => banging_door(state == "banging"),
        "lightning-window" => lightning_window(state == "flash"),
        "hay-bale" => hay_bale(),
        "corn-stalks" => corn_stalks(),
        "iron-fence" => iron_fence(),
        "gnarled-tree" => gnarled_tree(),
        "moon" => moon(),
        "coat-stand" => coat_stand(),
        "counter" => counter(),
        "sign" => sign(),
        _ => return None,
    })
}

/// The states whose meshes differ, for an asset: the shell (and a bake)
/// builds one mesh for each, and one for every other state.
#[must_use]
pub fn shaped_states(id: &str) -> &'static [&'static str] {
    match id {
        "jack-o-lantern" => &["grin", "sly", "toothy"],
        "candle" => &["lit", "out"],
        "candelabra" => &["lit", "dipped"],
        "coffin" => &["closed", "open"],
        "bookshelf" => &["still", "book-falls"],
        "hanging-sheet" => &["still", "ghost"],
        "banging-door" => &["shut", "banging"],
        "lightning-window" => &["dark", "flash"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kind, catalog};

    #[test]
    fn every_mesh_asset_has_a_mesh_in_every_state_that_fits_its_footprint() {
        let mut bad = Vec::new();
        for asset in &catalog().assets {
            let made = build(&asset.id, &asset.states[0]);
            match asset.kind {
                Kind::Mesh => {
                    for state in &asset.states {
                        let mesh = build(&asset.id, state)
                            .unwrap_or_else(|| panic!("no mesh for {} {state}", asset.id));
                        assert!(mesh.vertices() > 0, "{}", asset.id);
                        if asset.id == "moon" || asset.id == "gnarled-tree" {
                            continue;
                        }
                        // Within its footprint (with a little give for
                        // outlines and lids) and its height.
                        let [w, d, h] = asset.footprint;
                        let mut worst = [0.0_f32; 3];
                        for v in mesh.data.chunks(toon_mesh::STRIDE) {
                            worst[0] = worst[0].max(v[0].abs() - w / 2.0);
                            worst[1] = worst[1].max(v[1] - h).max(-0.1 - v[1]);
                            worst[2] = worst[2].max(v[2].abs() - d / 2.0);
                        }
                        if worst[0] > 0.25 || worst[1] > 0.25 || worst[2] > 0.3 {
                            bad.push(format!("{} {state} pokes out by {worst:?}", asset.id));
                        }
                    }
                }
                _ => assert!(made.is_none(), "{} is not a mesh", asset.id),
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn meshes_use_only_palette_colours() {
        let palette: Vec<[f32; 3]> = crate::PALETTE
            .iter()
            .map(|(_, hex, _)| toon_mesh::rgb(*hex))
            .collect();
        for asset in catalog().assets.iter().filter(|a| a.kind == Kind::Mesh) {
            let mesh = build(&asset.id, &asset.states[0]).unwrap();
            for v in mesh.data.chunks(toon_mesh::STRIDE) {
                let colour = [v[9], v[10], v[11]];
                assert!(palette.contains(&colour), "{} uses {colour:?}", asset.id);
            }
        }
    }
}
