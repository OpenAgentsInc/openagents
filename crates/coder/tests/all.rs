//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

mod chat_goldens;
mod conformance;
mod delegation;
mod desktop_local_coder;
mod dispatch_ack_eval;
mod eval_author_live;
mod evidence_select;
mod fanout_eval;
mod gateway_stream;
mod headless;
mod host_cli;
mod host_serve;
mod interop_processes;
mod interoperability;
mod issue_pick_eval;
mod phone_engine;
mod plugin_catalog;
mod program_run;
mod relay_binding;
mod relay_delegation;
mod relay_lifecycle;
mod route_map_sources;
mod router_eval;
mod router_suite;
mod runstate_recovery;
mod shell_verdicts;
mod state_caps;
mod suite_questions;
mod task_inbox;
mod task_owner;
mod trace;
mod wallet_eval;
mod worker_lifecycle;
