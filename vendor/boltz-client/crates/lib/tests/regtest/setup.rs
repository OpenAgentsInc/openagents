//! Regtest configuration and health check.

use boltz_client::{AlchemyConfig, BoltzConfig};

/// Regtest Boltz API URL.
const REGTEST_API_URL: &str = "http://localhost:9001";
/// Anvil RPC URL (from Docker stack).
const REGTEST_ANVIL_RPC: &str = "http://localhost:8545";
/// Anvil chain ID used by the Docker regtest stack.
const REGTEST_CHAIN_ID: u64 = 33;

/// Build a `BoltzConfig` for the local regtest environment.
pub fn regtest_config() -> BoltzConfig {
    BoltzConfig {
        api_url: REGTEST_API_URL.to_string(),
        alchemy_config: AlchemyConfig {
            gas_sponsor_url: "https://sponsor.test/".to_string(),
        },
        arbitrum_rpc_url: REGTEST_ANVIL_RPC.to_string(),
        chain_id: REGTEST_CHAIN_ID,
        referral_id: "regtest".to_string(),
        slippage_bps: 100,
        oft_deployments_url: boltz_client::DEFAULT_OFT_DEPLOYMENTS_URL.to_string(),
        cctp_api_url: boltz_client::DEFAULT_CCTP_API_URL.to_string(),
        solana_rpc_url: boltz_client::DEFAULT_SOLANA_RPC_URL.to_string(),
        lz_scan_api_url: boltz_client::DEFAULT_LZ_SCAN_API_URL.to_string(),
        delivery_poll_interval_secs: Some(boltz_client::DEFAULT_DELIVERY_POLL_INTERVAL_SECS),
        proxy: None,
    }
}

/// A regtest config reaching the backend by its container name and routed
/// through `proxy`.
///
/// The host cannot resolve that name, so the call can only succeed through
/// the proxy: a direct connection fails at DNS rather than quietly working.
pub fn proxied_config(proxy: platform_utils::ProxyConfig, api_url: &str) -> BoltzConfig {
    BoltzConfig {
        api_url: api_url.to_string(),
        proxy: Some(proxy),
        ..regtest_config()
    }
}

/// Seed bytes for regtest testing (deterministic, not secret).
pub fn regtest_seed() -> Vec<u8> {
    // "abandon" x11 + "about" mnemonic → always the same seed
    let mnemonic = bip39::Mnemonic::parse_normalized(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("valid test mnemonic");
    mnemonic.to_seed("").to_vec()
}
