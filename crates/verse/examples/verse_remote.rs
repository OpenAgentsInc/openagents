//! Explicit transport and local-asset configuration for the remote native
//! window. Over TLS the configuration names the trusted certificate and a
//! signing key file the host enrolls. Over REACH (`reach`) it names the
//! Coder host's key and the computers store (`openagents computer`'s, by
//! default `~/.openagents/coder-computers`) that holds this device's key and
//! the grant the host signed for it; the grant must hold the `world` right.
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    address: SocketAddr,
    #[serde(default)]
    server_name: Option<String>,
    instance: u64,
    #[serde(default)]
    trust_der: Option<PathBuf>,
    #[serde(default)]
    key_file: Option<PathBuf>,
    #[serde(default)]
    reach: Option<Reach>,
    pack: PathBuf,
    scene: PathBuf,
    dir: PathBuf,
    #[serde(default)]
    record: Option<verse::imported::remote_record::Options>,
}
/// A chamber served over a NIP-REACH channel.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reach {
    /// The Coder host's x-only public key.
    host: String,
    /// The computers store with `device.key` and `computers.json`.
    store: PathBuf,
    /// Open the channel over a WebSocket upgrade.
    #[serde(default)]
    websocket: bool,
}
fn bounded(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| "Cannot open remote configuration input")?;
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect remote configuration input")?
        .is_file()
    {
        return Err("Remote configuration input must be a regular file".into());
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read remote configuration input")?;
    if bytes.len() > limit {
        return Err("Remote configuration input exceeds its size bound".into());
    }
    Ok(bytes)
}
fn secret_file(path: &std::path::Path) -> Result<secp256k1::SecretKey, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path)
            .map_err(|_| "Cannot inspect signing key file")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("Signing key file must have owner-only permissions".into());
        }
    }
    let key_bytes = bounded(path, 256)?;
    let text = std::str::from_utf8(&key_bytes).map_err(|_| "Invalid signing key file")?;
    text.trim()
        .parse()
        .map_err(|_| "Invalid signing key file".into())
}
/// The channel configuration for `reach`: this device's key and the grant
/// `computers.json` holds from the host.
fn reach_config(
    reach: &Reach,
    instance: u64,
) -> Result<verse_world::service::reach::ClientConfig, String> {
    let device = secret_file(&reach.store.join("device.key"))?;
    let saved: serde_json::Value = serde_json::from_slice(&bounded(
        &reach.store.join("computers.json"),
        4 * 1024 * 1024,
    )?)
    .map_err(|_| "The saved computers record is unreadable")?;
    let grant = saved["hosts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|host| &host["access"]["grant"])
        .find(|grant| grant["host"].as_str() == Some(reach.host.as_str()))
        .ok_or("This device holds no grant from that host")?;
    let device_key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &device)
        .x_only_public_key()
        .0
        .to_string();
    if grant["device"].as_str() != Some(device_key.as_str()) {
        return Err("The saved grant names another device key".into());
    }
    Ok(verse_world::service::reach::ClientConfig {
        device,
        host: reach.host.clone(),
        grant: grant["grant"]
            .as_str()
            .ok_or("The saved grant has no ID")?
            .to_owned(),
        epoch: grant["epoch"]
            .as_u64()
            .ok_or("The saved grant has no epoch")?,
        // A chamber's channel names its instance as the generation.
        generation: instance,
        timeout: Duration::from_secs(10),
    })
}
async fn join_reach(
    reach: &Reach,
    address: SocketAddr,
    instance: u64,
    content: [u8; 32],
) -> Result<verse_world::service::client::Client, String> {
    use verse_world::service::reach;
    let config = reach_config(reach, instance)?;
    let socket = tokio::net::TcpStream::connect(address)
        .await
        .map_err(|_| "Cannot reach the chamber")?;
    let _ = socket.set_nodelay(true);
    if reach.websocket {
        let socket = reach::websocket::client(&format!("ws://{address}/"), socket)
            .await
            .map_err(|e| format!("Chamber channel refused: {e}"))?;
        reach::join(socket, &config, instance, Some(content)).await
    } else {
        reach::join(socket, &config, instance, Some(content)).await
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("Usage: verse_remote CONFIG.json")?;
    if args.next().is_some() {
        return Err("Usage: verse_remote CONFIG.json".into());
    }
    let config: Config = serde_json::from_slice(&bounded(std::path::Path::new(&path), 64 * 1024)?)
        .map_err(|_| "Invalid remote configuration")?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let content = verse::imported::remote_content::identity(&pack, &scene, &config.dir)?;
    let atlas = verse::imported::chamber::original_portrait_atlas(&config.dir, &pack)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let client = match &config.reach {
        Some(reach) => {
            if config.trust_der.is_some()
                || config.server_name.is_some()
                || config.key_file.is_some()
            {
                return Err("A REACH chamber takes no trust_der, server_name, or key_file".into());
            }
            runtime.block_on(join_reach(reach, config.address, config.instance, content))?
        }
        None => {
            let trust = config
                .trust_der
                .as_ref()
                .ok_or("trust_der is required over TLS")?;
            let server_name = config
                .server_name
                .clone()
                .ok_or("server_name is required over TLS")?;
            let key_file = config
                .key_file
                .as_ref()
                .ok_or("key_file is required over TLS")?;
            let mut roots = rustls::RootCertStore::empty();
            roots
                .add(rustls::pki_types::CertificateDer::from(bounded(
                    trust,
                    1024 * 1024,
                )?))
                .map_err(|_| "Invalid configured DER trust certificate")?;
            let tls = Arc::new(
                rustls::ClientConfig::builder_with_provider(Arc::new(
                    rustls::crypto::ring::default_provider(),
                ))
                .with_safe_default_protocol_versions()
                .map_err(|_| "Cannot configure TLS protocol versions")?
                .with_root_certificates(roots)
                .with_no_client_auth(),
            );
            let server_name = rustls::pki_types::ServerName::try_from(server_name)
                .map_err(|_| "Invalid TLS server name")?;
            let secret = secret_file(key_file)?;
            let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret);
            runtime.block_on(verse_world::service::client::Client::connect_with_content(
                config.address,
                server_name,
                tls,
                config.instance,
                Some(content),
                &key,
            ))?
        }
    };
    verse::imported::remote_window::run_recorded(
        client,
        runtime,
        pack,
        atlas,
        scene,
        config.dir,
        config.record,
    )
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
