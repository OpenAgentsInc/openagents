//! The keeper wire: one JSON object per line over the keeper's Unix socket.
//!
//! Version 1. An engine sends [`ToKeeper::Hello`] first; the keeper answers
//! [`FromKeeper::Welcome`] (or [`FromKeeper::Refused`]) and then streams
//! [`FromKeeper::Frame`]s. Both sides ignore operations they don't know, so a
//! later version can add operations without breaking an older peer. A peer
//! refuses a version outside [`MIN_PROTOCOL`]..=[`PROTOCOL`].
//!
//! A terminal keeper (#11951) holds a PTY instead of an agent's pipes. Its
//! stdout frames carry the PTY's output bytes in base64, and it takes
//! [`ToKeeper::Input`] and [`ToKeeper::Resize`]. Only engines that start
//! terminal keepers send those, and older keepers ignore them.

use serde::{Deserialize, Serialize};

/// The protocol version this build speaks.
pub const PROTOCOL: u32 = 1;
/// The oldest protocol version this build still adopts. An engine must adopt
/// keepers started by its previous release.
pub const MIN_PROTOCOL: u32 = 1;
/// The protocol version the previous release's keepers speak. An update
/// starts the new engine while those keepers still run, so
/// [`MIN_PROTOCOL`] may never pass it.
pub const PREVIOUS_RELEASE: u32 = 1;

/// What one journaled frame carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FrameBody {
    /// One line the agent wrote to stdout, without its newline.
    Stdout { line: String },
    /// One line the agent wrote to stderr, without its newline.
    Stderr { line: String },
    /// One line an engine wrote to the agent's stdin, without its newline.
    Stdin { line: String },
    /// The agent exited. `code` is its exit code; `signal` the signal that
    /// ended it, when one did.
    Exit {
        code: Option<i32>,
        signal: Option<i32>,
    },
}

/// One journaled frame. Sequence numbers start at 1 and increase by one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub seq: u64,
    #[serde(flatten)]
    pub body: FrameBody,
}

/// A signal an engine asks the keeper to send to the agent's process group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeeperSignal {
    Interrupt,
    Terminate,
    Kill,
}

/// Engine to keeper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ToKeeper {
    /// Authenticate and start streaming. `after` replays frames with a
    /// greater sequence number; `None` uses the last sequence number the
    /// keeper recorded as acknowledged.
    Hello {
        version: u32,
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<u64>,
    },
    /// Write `line` and a newline to the agent's stdin.
    Stdin { line: String },
    /// Close the agent's stdin.
    CloseStdin,
    /// Every frame up to and including `seq` is handled; don't replay it.
    Ack { seq: u64 },
    /// Replay frame `seq` to every later engine even after it is
    /// acknowledged, until it is released. A driver holds a question the
    /// agent is waiting on, so a new engine asks it again.
    Hold { seq: u64 },
    /// Stop holding frame `seq`.
    Release { seq: u64 },
    /// Signal the agent's process group.
    Signal { signal: KeeperSignal },
    /// The engine has handled the run's end: remove the run folder and exit.
    Finish,
    /// Terminal keepers: write these bytes, in base64, to the PTY.
    Input { data: String },
    /// Terminal keepers: resize the PTY.
    Resize { cols: u16, rows: u16 },
    #[serde(other)]
    Unknown,
}

/// Keeper to engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum FromKeeper {
    Welcome {
        version: u32,
        /// The agent's process id, which also leads its process group.
        pid: u32,
        /// The highest sequence number journaled so far.
        last_seq: u64,
        /// The agent has exited; its exit is journaled.
        #[serde(default)]
        exited: bool,
        /// The highest sequence number an engine acknowledged.
        #[serde(default)]
        acked: u64,
    },
    Refused {
        reason: String,
    },
    Frame(Frame),
    #[serde(other)]
    Unknown,
}

/// The version two peers use, or `None` when they share none.
pub fn negotiate(theirs: u32) -> Option<u32> {
    (theirs >= MIN_PROTOCOL).then_some(theirs.min(PROTOCOL))
}

/// Whether this build speaks `version`. A keeper's welcome names the version
/// the connection uses, so an engine refuses one outside its own range.
pub fn speaks(version: u32) -> bool {
    (MIN_PROTOCOL..=PROTOCOL).contains(&version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_unknown_operations_are_ignored() {
        let frame = FromKeeper::Frame(Frame {
            seq: 7,
            body: FrameBody::Stdout { line: "{}".into() },
        });
        let text = serde_json::to_string(&frame).unwrap();
        assert_eq!(
            text,
            r#"{"op":"frame","seq":7,"kind":"stdout","line":"{}"}"#
        );
        assert_eq!(serde_json::from_str::<FromKeeper>(&text).unwrap(), frame);
        assert_eq!(
            serde_json::from_str::<ToKeeper>(r#"{"op":"later","x":1}"#).unwrap(),
            ToKeeper::Unknown
        );
        assert_eq!(negotiate(1), Some(1));
        assert_eq!(negotiate(9), Some(PROTOCOL));
        assert_eq!(negotiate(0), None);
        assert!(speaks(PROTOCOL) && speaks(MIN_PROTOCOL));
        assert!(!speaks(PROTOCOL + 1) && !speaks(0));
    }

    #[test]
    fn this_build_adopts_the_previous_release_keepers() {
        assert!(MIN_PROTOCOL <= PREVIOUS_RELEASE && PREVIOUS_RELEASE <= PROTOCOL);
        assert!(speaks(PREVIOUS_RELEASE));
    }

    /// Protocol 1 as the previous release writes it. These lines are frozen:
    /// a later build must still read them.
    #[test]
    fn protocol_1_lines_still_parse() {
        let welcome = r#"{"op":"welcome","version":1,"pid":42,"last_seq":3}"#;
        assert_eq!(
            serde_json::from_str::<FromKeeper>(welcome).unwrap(),
            FromKeeper::Welcome {
                version: 1,
                pid: 42,
                last_seq: 3,
                exited: false,
                acked: 0,
            }
        );
        for line in [
            r#"{"op":"frame","seq":1,"kind":"stdout","line":"x"}"#,
            r#"{"op":"frame","seq":2,"kind":"stdin","line":"y"}"#,
            r#"{"op":"frame","seq":3,"kind":"exit","code":0,"signal":null}"#,
            r#"{"op":"refused","reason":"wrong token"}"#,
        ] {
            assert_ne!(
                serde_json::from_str::<FromKeeper>(line).unwrap(),
                FromKeeper::Unknown,
                "{line}"
            );
        }
        for line in [
            r#"{"op":"hello","version":1,"token":"t"}"#,
            r#"{"op":"stdin","line":"x"}"#,
            r#"{"op":"closeStdin"}"#,
            r#"{"op":"ack","seq":1}"#,
            r#"{"op":"hold","seq":1}"#,
            r#"{"op":"release","seq":1}"#,
            r#"{"op":"signal","signal":"interrupt"}"#,
            r#"{"op":"finish"}"#,
        ] {
            assert_ne!(
                serde_json::from_str::<ToKeeper>(line).unwrap(),
                ToKeeper::Unknown,
                "{line}"
            );
        }
    }

    #[test]
    fn terminal_operations_round_trip() {
        assert_eq!(
            serde_json::from_str::<ToKeeper>(r#"{"op":"input","data":"bHMK"}"#).unwrap(),
            ToKeeper::Input {
                data: "bHMK".into()
            }
        );
        assert_eq!(
            serde_json::to_string(&ToKeeper::Resize { cols: 80, rows: 24 }).unwrap(),
            r#"{"op":"resize","cols":80,"rows":24}"#
        );
    }
}
