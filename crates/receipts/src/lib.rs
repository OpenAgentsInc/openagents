//! Versioned receipts: the attributable claims a decision service emits.
//!
//! A receipt is what a call leaves behind that is not the answer — who
//! asked, under which authorization, what was requested versus what
//! answered, what happened, and when. It is the shared contract the HTTP
//! gateway and the relay lane both produce, so a caller can hold one
//! receipt shape regardless of which transport carried the call.
//!
//! Three rules govern the format:
//!
//! - **Digests, not content.** A receipt binds the request and the result
//!   by digest. Raw state, question text, and credentials never appear in
//!   one; operational metadata is for accounting and audit, not a side
//!   channel for the caller's data.
//! - **Outcomes are typed.** `answered`, `refused`, `unavailable`,
//!   `unattempted`, and `unknown` are different claims, and a receipt
//!   never smudges them — a door that never answered is not a door that
//!   answered wrong.
//! - **A receipt is a claim, not attestation.** It is attributable — the
//!   issuer's identity and the admission that authorized the call are in
//!   it — but it cannot prove which weights executed remotely. That
//!   distinction is the format's reason to exist.
//!
//! Receipt types are versioned and distinct: an execution receipt
//! ([`execution`]) describes a decision call, and the operational-feedback
//! and evaluation-commitment types that later issues define are their own
//! schemas, not fields grafted onto this one.

pub mod execution;
pub mod join;

pub use execution::{Evaluation, ExecutionReceipt, Outcome, ReceiptError, SCHEMA, Served, Timing};
