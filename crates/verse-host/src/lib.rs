//! Runs one explicitly configured authenticated TLS chamber.
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use verse_world::service::{host::Config, net, persistence::Store};
pub fn bounded(path: &Path, limit: usize, private: bool) -> Result<Vec<u8>, String> {
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
/// Admits scene, assets, collision, rights, and any matching durable checkpoint.
pub fn prepare(
    config: &Config,
) -> Result<(verse_world::service::auth::Gateway, Option<Store>), String> {
    config.validate()?;
    let scene =
        verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024, false)?)?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    let content = verse_content::remote_content::admit(
        &pack,
        &scene,
        config.pack.parent().unwrap_or(Path::new(".")),
        &config.outfits,
        &config.equipment,
    )?;
    let content = config.bind_content(content)?;
    let mut game = config.prepare_game(scene)?;
    verse_content::collision::admit_collision(&pack, &mut game)?;
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
    Ok((gateway, store))
}
pub async fn serve(
    config: Config,
    stop: impl std::future::Future<Output = ()>,
) -> Result<(), String> {
    let (gateway, store) = prepare(&config)?;
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
    let exit = match store {
        Some(store) => net::serve_durable(listener, Arc::new(tls), gateway, store, stop).await,
        None => net::serve(listener, Arc::new(tls), gateway, stop).await,
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
        "Movement expiry: {}",
        serde_json::to_string(&exit.gateway.game().movement_expiry)
            .map_err(|_| "Cannot encode movement expiry measurements")?
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
        ("Admitted read projection", &exit.stats.read_projection),
        ("Running checkpoint copy", &exit.stats.checkpoint_copy),
        ("Storage commit", &exit.stats.commits),
        ("Commit preparation", &exit.stats.commit_preparation),
        ("Reward history sync", &exit.stats.history_sync),
        ("Journal encoding", &exit.stats.journal_encoding),
        ("Journal write", &exit.stats.journal_write),
        ("Journal sync", &exit.stats.journal_sync),
        ("Snapshot compaction", &exit.stats.snapshot_compaction),
    ] {
        println!(
            "{}: {} observations, p50/p95/p99 upper bounds {:.6}/{:.6}/{:.6} seconds, maximum {:.6} seconds, total {:.6} seconds",
            label,
            timing.count,
            timing.percentile(0.5).unwrap_or(0.),
            timing.percentile(0.95).unwrap_or(0.),
            timing.percentile(0.99).unwrap_or(0.),
            timing.maximum_seconds,
            timing.total_seconds
        );
    }
    if let Some(error) = exit.failure {
        return Err(error);
    }
    Ok(())
}
pub fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("Usage: verse-host CONFIG.json [--check TICKS]")?;
    let mode = args.next();
    let ticks = if mode.as_deref() == Some(std::ffi::OsStr::new("--check")) {
        Some(
            args.next()
                .and_then(|n| n.to_str().and_then(|s| s.parse::<u32>().ok()))
                .filter(|n| *n > 0 && *n <= 108_000)
                .ok_or("Check ticks must be 1 through 108000")?,
        )
    } else if mode.is_none() {
        None
    } else {
        return Err("Unknown host option".into());
    };
    if args.next().is_some() {
        return Err("Unexpected host argument".into());
    }
    let config = Config::from_json(&bounded(Path::new(&path), 64 * 1024, false)?)?;
    if config.transport != (verse_world::service::host::Transport::Tls {}) {
        return Err("verse-host serves TLS; use openagents chamber host for REACH".into());
    }
    if let Some(ticks) = ticks {
        let (mut gateway, _) = prepare(&config)?;
        let mut schedule = verse_engine::core::FixedSchedule::new(30, 3)?;
        for _ in 0..ticks {
            let batch = schedule.advance(1. / 30.)?;
            for _ in 0..batch.steps {
                gateway.tick(batch.seconds)?;
            }
        }
        let checkpoint = gateway.checkpoint()?;
        let content = gateway
            .content()
            .ok_or("Missing admitted content identity")?;
        verse_world::service::auth::Gateway::restore(&checkpoint, content, config.instance)?;
        println!(
            "{}",
            serde_json::json!({"instance": config.instance, "ticks": schedule.tick,
            "content": content, "checkpoint_bytes": checkpoint.len(), "actors": gateway.game().frame().actors.len()})
        );
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let signal_failure = Arc::new(Mutex::new(None));
    let failure = signal_failure.clone();
    runtime.block_on(serve(config, async move {
        if let Err(error) = shutdown().await {
            *failure.lock().unwrap() = Some(error);
        }
    }))?;
    if let Some(error) = signal_failure.lock().unwrap().take() {
        return Err(error);
    }
    Ok(())
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
