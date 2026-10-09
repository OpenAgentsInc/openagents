//! The inference gateway library (`docs/inference/gateway.md`).
//!
//! [`meter`] is the measurement half (sections 5 and 6): one record per
//! upstream attempt, live rates, and the credit ledger with burn-down
//! alerts. Adapters report each attempt through [`meter::Recorder`].

pub mod meter;
