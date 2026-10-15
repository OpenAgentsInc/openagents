//! The kits, made in code: every piece drawn from its corners (so a square
//! that leans is drawn leaning), struts and bolts, the cargo ship, the
//! beacon, the robot crew, and the space backdrop. Colours are bright and
//! each piece also has its own shape, so a child who can't tell two
//! colours apart can still tell the pieces apart.

use glam::{Vec2, Vec3};
use snap_rules::PieceKind;

use crate::mesh::{Mesh, ccw, mix, rgb, shade, shrink};

/// Each piece's colour.
#[must_use]
pub fn colour(kind: PieceKind) -> u32 {
    match kind {
        PieceKind::Square => 0x3BA7F5,
        PieceKind::Triangle => 0xFFC83D,
        PieceKind::Wedge => 0xFF7A59,
        PieceKind::Panel => 0x4CC38A,
        PieceKind::Solar => 0x2A4FBF,
        PieceKind::Thruster => 0xFF8A1F,
        PieceKind::Cargo => 0xC9783A,
        PieceKind::Pod => 0xB9F27C,
        PieceKind::Module => 0xE9EEF5,
        PieceKind::Strut => 0xF2F2F2,
        PieceKind::Bolt => 0xC8CDD6,
    }
}

const MAGNET: u32 = 0xE5484D;
const SILVER: u32 = 0xC8CDD6;
const DARK: u32 = 0x2B2F3A;
const TEAL: u32 = 0x18B6C9;
const WINDOW: u32 = 0x1B2A4A;
const GOLD: u32 = 0xF4C542;
const LEAF: u32 = 0x2FA84F;
const POT: u32 = 0xD96C3F;

/// How a piece is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    /// Popped or come loose: drawn gray.
    pub broken: bool,
    /// High contrast: a white rim around every piece.
    pub contrast: bool,
    /// A matching dot pattern in the tray and on the piece.
    pub marks: bool,
    /// Raised toward the camera, for a held piece.
    pub lift: f32,
}

fn tint(hex: u32, style: Style) -> [f32; 3] {
    let colour = rgb(hex);
    if style.broken {
        let gray = (colour[0] + colour[1] + colour[2]) / 3.0;
        mix(colour, [gray * 0.7; 3], 0.85)
    } else {
        colour
    }
}

/// The corners' middle.
#[must_use]
pub fn middle(corners: &[Vec2]) -> Vec2 {
    corners.iter().copied().sum::<Vec2>() / corners.len().max(1) as f32
}

