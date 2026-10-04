//! Semantic animation selection and compiled clip playback settings.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Idle,
    Death,
    Walk,
    Run,
    Backpedal,
    StrafeLeft,
    StrafeRight,
    Airborne,
    CombatReady,
    CombatReadyAlternate,
    Cast,
    SpellRelease,
    BowReady,
    BowRelease,
    Prone,
    Yell,
    Affirm,
}
impl State {
    pub const ALL: [Self; 17] = [
        Self::Idle,
        Self::Death,
        Self::Walk,
        Self::Run,
        Self::Backpedal,
        Self::StrafeLeft,
        Self::StrafeRight,
        Self::Airborne,
        Self::CombatReady,
        Self::CombatReadyAlternate,
        Self::Cast,
        Self::SpellRelease,
        Self::BowReady,
        Self::BowRelease,
        Self::Prone,
        Self::Yell,
        Self::Affirm,
    ];
}
/// Named states are the owned contract. Numbers preserve retained research packs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Selection {
    Named(State),
    Legacy(u16),
}
impl From<State> for Selection {
    fn from(state: State) -> Self {
        Self::Named(state)
    }
}
impl From<u16> for Selection {
    fn from(id: u16) -> Self {
        Self::Legacy(id)
    }
}
impl Selection {
    pub fn grounded(self) -> bool {
        matches!(
            self,
            Self::Named(State::Death | State::Prone) | Self::Legacy(1 | 100)
        )
    }
    pub fn casting(self) -> bool {
        matches!(
            self,
            Self::Named(State::Cast | State::SpellRelease) | Self::Legacy(52 | 53)
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Loop,
    Hold,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub clip: u16,
    pub mode: Mode,
    pub transition_seconds: f32,
}
