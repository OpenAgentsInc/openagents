use std::path::PathBuf;

use oanix::OanixManifest;
use oanix::manifest::IdentityManifest;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WalletIdentityState {
    Unknown,
    Uninitialized,
    Initialized,
}

#[derive(Clone, Debug)]
pub(crate) struct WalletIdentitySnapshot {
    pub(crate) state: WalletIdentityState,
    pub(crate) npub: Option<String>,
    pub(crate) network: Option<String>,
    pub(crate) balance_sats: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct WalletConfigSnapshot {
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) exists: bool,
    pub(crate) enable_payments: Option<bool>,
    pub(crate) spark_url: Option<String>,
    pub(crate) spark_token_present: Option<bool>,
    pub(crate) data_dir: Option<PathBuf>,
    pub(crate) network: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct WalletSnapshot {
    pub(crate) identity: WalletIdentitySnapshot,
    pub(crate) config: WalletConfigSnapshot,
}

pub(crate) struct WalletState {
    pub(crate) snapshot: WalletSnapshot,
}

impl WalletState {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: WalletSnapshot::build(None),
        }
    }

    pub(crate) fn refresh(&mut self, manifest: Option<&OanixManifest>) {
        self.snapshot = WalletSnapshot::build(manifest);
    }
}

impl WalletSnapshot {
    pub(crate) fn build(manifest: Option<&OanixManifest>) -> Self {
        let identity = match manifest {
            Some(manifest) => identity_snapshot_from_identity(&manifest.identity),
            None => WalletIdentitySnapshot::unknown(),
        };
        let config = load_wallet_config();
        Self { identity, config }
    }
}

impl WalletIdentitySnapshot {
    fn unknown() -> Self {
        Self {
            state: WalletIdentityState::Unknown,
            npub: None,
            network: None,
            balance_sats: None,
        }
    }
}

pub(crate) fn identity_snapshot_from_identity(
    identity: &IdentityManifest,
) -> WalletIdentitySnapshot {
    let state = if identity.initialized {
        WalletIdentityState::Initialized
    } else {
        WalletIdentityState::Uninitialized
    };
    WalletIdentitySnapshot {
        state,
        npub: identity.npub.clone(),
        network: identity.network.clone(),
        balance_sats: identity.wallet_balance_sats,
    }
}

pub(crate) fn parse_wallet_config(
    value: &toml::Value,
    config_path: Option<PathBuf>,
    data_dir_fallback: Option<PathBuf>,
) -> WalletConfigSnapshot {
    let enable_payments = value.get("enable_payments").and_then(|v| v.as_bool());
    let spark_url = value
        .get("spark_url")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let spark_token_present = value
        .get("spark_token")
        .and_then(|v| v.as_str())
        .map(|token| !token.trim().is_empty());
    let network = value
        .get("network")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let data_dir = value
        .get("data_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or(data_dir_fallback);

    WalletConfigSnapshot {
        config_path,
        exists: true,
        enable_payments,
        spark_url,
        spark_token_present,
        data_dir,
        network,
        error: None,
    }
}

fn load_wallet_config() -> WalletConfigSnapshot {
    let pylon_dir = pylon_dir();
    let config_path = pylon_dir.as_ref().map(|dir| dir.join("config.toml"));
    let data_dir_fallback = pylon_dir.clone();
    let mut snapshot = WalletConfigSnapshot {
        config_path: config_path.clone(),
        exists: false,
        enable_payments: None,
        spark_url: None,
        spark_token_present: None,
        data_dir: data_dir_fallback.clone(),
        network: None,
        error: None,
    };

    let Some(ref path) = config_path else {
        return snapshot;
    };
    if !path.exists() {
        return snapshot;
    }

    snapshot.exists = true;
    match std::fs::read_to_string(&path) {
        Ok(contents) => match toml::from_str::<toml::Value>(&contents) {
            Ok(value) => parse_wallet_config(&value, config_path, data_dir_fallback),
            Err(err) => {
                snapshot.error = Some(format!("Failed to parse config: {}", err));
                snapshot
            }
        },
        Err(err) => {
            snapshot.error = Some(format!("Failed to read config: {}", err));
            snapshot
        }
    }
}

fn pylon_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".openagents").join("pylon"))
}
