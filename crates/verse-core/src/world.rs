//! The static world a zone builds (geometry uploaded once and the footprints
//! the player cannot walk through), the plaza's layout, and where a world
//! stands its Gym. `verse::world` builds the plaza from these.

use glam::Vec3;
use verse_pbr::mesh::Mesh;
use verse_world::social::controller::Footprint;

/// The static world: its geometry and what the player cannot walk through.
#[derive(Clone, Debug, Default)]
pub struct World {
    /// Lines and faces uploaded once.
    pub mesh: Mesh,
    /// Building and pylon footprints.
    pub blockers: Vec<Footprint>,
}

/// The plaza world's NIP-MV identifier.
pub const PLAZA_WORLD: &str = "verse-plaza";
/// The world of the OpenAgents app's bare plaza. It is a separate coordinate
/// space from [`PLAZA_WORLD`]: the bare world has none of the plaza's
/// buildings or collision, so its positions are not valid plaza positions
/// (NIP-MV requires a distinct world identifier for each separately loaded
/// coordinate space).
pub const BARE_WORLD: &str = "verse-bare";

/// Half the width of the walkable square, in meters.
pub const HALF: f32 = 264.0;
/// Width of one city lot, in meters.
pub const LOT: f32 = 24.0;
/// Spacing of the fine ground grid, in meters.
pub const GRID: f32 = 4.0;
/// Where the player starts.
pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -10.0);
/// The shared computer stands five meters in front of the initial spawn.
pub const COMPUTER: Vec3 = Vec3::new(0.0, 0.0, -5.0);
/// Center of the physical monitor's display and interaction plane.
pub const COMPUTER_SCREEN: Vec3 = Vec3::new(0.0, 2.4, -5.16);
/// Display bounds in the monitor's local horizontal and vertical axes.
pub const COMPUTER_SCREEN_HALF: [f32; 2] = [1.2, 0.65];
/// Maximum ground-plane distance at which the computer can be opened.
pub const COMPUTER_RANGE: f32 = 3.0;
/// Center of the Gym's walkable hall, east of the plaza.
pub const GYM_CENTER: Vec3 = Vec3::new(48.0, 0.0, 0.0);
/// The west doorway faces the plaza and has six meters of clear width.
pub const GYM_ENTRANCE: Vec3 = Vec3::new(36.0, 0.0, 0.0);
/// Center of the main bulletin board, facing the hall's entrance.
pub const GYM_BOARD: Vec3 = Vec3::new(58.8, 2.8, 0.0);
/// The central board's readable front, slightly west of its backing surface.
pub const GYM_BOARD_SCREEN: Vec3 = Vec3::new(58.788, 2.8, 0.0);
/// Half width along Z and half height of the central board, in meters.
pub const GYM_BOARD_HALF: [f32; 2] = [2.5, 1.55];
/// Maximum ground-plane distance for opening the board from inside the hall.
pub const GYM_BOARD_RANGE: f32 = 6.0;
/// Center of the Grid's RESULTS board: the right-hand plot panel beside the
/// central board, which the Grid letters and opens for published results.
pub const GYM_RESULTS_BOARD: Vec3 = Vec3::new(58.8, 2.8, 5.7);
/// The RESULTS board's readable front, in line with the central board's.
pub const GYM_RESULTS_SCREEN: Vec3 = Vec3::new(58.788, 2.8, 5.7);
/// Half width along Z and half height of the RESULTS board, in meters.
pub const GYM_RESULTS_HALF: [f32; 2] = [2.3, 1.55];
/// Center of the Grid's EVALS board: the left-hand plot panel beside the
/// central board, which the Grid letters and opens for published eval
/// results and the agents' notes.
pub const GYM_EVALS_BOARD: Vec3 = Vec3::new(58.8, 2.8, -5.7);
/// The EVALS board's readable front, in line with the central board's.
pub const GYM_EVALS_SCREEN: Vec3 = Vec3::new(58.788, 2.8, -5.7);
/// Half width along Z and half height of the EVALS board, in meters.
pub const GYM_EVALS_HALF: [f32; 2] = GYM_RESULTS_HALF;
/// Where a player stands to read the EVALS board: inside the hall, within
/// reach, a little to the side so the avatar doesn't cover its middle.
pub const GYM_EVALS_STAND: Vec3 = Vec3::new(54.0, 0.0, -4.6);
/// How far ahead of the spawn the Grid's Gym doorway stands, m. The hall
/// lies beyond it along the spawn's heading, past the ball, the stack, and
/// the dominoes, with the doorway facing the spawn between the stack and the
/// dominoes.
pub const GRID_GYM_AHEAD: f32 = 36.0;
/// The Gym's walls in its own (plaza) frame: min and max x and z, m. The
/// west wall has a six-meter doorway between its two halves.
pub const GYM_WALLS: [([f32; 2], [f32; 2]); 5] = [
    ([36.0, -9.0], [60.0, -8.5]),
    ([36.0, 8.5], [60.0, 9.0]),
    ([59.5, -8.5], [60.0, 8.5]),
    ([36.0, -8.5], [36.5, -3.0]),
    ([36.0, 3.0], [36.5, 8.5]),
];
/// The Gym walls' height, m: low, so the third-person camera sees over them.
pub const GYM_WALL_HEIGHT: f32 = 1.2;

