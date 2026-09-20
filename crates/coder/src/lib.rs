//! The Coder agent: two model calls behind one conversation.
//!
//! - [`classify`] is System One: one structured state object and a map of
//!   typed questions go to Jev, and typed answers — a `Choice` with
//!   probabilities, `Noul`s, `Score`s — come back. The loop asks it on every
//!   turn because it is fast, cheap, and cannot fabricate.
//! - [`generate`] is System Two: the [`Generate`] trait and a client that
//!   speaks Open Responses to a model door. The trait is the public
//!   contract; which door and which credentials back it is configuration,
//!   not code.
//! - [`agent`] holds the conversation: the transcript, the state Classify
//!   reads, and the routing table that turns answers into the next step.
//! - [`trace`] writes the conversation down: every turn, every command, and
//!   every decision call, appended to an ATIF session log on local disk as
//!   it happens.

pub mod agent;
pub mod classify;
pub mod generate;
pub mod relay;
pub mod repo;
pub mod shell;
pub mod trace;

pub use agent::{Agent, Classified, Verdict};
pub use classify::{Action, Judgment, Route, route, state_of};
pub use generate::{
    Door, Generate, GenerateError, Message, Meta, ResponsesDoor, Role, StubGenerate, Usage,
};
pub use relay::{Identity, RelayDoor};
pub use repo::Repo;
pub use shell::{Outcome, Proposal, ShellEvent, Status};
pub use trace::Recorder;
