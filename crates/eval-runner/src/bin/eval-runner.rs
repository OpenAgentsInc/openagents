//! `eval-runner`: the hosted eval runner's command.
//!
//! ```text
//! eval-runner serve            listen for hosted run requests and answer them
//! eval-runner check            load the configuration, the catalog, and the
//!                              sandbox, and say what would run; runs nothing
//! eval-runner pubkey           print the runner's public key
//! eval-runner release DIR...   release each catalog extension's test set
//!                              (its evals/) as the runner, once
//! ```
//!
//! The configuration is the environment (`deploy/eval-runner/`); read
//! `docs/deployment/eval-runner.md`.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use eval_runner::config::{Config, load_identity};
use eval_runner::runner::Runner;
use eval_runner::wire::{Blobs, Blossom, Relay, Wire};

const USAGE: &str = "usage: eval-runner serve | check | pubkey | release DIR...";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("serve") => serve().await,
        Some("check") => check().map(|()| ExitCode::SUCCESS),
        Some("pubkey") => pubkey(),
        Some("release") => release(&args[1..]).await,
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
    let blobs: Arc<dyn Blobs> = Arc::new(Blossom::new(config.blossom.as_deref(), &config.relay)?);
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
    println!("door     {}", config.door.label());
    println!(
        "decision {}",
        config
            .decision
            .as_ref()
            .map_or("none", |pin| pin.url.as_str())
    );
    println!(
        "agent    {} ({})",
        config.coder.display(),
        runner.agent().digest
    );
    for tool in &runner.catalog().tools {
        println!("catalog  {} {}", tool.name, tool.definition.id);
    }
    println!(
        "limits   {} runs per trainer per day, {} turns per day, {} suites and {} runs at once",
        config.limits.runs_per_trainer,
        config.limits.turns_per_day,
        config.limits.jobs,
        config.limits.concurrency
    );
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

async fn release(dirs: &[String]) -> Result<ExitCode, String> {
    if dirs.is_empty() {
        return Err("release needs at least one extension directory".into());
    }
    let runner = runner()?;
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
    let mut backoff = RECONNECT.0;
    loop {
        match eval_runner::runner::listen(&config.relay, &identity, &runner).await {
            Ok(()) => backoff = RECONNECT.0,
            Err(why) => eprintln!("relay: {why}; reconnecting in {} s", backoff.as_secs()),
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(RECONNECT.1);
    }
}
