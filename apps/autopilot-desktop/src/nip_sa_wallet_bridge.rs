use openagents_spark::Balance;

/// Convert Spark's layered balance representation to the total sats tracked by NIP-SA state.
pub fn spark_total_balance_sats(balance: &Balance) -> u64 {
    balance.total_sats()
}

/// Sync NIP-SA state wallet balance from an optional Spark balance snapshot.
///
/// Returns the updated wallet balance in sats.
#[cfg(test)]
pub fn sync_agent_state_wallet_balance(
    state: &mut nostr::AgentStateContent,
    spark_balance: Option<&Balance>,
) -> u64 {
    let total = spark_balance.map_or(0, spark_total_balance_sats);
    state.update_balance(total);
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::AgentStateContent;

    #[test]
    fn sync_from_balance_snapshot_updates_state() {
        let mut state = AgentStateContent::new();
        let balance = Balance {
            spark_sats: 900,
            lightning_sats: 100,
            onchain_sats: 50,
        };

        let total = sync_agent_state_wallet_balance(&mut state, Some(&balance));
        assert_eq!(total, 1_050);
        assert_eq!(state.wallet_balance_sats, 1_050);
    }

    #[test]
    fn sync_without_balance_zeros_state() {
        let mut state = AgentStateContent::new();
        state.update_balance(500);

        let total = sync_agent_state_wallet_balance(&mut state, None);
        assert_eq!(total, 0);
        assert_eq!(state.wallet_balance_sats, 0);
    }
}
