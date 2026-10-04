//! Committed authority events for presentation and replication.
use serde::{Deserialize, Serialize};
use verse_engine::core::LifeId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Dialogue { text: String },
    CameraHandoff,
    Damage { amount: i32, incoming: bool },
    Death,
    Respawn,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub instance: u64,
    pub serial: u64,
    pub tick: u64,
    pub time: f32,
    pub actor: Option<LifeId>,
    pub kind: Kind,
}
