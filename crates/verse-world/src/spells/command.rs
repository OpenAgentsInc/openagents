//! Typed spell controls carried through normal cast admission.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Explicit targets, by actor or prop life entity. Unused entries are zero.
    Targets {
        slot: u8,
        targets: [u64; 5],
        count: u8,
    },
    /// Change a held target's altitude, in millimeters.
    Altitude(i32),
    /// Steer the Telekinesis hand to a point in millimeters.
    Hand([i32; 3]),
    /// Release the current Telekinesis grip, preserving velocity.
    Release,
    /// Choose a ground point for an area spell, in millimeters.
    Point { slot: u8, point: [i32; 3] },
    /// Re-aim the wind with a horizontal direction in millimeters.
    Wind([i32; 3]),
    /// Make the controlled creature's Athletics escape check.
    Escape,
    /// Author a stone layout: straight, bridge, ramp, tower, or enclosure.
    Stone {
        shape: u8,
        from: [i32; 3],
        to: [i32; 3],
        count: u8,
        thin: bool,
    },
    /// Choose a continuous grounded Wind Wall path in millimeters.
    WindWall { points: [[i32; 2]; 8], count: u8 },
    /// Choose the four distinct Meteor Swarm impact points.
    Meteors([[i32; 3]; 4]),
    /// End the caster's current concentration.
    EndConcentration,
}
impl Command {
    pub fn slot(self) -> u8 {
        match self {
            Self::Targets { slot, .. } | Self::Point { slot, .. } => slot,
            Self::Altitude(_) => 2,
            Self::Hand(_) | Self::Release => 0,
            Self::EndConcentration => 9,
            Self::Wind(_) => 4,
            Self::Escape => 6,
            Self::Stone { .. } => 1,
            Self::Meteors(_) => 7,
            Self::WindWall { .. } => 5,
        }
    }
}
