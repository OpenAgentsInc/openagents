//! Earn on a paired computer (#12294): the engine methods behind the Earn
//! page, which run this computer's `openagents pylon` commands. A browser
//! reads with its grant's observe right and turns sharing on and off with
//! its run right; where payouts go is set only at the computer itself.
//!
//! Each answer is `{exitCode, document}`, the command's JSON document, with
//! anything that looks like a key redacted.

use serde::{Deserialize, Serialize};

/// `pylon status`: whether this computer is sharing, and why not.
pub const PYLON_STATUS: &str = "PylonStatus";
/// `pylon earnings`: what this computer earned, and where it goes.
pub const PYLON_EARNINGS: &str = "PylonEarnings";
/// `pylon settings`: the pause on battery and the pause when hot.
pub const PYLON_SETTINGS: &str = "PylonSettings";
/// `pylon up --yes`: start sharing. Answers when the command finishes, or
/// with `{running: true}` while it still sets up; read [`PYLON_STATUS`]
/// for its progress.
pub const PYLON_UP: &str = "PylonUp";
/// `pylon down`: stop sharing.
pub const PYLON_DOWN: &str = "PylonDown";
/// `pylon settings --pause-on-battery|--pause-when-hot on|off`, with
/// [`SetPause`] params.
pub const PYLON_SET_PAUSE: &str = "PylonSetPause";

/// Every method here.
pub const ALL: &[&str] = &[
    PYLON_STATUS,
    PYLON_EARNINGS,
    PYLON_SETTINGS,
    PYLON_UP,
    PYLON_DOWN,
    PYLON_SET_PAUSE,
];

/// Which pause [`PYLON_SET_PAUSE`] changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pause {
    /// Pause while the computer runs on battery.
    Battery,
    /// Pause while the computer runs hot.
    Hot,
}

impl Pause {
    /// The `pylon settings` flag.
    pub fn flag(self) -> &'static str {
        match self {
            Self::Battery => "--pause-on-battery",
            Self::Hot => "--pause-when-hot",
        }
    }
}

/// [`PYLON_SET_PAUSE`]'s params.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetPause {
    pub pause: Pause,
    pub on: bool,
}

impl SetPause {
    /// The `pylon` arguments that make this change.
    pub fn args(self) -> [&'static str; 3] {
        [
            "settings",
            self.pause.flag(),
            if self.on { "on" } else { "off" },
        ]
    }
}

/// Whether `method` is one of these.
pub fn handles(method: &str) -> bool {
    ALL.contains(&method)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pause_change_names_its_flag() {
        let set: SetPause = serde_json::from_value(serde_json::json!({
            "pause": "hot", "on": false
        }))
        .unwrap();
        assert_eq!(set.args(), ["settings", "--pause-when-hot", "off"]);
        assert!(handles(PYLON_UP) && !handles("PylonPayout"));
    }
}
