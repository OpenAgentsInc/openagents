//! Runs one explicitly configured authenticated TLS chamber.
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use verse_world::service::{host::Config, net, persistence::Store};
pub mod operations;
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
    let prepared = prepare(&config)?;
    serve_prepared(config, prepared, None, stop).await
}
async fn serve_prepared(
    config: Config,
    (gateway, store): (verse_world::service::auth::Gateway, Option<Store>),
    monitor: Option<verse_world::service::operator::Monitor>,
    stop: impl std::future::Future<Output = ()>,
) -> Result<(), String> {
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
    let exit = if let Some(monitor) = monitor {
        net::serve_monitored(listener, Arc::new(tls), gateway, store, None, monitor, stop).await
    } else {
        match store {
            Some(store) => net::serve_durable(listener, Arc::new(tls), gateway, store, stop).await,
            None => net::serve(listener, Arc::new(tls), gateway, stop).await,
        }
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
#[cfg(unix)]
pub async fn serve_with_operations(
    config: Config,
    root: &Path,
    stop: impl std::future::Future<Output = ()>,
) -> Result<(), String> {
    let prepared = prepare(&config)?;
    let monitor = verse_world::service::operator::Monitor::new(
        operations::build(),
        config.instance,
        prepared.0.content(),
    )?;
    let server = operations::Server::bind(root)?;
    let (stopping, stopped) = tokio::sync::oneshot::channel();
    let host_monitor = monitor.clone();
    let host = async {
        let result = serve_prepared(config, prepared, Some(host_monitor), stop).await;
        if result.is_err()
            && monitor.snapshot().phase == verse_world::service::operator::Phase::Starting
        {
            monitor.phase(
                verse_world::service::operator::Phase::Failed,
                verse_world::service::operator::Reason::TransportFailure,
                std::time::Instant::now(),
            );
        }
        let _ = stopping.send(());
        result
    };
    let operator = async {
        let result = server
            .run(monitor.clone(), async {
                let _ = stopped.await;
            })
            .await;
        if result.is_err() {
            monitor.request_drain();
        }
        result
    };
    let (host, operator) = tokio::join!(host, operator);
    host?;
    operator
}
pub fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("Usage: verse-host CONFIG [--check TICKS | --operations DIR | --backup NEW_DIR | --verify-backup DIR | --restore-backup DIR NEW_DIR]; verse-host --status DIR | --drain DIR")?;
    if path == "--status" || path == "--drain" {
        let root = args.next().ok_or("Missing operations directory")?;
        if args.next().is_some() {
            return Err("Unexpected operations argument".into());
        }
        println!(
            "{}",
            operations::command(Path::new(&root), path == "--drain")?
        );
        return Ok(());
    }
    let mode = args.next();
    let mut config = Config::from_json(&bounded(Path::new(&path), 64 * 1024, false)?)?;
    let mut ticks = None;
    let mut operations = None;
    match mode.as_deref().and_then(|s| s.to_str()) {
        None => {}
        Some("--check") => {
            ticks = Some(
                args.next()
                    .and_then(|n| n.to_str().and_then(|s| s.parse::<u32>().ok()))
                    .filter(|n| *n > 0 && *n <= 108_000)
                    .ok_or("Check ticks must be 1 through 108000")?,
            );
        }
        Some("--prune-history") => {
            if args.next().is_some() {
                return Err("Unexpected history retention argument".into());
            }
            let root = config
                .state_dir
                .take()
                .ok_or("History pruning requires configured durable storage")?;
            if !root.is_dir() {
                return Err("History retention source does not exist".into());
            }
            use verse_world::service::persistence::backup;
            backup::validate_source_path(&root)?;
            let (gateway, _) = prepare(&config)?;
            let store = Store::open(
                &root,
                gateway
                    .content()
                    .ok_or("Missing admitted retention content")?,
                config.instance,
            )?;
            println!(
                "{}",
                serde_json::to_string(&store.prune_history(backup::Budget::default())?)
                    .map_err(|_| "Cannot encode history retention report")?
            );
            return Ok(());
        }
        Some("--operations") => {
            operations = Some(std::path::PathBuf::from(
                args.next().ok_or("Missing operations directory")?,
            ));
        }
        Some(option @ ("--backup" | "--verify-backup" | "--restore-backup")) => {
            let source = std::path::PathBuf::from(args.next().ok_or("Missing backup directory")?);
            let destination = if option == "--restore-backup" {
                Some(std::path::PathBuf::from(
                    args.next().ok_or("Missing new restore directory")?,
                ))
            } else {
                None
            };
            if args.next().is_some() {
                return Err("Unexpected backup argument".into());
            }
            use verse_world::service::persistence::backup::{self, Budget};
            let report = if option == "--backup" {
                let root = config
                    .state_dir
                    .as_ref()
                    .ok_or("Backup requires configured durable storage")?;
                if !root.is_dir() {
                    return Err("Backup source storage does not exist".into());
                }
                backup::validate_source_path(root)?;
                let (_, store) = prepare(&config)?;
                store
                    .ok_or("Backup requires configured durable storage")?
                    .export_backup(&source, Budget::default())?
            } else {
                // Admits configured content without acquiring or creating configured live storage.
                config.state_dir = None;
                let (gateway, _) = prepare(&config)?;
                let content = gateway.content().ok_or("Missing admitted backup content")?;
                match destination {
                    Some(destination) => backup::restore(
                        &source,
                        &destination,
                        content,
                        config.instance,
                        Budget::default(),
                    )?,
                    None => backup::verify(&source, content, config.instance, Budget::default())?,
                }
            };
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|_| "Cannot encode backup report")?
            );
            return Ok(());
        }
        _ => return Err("Unknown host option".into()),
    }
    if args.next().is_some() {
        return Err("Unexpected host argument".into());
    }
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
    let stopping = async move {
        if let Err(error) = shutdown().await {
            *failure.lock().unwrap() = Some(error);
        }
    };
    match operations {
        #[cfg(unix)]
        Some(root) => runtime.block_on(serve_with_operations(config, &root, stopping))?,
        #[cfg(not(unix))]
        Some(_) => return Err("Local operations IPC requires a Unix host".into()),
        None => runtime.block_on(serve(config, stopping))?,
    }
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
