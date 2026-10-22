use kiln_host::{
    Result, load, objectives, save,
    worker::{self, Config, Task},
};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("kiln-host: {e}");
        std::process::exit(1)
    }
}
async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let action = args.first().map(String::as_str).unwrap_or("help");
    let option = |name: &str| -> Result<String> {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .ok_or_else(|| format!("missing {name}"))
    };
    if action == "help" || args.iter().any(|s| s == "--help") {
        println!(
            "kiln-host init|publish|publish-serve|objectives|submit|status|stop|serve --state DIR\ninit --config FILE\npublish[-serve] --key FILE --recipient PUBKEY [--relay URL]\nobjectives --objective SLUG\nsubmit --input TASK.json\nstatus|stop --id HEX"
        );
        return Ok(());
    }
    let root = PathBuf::from(option("--state")?);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    if action == "init" {
        if root.join("config.json").exists() {
            return Err("host already initialized".into());
        }
        let c: Config = load(&PathBuf::from(option("--config")?))?;
        c.validate()?;
        pylon::identity::Identity::load_or_create(&root.join("key"))?;
        save(&root.join("config.json"), &c)?;
    }
    if action == "publish" || action == "publish-serve" {
        let key = kiln_git::keys::load(&PathBuf::from(option("--key")?))?;
        let recipient = option("--recipient")?;
        let relay = option("--relay").unwrap_or_else(|_| "wss://relay.openagents.com".into());
        loop {
            let catalog = grid_foreman::objectives::Catalog::parse(
                grid_foreman::objectives::CATALOG.as_bytes(),
            )?;
            let ids =
                objectives::publish(&root, &key, &catalog, &recipient, &relay, kiln_host::now())
                    .await?;
            println!(
                "{}",
                serde_json::to_string(&ids).map_err(|e| e.to_string())?
            );
            if action == "publish" {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
        }
    }
    let key = kiln_git::keys::load(&root.join("key"))?;
    if action == "init" {
        println!("{}", key.key.pubkey());
        return Ok(());
    }
    let config: Config = load(&root.join("config.json"))?;
    config.validate()?;
    if action == "profile" {
        println!(
            "{}",
            serde_json::to_string(&config).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if action == "objectives" {
        let record = objectives::consult(
            &key,
            &config.authority,
            &config.relay,
            &option("--objective")?,
        )
        .await?;
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({"objective":record.objective,"revision":record.revision,"catalog":record.catalog})).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if action == "submit" {
        let task: Task = load(&PathBuf::from(option("--input")?))?;
        let job = worker::submit(&root, &config, &key, task).await?;
        println!(
            "{}",
            serde_json::to_string(&job.summary()).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if action == "status" || action == "stop" {
        let id = option("--id")?;
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid job ID".into());
        }
        let path = root.join("jobs").join(format!("{id}.json"));
        let j: worker::Job = load(&path)?;
        if action == "stop" {
            save(&root.join("jobs").join(format!("{id}.cancel")), &true)?;
        }
        println!(
            "{}",
            serde_json::to_string(&j.summary()).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if action != "serve" {
        return Err("unknown command".into());
    }
    let lock = std::fs::File::create(root.join("worker.lock")).map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| "another native Kiln worker is running")?;
    std::fs::create_dir_all(root.join("jobs")).map_err(|e| e.to_string())?;
    loop {
        let mut paths = std::fs::read_dir(root.join("jobs"))
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let j: worker::Job = load(&path)?;
            if j.phase == "unknown" && j.runner.is_none() {
                break;
            }
            if matches!(j.phase.as_str(), "completed" | "failed" | "stopped") {
                continue;
            }
            match worker::advance(&root, &config, &key, &path).await {
                Ok(j) => eprintln!("{}: {} ({} turns)", j.task.id, j.phase, j.turns),
                Err(e) => {
                    let mut j: worker::Job = load(&path)?;
                    j.error = Some(e.clone());
                    j.phase = if j.phase == "bridging" {
                        "bridging"
                    } else if matches!(j.phase.as_str(), "checking" | "unknown") {
                        "unknown"
                    } else {
                        "failed"
                    }
                    .into();
                    save(&path, &j)?;
                    eprintln!("{}: {e}", j.task.id);
                    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                }
            }
            // One active job at a time, including turns waiting on a runner.
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
