//! How each engine takes a steer, and what confirms that it did.
//!
//! Engines steer differently, and an accepted steer is not a consumed one.
//! NIP-SESS forbids treating a new turn, an enqueue, or a restart as native
//! steering unless the caller chose emulation explicitly. A [`Steering`]
//! description records, for one engine adapter:
//!
//! - [`Native`]: whether a running turn takes a message (`mid_turn`), the
//!   engine takes new input only when a turn ends (`turn_boundary`), or not
//!   at all (`unsupported`).
//! - [`Emulation`]: the emulated operation the host can run instead, which it
//!   runs only when the caller chose it.
//! - [`Acknowledgment`]: the evidence that says the engine consumed the
//!   steer, as opposed to the host accepting it.
//!
//! [`Steering::admit`] is the one decision every steering path takes. It
//! refuses native steering an engine lacks unless the caller chose the
//! emulated operation, and it never picks emulation on the caller's behalf.
//!
//! The rows below are the adapters in this workspace. `nips/openagents/NIP-SESS.md`
//! also records the known behavior of engines without an adapter here.

use serde::{Deserialize, Serialize};

/// What an engine does natively with a message for a running turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Native {
    /// A running turn takes the message.
    MidTurn,
    /// The engine takes new input only after a turn ends; a message for a
    /// running turn waits for, or becomes, the next turn.
    TurnBoundary,
    /// The engine takes no further input.
    Unsupported,
}

/// An emulated steering operation a host can run when the caller chose it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emulation {
    /// Stop the running turn, then start a new turn that carries the message.
    CancelAndContinue,
}

/// The evidence that an engine consumed a steer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Acknowledgment {
    /// The engine replays the message in its own output stream, as Claude
    /// Code does with `--replay-user-messages`.
    EngineReplay,
    /// The engine answers the steer request for the exact expected turn, as
    /// Codex app-server's `turn/steer` does.
    TurnAnswer,
    /// The next turn starts with the message: its admission records the
    /// revision it read.
    NextTurnStart,
    /// The message was written to the engine's input. That is acceptance;
    /// consumption is not confirmed.
    InputWritten,
    /// Nothing confirms a steer.
    None,
}

impl Acknowledgment {
    /// Whether this evidence confirms consumption rather than acceptance.
    #[must_use]
    pub fn confirms_consumption(self) -> bool {
        matches!(
            self,
            Self::EngineReplay | Self::TurnAnswer | Self::NextTurnStart
        )
    }
}

/// One engine adapter's steering capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Steering {
    /// The adapter's name.
    pub adapter: &'static str,
    pub native: Native,
    /// The emulated operation, if the adapter has one.
    pub emulation: Option<Emulation>,
    pub acknowledgment: Acknowledgment,
    /// Bounded, inert notes on the adapter's actual behavior.
    pub limitations: &'static [&'static str],
}

/// The steering operation a caller asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    /// Native steering of the running turn.
    Native,
    /// The adapter's emulated operation, chosen explicitly.
    Emulated,
}

/// What the host does with an admitted steer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    /// Deliver the message to the running turn.
    Deliver,
    /// Stop the running turn, then start a new turn with the message.
    CancelAndContinue,
    /// The targeted turn already ended before the steer reached it: the
    /// steer is re-dispatched as a new turn, as the routed-steer ledger
    /// requires of a steer nothing consumed.
    NewTurn,
}

/// Why a steer was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// The engine cannot steer a running turn natively, and the caller did
    /// not choose emulation.
    Unsupported,
    /// The caller chose emulation, and the adapter has none.
    NoEmulation,
}

/// The state of the turn a steer names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// The turn is running.
    Running,
    /// The turn ended before the steer reached it.
    Ended,
}

impl Steering {
    /// Decide what a steer for a turn in state `turn` does under `request`.
    ///
    /// # Errors
    /// Returns [`Refusal::Unsupported`] for native steering the engine
    /// lacks or an engine that takes no further input, and [`Refusal::NoEmulation`] for emulation the adapter lacks.
    pub fn admit(&self, turn: Turn, request: Request) -> Result<Plan, Refusal> {
        match (turn, request) {
            (Turn::Ended, _) if self.native == Native::Unsupported => Err(Refusal::Unsupported),
            (Turn::Ended, _) => Ok(Plan::NewTurn),
            (Turn::Running, Request::Native) => match self.native {
                Native::MidTurn => Ok(Plan::Deliver),
                Native::TurnBoundary | Native::Unsupported => Err(Refusal::Unsupported),
            },
            (Turn::Running, Request::Emulated) => match self.emulation {
                Some(Emulation::CancelAndContinue) => Ok(Plan::CancelAndContinue),
                None => Err(Refusal::NoEmulation),
            },
        }
    }
}

