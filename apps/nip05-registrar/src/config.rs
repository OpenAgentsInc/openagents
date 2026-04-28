use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;

pub const ENV_LISTEN_ADDR: &str = "NIP05_REGISTRAR_LISTEN_ADDR";
pub const ENV_DATA_FILE: &str = "NIP05_REGISTRAR_DATA_FILE";
pub const ENV_ADMIN_TOKEN: &str = "NIP05_REGISTRAR_ADMIN_TOKEN";
pub const ENV_RESERVED_EXTRA: &str = "NIP05_REGISTRAR_RESERVED_EXTRA";

pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8088";
pub const DEFAULT_DATA_FILE: &str = "apps/nip05-registrar/data/nostr.json";

/// Reserved names that can never be claimed at runtime. Lowercase only.
pub const RESERVED_NAMES: &[&str] = &[
    "_",
    "admin",
    "administrator",
    "agent",
    "api",
    "billing",
    "claim",
    "config",
    "contact",
    "help",
    "info",
    "legal",
    "mail",
    "me",
    "moderator",
    "nostr",
    "openagents",
    "operator",
    "owner",
    "postmaster",
    "press",
    "privacy",
    "registrar",
    "root",
    "security",
    "staff",
    "status",
    "support",
    "sysop",
    "system",
    "test",
    "webmaster",
    "well-known",
    "www",
];

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub data_file: PathBuf,
    pub admin_token: String,
    pub reserved: BTreeSet<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let listen_addr = std::env::var(ENV_LISTEN_ADDR)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string());
        let listen_addr: SocketAddr = listen_addr
            .parse()
            .map_err(|err| format!("invalid {ENV_LISTEN_ADDR}: {err}"))?;

        let data_file = std::env::var(ENV_DATA_FILE)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DATA_FILE.to_string());
        let data_file = PathBuf::from(data_file);

        let admin_token = std::env::var(ENV_ADMIN_TOKEN)
            .map_err(|_| format!("{ENV_ADMIN_TOKEN} must be set"))?;
        let admin_token = admin_token.trim().to_string();
        if admin_token.is_empty() {
            return Err(format!("{ENV_ADMIN_TOKEN} must not be empty"));
        }

        let mut reserved: BTreeSet<String> =
            RESERVED_NAMES.iter().map(|s| (*s).to_string()).collect();
        if let Ok(extra) = std::env::var(ENV_RESERVED_EXTRA) {
            for entry in extra.split(',') {
                let trimmed = entry.trim().to_ascii_lowercase();
                if !trimmed.is_empty() {
                    reserved.insert(trimmed);
                }
            }
        }

        Ok(Self {
            listen_addr,
            data_file,
            admin_token,
            reserved,
        })
    }
}
