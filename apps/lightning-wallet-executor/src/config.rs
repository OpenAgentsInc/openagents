use std::collections::{BTreeMap, BTreeSet};

use crate::error::WalletExecutorConfigError;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExecutorMode {
    Mock,
    Spark,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SparkNetwork {
    Mainnet,
    Regtest,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MnemonicProvider {
    Env,
    Gcp,
}

#[derive(Debug, Clone)]
pub struct WalletExecutorConfig {
    pub host: String,
    pub port: u16,
    pub wallet_id: String,
    pub auth_token: Option<String>,
    pub auth_token_version: u32,
    pub mode: ExecutorMode,
    pub network: SparkNetwork,
    pub spark_api_key: Option<String>,
    pub mnemonic_provider: MnemonicProvider,
    pub mnemonic_env_var: String,
    pub mnemonic_secret_version: Option<String>,
    pub request_cap_msats: u64,
    pub window_cap_msats: u64,
    pub window_ms: u64,
    pub payment_timeout_secs: u64,
    pub allowed_hosts: BTreeSet<String>,
}

impl WalletExecutorConfig {
    pub fn default_mock() -> Self {
        let mut allowed_hosts = BTreeSet::new();
        let _ = allowed_hosts.insert("sats4ai.com".to_string());
        let _ = allowed_hosts.insert("l402.openagents.com".to_string());

        Self {
            host: "127.0.0.1".to_string(),
            port: 8788,
            wallet_id: "openagents-ep212".to_string(),
            auth_token: None,
            auth_token_version: 1,
            mode: ExecutorMode::Mock,
            network: SparkNetwork::Regtest,
            spark_api_key: None,
            mnemonic_provider: MnemonicProvider::Env,
            mnemonic_env_var: "OA_LIGHTNING_WALLET_MNEMONIC".to_string(),
            mnemonic_secret_version: None,
            request_cap_msats: 200_000,
            window_cap_msats: 1_000_000,
            window_ms: 300_000,
            payment_timeout_secs: 45,
            allowed_hosts,
        }
    }

    pub fn from_process_env() -> Result<Self, WalletExecutorConfigError> {
        let map: BTreeMap<String, String> = std::env::vars().collect();
        Self::from_env_map(&map)
    }

    pub fn from_env_map(env: &BTreeMap<String, String>) -> Result<Self, WalletExecutorConfigError> {
        let default = Self::default_mock();

        let host = env
            .get("OA_LIGHTNING_WALLET_EXECUTOR_HOST")
            .map_or(default.host.clone(), |value| value.trim().to_string());

        let port = parse_u16(
            "OA_LIGHTNING_WALLET_EXECUTOR_PORT",
            env.get("OA_LIGHTNING_WALLET_EXECUTOR_PORT")
                .map(String::as_str),
            default.port,
            1,
            u16::MAX,
        )?;

        let wallet_id = non_empty(
            "OA_LIGHTNING_WALLET_ID",
            env.get("OA_LIGHTNING_WALLET_ID").map(String::as_str),
            Some(default.wallet_id.as_str()),
        )?;

        let auth_token = env
            .get("OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let auth_token_version = parse_u32(
            "OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN_VERSION",
            env.get("OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN_VERSION")
                .map(String::as_str),
            default.auth_token_version,
            1,
            1_000_000,
        )?;

        let mode = match env
            .get("OA_LIGHTNING_WALLET_EXECUTOR_MODE")
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "mock".to_string())
            .as_str()
        {
            "spark" => ExecutorMode::Spark,
            _ => ExecutorMode::Mock,
        };

        let network = match env
            .get("OA_LIGHTNING_SPARK_NETWORK")
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "regtest".to_string())
            .as_str()
        {
            "mainnet" => SparkNetwork::Mainnet,
            _ => SparkNetwork::Regtest,
        };

        let spark_api_key = env
            .get("OA_LIGHTNING_SPARK_API_KEY")
            .or_else(|| env.get("OA_LIGHTNING_BREEZ_API_KEY"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let mnemonic_provider = match env
            .get("OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER")
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "env".to_string())
            .as_str()
        {
            "gcp" => MnemonicProvider::Gcp,
            _ => MnemonicProvider::Env,
        };

        let mnemonic_env_var = env
            .get("OA_LIGHTNING_WALLET_MNEMONIC_ENV_VAR")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "OA_LIGHTNING_WALLET_MNEMONIC".to_string());

        let mnemonic_secret_version = env
            .get("OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let request_cap_msats = parse_u64(
            "OA_LIGHTNING_WALLET_REQUEST_CAP_MSATS",
            env.get("OA_LIGHTNING_WALLET_REQUEST_CAP_MSATS")
                .map(String::as_str),
            default.request_cap_msats,
            1_000,
            100_000_000,
        )?;

        let window_cap_msats = parse_u64(
            "OA_LIGHTNING_WALLET_WINDOW_CAP_MSATS",
            env.get("OA_LIGHTNING_WALLET_WINDOW_CAP_MSATS")
                .map(String::as_str),
            default.window_cap_msats,
            10_000,
            5_000_000_000,
        )?;

        let window_ms = parse_u64(
            "OA_LIGHTNING_WALLET_WINDOW_MS",
            env.get("OA_LIGHTNING_WALLET_WINDOW_MS").map(String::as_str),
            default.window_ms,
            1_000,
            86_400_000,
        )?;

        let payment_timeout_secs = parse_u64(
            "OA_LIGHTNING_SPARK_PAYMENT_TIMEOUT_SECS",
            env.get("OA_LIGHTNING_SPARK_PAYMENT_TIMEOUT_SECS")
                .map(String::as_str),
            default.payment_timeout_secs,
            1,
            300,
        )?;

        let allowed_hosts = normalize_host_set(
            env.get("OA_LIGHTNING_WALLET_ALLOWED_HOSTS")
                .map(String::as_str),
        );

        if mode == ExecutorMode::Spark {
            if spark_api_key.is_none() {
                return Err(WalletExecutorConfigError::new(
                    "OA_LIGHTNING_SPARK_API_KEY",
                    "required when OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark",
                ));
            }
            if auth_token.is_none() {
                return Err(WalletExecutorConfigError::new(
                    "OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN",
                    "required when OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark",
                ));
            }
            if mnemonic_provider == MnemonicProvider::Gcp && mnemonic_secret_version.is_none() {
                return Err(WalletExecutorConfigError::new(
                    "OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION",
                    "required when OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=gcp",
                ));
            }
            if allowed_hosts.is_empty() {
                return Err(WalletExecutorConfigError::new(
                    "OA_LIGHTNING_WALLET_ALLOWED_HOSTS",
                    "must include at least one host when mode=spark",
                ));
            }
        }

        Ok(Self {
            host,
            port,
            wallet_id,
            auth_token,
            auth_token_version,
            mode,
            network,
            spark_api_key,
            mnemonic_provider,
            mnemonic_env_var,
            mnemonic_secret_version,
            request_cap_msats,
            window_cap_msats,
            window_ms,
            payment_timeout_secs,
            allowed_hosts,
        })
    }

    pub fn auth_mode(&self) -> &'static str {
        if self.auth_token.is_some() {
            "bearer_static"
        } else {
            "disabled"
        }
    }
}