/// A tile drawn at its corners.
pub fn tile(mesh: &mut Mesh, kind: PieceKind, corners: &[Vec2], style: Style) {
    let z = style.lift;
    let base = tint(colour(kind), style);
    match kind {
        PieceKind::Square | PieceKind::Triangle | PieceKind::Wedge => {
            // A frame: rods along the sides, a pale window, magnets in the
            // corners.
            let n = corners.len();
            for i in 0..n {
                mesh.rod(
                    corners[i],
                    corners[(i + 1) % n],
                    0.13,
                    z - 0.12,
                    z + 0.12,
                    base,
                );
            }
            let window = shrink(corners, 0.12);
            mesh.prism(&window, z - 0.03, z + 0.03, mix(base, [1.0; 3], 0.6), base);
            for corner in corners {
                let inward = (middle(corners) - *corner).normalize_or_zero() * 0.1;
                mesh.disc(
                    *corner + inward,
                    0.065,
                    8,
                    0.0,
                    z + 0.1,
                    z + 0.15,
                    tint(MAGNET, style),
                );
            }
        }
        PieceKind::Panel => {
            mesh.prism(corners, z - 0.12, z + 0.12, base, shade(base, 0.75));
            let plate = shrink(corners, 0.16);
            mesh.prism(&plate, z + 0.12, z + 0.15, shade(base, 1.25), base);
        }
        PieceKind::Solar => {
            mesh.prism(corners, z - 0.06, z + 0.06, base, tint(GOLD, style));
            // Cells: a grid of lines over the face.
            let (a, b, d) = (corners[0], corners[1], corners[4]);
            let across = b - a;
            let up = d - a;
            let line = tint(0x7EA2FF, style);
            for i in 1..4 {
                let t = i as f32 / 4.0;
                mesh.rod(a + up * t, b + up * t, 0.03, z + 0.06, z + 0.08, line);
            }
            let t = 0.5;
            mesh.rod(
                a + across * t,
                d + across * t,
                0.03,
                z + 0.06,
                z + 0.08,
                line,
            );
            let n = corners.len();
            for i in 0..n {
                mesh.rod(
                    corners[i],
                    corners[(i + 1) % n],
                    0.06,
                    z - 0.07,
                    z + 0.09,
                    tint(GOLD, style),
                );
            }
        }
        PieceKind::Thruster => {
            mesh.prism(corners, z - 0.15, z + 0.15, base, shade(base, 0.7));
            // The nozzle at the tip.
            let tip = corners[2];
            let base_mid = corners[0].lerp(corners[1], 0.5);
            let dir = (tip - base_mid).normalize_or_zero();
            let side = dir.perp() * 0.18;
            let at = tip - dir * 0.12;
            let nozzle = ccw(vec![
                at - side,
                at + dir * 0.18 - side * 1.4,
                at + dir * 0.18 + side * 1.4,
                at + side,
            ]);
            mesh.prism(
                &nozzle,
                z - 0.12,
                z + 0.12,
                tint(DARK, style),
                tint(DARK, style),
            );
            mesh.disc(
                middle(corners),
                0.09,
                10,
                0.0,
                z + 0.15,
                z + 0.18,
                tint(SILVER, style),
            );
        }
        PieceKind::Cargo => {
            mesh.prism(corners, z - 0.2, z + 0.2, base, shade(base, 0.7));
            let band = tint(0x8A4E22, style);
            mesh.rod(corners[0], corners[2], 0.1, z + 0.2, z + 0.23, band);
            mesh.rod(corners[1], corners[3], 0.1, z + 0.2, z + 0.23, band);
            for i in 0..4 {
                mesh.rod(
                    corners[i],
                    corners[(i + 1) % 4],
                    0.1,
                    z + 0.2,
                    z + 0.23,
                    band,
                );
            }
        }
        PieceKind::Pod => {
            let glass = shrink(corners, 0.04);
            mesh.prism(&glass, z - 0.15, z + 0.15, base, shade(base, 0.8));
            let m = middle(corners);
            let up = (corners[3] - corners[0]).normalize_or_zero();
            let right = (corners[1] - corners[0]).normalize_or_zero();
            let pot = m - up * 0.25;
            mesh.disc(pot, 0.16, 8, 0.0, z + 0.15, z + 0.3, tint(POT, style));
            mesh.rod(
                pot,
                m + up * 0.12,
                0.05,
                z + 0.2,
                z + 0.26,
                tint(LEAF, style),
            );
            for (dx, dy, r) in [(-0.13, 0.12, 0.11), (0.14, 0.16, 0.12), (0.0, 0.28, 0.13)] {
                let leaf = m + right * dx + up * dy;
                mesh.sphere(
                    Vec3::new(leaf.x, leaf.y, z + 0.28),
                    r,
                    4,
                    6,
                    tint(LEAF, style),
                );
            }
        }
        PieceKind::Module => {
            mesh.prism(corners, z - 0.22, z + 0.22, base, shade(base, 0.7));
            let up = corners[3] - corners[0];
            let stripe_a = corners[0] + up * 0.18;
            let stripe_b = corners[1] + up * 0.18;
            mesh.rod(
                stripe_a,
                stripe_b,
                0.12,
                z + 0.22,
                z + 0.25,
                tint(TEAL, style),
            );
            let m = middle(corners) + up * 0.08;
            mesh.disc(m, 0.24, 14, 0.0, z + 0.22, z + 0.26, tint(SILVER, style));
            mesh.disc(m, 0.18, 14, 0.0, z + 0.26, z + 0.28, tint(WINDOW, style));
        }
        PieceKind::Strut | PieceKind::Bolt => {}
    }
    if style.marks {
        mark(mesh, kind, middle(corners), z + 0.48);
    }
    if style.contrast {
        let n = corners.len();
        for i in 0..n {
            mesh.rod(
                corners[i],
                corners[(i + 1) % n],
                0.04,
                z + 0.25,
                z + 0.3,
                [1.0; 3],
            );
        }
    }
}

