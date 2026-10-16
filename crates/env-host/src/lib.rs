//! NIP-ENV allocator, provider bindings, and participant/worker roles.
//!
//! A lease is not a wallet authorization, a commercial order, or a grant of
//! workspace or credential access. All fixtures use throwaway keys, local
//! state, and fake allowance units.
pub mod allocator;
pub mod cli;
pub mod local;
pub mod participant;
pub mod provider;
#[cfg(unix)]
pub mod ssh;

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
