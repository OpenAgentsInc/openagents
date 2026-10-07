//! Houses built from the licensed medieval kit
//! (`docs/verse/everglade-medieval-refactor.md`, "Lots, bays, and stories").
//!
//! A kit house is a lot, a facing, a story count, and a style. Its outline is
//! the lot: corner pieces 1 m along each side at every corner and wall bays
//! of 4 m and 2 m between them, so an 8 m front is a 4 m and a 2 m bay and a
//! 10 m side two 4 m bays. Each story is a 4 m wall under a 0.5 m timber
//! band, 4.5 m floor to floor, on a 2 m stone plinth sunk 1.25 m into the
//! ground, so the floor is 0.75 m up and steps climb to the door. A gabled
//! roof spans the depth from a ridge parallel to the front, with 1 m eaves
//! front and back and its gables on the sides.
//!
//! Every piece is a `kit/` model ([`crate::zones::everglade_pack::kit`]),
//! placed with no collision of its own. A walker meets each piece's proxy,
//! carved as one block (`demolition::carve`), so walking is the same with or
//! without the licensed kit. The villagers route around
//! [`KitHouse::walls`], the walls and plinth as blockers open at the door.
//! A lot may be turned from its street; the blockers of a turned house are
//! axis-aligned boxes around 1 m lengths of its walls.
//! [`KitHouse::surfaces`] states the walk up the steps and through the door
//! that the proxies give, as boxes, for tests.

use super::{Collision, Placement, height, noise};
use crate::controller::Footprint;
use std::f32::consts::{FRAC_PI_2, PI};

/// Floor to floor, m: a 4 m wall and the 0.5 m band over it.
pub const STORY: f32 = 4.5;
/// A wall's height, m.
pub const WALL: f32 = 4.0;
/// How high the ground floor stands over the ground at the lot's center, m:
/// the plinth's 2 m less the 1.25 m it is sunk.
pub const FLOOR_RISE: f32 = 0.75;
/// The plinth's height, m.
const PLINTH: f32 = 2.0;
/// How far the plinth stands out from the walls' face, m.
const PLINTH_OUT: f32 = 0.25;
/// A wall's thickness behind its outer face, m.
const THICK: f32 = 0.5;
/// The roof's rise from its eaves to the ridge, m, and the ridge caps'
/// height over the roof's base.
const RIDGE_CAP: f32 = 3.75;
/// A 4 m doorway's opening along its piece, m.
const DOOR_4: (f32, f32) = (1.13, 2.87);
/// A 2 m doorway's opening along its piece, m.
const DOOR_2: (f32, f32) = (0.2, 1.8);
/// The steps' run out from the plinth's face, m, and each step's height.
const STEP_RUN: f32 = 0.35;
const STEP_RISE: f32 = 0.25;

/// How a kit house's walls look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KitStyle {
    /// Plain plaster with windows.
    Plaster,
    /// Plaster with timber framing.
    Timber,
}

/// One house of the kit on its lot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KitHouse {
    pub name: &'static str,
    /// The lot's center, x and z, m.
    pub center: [f32; 2],
    /// The outline along the front and from front to back, m: the width
    /// 2 m plus 4 m and 2 m bays, the depth 10 m.
    pub width: f32,
    pub depth: f32,
    /// The front's outward heading, as the controller's yaw.
    pub facing: f32,
    pub stories: u8,
    pub style: KitStyle,
    /// Which front bay, from the left seen from the street, holds the door.
    pub door_bay: usize,
    /// Varies the windows, framing, and chimney.
    pub seed: u32,
}

/// One wall of the outline: its outward normal and the direction a wall
/// piece runs along it, in the house's frame (`u` right along the front,
/// `w` out of the front), its start corner, its length, and the yaw that
/// turns a piece's +x along it and its +z outward.
#[derive(Clone, Copy, Debug)]
struct Side {
    normal: [f32; 2],
    along: [f32; 2],
    start: [f32; 2],
    length: f32,
    yaw: f32,
}