/// A four-dot pattern that stays distinct without colour.
pub fn mark_pattern(kind: PieceKind) -> u8 {
    PieceKind::ALL.iter().position(|k| *k == kind).unwrap_or(0) as u8 + 1
}

fn mark(mesh: &mut Mesh, kind: PieceKind, at: Vec2, z: f32) {
    mesh.disc(at, 0.18, 8, 0., z, z + 0.01, [1.; 3]);
    let code = mark_pattern(kind);
    for bit in 0..4 {
        if code & (1 << bit) != 0 {
            let p = at
                + Vec2::new(
                    if bit % 2 == 0 { -0.065 } else { 0.065 },
                    if bit / 2 == 0 { 0.065 } else { -0.065 },
                );
            mesh.disc(p, 0.04, 8, 0., z + 0.01, z + 0.02, [0.; 3]);
        }
    }
}

/// A strut from corner `a` to corner `b`.
pub fn strut(mesh: &mut Mesh, a: Vec2, b: Vec2, style: Style) {
    if style.marks {
        mark(mesh, PieceKind::Strut, (a + b) * 0.5, style.lift + 0.52);
    }
    let dir = (b - a).normalize_or_zero() * 0.08;
    let z = style.lift;
    mesh.rod(
        a + dir,
        b - dir,
        0.09,
        z + 0.12,
        z + 0.2,
        tint(colour(PieceKind::Strut), style),
    );
    for end in [a + dir, b - dir] {
        mesh.disc(end, 0.07, 8, 0.0, z + 0.18, z + 0.22, tint(SILVER, style));
    }
}

/// A bolt at a seam's middle.
pub fn bolt(mesh: &mut Mesh, at: Vec2, style: Style) {
    if style.marks {
        mark(mesh, PieceKind::Bolt, at, style.lift + 0.58);
    }
    let z = style.lift;
    mesh.disc(
        at,
        0.13,
        6,
        0.0,
        z + 0.14,
        z + 0.24,
        tint(colour(PieceKind::Bolt), style),
    );
    mesh.disc(at, 0.06, 10, 0.0, z + 0.24, z + 0.27, tint(0x6B7280, style));
}

/// The cargo ship in its box: nose to the left.
pub fn ship(mesh: &mut Mesh, lit: &mut Mesh, low: Vec2, high: Vec2, engine: f32) {
    let mid = (low.y + high.y) / 2.0;
    let hull = [
        Vec2::new(low.x + 0.4, low.y),
        Vec2::new(high.x, low.y),
        Vec2::new(high.x, high.y),
        Vec2::new(low.x + 0.4, high.y),
        Vec2::new(low.x, mid),
    ];
    mesh.prism(&hull, -0.3, 0.3, rgb(0xDDE3EA), rgb(0x9AA5B4));
    mesh.rod(
        Vec2::new(low.x + 0.6, mid - 0.12),
        Vec2::new(high.x - 0.1, mid - 0.12),
        0.12,
        0.3,
        0.33,
        rgb(0xFF6B3D),
    );
    for i in 0..3 {
        let at = Vec2::new(low.x + 0.9 + i as f32 * 0.55, mid + 0.14);
        mesh.disc(at, 0.1, 10, 0.0, 0.3, 0.34, rgb(0x6FD3FF));
    }
    if engine > 0.0 {
        let glow = rgb(0xFFB45A);
        let back = Vec2::new(high.x, mid);
        lit.prism(
            &[
                back + Vec2::new(0.0, -0.25),
                back + Vec2::new(0.5 * engine, 0.0),
                back + Vec2::new(0.0, 0.25),
            ],
            -0.1,
            0.1,
            glow,
            glow,
        );
    }
}

