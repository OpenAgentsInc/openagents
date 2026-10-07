//! Explicitly configured resident retail HTTP service.

use retail_cloud::boat::{BoatAdapter, BoatConfig};
use retail_service::{Service, types::Config};
use serde::Deserialize;
use std::fs;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    customer: Config,
    listen: SocketAddr,
    boat_api_base: String,
    boat_org: Option<String>,
    boat_key_file: PathBuf,
    wallet_home: PathBuf,
    poll_seconds: u64,
}
fn private_read(path: &Path) -> Result<String, String> {
    if !path.is_absolute() {
        return Err("an absolute private file is required".into());
    }
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component);
        if fs::symlink_metadata(&prefix)
            .map_err(|_| "private file is unavailable")?
            .file_type()
            .is_symlink()
        {
            return Err("symlinks are not admitted for private configuration".into());
        }
    }
    let meta = fs::metadata(path).map_err(|_| "private file is unavailable")?;
    if !meta.is_file() || meta.len() > 256 * 1024 || meta.permissions().mode() & 0o077 != 0 {
        return Err("a bounded mode-0600 private file is required".into());
    }
    fs::read_to_string(path).map_err(|_| "private file is unavailable".into())
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("retail-service: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 || args[1] != "--config" {
        return Err("usage: retail-service --config /absolute/private/config.json".into());
    }
    let host: Host = serde_json::from_str(&private_read(Path::new(&args[2]))?)
        .map_err(|_| "invalid private retail configuration")?;
    if !host.listen.ip().is_loopback() || !(1..=30).contains(&host.poll_seconds) {
        return Err(
            "the retail listener must be loopback and the worker interval must be 1–30 seconds"
                .into(),
        );
    }
    if !host.wallet_home.is_absolute() {
        return Err("an absolute dedicated receiver wallet path is required".into());
    }
    // Live adapters are constructed on a blocking thread. No operator HOME,
    // BOAT_API_KEY, or customer model credential is imported.
    let listen = host.listen;
    let period = Duration::from_secs(host.poll_seconds);
    let service = tokio::task::spawn_blocking(move || {
        retail_service::store_private_dir(&host.customer.state)
            .map_err(|_| "private retail state is unavailable")?;
        let boat_state = host.customer.state.join("boat");
        retail_service::store_private_dir(&boat_state)
            .map_err(|_| "private Boat index is unavailable")?;
        let mut key = private_read(&host.boat_key_file)?;
        let api_key = boat::ApiKey::new(std::mem::take(&mut key))
            .map_err(|_| "invalid dedicated retail Boat key")?;
        let backend = BoatAdapter::new(
            api_key,
            &BoatConfig {
                base_url: host.boat_api_base,
                org: host.boat_org,
                state_dir: boat_state,
                retry: None,
            },
        )
        .map_err(|_| "retail Boat binding is unavailable")?;
        let wallet = openagents_wallet::resident::RemoteWallet::probe(&host.wallet_home)
            .ok_or("retail receiver wallet is unavailable")?;
        Service::open(host.customer, Arc::new(backend), Arc::new(wallet))
            .map(Arc::new)
            .map_err(|_| "retail service configuration or state is unavailable".to_owned())
    })
    .await
    .map_err(|_| "retail binding setup failed")??;
    let _worker = service.spawn_worker(period);
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|_| "retail loopback listener is unavailable")?;
    axum::serve(listener, retail_service::http::router(service))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "retail listener stopped unexpectedly".into())
}
