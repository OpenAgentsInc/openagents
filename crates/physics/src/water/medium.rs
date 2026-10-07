//! A character's medium: whether it stands on the ground, wades, swims on
//! the surface, or dives, and how a swimmer moves up and down in the water.
//!
//! The thresholds follow `docs/verse/water.md` (Swimming and wading): a
//! character wades where the water is deeper than [`WADE_DEPTH`] where it
//! stands, swims where it is deeper than [`SWIM_DEPTH`] or the feet are off
//! the bed, and dives while its eye is under the gameplay surface. Every
//! depth here comes from the gameplay surface ([`super::Surface::sample`]),
//! never the drawn one, so every client agrees.
//!
//! A swimmer is not a rigid body: buoyancy pulls it toward a float line
//! [`FLOAT_DEPTH`] under the surface, where its head is out of the water, as
//! a first-order approach rather than a force, and the current carries it
//! at the water's own speed. Rigid bodies float by [`super::apply`] instead.

use serde::{Deserialize, Serialize};

/// Water deeper than this where the character stands makes it wade, m.
pub const WADE_DEPTH: f64 = 0.4;
/// Water deeper than this makes it swim, m.
pub const SWIM_DEPTH: f64 = 1.25;
/// Where a floating swimmer's feet hang under the surface, m: a little
/// shallower than [`SWIM_DEPTH`], so the feet clear the bed where swimming
/// starts.
pub const FLOAT_DEPTH: f64 = 1.2;
/// The eye's height over the feet, m.
pub const EYE_HEIGHT: f64 = 1.6;
/// How far the feet may hang over the bed and still stand on it, m.
pub const STANDING: f64 = 0.05;
/// The highest lip a swimmer climbs out onto, over the surface, m.
pub const CLIMB_LIP: f64 = 0.6;
/// How fast buoyancy lifts an idle diver back toward the float line, m/s.
pub const RISE_SPEED: f64 = 0.6;
/// How fast a swimmer's height closes on the float line at the surface:
/// the rate of the first-order approach, 1/s.
pub const SETTLE_RATE: f64 = 6.0;

/// Where a character is with respect to water.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Medium {
    /// Dry ground, or water no deeper than [`WADE_DEPTH`].
    #[default]
    Ground,
    /// Water deeper than [`WADE_DEPTH`] with the feet on the bed.
    Wading,
    /// On the surface of water deeper than [`SWIM_DEPTH`], or with the feet
    /// off the bed.
    Swimming,
    /// The eye under the surface.
    Diving,
}

impl Medium {
    /// Whether the character is held up by the water rather than the bed.
    #[must_use]
    pub const fn afloat(self) -> bool {
        matches!(self, Self::Swimming | Self::Diving)
    }

    /// Whether the character is in water at all.
    #[must_use]
    pub const fn wet(self) -> bool {
        !matches!(self, Self::Ground)
    }

    /// A short name for logs and the HUD.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ground => "ground",
            Self::Wading => "wading",
            Self::Swimming => "swimming",
            Self::Diving => "diving",
        }
    }
}

/// The medium of a character whose feet are at `feet` over a bed at `bed`
/// under water whose gameplay surface is at `surface` (none: no water). A
/// character in the air over the water is on the ground's rules until it
/// lands in it.
#[must_use]
pub fn classify(surface: Option<f64>, bed: f64, feet: f64) -> Medium {
    let Some(surface) = surface else {
        return Medium::Ground;
    };
    let depth = surface - bed;
    if depth <= WADE_DEPTH || feet > surface {
        Medium::Ground
    } else if feet + EYE_HEIGHT < surface {
        Medium::Diving
    } else if depth > SWIM_DEPTH || feet > bed + STANDING {
        Medium::Swimming
    } else {
        Medium::Wading
    }
}

/// A swimmer's vertical intent for one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stroke {
    /// Vertical swimming speed the swimmer asks for, m/s: negative dives,
    /// positive rises, zero lets buoyancy bring it back to the float line.
    pub vertical: f64,
    /// Held swimmers (Paralyzed, Stunned, or asleep) float face up at the
    /// float line whatever they ask.
    pub held: bool,
}

