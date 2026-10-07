//! Explicit native runtime and bounded offline deployment operations.
use retail_cloud::boat::{BoatAdapter, BoatConfig};
use retail_service::{
    Service,
    package::{self, Host},
};
use route_contract::Digest;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
fn flag(args: &[String], name: &str) -> Result<PathBuf, String> {
    args.iter()
        .position(|s| s == name)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {name}"))
}
fn inspect(host: &Host, path: &Path) -> Result<serde_json::Value, String> {
    use openagents_wallet::LightningWallet;
    let wallet = openagents_wallet::resident::RemoteWallet::probe(&host.wallet_home)
        .ok_or("selected receiver is unavailable")?;
    let (identity, _) = host
        .identity(
            &wallet.node_id(),
            &std::env::current_exe().map_err(|_| "runtime executable unavailable")?,
            package::COMMIT,
            package::TREE,
        )
        .map_err(|e| e.to_string())?;
    let journal = retail_cloud::journal::Journal::open_read_only(
        host.customer.state.join("lifecycle.sqlite"),
    )
    .map_err(|_| "retained lifecycle schema is unavailable")?;
    let ledger = pay_ledger::Ledger::open_read_only(&host.customer.ledger)
        .map_err(|_| "retained ledger schema is unavailable")?;
    let funded = journal
        .all_funded()
        .map_err(|_| "funded records are unavailable")?;
    if funded.len() > retail_service::types::RECORD_MAX {
        return Err("retained execution inventory exceeds its bound".into());
    }
    let mut rows = Vec::new();
    for f in funded {
        let b = ledger
            .compute_balance(&f.account)
            .map_err(|_| "retained balance is unavailable")?;
        let h = ledger
            .hold(&f.request)
            .map_err(|_| "retained hold is unavailable")?;
        rows.push(serde_json::json!({"funded":f,"balance":[b.credited_msat,b.available_msat,b.held_msat,b.settled_msat,b.released_msat],"hold":h.map(|h|serde_json::json!({"state":h.state.as_str(),"amount_msat":h.request.amount_msat,"charge_msat":h.charge_msat}))}));
    }
    let _ = path;
    Ok(
        serde_json::json!({"identity":identity,"identity_digest":identity.digest(),"accounting":route_contract::digest_of(&rows),"activation":"inspection grants no paid availability; the deployed identity and native funded evidence must pass the runtime gate"}),
    )
}
fn offline(args: &[String]) -> Result<String, String> {
    let command = args.first().map(String::as_str).unwrap_or("");
    if command == "restore" {
        let result = retail_service::backup::restore(
            &flag(args, "--snapshot")?,
            &flag(args, "--state")?,
            &flag(args, "--ledger")?,
            retail_service::http::now(),
        )
        .map_err(|e| e.to_string())?;
        return serde_json::to_string_pretty(&result)
            .map_err(|_| "checkpoint rendering failed".into());
    }
    let path = flag(args, "--config")?;
    let host = package::load(&path).map_err(|e| e.to_string())?;
    let report = inspect(&host, &path)?;
    let result = match command {
        "inspect" => report,
        "snapshot" => {
            let identity = serde_json::from_value(report["identity"].clone())
                .map_err(|_| "runtime identity unavailable")?;
            serde_json::to_value(
                retail_service::backup::snapshot(
                    &host,
                    identity,
                    &flag(args, "--out")?,
                    retail_service::http::now(),
                )
                .map_err(|e| e.to_string())?,
            )
            .map_err(|_| "checkpoint rendering failed")?
        }
        "rollback-check" => {
            let digest = flag(args, "--digest")?
                .to_str()
                .and_then(|s| Digest::try_from(s.to_owned()).ok())
                .ok_or("a canonical candidate digest is required")?;
            retail_service::backup::rollback_check(
                &host,
                &path,
                &flag(args, "--snapshot")?,
                &flag(args, "--candidate")?,
                &digest,
                retail_service::http::now(),
            )
            .map_err(|e| e.to_string())?
        }
        _ => return Err("use --config PATH, inspect, snapshot, restore, or rollback-check".into()),
    };
    serde_json::to_string_pretty(&result).map_err(|_| "operation rendering failed".into())
}
#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = if args.first().is_some_and(|s| s == "--config") {
        run(&args).await
    } else {
        tokio::task::spawn_blocking(move || offline(&args))
            .await
            .map_err(|_| "offline operation failed".to_owned())
            .and_then(|r| r)
            .map(|s| println!("{s}"))
    };
    if let Err(error) = result {
        eprintln!("retail-service: {error}");
        std::process::exit(1);
    }
}
async fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err("usage: retail-service --config /absolute/private/config.json".into());
    }
    let path = PathBuf::from(&args[1]);
    let host = package::load(&path).map_err(|e| e.to_string())?;
    let listen = host.listen;
    let period = Duration::from_secs(host.poll_seconds);
    let service = tokio::task::spawn_blocking(move || {
        retail_service::store_private_dir(&host.customer.state)
            .map_err(|_| "private retail state unavailable")?;
        let boat_state = host.customer.state.join("boat");
        retail_service::store_private_dir(&boat_state)
            .map_err(|_| "private Boat index unavailable")?;
        let key_bytes = package::private_read(&host.boat_key_file, 8192)
            .map_err(|_| "private Boat key unavailable")?;
        let provider_source_digest = Digest::of_bytes(&key_bytes);
        let key = String::from_utf8(key_bytes).map_err(|_| "invalid dedicated Boat key")?;
        let api_key = boat::ApiKey::new(key).map_err(|_| "invalid dedicated Boat key")?;
        let backend = BoatAdapter::new(
            api_key,
            &BoatConfig {
                base_url: host.boat_api_base.clone(),
                org: host.boat_org.clone(),
                state_dir: boat_state,
                retry: None,
            },
        )
        .map_err(|_| "retail Boat binding unavailable")?;
        let wallet = openagents_wallet::resident::RemoteWallet::probe(&host.wallet_home)
            .ok_or("selected receiver is unavailable")?;
        Service::open(host.customer.clone(), Arc::new(backend), Arc::new(wallet))
            .and_then(|s| match host.commercial.clone() {
                Some(config) => s.with_commercial(config),
                None => Ok(s),
            })
            .and_then(|s| s.with_operations(host, &path, provider_source_digest))
            .map(Arc::new)
            .map_err(|_| "retail runtime configuration or custody is unavailable".to_owned())
    })
    .await
    .map_err(|_| "retail binding setup failed")??;
    let _worker = service.spawn_worker(period);
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|_| "retail loopback listener unavailable")?;
    axum::serve(listener, retail_service::http::router(service))
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|_| "retail listener stopped unexpectedly".into())
}
async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}};
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