/// Claude Code driven through `claude -p --input-format stream-json`.
pub const CLAUDE_CODE: Steering = Steering {
    adapter: "claude-code",
    native: Native::MidTurn,
    emulation: Some(Emulation::CancelAndContinue),
    acknowledgment: Acknowledgment::InputWritten,
    limitations: &[
        "A steer is a stream-json user line written to the running process.",
        "The adapter does not pass --replay-user-messages, so consumption is not confirmed.",
        "Emulation stops the process group and resumes the session with --resume.",
    ],
};

/// Codex driven through `codex exec`.
pub const CODEX_EXEC: Steering = Steering {
    adapter: "codex",
    native: Native::TurnBoundary,
    emulation: Some(Emulation::CancelAndContinue),
    acknowledgment: Acknowledgment::NextTurnStart,
    limitations: &[
        "codex exec reads one prompt and closes its input; a running turn takes no message.",
        "A new turn is codex exec resume with the message.",
        "Emulation stops the process group and resumes the thread.",
    ],
};

/// The local Devin CLI driven over ACP (`devin acp`), as a repository
/// run's engine. ACP takes one prompt at a time per session: a message for
/// a running turn is not delivered into it. `session/cancel` ends the turn
/// as `cancelled`, and the next turn reattaches to the same Devin session
/// with `session/load` and prompts it with the message, so the steer is
/// consumed when that turn starts.
pub const DEVIN_ACP: Steering = Steering {
    adapter: "devin-acp",
    native: Native::TurnBoundary,
    emulation: Some(Emulation::CancelAndContinue),
    acknowledgment: Acknowledgment::NextTurnStart,
    limitations: &[
        "ACP v1 takes one session/prompt at a time; a running turn takes no message.",
        "A new turn reattaches the same Devin session with session/load and prompts it.",
        "Emulation sends session/cancel, stops the process group, and continues in the next turn.",
    ],
};

/// OpenCode driven through `opencode run --format json`.
pub const OPENCODE_RUN: Steering = Steering {
    adapter: "opencode",
    native: Native::TurnBoundary,
    emulation: Some(Emulation::CancelAndContinue),
    acknowledgment: Acknowledgment::NextTurnStart,
    limitations: &[
        "opencode run reads its whole message from its input before the turn starts; a running turn takes no message.",
        "A new turn is opencode run --session with the message.",
        "Emulation stops the process group and continues the session with --session.",
    ],
};

/// Microluna in process.
pub const MICROLUNA: Steering = Steering {
    adapter: "microluna",
    native: Native::Unsupported,
    emulation: None,
    acknowledgment: Acknowledgment::None,
    limitations: &[
        "Each short session runs to its own bounds; the host does not stop, resume, or steer it.",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_steering_an_engine_lacks_is_refused_unless_emulation_was_chosen() {
        assert_eq!(
            CODEX_EXEC.admit(Turn::Running, Request::Native),
            Err(Refusal::Unsupported)
        );
        assert_eq!(
            CODEX_EXEC.admit(Turn::Running, Request::Emulated),
            Ok(Plan::CancelAndContinue)
        );
        assert_eq!(
            DEVIN_ACP.admit(Turn::Running, Request::Native),
            Err(Refusal::Unsupported)
        );
        assert_eq!(
            DEVIN_ACP.admit(Turn::Running, Request::Emulated),
            Ok(Plan::CancelAndContinue)
        );
        assert!(DEVIN_ACP.acknowledgment.confirms_consumption());
        assert_eq!(
            MICROLUNA.admit(Turn::Running, Request::Native),
            Err(Refusal::Unsupported)
        );
        assert_eq!(
            MICROLUNA.admit(Turn::Running, Request::Emulated),
            Err(Refusal::NoEmulation)
        );
        // Native steering is never replaced by emulation the caller did
        // not choose, and emulation is never replaced by native delivery.
        assert_eq!(
            CLAUDE_CODE.admit(Turn::Running, Request::Native),
            Ok(Plan::Deliver)
        );
        assert_eq!(
            CLAUDE_CODE.admit(Turn::Running, Request::Emulated),
            Ok(Plan::CancelAndContinue)
        );
    }

    #[test]
    fn a_steer_for_an_ended_turn_becomes_a_new_turn() {
        for steering in [CLAUDE_CODE, CODEX_EXEC, DEVIN_ACP] {
            for request in [Request::Native, Request::Emulated] {
                assert_eq!(steering.admit(Turn::Ended, request), Ok(Plan::NewTurn));
            }
        }
        // An engine that takes no further input starts no new turn either.
        assert_eq!(
            MICROLUNA.admit(Turn::Ended, Request::Native),
            Err(Refusal::Unsupported)
        );
    }

    #[test]
    fn only_engine_evidence_confirms_consumption() {
        assert!(Acknowledgment::EngineReplay.confirms_consumption());
        assert!(Acknowledgment::TurnAnswer.confirms_consumption());
        assert!(Acknowledgment::NextTurnStart.confirms_consumption());
        assert!(!Acknowledgment::InputWritten.confirms_consumption());
        assert!(!Acknowledgment::None.confirms_consumption());
        assert!(!CLAUDE_CODE.acknowledgment.confirms_consumption());
    }
}
