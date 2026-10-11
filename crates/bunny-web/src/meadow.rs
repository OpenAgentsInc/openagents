//! Warren Meadow, the hub (`docs/verse/games/grow-little-bunny.md`, The
//! meadow hub): the bunny hops around free; the Warren Mound sits in the
//! middle with the Burrow's door, the Color Pond to the north with its ring
//! of 21 ladder stones, the rabbit holes to the east, the Carrot Board to
//! the south and the exit arch to the west. Walking into a hole, the door,
//! the board or the arch is how the player picks.
//!
//! Metres, `x` east, `z` south; yaw 0 faces `+z` (south).

/// The meadow's half-width: the bunny stays inside.
pub const EDGE: f32 = 42.0;
/// The mound's footprint radius, and the pond's.
pub const MOUND: f32 = 6.6;
pub const POND_AT: (f32, f32) = (0.0, -24.0);
pub const POND: (f32, f32) = (8.0, 6.0);
/// The ring of ladder stones around the pond.
pub const STONE_RING: f32 = 10.5;
pub const STONES: usize = 21;
/// Where the bunny starts, south of the mound, facing north.
pub const START: (f32, f32) = (0.0, 11.0);
const SPEED: f32 = 6.0;
const TURN: f32 = 2.6;
const BODY: f32 = 0.45;

/// The Burrow's doors: four, around the mound, the first facing the start
/// (south) and the rest a quarter turn apart. `kit::piece(Piece::Mound)`
/// draws them here, and walking into any of them goes inside.
pub const DOORS: usize = 4;
pub const DOOR_TURN: f32 = 0.0;

/// Where door `i` is: on the mound's foot, just outside what is solid.
#[must_use]
pub fn door(i: usize) -> (f32, f32) {
    let a = door_angle(i);
    let r = MOUND + BODY + 0.15;
    (a.sin() * r, a.cos() * r)
}

/// The angle (about `y`, 0 facing `+z`) door `i` faces.
#[must_use]
pub fn door_angle(i: usize) -> f32 {
    DOOR_TURN + i as f32 * std::f32::consts::FRAC_PI_2
}

/// Somewhere the player can go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// Rabbit hole into garden N (1-based).
    Hole(usize),
    /// The Burrow's door: the record and settings.
    Burrow,
    /// The Carrot Board: best times and scores.
    Board,
    /// The exit arch.
    Arch,
}

/// Where each rabbit hole is: east of the mound, in an arc.
#[must_use]
pub fn hole(n: usize) -> (f32, f32) {
    let i = n as f32 - 3.0;
    (24.0 + (i * 0.5).abs() * -2.0, i * 7.0)
}

/// Each spot's place and how close counts as walking in.
#[must_use]
pub fn spots(gardens: usize) -> Vec<(Spot, (f32, f32), f32)> {
    let mut spots: Vec<(Spot, (f32, f32), f32)> = (1..=gardens)
        .map(|n| (Spot::Hole(n), hole(n), 1.2))
        .collect();
    spots.extend((0..DOORS).map(|i| (Spot::Burrow, door(i), 1.3)));
    spots.push((Spot::Board, (0.0, 26.0), 1.8));
    spots.push((Spot::Arch, (-30.0, 0.0), 2.0));
    spots
}

/// Where a ladder stone is.
#[must_use]
pub fn stone(index: usize) -> (f32, f32) {
    let a = index as f32 / STONES as f32 * std::f32::consts::TAU;
    (
        POND_AT.0 + a.sin() * STONE_RING,
        POND_AT.1 - a.cos() * STONE_RING * 0.8,
    )
}

/// Trees and logs that stand in the way.
pub const TREES: [(f32, f32); 10] = [
    (-14.0, -12.0),
    (-20.0, 6.0),
    (-12.0, 20.0),
    (14.0, -30.0),
    (-26.0, -26.0),
    (34.0, 26.0),
    (12.0, 32.0),
    (-34.0, 18.0),
    (36.0, -16.0),
    (-8.0, -36.0),
];

