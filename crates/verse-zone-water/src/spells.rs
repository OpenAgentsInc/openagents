//! The Water Lab's water spells, after the SRD 5.2.1 descriptions, with no
//! mana and no cooldowns, as in the Grove (`docs/verse/combat-model.md`:
//! the rules run behind the scenes, and the world shows what they do).
//!
//! - **Water Walk** (1 hour): the caster walks and runs on the surface.
//! - **Control Water** (concentration, up to 10 minutes, a 100-foot cube):
//!   Flood raises the water and drives big waves onto the shore (in a large
//!   body of water, the SRD's flood is a wave that crashes down); Part
//!   Water opens a trench with a wall of water to either side; Redirect
//!   Flow moves the water the way the caster faces; Whirlpool spins a
//!   vortex 50 feet wide that drags floating things in and down. Casting
//!   it again while it lasts switches to the next mode.
//! - **Create or Destroy Water** (instantaneous): Create rains on a
//!   30-foot cube, wetting what it falls on and filling puddles; Destroy
//!   lifts water away as vapor, drying the ground and drawing the sea down
//!   for a moment.
//! - **Sleet Storm** (concentration, up to 1 minute, a 40-foot-radius,
//!   20-foot-tall cylinder): freezing rain that ices the water over into
//!   slick, walkable ground, with steam where it hits open water. The ice
//!   lasts for the spell, then thaws over a few seconds.
//! - **Water Breathing** (24 hours): the caster can dive and stay under.
//!   Without it a held breath lasts 1 + Constitution modifier minutes,
//!   at least 30 seconds (the SRD's suffocation rule); the lab's character
//!   has a Constitution modifier of +1.

use glam::{Vec2, Vec3};

/// Feet to meters.
pub const FOOT: f32 = 0.3048;
/// Control Water's cube edge, m.
pub const CONTROL_SIDE: f32 = 100.0 * FOOT;
/// Control Water's longest concentration, s.
pub const CONTROL_DURATION: f32 = 600.0;
/// How far a flood raises the sea, m. The SRD allows up to 20 feet; the
/// lab raises it far enough to cover the beach.
pub const FLOOD_RISE: f32 = 1.8;
/// The flood's waves over the usual swell.
pub const FLOOD_SWELL: f32 = 2.8;
/// Whirlpool's radius at the surface: 50 feet across.
pub const WHIRL_RADIUS: f32 = 25.0 * FOOT;
/// Create Water's cube edge, m.
pub const RAIN_SIDE: f32 = 30.0 * FOOT;
/// How long the lab's rain falls, s.
pub const RAIN_TIME: f32 = 10.0;
/// How long rain's wetness takes to dry, s.
pub const DRY_TIME: f32 = 45.0;
/// Sleet Storm's cylinder: radius and height, m, and its longest
/// concentration, s.
pub const SLEET_RADIUS: f32 = 40.0 * FOOT;
pub const SLEET_HEIGHT: f32 = 20.0 * FOOT;
pub const SLEET_DURATION: f32 = 60.0;
/// How long ice takes to form and to thaw, s.
pub const FREEZE_TIME: f32 = 3.0;
pub const THAW_TIME: f32 = 5.0;
/// The lab character's Constitution modifier.
pub const CON_MODIFIER: i32 = 1;

/// A held breath, s: 1 + Constitution modifier minutes, at least 30 s.
#[must_use]
pub fn breath_limit(con: i32) -> f32 {
    ((1 + con) as f32 * 60.0).max(30.0)
}

/// Control Water's four effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Flood,
    Part,
    Redirect,
    Whirlpool,
}

impl Mode {
    pub const ALL: [Self; 4] = [Self::Flood, Self::Part, Self::Redirect, Self::Whirlpool];

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Flood => Self::Part,
            Self::Part => Self::Redirect,
            Self::Redirect => Self::Whirlpool,
            Self::Whirlpool => Self::Flood,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Flood => "Flood",
            Self::Part => "Part Water",
            Self::Redirect => "Redirect Flow",
            Self::Whirlpool => "Whirlpool",
        }
    }
}

/// A live Control Water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Control {
    pub mode: Mode,
    pub center: Vec2,
    /// The trench's or the current's direction.
    pub dir: Vec2,
    pub started: f32,
}

/// A spell placed somewhere for a while.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub center: Vec2,
    pub started: f32,
}