/// The beacon a crate is pushed to: a ring of lights.
pub fn beacon(glow: &mut Mesh, at: Vec2, r: f32, time: f32) {
    glow.annulus(at, r - 0.05, r + 0.05, 32, -0.02, 0.02, rgb(0x7CF2C8));
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU + time * 0.6;
        let p = at + Vec2::new(a.cos(), a.sin()) * r;
        let on = ((time * 3.0 + i as f32).sin() * 0.5 + 0.5).max(0.3);
        glow.sphere(
            Vec3::new(p.x, p.y, 0.0),
            0.09,
            3,
            6,
            shade(rgb(0xB8FFE6), on),
        );
    }
}

/// Who on the robot crew.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crew {
    Cog,
    Pip,
    Rivet,
    Nova,
    Tock,
}

impl Crew {
    pub const ALL: [Self; 5] = [Self::Cog, Self::Pip, Self::Rivet, Self::Nova, Self::Tock];

    /// The crew member a job file names.
    #[must_use]
    pub fn from_word(word: &str) -> Self {
        match word {
            "pip" => Self::Pip,
            "rivet" => Self::Rivet,
            "nova" => Self::Nova,
            "tock" => Self::Tock,
            _ => Self::Cog,
        }
    }

    #[must_use]
    pub fn colour(self) -> u32 {
        match self {
            Self::Cog => 0xFF8A3D,
            Self::Pip => 0x4AA8FF,
            Self::Rivet => 0x46C96B,
            Self::Nova => 0x9B6BFF,
            Self::Tock => 0xFFD23F,
        }
    }
}

/// How a crew member looks right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Waiting,
    Cheering,
    Thinking,
}

