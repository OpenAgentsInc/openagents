//! Read-only health probe for the systemd timer.
use lnd_sdk::{
    Client,
    health::{Alert, Policy},
    macaroon::{Macaroon, Role},
    service::{Health, LightningRail},
    transport::Config,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Deserialize)]
struct Settings {
    endpoint: String,
    certificate_sha256: String,
    macaroon: PathBuf,
    output: PathBuf,
    minimum_inbound_msat: u64,
    hot_cap_sats: u64,
}
#[derive(Serialize)]
struct Snapshot {
    healthy: bool,
    observed_at: u64,
    version: &'static str,
    node: Option<String>,
    health: Option<Health>,
    hot_sats: Option<u128>,
    alerts: Vec<Alert>,
    error: Option<String>,
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o640);
    }
    let mut file = options.open(&temporary)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
#[tokio::main]
async fn main() {
    let path = match std::env::args_os().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("a settings file is required");
            std::process::exit(2)
        }
    };
    let settings: Settings = match std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(s) => s,
        None => {
            eprintln!("could not read node settings");
            std::process::exit(2)
        }
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut snapshot = Snapshot {
        healthy: false,
        observed_at: now,
        version: env!("CARGO_PKG_VERSION"),
        node: None,
        health: None,
        hot_sats: None,
        alerts: vec![],
        error: None,
    };
    let check = async {
        let pin: [u8; 32] = hex::decode(&settings.certificate_sha256)
            .map_err(|_| lnd_sdk::Error::CertificatePin)?
            .try_into()
            .map_err(|_| lnd_sdk::Error::CertificatePin)?;
        let client = Client::connect(
            &Config::new(settings.endpoint.clone(), pin),
            Macaroon::from_file(Role::ReadOnly, 202, &settings.macaroon)?,
        )
        .await?;
        let health = client.health().await?;
        let balance = client.balance().await?;
        let hot_sats =
            u128::from(balance.onchain_total_sats) + u128::from(balance.lightning_total_sats);
        let alerts = Policy {
            minimum_inbound_msat: settings.minimum_inbound_msat,
            hot_cap_sats: settings.hot_cap_sats,
        }
        .alerts(&health, hot_sats);
        Ok::<_, lnd_sdk::Error>((client.node_id(), health, hot_sats, alerts))
    }
    .await;
    match check {
        Ok((node, health, hot, alerts)) => {
            snapshot.healthy = alerts.is_empty();
            snapshot.node = Some(node);
            snapshot.health = Some(health);
            snapshot.hot_sats = Some(hot);
            snapshot.alerts = alerts;
        }
        Err(error) => snapshot.error = Some(error.to_string()),
    }
    let bytes = serde_json::to_vec(&snapshot).expect("serializable health");
    if atomic_write(&settings.output, &bytes).is_err() {
        eprintln!("could not save node health");
        std::process::exit(2)
    }
    println!("{}", String::from_utf8_lossy(&bytes));
    if !snapshot.healthy {
        std::process::exit(1)
    }
}
