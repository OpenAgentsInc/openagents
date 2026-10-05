//! Runs one explicitly configured authenticated TLS chamber.
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use verse_world::service::{host::Config, net, persistence::Store};
fn bounded(path: &Path, limit: usize, private: bool) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| "Cannot open chamber host input")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect chamber host input")?;
    if !metadata.is_file() {
        return Err("Chamber host input must be a regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if private && metadata.permissions().mode() & 0o077 != 0 {
            return Err("TLS private key must have owner-only permissions".into());
        }
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read chamber host input")?;
    if bytes.len() > limit {
        return Err("Chamber host input exceeds its byte budget".into());
    }
    Ok(bytes)
}
async fn shutdown() -> Result<(), String> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(|_| "Cannot register chamber termination signal")?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(|_| "Cannot await chamber interrupt".into()),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|_| "Cannot await chamber interrupt".into())
    }
}
async fn serve(config: Config) -> Result<(), String> {
    let scene =
        verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024, false)?)?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    verse::imported::remote_content::outfit_models(&pack, &config.outfits)?;
    verse::imported::remote_content::equipment_models(
        &pack,
        &scene,
        &config.outfits,
        &config.equipment,
    )?;
    let content = verse::imported::remote_content::identity(
        &pack,
        &scene,
        config.pack.parent().unwrap_or(Path::new(".")),
    )?;
    let content = config.bind_content(content)?;
    let mut game = config.prepare_game(scene)?;
    verse::imported::props::admit_collision(&pack, &mut game)?;
    let mut store = config
        .state_dir
        .as_ref()
        .map(|path| Store::open(path, content, config.instance))
        .transpose()?;
    let gateway = match store.as_mut().and_then(Store::recover) {
        Some(gateway) => {
            config.validate_recovered_scene(&gateway, &game)?;
            gateway
        }
        None => config.gateway(game)?.with_content(content)?,
    };
    let certificate = rustls::pki_types::CertificateDer::from(bounded(
        &config.certificate_der,
        1024 * 1024,
        false,
    )?);
    let key = rustls::pki_types::PrivateKeyDer::try_from(bounded(
        &config.private_key_der,
        64 * 1024,
        true,
    )?)
    .map_err(|_| "Invalid configured DER private key")?;
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| "Cannot configure TLS protocol versions")?
    .with_no_client_auth()
    .with_single_cert(vec![certificate], key)
    .map_err(|_| "Configured TLS certificate or private key refused")?;
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|_| "Cannot bind configured chamber listener")?;
    println!(
        "Chamber {} listening on {}",
        config.instance,
        listener
            .local_addr()
            .map_err(|_| "Cannot inspect chamber listener")?
    );
    gateway.game().enable_query_profiling();
    let signal_failure = Arc::new(Mutex::new(None));
    let failure = signal_failure.clone();
    let shutdown = async move {
        if shutdown().await.is_err() {
            *failure.lock().unwrap() = Some("Cannot await chamber shutdown signal".to_string());
        }
    };
    let exit = match store {
        Some(store) => net::serve_durable(listener, Arc::new(tls), gateway, store, shutdown).await,
        None => net::serve(listener, Arc::new(tls), gateway, shutdown).await,
    };
    println!(
        "Chamber stopped: {} ticks, {} requests, {} completed connections, {:.6} dropped seconds",
        exit.stats.ticks,
        exit.stats.requests,
        exit.stats.completed_connections,
        exit.stats.dropped_seconds
    );
    println!(
        "Admission: {}",
        serde_json::to_string(&exit.stats.admission)
            .map_err(|_| "Cannot encode chamber admission measurements")?
    );
    if let Some(profile) = exit.gateway.game().query_profile() {
        println!(
            "Collision queries: {}",
            serde_json::to_string(&profile).map_err(|_| "Cannot encode collision measurements")?
        );
    }
    println!(
        "Motor recovery: {} blocked advances, last diagnostic {:?}",
        exit.gateway.game().motor_recovery.blocks,
        exit.gateway.game().motor_recovery.last_diagnostic
    );
    println!(
        "Checkpoint storage: {} commits, {} bytes, {:.6} seconds",
        exit.stats.checkpoint_commits, exit.stats.checkpoint_bytes, exit.stats.checkpoint_seconds
    );
    println!(
        "Storage backlog: {} peak copies, {} refused requests, {} paused ticks, {:.6} paused seconds",
        exit.stats.writer_queue_peak,
        exit.stats.storage_refusals,
        exit.stats.storage_paused_ticks,
        exit.stats.storage_paused_seconds
    );
    for (label, timing) in [
        ("Simulation", &exit.stats.simulation),
        ("Persistence capture", &exit.stats.capture),
        ("Storage commit", &exit.stats.commits),
    ] {
        println!(
            "{}: {} observations, p50/p95/p99 upper bounds {:.6}/{:.6}/{:.6} seconds, maximum {:.6} seconds",
            label,
            timing.count,
            timing.percentile(0.5).unwrap_or(0.),
            timing.percentile(0.95).unwrap_or(0.),
            timing.percentile(0.99).unwrap_or(0.),
            timing.maximum_seconds
        );
    }
    if let Some(error) = exit.failure {
        return Err(error);
    }
    if let Some(error) = signal_failure.lock().unwrap().take() {
        return Err(error);
    }
    Ok(())
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("Usage: verse_host CONFIG.json")?;
    if args.next().is_some() {
        return Err("Usage: verse_host CONFIG.json".into());
    }
    let config = Config::from_json(&bounded(Path::new(&path), 64 * 1024, false)?)?;
    if config.reach().is_some() {
        return Err(
            "verse_host serves TLS only; run `openagents chamber host` for a REACH chamber".into(),
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(serve(config))
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_file_inputs_reject_oversized_and_non_regular_sources() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("certificate.der");
        std::fs::write(&path, [1, 2, 3]).unwrap();
        assert_eq!(bounded(&path, 3, false).unwrap(), vec![1, 2, 3]);
        assert!(bounded(&path, 2, false).is_err());
        assert!(bounded(dir.path(), 16, false).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn tls_private_key_requires_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic-key.der");
        std::fs::write(&path, [1, 2, 3]).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(bounded(&path, 16, true).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(bounded(&path, 16, true).unwrap(), vec![1, 2, 3]);
    }
}