/// The bunny's hops around the meadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    /// Whether it moved in the last step, for the hop.
    pub moving: bool,
}

impl Default for Walker {
    fn default() -> Self {
        Self {
            x: START.0,
            z: START.1,
            yaw: std::f32::consts::PI,
            moving: false,
        }
    }
}

fn blocked(x: f32, z: f32) -> bool {
    let mound = x * x + z * z < (MOUND + BODY) * (MOUND + BODY);
    let (px, pz) = (x - POND_AT.0, z - POND_AT.1);
    let pond = (px / (POND.0 + BODY)).powi(2) + (pz / (POND.1 + BODY)).powi(2) < 1.0;
    let tree = TREES
        .iter()
        .any(|(tx, tz)| (x - tx).powi(2) + (z - tz).powi(2) < (1.4 + BODY).powi(2));
    mound || pond || tree || x.abs() > EDGE || z.abs() > EDGE
}

impl Walker {
    /// Moves for `dt` seconds: `forward` from -1 to 1, `turn` from -1 (left)
    /// to 1 (right). It slides along what it bumps.
    pub fn step(&mut self, dt: f32, forward: f32, turn: f32) {
        self.yaw -= turn.clamp(-1.0, 1.0) * TURN * dt;
        let step = forward.clamp(-1.0, 1.0) * SPEED * dt;
        let (dx, dz) = (self.yaw.sin() * step, self.yaw.cos() * step);
        let (x0, z0) = (self.x, self.z);
        if !blocked(self.x + dx, self.z + dz) {
            self.x += dx;
            self.z += dz;
        } else if !blocked(self.x + dx, self.z) {
            self.x += dx;
        } else if !blocked(self.x, self.z + dz) {
            self.z += dz;
        }
        self.moving = (self.x - x0).abs() + (self.z - z0).abs() > 1e-4;
    }

    /// The spot it is standing in, if any.
    #[must_use]
    pub fn at(&self, gardens: usize) -> Option<Spot> {
        spots(gardens).into_iter().find_map(|(spot, (x, z), r)| {
            ((self.x - x).powi(2) + (self.z - z).powi(2) < r * r).then_some(spot)
        })
    }

    /// The nearest spot within `reach`, for a prompt.
    #[must_use]
    pub fn near(&self, gardens: usize, reach: f32) -> Option<Spot> {
        spots(gardens)
            .into_iter()
            .map(|(spot, (x, z), _)| (spot, (self.x - x).powi(2) + (self.z - z).powi(2)))
            .filter(|(_, d)| *d < reach * reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(spot, _)| spot)
    }

    /// Faces back toward the mound after coming out of a hole; out of the
    /// Burrow, stands outside the nearest door facing away from it.
    pub fn out_of(&mut self, spot: Spot, gardens: usize) {
        if spot == Spot::Burrow {
            let i = (0..DOORS)
                .min_by(|a, b| {
                    let d = |i: usize| (door(i).0 - self.x).powi(2) + (door(i).1 - self.z).powi(2);
                    d(*a).total_cmp(&d(*b))
                })
                .unwrap_or(0);
            let a = door_angle(i);
            let r = MOUND + BODY + 3.0;
            self.x = a.sin() * r;
            self.z = a.cos() * r;
            self.yaw = a;
            return;
        }
        if let Some((_, (x, z), r)) = spots(gardens).into_iter().find(|(s, _, _)| *s == spot) {
            let (dx, dz) = (-x, START.1 - z);
            let len = (dx * dx + dz * dz).sqrt().max(1e-3);
            self.x = x + dx / len * (r + 4.0);
            self.z = z + dz / len * (r + 4.0);
            self.yaw = dx.atan2(dz);
        }
    }
}

