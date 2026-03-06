#![cfg_attr(
    test,
    allow(
        clippy::all,
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::pedantic,
        clippy::unwrap_used
    )
)]

//! Spacetime sync schema and reducer primitives for OpenAgents.

pub mod auth;
pub mod client;
pub mod live;
pub mod mapping;
pub mod presence;
pub mod reducers;
pub mod schema;
pub mod subscriptions;
