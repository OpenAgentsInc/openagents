//! What every zone shares with the world runtime above it: the controls a
//! zone answers, its atmosphere, and how a device opens a station's panel.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Enter,
    Return,
    Cancel,
    Retry,
    Firebolt,
    MagicMissile,
    Fireball,
    Grab,
    Release,
    /// Lagrange 1: unclip the safety tether, or clip it back on within reach
    /// of its clip.
    Tether,
    /// Show or hide the physics overlay: contacts, joints, and thrust.
    Forces,
    /// Lagrange 1: switch between the photographic camera and the readable
    /// art preset (brighter shadows and visible stars).
    Camera,
    /// Physics Lab: select the previous or next knob.
    KnobPrev,
    KnobNext,
    /// Physics Lab: step the selected knob down or up one option.
    Decrease,
    Increase,
    /// Physics Lab: rebuild the scenario from its knobs.
    Reset,
    /// Physics Lab: pause or resume the simulation.
    Pause,
    /// Physics Lab: pause and advance one fixed step.
    Step,
    /// Everglade: open the panel of the station in reach. The runtime
    /// changes nothing; the host opens the panel
    /// (`verse::runtime::WorldRuntime::studio_panel_here`).
    Interact,
    Jump,
    Sprint,
    Levitate,
    Rise,
    Lower,
    /// Everglade's hotbar spells
    /// (`verse::zones::everglade::spells`).
    FeatherFall,
    WallOfStone,
    WindWall,
    ReverseGravity,
    /// The Grove's druid spells beside Everglade's
    /// (`verse::zones::grove::kit`); Fire Bolt and Fireball
    /// are [`Self::Firebolt`] and [`Self::Fireball`] there.
    Thunderwave,
    GustOfWind,
    MistyStep,
    Web,
    /// The Grove's demo control: refills mana, cooldowns, and the dummies.
    LongRest,
    /// The Grove's hotbar slot at this index: it casts what the slot holds
    /// when the press arrives, which a Wild Shape changes
    /// (`verse::zones::grove::hotbar`).
    GroveSlot(u8),
    /// The demolition yard: swing the sledgehammer, aim Meteor Swarm, or
    /// rebuild the cottages (`verse::zones::everglade::demolition`).
    Swing,
    MeteorSwarm,
    Rebuild,
}

/// Linear-light scene values; the Coder application UI keeps its own palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub color: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
    /// Fog that thins with height and glows toward the Sun, for zones drawn
    /// on the physical path. `fog_end` stays the distance where fog is
    /// total. The amber zones keep their distance ramp.
    pub height_fog: Option<verse_engine::lighting::HeightFog>,
}
impl Atmosphere {
    pub fn validate(self) -> Result<Self, String> {
        if self
            .color
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || !self.fog_start.is_finite()
            || !self.fog_end.is_finite()
            || !(0.0..=1_000.0).contains(&self.fog_start)
            || self.fog_end <= self.fog_start
            || self.fog_end > 2_000.0
        {
            return Err("Zone atmosphere exceeds its bounds".into());
        }
        if let Some(fog) = &self.height_fog {
            fog.validate()?;
        }
        Ok(self)
    }
}

/// How a device opens the panel at a station, so a caption names the
/// control the person actually has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InteractHint {
    /// A keyboard: the interact key, F.
    #[default]
    Key,
    /// A touchscreen: the zone panel's button for the station.
    Tap,
    /// No panel opens here (the OpenAgents app's Grid, the web page), so no
    /// caption offers one and the zone panel shows no station button.
    None,
}
