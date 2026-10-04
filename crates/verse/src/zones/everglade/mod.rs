//! Everglade: the forest glade where the Agent Studio lives
//! (`docs/verse/everglade.md`).
//!
//! This module registers the zone with generated ground and greybox station
//! markers. The ground is a heightfield computed in Rust: flat inside the
//! clearing, rising toward the tree ring. The studio's stations have fixed
//! standing points in [`STATIONS`], so the textured layout (#10487) and the
//! studio workspace (#10465) build on the same coordinates. The player walks
//! the shared plaza controller over the heightfield; nothing is downloaded.

mod draw;
#[cfg(test)]
mod tests;

use crate::{
    controller::{Footprint, InputState, PlayerController},
    mesh::Mesh,
    world::World,
};
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;

/// Half the walkable square, m. The glade is about 120 m across; the square
/// runs past the tree ring so its rising ground closes the view.
pub const HALF_EXTENT: f32 = 75.0;
/// Radius of the flat clearing around the workshop, m.
pub const CLEARING_RADIUS: f32 = 34.0;
/// Radius of the tree ring, where the ground finishes its rise, m.
pub const RING_RADIUS: f32 = 58.0;
/// Height of the ground at the tree ring above the clearing, m.
pub const RING_RISE: f32 = 5.0;
/// Highest ground anywhere in the zone, m.
pub const MAX_HEIGHT: f32 = 10.0;
/// Amplitude of the low undulation on the rising ground, m.
const UNDULATION: f32 = 0.6;
/// Rise per meter beyond the tree ring.
const OUTER_SLOPE: f32 = 0.1;

/// The return portal, at the start of the approach path.
pub(crate) const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -32.0);
/// Where the player arrives: on the approach path, facing the workshop.
const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -25.0);
const SPAWN_YAW: f32 = 0.0;
/// Distance from a station's standing point within which the caption names
/// the station, m.
pub const STATION_RANGE: f32 = 3.0;

/// The workshop hall's floor: center x and z, and half extents, m. The
/// hall's door is in its south wall, facing the yard and the approach.
pub const HALL: ([f32; 2], [f32; 2]) = ([0.0, 6.0], [8.0, 5.0]);
/// The strongroom annex east of the hall: center and half extents, m.
pub const STRONGROOM: ([f32; 2], [f32; 2]) = ([11.0, 6.0], [3.0, 4.0]);
/// The yard in front of the hall: center and half extents, m.
pub const YARD: ([f32; 2], [f32; 2]) = ([0.0, -6.5], [13.0, 7.5]);
/// Half the width of the approach path, m. The path runs along x = 0 from
/// the return portal to the yard.
pub const PATH_HALF_WIDTH: f32 = 1.6;

/// One Agent Studio station: where a seat or the player stands to use it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// Stable identifier, also the map landmark ID.
    pub id: &'static str,
    /// The place in the glade, from the layout table.
    pub place: &'static str,
    /// The studio station it hosts.
    pub studio: &'static str,
    /// Marker lettering: A–Z, 0–9, and space.
    pub sign: &'static str,
    /// Standing point, x and z, m. The ground there is part of the flat
    /// clearing, so its height is zero.
    pub at: [f32; 2],
    /// Heading from the standing point toward the station's furniture, as
    /// the controller's yaw.
    pub facing: f32,
}

impl Station {
    /// The standing point on the ground.
    #[must_use]
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.at[0], height(self.at[0], self.at[1]), self.at[1])
    }

    /// Where the greybox marker post stands: ahead of the standing point,
    /// where the furniture will go.
    #[must_use]
    pub fn marker(&self) -> Vec3 {
        let ahead = crate::controller::forward(self.facing) * MARKER_OFFSET;
        let at = self.position() + ahead;
        Vec3::new(at.x, height(at.x, at.z), at.z)
    }
}

/// Distance from a standing point to its marker post, m.
const MARKER_OFFSET: f32 = 1.4;

