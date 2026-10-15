//! The pieces: their shapes, which sides snap, their masses, and how much
//! of the solar wind they catch.
//!
//! Every tile is a convex outline listed anticlockwise, so a side running
//! from corner `i` to corner `i + 1` has the tile on its left. Sides are
//! one metre long, except a half square's long side, which is a square's
//! diagonal; sides snap only to sides of the same length.

use crate::{H, V};

/// A kind of piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PieceKind {
    /// A square frame: four rods with magnets in the corners. Pushed
    /// sideways it leans, and leaning too far pops it apart.
    Square,
    /// An equilateral triangle: it can't lean.
    Triangle,
    /// A half square (a right triangle): two make a square that can't lean.
    Wedge,
    /// A solid square panel: stiff, twice a square's mass.
    Panel,
    /// A solar wing, one by two metres: stiff, light, and it catches the
    /// solar wind.
    Solar,
    /// A thruster: snapped on by its base, it pushes the build away from
    /// its nozzle while it burns.
    Thruster,
    /// A heavy cargo crate.
    Cargo,
    /// A plant pod; plants need gravity, and spinning makes it.
    Pod,
    /// A station module, the crew's own; jobs start with some.
    Module,
    /// A strut: dropped on a square, it braces it corner to corner so it
    /// can't lean.
    Strut,
    /// A bolt: dropped on a magnet join, it locks it so it can't let go.
    Bolt,
    Hinge,
    Spring,
    Tether,
    Wheel,
    CargoArm,
}

impl PieceKind {
    pub const ALL: [Self; 16] = [
        Self::Square,
        Self::Triangle,
        Self::Wedge,
        Self::Panel,
        Self::Solar,
        Self::Thruster,
        Self::Cargo,
        Self::Pod,
        Self::Module,
        Self::Strut,
        Self::Bolt,
        Self::Hinge,
        Self::Spring,
        Self::Tether,
        Self::Wheel,
        Self::CargoArm,
    ];

