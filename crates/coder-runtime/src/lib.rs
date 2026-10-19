//! Coder's program runtime, conversation loop, and execution entry points.

use coder_core::*;

pub mod agent;
pub mod execution;
pub mod runtime;
pub mod sites;
pub mod turn;
pub mod waves;
pub mod workflow_template;

pub use agent::{Agent, Classified, Ending, Exhausted, REPAIRS_MAX, Turned, Verdict};
pub use runtime::{Enforcement, Host, Inputs, Refused, Run, Runtime, Selected};
pub use turn::{Completion, Event, Failure, Finished};

#[cfg(test)]
mod delegate_turn_tests;
#[cfg(test)]
mod verification_tests;
