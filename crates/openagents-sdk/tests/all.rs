//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

mod attention;
mod auth;
mod chats;
mod computers;
mod computers_contract;
mod continuations;
mod copy;
mod dashboard;
mod history;
mod labs;
mod labs_worker;
mod land;
mod payment_required;
mod run_commands;
mod runs;
mod schedules;
mod sessions;
mod voice_agent;
mod work;
