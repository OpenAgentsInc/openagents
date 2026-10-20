//! The Coder runtime compilation unit.

pub mod about;
pub mod activity;
pub mod agent;
pub mod argv;
pub mod branch_checkout;
pub mod builtin_plugins;
pub mod capability;
pub mod child;
pub mod classify;
pub mod cloud;
pub mod decision;
pub mod defaults;
pub mod delegate;
pub mod delegate_door;
pub mod doctor;
pub mod evidence;
pub mod execution;
pub mod executor_door;
pub mod first;
pub mod generate;
pub mod hooks;
pub mod identity;
pub mod local_chain;
pub mod package;
pub mod permit;
pub mod plain_path;
pub mod profiles;
pub mod program;
pub mod program_authority;
pub mod questions;
pub mod reattach;
pub mod reconcile;
pub mod relay;
pub mod repo;
pub mod resolve;
pub mod review;
pub mod runstate;
pub mod runtime;
pub mod select;
pub mod shell;
pub mod sites;
pub mod source;
pub mod spend;
pub mod store;
pub mod survey;
pub mod trace;
pub mod tracker;
pub mod turn;
pub mod verification;
pub mod waves;
pub mod workflow_template;
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

pub mod capacity;