/// The hotbar's slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    WaterWalk,
    ControlWater,
    CreateWater,
    SleetStorm,
    WaterBreathing,
    /// Our own spell, after Create or Destroy Water and Control Water
    /// ([`crate::orb`]).
    WaterOrb,
    /// The Grove's Thunderbolt ([`crate::bolt`]).
    Thunderbolt,
    Drop,
}

impl Slot {
    pub const ALL: [Self; 8] = [
        Self::WaterWalk,
        Self::ControlWater,
        Self::CreateWater,
        Self::SleetStorm,
        Self::WaterBreathing,
        Self::WaterOrb,
        Self::Thunderbolt,
        Self::Drop,
    ];

    /// The slot's icon sprite, from the shared spell icons.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::WaterWalk => "freedom-of-movement-icon",
            Self::ControlWater => "elementalism-icon",
            Self::CreateWater => "fog-cloud-icon",
            Self::SleetStorm => "sleet-storm-icon",
            Self::WaterBreathing => "blur-icon",
            Self::WaterOrb => "water-orb-icon",
            Self::Thunderbolt => "thunderbolt-icon",
            Self::Drop => "conjure-animals-icon",
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::WaterWalk => "Water Walk",
            Self::ControlWater => "Control Water",
            Self::CreateWater => "Create or Destroy Water",
            Self::SleetStorm => "Sleet Storm",
            Self::WaterBreathing => "Water Breathing",
            Self::WaterOrb => "Water Orb",
            Self::Thunderbolt => "Thunderbolt",
            Self::Drop => "Drop a float",
        }
    }
}

/// Every water spell's state.
#[derive(Clone, Debug, Default)]
pub struct Spells {
    pub water_walk: bool,
    pub breathing: bool,
    pub control: Option<Control>,
    /// The mode the next Control Water starts in.
    pub mode: Mode,
    pub rain: Option<Placed>,
    /// Where rain or a burst orb last fell, and how wet it still is.
    pub wet: Option<(Vec2, f32)>,
    /// How far the wetness reaches from its center, m: Create Water's
    /// cube, or a burst orb's splash. Zero means the cube's.
    pub wet_radius: f32,
    pub drain: Option<Placed>,
    pub sleet: Option<Placed>,
    /// The ice: where, and how frozen, 0 to 1.
    pub ice: Option<(Vec2, f32)>,
    /// How far the flood has raised the sea, m, and its waves' gain.
    pub rise: f32,
    pub swell: f32,
    /// How far the trench has opened, and the whirlpool spun up, 0 to 1.
    pub part: f32,
    pub whirl: f32,
}

impl Spells {
    /// Advances the spells by `dt` at lab time `now`.
    pub fn tick(&mut self, dt: f32, now: f32) {
        let mode = self.control.map(|c| c.mode);
        if let Some(control) = self.control
            && now - control.started > CONTROL_DURATION
        {
            self.control = None;
        }
        let ease = |value: &mut f32, target: f32, rate: f32| {
            *value += (target - *value).clamp(-rate * dt, rate * dt);
        };
        ease(
            &mut self.rise,
            if mode == Some(Mode::Flood) {
                FLOOD_RISE
            } else {
                0.0
            },
            0.25,
        );
        ease(
            &mut self.swell,
            if mode == Some(Mode::Flood) {
                FLOOD_SWELL
            } else {
                1.0
            },
            0.4,
        );
        ease(
            &mut self.part,
            if mode == Some(Mode::Part) { 1.0 } else { 0.0 },
            0.5,
        );
        let draining = self.drain.is_some_and(|d| now - d.started < 5.0);
        let target = if mode == Some(Mode::Whirlpool) {
            1.0
        } else if draining {
            0.5
        } else {
            0.0
        };
        ease(&mut self.whirl, target, 0.3);
        if self.drain.is_some_and(|d| now - d.started > 12.0) {
            self.drain = None;
        }
        if let Some(rain) = self.rain {
            self.wet_radius = RAIN_SIDE * 0.5;
            let wet = self.wet.map_or(0.0, |(_, w)| w);
            self.wet = Some((rain.center, (wet + dt / 4.0).min(1.0)));
            if now - rain.started > RAIN_TIME {
                self.rain = None;
            }
        } else if let Some((at, wet)) = self.wet {
            let wet = wet - dt / DRY_TIME;
            self.wet = (wet > 0.0).then_some((at, wet));
        }
        if let Some(sleet) = self.sleet {
            let ice = self.ice.map_or(0.0, |(_, a)| a);
            self.ice = Some((sleet.center, (ice + dt / FREEZE_TIME).min(1.0)));
            if now - sleet.started > SLEET_DURATION {
                self.sleet = None;
            }
        } else if let Some((at, ice)) = self.ice {
            let ice = ice - dt / THAW_TIME;
            self.ice = (ice > 0.0).then_some((at, ice));
        }
    }