/// The feet's height after `dt` seconds for a swimmer at `feet` in water
/// whose surface is at `surface` over a bed at `bed`, under `stroke`.
///
/// At the float line the swimmer settles onto it; an idle diver rises
/// toward it at [`RISE_SPEED`]; a stroke moves it at the asked speed, never
/// below the bed nor above the float line.
#[must_use]
pub fn swim_height(feet: f64, surface: f64, bed: f64, stroke: Stroke, dt: f64) -> f64 {
    let float = surface - FLOAT_DEPTH;
    let floor = bed.min(float);
    if !dt.is_finite() || dt <= 0.0 || !feet.is_finite() {
        return feet.clamp(floor, float.max(floor));
    }
    let vertical = if stroke.held { 0.0 } else { stroke.vertical };
    let next = if vertical != 0.0 {
        feet + vertical * dt
    } else if feet < float - 0.05 {
        // Below the float line, buoyancy lifts the diver steadily.
        (feet + RISE_SPEED * dt).min(float)
    } else {
        // At the surface, it settles onto the float line.
        float + (feet - float) * (-SETTLE_RATE * dt).exp()
    };
    next.clamp(floor, float.max(floor))
}

/// Whether a swimmer whose surface is at `surface` can climb out onto a
/// lip at `lip`: one no more than [`CLIMB_LIP`] above the water.
#[must_use]
pub fn can_climb_out(surface: f64, lip: f64) -> bool {
    lip >= surface - STANDING && lip <= surface + CLIMB_LIP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_sets_the_medium() {
        // Dry ground, then shallow water, wading, and swimming as the bed
        // falls away under a surface at zero.
        assert_eq!(classify(None, 0.0, 0.0), Medium::Ground);
        assert_eq!(classify(Some(0.0), -0.3, -0.3), Medium::Ground);
        assert_eq!(classify(Some(0.0), -0.8, -0.8), Medium::Wading);
        assert_eq!(classify(Some(0.0), -2.0, -FLOAT_DEPTH), Medium::Swimming);
        // A wader whose feet leave the bed swims.
        assert_eq!(classify(Some(0.0), -1.0, -0.8), Medium::Swimming);
        // The eye under the surface dives.
        assert_eq!(classify(Some(0.0), -3.0, -2.5), Medium::Diving);
        assert!(Medium::Diving.afloat() && !Medium::Wading.afloat());
    }

    #[test]
    fn a_swimmer_settles_dives_and_rises_back() {
        let (surface, bed) = (0.0, -3.0);
        let mut feet = -0.5;
        for _ in 0..240 {
            feet = swim_height(feet, surface, bed, Stroke::default(), 1.0 / 120.0);
        }
        assert!((feet + FLOAT_DEPTH).abs() < 0.01, "{feet}");
        // Diving reaches the bed and stops there.
        for _ in 0..600 {
            let dive = Stroke {
                vertical: -1.5,
                held: false,
            };
            feet = swim_height(feet, surface, bed, dive, 1.0 / 120.0);
        }
        assert_eq!(feet, bed);
        // Let go, and buoyancy brings it back up at its rate.
        let start = feet;
        feet = swim_height(feet, surface, bed, Stroke::default(), 1.0);
        assert!((feet - start - RISE_SPEED).abs() < 1e-9);
        // A held swimmer floats at the float line whatever it asks.
        let held = Stroke {
            vertical: -2.0,
            held: true,
        };
        let mut floating = -FLOAT_DEPTH;
        for _ in 0..120 {
            floating = swim_height(floating, surface, bed, held, 1.0 / 60.0);
        }
        assert!((floating + FLOAT_DEPTH).abs() < 1e-9);
    }

    #[test]
    fn a_swimmer_climbs_onto_a_low_lip_only() {
        assert!(can_climb_out(0.0, 0.3));
        assert!(can_climb_out(0.0, CLIMB_LIP));
        assert!(!can_climb_out(0.0, 0.9));
        assert!(!can_climb_out(0.0, -1.0));
    }
}
