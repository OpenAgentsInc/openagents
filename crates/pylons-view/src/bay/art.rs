//! The Bay's line art, in the Grid's style: white lines on the near-black
//! field, with dark faces that hide what stands behind a machine. Verse
//! turns each piece into a model (`crates/everglade-web/src/pylon_bay.rs`).
//!
//! Lines carry a tone, a step on the Grid's ladder from faint (0) to
//! bright (3). A machine's outline is steady; its details light up while
//! it works, and its beam grows taller and brighter with its load.
//!
//! Machines are drawn facing the plaza (world −x) with their foot at the
//! origin, so Verse only moves them; the floor and the sign are drawn
//! where they stand.

use std::f64::consts::TAU;

use super::{
    HEAD_AT, Kind, LANE_LENGTH, LANES, Load, PAD, SECTION_PITCH, SECTION_WIDTH, SIGN_V, Shape,
    lane_start, member_at, world,
};

pub type Point = [f64; 3];

/// The faintest tone: the floor's markings.
pub const FAINT: u8 = 0;
/// A quiet tone: idle details.
pub const QUIET: u8 = 1;
/// A machine's outline.
pub const PRESENT: u8 = 2;
/// The brightest tone: lit details, beams at full load, pulses.
pub const BRIGHT: u8 = 3;

/// One piece of line art.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Art {
    /// Segments, each with its tone.
    pub lines: Vec<(Point, Point, u8)>,
    /// Dark faces that hide what stands behind them.
    pub faces: Vec<[Point; 4]>,
    /// Filled faces in a tone: the letters of the signs.
    pub lit: Vec<([Point; 4], u8)>,
}

/// A machine's own frame turned to face the plaza: its front (−z) looks
/// down world −x.
fn turn(p: Point) -> Point {
    [p[2], p[1], -p[0]]
}

impl Art {
    fn line(&mut self, a: Point, b: Point, tone: u8) {
        if a != b {
            self.lines.push((turn(a), turn(b), tone));
        }
    }

    fn face(&mut self, corners: [Point; 4]) {
        self.faces.push(corners.map(turn));
    }

    /// A box from `lo` to `hi`: its edges, and its faces when `solid`.
    fn cuboid(&mut self, lo: Point, hi: Point, tone: u8, solid: bool) {
        let c = |i: usize| -> Point {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        };
        // A flat box (a rectangle) has one outline, not two.
        let flat = (0..3).fold(0, |bits, axis| {
            if lo[axis] == hi[axis] {
                bits | 1 << axis
            } else {
                bits
            }
        });
        for i in (0..8).filter(|i| i & flat == 0) {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    self.line(c(i), c(i | bit), tone);
                }
            }
        }
        // No bottom face: every box stands on something.
        if solid {
            for f in [
                [0, 1, 3, 2],
                [4, 6, 7, 5],
                [2, 3, 7, 6],
                [0, 2, 6, 4],
                [1, 5, 7, 3],
            ] {
                self.face(f.map(c));
            }
        }
    }

    /// A horizontal ring at height `y` around (`x`, `z`).
    fn ring(&mut self, x: f64, y: f64, z: f64, radius: f64, sides: usize, tone: u8) {
        let at = |i: usize| {
            let a = i as f64 / sides as f64 * TAU;
            [x + radius * a.cos(), y, z + radius * a.sin()]
        };
        for i in 0..sides {
            self.line(at(i), at(i + 1), tone);
        }
    }

    /// The plinth every machine stands on: a low, sloped slab.
    fn plinth(&mut self) {
        let (low, high, y0, y1) = (0.7, 0.6, 0.02, 0.2);
        let corner = |h: f64, y: f64, i: usize| -> Point {
            let s = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)][i % 4];
            [s.0 * h, y, s.1 * h]
        };
        for i in 0..4 {
            self.line(corner(low, y0, i), corner(low, y0, i + 1), QUIET);
            self.line(corner(high, y1, i), corner(high, y1, i + 1), PRESENT);
            self.line(corner(low, y0, i), corner(high, y1, i), QUIET);
            self.face([
                corner(low, y0, i),
                corner(low, y0, i + 1),
                corner(high, y1, i + 1),
                corner(high, y1, i),
            ]);
        }
        self.face([0, 1, 2, 3].map(|i| corner(high, y1, i)));
    }

    /// The beam over a machine whose top is at `top`: a narrow column of
    /// light with a small crystal at its head, broken while it winds down.
    fn beam(&mut self, top: f64, load: Load) {
        let tone = match load {
            Load::Idle => FAINT,
            Load::Draining => QUIET,
            Load::Some => PRESENT,
            Load::Full => BRIGHT,
        };
        let (from, to) = (top + 0.15, top + load.beam());
        let w = 0.035;
        for (x, z) in [(-w, -w), (w, -w), (w, w), (-w, w)] {
            if load == Load::Draining {
                let mut y = from;
                while y < to {
                    self.line([x, y, z], [x, (y + 0.22).min(to), z], tone);
                    y += 0.4;
                }
            } else {
                self.line([x, from, z], [x, to, z], tone);
            }
        }
        let r = 0.11;
        let apex = to + r * 1.6;
        let mid = to + r * 0.8;
        let points = [[r, mid, 0.0], [0.0, mid, r], [-r, mid, 0.0], [0.0, mid, -r]];
        for i in 0..4 {
            self.line(points[i], points[(i + 1) % 4], tone);
            self.line(points[i], [0.0, apex, 0.0], tone);
            self.line(points[i], [0.0, to, 0.0], tone);
        }
    }
}

