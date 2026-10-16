//! NIP-SOV remote custody over NIP-46 and guardian quorum admission.
//!
//! The custodian never holds or exports the agent key: signing and NIP-44
//! operations go to the remote signer named by an authority-signed custody
//! policy. Guardians approve exact actions; they are not custodians. Nothing
//! here adds custody to a relay or moves money.
pub mod cli;
pub mod custodian;
pub mod signer;

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
