use std::path::PathBuf;

use adjutant::IdentityManifest;

use crate::app::wallet::{
    WalletIdentityState, identity_snapshot_from_identity, parse_wallet_config,
};

#[test]
fn parse_wallet_config_extracts_fields() {
    let value: toml::Value = toml::from_str(
        r#"
enable_payments = true
spark_url = "https://spark.local"
spark_token = "secret"
network = "regtest"
data_dir = "/tmp/openagents"
"#,
    )
    .expect("valid toml");

    let snapshot = parse_wallet_config(&value, Some(PathBuf::from("/tmp/config.toml")), None);
    assert!(snapshot.exists);
    assert_eq!(snapshot.enable_payments, Some(true));
    assert_eq!(snapshot.spark_url.as_deref(), Some("https://spark.local"));
    assert_eq!(snapshot.spark_token_present, Some(true));
    assert_eq!(snapshot.network.as_deref(), Some("regtest"));
    assert_eq!(snapshot.data_dir, Some(PathBuf::from("/tmp/openagents")));
    assert!(snapshot.error.is_none());
}

#[test]
fn parse_wallet_config_handles_empty_token() {
    let value: toml::Value = toml::from_str(
        r#"
spark_token = ""
"#,
    )
    .expect("valid toml");
    let snapshot = parse_wallet_config(
        &value,
        Some(PathBuf::from("/tmp/config.toml")),
        Some(PathBuf::from("/fallback")),
    );
    assert_eq!(snapshot.spark_token_present, Some(false));
    assert_eq!(snapshot.data_dir, Some(PathBuf::from("/fallback")));
}

#[test]
fn identity_snapshot_tracks_state() {
    let uninitialized = IdentityManifest {
        initialized: false,
        npub: None,
        wallet_balance_sats: None,
        network: None,
    };
    let snapshot = identity_snapshot_from_identity(&uninitialized);
    assert_eq!(snapshot.state, WalletIdentityState::Uninitialized);

    let initialized = IdentityManifest {
        initialized: true,
        npub: Some("npub123".to_string()),
        wallet_balance_sats: Some(42),
        network: Some("regtest".to_string()),
    };
    let snapshot = identity_snapshot_from_identity(&initialized);
    assert_eq!(snapshot.state, WalletIdentityState::Initialized);
    assert_eq!(snapshot.npub.as_deref(), Some("npub123"));
    assert_eq!(snapshot.balance_sats, Some(42));
}
