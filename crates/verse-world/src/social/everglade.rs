//! Everglade's renderer-free ground and studio places: the heightfield, the
//! walkable square, the workshop's footprint, and the Agent Studio's
//! stations and desks. Verse's Everglade zone and a hosted social instance
//! of it read the same values here (`docs/verse/everglade.md`).

use glam::Vec3;
use std::f32::consts::FRAC_PI_2;

/// Half the walkable square, m. The glade is about 240 m across, a small
/// town around the workshop; the square runs past the tree ring so its
/// rising ground closes the view. Navigation's 2 m grid starts at the
/// square's edge, so an odd half extent puts grid lines through the hall's
/// doorways at x = -1 and 1.
pub const HALF_EXTENT: f32 = 135.0;
/// Radius of the flat clearing the town stands in, m.
pub const CLEARING_RADIUS: f32 = 68.0;
/// Radius of the tree ring, where the ground finishes its rise, m.
pub const RING_RADIUS: f32 = 92.0;
/// Half the square the studio's seats route in, m: every station, with
/// room around them. Seats never walk into the rest of the town, so their
/// routes search this smaller grid.
pub const SEAT_EXTENT: f32 = 40.0;
/// Height of the ground at the tree ring above the clearing, m.
pub const RING_RISE: f32 = 5.0;
/// Highest ground anywhere in the zone, m.
pub const MAX_HEIGHT: f32 = 10.0;
/// Amplitude of the low undulation on the rising ground, m.
pub const UNDULATION: f32 = 0.6;
/// Rise per meter beyond the tree ring.
pub const OUTER_SLOPE: f32 = 0.1;

/// Where the player arrives: on the approach path, facing the workshop.
pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -20.0);
/// The heading the player arrives with, as the controller's yaw.
pub const SPAWN_YAW: f32 = 0.0;
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
/// Each workbench's standing point, x and z, m, facing +z (yaw zero): one
/// per seat, in a row across the hall behind the desks station.
pub const DESK_SEATS: [[f32; 2]; 4] = [[-3.3, 5.8], [-1.1, 5.8], [1.1, 5.8], [3.3, 5.8]];

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
}

/// The stations of the layout table in `docs/verse/everglade.md`, in the
/// table's order. Each station's furniture stands ahead of its point, in
/// the direction it faces; layout changes never move these points.
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
