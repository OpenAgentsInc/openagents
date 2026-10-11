//! Durable coordination with synchronous Rust handlers and PostgreSQL transactions.
//!
//! The `server` feature (default) adds the PostgreSQL store, the runtime, and
//! the HTTP routes; `net` adds the network client a remote executor uses.
//! Without features the crate is the actor definitions and wire types.
pub mod types;
pub use types::*;
pub mod core;
pub mod example;
pub use core::{Actor, Ctx, Definition, Handles, Message, Registry};
#[cfg(feature = "server")]
pub mod pool;
#[cfg(feature = "server")]
pub use pool::Pool;
#[cfg(feature = "server")]
pub mod http;
#[cfg(feature = "server")]
pub mod postgres;
#[cfg(feature = "server")]
mod queue_store;
#[cfg(feature = "server")]
pub mod runtime;
#[cfg(feature = "server")]
pub use postgres::PgStore;
#[cfg(feature = "server")]
pub use runtime::{EffectExecutor, Effects, Runtime, RuntimeConfig, RuntimeHandle};
#[cfg(feature = "server")]
pub mod client;
#[cfg(feature = "server")]
pub use client::{ActorRef, CallOptions, Client, TypedReply};
#[cfg(feature = "net")]
pub mod net;
