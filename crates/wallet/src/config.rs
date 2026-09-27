//! Wallet configuration under `~/.openagents/wallet/` (or `OPENAGENTS_WALLET_HOME`):
//! `config.json` for the network and chain source, `seed` for the BIP39
//! mnemonic, and the node's SQLite store next to them. The seed file is
//! created once and is never printed by any command.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::WalletError;

pub const CONFIG_FILE: &str = "config.json";
pub const SEED_FILE: &str = "seed";
pub const STORE_DIR: &str = "ldk";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Bitcoin,
    Testnet,
    Signet,
    Regtest,
}

impl Network {
    pub fn parse(text: &str) -> Result<Self, WalletError> {
        match text {
            "bitcoin" | "mainnet" => Ok(Self::Bitcoin),
            "testnet" => Ok(Self::Testnet),
            "signet" => Ok(Self::Signet),
            "regtest" => Ok(Self::Regtest),
            other => Err(WalletError::Invalid(format!(
                "unknown network {other:?}; use bitcoin, testnet, signet, or regtest"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bitcoin => "bitcoin",
            Self::Testnet => "testnet",
            Self::Signet => "signet",
            Self::Regtest => "regtest",
        }
    }

    /// The public Esplora server used when `init` names none. Regtest has
    /// none: the caller must run its own.
    pub fn default_esplora(self) -> Option<&'static str> {
        match self {
            Self::Bitcoin => Some("https://blockstream.info/api"),
            Self::Testnet => Some("https://blockstream.info/testnet/api"),
            Self::Signet => Some("https://mempool.space/signet/api"),
            Self::Regtest => None,
        }
    }
}

/// An LSPS2 liquidity provider that opens inbound channels just in time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lsp {
    /// 66 hex digits.
    pub node_id: String,
    /// `host:port`.
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Lsp {
    /// Parse `NODE_ID@HOST:PORT`.
    pub fn parse(text: &str, token: Option<&str>) -> Result<Self, WalletError> {
        let (node_id, address) = text
            .split_once('@')
            .ok_or_else(|| WalletError::Invalid("LSP must be NODE_ID@HOST:PORT".to_string()))?;
        if node_id.len() != 66 || !node_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(WalletError::Invalid(
                "LSP node id must be 66 hex digits".to_string(),
            ));
        }
        if address.rsplit_once(':').is_none() {
            return Err(WalletError::Invalid(
                "LSP address must be HOST:PORT".to_string(),
            ));
        }
        Ok(Self {
            node_id: node_id.to_ascii_lowercase(),
            address: address.to_string(),
            token: token.map(str::to_string),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletConfig {
    pub network: Network,
    pub esplora_url: String,
    /// `host:port` the node listens on for peers, when it listens at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lsp: Option<Lsp>,
}

impl WalletConfig {
    pub fn new(network: Network, esplora_url: Option<&str>) -> Result<Self, WalletError> {
        let esplora_url = match esplora_url {
            Some(url) => url.trim_end_matches('/').to_string(),
            None => network
                .default_esplora()
                .ok_or_else(|| {
                    WalletError::Invalid(format!(
                        "{} has no public Esplora server; pass --esplora URL",
                        network.as_str()
                    ))
                })?
                .to_string(),
        };
        if !(esplora_url.starts_with("http://") || esplora_url.starts_with("https://")) {
            return Err(WalletError::Invalid(
                "Esplora URL must start with http:// or https://".to_string(),
            ));
        }
        Ok(Self {
            network,
            esplora_url,
            listen: None,
            lsp: None,
        })
    }

    pub fn load(home: &Path) -> Result<Self, WalletError> {
        let path = home.join(CONFIG_FILE);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            WalletError::Setup(format!(
                "wallet is not initialized ({}: {error}); run `openagents wallet init`",
                path.display()
            ))
        })?;
        serde_json::from_str(&text)
            .map_err(|error| WalletError::Setup(format!("{}: {error}", path.display())))
    }

    pub fn save(&self, home: &Path) -> Result<(), WalletError> {
        std::fs::create_dir_all(home)
            .map_err(|error| WalletError::Setup(format!("{}: {error}", home.display())))?;
        let path = home.join(CONFIG_FILE);
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| WalletError::Setup(error.to_string()))?;
        std::fs::write(&path, text)
            .map_err(|error| WalletError::Setup(format!("{}: {error}", path.display())))
    }
}

/// The wallet directory: `OPENAGENTS_WALLET_HOME`, else `~/.openagents/wallet`.
pub fn home() -> PathBuf {
    if let Some(path) = std::env::var_os("OPENAGENTS_WALLET_HOME") {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".openagents").join("wallet")
}

/// Read the seed mnemonic, or write a fresh one when `create` is set and none
/// exists. Returns the mnemonic and whether it was created now.
pub fn load_or_create_seed(
    home: &Path,
    create: bool,
    generate: impl FnOnce() -> String,
) -> Result<(String, bool), WalletError> {
    let path = home.join(SEED_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok((text.trim().to_string(), false)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            std::fs::create_dir_all(home)
                .map_err(|error| WalletError::Setup(format!("{}: {error}", home.display())))?;
            let mnemonic = generate();
            write_private(&path, &mnemonic)?;
            Ok((mnemonic, true))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(WalletError::Setup(format!(
                "no seed at {}; run `openagents wallet init`",
                path.display()
            )))
        }
        Err(error) => Err(WalletError::Setup(format!("{}: {error}", path.display()))),
    }
}

fn write_private(path: &Path, text: &str) -> Result<(), WalletError> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| WalletError::Setup(format!("{}: {error}", path.display())))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|error| WalletError::Setup(format!("{}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openagents-wallet-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn config_round_trips() {
        let home = temp_home("config");
        let mut config = WalletConfig::new(Network::Signet, None).unwrap();
        config.lsp =
            Some(Lsp::parse(&format!("{}@lsp.example:9735", "ab".repeat(33)), Some("t")).unwrap());
        config.save(&home).unwrap();
        assert_eq!(WalletConfig::load(&home).unwrap(), config);
        assert_eq!(config.esplora_url, "https://mempool.space/signet/api");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn regtest_needs_an_esplora_url() {
        assert!(WalletConfig::new(Network::Regtest, None).is_err());
        let config = WalletConfig::new(Network::Regtest, Some("http://127.0.0.1:3002/")).unwrap();
        assert_eq!(config.esplora_url, "http://127.0.0.1:3002");
        assert!(WalletConfig::new(Network::Regtest, Some("127.0.0.1:3002")).is_err());
    }

    #[test]
    fn seed_is_created_once() {
        let home = temp_home("seed");
        assert!(load_or_create_seed(&home, false, || unreachable!()).is_err());
        let (first, created) = load_or_create_seed(&home, true, || "a b c".to_string()).unwrap();
        assert!(created);
        let (second, created) = load_or_create_seed(&home, true, || "x y z".to_string()).unwrap();
        assert!(!created);
        assert_eq!(first, second);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(home.join(SEED_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn lsp_is_validated() {
        assert!(Lsp::parse("nope", None).is_err());
        assert!(Lsp::parse(&format!("{}@host", "ab".repeat(33)), None).is_err());
        assert!(Lsp::parse("abc@host:1", None).is_err());
    }
}
