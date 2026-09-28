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

/// Which LSPS protocol the liquidity provider speaks.
///
/// `Lsps1` buys a channel in advance and the node still signs its own
/// invoices, so the node id stays a valid x402 `payTo`. `Lsps2` opens a
/// channel just in time on the first payment.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LspProtocol {
    Lsps1,
    #[default]
    Lsps2,
}

impl LspProtocol {
    pub fn parse(text: &str) -> Result<Self, WalletError> {
        match text {
            "lsps1" => Ok(Self::Lsps1),
            "lsps2" => Ok(Self::Lsps2),
            other => Err(WalletError::Invalid(format!(
                "LSP protocol must be lsps1 or lsps2, not `{other}`"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lsps1 => "lsps1",
            Self::Lsps2 => "lsps2",
        }
    }
}

/// A liquidity provider the node buys inbound capacity from.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lsp {
    /// 66 hex digits.
    pub node_id: String,
    /// `host:port`.
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default)]
    pub protocol: LspProtocol,
}

/// Olympus by ZEUS, LSPS1 endpoints from <https://docs.zeusln.app/lsp/api/lsps1/>.
const OLYMPUS_LSPS1_MAINNET: &str =
    "031b301307574bbe9b9ac7b79cbe1700e31e544513eae0b5d7497483083f99e581@45.79.192.236:9735";
const OLYMPUS_LSPS1_TESTNET: &str =
    "03e84a109cd70e57864274932fc87c5e6434c59ebb8e6e7d28532219ba38f7f6df@139.144.22.237:9735";

impl Lsp {
    /// The Olympus LSPS1 peer for `network`, or an error where Olympus
    /// runs no LSPS1 service.
    pub fn olympus(network: Network) -> Result<Self, WalletError> {
        let peer = match network {
            Network::Bitcoin => OLYMPUS_LSPS1_MAINNET,
            Network::Testnet => OLYMPUS_LSPS1_TESTNET,
            other => {
                return Err(WalletError::Invalid(format!(
                    "Olympus serves LSPS1 on bitcoin and testnet, not {}",
                    other.as_str()
                )));
            }
        };
        Self::parse(peer, None, LspProtocol::Lsps1)
    }

    /// Parse `NODE_ID@HOST:PORT`, or the preset name `olympus` for
    /// `network`.
    pub fn parse_or_preset(
        text: &str,
        token: Option<&str>,
        protocol: Option<LspProtocol>,
        network: Network,
    ) -> Result<Self, WalletError> {
        if text.eq_ignore_ascii_case("olympus") {
            let mut lsp = Self::olympus(network)?;
            lsp.token = token.map(str::to_owned);
            if let Some(protocol) = protocol
                && protocol != LspProtocol::Lsps1
            {
                return Err(WalletError::Invalid(
                    "the olympus preset is LSPS1; drop --lsp-protocol or give a peer".to_string(),
                ));
            }
            return Ok(lsp);
        }
        Self::parse(text, token, protocol.unwrap_or_default())
    }

    /// Parse `NODE_ID@HOST:PORT`.
    pub fn parse(
        text: &str,
        token: Option<&str>,
        protocol: LspProtocol,
    ) -> Result<Self, WalletError> {
        let (node_id, address) = text
            .split_once('@')
            .ok_or_else(|| WalletError::Invalid("LSP must be NODE_ID@HOST:PORT".to_string()))?;
        let node_id = parse_node_id(node_id)
            .map_err(|_| WalletError::Invalid("LSP node id must be 66 hex digits".to_string()))?;
        if address.rsplit_once(':').is_none() {
            return Err(WalletError::Invalid(
                "LSP address must be HOST:PORT".to_string(),
            ));
        }
        Ok(Self {
            node_id,
            address: address.to_string(),
            token: token.map(str::to_string),
            protocol,
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
    /// Node ids (66 hex digits) allowed to open anchor channels to this
    /// node without an on-chain anchor reserve here, such as the owner's
    /// other nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_peers: Vec<String>,
}

/// Validate a 66-hex-digit node id and lowercase it.
pub fn parse_node_id(text: &str) -> Result<String, WalletError> {
    if text.len() != 66 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WalletError::Invalid(
            "node id must be 66 hex digits".to_string(),
        ));
    }
    Ok(text.to_ascii_lowercase())
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
            trusted_peers: Vec::new(),
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
        config.lsp = Some(
            Lsp::parse(
                &format!("{}@lsp.example:9735", "ab".repeat(33)),
                Some("t"),
                LspProtocol::Lsps2,
            )
            .unwrap(),
        );
        config.trusted_peers = vec!["cd".repeat(33)];
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
        assert!(Lsp::parse("nope", None, LspProtocol::Lsps2).is_err());
        assert!(
            Lsp::parse(
                &format!("{}@host", "ab".repeat(33)),
                None,
                LspProtocol::Lsps2
            )
            .is_err()
        );
        assert!(Lsp::parse("abc@host:1", None, LspProtocol::Lsps2).is_err());
        let olympus = Lsp::parse_or_preset("olympus", None, None, Network::Bitcoin).unwrap();
        assert_eq!(olympus.protocol, LspProtocol::Lsps1);
        assert!(olympus.node_id.starts_with("031b3013"));
        assert!(Lsp::parse_or_preset("olympus", None, None, Network::Signet).is_err());
        assert!(
            Lsp::parse_or_preset("olympus", None, Some(LspProtocol::Lsps2), Network::Bitcoin)
                .is_err()
        );
        let plain: Lsp = serde_json::from_str(&format!(
            r#"{{"node_id":"{}","address":"h:1"}}"#,
            "ab".repeat(33)
        ))
        .unwrap();
        assert_eq!(plain.protocol, LspProtocol::Lsps2);
    }
}
