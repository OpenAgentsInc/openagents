//! `eval-runner`: the hosted eval runner's command.
//!
//! ```text
//! eval-runner serve            listen for hosted run requests and answer them
//! eval-runner check            load the configuration, the catalog, and the
//!                              sandbox, and say what would run; runs nothing
//! eval-runner pubkey           print the runner's public key
//! eval-runner release DIR...   release each catalog extension's test set
//!                              (its evals/) as the runner, once
//! eval-runner usage [--since YYYY-MM-DD] [--by key|action|subject|day|outcome]
//!                   [--json] [--dir DIR]
//!                              summarize the usage log (one line per job)
//! ```
//!
//! The configuration is the environment (`deploy/eval-runner/`); read
//! `docs/deployment/eval-runner.md`.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use coder::relay::liveness::Liveness;
use eval_runner::config::{Config, load_identity};
use eval_runner::runner::{PROBE_VAR, RENEW_VAR, Runner};
use eval_runner::wire::{Blobs, Blossom, Bucket, Relay, Wire};

const USAGE: &str = "usage: eval-runner serve | check | pubkey | release DIR... | usage [--since YYYY-MM-DD] [--by key|action|subject|day|outcome] [--json] [--dir DIR]";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("serve") => serve().await,
        Some("check") => check().map(|()| ExitCode::SUCCESS),
        Some("pubkey") => pubkey(),
        Some("release") => release(&args[1..]).await,
        Some("usage") => usage(&args[1..]),
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(64))
        }
    };
    outcome.unwrap_or_else(|error| {
        eprintln!("eval-runner: {error}");
        ExitCode::FAILURE
    })
}

fn pubkey() -> Result<ExitCode, String> {
    let path = std::env::var_os("EVAL_RUNNER_KEY_FILE").map_or_else(
        || {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".openagents/nostr/eval-runner-key")
        },
        std::path::PathBuf::from,
    );
    println!("{}", load_identity(&path)?.pubkey());
    Ok(ExitCode::SUCCESS)
}

fn runner() -> Result<Arc<Runner>, String> {
    let config = Config::from_env()?;
    let identity = load_identity(&config.key_file)?;
    let wire_identity = Arc::new(load_identity(&config.key_file)?);
    let wire: Arc<dyn Wire> = Arc::new(Relay::new(&config.relay, wire_identity));
    let blobs: Arc<dyn Blobs> = match &config.bucket {
        Some((bucket, gcloud)) => Arc::new(Bucket::new(
            config
                .blossom
                .as_deref()
                .ok_or("EVAL_RUNNER_BUCKET needs EVAL_RUNNER_BLOSSOM, its public read base")?,
            bucket,
            gcloud,
        )?),
        None => Arc::new(Blossom::new(config.blossom.as_deref(), &config.relay)?),
    };
    Runner::new(config, identity, wire, blobs)
}

fn check() -> Result<(), String> {
    let config = Config::from_env()?;
    ext_eval::sandbox::confinement_available()
        .map_err(|error| format!("unconfined_host: {error}"))?;
    let runner = runner()?;
    println!("runner   {}", runner.pubkey());
    if runner.pubkey() != nostr::eval_ext::hosted::RUNNER {
        println!(
            "warning  the app sends to {}; this key is another",
            nostr::eval_ext::hosted::RUNNER
        );
    }
    println!("relay    {}", config.relay);
    println!(
        "blobs    {}",
        match &config.bucket {
            Some((bucket, _)) => bucket.clone(),
            None => config
                .blossom
                .clone()
                .unwrap_or_else(|| config.relay.clone()),
        }
    );
    println!("door     {}", config.door.label());
    println!(
        "decision {}",
        config
            .decision
            .as_ref()
            .map_or_else(|| "none".to_string(), |pin| pin.doors().join(" → "))
    );
    println!(
        "agent    {} ({})",
        config.coder.display(),
        runner.agent().digest
    );
    for tool in &runner.catalog().tools {
        println!("catalog  {} {}", tool.name, tool.definition.id);
    }
    println!("limits   {}", limits_line(&config.limits));
    println!("usage    {}", config.usage_dir().display());
    println!(
        "admission {}",
        if config.closed() {
            "closed (remove the closed file to open it)"
        } else {
            "open"
        }
    );
    Ok(())
}

