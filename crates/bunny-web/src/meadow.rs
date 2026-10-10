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
    spots.push((Spot::Burrow, (0.0, MOUND + 0.3), 1.3));
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

    /// Faces back toward the mound after coming out of a hole.
    pub fn out_of(&mut self, spot: Spot, gardens: usize) {
        if let Some((_, (x, z), r)) = spots(gardens).into_iter().find(|(s, _, _)| *s == spot) {
            let (dx, dz) = (-x, START.1 - z);
            let len = (dx * dx + dz * dz).sqrt().max(1e-3);
            self.x = x + dx / len * (r + 4.0);
            self.z = z + dz / len * (r + 4.0);
            self.yaw = dx.atan2(dz);
        }
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
            if spot != Spot::Burrow {
                assert!(!blocked(x, z), "{spot:?}");
            }
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
}
