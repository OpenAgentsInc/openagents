//! Coder One: a minimal agent that turns a GitHub issue into a pull
//! request.
//!
//! The loop is the whole design. Each step asks Jev for typed judgments
//! over the current state, puts those judgments into the prompt, asks a
//! generator for one action, and runs that action:
//!
//! ```text
//! loop until the action is `finished` or a bound is hit:
//!     judgments = judge(state)
//!     action    = generate(state + prompt + judgments)
//!     observe(shell(action))
//! ```
//!
//! Code owns the loop, the bounds, and what an action may do. Judgments
//! change what the generator sees; they never run or authorize anything.
//! Issue #9531 describes the design and the evaluation. Delegate mode
//! (issue #9532, [`delegate`]) lets the loop explore first and then hands
//! the task to Claude Code with a briefing code builds from the evidence.

pub mod action;
pub mod agent;
pub mod credentials;
pub mod delegate;
pub mod episode;
pub mod generate;
pub mod judge;
pub mod record;
pub mod shell;
pub mod state;

pub use action::Action;
pub use agent::{Bounds, Ended, Generate, Judge, Judgments, Shell, run};
pub use state::{Environment, Issue, Observation, State, Turn};