/// The startup line for the runner's bounds: no usage limit unless an
/// operator set an emergency brake.
fn limits_line(limits: &eval_runner::config::Limits) -> String {
    let brake = match (limits.runs_per_trainer, limits.turns_per_day) {
        (None, None) => "no usage limit".to_string(),
        (runs, turns) => format!(
            "emergency brake on: {} runs per trainer per day, {} turns per day",
            runs.map_or_else(|| "any".to_string(), |n| n.to_string()),
            turns.map_or_else(|| "any".to_string(), |n| n.to_string()),
        ),
    };
    format!(
        "{brake}; {} suites and {} runs at once",
        limits.jobs, limits.concurrency
    )
}

/// `eval-runner usage`: the usage log, grouped. It reads the log only, so
/// it needs no key or catalog.
fn usage(args: &[String]) -> Result<ExitCode, String> {
    use eval_runner::usage::{By, read, stats, table};
    let mut since = None;
    let mut by = By::Day;
    let mut json = false;
    let mut dir = std::env::var_os("EVAL_RUNNER_STATE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".openagents/eval-runner")
        })
        .join("usage");
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--since" => since = Some(value()?),
            "--by" => by = By::parse(&value()?)?,
            "--dir" => dir = std::path::PathBuf::from(value()?),
            "--json" => json = true,
            other => return Err(format!("usage: unknown argument {other}")),
        }
    }
    let found = read(&dir, since.as_deref())?;
    let rows = stats(&found.records, by);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
        );
    } else {
        print!("{}", table(&rows, by));
    }
    if found.unreadable > 0 {
        eprintln!("{} lines did not read as records", found.unreadable);
    }
    Ok(ExitCode::SUCCESS)
}

async fn release(dirs: &[String]) -> Result<ExitCode, String> {
    if dirs.is_empty() {
        return Err("release needs at least one extension directory".into());
    }
    let runner = runner()?;
    for (name, release) in runner.release_tools().await? {
        println!(
            "{name} (tool) {}",
            release["id"].as_str().unwrap_or_default()
        );
    }
    for dir in dirs {
        let root = std::path::Path::new(dir);
        let tool = eval_runner::catalog::resolve(root)?;
        let package = format!("{}-tests", tool.package.slug);
        let release = runner.release_extension_suite(root, &package).await?;
        println!(
            "{} {}:{package} {}",
            tool.name,
            runner.pubkey(),
            release["id"].as_str().unwrap_or_default()
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// The shortest and longest wait between reconnects.
const RECONNECT: (Duration, Duration) = (Duration::from_secs(1), Duration::from_secs(60));

async fn serve() -> Result<ExitCode, String> {
    let config = Config::from_env()?;
    ext_eval::sandbox::confinement_available()
        .map_err(|error| format!("unconfined_host: {error}"))?;
    let identity = Arc::new(load_identity(&config.key_file)?);
    let liveness = Liveness::from_env(PROBE_VAR, RENEW_VAR)?;
    let runner = runner()?;
    eprintln!("runner  {}", runner.pubkey());
    eprintln!("relay   {}", config.relay);
    eprintln!(
        "catalog {}",
        runner
            .catalog()
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    eprintln!("agent   {}", runner.agent().digest);
    eprintln!("limits  {}", limits_line(&config.limits));
    eprintln!("usage   {}", config.usage_dir().display());
    eprintln!(
        "liveness a probe every {} s; the subscription is renewed every {} s",
        liveness.probe.as_secs_f64(),
        liveness.renew.as_secs_f64()
    );
    match runner.release_tools().await {
        Ok(released) => {
            for (name, release) in released {
                eprintln!(
                    "tool    {name} released as {}",
                    release["id"].as_str().unwrap_or_default()
                );
            }
        }
        Err(why) => eprintln!("tools   not released: {why}"),
    }
    // Logged by the runner itself the first time it reads them, and again
    // whenever the release changes.
    let _ = runner.defaults().await;
    let mut backoff = RECONNECT.0;
    loop {
        let ended = eval_runner::runner::listen(&config.relay, &identity, &runner, liveness).await;
        // A connection the relay had confirmed was working: the next one
        // starts from the shortest wait.
        if ended.subscribed {
            backoff = RECONNECT.0;
        }
        eprintln!(
            "relay: {}; reconnecting in {} s",
            ended.why,
            backoff.as_secs()
        );
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(RECONNECT.1);
    }
}
