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
//! - [`delegate`] hands one bounded task to one executor and runs six of
//!   them at once under a stated bound. It is not a tool the model may
//!   elect; the call site is the operator's sentence.
//! - [`agent`] holds the conversation: the transcript, the state Classify
//!   reads, and the routing table that turns answers into the next step.
//! - [`permit`] is what the host permits one turn to do. A reply is the
//!   model's; whether anything in it runs is not, and a permit is the
//!   host's answer, built before the turn generates a word.
//! - [`trace`] writes the conversation down: every turn, every command, and
//!   every decision call, appended to an ATIF session log on local disk as
//!   it happens.
//! - [`turn`] is one turn of that conversation, start to finish. The
//!   terminal and `coder --print` both call it, so neither can drift from
//!   the other.
//! - [`capability`] is the shared contract for what this machine can hand
//!   work to: an inert registry, a host-owned approval before any probe
//!   runs, and a typed answer — present, absent, unavailable, unprobed,
//!   or unknown.
//! - [`program`] reads the programs a run can take, each a state machine of
//!   named steps with per-step bounds.
//! - [`program_authority`] is the operator's answer to whether a selected
//!   program may run at all: which programs a session granted, and the
//!   effects a run under the grant may have. A selection is a proposal,
//!   and the grant is the authority it is proposed under.
//! - [`questions`] holds the wording a `decide` step names and must not
//!   carry, addressed by identifier and digested as a whole.
//! - [`source`] holds the work a `query` step names and must not carry:
//!   where a lookup's answer comes from, the order it is in, and what the
//!   lookup already knows about which items collide.
//! - [`survey`] is the two of them together: what a host knows about itself
//!   at the moment it starts choosing.
//! - [`runtime`] runs a program's steps from the program: the bounds it
//!   can enforce, the ones it refuses, and the trace of what happened.

pub mod about;
pub mod agent;
pub mod capability;
pub mod classify;
pub mod decision;
pub mod delegate;
pub mod executor_door;
pub mod generate;
pub mod permit;
pub mod program;
pub mod program_authority;
pub mod questions;
pub mod relay;
pub mod repo;
pub mod runtime;
pub mod shell;
pub mod source;
pub mod survey;
pub mod trace;
pub mod tracker;
pub mod turn;
pub mod verification;
mod worktree;

pub use crate::capability::{Found, Manifest, Presence};
pub use about::About;
pub use agent::{Agent, Classified, Ending, Exhausted, REPAIRS_MAX, Turned, Verdict};
pub use classify::{Action, Judgment, Route, route, state_of};
pub use delegate::{
    Bounds, Delegation, Delegator, EnforcedBoundary, Executor, Isolation, Policy, Task,
};
pub use generate::{
    Door, Generate, GenerateError, Message, Meta, ResponsesDoor, Role, StubGenerate, Usage,
};
pub use permit::Permit;
pub use program::Program;
pub use program_authority::{Effects, Grant, Programs};
pub use relay::{Identity, RelayDoor};
pub use repo::Repo;
pub use runtime::{Enforcement, Host, Inputs, Refused, Run, Runtime, Selected};
pub use shell::{NotAPlan, Outcome, Plan, Proposal, Reply, ShellEvent, Status};
pub use source::{Selection, Work};
pub use survey::Survey;
pub use trace::Recorder;
pub use turn::{Completion, Event, Failure, Finished};