/// A machine of `shape` at `load`: plinth, body, and beam.
#[must_use]
pub fn machine(shape: Shape, load: Load) -> Art {
    let mut art = Art::default();
    art.plinth();
    let detail = if load.busy() { BRIGHT } else { QUIET };
    let outline = if load == Load::Draining {
        QUIET
    } else {
        PRESENT
    };
    match shape {
        Shape::Mac => {
            let (h, y0, y1) = (0.24, 0.2, 1.9);
            art.cuboid([-h, y0, -h], [h, y1, h], outline, true);
            // The crystal cap.
            let apex = [0.0, shape.top(), 0.0];
            let corners = [[-h, y1, -h], [h, y1, -h], [h, y1, h], [-h, y1, h]];
            for i in 0..4 {
                art.line(corners[i], apex, outline);
                art.face([corners[i], corners[(i + 1) % 4], apex, apex]);
            }
            // A seam down the front, and bands round the tower.
            art.line([0.0, 0.4, -h - 0.002], [0.0, 1.75, -h - 0.002], detail);
            for y in [0.7, 1.1, 1.5] {
                art.cuboid(
                    [-h - 0.002, y, -h - 0.002],
                    [h + 0.002, y, h + 0.002],
                    detail,
                    false,
                );
            }
        }
        Shape::Gpu | Shape::Part => {
            let (hx, hz, y0, y1) = if shape == Shape::Gpu {
                (0.5, 0.35, 0.2, 1.7)
            } else {
                (0.38, 0.3, 0.2, 1.45)
            };
            art.cuboid([-hx, y0, -hz], [hx, y1, hz], outline, true);
            // Blades across the front, each with its light.
            let front = -hz - 0.002;
            let blades = if shape == Shape::Gpu { 6 } else { 5 };
            for i in 0..blades {
                let y = y0 + 0.17 + f64::from(i) * 0.21;
                art.line([-hx + 0.16, y, front], [hx - 0.08, y, front], detail);
                art.line(
                    [-hx + 0.07, y - 0.04, front],
                    [-hx + 0.07, y + 0.04, front],
                    detail,
                );
            }
            // Fans on top.
            if shape == Shape::Gpu {
                for x in [-0.23, 0.23] {
                    art.ring(x, y1 + 0.002, 0.0, 0.17, 8, detail);
                    art.line(
                        [x - 0.12, y1 + 0.002, 0.0],
                        [x + 0.12, y1 + 0.002, 0.0],
                        detail,
                    );
                }
            }
        }
        Shape::Cpu => {
            let (h, y0, y1) = (0.45, 0.2, shape.top());
            art.cuboid([-h, y0, -h], [h, y1, h], outline, true);
            // A chip on the front, with its pins.
            let front = -h - 0.002;
            let (c, cy0, cy1) = (0.2, 0.42, 0.82);
            art.cuboid([-c, cy0, front], [c, cy1, front], detail, false);
            for i in 0..3 {
                let t = -0.12 + f64::from(i) * 0.12;
                art.line([t, cy1, front], [t, cy1 + 0.07, front], detail);
                art.line([t, cy0, front], [t, cy0 - 0.07, front], detail);
                let y = 0.62 + t;
                art.line([c, y, front], [c + 0.07, y, front], detail);
                art.line([-c, y, front], [-c - 0.07, y, front], detail);
            }
            // Fins on top.
            for i in 0..6 {
                let x = -0.3 + f64::from(i) * 0.12;
                art.line([x, y1 + 0.002, -0.32], [x, y1 + 0.002, 0.32], detail);
            }
        }
        Shape::Secure => {
            // The machine inside, and the cage round it.
            art.cuboid([-0.3, 0.2, -0.3], [0.3, 1.3, 0.3], outline, true);
            let (h, y0, y1) = (0.5, 0.2, shape.top());
            art.cuboid([-h, y0, -h], [h, y1, h], outline, false);
            let corners = |side: usize| -> [Point; 4] {
                let s = [
                    (-h, -h, h, -h),
                    (h, -h, h, h),
                    (h, h, -h, h),
                    (-h, h, -h, -h),
                ][side];
                [
                    [s.0, y0, s.1],
                    [s.2, y0, s.3],
                    [s.2, y1, s.3],
                    [s.0, y1, s.1],
                ]
            };
            for side in 0..4 {
                let c = corners(side);
                art.line(c[0], c[2], FAINT.max(detail.saturating_sub(1)));
                art.line(c[1], c[3], FAINT.max(detail.saturating_sub(1)));
            }
            // A padlock on the cage's front.
            let front = -h - 0.002;
            art.cuboid([-0.15, 1.05, front], [0.15, 1.27, front], detail, false);
            let arc = |i: usize| {
                let a = std::f64::consts::PI * i as f64 / 6.0;
                [0.1 * a.cos(), 1.27 + 0.12 * a.sin(), front]
            };
            for i in 0..6 {
                art.line(arc(i), arc(i + 1), detail);
            }
        }
        Shape::Head => {
            // An eight-sided router with a halo over it.
            let (r, y0, y1) = (0.45, 0.2, 1.3);
            let at = |i: usize, y: f64| -> Point {
                let a = (i as f64 + 0.5) / 8.0 * TAU;
                [r * a.cos(), y, r * a.sin()]
            };
            for i in 0..8 {
                art.line(at(i, y0), at(i + 1, y0), outline);
                art.line(at(i, y1), at(i + 1, y1), outline);
                art.line(at(i, y0), at(i, y1), outline);
                art.face([at(i, y0), at(i + 1, y0), at(i + 1, y1), at(i, y1)]);
            }
            let top = [0, 1, 2, 3, 4, 5, 6, 7].map(|i| at(i, y1));
            for i in [0, 2, 4] {
                art.face([top[0], top[i + 1], top[i + 2], top[i + 3]]);
            }
            let halo = shape.top() - 0.05;
            art.ring(0.0, halo, 0.0, 0.62, 16, detail);
            for i in [0, 2, 4, 6] {
                let a = (i as f64 + 0.5) / 8.0 * TAU;
                art.line(at(i, y1), [0.62 * a.cos(), halo, 0.62 * a.sin()], QUIET);
            }
            // Ports down the front.
            for k in 0..4 {
                let y = 0.45 + f64::from(k) * 0.2;
                art.line([-0.12, y, -r - 0.01], [0.12, y, -r - 0.01], detail);
            }
        }
    }
    art.beam(shape.top(), load);
    art
}

