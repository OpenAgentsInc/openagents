//! Job tracking for buyer mode
//!
//! Stores submitted NIP-90 jobs in SQLite for tracking status and results.

mod store;

pub use store::{JobRecord, JobStore};
