//! The counter and its sign in the Grid's style: white lines on the
//! near-black field, dark faces that hide what stands behind them, and the
//! sign's block letters lit, as the Pylon Bay's and the Gym's signs are.
//!
//! Everything is in desk coordinates: x across the counter, y up, and z out
//! of its front toward the visitor.

pub type Point = [f64; 3];

/// Tones on the Grid's ladder, faint (0) to bright (3).
pub const QUIET: u8 = 1;
pub const PRESENT: u8 = 2;
pub const BRIGHT: u8 = 3;

/// The counter's top, where the devices stand, m.
pub const TOP: f64 = 1.06;
/// The counter's half width and half depth, m.
const HALF: [f64; 2] = [1.2, 0.35];
/// The top slab's overhang, m.
const LIP: f64 = 0.1;
/// The sign's letters: height, and the height of their foot, m.
const LETTERS: f64 = 0.5;
const SIGN_FOOT: f64 = 2.45;
/// The top of the sign's posts, m.
pub const SIGN_TOP: f64 = SIGN_FOOT + LETTERS + 0.2;
/// The sign stands this far behind the counter's middle, m.
pub const SIGN_BACK: f64 = -0.6;

/// Where the phone's stand stands on the counter: x and z, m.
pub const PHONE_STAND: [f64; 2] = [0.62, 0.08];
/// The height of the phone's back over the counter, m: its foot rests on
/// the stand's base (0.015 m) and its half length, 0.0755 m drawn at
/// [`crate::DEVICE_SCALE`], leans 72 degrees back (sin 72 = 0.951).
pub const PHONE_LIFT: f64 = 0.015 + 0.0755 * 2.0 * 0.951;

/// The desk as line art.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Art {
    /// Segments, each with its tone.
    pub lines: Vec<(Point, Point, u8)>,
    /// Dark faces that hide what stands behind them.
    pub faces: Vec<[Point; 4]>,
    /// Filled faces in a tone: the sign's letters.
    pub lit: Vec<([Point; 4], u8)>,
}

impl Art {
    fn line(&mut self, a: Point, b: Point, tone: u8) {
        if a != b {
            self.lines.push((a, b, tone));
        }
    }

    /// A box from `lo` to `hi`: its twelve edges and six dark faces.
    fn cuboid(&mut self, lo: Point, hi: Point, tone: u8) {
        let c = |i: usize| -> Point {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        };
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    self.line(c(i), c(i | bit), tone);
                }
            }
        }
        for q in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            self.faces.push(q.map(c));
        }
    }
}

/// The counter, the phone's stand, and the sign.
#[must_use]
pub fn desk() -> Art {
    let mut art = Art::default();
    let [w, d] = HALF;
    // The body, and the slab on top with its lip.
    art.cuboid([-w, 0.0, -d], [w, TOP - 0.06, d], PRESENT);
    art.cuboid(
        [-w - LIP, TOP - 0.06, -d - LIP],
        [w + LIP, TOP, d + LIP],
        PRESENT,
    );
    // The front: a kick line and a band under the lip.
    let front = d + 0.001;
    for y in [0.1, TOP - 0.2] {
        art.line([-w, y, front], [w, y, front], QUIET);
    }
    // Panels on the front, between the band and the kick.
    for x in [-w / 3.0, w / 3.0] {
        art.line([x, 0.1, front], [x, TOP - 0.2, front], QUIET);
    }
    // The phone's stand: a low base and two struts up to the phone's back.
    let [x, z] = PHONE_STAND;
    art.cuboid(
        [x - 0.07, TOP, z - 0.07],
        [x + 0.07, TOP + 0.015, z + 0.06],
        PRESENT,
    );
    for side in [-0.04, 0.04] {
        art.line(
            [x + side, TOP + 0.015, z - 0.06],
            [x + side, TOP + PHONE_LIFT - 0.01, z - 0.02],
            PRESENT,
        );
    }
    // The sign: two posts behind the counter and the letters between them.
    sign(&mut art, crate::SIGN);
    art
}

fn sign(art: &mut Art, text: &str) {
    let cell = LETTERS / 7.0;
    let width = (text.len() * 6 - 1) as f64 * cell;
    let half = width / 2.0 + 0.25;
    let z = SIGN_BACK;
    for x in [-half, half] {
        art.line([x, 0.0, z], [x, SIGN_TOP, z], PRESENT);
        art.line([x - 0.12, 0.0, z], [x + 0.12, 0.0, z], PRESENT);
    }
    art.line(
        [-half, SIGN_FOOT - 0.15, z],
        [half, SIGN_FOOT - 0.15, z],
        PRESENT,
    );
    let start = -width / 2.0;
    for (i, letter) in text.bytes().enumerate() {
        for (row, bits) in glyph(letter).into_iter().enumerate() {
            for (col, run) in runs(bits) {
                let x0 = start + (i * 6 + col) as f64 * cell;
                let y0 = SIGN_FOOT + (6 - row) as f64 * cell;
                let x1 = x0 + run as f64 * cell;
                art.lit.push((
                    [
                        [x0, y0, z],
                        [x1, y0, z],
                        [x1, y0 + cell, z],
                        [x0, y0 + cell, z],
                    ],
                    BRIGHT,
                ));
            }
        }
    }
}

/// A glyph row's lit cells as runs: where each starts and how many cells
/// it spans.
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

/// The sign's block letters: five columns by seven rows, as the Grid's
/// other signs draw them.
fn glyph(c: u8) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sign_spells_every_letter_and_stands_over_the_counter() {
        for letter in crate::SIGN.bytes().filter(|b| *b != b' ') {
            assert_ne!(glyph(letter), [0; 7], "{}", letter as char);
        }
        let art = desk();
        assert!(!art.lit.is_empty());
        let lowest = art
            .lit
            .iter()
            .flat_map(|(q, _)| q.iter().map(|p| p[1]))
            .fold(f64::MAX, f64::min);
        assert!(lowest > TOP + 1.0, "{lowest}");
        // Every letter sits between the posts.
        let posts = art
            .lines
            .iter()
            .filter(|(a, b, _)| a[0] == b[0] && a[1] == 0.0 && b[1] == SIGN_TOP)
            .map(|(a, _, _)| a[0])
            .collect::<Vec<_>>();
        assert_eq!(posts.len(), 2);
        for (q, _) in &art.lit {
            for p in q {
                assert!(p[0] > posts[0].min(posts[1]) && p[0] < posts[0].max(posts[1]));
            }
        }
    }
}