/// The meadow's free hop on Space or the hop button: the same curve as a
/// garden jump ([`bunny_rules::hop`]), in milliseconds. A tap hops a
/// little; holding keeps rising, up to about four times as high. It only
/// lifts the bunny: walking and what is solid stay as they are, so a hop
/// can't leave the meadow or go through the mound.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hopper {
    /// Milliseconds since take-off, and of rising so far.
    pub elapsed: f32,
    pub rise: f32,
    pub rising: bool,
    pub holding: bool,
    /// In the air.
    pub up: bool,
}

/// A tap hop's height at the top, in metres.
pub const HOP_PEAK: f32 = 0.7;

fn ms(ticks: u32) -> f32 {
    ticks as f32 * 1000.0 / bunny_rules::HZ as f32
}

impl Hopper {
    /// The hop key went down: take off, unless already in the air.
    pub fn press(&mut self) {
        if !self.up {
            *self = Self {
                elapsed: 0.0,
                rise: 0.0,
                rising: true,
                holding: true,
                up: true,
            };
        }
    }

    /// The hop key came up: stop rising (after a tap's rise at least).
    pub fn release(&mut self) {
        self.holding = false;
    }

    /// Moves on `dt` seconds; lands at the end.
    pub fn step(&mut self, dt: f32) {
        if !self.up {
            return;
        }
        self.elapsed += dt * 1000.0;
        let (tap, max) = (
            ms(bunny_rules::hop::RISE_TAP),
            ms(bunny_rules::hop::RISE_MAX),
        );
        if self.rising {
            self.rise = self.elapsed.min(max);
            if self.rise >= max || (!self.holding && self.rise >= tap) {
                self.rising = false;
            }
        }
        let span = self.rise + ms(bunny_rules::hop::HANG) + self.rise * 0.75;
        if !self.rising && self.elapsed >= span {
            *self = Self::default();
        }
    }

