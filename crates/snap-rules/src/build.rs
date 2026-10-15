//! The build: placed pieces, the magnet joins between them, struts and
//! bolts, and snapping.
//!
//! **Snapping.** A piece held near the build snaps when one of its sides
//! comes within [`SNAP_REACH`] of a free side of the build (middle to
//! middle) and the two sides line up within [`SNAP_ANGLE_DEGREES`], as
//! magnets do: it is pulled exactly onto that side, facing outward, as long
//! as it doesn't overlap another piece and stays inside the job's area.
//! Sides snap only to sides of the same length. Once placed, every side of
//! the new piece whose corners land on a free side's corners joins it too,
//! so a piece dropped into a corner joins both neighbours and a ring
//! closes by itself.
//!
//! A **seam** is one magnet join between two sides: two corner-to-corner
//! springs in the simulation. A bolt locks a seam so it can't let go. A
//! strut braces a square from corner to corner so it can't lean.

use crate::kinds::{PieceKind, side_len};
use crate::mechanism::Coupling;
use crate::{COS, SIN, V};

/// How close, middle to middle, a held side must come to a free side to
/// snap, in metres.
pub const SNAP_REACH: f64 = 0.7;

/// How well a held side must line up with a free side to snap.
pub const SNAP_ANGLE_DEGREES: u32 = 50;

/// The cosine of [`SNAP_ANGLE_DEGREES`].
pub const SNAP_COS: f64 = 0.642_787_609_686_539_4;

/// Corners this close join when a piece is placed, in metres.
pub const JOIN_GAP: f64 = 0.06;

/// How close a strut must be dropped to a square's middle, in metres.
pub const STRUT_REACH: f64 = 0.75;

/// How close a bolt must be dropped to a seam's middle, in metres.
pub const BOLT_REACH: f64 = 0.6;

/// How far inside another piece a point must be to count as an overlap.
const MARGIN: f64 = 0.02;

/// A placed piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub kind: PieceKind,
    /// Its corners, in the order of its kind's outline.
    pub at: Vec<V>,
    /// Given by the job: it can't be taken off.
    pub given: bool,
    /// The job's name for it, if any (`A`, `B`, ...).
    pub tag: Option<char>,
    /// A square's strut: 0 from corner 0 to 2, 1 from corner 1 to 3.
    pub strut: Option<u8>,
}

impl Placed {
    /// The middle of its corners.
    #[must_use]
    pub fn centre(&self) -> V {
        let mut sum = V::ZERO;
        for p in &self.at {
            sum += *p;
        }
        sum * (1.0 / self.at.len().max(1) as f64)
    }

    /// The corners of side `side`, start and end.
    #[must_use]
    pub fn side(&self, side: usize) -> (V, V) {
        (self.at[side], self.at[(side + 1) % self.at.len()])
    }

    /// Whether `point` is inside the outline.
    #[must_use]
    pub fn contains(&self, point: V) -> bool {
        inside(point, &self.at, 0.0)
    }
}

/// One magnet join: side `a_side` of piece `a` against side `b_side` of
/// piece `b`, running the other way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seam {
    pub a: usize,
    pub a_side: usize,
    pub b: usize,
    pub b_side: usize,
    pub bolted: bool,
    pub coupling: Coupling,
}

impl Seam {
    /// Its two corner-to-corner joins, as (piece, corner) pairs.
    #[must_use]
    pub fn joints(&self, build: &Build) -> [((usize, usize), (usize, usize)); 2] {
        let na = build.pieces[self.a].at.len();
        let nb = build.pieces[self.b].at.len();
        [
            ((self.a, self.a_side), (self.b, (self.b_side + 1) % nb)),
            ((self.a, (self.a_side + 1) % na), (self.b, self.b_side)),
        ]
    }

    /// Its middle.
    #[must_use]
    pub fn middle(&self, build: &Build) -> V {
        let (p, q) = build.pieces[self.a].side(self.a_side);
        p.mid(q)
    }
}

/// What taking a piece off gives back to the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Returned {
    pub kind: PieceKind,
    pub struts: u32,
    pub bolts: u32,
    /// Hinges, springs, and tethers installed on the removed joins.
    pub links: [u32; 3],
}

/// Whether `point` is inside the convex anticlockwise outline `shape`, by
/// more than `margin`.
#[must_use]
pub fn inside(point: V, shape: &[V], margin: f64) -> bool {
    let n = shape.len();
    if n < 3 {
        return false;
    }
    (0..n).all(|i| {
        let (a, b) = (shape[i], shape[(i + 1) % n]);
        let edge = b - a;
        edge.cross(point - a) > margin * edge.len()
    })
}

