//! Measurement and the credit ledger (`docs/inference/gateway.md`,
//! sections 5 and 6).
//!
//! Adapters and the router report each upstream attempt as an
//! [`Attempt`] through a [`Recorder`].

mod attempt;

pub use attempt::{Api, Attempt, Collect, ErrorClass, NoRecorder, Outcome, Recorder, Tokens};
