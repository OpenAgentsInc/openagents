//! NIP-SOV lifecycle coordination. One authoritative coordinator per agent
//! serializes activation, checkpoints, controller transfer, and recovery.
//! Finding another host never grants transfer authority, and a new
//! generation acts only after every old dispatcher acknowledged fencing.
pub mod cli;
pub mod coordinator;

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