/// Whether two convex outlines overlap by more than touching: a corner, a
/// side's middle, or the centre of one lies inside the other.
#[must_use]
pub fn overlaps(a: &[V], b: &[V]) -> bool {
    fn probes(shape: &[V]) -> Vec<V> {
        let n = shape.len();
        let mut out = Vec::with_capacity(2 * n + 1);
        let mut sum = V::ZERO;
        for i in 0..n {
            out.push(shape[i]);
            out.push(shape[i].mid(shape[(i + 1) % n]));
            sum += shape[i];
        }
        out.push(sum * (1.0 / n as f64));
        out
    }
    probes(a).into_iter().any(|p| inside(p, b, MARGIN))
        || probes(b).into_iter().any(|p| inside(p, a, MARGIN))
}

/// The build and the job's area it must stay in.
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub pieces: Vec<Placed>,
    pub seams: Vec<Seam>,
    /// The lowest and highest corners a piece may have.
    pub area: (V, V),
}

impl Build {
    #[must_use]
    pub fn new(area: (V, V)) -> Self {
        Self {
            pieces: Vec::new(),
            seams: Vec::new(),
            area,
        }
    }

    /// Whether side `side` of piece `piece` is in a seam.
    #[must_use]
    pub fn joined(&self, piece: usize, side: usize) -> bool {
        self.seams
            .iter()
            .any(|s| (s.a == piece && s.a_side == side) || (s.b == piece && s.b_side == side))
    }

    /// Every side that can still take a piece: a snapping side in no seam.
    #[must_use]
    pub fn free_sides(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (index, piece) in self.pieces.iter().enumerate() {
            for side in piece.kind.snap_sides() {
                if !self.joined(index, *side) {
                    out.push((index, *side));
                }
            }
        }
        out
    }

    /// A tile of `kind` held with its centre at `at`, turned `turn` steps.
    #[must_use]
    pub fn held(kind: PieceKind, at: V, turn: u8) -> Vec<V> {
        let centre = kind.centre();
        kind.shape()
            .iter()
            .map(|p| at + (*p - centre).turned(turn))
            .collect()
    }

    /// The corners of a tile of `kind` put with its side `side` against
    /// the free side `onto` (piece, side), facing out.
    #[must_use]
    pub fn onto(&self, kind: PieceKind, side: usize, onto: (usize, usize)) -> Vec<V> {
        let shape = kind.shape();
        let (fa, fb) = self.pieces[onto.0].side(onto.1);
        let la = shape[side];
        let lb = shape[(side + 1) % shape.len()];
        let u = (lb - la).unit();
        let v = (fa - fb).unit();
        let (c, s) = (u.dot(v), u.cross(v));
        shape.iter().map(|p| fb + (*p - la).rotate(c, s)).collect()
    }

    /// Whether corners would sit in the area without overlapping a piece.
    #[must_use]
    pub fn fits(&self, corners: &[V]) -> bool {
        let (low, high) = self.area;
        corners
            .iter()
            .all(|p| p.x >= low.x && p.y >= low.y && p.x <= high.x && p.y <= high.y)
            && self
                .pieces
                .iter()
                .all(|piece| !overlaps(corners, &piece.at))
    }

    /// Where a tile of `kind` held at `at`, turned `turn`, would snap, if
    /// anywhere: the nearest free side within reach that lines up and has
    /// room.
    #[must_use]
    pub fn candidate(&self, kind: PieceKind, at: V, turn: u8) -> Option<Vec<V>> {
        if !kind.is_tile() {
            return None;
        }
        let held = Self::held(kind, at, turn);
        let shape = kind.shape();
        let mut options: Vec<(f64, usize, (usize, usize))> = Vec::new();
        for side in kind.snap_sides() {
            let (ha, hb) = (held[*side], held[(*side + 1) % held.len()]);
            let len = side_len(shape, *side);
            let (mid, dir) = (ha.mid(hb), (hb - ha).unit());
            for free in self.free_sides() {
                let piece = &self.pieces[free.0];
                if (side_len(&piece.at, free.1) - len).abs() > 0.01 {
                    continue;
                }
                let (fa, fb) = piece.side(free.1);
                let gap = (fa.mid(fb) - mid).len2();
                if gap > SNAP_REACH * SNAP_REACH {
                    continue;
                }
                if dir.dot((fa - fb).unit()) < SNAP_COS {
                    continue;
                }
                options.push((gap, *side, free));
            }
        }
        options.sort_by(|x, y| x.0.total_cmp(&y.0));
        options
            .into_iter()
            .map(|(_, side, free)| self.onto(kind, side, free))
            .find(|corners| self.fits(corners))
    }

