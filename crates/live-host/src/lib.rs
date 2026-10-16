//! NIP-LIVE session controller over explicit media, capture, and device
//! adapters. A room, a recognized voice, or a transcript grants nothing;
//! capture transport alone never bypasses admission.
pub mod cli;
pub mod controller;
pub mod delivery;
pub mod document;
pub mod media;
#[cfg(unix)]
pub mod resident;

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
