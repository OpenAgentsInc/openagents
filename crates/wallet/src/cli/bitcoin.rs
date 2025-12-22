//! Bitcoin/Lightning CLI commands

use anyhow::Result;

pub fn balance() -> Result<()> {
    anyhow::bail!("Balance querying requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn balance_detailed() -> Result<()> {
    anyhow::bail!("Balance querying requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn receive(_amount: Option<u64>) -> Result<()> {
    anyhow::bail!("Receive address generation requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn send(_address: String, _amount: u64) -> Result<()> {
    anyhow::bail!("Send payments require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn invoice(_amount: u64, _description: Option<String>) -> Result<()> {
    anyhow::bail!("Lightning invoice generation requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn pay(_invoice: String) -> Result<()> {
    anyhow::bail!("Lightning invoice payment requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn history(_limit: usize) -> Result<()> {
    anyhow::bail!("Transaction history requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn deposit() -> Result<()> {
    anyhow::bail!("On-chain deposits require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn withdraw(_address: String, _amount: u64) -> Result<()> {
    anyhow::bail!("On-chain withdrawals require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn zap(_note_id: String, _amount: u64) -> Result<()> {
    anyhow::bail!("Zap payments require Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn zaps(_note_id: String) -> Result<()> {
    anyhow::bail!("Zap queries require Nostr relay integration. See directive d-002 for implementation.")
}

pub fn nwc_create(_name: Option<String>) -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn nwc_list() -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires Breez SDK integration. See directive d-001 for integration roadmap.")
}

pub fn nwc_revoke(_id: String) -> Result<()> {
    anyhow::bail!("NIP-47 Nostr Wallet Connect requires Breez SDK integration. See directive d-001 for integration roadmap.")
}
