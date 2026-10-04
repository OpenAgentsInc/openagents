//! Explicit TLS and local-asset configuration for the remote native window.
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    address: SocketAddr,
    server_name: String,
    instance: u64,
    trust_der: PathBuf,
    key_file: PathBuf,
    pack: PathBuf,
    scene: PathBuf,
    dir: PathBuf,
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
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("Usage: verse_remote CONFIG.json")?;
    if args.next().is_some() {
        return Err("Usage: verse_remote CONFIG.json".into());
    }
    let config: Config = serde_json::from_slice(&bounded(std::path::Path::new(&path), 64 * 1024)?)
        .map_err(|_| "Invalid remote configuration")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&config.key_file)
            .map_err(|_| "Cannot inspect signing key file")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("Signing key file must have owner-only permissions".into());
        }
    }
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(bounded(
            &config.trust_der,
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
    let server_name = rustls::pki_types::ServerName::try_from(config.server_name)
        .map_err(|_| "Invalid TLS server name")?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let atlas = verse::imported::chamber::original_portrait_atlas(&config.dir, &pack)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let client = {
        let key_bytes = bounded(&config.key_file, 256)?;
        let text = std::str::from_utf8(&key_bytes).map_err(|_| "Invalid signing key file")?;
        let secret: secp256k1::SecretKey = text
            .trim()
            .parse()
            .map_err(|_| "Invalid signing key file")?;
        let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret);
        runtime.block_on(verse_world::service::client::Client::connect(
            config.address,
            server_name,
            tls,
            config.instance,
            &key,
        ))?
    };
    verse::imported::remote_window::run(client, runtime, pack, atlas, scene, config.dir)
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