impl KitHouse {
    /// The house's four sides: front, right, back, and left.
    fn sides(&self) -> [Side; 4] {
        let (hw, hd) = (self.width / 2.0, self.depth / 2.0);
        [
            Side {
                normal: [0.0, 1.0],
                along: [1.0, 0.0],
                start: [-hw, hd],
                length: self.width,
                yaw: 0.0,
            },
            Side {
                normal: [1.0, 0.0],
                along: [0.0, -1.0],
                start: [hw, hd],
                length: self.depth,
                yaw: FRAC_PI_2,
            },
            Side {
                normal: [0.0, -1.0],
                along: [-1.0, 0.0],
                start: [hw, -hd],
                length: self.width,
                yaw: PI,
            },
            Side {
                normal: [-1.0, 0.0],
                along: [0.0, 1.0],
                start: [-hw, -hd],
                length: self.depth,
                yaw: -FRAC_PI_2,
            },
        ]
    }

    /// A point of the house's frame on the ground, x and z.
    #[must_use]
    pub fn world(&self, [u, w]: [f32; 2]) -> [f32; 2] {
        let (s, c) = self.facing.sin_cos();
        [
            self.center[0] + u * c + w * s,
            self.center[1] - u * s + w * c,
        ]
    }

    /// The ground floor's height, m.
    #[must_use]
    pub fn floor(&self) -> f32 {
        height(self.center[0], self.center[1]) + FLOOR_RISE
    }

    /// The roof's base: the top of the highest story's walls, m.
    #[must_use]
    pub fn eaves(&self) -> f32 {
        self.floor() + STORY * f32::from(self.stories.max(1) - 1) + WALL
    }

    /// The bays between a side's corners: each one's offset from the side's
    /// start and its length, 4 m bays with one 2 m bay where the length
    /// needs it, at the end `flip` picks.
    fn bays(length: f32, flip: bool) -> Vec<(f32, f32)> {
        let inner = (length - 2.0).max(0.0);
        let fours = (inner / 4.0).floor() as usize;
        let two = inner - 4.0 * fours as f32 >= 1.0;
        let mut lengths = vec![4.0; fours];
        if two {
            if flip {
                lengths.insert(0, 2.0);
            } else {
                lengths.push(2.0);
            }
        }
        let mut at = 1.0;
        lengths
            .into_iter()
            .map(|l| {
                at += l;
                (at - l, l)
            })
            .collect()
    }

    /// The front's bays, left to right.
    fn front_bays(&self) -> Vec<(f32, f32)> {
        Self::bays(self.width, noise(self.seed, 3) < 0.5)
    }

    /// The door's bay on the front: its offset and length.
    fn door(&self) -> (f32, f32) {
        let bays = self.front_bays();
        bays[self.door_bay.min(bays.len() - 1)]
    }

    /// The door opening's middle along the front, in the house's `u`.
    fn door_u(&self) -> f32 {
        let (offset, length) = self.door();
        let (a, b) = if length >= 4.0 { DOOR_4 } else { DOOR_2 };
        -self.width / 2.0 + offset + (a + b) / 2.0
    }

    /// The door's opening on the front, from and to, in the house's `u`.
    fn door_opening(&self) -> (f32, f32) {
        let (offset, length) = self.door();
        let (a, b) = if length >= 4.0 { DOOR_4 } else { DOOR_2 };
        let left = -self.width / 2.0 + offset;
        (left + a, left + b)
    }

    /// The middle of the doorway on the front wall's line, and the front's
    /// outward normal, x and z.
    #[must_use]
    pub fn front(&self) -> ([f32; 2], [f32; 2]) {
        let (s, c) = self.facing.sin_cos();
        (self.world([self.door_u(), self.depth / 2.0]), [s, c])
    }