/// A crew member at `at`, about a metre tall.
pub fn robot(mesh: &mut Mesh, crew: Crew, at: Vec2, time: f32, mood: Mood) {
    let bob = (time * 2.0).sin() * 0.08;
    let hop = if mood == Mood::Cheering {
        (time * 8.0).sin().abs() * 0.3
    } else {
        0.0
    };
    let tilt = if mood == Mood::Thinking { 0.15 } else { 0.0 };
    let c = Vec3::new(at.x, at.y + bob + hop, 0.5);
    let body = rgb(crew.colour());
    let white = rgb(0xFFFFFF);
    let pupil = rgb(0x1E2230);
    let eye = |mesh: &mut Mesh, p: Vec3, r: f32| {
        mesh.sphere(p, r, 4, 8, white);
        mesh.sphere(
            p + Vec3::new(tilt * 0.3, -tilt * 0.2, r * 0.7),
            r * 0.5,
            3,
            6,
            pupil,
        );
    };
    match crew {
        Crew::Cog => {
            mesh.sphere(c, 0.42, 6, 10, body);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU + time * 0.5;
                let d = Vec2::new(a.cos(), a.sin());
                let p = Vec2::new(c.x, c.y) + d * 0.46;
                mesh.disc(p, 0.09, 4, a, -0.08, 0.08, shade(body, 0.8));
            }
            eye(mesh, c + Vec3::new(-0.14, 0.08, 0.36), 0.11);
            eye(mesh, c + Vec3::new(0.14, 0.08, 0.36), 0.11);
            mesh.rod(
                Vec2::new(c.x, c.y + 0.4),
                Vec2::new(c.x, c.y + 0.68),
                0.04,
                -0.02,
                0.02,
                rgb(SILVER),
            );
            mesh.sphere(c + Vec3::new(0.0, 0.7, 0.0), 0.07, 3, 6, rgb(0xFF4D4D));
        }
        Crew::Pip => {
            mesh.sphere(c, 0.3, 6, 10, body);
            for side in [-1.0_f32, 1.0] {
                let p = Vec2::new(c.x + side * 0.42, c.y + 0.12);
                mesh.annulus(p, 0.08, 0.2, 12, -0.02, 0.02, rgb(SILVER));
            }
            eye(mesh, c + Vec3::new(0.0, 0.03, 0.26), 0.13);
        }
        Crew::Rivet => {
            let body_box = [
                Vec2::new(c.x - 0.28, c.y - 0.45),
                Vec2::new(c.x + 0.28, c.y - 0.45),
                Vec2::new(c.x + 0.28, c.y + 0.1),
                Vec2::new(c.x - 0.28, c.y + 0.1),
            ];
            mesh.prism(&body_box, -0.25, 0.25, body, shade(body, 0.75));
            let head = [
                Vec2::new(c.x - 0.22, c.y + 0.14),
                Vec2::new(c.x + 0.22, c.y + 0.14),
                Vec2::new(c.x + 0.22, c.y + 0.5),
                Vec2::new(c.x - 0.22, c.y + 0.5),
            ];
            mesh.prism(&head, -0.2, 0.2, shade(body, 1.2), body);
            eye(mesh, c + Vec3::new(-0.09, 0.33, 0.2), 0.08);
            eye(mesh, c + Vec3::new(0.09, 0.33, 0.2), 0.08);
            let wave = if mood == Mood::Cheering {
                (time * 10.0).sin() * 0.2
            } else {
                0.0
            };
            for side in [-1.0_f32, 1.0] {
                let shoulder = Vec2::new(c.x + side * 0.3, c.y);
                let hand = shoulder
                    + Vec2::new(
                        side * 0.25,
                        -0.2 + wave + if mood == Mood::Cheering { 0.4 } else { 0.0 },
                    );
                mesh.rod(shoulder, hand, 0.08, -0.05, 0.05, rgb(SILVER));
                mesh.sphere(Vec3::new(hand.x, hand.y, 0.5), 0.07, 3, 6, rgb(SILVER));
            }
        }
        Crew::Nova => {
            let centre = Vec2::new(c.x, c.y);
            mesh.annulus(centre, 0.28, 0.42, 20, -0.08, 0.08, body);
            mesh.sphere(c, 0.2, 5, 10, shade(body, 1.3));
            eye(mesh, c + Vec3::new(0.0, 0.0, 0.17), 0.1);
            for i in 0..3 {
                let a = time * 1.5 + i as f32 * 2.1;
                let p = centre + Vec2::new(a.cos(), a.sin()) * 0.55;
                mesh.sphere(Vec3::new(p.x, p.y, 0.5), 0.05, 3, 6, rgb(0xE2D4FF));
            }
        }
        Crew::Tock => {
            let square = [
                Vec2::new(c.x - 0.36, c.y - 0.36),
                Vec2::new(c.x + 0.36, c.y - 0.36),
                Vec2::new(c.x + 0.36, c.y + 0.36),
                Vec2::new(c.x - 0.36, c.y + 0.36),
            ];
            mesh.prism(&square, -0.22, 0.22, body, shade(body, 0.75));
            // A clock face on its tummy, eyes above it.
            let face = Vec2::new(c.x, c.y - 0.1);
            mesh.disc(face, 0.19, 16, 0.0, 0.22, 0.26, rgb(0xFFF6D6));
            let hand = time * 0.8;
            mesh.rod(
                face,
                face + Vec2::new(hand.cos(), hand.sin()) * 0.14,
                0.035,
                0.26,
                0.29,
                pupil,
            );
            mesh.rod(face, face + Vec2::new(0.0, 0.1), 0.045, 0.26, 0.29, pupil);
            eye(mesh, Vec3::new(c.x - 0.14, c.y + 0.2, 0.75), 0.08);
            eye(mesh, Vec3::new(c.x + 0.14, c.y + 0.2, 0.75), 0.08);
        }
    }
}

/// A thruster's flame, from its nozzle outward, `throttle` from 0 to 1.
pub fn flame(glow: &mut Mesh, corners: &[Vec2], throttle: f32, flicker: f32) {
    if throttle <= 0.0 {
        return;
    }
    let tip = corners[2];
    let base_mid = corners[0].lerp(corners[1], 0.5);
    let dir = (tip - base_mid).normalize_or_zero();
    let side = dir.perp();
    let start = tip + dir * 0.18;
    let length = (0.6 + 0.25 * flicker) * throttle;
    for (width, len, hex) in [(0.22, 1.0, 0xFF7A1A), (0.12, 0.7, 0xFFE08A)] {
        let outline = ccw(vec![
            start - side * width,
            start + dir * (length * len),
            start + side * width,
        ]);
        glow.prism(&outline, -0.05, 0.05 + len * 0.02, rgb(hex), rgb(hex));
    }
}

