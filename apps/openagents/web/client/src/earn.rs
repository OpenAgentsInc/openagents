//! Earn on a paired computer (#12294), as the browser reads it: the
//! `Pylon*` methods (`openagents_rpc::pylon`, which doesn't build for the
//! browser) and the documents they answer with. The computer runs its own
//! `openagents pylon` command; the browser sends the call through its
//! device room.

use serde_json::{Value, json};

pub const PYLON_STATUS: &str = "PylonStatus";
pub const PYLON_EARNINGS: &str = "PylonEarnings";
pub const PYLON_SETTINGS: &str = "PylonSettings";
pub const PYLON_UP: &str = "PylonUp";
pub const PYLON_DOWN: &str = "PylonDown";
pub const PYLON_SET_PAUSE: &str = "PylonSetPause";

/// The command line's exit code for a command it doesn't have.
const EXIT_USAGE: i64 = 64;

/// One answer, read like the desktop's Earn page reads the command.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    /// The command's data.
    Ok(Value),
    /// The computer's command line doesn't have this command.
    Unavailable,
    /// `up` still sets up on the computer.
    Running,
    /// It didn't finish; the message says why.
    Failed(String),
}

impl Reply {
    /// Reads a `Pylon*` answer: `{exitCode, document}` or `{running}`.
    pub fn read(answer: &Value) -> Self {
        if answer["running"] == true {
            return Self::Running;
        }
        let code = answer["exitCode"].as_i64();
        let document = &answer["document"];
        match code {
            Some(EXIT_USAGE) => Self::Unavailable,
            Some(0) => Self::Ok(if document.get("schema").is_some() {
                document["data"].clone()
            } else {
                document.clone()
            }),
            _ => Self::Failed(
                document["summary"]
                    .as_str()
                    .or_else(|| document["error"].as_str())
                    .unwrap_or("It didn't finish.")
                    .to_string(),
            ),
        }
    }

    pub fn data(&self) -> Option<&Value> {
        match self {
            Self::Ok(data) => Some(data),
            _ => None,
        }
    }
}

/// Which pause a [`PYLON_SET_PAUSE`] changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pause {
    Battery,
    Hot,
}

/// [`PYLON_SET_PAUSE`]'s params.
pub fn set_pause_params(pause: Pause, on: bool) -> Value {
    let pause = match pause {
        Pause::Battery => "battery",
        Pause::Hot => "hot",
    };
    json!({ "pause": pause, "on": on })
}

/// Whether `pylon status`'s data says sharing is on, as the desktop's
/// switch reads it: any state but off.
pub fn sharing(status: &Value) -> bool {
    status["state"].as_str().is_some_and(|s| s != "off")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_reads_as_the_desktop_reads_the_command() {
        let ok = json!({"exitCode": 0, "document": {"schema": "x", "data": {"state": "serving"}}});
        assert_eq!(Reply::read(&ok), Reply::Ok(json!({"state": "serving"})));
        assert!(sharing(Reply::read(&ok).data().unwrap()));
        assert_eq!(
            Reply::read(&json!({"exitCode": 64, "document": {}})),
            Reply::Unavailable
        );
        assert_eq!(Reply::read(&json!({"running": true})), Reply::Running);
        assert_eq!(
            Reply::read(&json!({"exitCode": 1, "document": {"summary": "No model"}})),
            Reply::Failed("No model".into())
        );
        assert_eq!(
            set_pause_params(Pause::Hot, false),
            json!({"pause": "hot", "on": false})
        );
    }
}
