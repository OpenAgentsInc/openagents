//! Where the player arrives in the meteor zones, which the website stands
//! CAD models in front of (`verse::zones::meteor_showcase::SPAWN` and
//! `verse::zones::meteor_stress::SPAWN` are these).

use glam::Vec3;

/// The Meteor Showcase: south of the lot, behind where the establishing
/// view looks from.
pub const METEOR_SHOWCASE: Vec3 = Vec3::new(-50.0, 0.0, -42.0);
/// The Meteor Stress Test: south of the castle.
pub const METEOR_STRESS: Vec3 = Vec3::new(0.0, 0.0, -84.0);