/// The space behind everything: stars, a planet, and the sun.
#[must_use]
pub fn backdrop() -> (Mesh, Mesh) {
    let mut lit = Mesh::new();
    let mut glow = Mesh::new();
    // Stars from a fixed sequence, so every load shows the same sky.
    let mut seed: u32 = 0x5EED_1234;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    for _ in 0..420 {
        let (x, y) = (next() * 260.0 - 130.0, next() * 180.0 - 90.0);
        let size = 0.12 + next() * 0.25;
        let bright = 0.55 + next() * 0.45;
        let colour = mix([bright; 3], rgb(0xBFD8FF), next() * 0.5);
        glow.disc(Vec2::new(x, y), size, 4, 0.0, -90.0, -89.9, colour);
    }
    lit.sphere(Vec3::new(-55.0, -48.0, -85.0), 40.0, 16, 24, rgb(0x2E6FB8));
    lit.sphere(Vec3::new(-55.0, -48.0, -85.0), 40.6, 16, 24, rgb(0x3E8FD8));
    glow.disc(
        Vec2::new(70.0, 52.0),
        5.0,
        24,
        0.0,
        -88.0,
        -87.9,
        rgb(0xFFF4C2),
    );
    glow.annulus(
        Vec2::new(70.0, 52.0),
        5.0,
        8.0,
        24,
        -88.1,
        -88.0,
        rgb(0x6B5A2A),
    );
    (lit, glow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ]
    }

    #[test]
    fn every_piece_is_made_within_its_budget_and_in_its_own_colour() {
        let mut colours = std::collections::HashSet::new();
        for kind in PieceKind::ALL {
            colours.insert(colour(kind));
            if !kind.is_tile() {
                continue;
            }
            let corners: Vec<Vec2> = snap_rules::build::Build::held(kind, snap_rules::V::ZERO, 0)
                .iter()
                .map(|p| Vec2::new(p.x as f32, p.y as f32))
                .collect();
            let mut mesh = Mesh::new();
            tile(&mut mesh, kind, &corners, Style::default());
            assert!(mesh.triangles() > 0, "{kind:?}");
            assert!(mesh.triangles() <= 600, "{kind:?}: {}", mesh.triangles());
        }
        assert_eq!(colours.len(), PieceKind::ALL.len(), "a colour each");
        let mut mesh = Mesh::new();
        strut(&mut mesh, Vec2::ZERO, Vec2::ONE, Style::default());
        bolt(&mut mesh, Vec2::ZERO, Style::default());
        assert!(mesh.triangles() < 200);
    }

    #[test]
    fn a_broken_piece_is_gray_and_the_crew_fit_their_budget() {
        let mut bright = Mesh::new();
        tile(&mut bright, PieceKind::Square, &square(), Style::default());
        let mut broken = Mesh::new();
        tile(
            &mut broken,
            PieceKind::Square,
            &square(),
            Style {
                broken: true,
                ..Style::default()
            },
        );
        let chroma = |mesh: &Mesh| {
            mesh.colours()
                .iter()
                .map(|c| c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2]))
                .fold(0.0_f32, f32::max)
        };
        assert!(chroma(&broken) < chroma(&bright) * 0.5);
        for crew in Crew::ALL {
            for mood in [Mood::Waiting, Mood::Cheering, Mood::Thinking] {
                let mut mesh = Mesh::new();
                robot(&mut mesh, crew, Vec2::ZERO, 1.0, mood);
                assert!(mesh.triangles() <= 1500, "{crew:?}: {}", mesh.triangles());
            }
        }
        let (lit, glow) = backdrop();
        assert!(lit.triangles() + glow.triangles() < 8000);
    }
}