    /// Puts a tile down at `corners` and joins every side that meets a
    /// free side. Returns its index.
    pub fn place(
        &mut self,
        kind: PieceKind,
        corners: Vec<V>,
        given: bool,
        tag: Option<char>,
    ) -> usize {
        let index = self.pieces.len();
        self.pieces.push(Placed {
            kind,
            at: corners,
            given,
            tag,
            strut: None,
        });
        self.join(index);
        index
    }

    /// Joins every free side of piece `index` whose corners meet another
    /// piece's free side.
    pub fn join(&mut self, index: usize) {
        let gap2 = JOIN_GAP * JOIN_GAP;
        let kind = self.pieces[index].kind;
        for side in kind.snap_sides() {
            if self.joined(index, *side) {
                continue;
            }
            let (c, d) = self.pieces[index].side(*side);
            let len = side_len(&self.pieces[index].at, *side);
            let hit = self.free_sides().into_iter().find(|(other, other_side)| {
                if *other == index {
                    return false;
                }
                let piece = &self.pieces[*other];
                let (a, b) = piece.side(*other_side);
                (side_len(&piece.at, *other_side) - len).abs() < 0.01
                    && (c - b).len2() < gap2
                    && (d - a).len2() < gap2
            });
            if let Some((other, other_side)) = hit {
                self.seams.push(Seam {
                    a: other,
                    a_side: other_side,
                    b: index,
                    b_side: *side,
                    bolted: false,
                    coupling: if self.pieces[index].kind == PieceKind::Wheel
                        || self.pieces[other].kind == PieceKind::Wheel
                    {
                        Coupling::Wheel
                    } else if self.pieces[index].kind == PieceKind::CargoArm
                        || self.pieces[other].kind == PieceKind::CargoArm
                    {
                        Coupling::CargoArm
                    } else {
                        Coupling::Magnet
                    },
                });
            }
        }
    }

    /// The square a strut dropped at `at`, held at `turn`, braces, and
    /// which diagonal: the nearest unbraced square within reach whose
    /// diagonal lines up.
    #[must_use]
    pub fn strut_target(&self, at: V, turn: u8) -> Option<(usize, u8)> {
        let held = V::new(COS[usize::from(turn % 12)], SIN[usize::from(turn % 12)]);
        let mut best: Option<(f64, usize, u8)> = None;
        for (index, piece) in self.pieces.iter().enumerate() {
            if piece.kind != PieceKind::Square || piece.strut.is_some() {
                continue;
            }
            let gap = (piece.centre() - at).len2();
            if gap > STRUT_REACH * STRUT_REACH {
                continue;
            }
            let d0 = (piece.at[2] - piece.at[0]).unit();
            let d1 = (piece.at[3] - piece.at[1]).unit();
            let (a0, a1) = (held.dot(d0).abs(), held.dot(d1).abs());
            let (diagonal, line) = if a0 >= a1 { (0, a0) } else { (1, a1) };
            if line < SNAP_COS {
                continue;
            }
            if best.is_none_or(|(old, _, _)| gap < old) {
                best = Some((gap, index, diagonal));
            }
        }
        best.map(|(_, index, diagonal)| (index, diagonal))
    }

    /// The seam a bolt dropped at `at` locks: the nearest unbolted seam
    /// within reach.
    #[must_use]
    pub fn bolt_target(&self, at: V) -> Option<usize> {
        let mut best: Option<(f64, usize)> = None;
        for (index, seam) in self.seams.iter().enumerate() {
            if seam.bolted {
                continue;
            }
            let gap = (seam.middle(self) - at).len2();
            if gap <= BOLT_REACH * BOLT_REACH && best.is_none_or(|(old, _)| gap < old) {
                best = Some((gap, index));
            }
        }
        best.map(|(_, index)| index)
    }

