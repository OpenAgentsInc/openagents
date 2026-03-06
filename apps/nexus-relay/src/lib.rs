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

//! Nexus relay crate entrypoint.
//!
//! The durable relay shell is the only supported runtime path. The older
//! in-memory relay harness was removed after production cutover so the crate no
//! longer exposes dead relay state or transitional proxy assumptions.

pub mod durable;
