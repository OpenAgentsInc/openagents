//! NIP-SESS managed sessions over admitted engine adapters.
//!
//! The retained-history observer stays in `coder-connect`; this crate admits
//! managed sessions separately. A session, an ENV execution attachment, a
//! RUN controller, and a commercial order keep separate identities.
pub mod cli;
pub mod engine;
pub mod host;

pub type Result<T> = std::result::Result<T, String>;

/// Trusted host clock in Unix seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
