//! The `oa-boat` server. Settings come from the environment (see
//! `docs/cloud/oa-boat.md`); the token and SSH key are Secret Manager values
//! Cloud Run passes in and are never printed.

use oa_boat::api::{Token, router};
use oa_boat::gce::{Rest, TokenSource};
use oa_boat::remote::Ssh;
use oa_boat::sizes::Provisioning;
use oa_boat::{Config, Service};
use std::path::PathBuf;
use std::time::Duration;

fn var(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn secret(name: &str) -> Result<String, String> {
    if let Ok(v) = std::env::var(name)
        && !v.trim().is_empty()
    {
        return Ok(v);
    }
    let file = std::env::var(format!("{name}_FILE")).map_err(|_| format!("{name} is not set"))?;
    std::fs::read_to_string(&file).map_err(|_| format!("{name}_FILE is unreadable"))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let token = secret("OA_BOAT_TOKEN")?;
    if token.trim().len() < 24 {
        return Err("OA_BOAT_TOKEN is too short".into());
    }
    let key = secret("OA_BOAT_SSH_KEY")?;
    let dir = PathBuf::from(var("OA_BOAT_STATE", "/tmp/oa-boat"));
    let control = dir.join("cm");
    std::fs::create_dir_all(&control).map_err(|_| "cannot make the state directory")?;
    let key_path = dir.join("id");
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&key_path)
            .map_err(|_| "cannot write the SSH key")?;
        let mut k = key.trim().to_owned();
        k.push('\n');
        f.write_all(k.as_bytes())
            .map_err(|_| "cannot write the SSH key")?;
    }
    let public = std::process::Command::new("ssh-keygen")
        .arg("-y")
        .arg("-f")
        .arg(&key_path)
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|_| "ssh-keygen cannot run")?;
    let public = String::from_utf8_lossy(&public.stdout).trim().to_owned();
    if !public.starts_with("ssh-") {
        return Err("OA_BOAT_SSH_KEY is not a usable private key".into());
    }
    let zones: Vec<String> = var(
        "OA_BOAT_ZONES",
        "us-central1-a,us-central1-b,us-central1-c,us-central1-f",
    )
    .split(',')
    .map(|z| z.trim().to_owned())
    .filter(|z| !z.is_empty())
    .collect();
    let cfg = Config {
        project: var("OA_BOAT_PROJECT", "openagentsgemini"),
        region: var("OA_BOAT_REGION", "us-central1"),
        zones,
        subnetwork: var("OA_BOAT_SUBNET", "default"),
        tag: var("OA_BOAT_TAG", "oa-boat-sandbox"),
        base_family: var("OA_BOAT_BASE_FAMILY", "oa-coder-host"),
        ssh_public_key: public,
        user: var("OA_BOAT_USER", "user"),
        run_dir: var("OA_BOAT_RUN_DIR", "/run/oa-boat"),
        default_ttl: var("OA_BOAT_TTL_SECONDS", "3600").parse().unwrap_or(3600),
        default_idle: var("OA_BOAT_IDLE_SECONDS", "1800").parse().unwrap_or(1800),
        default_provisioning: Provisioning::parse(&var("OA_BOAT_PROVISIONING", "standard"))
            .unwrap_or(Provisioning::Standard),
        max_active: var("OA_BOAT_MAX_ACTIVE", "50").parse().unwrap_or(50),
        max_run_seconds: var("OA_BOAT_MAX_RUN_SECONDS", "86400")
            .parse()
            .unwrap_or(86400),
    };
    let source = match var(
        "OA_BOAT_TOKEN_SOURCE",
        if std::env::var_os("K_SERVICE").is_some() {
            "metadata"
        } else {
            "gcloud"
        },
    )
    .as_str()
    {
        "gcloud" => TokenSource::Gcloud,
        _ => TokenSource::Metadata,
    };
    let compute = Rest::new(cfg.project.clone(), source);
    let remote = Ssh {
        key: key_path,
        user: cfg.user.clone(),
        control_dir: control,
    };
    let service = Service::new(cfg, compute, remote);
    let reap_every = Duration::from_secs(var("OA_BOAT_REAP_SECONDS", "60").parse().unwrap_or(60));
    let reaper = service.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(reap_every).await;
            reaper.reap().await;
        }
    });
    let app = router(service, Token::new(&token));
    let port = var("PORT", "8080");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|_| "cannot listen")?;
    eprintln!("oa-boat: listening on :{port}");
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}
