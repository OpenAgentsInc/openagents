//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

#[path = "support/chat_worker.rs"]
mod chat_worker;
#[path = "support/connect.rs"]
mod connect;
#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

#[cfg(feature = "host")]
mod adopt;
#[cfg(feature = "host")]
mod cj;
#[cfg(feature = "host")]
mod control;
mod control_tasks;
#[cfg(feature = "host")]
mod end_to_end;
#[cfg(feature = "host")]
mod headless;
#[cfg(feature = "host")]
mod idle;
#[cfg(feature = "host")]
mod iroh;
#[cfg(feature = "host")]
mod nearby;
#[cfg(feature = "host")]
mod robot_lab;
#[cfg(feature = "host")]
mod serve;
mod task_reads;
#[cfg(feature = "host")]
mod threads;
#[cfg(feature = "host")]
mod websocket;
mod world_listing;
#[cfg(feature = "host")]
mod wss;