    /// A plain join that can take a hinge, spring, or tether.
    pub fn link_target(&self, at: V) -> Option<usize> {
        self.seams
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.coupling == Coupling::Magnet
                    && !(self.pieces[s.a].given && self.pieces[s.b].given)
            })
            .map(|(i, s)| ((s.middle(self) - at).len2(), i))
            .filter(|(d, _)| *d <= BOLT_REACH * BOLT_REACH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, i)| i)
    }

    /// The piece at `at` that may be taken off: the last placed one under
    /// the point that the job didn't give.
    #[must_use]
    pub fn piece_at(&self, at: V) -> Option<usize> {
        (0..self.pieces.len())
            .rev()
            .find(|i| !self.pieces[*i].given && self.pieces[*i].contains(at))
    }

    /// Takes piece `index` off, with its seams; their bolts and its strut
    /// go back to the tray.
    pub fn remove(&mut self, index: usize) -> Returned {
        let piece = self.pieces.remove(index);
        let mut bolts = 0;
        let mut links = [0; 3];
        self.seams.retain(|s| {
            let gone = s.a == index || s.b == index;
            if gone {
                match s.coupling {
                    Coupling::Hinge => links[0] += 1,
                    Coupling::Spring => links[1] += 1,
                    Coupling::Tether => links[2] += 1,
                    _ => {}
                }
            }
            if gone && s.bolted {
                bolts += 1;
            }
            !gone
        });
        for seam in &mut self.seams {
            if seam.a > index {
                seam.a -= 1;
            }
            if seam.b > index {
                seam.b -= 1;
            }
        }
        Returned {
            kind: piece.kind,
            struts: u32::from(piece.strut.is_some()),
            bolts,
            links,
        }
    }

    /// Each piece's group: pieces joined by seams, directly or through
    /// others, share the lowest index among them.
    #[must_use]
    pub fn groups(&self) -> Vec<usize> {
        groups(self.pieces.len(), self.seams.iter().map(|s| (s.a, s.b)))
    }

    /// The piece the job tagged `tag`.
    #[must_use]
    pub fn tagged(&self, tag: char) -> Option<usize> {
        self.pieces.iter().position(|p| p.tag == Some(tag))
    }

    /// How many pieces, struts and bolts the player put in.
    #[must_use]
    pub fn used(&self) -> u32 {
        let pieces = self.pieces.iter().filter(|p| !p.given).count();
        let struts = self.pieces.iter().filter(|p| p.strut.is_some()).count();
        let bolts = self
            .seams
            .iter()
            .filter(|s| s.bolted && !(self.pieces[s.a].given && self.pieces[s.b].given))
            .count();
        let links = self
            .seams
            .iter()
            .filter(|s| {
                matches!(
                    s.coupling,
                    Coupling::Hinge | Coupling::Spring | Coupling::Tether
                )
            })
            .count();
        (pieces + struts + bolts + links) as u32
    }
}

