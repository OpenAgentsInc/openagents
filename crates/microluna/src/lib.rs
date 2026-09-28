//! Microluna: short GPT-6 Luna sessions on a logged-in Codex session.
//!
//! **Deprecated.** Microcoder (`crates/microcoder-loop`) replaced Microluna
//! as Coder's loop on 2026-09-25 (issue #9666), and the owner deprecated
//! Microluna on 2026-09-28 (issues #9878 and #9880). Coder's terminal and
//! `coder -p` answer through Microcoder, and the Codex login and transport
//! moved to `crates/codex-transport`. This crate stays in the workspace
//! only so the recorded Terminal-Bench evidence stays reproducible: Coder
//! One's mini-handoff loop (`coder_one::micro`) and its `microluna-*`
//! policies run these sessions, and so does the `microluna` binary. Don't
//! build new work on it.
//!
//! The Luna pivot (`docs/coder/design/luna-pivot.md`) runs a task as many
//! short sessions, each with a context that code and Jev rebuild from
//! scratch. This crate is the harness for one such session:
//!
//! - [`transport`] is the one seam to the model. [`codex::CodexTransport`]
//!   calls the ChatGPT Codex Responses endpoint on the operator's Codex
//!   login, and [`fake::FakeTransport`] answers from a script in tests.
//!   Both are `crates/codex-transport`'s, re-exported here.
//! - [`tools`] declares five native function tools — run a command, read a
//!   file region, apply a patch, write a file, and finish — and runs them
//!   inside one workspace. Commands run under `coder-boundary` and
//!   `supervise`.
//! - [`patch`] is the apply-patch format the model is trained on.
//! - [`session`] builds the input with the stable prefix first, runs the
//!   tool loop, and records every step as ATIF with usage and cost.
//! - [`finish`] holds a `done` finish until the score and a baseline
//!   command ran after the last edit.
//!
//! `docs/coder/design/microluna.md` records why Microluna calls the
//! endpoint directly instead of driving `codex app-server`, and what it
//! takes from Codex.

pub use codex_transport::{codex, fake, oneshot, price, transport};

pub mod finish;
pub mod openrouter;
pub mod patch;
pub mod remote;
pub use coder_delegate::seal;
pub mod session;
pub mod tools;

pub use finish::FinishRule;
pub use remote::Remote;
pub use seal::Seal;
pub use session::{
    Brief, Config, Ending, Evidence, Intervention, NoWatch, Persist, Recorder, Report, Watch, run,
    run_watched,
};
pub use tools::{Cause, Finish, FinishStatus, Isolation, Workspace};
pub use transport::{Reply, Request, TokenUsage, Transport, TransportError};