    /// The word level files and receipts use.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Triangle => "triangle",
            Self::Wedge => "wedge",
            Self::Panel => "panel",
            Self::Solar => "solar",
            Self::Thruster => "thruster",
            Self::Cargo => "cargo",
            Self::Pod => "pod",
            Self::Module => "module",
            Self::Strut => "strut",
            Self::Bolt => "bolt",
            Self::Hinge => "hinge",
            Self::Spring => "spring",
            Self::Tether => "tether",
            Self::Wheel => "wheel",
            Self::CargoArm => "cargo-arm",
        }
    }

    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }

    /// Whether the piece is a tile with corners; struts and bolts are
    /// dropped onto tiles and joins instead.
    #[must_use]
    pub fn is_tile(self) -> bool {
        !matches!(
            self,
            Self::Strut | Self::Bolt | Self::Hinge | Self::Spring | Self::Tether
        )
    }

    /// The outline, anticlockwise, in metres.
    #[must_use]
    pub fn shape(self) -> &'static [V] {
        const SQUARE: [V; 4] = [
            V::new(0.0, 0.0),
            V::new(1.0, 0.0),
            V::new(1.0, 1.0),
            V::new(0.0, 1.0),
        ];
        const TRIANGLE: [V; 3] = [V::new(0.0, 0.0), V::new(1.0, 0.0), V::new(0.5, H)];
        const WEDGE: [V; 3] = [V::new(0.0, 0.0), V::new(1.0, 0.0), V::new(0.0, 1.0)];
        const SOLAR: [V; 6] = [
            V::new(0.0, 0.0),
            V::new(1.0, 0.0),
            V::new(1.0, 1.0),
            V::new(1.0, 2.0),
            V::new(0.0, 2.0),
            V::new(0.0, 1.0),
        ];
        const THRUSTER: [V; 3] = [V::new(0.0, 0.0), V::new(1.0, 0.0), V::new(0.5, 0.75)];
        match self {
            Self::Square | Self::Panel | Self::Cargo | Self::Pod | Self::Module | Self::Wheel => {
                &SQUARE
            }
            Self::Triangle => &TRIANGLE,
            Self::Wedge => &WEDGE,
            Self::Solar | Self::CargoArm => &SOLAR,
            Self::Thruster => &THRUSTER,
            Self::Strut | Self::Bolt | Self::Hinge | Self::Spring | Self::Tether => &[],
        }
    }

    /// The sides that snap, by their first corner. A thruster snaps only
    /// by its base.
    #[must_use]
    pub fn snap_sides(self) -> &'static [usize] {
        match self {
            Self::Thruster | Self::CargoArm | Self::Wheel => &[0],
            Self::Triangle | Self::Wedge => &[0, 1, 2],
            Self::Solar => &[0, 1, 2, 3, 4, 5],
            Self::Square | Self::Panel | Self::Cargo | Self::Pod | Self::Module => &[0, 1, 2, 3],
            Self::Strut | Self::Bolt | Self::Hinge | Self::Spring | Self::Tether => &[],
        }
    }

    /// Stiff bars inside the outline (the outline's own sides are always
    /// bars), by corner pairs.
    #[must_use]
    pub fn inner_bars(self) -> &'static [(usize, usize)] {
        match self {
            Self::Panel | Self::Cargo | Self::Pod | Self::Module | Self::Wheel => &[(0, 2), (1, 3)],
            Self::Solar | Self::CargoArm => &[(0, 2), (1, 5), (5, 2), (5, 3), (2, 4)],
            _ => &[],
        }
    }

    /// Whether the piece leans: only the square frame.
    #[must_use]
    pub fn leans(self) -> bool {
        self == Self::Square
    }

    /// Mass in kilograms.
    #[must_use]
    pub fn mass(self) -> f64 {
        match self {
            Self::Square => 1.0,
            Self::Triangle => 0.6,
            Self::Wedge => 0.6,
            Self::Panel => 2.0,
            Self::Solar => 0.8,
            Self::Thruster => 1.0,
            Self::Cargo => 6.0,
            Self::Pod => 3.0,
            Self::Module => 4.0,
            Self::Strut => 0.3,
            Self::Bolt | Self::Hinge | Self::Spring | Self::Tether => 0.0,
            Self::Wheel => 1.4,
            Self::CargoArm => 2.2,
        }
    }

    /// The area the solar wind pushes on, in square metres: frames let it
    /// through, panels and wings catch it.
    #[must_use]
    pub fn sail(self) -> f64 {
        match self {
            Self::Square => 0.1,
            Self::Triangle | Self::Wedge => 0.05,
            Self::Panel | Self::Cargo | Self::Pod | Self::Module => 1.0,
            Self::Solar => 2.0,
            Self::Thruster => 0.3,
            Self::Strut | Self::Bolt | Self::Hinge | Self::Spring | Self::Tether => 0.0,
            Self::Wheel => 0.2,
            Self::CargoArm => 0.3,
        }
    }

    /// The outline's centre.
    #[must_use]
    pub fn centre(self) -> V {
        let shape = self.shape();
        if shape.is_empty() {
            return V::ZERO;
        }
        let mut sum = V::ZERO;
        for p in shape {
            sum += *p;
        }
        sum * (1.0 / shape.len() as f64)
    }
}

/// The length of side `side` of an outline.
#[must_use]
pub fn side_len(shape: &[V], side: usize) -> f64 {
    (shape[(side + 1) % shape.len()] - shape[side]).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outline_is_anticlockwise_and_convex_with_metre_sides() {
        assert!(
            PieceKind::ALL
                .iter()
                .all(|k| k.is_tile() == !k.shape().is_empty())
        );
        for kind in PieceKind::ALL.into_iter().filter(|k| k.is_tile()) {
            let shape = kind.shape();
            let n = shape.len();
            for i in 0..n {
                let (a, b, c) = (shape[i], shape[(i + 1) % n], shape[(i + 2) % n]);
                // A solar wing's long sides have a corner in the middle.
                assert!((b - a).cross(c - b) >= 0.0, "{kind:?} turns left at {i}");
            }
            for side in kind.snap_sides() {
                let len = side_len(shape, *side);
                let ok = (len - 1.0).abs() < 1e-12
                    || (kind == PieceKind::Wedge && (len - 2.0_f64.sqrt()).abs() < 1e-12);
                assert!(ok, "{kind:?} side {side} is {len}");
            }
            assert!(kind.mass() > 0.0);
        }
        assert_eq!(PieceKind::from_word("solar"), Some(PieceKind::Solar));
        assert_eq!(PieceKind::from_word("glue"), None);
        assert!(PieceKind::Square.leans() && !PieceKind::Triangle.leans());
    }
}
