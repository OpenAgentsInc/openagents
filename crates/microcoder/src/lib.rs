//! Microcoder: the simple coding loop.
//!
//! ```text
//! state = { environment, task }
//! while next_action isn't finished:
//!     jev_results = jev(state, user_prompt)
//!     prompt      = state + user_prompt + jev_results
//!     next_action = generate(prompt)        # one OpenRouter call, structured
//!     run next_action's commands
//! ```
//!
//! [`run::run`] is the loop. [`models`] holds the two calls a step makes: Jev
//! through `crates/jev`, and one structured OpenRouter call through
//! `crates/openrouter`, GPT-6 Luna by default. [`env`] is where commands run.
//! [`tbench`] runs the loop on a Terminal-Bench 4 task, and [`show`] streams a
//! run to the terminal. Issues #9666 to #9669 hold the design.
//!
//! With the knowledge base on (`crates/knowledge`, issue #9670), each step's
//! state also holds the entries Jev judges relevant, and the model can ask
//! for an entry's full body with `expand`. [`kbnet`] publishes entries to
//! a Nostr relay and syncs other authors' entries from one (NIP-KB), and
//! [`xpnet`] publishes quests and awards and derives the XP ledger (NIP-XP).

pub mod door;
pub mod env;
pub mod gate;
pub mod kbnet;
pub mod models;
pub mod run;
pub mod show;
pub mod state;
pub mod tbench;
pub mod xpnet;

/// The default model, reached through the operator's Codex login.
pub const MODEL: &str = "gpt-6-luna";

/// The stronger model that writes the acceptance tests when Jev judges a
/// task hard.
pub const STRONG_MODEL: &str = "gpt-6-sol";

#[cfg(test)]
mod tests;
