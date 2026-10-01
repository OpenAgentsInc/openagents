//! Microcoder: the simple coding loop.
//!
//! ```text
//! state = { environment, task }
//! while next_action isn't finished:
//!     jev_results = jev(state, user_prompt)
//!     prompt      = state + user_prompt + jev_results
//!     next_action = generate(prompt)        # one structured model call
//!     run next_action's commands
//! ```
//!
//! The loop itself lives in `crates/microcoder-loop`, which depends on
//! nothing from `crates/coder`, so Coder's delegate door runs it too. This
//! crate re-exports its modules under their old paths ([`run`], [`models`],
//! [`env`], and the rest) and adds what needs `crates/coder`: the task
//! owner's [`repository`] adapter, [`tbench`] for Terminal-Bench 4 tasks,
//! [`show`] to stream a run to the terminal, and the knowledge network.
//! Issues #9666 to #9669 hold the design.
//!
//! With the knowledge base on (`crates/knowledge`, issue #9670), each step's
//! state also holds the entries Jev judges relevant, and the model can ask
//! for an entry's full body with `expand`. [`kbnet`] publishes entries to
//! a Nostr relay and syncs other authors' entries from one (NIP-KB), and
//! [`xpnet`] publishes quests and awards and derives the XP ledger (NIP-XP).

pub use microcoder_loop::{
    MODEL, STRONG_MODEL, capacity, claude, door, env, failover, gate, images, models, run, state,
    vertex,
};

pub mod kbinput;
pub mod kbnet;
pub mod kbstudy;
pub mod repository;
pub mod show;
pub mod tbench;
pub mod xpnet;

/// How Microcoder takes a steer: at the next turn boundary, confirmed when
/// that turn's admission records the revision it read. The task owner's
/// repository adapter reports this statement in every admission.
pub use coder::task::adapter::STEERING;

#[cfg(test)]
mod tests {
    #[test]
    fn microcoder_states_that_it_steers_only_at_a_turn_boundary() {
        use coder::task::steering::{
            Acknowledgment, Emulation, Native, Plan, Refusal, Request, Turn,
        };
        assert_eq!(crate::STEERING.adapter, coder::task::adapter::NAME);
        assert_eq!(crate::STEERING.native, Native::TurnBoundary);
        assert_eq!(
            crate::STEERING.acknowledgment,
            Acknowledgment::NextTurnStart
        );
        assert_eq!(
            crate::STEERING.admit(Turn::Running, Request::Native),
            Err(Refusal::Unsupported)
        );
        // Stopping the turn and continuing with the message runs only when
        // the caller chose it.
        assert_eq!(
            crate::STEERING.emulation,
            Some(Emulation::CancelAndContinue)
        );
        assert_eq!(
            crate::STEERING.admit(Turn::Running, Request::Emulated),
            Ok(Plan::CancelAndContinue)
        );
    }
}
