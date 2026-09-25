//! Microluna: short GPT-6 Luna sessions on a logged-in Codex session.
//!
//! The Luna pivot (`docs/coder/design/luna-pivot.md`) runs a task as many
//! short sessions, each with a context that code and Jev rebuild from
//! scratch. This crate is the harness for one such session:
//!
//! - [`transport`] is the one seam to the model. [`codex::CodexTransport`]
//!   calls the ChatGPT Codex Responses endpoint on the operator's Codex
//!   login, and [`fake::FakeTransport`] answers from a script in tests.
//! - [`tools`] declares five native function tools — run a command, read a
//!   file region, apply a patch, write a file, and finish — and runs them
//!   inside one workspace. Commands run under `coder-boundary` and
//!   `supervise`.
//! - [`patch`] is the apply-patch format the model is trained on.
//! - [`session`] builds the input with the stable prefix first, runs the
//!   tool loop, and records every step as ATIF with usage and cost.
//! - [`finish`] holds a `done` finish until the score and a baseline
//!   command ran after the last edit.
//! - [`price`] turns usage into dollars at Luna's list prices.
//!
//! `docs/coder/design/microluna.md` records why Microluna calls the
//! endpoint directly instead of driving `codex app-server`, and what it
//! takes from Codex.

pub mod codex;
pub mod fake;
pub mod finish;
pub mod patch;
pub mod price;
pub mod seal;
pub mod session;
pub mod tools;
pub mod transport;

pub use finish::FinishRule;
pub use seal::Seal;
pub use session::{
    Brief, Config, Ending, Evidence, Intervention, NoWatch, Persist, Recorder, Report, Watch, run,
    run_watched,
};
pub use tools::{Cause, Finish, FinishStatus, Isolation, Workspace};
pub use transport::{Reply, Request, TokenUsage, Transport, TransportError};
