//! The places a replayed agent visits on the plaza.

use atif::activity::{Activity, Classified, Station};
use glam::Vec3;

use crate::world;

/// How far apart the agent and the ghost stand at one place, in meters.
pub const SIDE: f32 = 1.8;

/// A place an agent visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// The plaza's center: before a run starts and after it finishes.
    Plaza,
    /// Model steps and the commands they run.
    Workbench,
    /// Jev's typed questions.
    Oracle,
    /// Knowledge retrieval.
    Library,
    /// Acceptance tests and the task's verifier.
    ProvingGround,
}

impl Place {
    /// Where the replay draws a classified step. The replay's world has
    /// landmarks for the library, the oracle, and the proving ground; a
    /// finish is the plaza, and every other station is the workbench.
    #[must_use]
    pub fn of(step: Classified) -> Place {
        match (step.activity, step.station) {
            (Activity::Done | Activity::Failed, _) => Place::Plaza,
            (_, Station::Library) => Place::Library,
            (_, Station::Oracle) => Place::Oracle,
            (_, Station::ProvingGround) => Place::ProvingGround,
            _ => Place::Workbench,
        }
    }

    /// The places with a landmark.
    pub const LANDMARKS: [Place; 4] = [
        Place::Workbench,
        Place::Oracle,
        Place::Library,
        Place::ProvingGround,
    ];

    /// The place's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Place::Plaza => "plaza",
            Place::Workbench => "workbench",
            Place::Oracle => "oracle",
            Place::Library => "library",
            Place::ProvingGround => "proving ground",
        }
    }

    /// Where the place is on the ground.
    #[must_use]
    pub fn position(self) -> Vec3 {
        match self {
            Place::Plaza => world::PLAZA,
            Place::Workbench => world::WORKBENCH,
            Place::Oracle => world::ORACLE,
            Place::Library => world::LIBRARY,
            Place::ProvingGround => world::PROVING_GROUND,
        }
    }

    /// Where an agent hovers there: in front of the landmark, on the
    /// plaza's side, the ghost to the right of the player's agent.
    #[must_use]
    pub fn stand(self, ghost: bool) -> Vec3 {
        let at = self.position();
        let toward = (world::PLAZA - at).with_y(0.0);
        // The library's shelves face the spawn side, along -Z; every other
        // landmark faces the plaza.
        let front = if self == Place::Library || toward.length() < 1.0 {
            Vec3::NEG_Z
        } else {
            toward.normalize()
        };
        let right = front.cross(Vec3::Y);
        let reach = if self == Place::Plaza { 0.0 } else { 3.2 };
        let side = if ghost { SIDE } else { -SIDE };
        at + front * reach + right * side + Vec3::Y * crate::agent::HOVER
    }
}