fn non_empty(
    field: &str,
    value: Option<&str>,
    fallback: Option<&str>,
) -> Result<String, WalletExecutorConfigError> {
    let candidate = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| fallback.map(str::trim).filter(|value| !value.is_empty()))
        .unwrap_or_default();

    if candidate.is_empty() {
        return Err(WalletExecutorConfigError::new(
            field,
            "missing required environment variable",
        ));
    }

    Ok(candidate.to_string())
}

fn parse_u16(
    field: &str,
    value: Option<&str>,
    fallback: u16,
    min: u16,
    max: u16,
) -> Result<u16, WalletExecutorConfigError> {
    let parsed = match value.map(str::trim) {
        Some(raw) if !raw.is_empty() => raw
            .parse::<u16>()
            .map_err(|_| WalletExecutorConfigError::new(field, "must be an integer"))?,
        _ => fallback,
    };

    if parsed < min || parsed > max {
        return Err(WalletExecutorConfigError::new(
            field,
            format!("must be between {min} and {max}"),
        ));
    }

    Ok(parsed)
}

fn parse_u32(
    field: &str,
    value: Option<&str>,
    fallback: u32,
    min: u32,
    max: u32,
) -> Result<u32, WalletExecutorConfigError> {
    let parsed = match value.map(str::trim) {
        Some(raw) if !raw.is_empty() => raw
            .parse::<u32>()
            .map_err(|_| WalletExecutorConfigError::new(field, "must be an integer"))?,
        _ => fallback,
    };

    if parsed < min || parsed > max {
        return Err(WalletExecutorConfigError::new(
            field,
            format!("must be between {min} and {max}"),
        ));
    }

    Ok(parsed)
}