/// The pipe from a group's head to its `parts` parts, along the floor, from
/// the head's foot; lit while the group answers. In front of each part, as
/// many short marks as its place in the group.
#[must_use]
pub fn pipe(parts: u32, lit: bool) -> Art {
    let mut art = Art::default();
    let tone = if lit { BRIGHT } else { QUIET };
    let stops: Vec<f64> = std::iter::once(None)
        .chain((1..=parts).map(Some))
        .map(|part| member_at(part, parts) - HEAD_AT)
        .collect();
    // Bay coordinates from the head: `u` along the lane (world +z), `v`
    // across it (world +x).
    let at = |u: f64, v: f64, y: f64| -> Point { [v, y, u] };
    let y = 0.03;
    for pair in stops.windows(2) {
        let (a, b) = (pair[0] + 0.75, pair[1] - 0.75);
        if b <= a {
            continue;
        }
        for v in [-0.16, 0.16] {
            art.lines.push((at(a, v, y), at(b, v, y), tone));
        }
        let mut u = a;
        while u <= b + 1e-9 {
            art.lines.push((at(u, -0.16, y), at(u, 0.16, y), tone));
            u += 0.7;
        }
    }
    for (index, u) in stops.iter().enumerate().skip(1) {
        let marks = index.min(8) as f64;
        for k in 0..index.min(8) {
            let x = u + (k as f64 - (marks - 1.0) / 2.0) * 0.14;
            art.lines.push((at(x, -1.0, y), at(x, -0.82, y), QUIET));
        }
    }
    art
}