/// Groups of `count` things joined by `links`: each gets the lowest index
/// in its group.
#[must_use]
pub fn groups(count: usize, links: impl Iterator<Item = (usize, usize)>) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..count).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (a, b) in links {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb {
            let (low, high) = if ra < rb { (ra, rb) } else { (rb, ra) };
            parent[high] = low;
        }
    }
    (0..count).map(|i| root(&mut parent, i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> (V, V) {
        (V::new(-10.0, -10.0), V::new(10.0, 10.0))
    }

    fn one_module() -> Build {
        let mut build = Build::new(area());
        let corners = Build::held(PieceKind::Module, V::new(0.5, 0.5), 0);
        build.place(PieceKind::Module, corners, true, Some('A'));
        build
    }

    #[test]
    fn a_square_held_near_a_side_snaps_flush_against_it() {
        let build = one_module();
        // Held a little off to the right of the module, a little turned.
        let corners = build
            .candidate(PieceKind::Square, V::new(1.75, 0.6), 1)
            .expect("snaps");
        let mut xs: Vec<f64> = corners.iter().map(|p| p.x).collect();
        xs.sort_by(f64::total_cmp);
        assert!(
            (xs[0] - 1.0).abs() < 1e-12 && (xs[3] - 2.0).abs() < 1e-12,
            "{corners:?}"
        );
        let mut build = build;
        let index = build.place(PieceKind::Square, corners, false, None);
        assert_eq!(build.seams.len(), 1);
        assert_eq!(build.seams[0].b, index);
        assert_eq!(build.groups(), vec![0, 0]);
    }

    #[test]
    fn far_away_misaligned_or_overlapping_pieces_do_not_snap() {
        let build = one_module();
        assert!(
            build
                .candidate(PieceKind::Square, V::new(3.0, 0.5), 0)
                .is_none()
        );
        // A triangle's sides point every 120 degrees. Turned a quarter
        // turn, none is within 50 degrees of the module's side, so it
        // doesn't snap; turned 30 degrees, one lines up exactly.
        assert!(
            build
                .candidate(PieceKind::Triangle, V::new(1.4, 0.5), 3)
                .is_none()
        );
        assert!(
            build
                .candidate(PieceKind::Triangle, V::new(1.4, 0.5), 1)
                .is_some()
        );
        // Dropped on top of the module: no room.
        assert!(
            build
                .candidate(PieceKind::Square, V::new(0.5, 0.5), 0)
                .is_none()
        );
        // Outside the area.
        let mut small = Build::new((V::new(0.0, 0.0), V::new(1.0, 1.0)));
        small.place(
            PieceKind::Module,
            Build::held(PieceKind::Module, V::new(0.5, 0.5), 0),
            true,
            None,
        );
        assert!(
            small
                .candidate(PieceKind::Square, V::new(1.5, 0.5), 0)
                .is_none()
        );
    }

    #[test]
    fn a_half_square_long_side_snaps_only_to_another_long_side() {
        let mut build = Build::new(area());
        let wedge = Build::held(PieceKind::Wedge, V::new(0.0, 0.0), 0);
        build.place(PieceKind::Wedge, wedge, false, None);
        // A second wedge held across the long side, turned half way round.
        let centre = build.pieces[0].centre();
        let other = build
            .candidate(PieceKind::Wedge, centre + V::new(0.35, 0.35), 6)
            .expect("snaps on the long side");
        let index = build.place(PieceKind::Wedge, other, false, None);
        let seam = build.seams[0];
        assert_eq!((seam.a_side, seam.b), (1, index));
        // A square never snaps to the long side.
        let mut lone = Build::new(area());
        lone.place(
            PieceKind::Wedge,
            Build::held(PieceKind::Wedge, V::ZERO, 0),
            false,
            None,
        );
        let square = lone.candidate(PieceKind::Square, centre + V::new(0.6, 0.6), 1);
        if let Some(corners) = square {
            lone.place(PieceKind::Square, corners, false, None);
            assert_ne!(lone.seams[0].a_side, 1, "not on the long side");
        }
    }

    #[test]
    fn a_piece_dropped_into_a_corner_joins_both_neighbours() {
        let mut build = one_module();
        let right = build
            .candidate(PieceKind::Square, V::new(1.5, 0.5), 0)
            .unwrap();
        build.place(PieceKind::Square, right, false, None);
        let up = build
            .candidate(PieceKind::Square, V::new(0.5, 1.5), 0)
            .unwrap();
        build.place(PieceKind::Square, up, false, None);
        let corner = build
            .candidate(PieceKind::Square, V::new(1.5, 1.5), 0)
            .unwrap();
        build.place(PieceKind::Square, corner, false, None);
        assert_eq!(build.seams.len(), 4);
    }

    #[test]
    fn struts_bolts_and_taking_pieces_off() {
        let mut build = one_module();
        let corners = build
            .candidate(PieceKind::Square, V::new(1.5, 0.5), 0)
            .unwrap();
        let square = build.place(PieceKind::Square, corners, false, None);
        let (target, diagonal) = build.strut_target(V::new(1.5, 0.5), 1).expect("braces");
        assert_eq!(target, square);
        build.pieces[target].strut = Some(diagonal);
        assert!(build.strut_target(V::new(1.5, 0.5), 1).is_none(), "once");
        let seam = build.bolt_target(V::new(1.1, 0.5)).expect("a seam");
        build.seams[seam].bolted = true;
        assert!(build.bolt_target(V::new(1.1, 0.5)).is_none());
        assert_eq!(build.used(), 3);
        assert_eq!(build.piece_at(V::new(0.5, 0.5)), None, "given pieces stay");
        let back = build.remove(build.piece_at(V::new(1.5, 0.5)).unwrap());
        assert_eq!(
            back,
            Returned {
                kind: PieceKind::Square,
                struts: 1,
                bolts: 1,
                links: [0; 3],
            }
        );
        assert!(build.seams.is_empty());
    }

    #[test]
    fn overlap_needs_more_than_touching() {
        let a = Build::held(PieceKind::Square, V::new(0.5, 0.5), 0);
        let b = Build::held(PieceKind::Square, V::new(1.5, 0.5), 0);
        let c = Build::held(PieceKind::Square, V::new(1.2, 0.7), 0);
        assert!(!overlaps(&a, &b));
        assert!(overlaps(&a, &c));
        assert!(overlaps(&a, &a));
    }
}