    /// Casts Control Water at `center` facing `dir`, or, while it lasts,
    /// switches it to the next mode. Returns what happened.
    pub fn control_water(&mut self, center: Vec2, dir: Vec2, now: f32) -> String {
        let mode = match self.control {
            Some(live) => live.mode.next(),
            None => self.mode,
        };
        self.mode = mode;
        // Part Water's trench and Redirect Flow run the way the caster faces.
        self.control = Some(Control {
            mode,
            center,
            dir: dir.normalize_or(Vec2::Y),
            started: now,
        });
        format!("Control Water: {}", mode.name())
    }

    /// Ends Control Water.
    pub fn dismiss_control(&mut self) -> String {
        self.control = None;
        "Control Water ends".into()
    }

    /// Create Water's rain at `center`.
    pub fn create(&mut self, center: Vec2, now: f32) -> String {
        self.rain = Some(Placed {
            center,
            started: now,
        });
        "Create Water: rain falls on a 30-foot cube".into()
    }

    /// Destroy Water at `center`.
    pub fn destroy(&mut self, center: Vec2, now: f32) -> String {
        self.rain = None;
        self.wet = None;
        self.drain = Some(Placed {
            center,
            started: now,
        });
        "Destroy Water: the water lifts away as vapor".into()
    }

    /// Sleet Storm at `center`; a second cast ends it.
    pub fn sleet_storm(&mut self, center: Vec2, now: f32) -> String {
        if self.sleet.is_some() {
            self.sleet = None;
            return "Sleet Storm ends; the ice thaws".into();
        }
        self.sleet = Some(Placed {
            center,
            started: now,
        });
        "Sleet Storm: freezing rain in a 40-foot cylinder".into()
    }

    /// The point a placed spell lands on: `reach` m ahead of `at`.
    #[must_use]
    pub fn aim(at: Vec3, forward: Vec3, reach: f32) -> Vec2 {
        Vec2::new(at.x + forward.x * reach, at.z + forward.z * reach)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breath_follows_the_srd() {
        assert_eq!(breath_limit(1), 120.0);
        assert_eq!(breath_limit(-1), 30.0);
        assert_eq!(breath_limit(-3), 30.0);
    }

    #[test]
    fn control_water_cycles_its_modes_and_floods_then_recedes() {
        let mut s = Spells::default();
        assert!(s.control_water(Vec2::ZERO, Vec2::Y, 0.0).contains("Flood"));
        for i in 0..200 {
            s.tick(0.1, i as f32 * 0.1);
        }
        assert!((s.rise - FLOOD_RISE).abs() < 1e-3);
        assert!(s.control_water(Vec2::ZERO, Vec2::Y, 20.0).contains("Part"));
        assert!(
            s.control_water(Vec2::ZERO, Vec2::Y, 21.0)
                .contains("Redirect")
        );
        assert!(
            s.control_water(Vec2::ZERO, Vec2::Y, 22.0)
                .contains("Whirlpool")
        );
        assert!(s.control_water(Vec2::ZERO, Vec2::Y, 23.0).contains("Flood"));
        s.dismiss_control();
        for i in 0..200 {
            s.tick(0.1, 30.0 + i as f32 * 0.1);
        }
        assert!(s.rise.abs() < 1e-3);
    }

    #[test]
    fn sleet_ices_the_water_for_its_duration_then_it_thaws() {
        let mut s = Spells::default();
        s.sleet_storm(Vec2::ZERO, 0.0);
        let mut t = 0.0;
        while t < SLEET_DURATION - 1.0 {
            s.tick(0.1, t);
            t += 0.1;
        }
        assert_eq!(s.ice.map(|(_, a)| a), Some(1.0));
        while t < SLEET_DURATION + THAW_TIME + 1.0 {
            s.tick(0.1, t);
            t += 0.1;
        }
        assert!(s.ice.is_none());
    }
}
