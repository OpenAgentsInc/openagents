//! ACP child agent harness for Coder.
//!
//! There is one harness, and it lives in `crate::acp` — the crate
//! Coder already takes its runtime from. This file was a second copy of
//! it, and the two had already drifted: the copy carried a character-boundary
//! fix the original did not, the original carried cancellation the copy did
//! not, and the Computer controller could use neither because a crate cannot
//! depend on the crate that depends on it. Both fixes now live in the one
//! implementation, and this is the name Coder reaches it by.
//!
//! What it does is unchanged. It spawns an ACP-compatible CLI agent over stdio
//! and streams JSON-RPC `session/update` events as they arrive: `initialize`,
//! `session/new` (or `session/load` for a resume), an optional
//! `session/set_mode`, then `session/prompt`. A `session/request_permission`
//! the agent sends back is answered without asking the reader, unless a caller
//! supplies a gate — the Computer controller does, so a delegated agent runs
//! under the machine's policy rather than around it.
//!
//! ## The child is stopped with its whole tree
//!
//! A coding agent shells out. Killing only the agent leaves its build, its
//! test run, or its `sleep` behind with nothing left to stop them, so the
//! child is spawned into a process group of its own and
//! [`crate::signals::stop_tree`] signals the group — `SIGTERM`, then
//! `SIGKILL` after a grace period, so an agent that writes a transcript on the
//! way out gets to write it. This used to be a bare `child.kill()`, which
//! stopped the agent and orphaned everything under it.

pub use crate::acp::{
    AcpEvent, AcpFailure, AcpHarness, AcpOutcome, PermissionGate, PermissionMode, PermissionQuery,
};

#[cfg(test)]
mod tests {
    /// A refusal carrying a multi-byte character across the 200-byte bound
    /// used to panic here, which killed the whole session rather than the one
    /// delegation. Same defect class as `28704f72ff`.
    #[test]
    fn a_long_refusal_with_a_multibyte_character_on_the_bound_does_not_panic() {
        // Sized so the multi-byte character straddles byte 200 of the
        // *encoded* JSON, not of the message: the envelope counts too, and a
        // fixture that guessed the offset proved nothing. Searched rather than
        // computed, so a change in how `serde_json` encodes cannot silently
        // turn this into a test of a string that was ASCII all along. The
        // length check matters as much as the boundary one — `is_char_boundary`
        // is also false for an index past the end.
        let text = (150..250)
            .map(|width| {
                let message = format!("{}\u{20ac}", "x".repeat(width));
                serde_json::to_string(&serde_json::json!({ "message": message })).unwrap()
            })
            .find(|text| text.len() > 200 && !text.is_char_boundary(200))
            .expect("no width made the character straddle byte 200");

        let end = crate::tracker::floor_char_boundary(&text, 200);
        let head = &text[..end]; // the old code was `&text[..200]`, which panics
        assert!(end < 200);
        assert!(head.len() == end);
    }
}
