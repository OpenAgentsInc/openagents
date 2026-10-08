//! An engine sign-in status read.
//!
//! A device asks the host for one engine's sign-in status. The host runs
//! the engine's own status command inside the computer and answers with
//! [`Value::EngineStatus`](crate::wire::Value::EngineStatus): a
//! [`Status`] that holds no text, so no credential crosses the computer
//! boundary (`docs/cloud/claude-code-byo.md`). The device names only the
//! engine; the host chooses the program.

use serde::{Deserialize, Serialize};

pub use coder_engine_status::{Engine, Method, Notice, Plan, State, Status};

use crate::wire::{Refusal, common_id, version};

/// `v` of an engine status read.
pub const ENGINE_STATUS: &str = "openagents.terminal-engine-status.v1";

/// Read one engine's sign-in status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineStatusRead {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub engine: Engine,
}

impl EngineStatusRead {
    #[must_use]
    pub fn new(request: impl Into<String>, engine: Engine) -> Self {
        EngineStatusRead {
            v: ENGINE_STATUS.into(),
            requires: Vec::new(),
            request: request.into(),
            engine,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, ENGINE_STATUS)?;
        if !self.requires.is_empty() {
            return Err(Refusal::new(
                crate::wire::Reason::UnsupportedFeature,
                "an engine status read requires no feature",
            ));
        }
        common_id(&self.request, "request")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_names_only_the_engine() {
        let read = EngineStatusRead::new("a".repeat(64), Engine::Claude);
        assert!(read.check().is_ok());
        let mut value = serde_json::to_value(&read).unwrap();
        value["program"] = "/bin/sh".into();
        assert!(serde_json::from_value::<EngineStatusRead>(value).is_err());
        let mut other = read.clone();
        other.requires = vec!["sessions".into()];
        assert!(other.check().is_err());
        let answer = crate::wire::Value::EngineStatus {
            status: Status::unavailable(Engine::Claude, 1),
        };
        let text = serde_json::to_string(&answer).unwrap();
        assert_eq!(
            serde_json::from_str::<crate::wire::Value>(&text).unwrap(),
            answer
        );
    }
}
