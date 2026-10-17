//! The private credential service. It has no guest execution or public listener
//! outside the configured service ingress, and never logs request bodies.
use computer_credentials::{
    Broker, Secret, SystemClock, gateway, secret_manager::GoogleSecretManager,
};
use oa_computers::{
    Db, Store,
    credential_broker::{Authority, Repository, control_router},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    sources: PathBuf,
    signing_key: PathBuf,
    native_token: PathBuf,
    public_origin: String,
    native_origin: String,
    #[serde(default)]
    root_ca: Option<PathBuf>,
    deployment: String,
    nodes: BTreeMap<String, String>,
    #[serde(default)]
    certificate_authority: Option<CertificateAuthority>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CertificateAuthority {
    authority: String,
    caller_token: PathBuf,
}
fn private(path: &Path, max: u64) -> Result<Vec<u8>, &'static str> {
    use std::{
        io::Read,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };
    let f = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "A private configuration file couldn't be read.")?;
    let metadata = f
        .metadata()
        .map_err(|_| "A private configuration file couldn't be read.")?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 || metadata.len() > max {
        return Err("A private configuration file has invalid permissions or size.");
    }
    let mut bytes = Vec::new();
    f.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "A private configuration file couldn't be read.")?;
    if bytes.len() as u64 > max {
        return Err("A private configuration file is too large.");
    }
    Ok(bytes)
}
fn secret(path: &Path) -> Result<Secret, &'static str> {
    let mut bytes = private(path, 16384)?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Secret::new(bytes).map_err(|_| "A credential file is invalid.")
}
async fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .map(PathBuf::from)
        .ok_or("Provide the private broker configuration path.")?;
    let initialize = match args.next() {
        None => false,
        Some(flag) if flag == "--initialize" => true,
        _ => return Err("Use CONFIG.json [--initialize].".into()),
    };
    if args.next().is_some() {
        return Err("Use CONFIG.json [--initialize].".into());
    }
    let config: Configuration = serde_json::from_slice(&private(&path, 262144)?)
        .map_err(|_| "The broker configuration is invalid.")?;
    let dsn = zeroize::Zeroizing::new(
        std::env::var("OA_CREDENTIAL_DATABASE_URL")
            .map_err(|_| "The broker database is not configured.")?,
    );
    let db = Db::connect_existing(&dsn)
        .await
        .map_err(|_| "The broker database is unavailable.")?;
    let repository = Arc::new(
        if initialize {
            Repository::new(db.clone()).await
        } else {
            Repository::existing(db.clone()).await
        }
        .map_err(|_| "The broker database is unavailable.")?,
    );
    if initialize {
        return Ok(());
    }
    repository
        .revoke_after_restart(&config.deployment)
        .await
        .map_err(|_| "Old credential references couldn't be disabled.")?;
    let mut authority = Authority::new(
        repository.clone(),
        Store::new(db),
        config.sources,
        &config.public_origin,
        &config.native_origin,
        secret(&config.native_token)?,
        &config.deployment,
    )
    .map_err(|_| "The broker configuration is invalid.")?;
    if let Some(path) = config.root_ca {
        authority = authority
            .with_root_certificate(&private(&path, 65536)?)
            .map_err(|_| "The private service CA is invalid.")?;
    }
    let authority = Arc::new(authority);
    let sources = authority
        .sources()
        .map_err(|_| "The source connections couldn't be read.")?;
    let broker = Arc::new(
        Broker::new(
            repository,
            authority.clone(),
            Arc::new(
                GoogleSecretManager::from_metadata()
                    .map_err(|_| "Secret Manager is unavailable.")?,
            ),
            Arc::new(SystemClock),
            sources,
            secret(&config.signing_key)?,
        )
        .map_err(|_| "The broker configuration is invalid.")?,
    );
    let mut nodes = BTreeMap::new();
    for (hash, node) in config.nodes {
        if !computer_credentials::digest(&hash) || !computer_credentials::opaque(&node, 128) {
            return Err("A node identity is invalid.".into());
        }
        let mut bytes = [0; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16)
                .map_err(|_| "A node identity is invalid.")?;
        }
        nodes.insert(bytes, node);
    }
    if nodes.is_empty() || nodes.len() > 128 {
        return Err("Configure the allowed node identities.".into());
    }
    let cleanup = authority.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let _ = cleanup.prune_grants().await;
        }
    });
    let gateway = Arc::new(gateway::Service {
        broker,
        authority: authority.clone(),
        nodes,
    });
    let mut router = control_router(authority).merge(gateway.router());
    if let Some(ca) = config.certificate_authority {
        use computer_credentials::certificate_authority::{GoogleCa, Service};
        let issuer = GoogleCa::from_metadata(&ca.authority)
            .map_err(|_| "The certificate authority settings are invalid.")?;
        let service = Arc::new(
            Service::new(issuer, secret(&ca.caller_token)?)
                .map_err(|_| "The certificate authority settings are invalid.")?,
        );
        router = router.merge(service.router());
    }
    let port = std::env::var("PORT").unwrap_or_else(|_| "8791".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|_| "The broker couldn't start.")?;
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "The broker stopped unexpectedly.".into())
}
#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
