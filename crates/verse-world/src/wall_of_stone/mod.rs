//! Wall of Stone (SRD 5.2.1, level 5 Evocation) as jointed, breakable,
//! load-bearing stone panels in a [`physics::World`].
//!
//! - [`shapes`]: panel layouts for a straight wall, a bridge, a ramp, an
//!   enclosure, and a tower.
//! - [`validate`]: the SRD placement rules: contiguous panels, a stone
//!   contact, and the 20-foot span limit.
//! - [`rig`]: the panels as dynamic granite cuboids joined to each other and
//!   to stone; joints break when the solver holds them at a limit, so
//!   collapse comes from the solver rather than from a rule. Damage, debris,
//!   the end of concentration, and permanence.
//! - [`creatures`]: pushing a creature out of a panel's space and the
//!   Dexterity save of a creature the wall would enclose.
//!
//! Every length is in meters, built from [`FEET`] and [`INCH`]. The physics
//! units are SI: kilograms, newtons, and seconds.

pub mod creatures;
pub mod rig;
pub mod shapes;
pub mod validate;

#[cfg(test)]
mod tests;

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FEET: f64 = 0.3048;
/// One inch, m.
pub const INCH: f64 = FEET / 12.0;

/// Spell level.
pub const LEVEL: u8 = 5;
/// Range, m (120 feet).
pub const RANGE: f64 = 120.0 * FEET;
/// Concentration, up to 10 minutes, s. Held this long, the wall is permanent.
pub const DURATION: f64 = 600.0;
/// Armor Class of each panel.
pub const PANEL_AC: i32 = 15;
/// Hit points per inch of panel thickness.
pub const HP_PER_INCH: i32 = 30;
/// Largest unsupported span, m (20 feet).
pub const SPAN_LIMIT: f64 = 20.0 * FEET;
/// Granite density, kg/m^3.
pub const GRANITE_DENSITY: f64 = 2700.0;
/// The wall's size budget in quarter panels: ten 10-by-10-foot panels, ten
/// 10-by-20-foot panels, or forty half-size panels.
pub const BUDGET: u32 = 40;
/// How long debris from a destroyed panel stays, s.
pub const DEBRIS_LIFETIME: f64 = 20.0;
/// A creature's Speed, m (30 feet): how far an enclosed creature that makes
/// its save may move out.
pub const SPEED: f64 = 30.0 * FEET;

/// SRD line for overlays and evidence.
pub const SRD_LINE: &str = "Wall of Stone · level 5 Evocation · range 120 ft · ten 10×10 ft panels, 6 in thick · Dex save if enclosed · concentration, 10 min";

/// The panel sizes the SRD allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    /// 10 by 10 feet, 6 inches thick.
    Thick,
    /// 10 by 20 feet, 3 inches thick.
    Thin,
    /// A thick panel with its edge lengths halved (5 by 5 feet, 6 inches
    /// thick), which a span over 20 feet needs to create supports.
    Half,
    /// A thin panel with its edge lengths halved (5 by 10 feet, 3 inches
    /// thick).
    HalfThin,
}

impl Form {
    /// Full edge lengths: x along the wall, y across it, z through it, m.
    #[must_use]
    pub fn size(self) -> DVec3 {
        match self {
            Self::Thick => DVec3::new(10.0 * FEET, 10.0 * FEET, 6.0 * INCH),
            Self::Thin => DVec3::new(20.0 * FEET, 10.0 * FEET, 3.0 * INCH),
            Self::Half => DVec3::new(5.0 * FEET, 5.0 * FEET, 6.0 * INCH),
            Self::HalfThin => DVec3::new(5.0 * FEET, 10.0 * FEET, 3.0 * INCH),
        }
    }

    #[must_use]
    pub fn thickness_inches(self) -> i32 {
        match self {
            Self::Thick | Self::Half => 6,
            Self::Thin | Self::HalfThin => 3,
        }
    }

    /// Whether the panel is a halved one, which may create supports.
    #[must_use]
    pub fn is_half(self) -> bool {
        matches!(self, Self::Half | Self::HalfThin)
    }

    /// SRD hit points: 30 per inch of thickness.
    #[must_use]
    pub fn hit_points(self) -> i32 {
        HP_PER_INCH * self.thickness_inches()
    }

    /// Granite mass, kg.
    #[must_use]
    pub fn mass(self) -> f64 {
        let s = self.size();
        s.x * s.y * s.z * GRANITE_DENSITY
    }

    /// Cost against [`BUDGET`].
    #[must_use]
    pub fn cost(self) -> u32 {
        match self {
            Self::Thick | Self::Thin => 4,
            Self::Half | Self::HalfThin => 1,
        }
    }
}

/// One panel's pose: its center and the rotation from the panel frame (x
/// along, y across, z through the thickness) to the world.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub form: Form,
    pub center: DVec3,
    pub orientation: DQuat,
}

impl Placement {
    #[must_use]
    pub fn half(&self) -> DVec3 {
        self.form.size() * 0.5
    }

    /// The face normal, world frame.
    #[must_use]
    pub fn normal(&self) -> DVec3 {
        self.orientation * DVec3::Z
    }

    /// Whether the panel stands upright: its face normal is within about
    /// 11 degrees of horizontal.
    #[must_use]
    pub fn vertical(&self) -> bool {
        self.normal().y.abs() < 0.2
    }

    /// World point of a panel-frame point.
    #[must_use]
    pub fn to_world(&self, local: DVec3) -> DVec3 {
        self.center + self.orientation * local
    }
}

/// Damage types a panel distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    Bludgeoning,
    Fire,
    Force,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Slashing,
    Thunder,
}

impl DamageType {
    /// Panels are immune to Poison and Psychic damage.
    #[must_use]
    pub fn harms_panels(self) -> bool {
        !matches!(self, Self::Poison | Self::Psychic)
    }
}

/// Whether an attack roll total hits a panel.
#[must_use]
pub fn hits_panel(attack_total: i32) -> bool {
    attack_total >= PANEL_AC
}

/// Signed distance from `p` to an oriented box, m: negative inside.
#[must_use]
pub fn box_distance(center: DVec3, orientation: DQuat, half: DVec3, p: DVec3) -> f64 {
    let local = orientation.inverse() * (p - center);
    let q = local.abs() - half;
    q.max(DVec3::ZERO).length() + q.max_element().min(0.0)
}

/// Where a ray from `origin` along unit `dir` enters and leaves an oriented
/// box, as distances along the ray, if it crosses it ahead of the origin.
#[must_use]
pub fn ray_box(
    center: DVec3,
    orientation: DQuat,
    half: DVec3,
    origin: DVec3,
    dir: DVec3,
) -> Option<(f64, f64)> {
    let inverse = orientation.inverse();
    let o = inverse * (origin - center);
    let d = inverse * dir;
    let (mut near, mut far) = (f64::NEG_INFINITY, f64::INFINITY);
    for axis in 0..3 {
        if d[axis].abs() < 1e-12 {
            if o[axis].abs() > half[axis] {
                return None;
            }
            continue;
        }
        let a = (-half[axis] - o[axis]) / d[axis];
        let b = (half[axis] - o[axis]) / d[axis];
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far && far >= 0.0).then_some((near, far))
}