/// A pulse's ring on the floor, radius 1 m, in `tone`. Verse grows and
/// fades it when a job's receipt arrives.
#[must_use]
pub fn ring(tone: u8) -> Art {
    let mut art = Art::default();
    art.ring(0.0, 0.05, 0.0, 1.0, 32, tone);
    art
}

/// A spark that climbs a working machine's beam.
#[must_use]
pub fn spark() -> Art {
    let mut art = Art::default();
    art.ring(0.0, 0.0, 0.0, 0.16, 4, BRIGHT);
    art.ring(0.0, 0.0, 0.0, 0.09, 4, BRIGHT);
    art
}

/// The packet that runs down a group's pipe while it answers.
#[must_use]
pub fn packet() -> Art {
    let mut art = Art::default();
    let r = 0.15;
    let points = [[r, 0.0, 0.0], [0.0, 0.0, r], [-r, 0.0, 0.0], [0.0, 0.0, -r]];
    for i in 0..4 {
        art.lines.push((points[i], points[(i + 1) % 4], BRIGHT));
        art.lines.push((points[i], [0.0, r, 0.0], BRIGHT));
        art.lines.push((points[i], [0.0, -r, 0.0], BRIGHT));
    }
    art
}

/// Brackets on the floor round a machine under the pointer or with its
/// card open.
#[must_use]
pub fn focus() -> Art {
    let mut art = Art::default();
    let (h, arm, y) = (0.95, 0.32, 0.04);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = [sx * h, y, sz * h];
        art.lines
            .push((corner, [sx * (h - arm), y, sz * h], BRIGHT));
        art.lines
            .push((corner, [sx * h, y, sz * (h - arm)], BRIGHT));
    }
    art
}

/// The Bay's floor and sign, where they stand: its outline, a pad and a
/// name for each section, a lane for each model group, and the sign at its
/// front that reads PYLON BAY.
#[must_use]
pub fn floor() -> Art {
    let mut art = Art::default();
    let y = 0.012;
    let at = |u: f64, v: f64| -> Point {
        let p = world(u, v);
        [p[0], y, p[2]]
    };
    let rect = |art: &mut Art, u0: f64, u1: f64, v0: f64, v1: f64, tone: u8| {
        let c = [at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)];
        for i in 0..4 {
            art.lines.push((c[i], c[(i + 1) % 4], tone));
        }
    };
    let [u0, u1, v0, v1] = PAD;
    rect(&mut art, u0, u1, v0, v1, QUIET);
    rect(&mut art, u0 + 0.3, u1 - 0.3, v0 + 0.3, v1 - 0.3, FAINT);
    for kind in Kind::ALL {
        let start = kind as usize as f64 * SECTION_PITCH;
        rect(
            &mut art,
            start - 0.4,
            start + SECTION_WIDTH + 0.4,
            -0.9,
            6.5,
            FAINT,
        );
        floor_text(
            &mut art,
            kind.sign(),
            start + SECTION_WIDTH / 2.0,
            -1.85,
            0.6,
            QUIET,
        );
    }
    floor_text(&mut art, "MODEL GROUPS", (u0 + u1) / 2.0, 8.1, 0.6, QUIET);
    for lane in 0..LANES {
        if let Some((u, v)) = lane_start(lane) {
            rect(&mut art, u - 0.3, u + LANE_LENGTH, v - 1.2, v + 1.0, QUIET);
            // The head's own pad at the lane's start.
            rect(&mut art, u - 0.1, u + 2.1, v - 1.0, v + 0.8, FAINT);
        }
    }
    sign(&mut art, "PYLON BAY", (u0 + u1) / 2.0, SIGN_V, 1.4);
    art
}

