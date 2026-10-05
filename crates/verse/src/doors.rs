//! Local demonstration doors. Keys select routes, never permissions or services.

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub mod hud;
mod mesh;
#[cfg(feature = "desktop")]
pub mod store;
#[cfg(test)]
mod tests;

pub use mesh::{geometry, held_mesh};
pub use verse_core::label::label as scene_label;

pub const RANGE: f32 = 5.0;
pub const REACTION_SECONDS: f32 = 0.8;
const COOLDOWN_SECONDS: f32 = 0.5;
pub const PREFERENCES_LIMIT: usize = 2_048;
pub const PLANE_HALF: [f32; 2] = [1.35, 1.65];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoorId {
    Spark,
    Halo,
}
impl DoorId {
    pub const ALL: [Self; 2] = [Self::Spark, Self::Halo];
    pub const fn index(self) -> usize {
        match self {
            Self::Spark => 0,
            Self::Halo => 1,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spark => "spark",
            Self::Halo => "halo",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Spark => "Spark",
            Self::Halo => "Halo",
        }
    }
    pub fn position(self) -> Vec3 {
        Vec3::new(if self == Self::Spark { -12.0 } else { 12.0 }, 0.0, -6.0)
    }
    pub fn plane(self) -> Vec3 {
        self.position() + Vec3::new(0.0, 1.8, -0.12)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DemoItem {
    #[default]
    Prism,
    Ring,
    Bolt,
    Empty,
}
impl DemoItem {
    pub const ALL: [Self; 4] = [Self::Prism, Self::Ring, Self::Bolt, Self::Empty];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prism => "prism",
            Self::Ring => "ring",
            Self::Bolt => "bolt",
            Self::Empty => "empty",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prism => "Prism",
            Self::Ring => "Ring",
            Self::Bolt => "Bolt",
            Self::Empty => "Empty",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    Library,
    GymApproach,
    ProvingGround,
    Oracle,
}
impl Destination {
    pub const fn point(self) -> [f32; 2] {
        match self {
            Self::Library => [-26.0, 24.0],
            Self::GymApproach => [33.0, 0.0],
            Self::ProvingGround => [0.0, 41.0],
            Self::Oracle => [27.0, 25.0],
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::GymApproach => "Gym approach",
            Self::ProvingGround => "Proving ground",
            Self::Oracle => "Oracle",
        }
    }
    pub const fn caption(self) -> &'static str {
        match self {
            Self::Library => "Library / tap to walk",
            Self::GymApproach => "Gym / tap to walk",
            Self::ProvingGround => "Proving ground / tap to walk",
            Self::Oracle => "Oracle / tap to walk",
        }
    }
}

pub const fn destination(id: DoorId, item: DemoItem) -> Option<Destination> {
    match (id, item) {
        (DoorId::Spark, DemoItem::Prism) => Some(Destination::Library),
        (DoorId::Spark, DemoItem::Bolt) => Some(Destination::GymApproach),
        (DoorId::Halo, DemoItem::Prism) => Some(Destination::ProvingGround),
        (DoorId::Halo, DemoItem::Ring) => Some(Destination::Oracle),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorPhase {
    #[default]
    Idle,
    Reacting,
    Selected,
    Cooldown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorIntent {
    Hold(DemoItem),
    Tap(DoorId),
    Reset(DoorId),
}
impl Serialize for DoorIntent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::Hold(item) => {
                map.serialize_entry("action", "door_hold")?;
                map.serialize_entry("item", item)?;
            }
            Self::Tap(door) => {
                map.serialize_entry("action", "door_tap")?;
                map.serialize_entry("door", door)?;
            }
            Self::Reset(door) => {
                map.serialize_entry("action", "door_reset")?;
                map.serialize_entry("door", door)?;
            }
        }
        map.end()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapResult {
    Reacted,
    Walk(Destination),
    Refused,
    Ignored,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DoorState {
    pub last: Option<DemoItem>,
    pub selected: Option<Destination>,
    pub phase: DoorPhase,
    pub reaction: u64,
    elapsed: f32,
    notice: Option<&'static str>,
}
impl DoorState {
    pub fn caption(&self) -> &'static str {
        self.notice.unwrap_or_else(|| {
            self.selected
                .map_or("Choose a key. Tap the door.", Destination::caption)
        })
    }
    pub fn reaction_progress(&self) -> Option<f32> {
        (self.phase == DoorPhase::Reacting)
            .then_some((self.elapsed / REACTION_SECONDS).clamp(0.0, 1.0))
    }
    fn clear(&mut self) {
        self.selected = None;
        self.phase = DoorPhase::Idle;
        self.elapsed = 0.0;
        self.notice = None;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Doors {
    held: DemoItem,
    states: [DoorState; 2],
    revision: u64,
    pub(crate) route_owner: Option<DoorId>,
}
impl Doors {
    pub fn held(&self) -> DemoItem {
        self.held
    }
    pub fn state(&self, id: DoorId) -> &DoorState {
        &self.states[id.index()]
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn hold(&mut self, item: DemoItem) {
        if self.held != item {
            self.held = item;
            self.revision = self.revision.saturating_add(1);
            self.cancel_transient();
        }
    }
    pub fn tap(&mut self, id: DoorId) -> TapResult {
        let state = &mut self.states[id.index()];
        if matches!(state.phase, DoorPhase::Reacting | DoorPhase::Cooldown) {
            return TapResult::Ignored;
        }
        let item = if self.held == DemoItem::Empty {
            state.last
        } else {
            Some(self.held)
        };
        let Some(item) = item else {
            state.notice = Some("Choose a key");
            return TapResult::Refused;
        };
        let Some(target) = destination(id, item) else {
            state.notice = Some("This key does not fit");
            return TapResult::Refused;
        };
        state.notice = None;
        if state.phase == DoorPhase::Selected
            && state.selected == Some(target)
            && state.last == Some(item)
        {
            state.phase = DoorPhase::Cooldown;
            state.elapsed = 0.0;
            return TapResult::Walk(target);
        }
        if state.last != Some(item) {
            state.last = Some(item);
            self.revision = self.revision.saturating_add(1);
        }
        state.selected = Some(target);
        state.phase = DoorPhase::Reacting;
        state.elapsed = 0.0;
        state.reaction = state.reaction.saturating_add(1);
        TapResult::Reacted
    }
    pub fn reset(&mut self, id: DoorId) {
        let state = &mut self.states[id.index()];
        if state.last.take().is_some() {
            self.revision = self.revision.saturating_add(1);
        }
        state.clear();
    }
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(crate::runtime::MAX_FRAME_SECONDS);
        for state in &mut self.states {
            match state.phase {
                DoorPhase::Reacting => {
                    state.elapsed += dt;
                    if state.elapsed >= REACTION_SECONDS {
                        state.phase = DoorPhase::Selected;
                        state.elapsed = 0.0;
                    }
                }
                DoorPhase::Cooldown => {
                    state.elapsed += dt;
                    if state.elapsed >= COOLDOWN_SECONDS {
                        state.clear();
                    }
                }
                _ => {}
            }
        }
    }
    pub fn cancel_transient(&mut self) {
        for state in &mut self.states {
            state.clear();
        }
    }
    pub fn route_failed(&mut self, id: DoorId) {
        let state = &mut self.states[id.index()];
        state.clear();
        state.notice = Some("No route found");
    }
    pub fn mesh(&self, player: &crate::controller::PlayerController) -> crate::mesh::Mesh {
        let mut mesh = held_mesh(self.held, player);
        mesh.extend(&mesh::dynamic(self));
        mesh
    }
    pub fn document(&self) -> String {
        serde_json::to_string(&Preferences {
            v: 1,
            world: "verse-plaza".into(),
            definition: "demo-doors-v1".into(),
            held: self.held,
            last: self.states.clone().map(|state| state.last),
        })
        .expect("closed preferences serialize")
    }
    pub fn restore(&mut self, document: &str) -> Result<(), String> {
        if document.len() > PREFERENCES_LIMIT {
            return Err("Door choices exceed the storage limit".into());
        }
        let prefs: Preferences =
            serde_json::from_str(document).map_err(|_| "Door choices are invalid")?;
        if prefs.v != 1
            || prefs.world != "verse-plaza"
            || prefs.definition != "demo-doors-v1"
            || DoorId::ALL.iter().any(|&id| {
                prefs.last[id.index()].is_some_and(|item| destination(id, item).is_none())
            })
        {
            return Err("Door choices use an unsupported definition".into());
        }
        let revision = self.revision.saturating_add(1);
        *self = Self::default();
        self.held = prefs.held;
        self.revision = revision;
        for id in DoorId::ALL {
            self.states[id.index()].last = prefs.last[id.index()];
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    v: u8,
    world: String,
    definition: String,
    held: DemoItem,
    last: [Option<DemoItem>; 2],
}
