//! NIP-SOV treasury over existing owners: `pay-ledger` for earnings and
//! payouts, the agent's wallet for paying and receiving, and the X402 router
//! plus account service for top-ups. Null treasury spends nothing; income
//! raises an allowance only under explicit policy. Fixtures use fake money.
pub mod adapters;
pub mod cli;
pub mod treasury;

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
