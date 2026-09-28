//! The Microcoder loop.
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
//! [`run::run`] is the loop. [`models`] holds the two calls a step makes: Jev
//! through `crates/jev`, and one structured model call, GPT-6 Luna on the
//! operator's Codex login by default, or Claude through the `claude` binary
//! ([`claude`]), Vertex ([`vertex`]), OpenRouter, or the OpenAgents door
//! ([`door`]). [`env`] is where commands run. [`capacity`] is the
//! provider capacity book and [`failover`] the switch to the next provider
//! with capacity when one refuses for a usage or rate limit.
//!
//! This crate depends on nothing from `crates/coder`, so both Coder's
//! delegate door (`coder::delegate_door`) and the `microcoder` binary
//! (`crates/microcoder`, which adds the task owner's repository adapter,
//! Terminal-Bench, and the knowledge network) run the same loop. Issues
//! #9666 to #9669 hold the design, and #9879 the split.

pub mod capacity;
pub mod claude;
pub mod door;
pub mod env;
pub mod failover;
pub mod gate;
pub mod models;
pub mod run;
pub mod state;
pub mod usage;
pub mod vertex;

/// The default model, reached through the operator's Codex login.
pub const MODEL: &str = "gpt-6-luna";

/// The stronger model that writes the acceptance tests when Jev judges a
/// task hard. Routing to it is off by default (`Route::Never`).
pub const STRONG_MODEL: &str = "gpt-6-sol";

#[cfg(test)]
mod tests;
