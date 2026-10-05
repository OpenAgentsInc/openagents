//! Bounded authenticated movement and combat load using the native replication worker.
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use verse_world::service::client::Client;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    address: SocketAddr,
    server_name: String,
    instance: u64,
    trust_der: PathBuf,
    pack: PathBuf,
    scene: PathBuf,
    dir: PathBuf,
    keys: Vec<PathBuf>,
    seconds: u32,
    output: PathBuf,
    #[serde(default)]
    movement_frames: bool,
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

#[path = "common/battle_player.rs"]
mod battle_player;
use battle_player::player;

async fn run(config: Config) -> Result<(), String> {
    if !(1..=20).contains(&config.keys.len()) || !(1..=120).contains(&config.seconds) {
        return Err("Load count or duration exceeds bounds".into());
    }
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let content = verse_content::remote_content::identity(&pack, &scene, &config.dir)?;
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(bounded(
            &config.trust_der,
            1024 * 1024,
        )?))
        .map_err(|_| "Invalid load trust certificate")?;
    let tls = Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| "Invalid load TLS versions")?
        .with_root_certificates(roots)
        .with_no_client_auth(),
    );
    let name = rustls::pki_types::ServerName::try_from(config.server_name)
        .map_err(|_| "Invalid load server name")?;
    let mut clients = Vec::new();
    for path in config.keys {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&path)
                .map_err(|_| "Cannot inspect load key")?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("Load signing key must have owner-only permissions".into());
            }
        }
        let bytes = bounded(&path, 256)?;
        let key: secp256k1::SecretKey = std::str::from_utf8(&bytes)
            .map_err(|_| "Invalid load key")?
            .trim()
            .parse()
            .map_err(|_| "Invalid load key")?;
        let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &key);
        clients.push(
            Client::connect_with_content(
                config.address,
                name.clone(),
                tls.clone(),
                config.instance,
                Some(content),
                &key,
            )
            .await?,
        );
    }
    println!("Load ready: {} authenticated players", clients.len());
    let end = tokio::time::Instant::now() + Duration::from_secs(config.seconds as u64);
    let mut tasks = tokio::task::JoinSet::new();
    for (index, client) in clients.into_iter().enumerate() {
        tasks.spawn(player(client, index, end, config.movement_frames, false));
    }
    let mut rows = Vec::new();
    while let Some(result) = tasks.join_next().await {
        rows.push(match result {
            Ok(Ok(row)) => row,
            Ok(Err(_)) => {
                serde_json::json!({"player":null,"status":"failed","failure_stage":"player"})
            }
            Err(_) => {
                serde_json::json!({"player":null,"status":"failed","failure_stage":"player_task"})
            }
        });
    }
    rows.sort_by_key(|r| r["player"].as_u64());
    let failed = rows.iter().any(|row| row["status"] != "complete");
    let receipt = serde_json::json!({"schema":"verse.multiplayer.load.v3","movement_frames_requested":config.movement_frames,"status":if failed {"failed"} else {"complete"},"seconds":config.seconds,"players":rows,"limits":["Headless authenticated clients measure transport and authority load, not rendering.","Headless intervals use confirmed server time plus the existing bounded authority lookahead, without native prediction or rendering; the receipt declares the requested movement mode.","Binding-to-outcome includes server processing and client delivery, not isolated RTT.","Timing retains at most 8192 latency samples and two 128-observation producer windows and eight context-transition histories per player. Each transition retains up to 128 preceding observations. The windows can overlap."]});
    std::fs::write(
        config.output,
        serde_json::to_vec_pretty(&receipt).map_err(|_| "Cannot encode load receipt")?,
    )
    .map_err(|_| "Cannot write load receipt")?;
    if failed {
        return Err("Load failed; partial player measurements retained in the receipt".into());
    }
    Ok(())
}
fn main() {
    let result = (|| {
        let mut args = std::env::args_os().skip(1);
        let path = args.next().ok_or("Usage: verse_load CONFIG.json")?;
        if args.next().is_some() {
            return Err("Usage: verse_load CONFIG.json".into());
        }
        let config: Config =
            serde_json::from_slice(&bounded(std::path::Path::new(&path), 64 * 1024)?)
                .map_err(|_| "Invalid load configuration")?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Cannot create load runtime")?
            .block_on(run(config))
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
