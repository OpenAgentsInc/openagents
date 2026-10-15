//! Snap Station's rules (`docs/verse/games/snap-station.md`).
//!
//! Kids snap magnetic pieces together into a build, then press Go and watch
//! simple, honest physics: thrusters push, spinning makes gravity, magnets
//! let go under too much load unless a bolt locks them, squares lean and
//! pop while triangles and braced squares hold. Jobs are `snap.level.v1`
//! files ([`level`]); a run's inputs make a receipt that verifies by replay
//! ([`receipt`]).
//!
//! The build lies in a plane: pieces are flat tiles with edges one metre
//! long (a half square's long side is the square's diagonal), placed edge
//! to edge. Go turns the build into a truss ([`sim`]): every corner is a
//! point mass, every piece's sides are stiff bars, and every magnet join is
//! a stiff spring between two corners that lets go when it is pulled too
//! hard.
//!
//! **Determinism.** The simulation uses `f64` with only addition,
//! subtraction, multiplication, division and square roots, which IEEE 754
//! rounds exactly the same way on every platform Rust targets here (no
//! trigonometry, no fused multiply-add, no library maths), and loops in a
//! fixed order. Angles come from a table of exact constants ([`COS`],
//! [`SIN`]). So the same inputs on the same ticks end in the same bits on
//! native and wasm32; `scripts/snap-wasm-test.sh` checks a pinned digest.

pub mod bot;
pub mod build;
pub mod game;
pub mod kinds;
pub mod level;
pub mod progress;
pub mod receipt;
pub mod sim;

#[cfg(test)]
mod lessons;

pub use build::{Build, Placed, Seam};
pub use game::{Event, Fail, Game, Input, Status};
pub use kinds::PieceKind;
pub use level::{Job, Level};

/// Simulation ticks per second.
pub const HZ: u32 = 60;

/// Physics substeps per tick.
pub const SUBSTEPS: u32 = 12;

/// Seconds to ticks, rounded.
#[must_use]
pub fn ticks(seconds: f64) -> u32 {
    (seconds * f64::from(HZ) + 0.5) as u32
}

/// `sqrt(3) / 2`: an equilateral triangle's height.
pub const H: f64 = 0.866_025_403_784_438_6;

/// How many turns a held piece can take, in steps of 30 degrees.
pub const TURNS: u8 = 12;

/// Cosines of the turns, as exact constants.
pub const COS: [f64; 12] = [1.0, H, 0.5, 0.0, -0.5, -H, -1.0, -H, -0.5, 0.0, 0.5, H];

/// Sines of the turns, as exact constants.
pub const SIN: [f64; 12] = [0.0, 0.5, H, 1.0, H, 0.5, 0.0, -0.5, -H, -1.0, -H, -0.5];

/// A point or a direction in the build's plane, in metres: x to the right,
/// y up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V {
    pub x: f64,
    pub y: f64,
}

impl V {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// The z part of the cross product: positive when `other` is
    /// anticlockwise from `self`.
    #[must_use]
    pub fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    #[must_use]
    pub fn len2(self) -> f64 {
        self.dot(self)
    }

    #[must_use]
    pub fn len(self) -> f64 {
        self.len2().sqrt()
    }

    /// The same direction one long, or zero.
    #[must_use]
    pub fn unit(self) -> Self {
        let len = self.len();
        if len > 1e-12 {
            self * (1.0 / len)
        } else {
            Self::ZERO
        }
    }

    /// A quarter turn anticlockwise.
    #[must_use]
    pub fn perp(self) -> Self {
        Self::new(-self.y, self.x)
    }

    /// Rotated by the angle whose cosine and sine are `c` and `s`.
    #[must_use]
    pub fn rotate(self, c: f64, s: f64) -> Self {
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// Rotated by a turn from the table.
    #[must_use]
    pub fn turned(self, turn: u8) -> Self {
        let t = usize::from(turn % TURNS);
        self.rotate(COS[t], SIN[t])
    }

    #[must_use]
    pub fn mid(self, other: Self) -> Self {
        (self + other) * 0.5
    }
}

impl std::ops::Add for V {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl std::ops::Sub for V {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl std::ops::Mul<f64> for V {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k)
    }
}

impl std::ops::Neg for V {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl std::ops::AddAssign for V {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

impl std::ops::SubAssign for V {
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
    }
}

/// The turn from the table nearest to the direction `(c, s)`.
#[must_use]
pub fn nearest_turn(c: f64, s: f64) -> u8 {
    let mut best = 0;
    let mut score = f64::MIN;
    for t in 0..TURNS {
        let dot = COS[usize::from(t)] * c + SIN[usize::from(t)] * s;
        if dot > score {
            score = dot;
            best = t;
        }
    }
    best
}

/// Millimetres to metres, for the integer coordinates inputs carry.
#[must_use]
pub fn metres(mm: i32) -> f64 {
    f64::from(mm) / 1000.0
}

/// Metres to whole millimetres, rounded.
#[must_use]
pub fn mm(metres: f64) -> i32 {
    let scaled = metres * 1000.0;
    if scaled >= 0.0 {
        (scaled + 0.5) as i32
    } else {
        -((-scaled + 0.5) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turn_table_is_a_circle_of_unit_vectors() {
        for t in 0..usize::from(TURNS) {
            let len = COS[t] * COS[t] + SIN[t] * SIN[t];
            assert!((len - 1.0).abs() < 1e-15, "turn {t}");
            assert_eq!(nearest_turn(COS[t], SIN[t]), t as u8);
        }
        assert!((H * H - 0.75).abs() < 1e-15);
    }

    #[test]
    fn vectors_and_millimetres() {
        let a = V::new(3.0, 4.0);
        assert_eq!(a.len(), 5.0);
        assert_eq!(a.perp(), V::new(-4.0, 3.0));
        assert_eq!(V::new(1.0, 0.0).turned(3), V::new(0.0, 1.0));
        assert_eq!(mm(1.2345), 1235);
        assert_eq!(mm(-0.5004), -500);
        assert_eq!(metres(-1500), -1.5);
        assert_eq!(ticks(2.5), 150);
    }
}
