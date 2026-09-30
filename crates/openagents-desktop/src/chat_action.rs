//! The closed actions a desktop chat view admits.
use serde::Serialize;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Send,
    Stop,
    Retry,
    Archive,
    Restore,
    Earlier,
    Latest,
    Followup { text: String },
}