fn parse_u64(
    field: &str,
    value: Option<&str>,
    fallback: u64,
    min: u64,
    max: u64,
) -> Result<u64, WalletExecutorConfigError> {
    let parsed = match value.map(str::trim) {
        Some(raw) if !raw.is_empty() => raw
            .parse::<u64>()
            .map_err(|_| WalletExecutorConfigError::new(field, "must be an integer"))?,
        _ => fallback,
    };

    if parsed < min || parsed > max {
        return Err(WalletExecutorConfigError::new(
            field,
            format!("must be between {min} and {max}"),
        ));
    }

    Ok(parsed)
}

fn normalize_host_set(value: Option<&str>) -> BTreeSet<String> {
    let mut hosts = BTreeSet::new();
    if let Some(raw) = value {
        for host in raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let _ = hosts.insert(host.to_ascii_lowercase());
        }
    }
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn spark_mode_requires_api_key() {
        let env = map(&[
            ("OA_LIGHTNING_WALLET_EXECUTOR_MODE", "spark"),
            ("OA_LIGHTNING_WALLET_ALLOWED_HOSTS", "sats4ai.com"),
        ]);

        let error =
            WalletExecutorConfig::from_env_map(&env).expect_err("config should fail closed");
        assert_eq!(error.field, "OA_LIGHTNING_SPARK_API_KEY");
    }

    #[test]
    fn spark_mode_requires_auth_token() {
        let env = map(&[
            ("OA_LIGHTNING_WALLET_EXECUTOR_MODE", "spark"),
            ("OA_LIGHTNING_SPARK_API_KEY", "spark-key"),
            ("OA_LIGHTNING_WALLET_ALLOWED_HOSTS", "sats4ai.com"),
        ]);

        let error =
            WalletExecutorConfig::from_env_map(&env).expect_err("config should fail closed");
        assert_eq!(error.field, "OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN");
    }

    #[test]
    fn spark_mode_requires_allowlist() {
        let env = map(&[
            ("OA_LIGHTNING_WALLET_EXECUTOR_MODE", "spark"),
            ("OA_LIGHTNING_SPARK_API_KEY", "spark-key"),
            ("OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN", "token"),
        ]);

        let error =
            WalletExecutorConfig::from_env_map(&env).expect_err("config should fail closed");
        assert_eq!(error.field, "OA_LIGHTNING_WALLET_ALLOWED_HOSTS");
    }

    #[test]
    fn accepts_valid_spark_config() {
        let env = map(&[
            ("OA_LIGHTNING_WALLET_EXECUTOR_MODE", "spark"),
            ("OA_LIGHTNING_SPARK_API_KEY", "spark-key"),
            ("OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN", "token"),
            (
                "OA_LIGHTNING_WALLET_ALLOWED_HOSTS",
                "sats4ai.com,l402.openagents.com",
            ),
            ("OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER", "gcp"),
            (
                "OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION",
                "projects/p/secrets/s/versions/latest",
            ),
            ("OA_LIGHTNING_SPARK_NETWORK", "mainnet"),
        ]);

        let config = WalletExecutorConfig::from_env_map(&env).expect("spark config should parse");
        assert_eq!(config.mode, ExecutorMode::Spark);
        assert_eq!(config.network, SparkNetwork::Mainnet);
        assert!(config.allowed_hosts.contains("sats4ai.com"));
        assert!(config.allowed_hosts.contains("l402.openagents.com"));
        assert_eq!(config.auth_token.as_deref(), Some("token"));
        assert_eq!(config.auth_token_version, 1);
        assert_eq!(config.mnemonic_provider, MnemonicProvider::Gcp);
    }
}
