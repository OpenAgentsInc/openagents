//! Where a turn runs when the chat is off the computer (#11456, #11451
//! owner decision 5): placement is a candidate filter, never a question.
//!
//! On the phone or the web, the person's linked computer decides where a
//! piece of work can go. While it is online, the harnesses that run on a
//! computer (Claude Code, Codex) stay candidates there, reached as a task
//! on it (NIP-HOST), beside Coder's own loop. While it is offline, still
//! connecting, or there is none, the turn runs on the OpenAgents gateway
//! (the Coder loop) in the same chat, with no error and no question, and
//! the [`Placement`] says why so a routing record can keep the reason.
//!
//! A chat whose work was on the computer goes on from its transcript on
//! the gateway, and its next piece of work goes back to the computer once
//! it is online again: each turn is placed afresh from the presence the
//! client sees then.

use crate::harness::{DECLARATIONS, Harness, Place};
use serde::{Deserialize, Serialize};

/// The linked computer's presence, as the client last saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Presence {
    /// It answers, and this device may run work on it.
    Online,
    /// The client is still reaching it, as just after launch.
    Connecting,
    /// It is not reachable, or too old to take work.
    Offline,
}

/// The person's linked computer: its presence, and the harnesses its
/// presence says are signed in, enabled, and under their limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linked {
    pub presence: Presence,
    pub ready: Vec<Harness>,
}

/// Why a turn runs on the gateway rather than on the person's computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Because {
    /// The person has no linked computer.
    NoComputer,
    /// Their computer is still connecting.
    ComputerConnecting,
    /// Their computer is offline.
    ComputerOffline,
}

/// Where one turn runs, the harnesses that may take it there in the order
/// they are preferred, and, for the gateway, why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub place: Place,
    pub candidates: Vec<Harness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub because: Option<Because>,
}

impl Placement {
    /// The placement a request's `context.placement` carries, or `None`
    /// when it is missing or names a word this build does not know.
    #[must_use]
    pub fn of(value: &serde_json::Value) -> Option<Self> {
        let placement: Self = serde_json::from_value(value.clone()).ok()?;
        (placement.candidates.len() <= DECLARATIONS.len()).then_some(placement)
    }

    /// The placement as a request's `context.placement`.
    #[must_use]
    pub fn json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// The turn runs on the gateway because the person's computer is
    /// away: offline or still connecting, not merely missing.
    #[must_use]
    pub fn computer_away(&self) -> bool {
        self.place == Place::Gateway
            && matches!(
                self.because,
                Some(Because::ComputerOffline | Because::ComputerConnecting)
            )
    }
}

/// Where a turn from a chat off the computer runs, given the person's
/// linked computer.
#[must_use]
pub fn place(linked: Option<&Linked>) -> Placement {
    let because = match linked.map(|linked| linked.presence) {
        Some(Presence::Online) => None,
        Some(Presence::Connecting) => Some(Because::ComputerConnecting),
        Some(Presence::Offline) => Some(Because::ComputerOffline),
        None => Some(Because::NoComputer),
    };
    match (linked, because) {
        (Some(linked), None) => {
            // The computer's own harnesses first, in the declared order,
            // then Coder's loop, which every computer runs.
            let mut candidates: Vec<Harness> = DECLARATIONS
                .iter()
                .filter(|declaration| declaration.place == Place::ThisComputer)
                .map(|declaration| declaration.harness)
                .filter(|harness| linked.ready.contains(harness))
                .collect();
            candidates.push(Harness::CoderLoop);
            Placement {
                place: Place::LinkedComputer,
                candidates,
                because: None,
            }
        }
        (_, because) => Placement {
            place: Place::Gateway,
            candidates: vec![Harness::CoderLoop],
            because,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linked(presence: Presence) -> Linked {
        Linked {
            presence,
            ready: vec![Harness::Codex, Harness::ClaudeCode],
        }
    }

    #[test]
    fn an_online_computer_keeps_its_harnesses_as_candidates() {
        let placement = place(Some(&linked(Presence::Online)));
        assert_eq!(placement.place, Place::LinkedComputer);
        assert_eq!(
            placement.candidates,
            vec![Harness::ClaudeCode, Harness::Codex, Harness::CoderLoop]
        );
        assert_eq!(placement.because, None);
        assert!(!placement.computer_away());
    }

    #[test]
    fn an_online_computer_with_no_ready_agent_still_runs_coder() {
        let placement = place(Some(&Linked {
            presence: Presence::Online,
            ready: vec![],
        }));
        assert_eq!(placement.place, Place::LinkedComputer);
        assert_eq!(placement.candidates, vec![Harness::CoderLoop]);
    }

    #[test]
    fn an_offline_computer_falls_back_to_the_gateway_with_its_reason() {
        let placement = place(Some(&linked(Presence::Offline)));
        assert_eq!(placement.place, Place::Gateway);
        assert_eq!(placement.candidates, vec![Harness::CoderLoop]);
        assert_eq!(placement.because, Some(Because::ComputerOffline));
        assert!(placement.computer_away());

        let connecting = place(Some(&linked(Presence::Connecting)));
        assert_eq!(connecting.place, Place::Gateway);
        assert_eq!(connecting.because, Some(Because::ComputerConnecting));
        assert!(connecting.computer_away());
    }

    #[test]
    fn no_computer_runs_on_the_gateway() {
        let placement = place(None);
        assert_eq!(placement.place, Place::Gateway);
        assert_eq!(placement.because, Some(Because::NoComputer));
        assert!(!placement.computer_away());
    }

    #[test]
    fn the_wire_form_round_trips_and_refuses_unknown_words() {
        let placement = place(Some(&linked(Presence::Offline)));
        let json = placement.json();
        assert_eq!(
            json,
            serde_json::json!({
                "place": "gateway",
                "candidates": ["coder-loop"],
                "because": "computer-offline",
            })
        );
        assert_eq!(Placement::of(&json), Some(placement));
        assert_eq!(
            Placement::of(&serde_json::json!({"place": "moon", "candidates": []})),
            None
        );
        assert_eq!(Placement::of(&serde_json::Value::Null), None);
    }
}