/// Where a world stands its Gym. The Gym's geometry and interaction
/// constants (`GYM_*`) are in its own frame, which is Coder's plaza; a site
/// turns that frame `yaw` about the vertical and then moves it by `offset`.
/// Coder's plaza uses [`GymSite::PLAZA`], the identity, so its Gym is
/// unchanged; the Grid uses [`GymSite::GRID`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GymSite {
    /// Rotation about +Y, radians, as `glam::Quat::from_rotation_y`.
    pub yaw: f32,
    /// Translation after the rotation, m.
    pub offset: Vec3,
}

impl GymSite {
    /// Coder's plaza: the Gym east of the plaza, its doorway facing west.
    pub const PLAZA: Self = Self {
        yaw: 0.0,
        offset: Vec3::ZERO,
    };
    /// The Grid: the Gym turned a quarter turn so that its doorway faces the
    /// spawn from [`GRID_GYM_AHEAD`] meters straight ahead, and its hall
    /// runs away from the spawn.
    pub const GRID: Self = Self {
        yaw: -std::f32::consts::FRAC_PI_2,
        offset: Vec3::new(SPAWN.x, 0.0, SPAWN.z + GRID_GYM_AHEAD - GYM_ENTRANCE.x),
    };

    fn rotation(self) -> glam::Quat {
        glam::Quat::from_rotation_y(self.yaw)
    }

    fn identity(self) -> bool {
        self == Self::PLAZA
    }

    /// A Gym-frame point in the world.
    #[must_use]
    pub fn point(self, local: Vec3) -> Vec3 {
        if self.identity() {
            local
        } else {
            self.rotation() * local + self.offset
        }
    }

    /// A Gym-frame direction in the world.
    #[must_use]
    pub fn direction(self, local: Vec3) -> Vec3 {
        if self.identity() {
            local
        } else {
            self.rotation() * local
        }
    }

    /// A world point in the Gym's frame.
    #[must_use]
    pub fn local(self, world: Vec3) -> Vec3 {
        if self.identity() {
            world
        } else {
            self.rotation().inverse() * (world - self.offset)
        }
    }

    /// A world direction in the Gym's frame.
    #[must_use]
    pub fn local_direction(self, world: Vec3) -> Vec3 {
        if self.identity() {
            world
        } else {
            self.rotation().inverse() * world
        }
    }

    /// A Gym-frame heading (the controller's yaw) in the world.
    #[must_use]
    pub fn yaw_of(self, local: f32) -> f32 {
        verse_world::social::controller::wrap(local + self.yaw)
    }

    /// A Gym-frame footprint in the world, as the box around its corners.
    /// Quarter turns keep it exact.
    #[must_use]
    pub fn footprint(self, local: Footprint) -> Footprint {
        if self.identity() {
            return local;
        }
        let corners = [
            [local.min[0], local.min[1]],
            [local.max[0], local.min[1]],
            [local.max[0], local.max[1]],
            [local.min[0], local.max[1]],
        ]
        .map(|[x, z]| self.point(Vec3::new(x, 0.0, z)));
        let fold = |f: fn(f32, f32) -> f32, pick: fn(Vec3) -> f32| {
            corners.iter().copied().map(pick).reduce(f).unwrap_or(0.0)
        };
        // Round away the rotation's float noise, so quarter turns land on
        // the millimeter the walls were drawn on.
        let snap = |v: f32| (v * 1000.0).round() / 1000.0;
        Footprint {
            min: [snap(fold(f32::min, |p| p.x)), snap(fold(f32::min, |p| p.z))],
            max: [snap(fold(f32::max, |p| p.x)), snap(fold(f32::max, |p| p.z))],
        }
    }

    /// The Gym's wall footprints in the world.
    #[must_use]
    pub fn walls(self) -> [Footprint; 5] {
        GYM_WALLS.map(|(min, max)| self.footprint(Footprint { min, max }))
    }

    /// Whether world feet at `position` stand inside the hall, below its
    /// upper structure.
    #[must_use]
    pub fn inside(self, position: Vec3) -> bool {
        let p = self.local(position);
        (36.5..59.5).contains(&p.x) && (-8.5..8.5).contains(&p.z) && (0.0..=4.5).contains(&p.y)
    }

    /// Moves a Gym-frame mesh into the world.
    pub fn transform(self, mesh: &mut Mesh) {
        if self.identity() {
            return;
        }
        for vertex in mesh.lines.iter_mut().chain(mesh.faces.iter_mut()) {
            vertex.pos = self.point(Vec3::from_array(vertex.pos)).to_array();
        }
    }
}

/// Where the pylon stands.
pub const PYLON: Vec3 = Vec3::new(0.0, 0.0, 14.0);
/// Where the quest board stands, facing the plaza.
pub const QUEST_BOARD: Vec3 = Vec3::new(-22.0, 0.0, 0.0);
/// The plaza's center, where a replay's agents start and finish.
pub const PLAZA: Vec3 = Vec3::new(0.0, 0.0, -2.0);
/// The workbench, where an agent's model steps and commands run.
pub const WORKBENCH: Vec3 = Vec3::new(18.0, 0.0, 0.0);
/// The oracle, a door where an agent asks Jev a typed question.
pub const ORACLE: Vec3 = Vec3::new(30.0, 0.0, 28.0);
/// The library, where an agent retrieves and reads knowledge entries.
pub const LIBRARY: Vec3 = Vec3::new(-30.0, 0.0, 28.0);
/// The proving ground, where acceptance tests and the verifier check work.
pub const PROVING_GROUND: Vec3 = Vec3::new(0.0, 0.0, 44.0);
/// Distance of the horizon ridge, in meters.
pub const HORIZON: f32 = 900.0;