/// The stations of the layout table in `docs/verse/everglade.md`, in the
/// table's order. Later layout work replaces the markers with furniture at
/// these points; it does not move them.
pub const STATIONS: [Station; 10] = [
    Station {
        id: "approach",
        place: "Approach path",
        studio: "Spawn and return",
        sign: "APPROACH",
        at: [-3.0, -27.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "task_wall",
        place: "Yard notice board",
        studio: "Task Wall",
        sign: "TASK WALL",
        at: [-7.0, -9.0],
        facing: -FRAC_PI_2,
    },
    Station {
        id: "desks",
        place: "Workshop hall",
        studio: "Desks",
        sign: "DESKS",
        at: [0.0, 5.0],
        facing: 0.0,
    },
    Station {
        id: "library",
        place: "Hall gallery",
        studio: "Library",
        sign: "LIBRARY",
        at: [-5.0, 9.0],
        facing: -FRAC_PI_2,
    },
    Station {
        id: "oracle",
        place: "Hearth corner",
        studio: "Oracle",
        sign: "ORACLE",
        at: [5.0, 9.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "proving",
        place: "Yard ring",
        studio: "Proving ground",
        sign: "PROVING GROUND",
        at: [8.0, -8.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "podium",
        place: "Lectern by the door",
        studio: "Podium",
        sign: "PODIUM",
        at: [-3.0, -2.5],
        facing: 0.0,
    },
    Station {
        id: "merge",
        place: "Strongroom",
        studio: "Merge station",
        sign: "MERGE",
        at: [10.5, 5.0],
        facing: FRAC_PI_2,
    },
    Station {
        id: "lounge",
        place: "Bench under the trees",
        studio: "Lounge",
        sign: "LOUNGE",
        at: [-24.0, -20.0],
        facing: -FRAC_PI_2,
    },
    Station {
        id: "workbench",
        place: "Wagon by the gate",
        studio: "Workbench",
        sign: "WORKBENCH",
        at: [9.0, -24.0],
        facing: FRAC_PI_2,
    },
];

/// Ground height at `(x, z)`, m: zero inside the clearing, rising smoothly to
/// [`RING_RISE`] at the tree ring with a low undulation, then climbing
/// gently to the edge of the zone. Always finite and within
/// `0..=MAX_HEIGHT`; a nonfinite coordinate reads as the clearing.
#[must_use]
pub fn height(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let r = x.hypot(z);
    let t = ((r - CLEARING_RADIUS) / (RING_RADIUS - CLEARING_RADIUS)).clamp(0.0, 1.0);
    let rise = t * t * (3.0 - 2.0 * t);
    // The undulation scales with the rise, so the clearing stays flat and
    // the ground never dips below it.
    let wave = (x * 0.13 + 0.4).sin() * (z * 0.11 - 0.7).cos();
    let outer = (r - RING_RADIUS).max(0.0) * OUTER_SLOPE;
    (rise * (RING_RISE + UNDULATION * wave) + outer).clamp(0.0, MAX_HEIGHT)
}

/// The station whose standing point is nearest `(x, z)` within
/// [`STATION_RANGE`].
#[must_use]
pub fn station_near(x: f32, z: f32) -> Option<&'static Station> {
    STATIONS
        .iter()
        .map(|s| (s, (s.at[0] - x).hypot(s.at[1] - z)))
        .filter(|(_, d)| *d <= STATION_RANGE)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
}

/// The zone's live state: the clock that bobs the station beacons.
pub(crate) struct Everglade {
    elapsed: f32,
    rendered: Mesh,
}

impl Everglade {
    pub fn new() -> Self {
        let mut zone = Self {
            elapsed: 0.0,
            rendered: Mesh::default(),
        };
        zone.rendered = draw::beacons(zone.elapsed);
        zone
    }

    pub fn spawn() -> Vec3 {
        Vec3::new(SPAWN.x, height(SPAWN.x, SPAWN.z), SPAWN.z)
    }

    pub fn spawn_yaw() -> f32 {
        SPAWN_YAW
    }

    /// The ground, the workshop outline, and the station markers.
    pub fn world() -> World {
        let mut world = World::default();
        draw::ground(&mut world.mesh);
        draw::outlines(&mut world.mesh);
        draw::markers(&mut world.mesh);
        place_layout(&mut world);
        world
    }

    /// Walk the shared plaza controller over the heightfield. The controller
    /// sees flat ground at the terrain under the character: feet are moved
    /// into height above ground before the step and back after it, so walking,
    /// jumping, and landing follow the slope.
    pub fn move_player(
        player: &mut PlayerController,
        input: &InputState,
        blockers: &[Footprint],
        dt: f32,
    ) {
        player.pos.y -= height(player.pos.x, player.pos.z);
        player.set_surface_height(0.0);
        player.update(input, dt, blockers, HALF_EXTENT);
        let ground = height(player.pos.x, player.pos.z);
        player.pos.y += ground;
        player.set_surface_height(ground);
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt) % 1000.0;
        self.rendered = draw::beacons(self.elapsed);
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The HUD caption for a player standing at `at`.
    pub fn caption(at: Vec3) -> String {
        match station_near(at.x, at.z) {
            Some(station) => format!("Everglade\n{} · {}", station.studio, station.place),
            None => "Everglade\nWalk to a station marker".into(),
        }
    }
}

/// Where the textured glade and workshop join the zone (#10487). The admitted
/// Everglade pack's placements (#10484), drawn as textured, alpha-tested
/// cells by the zone renderer (#10485), extend `world` here with their prop
/// blockers, keeping [`STATIONS`] fixed. Until then the vertex-colored ground
/// and greybox markers stand alone, and this adds nothing.
fn place_layout(_world: &mut World) {}