/// `text` painted flat on the floor, centered on `u`, its letters `height`
/// tall from `v` into the Bay: it reads from the plaza.
fn floor_text(art: &mut Art, text: &str, u: f64, v: f64, height: f64, tone: u8) {
    let cell = height / 7.0;
    let width = (text.len() * 6 - 1) as f64 * cell;
    let start = u - width / 2.0;
    let y = 0.016;
    for (i, letter) in text.bytes().enumerate() {
        for (row, bits) in glyph(letter).into_iter().enumerate() {
            for (col, run) in runs(bits) {
                let u0 = start + (i * 6 + col) as f64 * cell;
                let v0 = v + (6 - row) as f64 * cell;
                let corner = |du: f64, dv: f64| {
                    let p = world(u0 + du, v0 + dv);
                    [p[0], y, p[2]]
                };
                let long = run as f64 * cell;
                art.lit.push((
                    [
                        corner(0.0, 0.0),
                        corner(long, 0.0),
                        corner(long, cell),
                        corner(0.0, cell),
                    ],
                    tone,
                ));
            }
        }
    }
}

/// The Bay's sign: `text` in bright letters `height` tall, raised on two
/// posts at `v` and facing the plaza, high enough to read over the
/// machines from across the plaza.
fn sign(art: &mut Art, text: &str, u: f64, v: f64, height: f64) {
    let cell = height / 7.0;
    let width = (text.len() * 6 - 1) as f64 * cell;
    let half = width / 2.0 + 0.5;
    let base = 3.2;
    let p = |u: f64, y: f64| -> Point {
        let w = world(u, v);
        [w[0], y, w[2]]
    };
    for side in [u - half, u + half] {
        art.lines
            .push((p(side, 0.0), p(side, base + height + 0.2), PRESENT));
        art.lines
            .push((p(side - 0.15, 0.0), p(side + 0.15, 0.0), PRESENT));
    }
    art.lines
        .push((p(u - half, base - 0.2), p(u + half, base - 0.2), PRESENT));
    let start = u - width / 2.0;
    for (i, letter) in text.bytes().enumerate() {
        for (row, bits) in glyph(letter).into_iter().enumerate() {
            for (col, run) in runs(bits) {
                let u0 = start + (i * 6 + col) as f64 * cell;
                let y0 = base + (6 - row) as f64 * cell;
                let long = run as f64 * cell;
                art.lit.push((
                    [
                        p(u0, y0),
                        p(u0 + long, y0),
                        p(u0 + long, y0 + cell),
                        p(u0, y0 + cell),
                    ],
                    BRIGHT,
                ));
            }
        }
    }
}

/// A glyph row's lit cells as runs: where each starts, and how many cells
/// it spans, so a row is a few quads rather than one per cell.
fn runs(bits: u8) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut col = 0;
    while col < 5 {
        if bits & (1 << (4 - col)) == 0 {
            col += 1;
            continue;
        }
        let start = col;
        while col < 5 && bits & (1 << (4 - col)) != 0 {
            col += 1;
        }
        out.push((start, col - start));
    }
    out
}

/// The block letters the Bay's signs use: five columns by seven rows, the
/// same as Verse's other signs (`crates/verse-core/src/label.rs`).
fn glyph(c: u8) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        _ => [0; 7],
    }
}

/// How many runs of cells a letter has, for the tests.
#[cfg(test)]
pub(super) fn tests_glyph_cells(c: u8) -> usize {
    glyph(c).iter().map(|row| runs(*row).len()).sum()
}
