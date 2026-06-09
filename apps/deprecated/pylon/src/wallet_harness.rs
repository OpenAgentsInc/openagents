use serde::{Deserialize, Serialize};

pub const PYLON_LDK_WALLET_HARNESS_SCHEMA: &str = "pylon.ldk_wallet_harness.plan.v1";
pub const PYLON_LDK_WALLET_HARNESS_SCRIPT: &str = "scripts/pylon/ldk-wallet-regtest-harness.sh";
pub const PYLON_LDK_WALLET_HARNESS_TEST: &str = "ldk_wallet_regtest_harness";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonLdkWalletHarnessPlan {
    pub schema: String,
    pub default_network: String,
    pub script: String,
    pub ignored_test: String,
    pub required_local_services: Vec<String>,
    pub required_proofs: Vec<String>,
    pub required_artifacts: Vec<String>,
    pub steps: Vec<PylonLdkWalletHarnessStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonLdkWalletHarnessStep {
    pub id: String,
    pub description: String,
    pub required: bool,
    pub artifact_keys: Vec<String>,
}

pub fn pylon_ldk_wallet_harness_plan() -> PylonLdkWalletHarnessPlan {
    PylonLdkWalletHarnessPlan {
        schema: PYLON_LDK_WALLET_HARNESS_SCHEMA.to_string(),
        default_network: "regtest".to_string(),
        script: PYLON_LDK_WALLET_HARNESS_SCRIPT.to_string(),
        ignored_test: PYLON_LDK_WALLET_HARNESS_TEST.to_string(),
        required_local_services: vec![
            "bitcoind regtest".to_string(),
            "electrsd regtest Electrum indexer".to_string(),
            "two ldk-node instances with private storage directories".to_string(),
        ],
        required_proofs: vec![
            "real BOLT11 payment succeeds end to end".to_string(),
            "on-chain receive and withdrawal are observed".to_string(),
            "restart preserves payment and channel state".to_string(),
            "backup/restore copy preserves receiver payment history".to_string(),
            "BOLT12 receive/send is attempted and recorded when ldk-node exposes it".to_string(),
            "artifact output records payment hash, balances, channel ids, txids, and receipt ids"
                .to_string(),
        ],
        required_artifacts: vec![
            "harness-summary.json".to_string(),
            "receiver-backup/".to_string(),
            "payer-storage/".to_string(),
            "receiver-storage/".to_string(),
            "restored-receiver-storage/".to_string(),
        ],
        steps: vec![
            PylonLdkWalletHarnessStep {
                id: "regtest_services".to_string(),
                description: "start isolated bitcoind and electrsd services".to_string(),
                required: true,
                artifact_keys: vec!["bitcoind_rpc".to_string(), "electrs_urls".to_string()],
            },
            PylonLdkWalletHarnessStep {
                id: "node_bootstrap".to_string(),
                description: "start payer and receiver ldk-node wallets with persisted storage"
                    .to_string(),
                required: true,
                artifact_keys: vec![
                    "payer_node_id".to_string(),
                    "receiver_node_id".to_string(),
                    "storage_paths".to_string(),
                ],
            },
            PylonLdkWalletHarnessStep {
                id: "onchain_receive".to_string(),
                description: "mine regtest funds into both wallet on-chain addresses".to_string(),
                required: true,
                artifact_keys: vec![
                    "funding_txid".to_string(),
                    "payer_pre_balance".to_string(),
                    "receiver_pre_balance".to_string(),
                ],
            },
            PylonLdkWalletHarnessStep {
                id: "open_channel".to_string(),
                description: "open and confirm a channel from payer to receiver".to_string(),
                required: true,
                artifact_keys: vec!["funding_outpoint".to_string(), "channel_ids".to_string()],
            },
            PylonLdkWalletHarnessStep {
                id: "bolt11_payment".to_string(),
                description: "create a receiver BOLT11 invoice and pay it from payer".to_string(),
                required: true,
                artifact_keys: vec![
                    "payment_hash".to_string(),
                    "payment_id".to_string(),
                    "receipt_ids".to_string(),
                    "payer_post_balance".to_string(),
                    "receiver_post_balance".to_string(),
                ],
            },
            PylonLdkWalletHarnessStep {
                id: "bolt12_attempt".to_string(),
                description: "attempt BOLT12 offer receive/send and record whether this ldk-node build supports it"
                    .to_string(),
                required: false,
                artifact_keys: vec!["bolt12_status".to_string()],
            },
            PylonLdkWalletHarnessStep {
                id: "onchain_withdrawal".to_string(),
                description: "send receiver on-chain funds back to payer and mine confirmation"
                    .to_string(),
                required: true,
                artifact_keys: vec!["withdrawal_txid".to_string()],
            },
            PylonLdkWalletHarnessStep {
                id: "restart_persistence".to_string(),
                description: "stop/restart both nodes and assert payment/channel state persists"
                    .to_string(),
                required: true,
                artifact_keys: vec!["restart_payment_status".to_string()],
            },
            PylonLdkWalletHarnessStep {
                id: "backup_restore".to_string(),
                description: "copy receiver wallet storage into a backup artifact and restore it into a fresh node directory"
                    .to_string(),
                required: true,
                artifact_keys: vec![
                    "receiver_backup_digest".to_string(),
                    "restored_payment_status".to_string(),
                    "restored_channel_count".to_string(),
                ],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::pylon_ldk_wallet_harness_plan;

    #[test]
    fn pylon_ldk_wallet_harness_plan_covers_required_evidence() {
        let plan = pylon_ldk_wallet_harness_plan();

        assert_eq!(plan.default_network, "regtest");
        assert_eq!(plan.ignored_test, "ldk_wallet_regtest_harness");
        assert!(
            plan.required_local_services
                .iter()
                .any(|service| service.contains("bitcoind"))
        );
        assert!(
            plan.required_proofs
                .iter()
                .any(|proof| proof.contains("BOLT11 payment"))
        );
        assert!(
            plan.required_proofs
                .iter()
                .any(|proof| proof.contains("backup/restore"))
        );
        assert!(
            plan.required_proofs
                .iter()
                .any(|proof| proof.contains("BOLT12"))
        );

        let required_steps: Vec<_> = plan
            .steps
            .iter()
            .filter(|step| step.required)
            .map(|step| step.id.as_str())
            .collect();
        for expected in [
            "regtest_services",
            "node_bootstrap",
            "onchain_receive",
            "open_channel",
            "bolt11_payment",
            "onchain_withdrawal",
            "restart_persistence",
            "backup_restore",
        ] {
            assert!(
                required_steps.contains(&expected),
                "missing required harness step {expected}"
            );
        }
    }
}