    /// The height now, in metres.
    #[must_use]
    pub fn height(&self) -> f32 {
        if !self.up {
            return 0.0;
        }
        let to_ticks = bunny_rules::HZ as f32 / 1000.0;
        bunny_rules::hop::lift(self.elapsed * to_ticks, self.rise * to_ticks, self.rising)
            * HOP_PEAK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk_to(w: &mut Walker, target: (f32, f32), gardens: usize) -> Option<Spot> {
        for _ in 0..2_000 {
            let want = (target.0 - w.x).atan2(target.1 - w.z);
            let pi = std::f32::consts::PI;
            let diff = (want - w.yaw + pi).rem_euclid(2.0 * pi) - pi;
            w.step(1.0 / 60.0, 1.0, (-diff * 3.0).clamp(-1.0, 1.0));
            if let Some(spot) = w.at(gardens) {
                return Some(spot);
            }
        }
        None
    }

    #[test]
    fn walking_into_a_hole_picks_its_garden_and_coming_out_leaves_it() {
        let mut w = Walker::default();
        assert_eq!(w.at(5), None, "the start is clear of every spot");
        assert_eq!(walk_to(&mut w, hole(3), 5), Some(Spot::Hole(3)));
        w.out_of(Spot::Hole(3), 5);
        assert_eq!(w.at(5), None);
    }

    #[test]
    fn the_mound_the_pond_and_the_edge_are_solid() {
        let mut w = Walker::default();
        for _ in 0..600 {
            w.step(1.0 / 60.0, 1.0, 0.0);
        }
        assert!(w.x * w.x + w.z * w.z >= MOUND * MOUND);
        let mut w = Walker {
            x: 40.0,
            z: 0.0,
            yaw: std::f32::consts::FRAC_PI_2,
            moving: false,
        };
        for _ in 0..600 {
            w.step(1.0 / 60.0, 1.0, 0.0);
        }
        assert!(w.x <= EDGE);
    }

    #[test]
    fn spots_and_stones_do_not_overlap_anything_solid() {
        for (spot, (x, z), _) in spots(5) {
            assert!(!blocked(x, z), "{spot:?}");
        }
        for i in 0..STONES {
            let (x, z) = stone(i);
            let (px, pz) = (x - POND_AT.0, z - POND_AT.1);
            assert!(
                (px / POND.0).powi(2) + (pz / POND.1).powi(2) > 1.0,
                "stone {i}"
            );
        }
        assert!(!blocked(START.0, START.1));
    }

    /// Walks toward `(x, z)` until it stops in a spot or gets nowhere.
    fn walk_toward(w: &mut Walker, to: (f32, f32)) -> Option<Spot> {
        for _ in 0..2000 {
            w.yaw = (to.0 - w.x).atan2(to.1 - w.z);
            w.step(1.0 / 60.0, 1.0, 0.0);
            if let Some(spot) = w.at(5) {
                return Some(spot);
            }
        }
        None
    }

    #[test]
    fn walking_into_any_door_of_the_mound_enters_the_burrow() {
        for i in 0..DOORS {
            let a = door_angle(i);
            // From a few metres out in front of the door, head for the
            // mound's middle, as a player aiming at the door does.
            let mut w = Walker {
                x: a.sin() * (MOUND + 6.0),
                z: a.cos() * (MOUND + 6.0),
                yaw: 0.0,
                moving: false,
            };
            assert_eq!(
                walk_toward(&mut w, (0.0, 0.0)),
                Some(Spot::Burrow),
                "door {i}"
            );
            w.out_of(Spot::Burrow, 5);
            assert_eq!(w.at(5), None, "out of door {i}, clear of it");
            assert!(!blocked(w.x, w.z));
        }
        // Straight ahead from the start is a door.
        let mut w = Walker::default();
        let mut found = None;
        for _ in 0..600 {
            w.step(1.0 / 60.0, 1.0, 0.0);
            found = found.or(w.at(5));
        }
        assert_eq!(found, Some(Spot::Burrow));
    }

    #[test]
    fn the_doors_are_where_the_mound_draws_them() {
        // The mound's model puts a door every quarter turn from DOOR_TURN.
        for i in 0..DOORS {
            let (x, z) = door(i);
            let a = x.atan2(z);
            let want = door_angle(i);
            let diff = (a - want).rem_euclid(std::f32::consts::TAU);
            assert!(
                diff < 1e-4 || diff > std::f32::consts::TAU - 1e-4,
                "door {i}"
            );
        }
    }

    /// The hop's highest point, holding the key for `held` seconds.
    fn hop_peak(held: f32) -> (f32, f32) {
        let mut h = Hopper::default();
        h.press();
        let mut t = 0.0;
        let mut top: f32 = 0.0;
        while h.up {
            if t >= held {
                h.release();
            }
            h.step(1.0 / 120.0);
            t += 1.0 / 120.0;
            top = top.max(h.height());
            assert!(t < 10.0, "lands");
        }
        assert_eq!(h.height(), 0.0, "on the ground");
        (top, t)
    }

    #[test]
    fn a_tap_hops_a_little_and_holding_hops_higher_up_to_a_cap() {
        let (tap, tap_time) = hop_peak(0.0);
        let (mid, _) = hop_peak(0.6);
        let (max, max_time) = hop_peak(1.2);
        let (longer, _) = hop_peak(5.0);
        assert!((HOP_PEAK..HOP_PEAK * 1.1).contains(&tap), "{tap}");
        assert!(mid > tap * 2.0, "{mid}");
        assert!(max > mid, "{max}");
        assert!(max > HOP_PEAK * 4.0, "a big hop: {max}");
        assert!((longer - max).abs() < 0.05, "the cap: {longer} {max}");
        assert!((0.5..0.62).contains(&tap_time), "{tap_time}");
        assert!(max_time > tap_time * 3.0);
        // Pressing again in the air doesn't hop again.
        let mut h = Hopper::default();
        h.press();
        h.step(0.1);
        let before = h;
        h.press();
        assert_eq!(h, before);
    }
}