    /// The front door: a point outside past the steps and one inside.
    #[must_use]
    pub fn door_points(&self) -> ([f32; 2], [f32; 2]) {
        let u = self.door_u();
        let hd = self.depth / 2.0;
        (self.world([u, hd + 2.5]), self.world([u, hd - 2.5]))
    }

    /// Every piece of the house.
    pub fn raise(&self, out: &mut Vec<Placement>) {
        let floor = self.floor();
        let mut put = |model: &'static str, [u, w]: [f32; 2], y: f32, yaw: f32| {
            let at = self.world([u, w]);
            out.push(
                Placement::new(model, at, self.facing + yaw, Collision::None)
                    .lift(y - height(at[0], at[1])),
            );
        };
        let point = |side: &Side, along: f32, out: f32| {
            [
                side.start[0] + side.along[0] * along + side.normal[0] * out,
                side.start[1] + side.along[1] * along + side.normal[1] * out,
            ]
        };
        let top_story = self.stories.max(1) - 1;
        for (k, side) in self.sides().iter().enumerate() {
            let bays = if k == 0 {
                self.front_bays()
            } else {
                Self::bays(
                    side.length,
                    noise(self.seed.wrapping_add(k as u32), 5) < 0.5,
                )
            };
            // The plinth: a corner block at the side's start, then a block
            // under each bay, turned to face out.
            put(
                "kit/plinth-corner",
                point(side, -PLINTH_OUT, PLINTH_OUT),
                floor - PLINTH,
                side.yaw,
            );
            for &(offset, length) in &bays {
                let model = if length >= 4.0 {
                    "kit/plinth-4"
                } else {
                    "kit/plinth-2"
                };
                put(
                    model,
                    point(side, offset + length, PLINTH_OUT),
                    floor - PLINTH,
                    side.yaw + PI,
                );
            }
            for story in 0..=top_story {
                let y = floor + STORY * f32::from(story);
                put("kit/corner", point(side, 0.0, 0.0), y, side.yaw);
                for (b, &(offset, length)) in bays.iter().enumerate() {
                    let model = self.wall(k, story, b, length);
                    put(model, point(side, offset, -THICK), y, side.yaw);
                }
                if story < top_story {
                    // The band over the walls, round the corners too.
                    let band = y + WALL;
                    put("kit/band-1", point(side, 0.0, -THICK), band, side.yaw);
                    put(
                        "kit/band-1",
                        point(side, side.length - 1.0, -THICK),
                        band,
                        side.yaw,
                    );
                    for &(offset, length) in &bays {
                        let model = if length >= 4.0 {
                            "kit/band-4"
                        } else {
                            "kit/band-2"
                        };
                        put(model, point(side, offset, -THICK), band, side.yaw);
                    }
                }
            }
        }
        // Floors: one slab a story, 0.5 m thick under its floor line, over
        // the whole outline in 4 m squares and a 4 m by 2 m row.
        let (hw, hd) = (self.width / 2.0, self.depth / 2.0);
        for story in 0..=top_story {
            let y = floor + STORY * f32::from(story) - 0.5;
            let mut u = -hw;
            while u < hw - 0.5 {
                let mut w = -hd;
                while w < hd - 0.5 {
                    let model = if hd - w >= 4.0 {
                        "kit/floor-4"
                    } else {
                        "kit/floor-4x2"
                    };
                    put(model, [u, w], y, 0.0);
                    w += if hd - w >= 4.0 { 4.0 } else { 2.0 };
                }
                u += 4.0;
            }
        }
        self.roof(&mut put);
        // The steps up to the door, running out from the plinth, their top
        // at the floor.
        let u = self.door_u();
        put(
            "kit/stairs",
            [u + 1.0, hd + PLINTH_OUT],
            floor - PLINTH,
            -FRAC_PI_2,
        );
    }

    /// The wall piece of side `k`'s bay `b` on `story`.
    fn wall(&self, k: usize, story: u8, b: usize, length: f32) -> &'static str {
        let timber = self.style == KitStyle::Timber;
        let r = noise(
            self.seed.wrapping_mul(31) ^ (k as u32 * 7 + u32::from(story) * 3),
            b as u32,
        );
        if length < 4.0 {
            if k == 0 && story == 0 && b == self.door_bay {
                return "kit/door-2";
            }
            return if timber {
                "kit/wall-2-timber"
            } else {
                "kit/wall-2"
            };
        }
        if k == 0 && story == 0 && b == self.door_bay {
            return "kit/door-4";
        }
        // Fronts are mostly windows, sides half, backs a few.
        let glazed = match k {
            0 => 0.85,
            2 => 0.3,
            _ => 0.55,
        };
        if r < glazed {
            const WINDOWS: [&str; 4] = [
                "kit/window-4",
                "kit/window-4-tall",
                "kit/window-4-wide",
                "kit/window-4-small",
            ];
            // One window design a house, with the odd one out.
            let pick = if r < glazed * 0.8 {
                (self.seed % 4) as usize
            } else {
                (self.seed / 4 % 4) as usize
            };
            WINDOWS[pick]
        } else if timber {
            "kit/wall-4-timber"
        } else {
            "kit/wall-4"
        }
    }

    /// The roof: two slopes from a ridge along `u`, gables at both ends,
    /// ridge caps, and sometimes a chimney.
    fn roof(&self, put: &mut impl FnMut(&'static str, [f32; 2], f32, f32)) {
        // The kit's slopes run 6 m from the ridge, so they span a 10 m side
        // with 1 m eaves: the depth, from a ridge along the front, or the
        // width, from a ridge front to back with the gables on the street.
        let along_front = (self.depth - 10.0).abs() <= (self.width - 10.0).abs();
        let (len, turn) = if along_front {
            (self.width, 0.0)
        } else {
            (self.depth, FRAC_PI_2)
        };
        // The roof's own frame: x along the ridge, z toward the eaves of
        // the slope that turns no further.
        let mut on_ridge = |model: &'static str, x: f32, y: f32, yaw: f32| {
            let at = if along_front { [x, 0.0] } else { [0.0, -x] };
            put(model, at, y, turn + yaw);
        };
        let half = len / 2.0;
        let eaves = self.eaves();
        let remainder = (len - 8.0) % 4.0 >= 1.0;
        // One slope: pieces run along -x from their origin, the gable of
        // `roof-end` at its origin.
        on_ridge("kit/roof-end-mirror", -half, eaves, 0.0);
        on_ridge("kit/roof-end", half, eaves, 0.0);
        let mut x = -half + 8.0;
        while x <= half - 4.0 + 0.01 {
            on_ridge("kit/roof-mid", x, eaves, 0.0);
            x += 4.0;
        }
        if remainder {
            on_ridge("kit/roof-mid-2", half - 4.0, eaves, 0.0);
        }
        // The other slope, turned: pieces run along +x from their origin.
        on_ridge("kit/roof-end", -half, eaves, PI);
        on_ridge("kit/roof-end-mirror", half, eaves, PI);
        let mut x = -half + 4.0;
        while x <= half - 8.0 + 0.01 {
            on_ridge("kit/roof-mid", x, eaves, PI);
            x += 4.0;
        }
        if remainder {
            on_ridge("kit/roof-mid-2", half - 6.0, eaves, PI);
        }
        // Ridge caps from 1 m past one gable to 1 m past the other.
        let cap = eaves + RIDGE_CAP;
        on_ridge("kit/ridge-end-mirror", -half - 1.0, cap, 0.0);
        on_ridge("kit/ridge-end", half + 1.0, cap, 0.0);
        let mut x = -half + 3.0;
        while x < half - 3.0 - 0.01 {
            if half - 3.0 - x >= 4.0 {
                on_ridge("kit/ridge-4", x + 4.0, cap, 0.0);
                x += 4.0;
            } else {
                on_ridge("kit/ridge-2", x + 2.0, cap, 0.0);
                x += 2.0;
            }
        }
        if noise(self.seed, 7) < 0.6 {
            let side = if noise(self.seed, 8) < 0.5 { -1.0 } else { 1.0 };
            // Through one slope, 2 m down it from the ridge.
            let x = side * (half - 1.5);
            let at = if along_front { [x, -2.0] } else { [2.0, -x] };
            put("kit/chimney", at, eaves + 1.0, 0.0);
        }
    }

    /// The axis-aligned box around a box of the house's frame.
    fn bound(&self, lo: [f32; 2], hi: [f32; 2]) -> Footprint {
        let corners = [
            self.world([lo[0], lo[1]]),
            self.world([hi[0], lo[1]]),
            self.world([hi[0], hi[1]]),
            self.world([lo[0], hi[1]]),
        ];
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for c in corners {
            min = [min[0].min(c[0]), min[1].min(c[1])];
            max = [max[0].max(c[0]), max[1].max(c[1])];
        }
        Footprint { min, max }
    }

    /// Whether the house stands square to the world's axes.
    fn square(&self) -> bool {
        let q = self.facing / FRAC_PI_2;
        (q - q.round()).abs() < 1e-4
    }

    /// The walls and the plinth as blockers, each a footprint and its top,
    /// m: one per run between corners, open at the door, or, for a turned
    /// house, one per meter of each run.
    #[must_use]
    pub fn walls(&self) -> Vec<(Footprint, f32)> {
        let top = self.eaves();
        let (door_from, door_to) = self.door_opening();
        let mut out = Vec::new();
        for (k, side) in self.sides().iter().enumerate() {
            // Runs along the side, from its start, around the door.
            let mut runs = vec![(0.0, side.length)];
            if k == 0 {
                let hw = self.width / 2.0;
                runs = vec![(0.0, door_from + hw), (door_to + hw, side.length)];
            }
            let step = if self.square() { f32::INFINITY } else { 1.0 };
            for (a, b) in runs {
                let mut from = a;
                while from < b - 1e-3 {
                    let to = (from + step).min(b);
                    // From the plinth's face to the walls' inner face.
                    let p = |along: f32, out: f32| {
                        [
                            side.start[0] + side.along[0] * along + side.normal[0] * out,
                            side.start[1] + side.along[1] * along + side.normal[1] * out,
                        ]
                    };
                    let (c0, c1) = (p(from, PLINTH_OUT), p(to, -THICK));
                    let lo = [c0[0].min(c1[0]), c0[1].min(c1[1])];
                    let hi = [c0[0].max(c1[0]), c0[1].max(c1[1])];
                    out.push((self.bound(lo, hi), top));
                    from = to;
                }
            }
        }
        out
    }

    /// What a walker stands on, as the proxies give it: the floor inside the
    /// walls and the door's threshold at the floor's height, and the steps
    /// down from it, each a footprint and its top, m.
    #[must_use]
    pub fn surfaces(&self) -> Vec<(Footprint, f32)> {
        let floor = self.floor();
        let (hw, hd) = (self.width / 2.0, self.depth / 2.0);
        let mut out = Vec::new();
        // The floor in 1 m squares, so a turned house's boxes stay inside.
        let inner = (hw - THICK, hd - THICK);
        let mut u = -inner.0;
        while u < inner.0 - 1e-3 {
            let mut w = -inner.1;
            while w < inner.1 - 1e-3 {
                let hi = [(u + 1.0).min(inner.0), (w + 1.0).min(inner.1)];
                out.push((self.bound([u, w], hi), floor));
                w += 1.0;
            }
            u += 1.0;
        }
        // The threshold through the doorway and over the plinth.
        let (from, to) = self.door_opening();
        out.push((self.bound([from, hd - THICK], [to, hd + PLINTH_OUT]), floor));
        // The steps, each a quarter meter down from the one inside it.
        let u = self.door_u();
        for i in 0..4 {
            let near = hd + PLINTH_OUT + STEP_RUN * i as f32;
            out.push((
                self.bound([u - 1.0, near], [u + 1.0, near + STEP_RUN]),
                floor - STEP_RISE * (i + 1) as f32,
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::everglade_pack::kit;

    fn house(facing: f32) -> KitHouse {
        KitHouse {
            name: "test house",
            center: [-60.0, 20.0],
            width: 8.0,
            depth: 10.0,
            facing,
            stories: 2,
            style: KitStyle::Timber,
            door_bay: 0,
            seed: 7,
        }
    }

    #[test]
    fn an_eight_meter_front_is_a_four_and_a_two_meter_bay_between_corners() {
        assert_eq!(KitHouse::bays(8.0, false), [(1.0, 4.0), (5.0, 2.0)]);
        assert_eq!(KitHouse::bays(8.0, true), [(1.0, 2.0), (3.0, 4.0)]);
        assert_eq!(KitHouse::bays(10.0, false), [(1.0, 4.0), (5.0, 4.0)]);
        assert_eq!(KitHouse::bays(16.0, false).len(), 4);
    }

    #[test]
    fn every_piece_is_a_kit_piece_on_the_lot() {
        for facing in [0.0, 0.15, PI / 2.0, -2.9] {
            let h = house(facing);
            let mut out = Vec::new();
            h.raise(&mut out);
            assert!(out.len() > 60, "{}", out.len());
            for p in &out {
                assert!(kit::piece_of(p.model).is_some(), "{}", p.model);
                assert_eq!(p.collision, Collision::None);
                let d = (p.at[0] - h.center[0]).hypot(p.at[1] - h.center[1]);
                assert!(d < 9.0, "{} at {d}", p.model);
            }
            // Doors, walls, and the roof are all there.
            let count = |m: &str| out.iter().filter(|p| p.model == m).count();
            assert_eq!(count("kit/door-4") + count("kit/door-2"), 1);
            assert_eq!(count("kit/corner"), 8);
            assert_eq!(count("kit/roof-end") + count("kit/roof-end-mirror"), 4);
            assert_eq!(count("kit/stairs"), 1);
        }
    }

    #[test]
    fn the_walls_are_open_at_the_door_and_the_floor_is_up_the_steps() {
        for facing in [0.0, 0.15] {
            let h = house(facing);
            let walls = h.walls();
            let (outside, inside) = h.door_points();
            let blocked = |p: [f32; 2]| {
                walls.iter().any(|(f, _)| {
                    p[0] > f.min[0] && p[0] < f.max[0] && p[1] > f.min[1] && p[1] < f.max[1]
                })
            };
            // Walking from outside to inside through the door meets no wall.
            for i in 0..=20 {
                let t = i as f32 / 20.0;
                let p = [
                    outside[0] + (inside[0] - outside[0]) * t,
                    outside[1] + (inside[1] - outside[1]) * t,
                ];
                assert!(!blocked(p), "blocked at {t} facing {facing}");
            }
            // But the middle of the back wall is blocked.
            assert!(blocked(h.world([0.0, -h.depth / 2.0 - 0.1])));
            // Each surface along the walk is at most a step above the next.
            let surfaces = h.surfaces();
            let top_at = |p: [f32; 2]| {
                surfaces
                    .iter()
                    .filter(|(f, _)| {
                        p[0] >= f.min[0] && p[0] <= f.max[0] && p[1] >= f.min[1] && p[1] <= f.max[1]
                    })
                    .map(|(_, t)| *t)
                    .fold(height(p[0], p[1]), f32::max)
            };
            let mut last = top_at(outside);
            for i in 1..=40 {
                let t = i as f32 / 40.0;
                let p = [
                    outside[0] + (inside[0] - outside[0]) * t,
                    outside[1] + (inside[1] - outside[1]) * t,
                ];
                let top = top_at(p);
                assert!(top - last <= 0.35 + 1e-3, "a {} m step at {t}", top - last);
                last = top;
            }
            assert!((last - h.floor()).abs() < 1e-3);
        }
    }
}
