//! The pinned chamber the Grid's RITUAL arch joins.
//!
//! `~/.verse/ritual.json` (or `verse --ritual CONFIG`) names an authoritative
//! chamber the desktop trusts out of band: its address, instance, DER
//! certificate, and the local content pack and scene whose identity the host
//! checks. Walking through the arch opens the chamber in its own window
//! (`verse --chamber CONFIG`), signed by the player's profile key, which the
//! host admits as a guest or by enrollment. Closing that window returns the
//! player to the arch.
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The configuration file beside the profile keys.
pub const FILE: &str = "ritual.json";

/// A chamber to join over TLS.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The host's `HOST:PORT`.
    pub address: std::net::SocketAddr,
    pub instance: u64,
    /// DER trust certificate, required for TLS and `wss` routes.
    #[serde(default)]
    pub trust_der: Option<PathBuf>,
    /// The name the certificate carries; `localhost` by default.
    #[serde(default)]
    pub server_name: Option<String>,
    /// The runtime pack `openagents chamber pack` wrote.
    pub pack: PathBuf,
    /// The scene, `assets/verse/original/ritual.json`.
    pub scene: PathBuf,
    /// The directory holding the pack's textures and portraits.
    pub dir: PathBuf,
    /// A signing key file in place of the profile's key.
    #[serde(default)]
    pub key_file: Option<PathBuf>,
    /// Explicit host grant for the authenticated reachable channel.
    #[serde(default)]
    pub reach: Option<Reach>,
}

/// Host enrollment values for a direct chamber channel; these grant no role by themselves.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Reach {
    pub host: String,
    pub grant: String,
    pub epoch: u64,
    pub generation: u64,
    /// When present, upgrade the configured address to this exact WebSocket URL.
    pub websocket: Option<String>,
}
impl Config {
    /// Reads and validates a configuration file.
    ///
    /// # Errors
    /// The file cannot be read, is not a configuration, or names a missing
    /// certificate, pack, or scene.
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = bounded(path, 64 * 1024)?;
        let config: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}: invalid ritual configuration: {e}", path.display()))?;
        if let Some(reach) = &config.reach {
            if reach.host.len() != 64
                || reach.grant.len() != 64
                || !reach
                    .host
                    .bytes()
                    .chain(reach.grant.bytes())
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("Malformed chamber reach host or grant identity".into());
            }
            if reach.websocket.as_ref().is_some_and(|url| {
                url.len() > 512 || !(url.starts_with("ws://") || url.starts_with("wss://"))
            }) {
                return Err("Chamber reach WebSocket URL must use ws:// or wss://".into());
            }
        }
        let needs_tls = config.reach.as_ref().is_none_or(|reach| {
            reach
                .websocket
                .as_deref()
                .is_some_and(|url| url.starts_with("wss://"))
        });
        if needs_tls && config.trust_der.as_ref().is_none_or(|file| !file.is_file()) {
            return Err("The chamber trust_der certificate is missing".into());
        }
        for (name, file) in [("pack", &config.pack), ("scene", &config.scene)] {
            if !file.is_file() {
                return Err(format!(
                    "{}: {name} {} is missing",
                    path.display(),
                    file.display()
                ));
            }
        }
        if !config.dir.is_dir() {
            return Err(format!(
                "{}: dir {} is missing",
                path.display(),
                config.dir.display()
            ));
        }
        Ok(config)
    }
}

/// `~/.verse/ritual.json` when it exists, else no arch.
#[must_use]
pub fn default_config() -> Option<PathBuf> {
    let path = crate::identity::home().join(FILE);
    path.is_file().then_some(path)
}

/// Opens the chamber `config` names in its own window, run by this same
/// program, so the Grid window stays and the player returns to it when the
/// chamber closes.
///
/// # Errors
/// The configuration is invalid, or the program cannot start.
pub fn open(config: &Path, profile: &str) -> Result<std::process::Child, String> {
    Config::read(config)?;
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find this program: {e}"))?;
    std::process::Command::new(exe)
        .arg("--chamber")
        .arg(config)
        .arg("--profile")
        .arg(profile)
        .spawn()
        .map_err(|e| format!("Cannot open the chamber: {e}"))
}

fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > limit as u64 {
        return Err(format!("{} is larger than {limit} bytes", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A connected chamber client and the content it was admitted with, ready
/// for a window or a phone surface to mount.
#[cfg(feature = "remote-chamber")]
pub struct Opened {
    pub client: verse_world::service::client::Client,
    pub runtime: tokio::runtime::Runtime,
    pub pack: verse_engine::assets::Pack,
    pub atlas: crate::ui::Atlas,
    pub scene: verse_engine::director::Scene,
    pub dir: PathBuf,
}

/// Reads `config`, loads its pack and scene, and connects to the chamber
/// host as `profile`'s identity (or the config's key file).
///
/// # Errors
/// The configuration is invalid, or the host refuses the key or the content.
#[cfg(feature = "remote-chamber")]
pub fn connect(config: &Path, profile: &str) -> Result<Opened, String> {
    connect_signed(config, || {
        Ok(crate::identity::load_or_create(&crate::identity::home(), profile)?.secret)
    })
}

/// [`connect`] signed by `secret`, such as a phone's world identity, unless
/// the configuration names its own key file.
///
/// # Errors
/// The configuration is invalid, or the host refuses the key or the content.
#[cfg(feature = "remote-chamber")]
pub fn connect_as(config: &Path, secret: secp256k1::SecretKey) -> Result<Opened, String> {
    connect_signed(config, || Ok(secret))
}

#[cfg(feature = "remote-chamber")]
fn connect_signed(
    config: &Path,
    identity: impl FnOnce() -> Result<secp256k1::SecretKey, String>,
) -> Result<Opened, String> {
    use std::sync::Arc;
    let config = Config::read(config)?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let content = crate::imported::remote_content::identity(&pack, &scene, &config.dir)?;
    let atlas = crate::imported::chamber::original_portrait_atlas(&config.dir, &pack)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let mut roots = rustls::RootCertStore::empty();
    if let Some(trust_der) = &config.trust_der {
        roots
            .add(rustls::pki_types::CertificateDer::from(bounded(
                trust_der,
                1024 * 1024,
            )?))
            .map_err(|_| "Invalid DER trust certificate")?;
    }
    let tls = Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| "Cannot configure TLS protocol versions")?
        .with_root_certificates(roots)
        .with_no_client_auth(),
    );
    let server_name = rustls::pki_types::ServerName::try_from(
        config
            .server_name
            .clone()
            .unwrap_or_else(|| "localhost".to_owned()),
    )
    .map_err(|_| "Invalid TLS server name")?;
    let secret = match &config.key_file {
        Some(file) => {
            let text =
                String::from_utf8(bounded(file, 256)?).map_err(|_| "The key file is not text")?;
            text.trim()
                .parse()
                .map_err(|_| "The key file is not a secret key")?
        }
        None => identity()?,
    };
    let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret);
    let client = match &config.reach {
        None => runtime.block_on(verse_world::service::client::Client::connect_with_content(
            config.address,
            server_name,
            tls,
            config.instance,
            Some(content),
            &key,
        ))?,
        Some(reach) => runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(30), async {
                use tokio::net::TcpStream;
                use verse_world::service::{
                    reach::{self, ClientConfig},
                    transport::Transport,
                };
                let socket = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    TcpStream::connect(config.address),
                )
                .await
                .map_err(|_| "Chamber connection timed out")?
                .map_err(|_| "Cannot connect to chamber")?;
                socket
                    .set_nodelay(true)
                    .map_err(|_| "Cannot configure chamber socket")?;
                let stream: Box<dyn Transport> = match reach.websocket.as_deref() {
                    Some(url) if url.starts_with("wss://") => {
                        let stream = tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            verse_world::service::client::connect_tls_stream(
                                socket,
                                tls,
                                server_name,
                            ),
                        )
                        .await
                        .map_err(|_| "Chamber TLS timed out")??;
                        Box::new(
                            reach::websocket::client(url, stream)
                                .await
                                .map_err(|e| format!("Chamber WebSocket refused: {e}"))?,
                        )
                    }
                    Some(url) => Box::new(
                        reach::websocket::client(url, socket)
                            .await
                            .map_err(|e| format!("Chamber WebSocket refused: {e}"))?,
                    ),
                    None => Box::new(socket),
                };
                reach::join(
                    stream,
                    &ClientConfig {
                        device: secret,
                        host: reach.host.clone(),
                        grant: reach.grant.clone(),
                        epoch: reach.epoch,
                        generation: reach.generation,
                        timeout: std::time::Duration::from_secs(10),
                    },
                    config.instance,
                    Some(content),
                )
                .await
            })
            .await
            .map_err(|_| "Chamber reachable connection timed out")?
        })?,
    };
    Ok(Opened {
        client,
        runtime,
        pack,
        atlas,
        scene,
        dir: config.dir,
    })
}

/// Connects to the chamber `config` names and runs the desktop window for it.
///
/// # Errors
/// The configuration is invalid, the host refuses the key or the content,
/// or this build has no chamber client.
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
pub fn run(config: &Path, profile: &str) -> Result<(), String> {
    let opened = connect(config, profile)?;
    crate::imported::remote_window::run(
        opened.client,
        opened.runtime,
        opened.pack,
        opened.atlas,
        opened.scene,
        opened.dir,
    )
}

/// This build has no chamber client.
///
/// # Errors
/// Always: build `verse` with `--features remote-chamber,imported-desktop`.
#[cfg(not(all(feature = "remote-chamber", feature = "imported-desktop")))]
pub fn run(_config: &Path, _profile: &str) -> Result<(), String> {
    Err("This build has no chamber client; build verse with --features remote-chamber,imported-desktop".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_needs_its_files() {
        let dir = std::env::temp_dir().join(format!("verse-ritual-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        std::fs::write(
            &path,
            format!(
                r#"{{"address":"127.0.0.1:47831","instance":7,"trust_der":"{d}/cert.der","pack":"{d}/pack.json","scene":"{d}/scene.json","dir":"{d}"}}"#,
                d = dir.display()
            ),
        )
        .unwrap();
        assert!(Config::read(&path).unwrap_err().contains("trust_der"));
        for name in ["cert.der", "pack.json", "scene.json"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let config = Config::read(&path).unwrap();
        assert_eq!(config.instance, 7);
        assert!(config.server_name.is_none() && config.key_file.is_none());
        assert!(open(&dir.join("missing.json"), "default").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
