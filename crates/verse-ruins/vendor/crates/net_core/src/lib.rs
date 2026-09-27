//! `net_core`: snapshot schema + in-proc replication plumbing (scaffold)
//!
//! Scope
//! - Defines minimal snapshot encode/decode traits and stub messages
//! - Provides apply and interest stubs to be filled in Phase 3
//!
#![deny(warnings, clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod apply;
pub mod channel;
pub mod command;
pub mod frame;
pub mod interest;
pub mod snapshot;
pub mod transport;

#[cfg(test)]
mod tests {
    #[test]
    fn compiles_and_links() {
        // Trivial smoke test to ensure the crate participates in CI.
        assert_eq!(2 + 2, 4);
    }
}
