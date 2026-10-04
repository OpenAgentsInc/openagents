//! Everglade: the forest glade where the Agent Studio lives
//! (`docs/verse/everglade.md`).
//!
//! The ground is a heightfield computed in Rust: flat inside the clearing,
//! rising toward the tree ring. The glade and the workshop are placements
//! of the pinned Everglade pack's models (`layout`), drawn as textured,
//! alpha-tested cells on a lit stage, with the Task Wall and the desk
//! monitors drawn by Verse (`boards`). The pack loads on portal entry, as
//! the Ruins pack does. The studio's stations have fixed standing points in
//! [`STATIONS`], which the studio workspace (#10465) builds on. The player
//! walks the shared plaza controller over the heightfield.

mod boards;
mod draw;
pub mod layout;
mod scene;
#[cfg(test)]
mod tests;

use crate::{
    controller::{Footprint, InputState, PlayerController},
    mesh::Mesh,
    pbr::{Key, Neon},
    world::World,
};
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use super::everglade_pack::ZonePack;

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

/// The zone's live state: its clock and the lit stage its frames draw on.
pub(crate) struct Everglade {
    elapsed: f32,
    rendered: Mesh,
}

impl Everglade {
    pub fn new() -> Self {
        Self {
            elapsed: 0.0,
            rendered: Self::stage(0.0),
        }
    }

    /// The physical stage: the zone's green-gold air as background and fog,
    /// a warm afternoon sun from behind the approach that casts shadows
    /// over the clearing, and sky and ground fill. Textured meshes draw only
    /// on a lit stage.
    fn stage(time: f32) -> Mesh {
        let air = super::atmosphere(super::ZoneId::Everglade);
        Mesh {
            neon: Some(Neon {
                field: air.color,
                fog_start: air.fog_start,
                fog_end: air.fog_end,
                line_gain: 1.0,
                line_width: 1.4,
                bloom: 0.04,
                vignette: 0.15,
                time,
                key: Some(Key {
                    dir: Vec3::new(-0.35, 0.8, -0.45).normalize(),
                    illuminance: 4_000.0,
                    angular_radius: 0.03,
                    rim_dir: Vec3::new(0.5, 0.35, 0.6).normalize(),
                    rim_illuminance: 900.0,
                    rim_angular_radius: 0.1,
                    sky: 1_200.0,
                    ground: 450.0,
                    ev100: 10.0,
                    shadow_center: Vec3::new(0.0, 0.0, -4.0),
                    shadow_half: 40.0,
                }),
            }),
            ..Mesh::default()
        }
    }

    pub fn spawn() -> Vec3 {
        Vec3::new(SPAWN.x, height(SPAWN.x, SPAWN.z), SPAWN.z)
    }

    pub fn spawn_yaw() -> f32 {
        SPAWN_YAW
    }

    /// The ground, the textured glade and workshop from `pack` with their
    /// blockers, and the boards.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model or the scene
    /// exceeds the renderer's bounds.
    pub fn world(pack: &ZonePack) -> Result<World, String> {
        let mut world = World::default();
        draw::ground(&mut world.mesh);
        let (scene, blockers) = scene::build(pack, &layout::placements())?;
        world.mesh.textured = Some(Arc::new(scene));
        world.blockers = blockers;
        world.blockers.extend(layout::board_blockers());
        boards::draw(&mut world.mesh);
        Ok(world)
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
        self.rendered = Self::stage(self.elapsed);
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The HUD caption for a player standing at `at`.
    pub fn caption(at: Vec3) -> String {
        match station_near(at.x, at.z) {
            Some(station) => format!("Everglade\n{} · {}", station.studio, station.place),
            None => "Everglade\nWalk up to a station".into(),
        }
    }
}

/// The hall's interior, inset from its walls, and the camera's height
/// limit there, m: the third-person camera stays inside while the player
/// does, at about head height so it looks along the hall rather than up
/// into the roof slopes.
const INTERIOR: ([f32; 2], [f32; 2]) = ([-7.4, 1.5], [7.4, 10.5]);
const INTERIOR_TOP: f32 = 2.2;

/// Pulls the camera's `eye` toward the player's `focus` so it stays inside
/// the hall while the focus is inside, instead of looking through a wall or
/// the roof. Elsewhere `eye` is returned unchanged.
#[must_use]
pub fn keep_eye_inside(focus: Vec3, eye: Vec3) -> Vec3 {
    let (min, max) = INTERIOR;
    let within = |p: Vec3| (min[0]..=max[0]).contains(&p.x) && (min[1]..=max[1]).contains(&p.z);
    if !focus.is_finite() || !eye.is_finite() || !within(focus) {
        return eye;
    }
    let delta = eye - focus;
    let mut t = 1.0_f32;
    let limits = [
        (focus.x, delta.x, min[0], max[0]),
        (focus.z, delta.z, min[1], max[1]),
        (
            focus.y,
            delta.y,
            f32::NEG_INFINITY,
            INTERIOR_TOP.max(focus.y),
        ),
    ];
    for (start, step, low, high) in limits {
        if step > 0.0 && start + step > high {
            t = t.min((high - start) / step);
        } else if step < 0.0 && start + step < low {
            t = t.min((low - start) / step);
        }
    }
    if t >= 1.0 {
        return eye;
    }
    focus + delta * t.max(0.0)
}
