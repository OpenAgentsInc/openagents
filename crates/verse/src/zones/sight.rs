//! What the third-person camera sees through in each zone, and the one
//! camera-collision step every surface (desktop, web, and phones) frames
//! with ([`crate::camera::FollowCamera::frame`]).
//!
//! - Everglade, the Grove, the demolition yard, and the crypt: the zone's
//!   solids, which hold every blocker with its top, the roofs and the
//!   crypt's vault, the generated models' columns (a destructible
//!   building's standing pieces only), the yard's standing pieces, and the
//!   terrain.
//! - The plaza, the Grid, and the Physics Lab: the world's blockers, each
//!   up to the top of the geometry standing on it (the Gym's walls are
//!   low), over flat ground.
//! - Ruins: its terrain.
//! - Lagrange 1: nothing; the player flies free.

use glam::Vec3;
use verse_world::social::sight::{Footprints, Ground, Open, Sight};

use super::ZoneId;
use crate::camera::{FOCUS_HEIGHT, Framing};
use crate::controller::Footprint;
use crate::runtime::WorldRuntime;

impl WorldRuntime {
    /// Calls `f` with the current zone's solids as the camera sees them.
    pub(crate) fn with_sight<R>(&self, f: impl FnOnce(&dyn Sight) -> R) -> R {
        match self.zone {
            ZoneId::Lagrange1 => f(&Open),
            ZoneId::Ruins => f(&Ground(|x: f32, z: f32| {
                verse_ruins::scene::Terrain::bundled().height(x, z)
            })),
            _ => {
                if let Some(everglade) = &self.zone_state.everglade {
                    return f(everglade.solids());
                }
                let (revision, count, tops) = &self.zone_state.sight_tops;
                let fresh = *revision == self.zone_revision && *count == self.world.blockers.len();
                f(&Footprints {
                    blocks: &self.world.blockers,
                    tops: if fresh { tops } else { &[] },
                    default_top: f32::INFINITY,
                    floor: 0.0,
                })
            }
        }
    }

    /// Where the orbit wants the eye this frame, before the zone's solids:
    /// over the ground, stood back for a Grove dragon, and inside
    /// Everglade's hall at about head height.
    fn camera_desired(&self, sight: &dyn Sight) -> Vec3 {
        let mut eye = self.camera.desired(self.player.pos, self.player.yaw, sight);
        let focus = self.player.pos + Vec3::Y * FOCUS_HEIGHT;
        if self.zone == ZoneId::Grove {
            // A dragon's shape pulls the camera back along its view and
            // up to its body, so the whole beast fits.
            let pull = self.grove_camera();
            if pull > 1.0 && self.camera.blend() <= 0.0 {
                eye = focus + (eye - focus) * pull + Vec3::Y * (pull - 1.0);
                if let Some(ground) = sight.ground(eye.x, eye.z) {
                    eye.y = eye.y.max(ground + crate::camera::GROUND_CLEARANCE);
                }
            }
        } else if self.zone == ZoneId::Everglade {
            eye = super::everglade::keep_eye_inside(focus, eye);
        }
        eye
    }

    /// This frame's eye after the zone's solids, before any shake.
    #[must_use]
    pub fn framing(&self) -> Framing {
        self.with_sight(|sight| {
            let desired = self.camera_desired(sight);
            self.camera.frame(self.player.pos, desired, sight)
        })
    }

    /// Follows the zone's solids after the player moved this frame: a wall
    /// holds the eye in, and a cleared view lets it ease back out.
    pub(crate) fn track_camera(&mut self, dt: f32) {
        self.refresh_sight_tops();
        let mut camera = self.camera;
        self.with_sight(|sight| {
            let desired = self.camera_desired(sight);
            camera.track(self.player.pos, desired, sight, dt);
        });
        self.camera = camera;
    }

    /// Measures the world's blockers' tops once per zone visit.
    fn refresh_sight_tops(&mut self) {
        let key = (self.zone_revision, self.world.blockers.len());
        let state = &self.zone_state.sight_tops;
        if (state.0, state.1) == key || self.zone_state.everglade.is_some() {
            return;
        }
        let tops = if self.is_bare() {
            // The Grid's only blockers are the Gym's low walls.
            vec![crate::world::GYM_WALL_HEIGHT; self.world.blockers.len()]
        } else {
            let gym = crate::world::GymSite::PLAZA.walls();
            let mut tops = tops_from_mesh(&self.world.mesh, &self.world.blockers);
            for (top, block) in tops.iter_mut().zip(&self.world.blockers) {
                if self.is_plaza() && gym.contains(block) {
                    *top = crate::world::GYM_WALL_HEIGHT;
                }
            }
            tops
        };
        self.zone_state.sight_tops = (key.0, key.1, tops);
    }
}

/// The top of the geometry standing on each footprint, m: the highest
/// vertex over it. A footprint with nothing drawn over it is as tall as
/// anything.
#[must_use]
pub(crate) fn tops_from_mesh(mesh: &crate::mesh::Mesh, blocks: &[Footprint]) -> Vec<f32> {
    const MARGIN: f32 = 0.02;
    let mut tops = vec![f32::NEG_INFINITY; blocks.len()];
    let points = mesh
        .lines
        .iter()
        .chain(&mesh.faces)
        .map(|v| v.pos)
        .chain(mesh.lit.iter().map(|v| v.pos));
    for [x, y, z] in points {
        for (top, block) in tops.iter_mut().zip(blocks) {
            if y > *top && block.contains(x, z, MARGIN) {
                *top = y;
            }
        }
    }
    for top in &mut tops {
        if !top.is_finite() {
            *top = f32::INFINITY;
        }
    }
    tops
}
