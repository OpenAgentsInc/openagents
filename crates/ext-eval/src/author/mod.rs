//! The authoring interview: one typed state machine that drafts a test
//! set with a person, step by step, with explicit approvals and the floor
//! enforced in code (`docs/extensions/evaluation.md`, *Authoring a suite*).
//!
//! Two drivers run it: `openagents ext eval init` in a terminal, where a
//! gate is `y`, and the chat's `eval.author` route
//! (`coder::eval_author`), where a gate is a tap on **Looks good** and the
//! draft (`openagents.eval-draft.v1`) lives on the phone. Both send the
//! model the same [`prompt`] and read its answer as a typed [`proposal`];
//! the [`machine`] decides what is kept.
//!
//! - [`stage`]: the steps 0 to 7 and the fixed line each gate ends with.
//! - [`catalog`]: the tool under test, and the catalog chat picks from.
//! - [`machine`]: events in, needs out; proposals in, turns out.
//! - [`floor`]: what every returned draft holds, whatever was proposed.
//! - [`render`]: typed proposals to the exact case files a runner reads.
//! - [`files`]: a draft's tests to an eval directory and back,
//!   byte-identical.
//! - [`runner`]: the try and the full run, as results the interview reads,
//!   behind a trait with a fake over the real engine.
//! - [`prompt`]: the specification's interview prompt and each step's task.
//!
//! The interview reads the tool and never writes into it: the only write
//! is [`files::write`] at the last step, to the eval directory the driver
//! names, and it never overwrites a test.

pub mod catalog;
pub mod files;
pub mod floor;
pub mod machine;
pub mod prompt;
pub mod proposal;
pub mod render;
pub mod runner;
pub mod stage;

pub use catalog::{Catalog, Source, Tool};
pub use machine::{Event, Interview, Need, Pick, Planned, Proposal, Refused, Turn};
pub use runner::{NoRunner, RunRequest, Runner, Tried};
pub use stage::{Stage, Surface};

#[cfg(test)]
mod tests;
