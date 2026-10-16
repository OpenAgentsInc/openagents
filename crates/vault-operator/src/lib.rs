//! NIP-VAULT tier `operator` as an explicitly labeled custody profile.
//!
//! The operator can read these objects: anyone holding decrypt on the KMS
//! key opens them. What this service adds is control and evidence, not the
//! `user` or `sealed` guarantee:
//!
//! - an operator-signed grant names exact objects, one recipient, a
//!   purpose, an expiry within a day, and a use count;
//! - the recipient's signed use request is checked against the grant;
//! - every key use is preceded by a durable, hash-chained audit entry, and a
//!   use whose audit cannot be written does not happen;
//! - a request ID binds its exact bytes: a retry re-delivers or reconciles
//!   with another audited key use, a conflicting reuse refuses, and an
//!   unknown key-service outcome stays unknown until reconciled.
pub mod audit;
pub mod cli;
pub mod service;

pub type Result<T> = std::result::Result<T, String>;

/// Trusted host clock in Unix seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// The longest a grant may live.
pub const MAX_GRANT_SECONDS: u64 = 86_400;

#[cfg(test)]
mod tests;
